use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use image::DynamicImage;
use rayon::prelude::*;

use crate::{
    border::{add_final_border_and_resize, calculate_dynamic_border},
    collage::create_collage,
    config::CollageConfig,
    error::AppError,
    image_proc::{add_square_border, resample},
    progress::{self, ProgressMessage, Stage},
    watermark::add_watermark,
};

pub fn run(config: &CollageConfig) -> Result<Vec<PathBuf>, AppError> {
    // 阶段 1：并行处理单图
    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::ProcessingImages,
        message: format!("正在并行处理 {} 张图片...", config.image_paths.len()),
    });

    let total = config.image_paths.len();
    let counter = Arc::new(AtomicUsize::new(0));

    let results: Vec<Result<DynamicImage, AppError>> = config
        .image_paths
        .par_iter()
        .map(|img_path| {
            let img = image::open(img_path).map_err(AppError::Image)?;
            let resampled = resample(img, config.resample_size);
            let bordered = add_square_border(resampled, config.border_size, &config.background_color);

            let done = counter.fetch_add(1, Ordering::SeqCst) + 1;
            progress::send(&ProgressMessage::ImageProcessed { index: done, total });

            Ok(bordered)
        })
        .collect();

    // 分离成功/失败
    let mut bordered_images: Vec<DynamicImage> = Vec::new();
    let mut failed_count = 0usize;
    for r in results {
        match r {
            Ok(img) => bordered_images.push(img),
            Err(e) => {
                eprintln!("处理单图失败: {}", e);
                failed_count += 1;
            }
        }
    }

    if bordered_images.is_empty() {
        return Err(AppError::NoImagesProcessed);
    }
    if failed_count > 0 {
        eprintln!("警告: {} 张图片处理失败，继续处理 {} 张成功图片", failed_count, bordered_images.len());
    }

    // 阶段 2：创建拼贴图
    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::CreatingCollage,
        message: "正在创建拼贴图...".into(),
    });

    let collage_path = config.output_dir.join(format!("{}_collage.jpg", config.prefix));
    create_collage(&bordered_images, &collage_path, config)?;

    // 阶段 3：添加最终边框
    progress::send(&ProgressMessage::StageChanged {
        stage: Stage::AddingBorder,
        message: "正在添加最终边框...".into(),
    });

    let grid_cols = (bordered_images.len() as f64).sqrt().ceil() as u32;
    let dynamic_border = calculate_dynamic_border(grid_cols);
    let final_path = config.output_dir.join(format!("{}_collage_final.jpg", config.prefix));
    add_final_border_and_resize(&collage_path, &final_path, config, dynamic_border)?;

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
        add_watermark(&final_path, &wm_path, wm_config, config.dpi)?;
        outputs.push(wm_path);
    }

    Ok(outputs)
}
