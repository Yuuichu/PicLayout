use std::io::Write;
use std::path::Path;

use image::codecs::jpeg::{JpegEncoder, PixelDensity};
use image::{DynamicImage, GenericImageView, ImageEncoder, PixelWithColorType};
use tempfile::Builder;

use crate::{
    config::CollageConfig,
    dpi::{inject_dpi_into_jpeg, inject_icc_into_jpeg},
    error::AppError,
};

pub fn save_user_jpeg(
    img: &DynamicImage,
    output: &Path,
    config: &CollageConfig,
    icc_profile: Option<&[u8]>,
) -> Result<(), AppError> {
    if let Ok(jpeg) = encode_turbojpeg(img, config, icc_profile) {
        return write_atomic(output, &jpeg, config.overwrite);
    }
    let jpeg = encode_with_image_crate(img, config, icc_profile)?;
    write_atomic(output, &jpeg, config.overwrite)
}

pub fn save_user_jpeg_view<I>(
    img: &I,
    output: &Path,
    config: &CollageConfig,
    icc_profile: Option<&[u8]>,
) -> Result<(), AppError>
where
    I: GenericImageView,
    I::Pixel: PixelWithColorType,
{
    let jpeg = encode_with_image_crate(img, config, icc_profile)?;
    write_atomic(output, &jpeg, config.overwrite)
}

fn encode_turbojpeg(
    img: &DynamicImage,
    config: &CollageConfig,
    icc_profile: Option<&[u8]>,
) -> Result<Vec<u8>, AppError> {
    let rgb = img.to_rgb8();
    let jpeg = turbojpeg::compress_image(
        &rgb,
        config.output_settings.jpeg_quality as i32,
        turbojpeg::Subsamp::None,
    )
    .map_err(|e| AppError::Processing(format!("turbojpeg encode failed: {}", e)))?;
    let jpeg = inject_dpi_into_jpeg(jpeg.as_ref().to_vec(), config.dpi as u16);
    Ok(inject_icc_into_jpeg(jpeg, icc_profile))
}

fn encode_with_image_crate<I>(
    img: &I,
    config: &CollageConfig,
    icc_profile: Option<&[u8]>,
) -> Result<Vec<u8>, AppError>
where
    I: GenericImageView,
    I::Pixel: PixelWithColorType,
{
    let mut jpeg = Vec::new();
    {
        let mut encoder =
            JpegEncoder::new_with_quality(&mut jpeg, config.output_settings.jpeg_quality);
        encoder.set_pixel_density(PixelDensity::dpi(config.dpi as u16));
        if let Some(icc) = icc_profile {
            encoder
                .set_icc_profile(icc.to_vec())
                .map_err(|e| AppError::Processing(format!("write ICC profile failed: {}", e)))?;
        }
        encoder.encode_image(img)?;
    }
    Ok(jpeg)
}

fn write_atomic(output: &Path, bytes: &[u8], overwrite: bool) -> Result<(), AppError> {
    if !overwrite && output.exists() {
        return Err(AppError::Processing(format!(
            "output file already exists; change prefix or output directory: {}",
            output.display()
        )));
    }

    let dir = output.parent().ok_or_else(|| {
        AppError::Processing(format!("output path has no parent directory: {}", output.display()))
    })?;
    let mut temp = Builder::new()
        .prefix(".piclayout-")
        .suffix(".tmp")
        .tempfile_in(dir)?;
    temp.write_all(bytes)?;
    temp.flush()?;

    if overwrite {
        temp.persist(output)
            .map_err(|e| AppError::Io(e.error))?;
    } else {
        temp.persist_noclobber(output)
            .map_err(|e| AppError::Io(e.error))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{
            BackgroundColor, ColorManagementConfig, OutputSettings, ProcessingMode,
            RenderingIntent, TargetProfileMode,
        },
        metadata,
    };
    use image::{ImageBuffer, Rgba};
    use std::fs;

    fn config(output_dir: std::path::PathBuf) -> CollageConfig {
        CollageConfig {
            image_paths: vec![],
            image_rotations: Default::default(),
            processing_mode: ProcessingMode::StandardHighQuality,
            output_dir,
            prefix: "test".into(),
            resample_size: 40,
            border_size: 60,
            tile_border_px: None,
            gap_x_px: 0,
            gap_y_px: 0,
            outer_border_px: None,
            final_size: 2100,
            dpi: 300,
            background_color: BackgroundColor::White,
            watermark: None,
            text_block: None,
            overwrite: true,
            output_settings: OutputSettings {
                jpeg_quality: 95,
                auto_orient: true,
                linear_light_resize: Some(false),
            },
            color_management: ColorManagementConfig {
                enabled: true,
                target_profile: TargetProfileMode::Srgb,
                target_profile_path: None,
                rendering_intent: RenderingIntent::Perceptual,
            },
        }
    }

    fn read_jfif_dpi(path: &Path) -> Option<u16> {
        let data = fs::read(path).ok()?;
        if data.len() < 16 || data[0] != 0xFF || data[1] != 0xD8 {
            return None;
        }
        if data[2] == 0xFF && data[3] == 0xE0 && &data[6..11] == b"JFIF\0" {
            return Some(u16::from_be_bytes([data[14], data[15]]));
        }
        None
    }

    #[test]
    fn writes_jpeg_with_icc_profile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.jpg");
        let mut config = config(dir.path().to_path_buf());
        config.dpi = 300;
        let icc = lcms2::Profile::new_srgb().icc().unwrap();
        let img = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(4, 4, Rgba([20, 30, 40, 255])));

        save_user_jpeg(&img, &path, &config, Some(&icc)).unwrap();

        assert_eq!(read_jfif_dpi(&path), Some(300));
        assert!(metadata::extract_icc_profile(&path).unwrap().is_some());
    }

    #[test]
    fn atomic_write_does_not_overwrite_when_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.jpg");
        fs::write(&path, b"old").unwrap();

        let err = write_atomic(&path, b"new", false).unwrap_err().to_string();

        assert!(err.contains("already exists"));
        assert_eq!(fs::read(&path).unwrap(), b"old");
    }
}
