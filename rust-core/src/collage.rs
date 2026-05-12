use std::path::Path;

use image::{imageops, DynamicImage, ImageBuffer, Rgba};

use crate::{config::CollageConfig, error::AppError, jpeg_output::save_user_jpeg, progress};

const CHUNK_SIZE: u32 = 2; // 每次处理的行数，控制内存占用

/// 直接接受内存中的 DynamicImage，避免临时文件的 JPEG 编解码开销
pub fn create_collage(
    images: &[DynamicImage],
    output_path: &Path,
    config: &CollageConfig,
    icc_profile: Option<&[u8]>,
) -> Result<(), AppError> {
    let num = images.len() as u32;
    let grid_cols = (num as f64).sqrt().ceil() as u32;
    let grid_rows = (num as f64 / grid_cols as f64).ceil() as u32;
    let bs = config.border_size;
    let total_width = grid_cols * bs;
    let total_height = grid_rows * bs;

    let [r, g, b] = config.background_color.to_rgb();
    let bg_pixel = Rgba([r, g, b, 255]);

    // 多 chunk 时需要临时文件合并，单 chunk 直接输出
    let temp_dir = tempfile::tempdir()?;
    let mut chunk_paths: Vec<std::path::PathBuf> = Vec::new();

    for start_row in (0..grid_rows).step_by(CHUNK_SIZE as usize) {
        let end_row = (start_row + CHUNK_SIZE).min(grid_rows);
        let chunk_h = (end_row - start_row) * bs;

        let mut chunk: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(total_width, chunk_h, bg_pixel);

        for row_offset in 0..(end_row - start_row) {
            let row = start_row + row_offset;
            for col in 0..grid_cols {
                let idx = (row * grid_cols + col) as usize;
                if idx >= images.len() {
                    continue;
                }
                let x_off = col * bs;
                let y_off = row_offset * bs;
                imageops::overlay(
                    &mut chunk,
                    &images[idx].to_rgba8(),
                    x_off as i64,
                    y_off as i64,
                );
            }
        }

        if grid_rows <= CHUNK_SIZE {
            // 只有一个 chunk，直接保存为最终输出
            save_user_jpeg(
                &DynamicImage::ImageRgba8(chunk),
                output_path,
                config,
                icc_profile,
            )?;
            progress::send(&progress::ProgressMessage::StageChanged {
                stage: progress::Stage::CreatingCollage,
                message: format!("拼贴图已创建（{}×{} 网格）", grid_cols, grid_rows),
            });
            return Ok(());
        }

        let chunk_path = temp_dir.path().join(format!("chunk_{}.png", start_row));
        DynamicImage::ImageRgba8(chunk).save(&chunk_path)?;
        chunk_paths.push(chunk_path);
    }

    // 多 chunk 合并
    let mut final_canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(total_width, total_height, bg_pixel);
    let mut y_off = 0i64;
    for chunk_path in &chunk_paths {
        let chunk_img = image::open(chunk_path)?.to_rgba8();
        let ch = chunk_img.height() as i64;
        imageops::overlay(&mut final_canvas, &chunk_img, 0, y_off);
        y_off += ch;
    }
    save_user_jpeg(
        &DynamicImage::ImageRgba8(final_canvas),
        output_path,
        config,
        icc_profile,
    )?;

    progress::send(&progress::ProgressMessage::StageChanged {
        stage: progress::Stage::CreatingCollage,
        message: format!("拼贴图已创建（{}×{} 网格）", grid_cols, grid_rows),
    });

    Ok(())
}
