use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rayon::prelude::*;

use crate::{
    border::{add_final_border_and_resize, calculate_dynamic_border},
    collage::create_collage,
    color,
    config::{CollageConfig, TargetProfileMode},
    error::AppError,
    image_loader::open_image,
    image_proc::{add_square_border, apply_manual_rotation, resample},
    progress::{self, FailedImage, ProgressMessage, Stage},
    watermark::add_watermark,
};

const RECOMMENDED_MAX_IMAGES: usize = 30;
const HARD_MAX_IMAGES: usize = 500;
const MAX_JPEG_DIMENSION: u32 = u16::MAX as u32;
const MIN_WATERMARK_SCALE_PERCENT: f32 = 10.0;
const MAX_WATERMARK_SCALE_PERCENT: f32 = 300.0;

#[derive(Debug)]
pub struct PipelineReport {
    pub outputs: Vec<PathBuf>,
    pub processed_count: usize,
    pub failed_images: Vec<FailedImage>,
    pub warnings: Vec<String>,
}

struct ProcessedImage {
    path: PathBuf,
    warnings: Vec<String>,
}

pub fn run(config: &CollageConfig) -> Result<PipelineReport, AppError> {
    let mut warnings = validate_config(config)?;
    let target_profile = color::load_target_profile(config)?;
    let output_icc = target_profile.icc.as_deref();

    // 阶段 1：并行处理单图
    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::ProcessingImages,
        message: format!("正在并行处理 {} 张图片...", config.image_paths.len()),
    });

    let total = config.image_paths.len();
    let counter = Arc::new(AtomicUsize::new(0));
    let processed_dir = tempfile::tempdir()?;

    let results: Vec<Result<ProcessedImage, FailedImage>> = config
        .image_paths
        .par_iter()
        .enumerate()
        .map(|(index, img_path)| {
            let result: Result<ProcessedImage, AppError> = (|| {
                let img = open_image(img_path)?;
                let (prepared, image_warnings) =
                    color::prepare_image(img_path, img, config, &target_profile)?;
                let rotated =
                    apply_manual_rotation(prepared, config.image_rotation_degrees(img_path));
                let resampled = resample(rotated, config.resample_size);
                let bordered =
                    add_square_border(resampled, config.border_size, &config.background_color);
                let temp_path = processed_dir
                    .path()
                    .join(format!("processed_{:04}.png", index));
                bordered.save(&temp_path)?;
                Ok(ProcessedImage {
                    path: temp_path,
                    warnings: image_warnings,
                })
            })();

            let done = counter.fetch_add(1, Ordering::SeqCst) + 1;
            progress::send(&ProgressMessage::ImageProcessed { index: done, total });

            result.map_err(|e| FailedImage {
                path: img_path.to_string_lossy().into_owned(),
                message: e.to_string(),
            })
        })
        .collect();

    // 分离成功/失败
    let mut bordered_images: Vec<PathBuf> = Vec::new();
    let mut failed_images: Vec<FailedImage> = Vec::new();
    let mut failed_count = 0usize;
    for r in results {
        match r {
            Ok(processed) => {
                bordered_images.push(processed.path);
                warnings.extend(processed.warnings);
            }
            Err(failed) => {
                eprintln!("处理单图失败: {}: {}", failed.path, failed.message);
                failed_images.push(failed);
                failed_count += 1;
            }
        }
    }

    if bordered_images.is_empty() {
        return Err(AppError::NoImagesProcessed);
    }
    if failed_count > 0 {
        warnings.push(format!(
            "{} 张图片处理失败，已继续处理 {} 张成功图片",
            failed_count,
            bordered_images.len()
        ));
    }

    let grid_cols = (bordered_images.len() as f64).sqrt().ceil() as u32;
    let dynamic_border = calculate_dynamic_border(grid_cols);
    if config.final_size <= dynamic_border * 2 {
        return Err(AppError::Processing(format!(
            "最终图像大小 {} px 过小；当前成功图片数量需要大于 {} px",
            config.final_size,
            dynamic_border * 2
        )));
    }

    // 阶段 2：创建拼贴图
    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::CreatingCollage,
        message: "正在创建拼贴图...".into(),
    });

    let collage_path = processed_dir
        .path()
        .join(format!("{}_collage.jpg", config.prefix));
    create_collage(&bordered_images, &collage_path, config, output_icc)?;

    // 阶段 3：添加最终边框
    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::AddingBorder,
        message: "正在添加最终边框...".into(),
    });

    let final_path = if config.watermark.is_some() {
        processed_dir
            .path()
            .join(format!("{}_collage_final.jpg", config.prefix))
    } else {
        config
            .output_dir
            .join(format!("{}_collage_final.jpg", config.prefix))
    };
    add_final_border_and_resize(
        &collage_path,
        &final_path,
        config,
        dynamic_border,
        output_icc,
    )?;

    let mut outputs = Vec::new();

    // 阶段 4：添加水印（可选）
    if let Some(ref wm_config) = config.watermark {
        progress::send(&ProgressMessage::StageChanged {
            stage: Stage::AddingWatermark,
            message: "正在添加水印...".into(),
        });

        let wm_path = config
            .output_dir
            .join(format!("{}_collage_final_watermarked.jpg", config.prefix));
        warnings.extend(add_watermark(
            &final_path,
            &wm_path,
            wm_config,
            config,
            &target_profile,
            output_icc,
        )?);
        outputs.push(wm_path);
    } else {
        outputs.push(final_path);
    }

    Ok(PipelineReport {
        outputs,
        processed_count: bordered_images.len(),
        failed_images,
        warnings,
    })
}

fn validate_config(config: &CollageConfig) -> Result<Vec<String>, AppError> {
    let mut warnings = Vec::new();

    if config.image_paths.is_empty() {
        return Err(AppError::Processing("请选择至少 1 张图片".into()));
    }

    for (path, degrees) in &config.image_rotations {
        if !matches!(degrees, 0 | 90 | 180 | 270) {
            return Err(AppError::Processing(format!(
                "图片旋转角度必须为 0、90、180 或 270 度：{} = {}",
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
                "图片旋转配置包含未选择的图片：{}",
                path.display()
            )));
        }
    }

    if config.image_paths.len() > HARD_MAX_IMAGES {
        return Err(AppError::Processing(format!(
            "图片数量不能超过 {} 张，当前选择了 {} 张",
            HARD_MAX_IMAGES,
            config.image_paths.len()
        )));
    }

    if config.image_paths.len() > RECOMMENDED_MAX_IMAGES {
        warnings.push(format!(
            "当前选择了 {} 张图片，高分辨率参数可能占用大量内存",
            config.image_paths.len()
        ));
    }

    let trimmed_prefix = config.prefix.trim();
    if trimmed_prefix.is_empty() {
        return Err(AppError::Processing("输出文件名前缀不能为空".into()));
    }
    if trimmed_prefix != config.prefix {
        return Err(AppError::Processing(
            "输出文件名前缀不能包含首尾空白".into(),
        ));
    }
    if config.prefix.chars().any(|c| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    }) {
        return Err(AppError::Processing(
            "输出文件名前缀包含非法文件名字符".into(),
        ));
    }

    let output_meta = std::fs::metadata(&config.output_dir).map_err(|e| {
        AppError::Processing(format!(
            "导出目录不可访问: {} ({})",
            config.output_dir.display(),
            e
        ))
    })?;
    if !output_meta.is_dir() {
        return Err(AppError::Processing(format!(
            "导出路径不是目录: {}",
            config.output_dir.display()
        )));
    }

    if config.resample_size == 0 || config.border_size == 0 || config.final_size == 0 {
        return Err(AppError::Processing("尺寸参数必须大于 0".into()));
    }
    if config.final_size > MAX_JPEG_DIMENSION {
        return Err(AppError::Processing(format!(
            "最终图像大小 {} px 超过 JPEG 支持上限 {} px",
            config.final_size, MAX_JPEG_DIMENSION
        )));
    }
    if config.dpi == 0 || config.dpi > u16::MAX as u32 {
        return Err(AppError::Processing(format!(
            "DPI 必须在 1-{} 之间，当前为 {}",
            u16::MAX,
            config.dpi
        )));
    }
    if !(1..=100).contains(&config.output_settings.jpeg_quality) {
        return Err(AppError::Processing(format!(
            "JPEG 质量必须在 1-100 之间，当前为 {}",
            config.output_settings.jpeg_quality
        )));
    }
    if config.resample_size > config.border_size {
        return Err(AppError::Processing(format!(
            "重采样大小 {} px 不能大于单图边框大小 {} px",
            config.resample_size, config.border_size
        )));
    }

    let planned_cols = (config.image_paths.len() as f64).sqrt().ceil() as u32;
    let planned_rows = (config.image_paths.len() as f64 / planned_cols as f64).ceil() as u32;
    let collage_width = planned_cols
        .checked_mul(config.border_size)
        .ok_or_else(|| AppError::Processing("拼贴图尺寸过大，宽度计算溢出".into()))?;
    let collage_height = planned_rows
        .checked_mul(config.border_size)
        .ok_or_else(|| AppError::Processing("拼贴图尺寸过大，高度计算溢出".into()))?;
    if collage_width > MAX_JPEG_DIMENSION || collage_height > MAX_JPEG_DIMENSION {
        return Err(AppError::Processing(format!(
            "拼贴图尺寸 {}×{} px 超过 JPEG 支持上限 {} px",
            collage_width, collage_height, MAX_JPEG_DIMENSION
        )));
    }

    let dynamic_border = calculate_dynamic_border(planned_cols);
    if config.final_size <= dynamic_border * 2 {
        return Err(AppError::Processing(format!(
            "最终图像大小 {} px 过小；当前图片数量需要大于 {} px",
            config.final_size,
            dynamic_border * 2
        )));
    }

    if let Some(watermark) = &config.watermark {
        let wm_meta = std::fs::metadata(&watermark.path).map_err(|e| {
            AppError::Processing(format!(
                "水印图片不可访问: {} ({})",
                watermark.path.display(),
                e
            ))
        })?;
        if !wm_meta.is_file() {
            return Err(AppError::Processing(format!(
                "水印路径不是文件: {}",
                watermark.path.display()
            )));
        }
        if !(MIN_WATERMARK_SCALE_PERCENT..=MAX_WATERMARK_SCALE_PERCENT)
            .contains(&watermark.scale_percent)
        {
            return Err(AppError::Processing(format!(
                "水印缩放比例必须在 {}-{}% 之间",
                MIN_WATERMARK_SCALE_PERCENT, MAX_WATERMARK_SCALE_PERCENT
            )));
        }
        if !watermark.position_x_percent.is_finite()
            || !watermark.position_y_percent.is_finite()
            || !(0.0..=100.0).contains(&watermark.position_x_percent)
            || !(0.0..=100.0).contains(&watermark.position_y_percent)
        {
            return Err(AppError::Processing("水印位置必须在 0-100% 之间".into()));
        }
    }

    if config.color_management.enabled
        && config.color_management.target_profile == TargetProfileMode::Custom
    {
        let path = config
            .color_management
            .target_profile_path
            .as_ref()
            .ok_or_else(|| AppError::Processing("请选择目标 ICC profile 文件".into()))?;
        if !path.is_file() {
            return Err(AppError::Processing(format!(
                "目标 ICC profile 不可访问: {}",
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
                "输出文件已存在，为避免覆盖请更换前缀或导出目录: {}",
                existing.join(", ")
            )));
        }
    }

    Ok(warnings)
}

fn expected_output_paths(config: &CollageConfig) -> Vec<PathBuf> {
    if config.watermark.is_some() {
        vec![config
            .output_dir
            .join(format!("{}_collage_final_watermarked.jpg", config.prefix))]
    } else {
        vec![config
            .output_dir
            .join(format!("{}_collage_final.jpg", config.prefix))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        BackgroundColor, ColorManagementConfig, OutputSettings, RenderingIntent, TargetProfileMode,
        WatermarkConfig,
    };
    use image::{DynamicImage, ImageBuffer};
    use std::fs;
    use std::path::Path;

    fn base_config(output_dir: PathBuf, image_paths: Vec<PathBuf>) -> CollageConfig {
        CollageConfig {
            image_paths,
            image_rotations: Default::default(),
            output_dir,
            prefix: "output".into(),
            resample_size: 40,
            border_size: 60,
            final_size: 2100,
            dpi: 300,
            background_color: BackgroundColor::White,
            watermark: None,
            overwrite: false,
            output_settings: OutputSettings::default(),
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

    fn save_split_test_image(path: &Path) {
        let img = DynamicImage::ImageRgb8(ImageBuffer::from_fn(20, 10, |x, _y| {
            if x < 10 {
                image::Rgb([240, 0, 0])
            } else {
                image::Rgb([0, 0, 240])
            }
        }));
        img.save(path).unwrap();
    }

    fn save_split_test_jpeg_with_orientation(path: &Path, orientation: u16) {
        save_split_test_image(path);
        inject_exif_orientation(path, orientation);
    }

    fn inject_exif_orientation(path: &Path, orientation: u16) {
        let mut data = fs::read(path).unwrap();
        assert!(data.starts_with(&[0xFF, 0xD8]));

        let orientation_bytes = orientation.to_le_bytes();
        let app1 = [
            0xFF,
            0xE1,
            0x00,
            0x22,
            b'E',
            b'x',
            b'i',
            b'f',
            0x00,
            0x00,
            b'I',
            b'I',
            0x2A,
            0x00,
            0x08,
            0x00,
            0x00,
            0x00,
            0x01,
            0x00,
            0x12,
            0x01,
            0x03,
            0x00,
            0x01,
            0x00,
            0x00,
            0x00,
            orientation_bytes[0],
            orientation_bytes[1],
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
        ];
        data.splice(2..2, app1);
        fs::write(path, data).unwrap();
    }

    fn read_jfif_dpi(path: &Path) -> Option<u16> {
        let data = fs::read(path).ok()?;
        if data.len() < 16 || data[0] != 0xFF || data[1] != 0xD8 {
            return None;
        }
        if data[2] == 0xFF && data[3] == 0xE0 && &data[6..11] == b"JFIF\0" {
            return Some(u16::from_be_bytes([data[12], data[13]]));
        }
        None
    }

    #[test]
    fn validate_rejects_empty_image_list() {
        let dir = tempfile::tempdir().unwrap();
        let config = base_config(dir.path().to_path_buf(), vec![]);

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("至少 1 张图片"));
    }

    #[test]
    fn validate_rejects_resample_larger_than_border() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = base_config(dir.path().to_path_buf(), vec![dir.path().join("a.jpg")]);
        config.resample_size = 100;
        config.border_size = 50;

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("不能大于单图边框大小"));
    }

    #[test]
    fn validate_rejects_too_small_final_size() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = base_config(dir.path().to_path_buf(), vec![dir.path().join("a.jpg")]);
        config.final_size = 2000;

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("最终图像大小"));
    }

    #[test]
    fn validate_rejects_invalid_jpeg_quality() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = base_config(dir.path().to_path_buf(), vec![dir.path().join("a.jpg")]);
        config.output_settings.jpeg_quality = 0;

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("JPEG 质量"));
    }

    #[test]
    fn validate_rejects_final_size_that_exceeds_jpeg_limits() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = base_config(dir.path().to_path_buf(), vec![dir.path().join("a.jpg")]);
        config.final_size = u16::MAX as u32 + 1;

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("最终图像大小"));
    }

    #[test]
    fn validate_rejects_dpi_that_cannot_be_written_to_jfif() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = base_config(dir.path().to_path_buf(), vec![dir.path().join("a.jpg")]);
        config.dpi = u16::MAX as u32 + 1;

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("DPI"));
    }

    #[test]
    fn validate_rejects_missing_watermark_path() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = base_config(dir.path().to_path_buf(), vec![dir.path().join("a.jpg")]);
        config.watermark = Some(WatermarkConfig {
            path: dir.path().join("missing.png"),
            scale_percent: 100.0,
            position_x_percent: 50.0,
            position_y_percent: 95.0,
        });

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("水印图片不可访问"));
    }

    #[test]
    fn validate_rejects_watermark_scale_outside_ui_range() {
        let dir = tempfile::tempdir().unwrap();
        let watermark = dir.path().join("watermark.png");
        save_test_image(&watermark, 4, 4);
        let mut config = base_config(dir.path().to_path_buf(), vec![dir.path().join("a.jpg")]);
        config.watermark = Some(WatermarkConfig {
            path: watermark,
            scale_percent: 1000.0,
            position_x_percent: 50.0,
            position_y_percent: 50.0,
        });

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("水印缩放比例"));
    }

    #[test]
    fn validate_rejects_watermark_position_outside_canvas_percent_range() {
        let dir = tempfile::tempdir().unwrap();
        let watermark = dir.path().join("watermark.png");
        save_test_image(&watermark, 4, 4);
        let mut config = base_config(dir.path().to_path_buf(), vec![dir.path().join("a.jpg")]);
        config.watermark = Some(WatermarkConfig {
            path: watermark,
            scale_percent: 100.0,
            position_x_percent: -1.0,
            position_y_percent: 50.0,
        });

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("水印位置"));
    }

    #[test]
    fn validate_rejects_collage_dimensions_that_exceed_jpeg_limits() {
        let dir = tempfile::tempdir().unwrap();
        let image_paths = (0..16)
            .map(|i| dir.path().join(format!("image_{}.jpg", i)))
            .collect();
        let mut config = base_config(dir.path().to_path_buf(), image_paths);
        config.border_size = 20_000;
        config.resample_size = 20_000;

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("拼贴图尺寸"));
    }

    #[test]
    fn validate_rejects_existing_outputs_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("output_collage_final.jpg"), b"existing").unwrap();
        let config = base_config(dir.path().to_path_buf(), vec![dir.path().join("a.jpg")]);

        let err = validate_config(&config).unwrap_err().to_string();

        assert!(err.contains("输出文件已存在"));
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
        assert_eq!(report.outputs.len(), 1);
        assert!(report.outputs.iter().all(|path| path.exists()));
        assert!(!dir.path().join("partial_collage.jpg").exists());
    }

    #[test]
    fn run_creates_outputs_and_injects_dpi() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.jpg");
        let second = dir.path().join("second.jpg");
        save_test_image(&first, 20, 10);
        save_test_image(&second, 10, 20);

        let mut config = base_config(dir.path().to_path_buf(), vec![first, second]);
        config.prefix = "full".into();

        let report = run(&config).unwrap();

        assert_eq!(report.processed_count, 2);
        assert!(report.failed_images.is_empty());
        assert_eq!(report.outputs.len(), 1);
        assert_eq!(read_jfif_dpi(&report.outputs[0]), Some(300));
    }

    #[test]
    fn run_applies_image_rotation_from_preview_config() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("split.png");
        save_split_test_image(&input);

        let config: CollageConfig = serde_json::from_value(serde_json::json!({
            "image_paths": [input],
            "image_rotations": {
                input.to_string_lossy().as_ref(): 90
            },
            "output_dir": dir.path(),
            "prefix": "rotated",
            "resample_size": 40,
            "border_size": 60,
            "final_size": 2100,
            "dpi": 300,
            "background_color": "white",
            "overwrite": false,
            "output_settings": {
                "jpeg_quality": 100,
                "auto_orient": false
            },
            "color_management": {
                "enabled": false,
                "target_profile": "srgb",
                "rendering_intent": "perceptual"
            }
        }))
        .unwrap();

        let report = run(&config).unwrap();
        let collage = image::open(&report.outputs[0]).unwrap().to_rgb8();
        let top_center = collage.get_pixel(1050, 1025).0;

        assert!(
            top_center[0] > 180 && top_center[2] < 80,
            "expected top center to be red after 90 degree rotation, got {:?}",
            top_center
        );
    }

    #[test]
    fn manual_rotation_overrides_exif_auto_orientation_for_that_image() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("split.jpg");
        save_split_test_jpeg_with_orientation(&input, 8);

        let config: CollageConfig = serde_json::from_value(serde_json::json!({
            "image_paths": [input],
            "image_rotations": {
                input.to_string_lossy().as_ref(): 90
            },
            "output_dir": dir.path(),
            "prefix": "manual_over_exif",
            "resample_size": 40,
            "border_size": 60,
            "final_size": 2100,
            "dpi": 300,
            "background_color": "white",
            "overwrite": false,
            "output_settings": {
                "jpeg_quality": 100,
                "auto_orient": true
            },
            "color_management": {
                "enabled": false,
                "target_profile": "srgb",
                "rendering_intent": "perceptual"
            }
        }))
        .unwrap();

        let report = run(&config).unwrap();
        let collage = image::open(&report.outputs[0]).unwrap().to_rgb8();
        let top_center = collage.get_pixel(1050, 1025).0;

        assert!(
            top_center[0] > 180 && top_center[2] < 100,
            "expected manual rotation to match preview instead of stacking on EXIF, got {:?}",
            top_center
        );
    }

    #[test]
    fn run_without_watermark_removes_collage_intermediate() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        save_test_image(&image, 20, 10);

        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.prefix = "no_watermark".into();

        let report = run(&config).unwrap();

        assert_eq!(
            report.outputs,
            vec![dir.path().join("no_watermark_collage_final.jpg")]
        );
        assert!(dir.path().join("no_watermark_collage_final.jpg").exists());
        assert!(!dir.path().join("no_watermark_collage.jpg").exists());
    }

    #[test]
    fn run_with_watermark_removes_collage_and_unwatermarked_intermediates() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        let watermark = dir.path().join("watermark.png");
        save_test_image(&image, 20, 10);
        save_test_image(&watermark, 4, 4);

        let mut config = base_config(dir.path().to_path_buf(), vec![image]);
        config.prefix = "watermark_only".into();
        config.watermark = Some(WatermarkConfig {
            path: watermark,
            scale_percent: 100.0,
            position_x_percent: 50.0,
            position_y_percent: 50.0,
        });

        let report = run(&config).unwrap();

        assert_eq!(
            report.outputs,
            vec![dir
                .path()
                .join("watermark_only_collage_final_watermarked.jpg")]
        );
        assert!(dir
            .path()
            .join("watermark_only_collage_final_watermarked.jpg")
            .exists());
        assert!(!dir.path().join("watermark_only_collage.jpg").exists());
        assert!(!dir.path().join("watermark_only_collage_final.jpg").exists());
    }

    #[test]
    fn run_creates_watermarked_output() {
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

        assert_eq!(report.outputs.len(), 1);
        assert!(report.outputs[0].exists());
        assert!(report.outputs[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("_collage_final_watermarked.jpg"));
    }
}
