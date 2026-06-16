use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight,
    Wrap,
};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

use crate::{
    config::{TextAlign, TextBlockConfig, TextFontStyle},
    error::AppError,
    fonts,
    watermark::{alpha_composite, composite_visible_overlay},
};

const MAX_TEXT_BLOCK_CANVAS_HEIGHT: f32 = 100_000.0;

pub fn add_text_block_to_image(
    base: DynamicImage,
    text_config: &TextBlockConfig,
) -> Result<(DynamicImage, Vec<String>), AppError> {
    let text = text_config.text.trim();
    if text.is_empty() {
        return Err(AppError::Processing("text block cannot be empty".into()));
    }

    let (img_w, img_h) = base.dimensions();
    let mut warnings = Vec::new();
    if !fonts::system_has_family(&text_config.font_family) {
        warnings.push(format!(
            "font '{}' was not found; system fallback was used",
            text_config.font_family
        ));
    }

    let block = render_text_block(text_config, img_w)?;
    let center_x = (img_w as f32 * text_config.position_x_percent / 100.0).round() as i64;
    let center_y = (img_h as f32 * text_config.position_y_percent / 100.0).round() as i64;
    let x = center_x - (block.width() / 2) as i64;
    let y = center_y - (block.height() / 2) as i64;

    let mut canvas = base.to_rgba8();
    composite_visible_overlay(&mut canvas, &block, x, y);

    Ok((DynamicImage::ImageRgba8(canvas), warnings))
}

fn render_text_block(text_config: &TextBlockConfig, canvas_width: u32) -> Result<RgbaImage, AppError> {
    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();
    let metrics = Metrics::new(text_config.font_size_px, text_config.line_height_px);
    let content_width = text_config_content_width(text_config, canvas_width)?;
    let attrs = text_attrs(text_config);

    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_size(
        &mut font_system,
        Some(content_width as f32),
        Some(MAX_TEXT_BLOCK_CANVAS_HEIGHT),
    );
    buffer.set_wrap(&mut font_system, Wrap::WordOrGlyph);
    buffer.set_rich_text(
        &mut font_system,
        [(&text_config.text as &str, attrs.clone())],
        &attrs,
        Shaping::Advanced,
        Some(text_align(text_config.align)),
    );
    buffer.shape_until_scroll(&mut font_system, false);

    let text_color = rgba_color(text_config.text_rgba);
    let mut pixels = Vec::<(i32, i32, Rgba<u8>)>::new();
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let physical = glyph.physical((0.0, run.line_y), 1.0);
            swash_cache.with_pixels(&mut font_system, physical.cache_key, text_color, |dx, dy, color| {
                let px = physical.x + dx;
                let py = physical.y + dy;
                let rgba = color.as_rgba();
                if rgba[3] == 0 {
                    return;
                }
                min_x = min_x.min(px);
                min_y = min_y.min(py);
                max_x = max_x.max(px);
                max_y = max_y.max(py);
                pixels.push((px, py, Rgba(rgba)));
            });
        }
    }

    if pixels.is_empty() {
        return Err(AppError::Processing(
            "text block did not produce visible pixels".into(),
        ));
    }

    let padding = text_config.padding_px;
    let left_adjust = if min_x < 0 { (-min_x) as u32 } else { 0 };
    let top_adjust = if min_y < 0 { (-min_y) as u32 } else { 0 };
    let text_height = (max_y - min_y + 1).max(1) as u32;
    let block_width = content_width
        .checked_add(left_adjust)
        .and_then(|w| w.checked_add(padding.saturating_mul(2)))
        .ok_or_else(|| AppError::Processing("text block width calculation overflowed".into()))?;
    let block_height = text_height
        .checked_add(top_adjust)
        .and_then(|h| h.checked_add(padding.saturating_mul(2)))
        .ok_or_else(|| AppError::Processing("text block height calculation overflowed".into()))?;

    let mut block = RgbaImage::from_pixel(block_width, block_height, Rgba(text_config.background_rgba));
    for (px, py, color) in pixels {
        let out_x = px + padding as i32 + left_adjust as i32;
        let out_y = py - min_y + padding as i32 + top_adjust as i32;
        if out_x < 0 || out_y < 0 {
            continue;
        }
        let (out_x, out_y) = (out_x as u32, out_y as u32);
        if out_x >= block.width() || out_y >= block.height() {
            continue;
        }
        let base = *block.get_pixel(out_x, out_y);
        block.put_pixel(out_x, out_y, alpha_composite(base, color));
    }

    Ok(block)
}

fn text_config_content_width(
    text_config: &TextBlockConfig,
    canvas_width: u32,
) -> Result<u32, AppError> {
    let width = (canvas_width as f32 * text_config.max_width_percent / 100.0)
        .round()
        .max(1.0) as u32;
    Ok(width.saturating_sub(text_config.padding_px.saturating_mul(2)).max(1))
}

fn text_attrs(text_config: &TextBlockConfig) -> Attrs<'_> {
    Attrs::new()
        .family(text_family(&text_config.font_family))
        .weight(Weight(text_config.font_weight))
        .style(text_style(text_config.font_style))
}

fn text_family(family: &str) -> Family<'_> {
    match family.trim().to_ascii_lowercase().as_str() {
        "" | "sans" | "sans-serif" => Family::SansSerif,
        "serif" => Family::Serif,
        "monospace" | "monospaced" => Family::Monospace,
        "cursive" => Family::Cursive,
        "fantasy" => Family::Fantasy,
        _ => Family::Name(family),
    }
}

fn text_style(style: TextFontStyle) -> Style {
    match style {
        TextFontStyle::Normal => Style::Normal,
        TextFontStyle::Italic => Style::Italic,
        TextFontStyle::Oblique => Style::Oblique,
    }
}

fn text_align(align: TextAlign) -> Align {
    match align {
        TextAlign::Left => Align::Left,
        TextAlign::Center => Align::Center,
        TextAlign::Right => Align::Right,
    }
}

fn rgba_color(color: [u8; 4]) -> Color {
    Color::rgba(color[0], color[1], color[2], color[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_config(text: &str) -> TextBlockConfig {
        TextBlockConfig {
            text: text.into(),
            font_family: "sans-serif".into(),
            font_weight: 400,
            font_style: TextFontStyle::Normal,
            font_size_px: 24.0,
            line_height_px: 32.0,
            max_width_percent: 50.0,
            align: TextAlign::Left,
            text_rgba: [255, 255, 255, 255],
            background_rgba: [0, 0, 0, 128],
            padding_px: 4,
            position_x_percent: 50.0,
            position_y_percent: 50.0,
        }
    }

    #[test]
    fn empty_text_block_errors() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(100, 100, Rgba([0, 0, 0, 255])));
        let err = add_text_block_to_image(img, &text_config("   ")).unwrap_err();
        assert!(err.to_string().contains("text block cannot be empty"));
    }

    #[test]
    fn text_block_renders_visible_pixels() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(400, 300, Rgba([0, 0, 0, 255])));
        let (out, _) = add_text_block_to_image(img, &text_config("Hello\n中文")).unwrap();
        let rgba = out.to_rgba8();
        assert!(rgba.pixels().any(|pixel| pixel.0 != [0, 0, 0, 255]));
    }
}
