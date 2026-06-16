use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use rayon::prelude::*;

use crate::{
    border::calculate_dynamic_border,
    collage::{
        create_final_collage_image, grid_dimensions, grid_shape, FinalCollageLayout, ProcessedTile,
    },
    color,
    config::{CollageConfig, TargetProfileMode},
    error::AppError,
    image_loader::{ensure_rgba_allocation_safe, load_image},
    image_proc::{apply_manual_rotation, fit_long_edge, resize_high_quality_with_options},
    jpeg_output::save_user_jpeg,
    progress::{self, FailedImage, ProgressMessage, Stage, StageTiming, StageTimingDetail},
    text_block::add_text_block_to_image,
    watermark::add_watermark_to_image,
};

const RECOMMENDED_MAX_IMAGES: usize = 40;
const HARD_MAX_IMAGES: usize = 500;
const MAX_JPEG_DIMENSION: u32 = u16::MAX as u32;
const MIN_WATERMARK_SCALE_PERCENT: f32 = 10.0;
const MAX_WATERMARK_SCALE_PERCENT: f32 = 300.0;
const WARN_ESTIMATED_RGBA_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const HARD_ESTIMATED_RGBA_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug)]
pub struct PipelineReport {
    pub outputs: Vec<PathBuf>,
    pub processed_count: usize,
    pub failed_images: Vec<FailedImage>,
    pub warnings: Vec<String>,
    pub elapsed_ms: u128,
    pub stage_timings: Vec<StageTiming>,
}

struct ProcessedImage {
    tile: ProcessedTile,
    warnings: Vec<String>,
    missing_icc: bool,
}

#[derive(Default)]
struct ProcessingStageMetrics {
    decode_ms: AtomicU64,
    color_orient_ms: AtomicU64,
    resize_ms: AtomicU64,
}

impl ProcessingStageMetrics {
    fn add_decode(&self, elapsed_ms: u128) {
        self.decode_ms
            .fetch_add(elapsed_ms.min(u64::MAX as u128) as u64, Ordering::Relaxed);
    }

    fn add_color_orient(&self, elapsed_ms: u128) {
        self.color_orient_ms
            .fetch_add(elapsed_ms.min(u64::MAX as u128) as u64, Ordering::Relaxed);
    }

    fn add_resize(&self, elapsed_ms: u128) {
        self.resize_ms
            .fetch_add(elapsed_ms.min(u64::MAX as u128) as u64, Ordering::Relaxed);
    }

    fn details(&self) -> Vec<StageTimingDetail> {
        vec![
            StageTimingDetail {
                name: "decode".into(),
                elapsed_ms: self.decode_ms.load(Ordering::Relaxed) as u128,
            },
            StageTimingDetail {
                name: "color_orient".into(),
                elapsed_ms: self.color_orient_ms.load(Ordering::Relaxed) as u128,
            },
            StageTimingDetail {
                name: "resize".into(),
                elapsed_ms: self.resize_ms.load(Ordering::Relaxed) as u128,
            },
        ]
    }
}

struct PipelineTimer {
    job_start: Instant,
    stage_start: Instant,
    stage_timings: Vec<StageTiming>,
}

impl PipelineTimer {
    fn new(job_start: Instant) -> Self {
        Self {
            job_start,
            stage_start: Instant::now(),
            stage_timings: Vec::new(),
        }
    }

    fn total_elapsed_ms(&self) -> u128 {
        self.job_start.elapsed().as_millis()
    }

    fn finish_stage(&mut self, stage: Stage) {
        self.finish_stage_with_details(stage, Vec::new());
    }

    fn finish_stage_with_details(&mut self, stage: Stage, details: Vec<StageTimingDetail>) {
        let elapsed_ms = self.stage_start.elapsed().as_millis();
        self.stage_timings.push(StageTiming {
            stage: stage_key(stage).into(),
            elapsed_ms,
            details: details.clone(),
        });
        progress::send(&ProgressMessage::StageFinished {
            stage,
            elapsed_ms,
            total_elapsed_ms: self.total_elapsed_ms(),
            details,
        });
        self.stage_start = Instant::now();
    }
}

fn stage_key(stage: Stage) -> &'static str {
    match stage {
        Stage::ProcessingImages => "processing_images",
        Stage::CreatingCollage => "creating_collage",
        Stage::AddingBorder => "adding_border",
        Stage::AddingWatermark => "adding_watermark",
        Stage::SavingOutput => "saving_output",
    }
}

pub fn run(config: &CollageConfig) -> Result<PipelineReport, AppError> {
    let job_start = Instant::now();
    let mut warnings = validate_config(config)?;
    let target_profile = color::load_target_profile(config)?;
    let output_icc = target_profile.icc.as_deref();
    let mut timer = PipelineTimer::new(job_start);
    let total = config.image_paths.len();
    let planned_cols = (total as f64).sqrt().ceil() as u32;
    let outer_border = config
        .outer_border_px
        .unwrap_or_else(|| calculate_dynamic_border(planned_cols));
    let final_layout = FinalCollageLayout::new(total as u32, config, outer_border)?;

    progress::send(&ProgressMessage::JobStarted {
        total,
    });
    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::ProcessingImages,
        message: format!(
            "Processing {} images in parallel...",
            total
        ),
        elapsed_ms: timer.total_elapsed_ms(),
    });

    let counter = Arc::new(AtomicUsize::new(0));
    let processing_metrics = Arc::new(ProcessingStageMetrics::default());
    let thread_count = processing_thread_count(config);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build()
        .map_err(|e| AppError::Processing(format!("failed to create image worker pool: {}", e)))?;
    let results: Vec<Result<ProcessedImage, FailedImage>> = pool.install(|| {
        config
            .image_paths
            .par_iter()
            .enumerate()
            .map(|(image_index, img_path)| {
                let result: Result<ProcessedImage, AppError> = (|| {
                    let decode_started = Instant::now();
                    let loaded = load_image(img_path)?;
                    let missing_icc = loaded.icc_profile.is_none();
                    processing_metrics.add_decode(decode_started.elapsed().as_millis());

                    let color_orient_started = Instant::now();
                    let (prepared, image_warnings) = color::prepare_image_with_metadata(
                        img_path,
                        loaded.image,
                        loaded.orientation,
                        loaded.icc_profile,
                        config,
                        &target_profile,
                    )?;
                    let rotated =
                        apply_manual_rotation(prepared, config.image_rotation_degrees(img_path));
                    processing_metrics
                        .add_color_orient(color_orient_started.elapsed().as_millis());

                    let resize_started = Instant::now();
                    // Preserve the image-to-border ratio while resizing directly to the final tile.
                    let (virtual_w, virtual_h) =
                        fit_long_edge(rotated.width(), rotated.height(), config.resample_size)?;
                    let placement =
                        final_layout.tile_placement(image_index as u32, virtual_w, virtual_h);
                    let resampled = resize_high_quality_with_options(
                        rotated,
                        placement.width,
                        placement.height,
                        config.linear_light_resize(),
                    )?;
                    processing_metrics.add_resize(resize_started.elapsed().as_millis());
                    Ok(ProcessedImage {
                        tile: ProcessedTile {
                            image: resampled.into_rgba8(),
                            x: placement.x,
                            y: placement.y,
                        },
                        warnings: image_warnings,
                        missing_icc,
                    })
                })();

                let done = counter.fetch_add(1, Ordering::SeqCst) + 1;
                progress::send(&ProgressMessage::ImageProcessed {
                    index: done,
                    total,
                    elapsed_ms: timer.total_elapsed_ms(),
                });

                result.map_err(|e| FailedImage {
                    path: img_path.to_string_lossy().into_owned(),
                    message: e.to_string(),
                })
            })
            .collect()
    });
    timer.finish_stage_with_details(Stage::ProcessingImages, processing_metrics.details());

    let mut bordered_images: Vec<ProcessedTile> = Vec::new();
    let mut failed_images: Vec<FailedImage> = Vec::new();
    let mut missing_icc_count = 0usize;
    for result in results {
        match result {
            Ok(processed) => {
                if processed.missing_icc {
                    missing_icc_count += 1;
                }
                bordered_images.push(processed.tile);
                warnings.extend(processed.warnings);
            }
            Err(failed) => {
                eprintln!(
                    "single image processing failed: {}: {}",
                    failed.path, failed.message
                );
                failed_images.push(failed);
            }
        }
    }

    if bordered_images.is_empty() {
        return Err(AppError::NoImagesProcessed);
    }
    if missing_icc_count > 0 {
        warnings.push(format!(
            "{} 张图片未包含 ICC profile，已按 sRGB 处理",
            missing_icc_count
        ));
    }
    if !failed_images.is_empty() {
        warnings.push(format!(
            "{} image(s) failed; continued with {} successfully processed image(s)",
            failed_images.len(),
            bordered_images.len()
        ));
    }

    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::CreatingCollage,
        message: "贴入最终画布...".into(),
        elapsed_ms: timer.total_elapsed_ms(),
    });
    let mut final_image = create_final_collage_image(&bordered_images, config, &final_layout)?;
    timer.finish_stage(Stage::CreatingCollage);

    let mut outputs = Vec::new();
    if config.has_overlay() {
        progress::send(&ProgressMessage::StageChanged {
            stage: Stage::AddingWatermark,
            message: "Adding overlays...".into(),
            elapsed_ms: timer.total_elapsed_ms(),
        });

        if let Some(ref wm_config) = config.watermark {
            let (watermarked, watermark_warnings) =
                add_watermark_to_image(final_image, wm_config, config, &target_profile)?;
            final_image = watermarked;
            warnings.extend(watermark_warnings);
        }

        if let Some(ref text_config) = config.text_block {
            let (texted, text_warnings) = add_text_block_to_image(final_image, text_config)?;
            final_image = texted;
            warnings.extend(text_warnings);
        }
        timer.finish_stage(Stage::AddingWatermark);
    }

    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::SavingOutput,
        message: "Saving JPEG output...".into(),
        elapsed_ms: timer.total_elapsed_ms(),
    });
    if config.has_overlay() {
        let wm_path = config
            .output_dir
            .join(format!("{}_collage_final_watermarked.jpg", config.prefix));
        save_user_jpeg(&final_image, &wm_path, config, output_icc)?;
        outputs.push(wm_path);
    } else {
        let final_path = config
            .output_dir
            .join(format!("{}_collage_final.jpg", config.prefix));
        save_user_jpeg(&final_image, &final_path, config, output_icc)?;
        outputs.push(final_path);
    }
    timer.finish_stage(Stage::SavingOutput);

    Ok(PipelineReport {
        outputs,
        processed_count: bordered_images.len(),
        failed_images,
        warnings,
        elapsed_ms: timer.total_elapsed_ms(),
        stage_timings: timer.stage_timings,
    })
}

fn validate_config(config: &CollageConfig) -> Result<Vec<String>, AppError> {
    let mut warnings = Vec::new();
    if config.image_paths.is_empty() {
        return Err(AppError::Processing("select at least one image".into()));
    }
    if config.image_paths.len() > HARD_MAX_IMAGES {
        return Err(AppError::Processing(format!(
            "image count cannot exceed {}; got {}",
            HARD_MAX_IMAGES,
            config.image_paths.len()
        )));
    }
    if config.image_paths.len() > RECOMMENDED_MAX_IMAGES {
        warnings.push(format!(
            "{} images selected; high resolution settings may use substantial memory",
            config.image_paths.len()
        ));
    }

    for (path, degrees) in &config.image_rotations {
        if !matches!(degrees, 0 | 90 | 180 | 270) {
            return Err(AppError::Processing(format!(
                "image rotation must be 0, 90, 180, or 270 degrees: {} = {}",
                path.display(),
                degrees
            )));
        }
        if !config
            .image_paths
            .iter()
            .any(|image_path| image_path == path)
        {
            return Err(AppError::Processing(format!(
                "rotation config contains an unselected image: {}",
                path.display()
            )));
        }
    }

    let trimmed_prefix = config.prefix.trim();
    if trimmed_prefix.is_empty() {
        return Err(AppError::Processing(
            "output filename prefix cannot be empty".into(),
        ));
    }
    if trimmed_prefix != config.prefix {
        return Err(AppError::Processing(
            "output filename prefix cannot contain leading or trailing whitespace".into(),
        ));
    }
    if config.prefix.chars().any(|c| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    }) {
        return Err(AppError::Processing(
            "output filename prefix contains illegal filename characters".into(),
        ));
    }

    let output_meta = std::fs::metadata(&config.output_dir).map_err(|e| {
        AppError::Processing(format!(
            "output directory is not accessible: {} ({})",
            config.output_dir.display(),
            e
        ))
    })?;
    if !output_meta.is_dir() {
        return Err(AppError::Processing(format!(
            "output path is not a directory: {}",
            config.output_dir.display()
        )));
    }

    if config.resample_size == 0 || config.final_size == 0 {
        return Err(AppError::Processing(
            "size parameters must be greater than 0".into(),
        ));
    }
    let tile_size = config
        .tile_size()
        .ok_or_else(|| AppError::Processing("tile size calculation overflowed".into()))?;
    if tile_size == 0 {
        return Err(AppError::Processing("tile size must be greater than 0".into()));
    }
    if config.tile_border_px.is_none() && config.border_size == 0 {
        return Err(AppError::Processing(
            "legacy tile border size must be greater than 0".into(),
        ));
    }
    if config.resample_size > tile_size {
        return Err(AppError::Processing(format!(
            "resample size {} px cannot exceed tile border size {} px",
            config.resample_size, tile_size
        )));
    }
    if config.final_size > MAX_JPEG_DIMENSION {
        return Err(AppError::Processing(format!(
            "final image size {} px exceeds JPEG limit {} px",
            config.final_size, MAX_JPEG_DIMENSION
        )));
    }
    if config.dpi == 0 || config.dpi > u16::MAX as u32 {
        return Err(AppError::Processing(format!(
            "DPI must be between 1 and {}; got {}",
            u16::MAX,
            config.dpi
        )));
    }
    if !(1..=100).contains(&config.output_settings.jpeg_quality) {
        return Err(AppError::Processing(format!(
            "JPEG quality must be between 1 and 100; got {}",
            config.output_settings.jpeg_quality
        )));
    }

    let (planned_cols, planned_rows) = grid_shape(config.image_paths.len() as u32);
    let (collage_width, collage_height) = grid_dimensions(
        planned_cols,
        planned_rows,
        tile_size,
        config.gap_x_px,
        config.gap_y_px,
    )?;
    if collage_width > MAX_JPEG_DIMENSION || collage_height > MAX_JPEG_DIMENSION {
        return Err(AppError::Processing(format!(
            "collage dimensions {}x{} px exceed JPEG limit {} px",
            collage_width, collage_height, MAX_JPEG_DIMENSION
        )));
    }

    let outer_border = config
        .outer_border_px
        .unwrap_or_else(|| calculate_dynamic_border(planned_cols));
    let double_outer_border = outer_border
        .checked_mul(2)
        .ok_or_else(|| AppError::Processing("outer border calculation overflowed".into()))?;
    if config.final_size <= double_outer_border {
        return Err(AppError::Processing(format!(
            "final image size {} px is too small; current image count requires more than {} px",
            config.final_size,
            double_outer_border
        )));
    }
    ensure_rgba_allocation_safe(config.final_size, config.final_size, "final output")?;
    let estimated_rgba_bytes = estimate_pipeline_rgba_bytes(config);
    if estimated_rgba_bytes > HARD_ESTIMATED_RGBA_BYTES {
        return Err(AppError::Processing(format!(
            "estimated RGBA working set is {:.2}GB, above the {:.2}GB safety limit; reduce image count or size settings",
            bytes_to_gib(estimated_rgba_bytes),
            bytes_to_gib(HARD_ESTIMATED_RGBA_BYTES)
        )));
    }
    if estimated_rgba_bytes > WARN_ESTIMATED_RGBA_BYTES {
        warnings.push(format!(
            "estimated RGBA working set is {:.2}GB; processing will limit worker concurrency",
            bytes_to_gib(estimated_rgba_bytes)
        ));
    }

    if let Some(watermark) = &config.watermark {
        let wm_meta = std::fs::metadata(&watermark.path).map_err(|e| {
            AppError::Processing(format!(
                "watermark image is not accessible: {} ({})",
                watermark.path.display(),
                e
            ))
        })?;
        if !wm_meta.is_file() {
            return Err(AppError::Processing(format!(
                "watermark path is not a file: {}",
                watermark.path.display()
            )));
        }
        if !(MIN_WATERMARK_SCALE_PERCENT..=MAX_WATERMARK_SCALE_PERCENT)
            .contains(&watermark.scale_percent)
        {
            return Err(AppError::Processing(format!(
                "watermark scale must be between {} and {} percent",
                MIN_WATERMARK_SCALE_PERCENT, MAX_WATERMARK_SCALE_PERCENT
            )));
        }
        if !watermark.position_x_percent.is_finite()
            || !watermark.position_y_percent.is_finite()
            || !(0.0..=100.0).contains(&watermark.position_x_percent)
            || !(0.0..=100.0).contains(&watermark.position_y_percent)
        {
            return Err(AppError::Processing(
                "watermark position must be between 0 and 100 percent".into(),
            ));
        }
    }

    if let Some(text_block) = &config.text_block {
        validate_text_block(text_block)?;
    }

    if config.color_management.enabled
        && config.color_management.target_profile == TargetProfileMode::Custom
    {
        let path = config
            .color_management
            .target_profile_path
            .as_ref()
            .ok_or_else(|| AppError::Processing("select a target ICC profile file".into()))?;
        if !path.is_file() {
            return Err(AppError::Processing(format!(
                "target ICC profile is not accessible: {}",
                path.display()
            )));
        }
    }

    if !config.overwrite {
        let existing: Vec<String> = expected_output_paths(config)
            .into_iter()
            .filter(|path| path.exists())
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        if !existing.is_empty() {
            return Err(AppError::Processing(format!(
                "output file already exists; change prefix or output directory: {}",
                existing.join(", ")
            )));
        }
    }

    Ok(warnings)
}

fn processing_thread_count(config: &CollageConfig) -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let memory_sensitive_cap = if config.resample_size >= 3500 || config.image_paths.len() > 20 {
        4
    } else {
        8
    };
    cpus.min(memory_sensitive_cap).max(1)
}

fn estimate_pipeline_rgba_bytes(config: &CollageConfig) -> u64 {
    let final_canvas = config.final_size as u64 * config.final_size as u64 * 4;
    let (planned_cols, planned_rows) = grid_shape(config.image_paths.len() as u32);
    let outer_border = config
        .outer_border_px
        .unwrap_or_else(|| calculate_dynamic_border(planned_cols));
    let tile_size = config.tile_size().unwrap_or(config.border_size).max(1);
    let (grid_width, grid_height) = grid_dimensions(
        planned_cols,
        planned_rows,
        tile_size,
        config.gap_x_px,
        config.gap_y_px,
    )
    .unwrap_or((planned_cols.saturating_mul(tile_size), planned_rows.saturating_mul(tile_size)));
    let inner_size = config
        .final_size
        .saturating_sub(outer_border.saturating_mul(2))
        .max(1);
    let scale = inner_size as f64 / grid_width.max(grid_height).max(1) as f64;
    let max_tile_edge = (config.resample_size as f64 * scale).ceil().max(1.0) as u64;
    let per_tile = max_tile_edge * max_tile_edge * 4;
    let tile_cache = per_tile.saturating_mul(config.image_paths.len() as u64);
    final_canvas.saturating_add(tile_cache)
}

fn bytes_to_gib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0 / 1024.0
}

fn expected_output_paths(config: &CollageConfig) -> Vec<PathBuf> {
    if config.has_overlay() {
        vec![config
            .output_dir
            .join(format!("{}_collage_final_watermarked.jpg", config.prefix))]
    } else {
        vec![config
            .output_dir
            .join(format!("{}_collage_final.jpg", config.prefix))]
    }
}

fn validate_text_block(text_block: &crate::config::TextBlockConfig) -> Result<(), AppError> {
    if text_block.text.trim().is_empty() {
        return Err(AppError::Processing("text block cannot be empty".into()));
    }
    if !(1..=999).contains(&text_block.font_weight) {
        return Err(AppError::Processing(
            "text block font weight must be between 1 and 999".into(),
        ));
    }
    if !text_block.font_size_px.is_finite()
        || !text_block.line_height_px.is_finite()
        || text_block.font_size_px <= 0.0
        || text_block.line_height_px <= 0.0
    {
        return Err(AppError::Processing(
            "text block font size and line height must be greater than 0".into(),
        ));
    }
    if !text_block.max_width_percent.is_finite()
        || !(1.0..=100.0).contains(&text_block.max_width_percent)
    {
        return Err(AppError::Processing(
            "text block max width must be between 1 and 100 percent".into(),
        ));
    }
    if !text_block.position_x_percent.is_finite()
        || !text_block.position_y_percent.is_finite()
        || !(0.0..=100.0).contains(&text_block.position_x_percent)
        || !(0.0..=100.0).contains(&text_block.position_y_percent)
    {
        return Err(AppError::Processing(
            "text block position must be between 0 and 100 percent".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        BackgroundColor, ColorManagementConfig, OutputSettings, ProcessingMode, RenderingIntent,
        TargetProfileMode, TextAlign, TextBlockConfig, TextFontStyle, WatermarkConfig,
    };
    use image::{DynamicImage, ImageBuffer};
    use std::fs;
    use std::path::Path;
    use std::time::Instant;

    fn base_config(output_dir: PathBuf, image_paths: Vec<PathBuf>) -> CollageConfig {
        CollageConfig {
            image_paths,
            image_rotations: Default::default(),
            processing_mode: ProcessingMode::StandardHighQuality,
            output_dir,
            prefix: "output".into(),
            resample_size: 40,
            border_size: 60,
            tile_border_px: None,
            gap_x_px: 0,
            gap_y_px: 0,
            outer_border_px: None,
            final_size: 2100,
            dpi: 300,
            background_color: BackgroundColor::White,
            watermark: None,
            text_block: None,
            overwrite: false,
            output_settings: OutputSettings {
                jpeg_quality: 95,
                auto_orient: true,
                linear_light_resize: Some(false),
            },
            color_management: ColorManagementConfig {
                enabled: true,
                target_profile: TargetProfileMode::Srgb,
                target_profile_path: None,
                rendering_intent: RenderingIntent::Perceptual,
            },
        }
    }

    fn save_test_image(path: &Path, width: u32, height: u32) {
        let img = DynamicImage::ImageRgb8(ImageBuffer::from_fn(width, height, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, 180])
        }));
        img.save(path).unwrap();
    }

    fn read_jfif_dpi(path: &Path) -> Option<u16> {
        let data = fs::read(path).ok()?;
        if data.len() < 16 || data[0] != 0xFF || data[1] != 0xD8 {
            return None;
        }
        if data[2] == 0xFF && data[3] == 0xE0 && &data[6..11] == b"JFIF\0" {
            return Some(u16::from_be_bytes([data[14], data[15]]));
        }
        None
    }

    #[test]
    fn run_creates_final_output_without_intermediate_files() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.jpg");
        let second = dir.path().join("second.jpg");
        save_test_image(&first, 20, 10);
        save_test_image(&second, 10, 20);

        let mut config = base_config(dir.path().to_path_buf(), vec![first, second]);
        config.prefix = "streamed".into();

        let report = run(&config).unwrap();

        assert_eq!(report.processed_count, 2);
        assert_eq!(
            report.outputs,
            vec![dir.path().join("streamed_collage_final.jpg")]
        );
        assert!(report.outputs[0].exists());
        assert_eq!(read_jfif_dpi(&report.outputs[0]), Some(300));
        assert!(report
            .stage_timings
            .iter()
            .any(|timing| timing.stage == "processing_images"));
        assert!(report
            .stage_timings
            .iter()
            .any(|timing| timing.stage == "saving_output"));
        assert!(!dir.path().join("streamed_collage.jpg").exists());
        assert!(!dir
            .path()
            .join("streamed_collage_final_watermarked.jpg")
            .exists());
    }

    #[test]
    fn run_creates_watermarked_output_without_unwatermarked_intermediate() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        let watermark = dir.path().join("watermark.png");
        save_test_image(&image, 20, 10);
        save_test_image(&watermark, 4, 4);

        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.prefix = "watermarked".into();
        config.watermark = Some(WatermarkConfig {
            path: watermark,
            scale_percent: 100.0,
            position_x_percent: 50.0,
            position_y_percent: 50.0,
        });

        let report = run(&config).unwrap();

        assert_eq!(
            report.outputs,
            vec![dir.path().join("watermarked_collage_final_watermarked.jpg")]
        );
        assert!(report.outputs[0].exists());
        assert!(!dir.path().join("watermarked_collage.jpg").exists());
        assert!(!dir.path().join("watermarked_collage_final.jpg").exists());
    }

    #[test]
    fn run_creates_text_block_output_with_custom_spacing() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        save_test_image(&image, 20, 10);

        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.prefix = "texted".into();
        config.tile_border_px = Some(8);
        config.gap_x_px = 4;
        config.gap_y_px = 6;
        config.outer_border_px = Some(20);
        config.text_block = Some(TextBlockConfig {
            text: "Hello\n中文".into(),
            font_family: "sans-serif".into(),
            font_weight: 400,
            font_style: TextFontStyle::Normal,
            font_size_px: 28.0,
            line_height_px: 34.0,
            max_width_percent: 80.0,
            align: TextAlign::Center,
            text_rgba: [255, 255, 255, 255],
            background_rgba: [0, 0, 0, 128],
            padding_px: 4,
            position_x_percent: 50.0,
            position_y_percent: 50.0,
        });

        let report = run(&config).unwrap();

        assert_eq!(
            report.outputs,
            vec![dir.path().join("texted_collage_final_watermarked.jpg")]
        );
        assert!(report.outputs[0].exists());
        assert!(!dir.path().join("texted_collage_final.jpg").exists());
    }

    #[test]
    fn run_reports_partial_image_failures() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.jpg");
        let bad = dir.path().join("bad.jpg");
        save_test_image(&good, 20, 10);
        fs::write(&bad, b"not an image").unwrap();

        let mut config = base_config(dir.path().to_path_buf(), vec![good, bad.clone()]);
        config.prefix = "partial".into();

        let report = run(&config).unwrap();

        assert_eq!(report.processed_count, 1);
        assert_eq!(report.failed_images.len(), 1);
        assert_eq!(report.failed_images[0].path, bad.to_string_lossy());
        assert_eq!(
            report.outputs,
            vec![dir.path().join("partial_collage_final.jpg")]
        );
        assert!(report.outputs[0].exists());
    }

    #[test]
    fn run_summarizes_missing_icc_warnings() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.jpg");
        let second = dir.path().join("second.jpg");
        save_test_image(&first, 20, 10);
        save_test_image(&second, 10, 20);

        let mut config = base_config(dir.path().to_path_buf(), vec![first, second]);
        config.prefix = "icc_summary".into();

        let report = run(&config).unwrap();
        let missing_icc_warnings: Vec<&String> = report
            .warnings
            .iter()
            .filter(|warning| warning.contains("未包含 ICC profile"))
            .collect();

        assert_eq!(missing_icc_warnings.len(), 1);
        assert!(missing_icc_warnings[0].starts_with("2 张图片"));
        assert!(!report.warnings.iter().any(|warning| warning.contains(".jpg 未包含")));
    }

    #[test]
    #[ignore = "manual performance smoke: cargo test benchmark_small_pipeline -- --ignored --nocapture"]
    fn benchmark_small_pipeline_smoke() {
        let dir = tempfile::tempdir().unwrap();
        let images: Vec<PathBuf> = (0..12)
            .map(|i| {
                let path = dir.path().join(format!("image_{}.jpg", i));
                save_test_image(&path, 1200 + i * 7, 900 + i * 5);
                path
            })
            .collect();
        let mut config = base_config(dir.path().to_path_buf(), images);
        config.prefix = "bench".into();
        config.resample_size = 800;
        config.border_size = 900;
        config.final_size = 3000;

        let start = Instant::now();
        let report = run(&config).unwrap();
        eprintln!(
            "benchmark_small_pipeline: {:?}, outputs={:?}, processed={}",
            start.elapsed(),
            report.outputs,
            report.processed_count
        );
    }

    #[test]
    fn expected_output_path_uses_watermarked_name_for_text_block() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = base_config(dir.path().to_path_buf(), vec![dir.path().join("image.jpg")]);
        config.text_block = Some(TextBlockConfig {
            text: "caption".into(),
            font_family: "sans-serif".into(),
            font_weight: 400,
            font_style: TextFontStyle::Normal,
            font_size_px: 24.0,
            line_height_px: 30.0,
            max_width_percent: 50.0,
            align: TextAlign::Center,
            text_rgba: [255, 255, 255, 255],
            background_rgba: [0, 0, 0, 0],
            padding_px: 0,
            position_x_percent: 50.0,
            position_y_percent: 50.0,
        });

        assert_eq!(
            expected_output_paths(&config),
            vec![dir.path().join("output_collage_final_watermarked.jpg")]
        );
    }

    #[test]
    fn custom_outer_border_validation_overrides_dynamic_border() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        save_test_image(&image, 20, 10);
        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.final_size = 80;
        config.outer_border_px = Some(10);

        assert!(validate_config(&config).is_ok());
    }
}
