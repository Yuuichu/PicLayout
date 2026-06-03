use std::fs::File;
use std::path::Path;

use image::{codecs::jpeg::JpegEncoder, DynamicImage, ImageEncoder};
use image::{GenericImageView, PixelWithColorType};

use crate::{config::CollageConfig, dpi::inject_dpi, error::AppError};

pub fn save_user_jpeg(
    img: &DynamicImage,
    output: &Path,
    config: &CollageConfig,
    icc_profile: Option<&[u8]>,
) -> Result<(), AppError> {
    save_user_jpeg_view(img, output, config, icc_profile)
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
    let file = File::create(output)?;
    let mut encoder = JpegEncoder::new_with_quality(file, config.output_settings.jpeg_quality);
    if let Some(icc) = icc_profile {
        encoder
            .set_icc_profile(icc.to_vec())
            .map_err(|e| AppError::Processing(format!("写入 ICC profile 失败: {}", e)))?;
    }

    encoder.encode_image(img)?;
    inject_dpi(output, config.dpi as u16)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{
            BackgroundColor, ColorManagementConfig, OutputSettings, RenderingIntent,
            TargetProfileMode,
        },
        metadata,
    };
    use image::{ImageBuffer, Rgba};

    fn config(output_dir: std::path::PathBuf) -> CollageConfig {
        CollageConfig {
            image_paths: vec![],
            output_dir,
            prefix: "test".into(),
            resample_size: 40,
            border_size: 60,
            final_size: 2100,
            dpi: 300,
            background_color: BackgroundColor::White,
            watermark: None,
            overwrite: true,
            output_settings: OutputSettings {
                jpeg_quality: 95,
                auto_orient: true,
            },
            color_management: ColorManagementConfig {
                enabled: true,
                target_profile: TargetProfileMode::Srgb,
                target_profile_path: None,
                rendering_intent: RenderingIntent::Perceptual,
            },
        }
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

        assert!(metadata::extract_icc_profile(&path).unwrap().is_some());
    }
}
