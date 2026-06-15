use fast_image_resize::{self as fr, IntoImageView};
use image::{imageops, DynamicImage, GenericImageView, ImageBuffer, Rgba, Rgba32FImage};

use crate::{config::BackgroundColor, error::AppError};

#[allow(dead_code)]
pub fn resample(img: DynamicImage, max_size: u32) -> Result<DynamicImage, AppError> {
    resample_with_options(img, max_size, false)
}

pub fn resample_with_options(
    img: DynamicImage,
    max_size: u32,
    linear_light: bool,
) -> Result<DynamicImage, AppError> {
    let (w, h) = img.dimensions();
    let (new_w, new_h) = fit_long_edge(w, h, max_size)?;
    resize_high_quality_with_options(img, new_w, new_h, linear_light)
}

pub fn fit_long_edge(w: u32, h: u32, max_size: u32) -> Result<(u32, u32), AppError> {
    if w == 0 || h == 0 || max_size == 0 {
        return Err(AppError::Processing(
            "resize source and target dimensions must be greater than 0".into(),
        ));
    }
    if w >= h {
        let new_w = max_size;
        let new_h = (max_size as f64 / w as f64 * h as f64).round() as u32;
        Ok((new_w, new_h.max(1)))
    } else {
        let new_h = max_size;
        let new_w = (max_size as f64 / h as f64 * w as f64).round() as u32;
        Ok((new_w.max(1), new_h))
    }
}

pub fn resize_high_quality(
    img: DynamicImage,
    width: u32,
    height: u32,
) -> Result<DynamicImage, AppError> {
    resize_high_quality_with_options(img, width, height, false)
}

pub fn resize_high_quality_with_options(
    img: DynamicImage,
    width: u32,
    height: u32,
    linear_light: bool,
) -> Result<DynamicImage, AppError> {
    if linear_light {
        resize_linear_light(img, width, height)
    } else {
        resize_srgb(img, width, height)
    }
}

fn resize_srgb(img: DynamicImage, width: u32, height: u32) -> Result<DynamicImage, AppError> {
    validate_resize_target(width, height)?;

    let rgba = img.into_rgba8();
    let src = DynamicImage::ImageRgba8(rgba);
    let mut dst = fr::images::Image::new(
        width,
        height,
        src.pixel_type().ok_or_else(|| {
            AppError::Processing("unsupported image pixel type for high quality resize".into())
        })?,
    );

    resize_into(&src, &mut dst)?;
    let out = image::RgbaImage::from_raw(width, height, dst.buffer().to_vec()).ok_or_else(|| {
        AppError::Processing("high quality resize produced invalid RGBA buffer".into())
    })?;
    Ok(DynamicImage::ImageRgba8(out))
}

fn resize_linear_light(
    img: DynamicImage,
    width: u32,
    height: u32,
) -> Result<DynamicImage, AppError> {
    validate_resize_target(width, height)?;

    let rgba = img.into_rgba8();
    let linear = Rgba32FImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let px = rgba.get_pixel(x, y);
        image::Rgba([
            srgb_to_linear(px[0]),
            srgb_to_linear(px[1]),
            srgb_to_linear(px[2]),
            px[3] as f32 / 255.0,
        ])
    });
    let src = DynamicImage::ImageRgba32F(linear);
    let mut dst = fr::images::Image::new(width, height, fr::PixelType::F32x4);
    resize_into(&src, &mut dst)?;

    let mut out = Vec::with_capacity(width as usize * height as usize * 4);
    for px in dst.buffer().chunks_exact(16) {
        let r = f32::from_ne_bytes([px[0], px[1], px[2], px[3]]);
        let g = f32::from_ne_bytes([px[4], px[5], px[6], px[7]]);
        let b = f32::from_ne_bytes([px[8], px[9], px[10], px[11]]);
        let a = f32::from_ne_bytes([px[12], px[13], px[14], px[15]]);
        out.push(linear_to_srgb(r));
        out.push(linear_to_srgb(g));
        out.push(linear_to_srgb(b));
        out.push((a.clamp(0.0, 1.0) * 255.0).round() as u8);
    }
    let out = image::RgbaImage::from_raw(width, height, out).ok_or_else(|| {
        AppError::Processing("linear light resize produced invalid RGBA buffer".into())
    })?;
    Ok(DynamicImage::ImageRgba8(out))
}

fn resize_into(src: &DynamicImage, dst: &mut fr::images::Image<'static>) -> Result<(), AppError> {
    let options = fr::ResizeOptions::new()
        .resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3))
        .use_alpha(true);
    let mut resizer = fr::Resizer::new();

    #[cfg(target_arch = "x86_64")]
    unsafe {
        if fr::CpuExtensions::Avx2.is_supported() {
            resizer.set_cpu_extensions(fr::CpuExtensions::Avx2);
        } else if fr::CpuExtensions::Sse4_1.is_supported() {
            resizer.set_cpu_extensions(fr::CpuExtensions::Sse4_1);
        }
    }

    resizer
        .resize(src, dst, Some(&options))
        .map_err(|e| AppError::Processing(format!("high quality resize failed: {}", e)))
}

fn validate_resize_target(width: u32, height: u32) -> Result<(), AppError> {
    if width == 0 || height == 0 {
        return Err(AppError::Processing(
            "resize target dimensions must be greater than 0".into(),
        ));
    }
    Ok(())
}

fn srgb_to_linear(v: u8) -> f32 {
    let v = v as f32 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let srgb = if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round().clamp(0.0, 255.0) as u8
}

pub fn apply_manual_rotation(img: DynamicImage, degrees: u16) -> DynamicImage {
    match degrees {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img,
    }
}

#[allow(dead_code)]
pub fn add_square_border(
    img: DynamicImage,
    border_size: u32,
    bg: &BackgroundColor,
) -> DynamicImage {
    let (iw, ih) = img.dimensions();
    let rgba = img.into_rgba8();

    let [r, g, b] = bg.to_rgb();
    let mut canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(border_size, border_size, Rgba([r, g, b, 255]));

    let x_offset = (border_size.saturating_sub(iw)) / 2;
    let y_offset = (border_size.saturating_sub(ih)) / 2;

    imageops::overlay(&mut canvas, &rgba, x_offset as i64, y_offset as i64);
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
        let out = resample(img, 4000).unwrap();
        assert_eq!(out.width(), 4000);
        assert_eq!(out.height(), 2000);
    }

    #[test]
    fn resample_tall_image() {
        let img = make_test_image(3000, 6000);
        let out = resample(img, 4000).unwrap();
        assert_eq!(out.height(), 4000);
        assert_eq!(out.width(), 2000);
    }

    #[test]
    fn linear_light_resize_uses_requested_dimensions() {
        let img = make_test_image(12, 8);
        let out = resize_high_quality_with_options(img, 6, 4, true).unwrap();
        assert_eq!(out.dimensions(), (6, 4));
    }

    #[test]
    fn manual_rotation_rotates_90_degrees() {
        let img = make_test_image(2, 3);

        let out = apply_manual_rotation(img, 90);

        assert_eq!(out.width(), 3);
        assert_eq!(out.height(), 2);
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
