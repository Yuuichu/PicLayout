use image::{DynamicImage, Rgba};
use rayon::prelude::*;

use crate::{
    collage::PositionReferenceArea,
    color::{self, TargetColorProfile},
    config::{CollageConfig, WatermarkConfig},
    error::AppError,
    image_loader::open_image,
    image_proc::resize_high_quality_with_options,
};

pub fn add_watermark_to_image(
    base: DynamicImage,
    wm_config: &WatermarkConfig,
    config: &CollageConfig,
    target_profile: &TargetColorProfile,
    reference_area: PositionReferenceArea,
) -> Result<(DynamicImage, Vec<String>), AppError> {
    let wm_raw = open_image(&wm_config.path)?;
    let (wm_raw, warnings) = color::prepare_image(&wm_config.path, wm_raw, config, target_profile)?;

    // 缩放水印
    let reference_scale = match wm_config.position_reference {
        crate::config::PositionReference::Canvas => 1.0,
        crate::config::PositionReference::Content => {
            reference_area.width as f32 / config.final_size.max(1) as f32
        }
    };
    let scale = wm_config.scale_percent / 100.0 * reference_scale;
    let wm_w = (wm_raw.width() as f32 * scale).round() as u32;
    let wm_h = (wm_raw.height() as f32 * scale).round() as u32;
    let wm_scaled = resize_high_quality_with_options(
        wm_raw,
        wm_w.max(1),
        wm_h.max(1),
        config.linear_light_resize(),
    )?;

    let (x, y) = watermark_origin(
        wm_w,
        wm_h,
        wm_config.position_x_percent,
        wm_config.position_y_percent,
        reference_area,
    );

    // 将底图转为 RGBA，然后合成水印
    let mut canvas = base.to_rgba8();
    let wm_rgba = wm_scaled.to_rgba8();

    composite_visible_overlay(&mut canvas, &wm_rgba, x, y);

    Ok((DynamicImage::ImageRgba8(canvas), warnings))
}

fn watermark_origin(
    watermark_width: u32,
    watermark_height: u32,
    position_x_percent: f32,
    position_y_percent: f32,
    reference_area: PositionReferenceArea,
) -> (i64, i64) {
    let (center_x, center_y) =
        reference_area.center_at_percent(position_x_percent, position_y_percent);

    (
        center_x - (watermark_width / 2) as i64,
        center_y - (watermark_height / 2) as i64,
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watermark_origin_is_relative_to_full_canvas() {
        let first = PositionReferenceArea {
            x: 0,
            y: 0,
            width: 300,
            height: 150,
        };
        let second = PositionReferenceArea {
            x: 0,
            y: 0,
            width: 300,
            height: 400,
        };
        assert_eq!(watermark_origin(30, 10, 50.0, 90.0, first), (135, 130));
        assert_eq!(watermark_origin(30, 10, 50.0, 90.0, second), (135, 355));
    }

    #[test]
    fn watermark_origin_uses_content_area_and_allows_outside_percentages() {
        let content = PositionReferenceArea {
            x: 40,
            y: 30,
            width: 200,
            height: 100,
        };
        assert_eq!(watermark_origin(40, 20, 0.0, 0.0, content), (20, 20));
        assert_eq!(watermark_origin(40, 20, 100.0, 100.0, content), (220, 120));
        assert_eq!(watermark_origin(40, 20, -20.0, 150.0, content), (-20, 170));
    }
}
