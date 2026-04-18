use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba, imageops, imageops::FilterType};

use crate::config::BackgroundColor;

/// 重采样：等比缩放使长边 = max_size
pub fn resample(img: DynamicImage, max_size: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let (new_w, new_h) = if w >= h {
        let new_w = max_size;
        let new_h = (max_size as f64 / w as f64 * h as f64).round() as u32;
        (new_w, new_h.max(1))
    } else {
        let new_h = max_size;
        let new_w = (max_size as f64 / h as f64 * w as f64).round() as u32;
        (new_w.max(1), new_h)
    };
    img.resize_exact(new_w, new_h, FilterType::Lanczos3)
}

/// 添加正方形边框：将图片（已由 resample 缩放好）居中放置在 border_size × border_size 背景上
pub fn add_square_border(img: DynamicImage, border_size: u32, bg: &BackgroundColor) -> DynamicImage {
    let (iw, ih) = img.dimensions();

    let [r, g, b] = bg.to_rgb();
    let mut canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(border_size, border_size, Rgba([r, g, b, 255]));

    let x_offset = (border_size.saturating_sub(iw)) / 2;
    let y_offset = (border_size.saturating_sub(ih)) / 2;

    imageops::overlay(&mut canvas, &img.to_rgba8(), x_offset as i64, y_offset as i64);

    DynamicImage::ImageRgba8(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    fn make_test_image(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(w, h, |x, _y| {
            image::Rgb([(x % 255) as u8, 100, 200])
        }))
    }

    #[test]
    fn resample_wide_image() {
        let img = make_test_image(8000, 4000);
        let out = resample(img, 4000);
        assert_eq!(out.width(), 4000);
        assert_eq!(out.height(), 2000);
    }

    #[test]
    fn resample_tall_image() {
        let img = make_test_image(3000, 6000);
        let out = resample(img, 4000);
        assert_eq!(out.height(), 4000);
        assert_eq!(out.width(), 2000);
    }

    #[test]
    fn add_square_border_produces_correct_size() {
        let img = make_test_image(3000, 2000);
        let bg = BackgroundColor::White;
        let out = add_square_border(img, 4200, &bg);
        assert_eq!(out.width(), 4200);
        assert_eq!(out.height(), 4200);
    }
}
