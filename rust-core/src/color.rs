use std::path::Path;

use image::DynamicImage;
use lcms2::{ColorSpaceSignature, Intent, PixelFormat, Profile, Transform};

use crate::{
    config::{CollageConfig, RenderingIntent, TargetProfileMode},
    error::AppError,
    metadata,
};

#[derive(Debug, Clone)]
pub struct TargetColorProfile {
    pub enabled: bool,
    pub icc: Option<Vec<u8>>,
}

pub fn load_target_profile(config: &CollageConfig) -> Result<TargetColorProfile, AppError> {
    if !config.color_management.enabled {
        return Ok(TargetColorProfile {
            enabled: false,
            icc: None,
        });
    }

    let icc = match config.color_management.target_profile {
        TargetProfileMode::Srgb => Profile::new_srgb()
            .icc()
            .map_err(|e| AppError::Processing(format!("创建 sRGB ICC profile 失败: {}", e)))?,
        TargetProfileMode::Custom => {
            let path = config
                .color_management
                .target_profile_path
                .as_ref()
                .ok_or_else(|| AppError::Processing("请选择目标 ICC profile 文件".into()))?;
            let bytes = std::fs::read(path).map_err(|e| {
                AppError::Processing(format!(
                    "读取目标 ICC profile 失败: {} ({})",
                    path.display(),
                    e
                ))
            })?;
            validate_rgb_profile(&bytes, "目标 ICC profile")?;
            bytes
        }
    };

    validate_rgb_profile(&icc, "目标 ICC profile")?;
    Ok(TargetColorProfile {
        enabled: true,
        icc: Some(icc),
    })
}

pub fn prepare_image(
    path: &Path,
    img: DynamicImage,
    config: &CollageConfig,
    target_profile: &TargetColorProfile,
) -> Result<(DynamicImage, Vec<String>), AppError> {
    let mut warnings = Vec::new();
    let oriented = metadata::apply_orientation(
        img,
        metadata::read_orientation(path),
        config.output_settings.auto_orient,
    );

    if !target_profile.enabled {
        return Ok((oriented, warnings));
    }

    let target_icc = target_profile
        .icc
        .as_deref()
        .ok_or_else(|| AppError::Processing("目标 ICC profile 未初始化".into()))?;

    let input_icc = match metadata::extract_icc_profile(path)? {
        Some(icc) => {
            validate_rgb_profile(&icc, "输入 ICC profile")?;
            icc
        }
        None => {
            warnings.push(format!(
                "{} 未包含 ICC profile，已按 sRGB 解释",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("图片")
            ));
            Profile::new_srgb()
                .icc()
                .map_err(|e| AppError::Processing(format!("创建 sRGB ICC profile 失败: {}", e)))?
        }
    };

    if input_icc == target_icc {
        return Ok((oriented, warnings));
    }

    let converted = convert_to_target(
        oriented,
        &input_icc,
        target_icc,
        config.color_management.rendering_intent,
    )?;
    Ok((converted, warnings))
}

fn convert_to_target(
    img: DynamicImage,
    input_icc: &[u8],
    target_icc: &[u8],
    rendering_intent: RenderingIntent,
) -> Result<DynamicImage, AppError> {
    let input_profile = Profile::new_icc(input_icc)
        .map_err(|e| AppError::Processing(format!("解析输入 ICC profile 失败: {}", e)))?;
    let target_profile = Profile::new_icc(target_icc)
        .map_err(|e| AppError::Processing(format!("解析目标 ICC profile 失败: {}", e)))?;
    let transform = Transform::<u8, u8>::new(
        &input_profile,
        PixelFormat::RGBA_8,
        &target_profile,
        PixelFormat::RGBA_8,
        to_lcms_intent(rendering_intent),
    )
    .map_err(|e| AppError::Processing(format!("创建 ICC 色彩转换失败: {}", e)))?;

    let mut rgba = img.to_rgba8();
    transform.transform_in_place(rgba.as_mut());
    Ok(DynamicImage::ImageRgba8(rgba))
}

pub fn validate_rgb_profile(icc: &[u8], label: &str) -> Result<(), AppError> {
    let profile = Profile::new_icc(icc)
        .map_err(|e| AppError::Processing(format!("{} 无法解析: {}", label, e)))?;
    if profile.color_space() != ColorSpaceSignature::RgbData {
        return Err(AppError::Processing(format!(
            "{} 不是 RGB profile，本版本不支持 CMYK/灰度输出",
            label
        )));
    }
    Ok(())
}

fn to_lcms_intent(intent: RenderingIntent) -> Intent {
    match intent {
        RenderingIntent::Perceptual => Intent::Perceptual,
        RenderingIntent::RelativeColorimetric => Intent::RelativeColorimetric,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        BackgroundColor, ColorManagementConfig, OutputSettings, RenderingIntent, TargetProfileMode,
    };
    use image::{ImageBuffer, Rgba};

    fn config() -> CollageConfig {
        CollageConfig {
            image_paths: vec![],
            output_dir: std::path::PathBuf::new(),
            prefix: "test".into(),
            resample_size: 40,
            border_size: 60,
            final_size: 2100,
            dpi: 300,
            background_color: BackgroundColor::White,
            watermark: None,
            overwrite: false,
            output_settings: OutputSettings::default(),
            color_management: ColorManagementConfig {
                enabled: true,
                target_profile: TargetProfileMode::Srgb,
                target_profile_path: None,
                rendering_intent: RenderingIntent::Perceptual,
            },
        }
    }

    #[test]
    fn srgb_to_srgb_conversion_keeps_pixels() {
        let icc = Profile::new_srgb().icc().unwrap();
        let img = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 2, Rgba([10, 20, 30, 255])));

        let out = convert_to_target(img, &icc, &icc, RenderingIntent::Perceptual).unwrap();

        assert_eq!(out.to_rgba8().get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn rejects_non_rgb_target_profile() {
        let curve = lcms2::ToneCurve::new(2.2);
        let gray = Profile::new_gray(
            &lcms2::CIExyY {
                x: 0.3127,
                y: 0.3290,
                Y: 1.0,
            },
            &curve,
        )
        .unwrap();
        let icc = gray.icc().unwrap();

        let err = validate_rgb_profile(&icc, "目标 ICC profile")
            .unwrap_err()
            .to_string();

        assert!(err.contains("不是 RGB profile"));
    }

    #[test]
    fn loads_default_srgb_target_profile() {
        let loaded = load_target_profile(&config()).unwrap();

        assert!(loaded.enabled);
        assert!(loaded.icc.unwrap().len() > 100);
    }
}
