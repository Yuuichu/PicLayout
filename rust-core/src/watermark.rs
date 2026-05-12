use std::path::Path;

use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgba};

use crate::{config::WatermarkConfig, dpi::inject_dpi, error::AppError};

pub fn add_watermark(
    input: &Path,
    output: &Path,
    wm_config: &WatermarkConfig,
    dpi: u32,
) -> Result<(), AppError> {
    let base = image::open(input)?;
    let (img_w, img_h) = base.dimensions();

    let wm_raw = image::open(&wm_config.path)?;

    // 缩放水印
    let scale = wm_config.scale_percent / 100.0;
    let wm_w = (wm_raw.width() as f32 * scale).round() as u32;
    let wm_h = (wm_raw.height() as f32 * scale).round() as u32;
    let wm_scaled = wm_raw.resize_exact(wm_w.max(1), wm_h.max(1), FilterType::Lanczos3);

    // 计算水印位置（中心对齐到百分比坐标）
    let center_x = (img_w as f32 * wm_config.position_x_percent / 100.0) as i64;
    let center_y = (img_h as f32 * wm_config.position_y_percent / 100.0) as i64;
    let x = center_x - (wm_w / 2) as i64;
    let y = center_y - (wm_h / 2) as i64;

    // 将底图转为 RGBA，然后合成水印
    let mut canvas = base.to_rgba8();
    let wm_rgba = wm_scaled.to_rgba8();

    for (px, py, wm_pixel) in wm_rgba.enumerate_pixels() {
        let tx = x + px as i64;
        let ty = y + py as i64;
        if tx < 0 || ty < 0 || tx >= img_w as i64 || ty >= img_h as i64 {
            continue;
        }
        let tx = tx as u32;
        let ty = ty as u32;

        let base_pixel = canvas.get_pixel(tx, ty);
        let blended = alpha_composite(*base_pixel, *wm_pixel);
        canvas.put_pixel(tx, ty, blended);
    }

    DynamicImage::ImageRgba8(canvas).save(output)?;
    inject_dpi(output, dpi as u16)?;
    Ok(())
}

/// Alpha 合成（Porter-Duff "over" 操作）
fn alpha_composite(base: Rgba<u8>, overlay: Rgba<u8>) -> Rgba<u8> {
    let alpha_o = overlay[3] as f32 / 255.0;
    let alpha_b = base[3] as f32 / 255.0;
    let alpha_out = alpha_o + alpha_b * (1.0 - alpha_o);

    if alpha_out == 0.0 {
        return Rgba([0, 0, 0, 0]);
    }

    let blend = |co: u8, cb: u8| -> u8 {
        let co = co as f32 / 255.0;
        let cb = cb as f32 / 255.0;
        let out = (co * alpha_o + cb * alpha_b * (1.0 - alpha_o)) / alpha_out;
        (out * 255.0).round().clamp(0.0, 255.0) as u8
    };

    Rgba([
        blend(overlay[0], base[0]),
        blend(overlay[1], base[1]),
        blend(overlay[2], base[2]),
        (alpha_out * 255.0).round() as u8,
    ])
}
