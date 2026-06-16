use std::path::Path;

use image::{DynamicImage, GenericImageView, Rgba};
use rayon::prelude::*;

use crate::{
    color::{self, TargetColorProfile},
    config::{CollageConfig, WatermarkConfig},
    error::AppError,
    image_loader::open_image,
    image_proc::resize_high_quality_with_options,
    jpeg_output::save_user_jpeg,
};

#[allow(dead_code)]
pub fn add_watermark(
    input: &Path,
    output: &Path,
    wm_config: &WatermarkConfig,
    config: &CollageConfig,
    target_profile: &TargetColorProfile,
    icc_profile: Option<&[u8]>,
) -> Result<Vec<String>, AppError> {
    let base = open_image(input)?;
    let (watermarked, warnings) = add_watermark_to_image(base, wm_config, config, target_profile)?;
    save_user_jpeg(&watermarked, output, config, icc_profile)?;
    Ok(warnings)
}

pub fn add_watermark_to_image(
    base: DynamicImage,
    wm_config: &WatermarkConfig,
    config: &CollageConfig,
    target_profile: &TargetColorProfile,
) -> Result<(DynamicImage, Vec<String>), AppError> {
    let (img_w, img_h) = base.dimensions();

    let wm_raw = open_image(&wm_config.path)?;
    let (wm_raw, warnings) = color::prepare_image(&wm_config.path, wm_raw, config, target_profile)?;

    // 缩放水印
    let scale = wm_config.scale_percent / 100.0;
    let wm_w = (wm_raw.width() as f32 * scale).round() as u32;
    let wm_h = (wm_raw.height() as f32 * scale).round() as u32;
    let wm_scaled = resize_high_quality_with_options(
        wm_raw,
        wm_w.max(1),
        wm_h.max(1),
        config.linear_light_resize(),
    )?;

    // 计算水印位置（中心对齐到百分比坐标）
    let center_x = (img_w as f32 * wm_config.position_x_percent / 100.0) as i64;
    let center_y = (img_h as f32 * wm_config.position_y_percent / 100.0) as i64;
    let x = center_x - (wm_w / 2) as i64;
    let y = center_y - (wm_h / 2) as i64;

    // 将底图转为 RGBA，然后合成水印
    let mut canvas = base.to_rgba8();
    let wm_rgba = wm_scaled.to_rgba8();

    composite_visible_overlay(&mut canvas, &wm_rgba, x, y);

    Ok((DynamicImage::ImageRgba8(canvas), warnings))
}

/// Alpha 合成（Porter-Duff "over" 操作）
pub fn alpha_composite(base: Rgba<u8>, overlay: Rgba<u8>) -> Rgba<u8> {
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

pub fn composite_visible_overlay(
    canvas: &mut image::RgbaImage,
    overlay_image: &image::RgbaImage,
    x: i64,
    y: i64,
) {
    let img_w = canvas.width() as i64;
    let img_h = canvas.height() as i64;
    let wm_w = overlay_image.width() as i64;
    let wm_h = overlay_image.height() as i64;

    let start_x = x.max(0);
    let start_y = y.max(0);
    let end_x = (x + wm_w).min(img_w);
    let end_y = (y + wm_h).min(img_h);
    if start_x >= end_x || start_y >= end_y {
        return;
    }

    let row_bytes = canvas.width() as usize * 4;
    let wm_row_bytes = overlay_image.width() as usize * 4;
    let wm_raw = overlay_image.as_raw();
    canvas
        .as_mut()
        .par_chunks_mut(row_bytes)
        .enumerate()
        .for_each(|(ty, row)| {
            let ty = ty as i64;
            if ty < start_y || ty >= end_y {
                return;
            }
            let wm_y = (ty - y) as usize;
            for tx in start_x..end_x {
                let wm_x = (tx - x) as usize;
                let dst_idx = tx as usize * 4;
                let wm_idx = wm_y * wm_row_bytes + wm_x * 4;
                let base = Rgba([
                    row[dst_idx],
                    row[dst_idx + 1],
                    row[dst_idx + 2],
                    row[dst_idx + 3],
                ]);
                let overlay = Rgba([
                    wm_raw[wm_idx],
                    wm_raw[wm_idx + 1],
                    wm_raw[wm_idx + 2],
                    wm_raw[wm_idx + 3],
                ]);
                let blended = alpha_composite(base, overlay);
                row[dst_idx..dst_idx + 4].copy_from_slice(&blended.0);
            }
        });
}
