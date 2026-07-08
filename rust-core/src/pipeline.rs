use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use image::{imageops::FilterType, DynamicImage};
use rayon::prelude::*;

use crate::{
    border::calculate_dynamic_border,
    collage::{
        create_final_collage_image, grid_dimensions, grid_shape, FinalCollageLayout, ProcessedTile,
    },
    color,
    config::{CollageConfig, ResolvedLayout, TargetProfileMode},
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
const MIN_TARGET_ASPECT_RATIO: f64 = 0.1;
const MAX_TARGET_ASPECT_RATIO: f64 = 10.0;
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

#[derive(Debug)]
pub struct RenderedImageReport {
    pub image: DynamicImage,
    pub processed_count: usize,
    pub failed_images: Vec<FailedImage>,
    pub warnings: Vec<String>,
    pub elapsed_ms: u128,
    pub stage_timings: Vec<StageTiming>,
}

#[derive(Debug)]
pub struct PreviewReport {
    pub output_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub final_width: u32,
    pub final_height: u32,
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
    let mut rendered = render_final_image(config, true)?;
    let target_profile = color::load_target_profile(config)?;
    let output_icc = target_profile.icc.as_deref();
    let mut outputs = Vec::new();
    let save_started = Instant::now();

    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::SavingOutput,
        message: "Saving JPEG output...".into(),
        elapsed_ms: rendered.elapsed_ms,
    });
    if config.has_overlay() {
        let wm_path = config
            .output_dir
            .join(format!("{}_collage_final_watermarked.jpg", config.prefix));
        save_user_jpeg(&rendered.image, &wm_path, config, output_icc)?;
        outputs.push(wm_path);
    } else {
        let final_path = config
            .output_dir
            .join(format!("{}_collage_final.jpg", config.prefix));
        save_user_jpeg(&rendered.image, &final_path, config, output_icc)?;
        outputs.push(final_path);
    }

    let save_elapsed_ms = save_started.elapsed().as_millis();
    let elapsed_ms = rendered.elapsed_ms + save_elapsed_ms;
    rendered.stage_timings.push(StageTiming {
        stage: stage_key(Stage::SavingOutput).into(),
        elapsed_ms: save_elapsed_ms,
        details: Vec::new(),
    });
    progress::send(&ProgressMessage::StageFinished {
        stage: Stage::SavingOutput,
        elapsed_ms: save_elapsed_ms,
        total_elapsed_ms: elapsed_ms,
        details: Vec::new(),
    });

    Ok(PipelineReport {
        outputs,
        processed_count: rendered.processed_count,
        failed_images: rendered.failed_images,
        warnings: rendered.warnings,
        elapsed_ms,
        stage_timings: rendered.stage_timings,
    })
}

pub fn render_preview(
    config: &CollageConfig,
    output_path: &Path,
    preview_long_edge: u32,
) -> Result<PreviewReport, AppError> {
    if preview_long_edge == 0 {
        return Err(AppError::Processing(
            "preview long edge must be greater than 0".into(),
        ));
    }

    let preview_started = Instant::now();
    let mut rendered = render_final_image(config, false)?;
    let final_width = rendered.image.width();
    let final_height = rendered.image.height();
    let save_started = Instant::now();

    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::SavingOutput,
        message: "Saving preview PNG...".into(),
        elapsed_ms: rendered.elapsed_ms,
    });

    let preview = rendered
        .image
        .resize(preview_long_edge, preview_long_edge, FilterType::Lanczos3);
    preview.save_with_format(output_path, image::ImageFormat::Png)?;

    let save_elapsed_ms = save_started.elapsed().as_millis();
    let elapsed_ms = preview_started.elapsed().as_millis();
    rendered.stage_timings.push(StageTiming {
        stage: stage_key(Stage::SavingOutput).into(),
        elapsed_ms: save_elapsed_ms,
        details: Vec::new(),
    });
    progress::send(&ProgressMessage::StageFinished {
        stage: Stage::SavingOutput,
        elapsed_ms: save_elapsed_ms,
        total_elapsed_ms: elapsed_ms,
        details: Vec::new(),
    });

    Ok(PreviewReport {
        output_path: output_path.to_path_buf(),
        width: preview.width(),
        height: preview.height(),
        final_width,
        final_height,
        processed_count: rendered.processed_count,
        failed_images: rendered.failed_images,
        warnings: rendered.warnings,
        elapsed_ms,
        stage_timings: rendered.stage_timings,
    })
}

pub fn render_final_image(
    config: &CollageConfig,
    check_existing_output: bool,
) -> Result<RenderedImageReport, AppError> {
    let job_start = Instant::now();
    let mut warnings = validate_config(config, check_existing_output)?;
    let target_profile = color::load_target_profile(config)?;
    let mut timer = PipelineTimer::new(job_start);
    let total = config.image_paths.len();
    let planned_cols = (total as f64).sqrt().ceil() as u32;
    let layout = resolve_layout(config)?;
    let outer_border = resolve_outer_border(config, planned_cols);
    let final_layout = FinalCollageLayout::new(total as u32, config, outer_border)?;

    progress::send(&ProgressMessage::JobStarted { total });
    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::ProcessingImages,
        message: format!("Processing {} images in parallel...", total),
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
                    let mut image_warnings = loaded.warnings;
                    processing_metrics.add_decode(decode_started.elapsed().as_millis());

                    let color_orient_started = Instant::now();
                    let (prepared, color_warnings) = color::prepare_image_with_metadata(
                        img_path,
                        loaded.image,
                        loaded.orientation,
                        loaded.icc_profile,
                        config,
                        &target_profile,
                    )?;
                    image_warnings.extend(color_warnings);
                    let rotated =
                        apply_manual_rotation(prepared, config.image_rotation_degrees(img_path));
                    processing_metrics.add_color_orient(color_orient_started.elapsed().as_millis());

                    let resize_started = Instant::now();
                    // Preserve the image-to-border ratio while resizing directly to the final tile.
                    let (virtual_w, virtual_h) = fit_long_edge(
                        rotated.width(),
                        rotated.height(),
                        layout.content_long_edge_px,
                    )?;
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

    if config.has_overlay() {
        progress::send(&ProgressMessage::StageChanged {
            stage: Stage::AddingWatermark,
            message: "Adding overlays...".into(),
            elapsed_ms: timer.total_elapsed_ms(),
        });

        if let Some(ref wm_config) = config.watermark {
            let reference_area = final_layout.position_reference_area(wm_config.position_reference);
            let (watermarked, watermark_warnings) = add_watermark_to_image(
                final_image,
                wm_config,
                config,
                &target_profile,
                reference_area,
            )?;
            final_image = watermarked;
            warnings.extend(watermark_warnings);
        }

        if let Some(ref text_config) = config.text_block {
            let reference_area =
                final_layout.position_reference_area(text_config.position_reference);
            let (texted, text_warnings) = add_text_block_to_image(
                final_image,
                text_config,
                reference_area,
                config.final_size,
            )?;
            final_image = texted;
            warnings.extend(text_warnings);
        }
        timer.finish_stage(Stage::AddingWatermark);
    }

    Ok(RenderedImageReport {
        image: final_image,
        processed_count: bordered_images.len(),
        failed_images,
        warnings,
        elapsed_ms: timer.total_elapsed_ms(),
        stage_timings: timer.stage_timings,
    })
}

fn resolve_layout(config: &CollageConfig) -> Result<ResolvedLayout, AppError> {
    config
        .resolved_layout()
        .ok_or_else(|| AppError::Processing("layout size calculation overflowed".into()))
}

fn resolve_outer_border(config: &CollageConfig, grid_cols: u32) -> u32 {
    config
        .explicit_outer_border_px()
        .unwrap_or_else(|| calculate_dynamic_border(grid_cols, config.final_size))
}

fn validate_layout_percent_config(config: &CollageConfig) -> Result<(), AppError> {
    validate_percent_range(
        "content_long_edge_percent",
        config.layout_percent.content_long_edge_percent,
        0.0,
        false,
        100.0,
        true,
    )?;
    validate_percent_range(
        "tile_border_percent",
        config.layout_percent.tile_border_percent,
        0.0,
        true,
        50.0,
        true,
    )?;
    validate_percent_range(
        "gap_x_percent",
        config.layout_percent.gap_x_percent,
        0.0,
        true,
        100.0,
        true,
    )?;
    validate_percent_range(
        "gap_y_percent",
        config.layout_percent.gap_y_percent,
        0.0,
        true,
        100.0,
        true,
    )?;
    validate_percent_range(
        "outer_border_percent",
        config.layout_percent.outer_border_percent,
        0.0,
        true,
        50.0,
        false,
    )
}

fn validate_target_aspect_ratio(config: &CollageConfig) -> Result<(), AppError> {
    let Some(target) = config.target_aspect_ratio else {
        return Ok(());
    };

    let ratio = target.normalized_ratio().ok_or_else(|| {
        AppError::Processing(
            "target aspect ratio width and height must be finite positive numbers".into(),
        )
    })?;
    if !(MIN_TARGET_ASPECT_RATIO..=MAX_TARGET_ASPECT_RATIO).contains(&ratio) {
        return Err(AppError::Processing(format!(
            "target aspect ratio must be between {}:1 and {}:1; got {:.4}:1",
            MIN_TARGET_ASPECT_RATIO, MAX_TARGET_ASPECT_RATIO, ratio
        )));
    }
    target.canvas_dimensions(config.final_size).ok_or_else(|| {
        AppError::Processing(
            "target aspect ratio could not be converted to final dimensions".into(),
        )
    })?;

    Ok(())
}

fn validate_percent_range(
    name: &str,
    value: Option<f32>,
    min: f32,
    min_inclusive: bool,
    max: f32,
    max_inclusive: bool,
) -> Result<(), AppError> {
    let Some(value) = value else {
        return Ok(());
    };
    let min_ok = if min_inclusive {
        value >= min
    } else {
        value > min
    };
    let max_ok = if max_inclusive {
        value <= max
    } else {
        value < max
    };

    if value.is_finite() && min_ok && max_ok {
        return Ok(());
    }

    let lower = if min_inclusive { ">=" } else { ">" };
    let upper = if max_inclusive { "<=" } else { "<" };
    Err(AppError::Processing(format!(
        "{} must be {} {} and {} {}; got {}",
        name, lower, min, upper, max, value
    )))
}

fn validate_config(
    config: &CollageConfig,
    check_existing_output: bool,
) -> Result<Vec<String>, AppError> {
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

    validate_layout_percent_config(config)?;
    validate_target_aspect_ratio(config)?;

    if config.final_size == 0 {
        return Err(AppError::Processing(
            "final image size must be greater than 0".into(),
        ));
    }
    let layout = resolve_layout(config)?;
    let tile_size = layout.tile_size_px;
    if tile_size == 0 {
        return Err(AppError::Processing(
            "tile size must be greater than 0".into(),
        ));
    }
    if layout.content_long_edge_px == 0 {
        return Err(AppError::Processing(
            "content long edge must be greater than 0".into(),
        ));
    }
    if config.layout_percent.tile_border_percent.is_none()
        && config.tile_border_px.is_none()
        && config.border_size == 0
    {
        return Err(AppError::Processing(
            "legacy tile border size must be greater than 0".into(),
        ));
    }
    if layout.content_long_edge_px > tile_size {
        return Err(AppError::Processing(format!(
            "resample size {} px cannot exceed tile border size {} px",
            layout.content_long_edge_px, tile_size
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
        layout.gap_x_px,
        layout.gap_y_px,
    )?;
    if collage_width > MAX_JPEG_DIMENSION || collage_height > MAX_JPEG_DIMENSION {
        return Err(AppError::Processing(format!(
            "collage dimensions {}x{} px exceed JPEG limit {} px",
            collage_width, collage_height, MAX_JPEG_DIMENSION
        )));
    }

    let outer_border = resolve_outer_border(config, planned_cols);
    let double_outer_border = outer_border
        .checked_mul(2)
        .ok_or_else(|| AppError::Processing("outer border calculation overflowed".into()))?;
    if config.final_size <= double_outer_border {
        return Err(AppError::Processing(format!(
            "final image size {} px is too small; current image count requires more than {} px",
            config.final_size, double_outer_border
        )));
    }
    let final_layout =
        FinalCollageLayout::new(config.image_paths.len() as u32, config, outer_border)?;
    ensure_rgba_allocation_safe(
        final_layout.canvas_width,
        final_layout.canvas_height,
        "final output",
    )?;
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
        if !watermark.position_x_percent.is_finite() || !watermark.position_y_percent.is_finite() {
            return Err(AppError::Processing(
                "watermark position must contain finite percentages".into(),
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

    if check_existing_output && !config.overwrite {
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
    let content_long_edge_px = config
        .resolved_layout()
        .map(|layout| layout.content_long_edge_px)
        .unwrap_or(config.resample_size);
    let memory_sensitive_cap = if content_long_edge_px >= 3500 || config.image_paths.len() > 20 {
        4
    } else {
        8
    };
    cpus.min(memory_sensitive_cap).max(1)
}

fn estimate_pipeline_rgba_bytes(config: &CollageConfig) -> u64 {
    let (planned_cols, planned_rows) = grid_shape(config.image_paths.len() as u32);
    let outer_border = resolve_outer_border(config, planned_cols);
    let layout = config.resolved_layout().unwrap_or(ResolvedLayout {
        content_long_edge_px: config.resample_size,
        tile_size_px: config.border_size,
        gap_x_px: config.gap_x_px,
        gap_y_px: config.gap_y_px,
    });
    let final_layout =
        FinalCollageLayout::new(config.image_paths.len() as u32, config, outer_border).ok();
    let final_canvas = final_layout
        .as_ref()
        .map(|layout| layout.canvas_width as u64 * layout.canvas_height as u64 * 4)
        .unwrap_or_else(|| config.final_size as u64 * config.final_size as u64 * 4);
    let tile_size = layout.tile_size_px.max(1);
    let (grid_width, grid_height) = grid_dimensions(
        planned_cols,
        planned_rows,
        tile_size,
        layout.gap_x_px,
        layout.gap_y_px,
    )
    .unwrap_or((
        planned_cols.saturating_mul(tile_size),
        planned_rows.saturating_mul(tile_size),
    ));
    let scale = final_layout
        .as_ref()
        .map(|layout| layout.scale)
        .unwrap_or_else(|| {
            let inner_size = config
                .final_size
                .saturating_sub(outer_border.saturating_mul(2))
                .max(1);
            inner_size as f64 / grid_width.max(grid_height).max(1) as f64
        });
    let max_tile_edge = (layout.content_long_edge_px as f64 * scale).ceil().max(1.0) as u64;
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
    if !text_block.position_x_percent.is_finite() || !text_block.position_y_percent.is_finite() {
        return Err(AppError::Processing(
            "text block position must contain finite percentages".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AspectRatioConfig, BackgroundColor, ColorManagementConfig, LayoutPercentConfig,
        OutputSettings, PositionReference, ProcessingMode, RenderingIntent, TargetProfileMode,
        TextAlign, TextBlockConfig, TextFontStyle, WatermarkConfig,
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
            layout_percent: Default::default(),
            final_size: 2100,
            target_aspect_ratio: None,
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
            hdr_output: false,
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
            position_reference: PositionReference::Canvas,
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
            position_reference: PositionReference::Canvas,
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
    fn render_preview_creates_png_with_overlay_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        let watermark = dir.path().join("watermark.png");
        save_test_image(&image, 20, 10);
        save_test_image(&watermark, 4, 4);

        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.prefix = "previewed".into();
        config.watermark = Some(WatermarkConfig {
            path: watermark,
            scale_percent: 100.0,
            position_reference: PositionReference::Canvas,
            position_x_percent: 80.0,
            position_y_percent: 80.0,
        });
        config.text_block = Some(TextBlockConfig {
            text: "Preview".into(),
            font_family: "sans-serif".into(),
            font_weight: 400,
            font_style: TextFontStyle::Normal,
            font_size_px: 24.0,
            line_height_px: 28.0,
            max_width_percent: 50.0,
            align: TextAlign::Left,
            text_rgba: [255, 255, 0, 255],
            background_rgba: [0, 0, 0, 0],
            padding_px: 0,
            position_reference: PositionReference::Canvas,
            position_x_percent: 50.0,
            position_y_percent: 50.0,
        });
        fs::write(
            dir.path().join("previewed_collage_final_watermarked.jpg"),
            b"existing export",
        )
        .unwrap();

        let preview_path = dir.path().join("preview.png");
        let report = render_preview(&config, &preview_path, 256).unwrap();
        let preview = image::open(&preview_path).unwrap();

        assert!(preview_path.exists());
        assert!(report.width <= 256);
        assert!(report.height <= 256);
        assert_eq!(preview.width(), report.width);
        assert_eq!(preview.height(), report.height);
        assert_eq!(report.final_width, 2100);
        assert_eq!(report.final_height, 2100);
        assert_eq!(report.processed_count, 1);
        assert!(report
            .stage_timings
            .iter()
            .any(|timing| timing.stage == "saving_output"));
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
        assert!(!report
            .warnings
            .iter()
            .any(|warning| warning.contains(".jpg 未包含")));
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
            position_reference: PositionReference::Canvas,
            position_x_percent: 50.0,
            position_y_percent: 50.0,
        });

        assert_eq!(
            expected_output_paths(&config),
            vec![dir.path().join("output_collage_final_watermarked.jpg")]
        );
    }

    #[test]
    fn text_block_position_validation_allows_values_outside_content_bounds() {
        let text_block = TextBlockConfig {
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
            position_reference: PositionReference::Content,
            position_x_percent: -45.0,
            position_y_percent: 180.0,
        };

        assert!(validate_text_block(&text_block).is_ok());
    }

    #[test]
    fn text_block_position_validation_rejects_non_finite_values() {
        let text_block = TextBlockConfig {
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
            position_reference: PositionReference::Content,
            position_x_percent: f32::NAN,
            position_y_percent: 50.0,
        };

        assert!(validate_text_block(&text_block).is_err());
    }

    #[test]
    fn custom_outer_border_validation_overrides_dynamic_border() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        save_test_image(&image, 20, 10);
        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.final_size = 80;
        config.outer_border_px = Some(10);

        assert!(validate_config(&config, true).is_ok());
    }

    #[test]
    fn percent_layout_config_runs_pipeline() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.jpg");
        let second = dir.path().join("second.jpg");
        save_test_image(&first, 20, 10);
        save_test_image(&second, 10, 20);

        let mut config = base_config(dir.path().to_path_buf(), vec![first, second]);
        config.prefix = "percent".into();
        config.final_size = 1000;
        config.layout_percent = LayoutPercentConfig {
            content_long_edge_percent: Some(40.0),
            tile_border_percent: Some(1.0),
            gap_x_percent: Some(0.0),
            gap_y_percent: Some(0.0),
            outer_border_percent: Some(10.0),
        };

        let report = run(&config).unwrap();

        assert_eq!(report.processed_count, 2);
        assert_eq!(
            report.outputs,
            vec![dir.path().join("percent_collage_final.jpg")]
        );
        assert!(report.outputs[0].exists());
    }

    #[test]
    fn percent_layout_validation_rejects_out_of_range_values() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        save_test_image(&image, 20, 10);
        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.layout_percent.content_long_edge_percent = Some(0.0);

        let err = validate_config(&config, true).unwrap_err();

        assert!(err
            .to_string()
            .contains("content_long_edge_percent must be > 0"));
    }

    #[test]
    fn dynamic_outer_border_scales_with_final_size() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        save_test_image(&image, 20, 10);
        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.final_size = 20_000;

        assert_eq!(resolve_outer_border(&config, 2), 2000);
        assert_eq!(resolve_outer_border(&config, 10), 400);
    }

    #[test]
    fn auto_aspect_keeps_existing_collage_shape() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.jpg");
        let second = dir.path().join("second.jpg");
        save_test_image(&first, 20, 10);
        save_test_image(&second, 20, 10);
        let mut config = base_config(dir.path().to_path_buf(), vec![first, second]);
        config.final_size = 1000;
        config.outer_border_px = Some(0);

        let rendered = render_final_image(&config, false).unwrap();

        assert_eq!(rendered.image.width(), 1000);
        assert_eq!(rendered.image.height(), 500);
    }

    #[test]
    fn target_aspect_ratio_sets_final_canvas_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        save_test_image(&image, 20, 10);
        let cases = [
            (3.0, 4.0, 750, 1000),
            (4.0, 3.0, 1000, 750),
            (1.0, 1.0, 1000, 1000),
            (1.91, 1.0, 1000, 524),
        ];

        for (width, height, expected_width, expected_height) in cases {
            let mut config = base_config(dir.path().to_path_buf(), vec![image.clone()]);
            config.final_size = 1000;
            config.outer_border_px = Some(0);
            config.target_aspect_ratio = Some(AspectRatioConfig { width, height });

            let rendered = render_final_image(&config, false).unwrap();

            assert_eq!(
                (rendered.image.width(), rendered.image.height()),
                (expected_width, expected_height)
            );
        }
    }

    #[test]
    fn target_aspect_ratio_validation_rejects_extreme_values() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        save_test_image(&image, 20, 10);
        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.target_aspect_ratio = Some(AspectRatioConfig {
            width: 1000.0,
            height: 1.0,
        });

        let err = validate_config(&config, true).unwrap_err();

        assert!(err.to_string().contains("target aspect ratio"));
    }

    #[test]
    fn target_aspect_ratio_validation_rejects_border_larger_than_short_side() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        save_test_image(&image, 20, 10);
        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.final_size = 1000;
        config.outer_border_px = Some(400);
        config.target_aspect_ratio = Some(AspectRatioConfig {
            width: 3.0,
            height: 4.0,
        });

        let err = validate_config(&config, true).unwrap_err();

        assert!(err.to_string().contains("too small"));
    }
}
