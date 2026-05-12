use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use image::DynamicImage;
use rayon::prelude::*;

use crate::{
    border::{add_final_border_and_resize, calculate_dynamic_border},
    collage::create_collage,
    color,
    config::{CollageConfig, TargetProfileMode},
    error::AppError,
    image_proc::{add_square_border, resample},
    progress::{self, FailedImage, ProgressMessage, Stage},
    watermark::add_watermark,
};

const RECOMMENDED_MAX_IMAGES: usize = 30;
const HARD_MAX_IMAGES: usize = 500;

#[derive(Debug)]
pub struct PipelineReport {
    pub outputs: Vec<PathBuf>,
    pub processed_count: usize,
    pub failed_images: Vec<FailedImage>,
    pub warnings: Vec<String>,
}

struct ProcessedImage {
    image: DynamicImage,
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

    let results: Vec<Result<ProcessedImage, FailedImage>> = config
        .image_paths
        .par_iter()
        .map(|img_path| {
            let result: Result<ProcessedImage, AppError> = (|| {
                let img = image::open(img_path).map_err(AppError::Image)?;
                let (prepared, image_warnings) =
                    color::prepare_image(img_path, img, config, &target_profile)?;
                let resampled = resample(prepared, config.resample_size);
                let bordered =
                    add_square_border(resampled, config.border_size, &config.background_color);
                Ok(ProcessedImage {
                    image: bordered,
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
    let mut bordered_images: Vec<DynamicImage> = Vec::new();
    let mut failed_images: Vec<FailedImage> = Vec::new();
    let mut failed_count = 0usize;
    for r in results {
        match r {
            Ok(processed) => {
                bordered_images.push(processed.image);
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

    let collage_path = config
        .output_dir
        .join(format!("{}_collage.jpg", config.prefix));
    create_collage(&bordered_images, &collage_path, config, output_icc)?;

    // 阶段 3：添加最终边框
    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::AddingBorder,
        message: "正在添加最终边框...".into(),
    });

    let final_path = config
        .output_dir
        .join(format!("{}_collage_final.jpg", config.prefix));
    add_final_border_and_resize(
        &collage_path,
        &final_path,
        config,
        dynamic_border,
        output_icc,
    )?;

    let mut outputs = vec![collage_path, final_path.clone()];

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
    let mut paths = vec![
        config
            .output_dir
            .join(format!("{}_collage.jpg", config.prefix)),
        config
            .output_dir
            .join(format!("{}_collage_final.jpg", config.prefix)),
    ];
    if config.watermark.is_some() {
        paths.push(
            config
                .output_dir
                .join(format!("{}_collage_final_watermarked.jpg", config.prefix)),
        );
    }
    paths
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
    fn validate_rejects_existing_outputs_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("output_collage.jpg"), b"existing").unwrap();
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
        assert_eq!(report.outputs.len(), 2);
        assert!(report.outputs.iter().all(|path| path.exists()));
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
        assert_eq!(report.outputs.len(), 2);
        assert_eq!(read_jfif_dpi(&report.outputs[1]), Some(300));
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

        assert_eq!(report.outputs.len(), 3);
        assert!(report.outputs[2].exists());
    }
}
