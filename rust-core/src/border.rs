use std::path::Path;

use image::{imageops, DynamicImage, ImageBuffer, Rgba};

use crate::{
    config::{percent_to_px, CollageConfig},
    error::AppError,
    image_loader::open_image,
    image_proc::resize_high_quality,
    jpeg_output::save_user_jpeg,
};

/// 根据列数动态计算外边距百分比。
pub fn calculate_dynamic_border_percent(grid_cols: u32) -> f32 {
    if grid_cols >= 10 {
        2.0
    } else if grid_cols <= 2 {
        10.0
    } else {
        // 线性插值：cols=10 -> 2%，cols=2 -> 10%
        2.0 + (10.0 - 2.0) * (10 - grid_cols) as f32 / 8.0
    }
}

pub fn calculate_dynamic_border(grid_cols: u32, final_size: u32) -> u32 {
    percent_to_px(final_size, calculate_dynamic_border_percent(grid_cols)).unwrap_or(0)
}

/// 添加最终外边框并缩放整体，使加边框后的长边 = max_size
#[allow(dead_code)]
pub fn add_final_border_and_resize(
    input: &Path,
    output: &Path,
    config: &CollageConfig,
    border_px: u32,
    icc_profile: Option<&[u8]>,
) -> Result<(), AppError> {
    let img = open_image(input)?;
    let canvas = add_final_border_and_resize_image(img, config, border_px)?;
    save_user_jpeg(&canvas, output, config, icc_profile)?;
    Ok(())
}

pub fn add_final_border_and_resize_image(
    img: DynamicImage,
    config: &CollageConfig,
    border_px: u32,
) -> Result<DynamicImage, AppError> {
    let (w, h) = (img.width(), img.height());

    // 计算缩放比例：加边框后长边 = final_size（防止 border_px 过大时下溢）
    let inner_size = config.final_size.saturating_sub(2 * border_px).max(1);
    let scale = inner_size as f64 / w.max(h) as f64;
    let new_w = (w as f64 * scale).round() as u32;
    let new_h = (h as f64 * scale).round() as u32;

    let scaled = resize_high_quality(img, new_w.max(1), new_h.max(1))?;

    let canvas_w = new_w + 2 * border_px;
    let canvas_h = new_h + 2 * border_px;

    let [r, g, b] = config.background_color.to_rgb();
    let bg_pixel = Rgba([r, g, b, 255]);
    let mut canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(canvas_w, canvas_h, bg_pixel);

    imageops::overlay(
        &mut canvas,
        &scaled.to_rgba8(),
        border_px as i64,
        border_px as i64,
    );

    Ok(DynamicImage::ImageRgba8(canvas))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_border_boundary_values() {
        assert_eq!(calculate_dynamic_border_percent(10), 2.0);
        assert_eq!(calculate_dynamic_border_percent(15), 2.0);
        assert_eq!(calculate_dynamic_border_percent(2), 10.0);
        assert_eq!(calculate_dynamic_border_percent(1), 10.0);

        assert_eq!(calculate_dynamic_border(10, 10_000), 200);
        assert_eq!(calculate_dynamic_border(2, 10_000), 1000);
    }

    #[test]
    fn dynamic_border_interpolation() {
        // cols=6 -> 2 + 8*(10-6)/8 = 6%
        assert_eq!(calculate_dynamic_border_percent(6), 6.0);
        // cols=9 -> 2 + 8*1/8 = 3%
        assert_eq!(calculate_dynamic_border_percent(9), 3.0);

        assert_eq!(calculate_dynamic_border(6, 20_000), 1200);
        assert_eq!(calculate_dynamic_border(9, 20_000), 600);
    }
}
