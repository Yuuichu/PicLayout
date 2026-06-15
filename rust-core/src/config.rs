use image::Rgba;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct CollageConfig {
    pub image_paths: Vec<PathBuf>,
    #[serde(default)]
    pub image_rotations: HashMap<PathBuf, u16>,
    #[serde(default)]
    pub processing_mode: ProcessingMode,
    pub output_dir: PathBuf,
    pub prefix: String,
    #[serde(default = "default_resample_size")]
    pub resample_size: u32,
    #[serde(default = "default_border_size")]
    pub border_size: u32,
    #[serde(default = "default_final_size")]
    pub final_size: u32,
    #[serde(default = "default_dpi")]
    pub dpi: u32,
    #[serde(default)]
    pub background_color: BackgroundColor,
    pub watermark: Option<WatermarkConfig>,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub output_settings: OutputSettings,
    #[serde(default)]
    pub color_management: ColorManagementConfig,
}

impl CollageConfig {
    pub fn image_rotation_degrees(&self, path: &Path) -> u16 {
        self.image_rotations.get(path).copied().unwrap_or(0)
    }

    pub fn auto_orient_image(&self, path: &Path) -> bool {
        self.output_settings.auto_orient && !self.image_rotations.contains_key(path)
    }

    pub fn linear_light_resize(&self) -> bool {
        self.output_settings
            .linear_light_resize
            .unwrap_or_else(|| self.processing_mode.default_linear_light_resize())
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundColor {
    #[default]
    White,
    Black,
    Grey,
    Lightgrey,
    Beige,
    Lightblue,
    Lightyellow,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingMode {
    #[default]
    #[serde(alias = "standard")]
    StandardHighQuality,
    #[serde(alias = "high_quality")]
    MaximumQuality,
    #[serde(alias = "fast")]
    FastPreview,
}

impl ProcessingMode {
    pub fn default_linear_light_resize(self) -> bool {
        matches!(self, ProcessingMode::MaximumQuality)
    }
}

impl BackgroundColor {
    pub fn to_rgba(&self) -> Rgba<u8> {
        match self {
            BackgroundColor::White => Rgba([255, 255, 255, 255]),
            BackgroundColor::Black => Rgba([0, 0, 0, 255]),
            BackgroundColor::Grey => Rgba([128, 128, 128, 255]),
            BackgroundColor::Lightgrey => Rgba([211, 211, 211, 255]),
            BackgroundColor::Beige => Rgba([245, 245, 220, 255]),
            BackgroundColor::Lightblue => Rgba([173, 216, 230, 255]),
            BackgroundColor::Lightyellow => Rgba([255, 255, 224, 255]),
        }
    }

    /// 转换为 RGB 数组（用于无 alpha 的 ImageBuffer）
    pub fn to_rgb(&self) -> [u8; 3] {
        let rgba = self.to_rgba();
        [rgba[0], rgba[1], rgba[2]]
    }
}

#[derive(Debug, Deserialize)]
pub struct WatermarkConfig {
    pub path: PathBuf,
    #[serde(default = "default_watermark_scale")]
    pub scale_percent: f32,
    #[serde(default = "default_watermark_x")]
    pub position_x_percent: f32,
    #[serde(default = "default_watermark_y")]
    pub position_y_percent: f32,
}

#[derive(Debug, Deserialize)]
pub struct OutputSettings {
    #[serde(default = "default_jpeg_quality")]
    pub jpeg_quality: u8,
    #[serde(default = "default_auto_orient")]
    pub auto_orient: bool,
    #[serde(default)]
    pub linear_light_resize: Option<bool>,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            jpeg_quality: default_jpeg_quality(),
            auto_orient: default_auto_orient(),
            linear_light_resize: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ColorManagementConfig {
    #[serde(default = "default_color_management_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub target_profile: TargetProfileMode,
    pub target_profile_path: Option<PathBuf>,
    #[serde(default)]
    pub rendering_intent: RenderingIntent,
}

impl Default for ColorManagementConfig {
    fn default() -> Self {
        Self {
            enabled: default_color_management_enabled(),
            target_profile: TargetProfileMode::Srgb,
            target_profile_path: None,
            rendering_intent: RenderingIntent::Perceptual,
        }
    }
}

#[derive(Debug, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetProfileMode {
    #[default]
    Srgb,
    Custom,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum RenderingIntent {
    #[default]
    Perceptual,
    RelativeColorimetric,
}

fn default_resample_size() -> u32 {
    4000
}
fn default_border_size() -> u32 {
    4200
}
fn default_final_size() -> u32 {
    10000
}
fn default_dpi() -> u32 {
    300
}
fn default_watermark_scale() -> f32 {
    100.0
}
fn default_watermark_x() -> f32 {
    50.0
}
fn default_watermark_y() -> f32 {
    95.0
}
fn default_jpeg_quality() -> u8 {
    95
}
fn default_auto_orient() -> bool {
    true
}
fn default_color_management_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse_config(value: serde_json::Value) -> CollageConfig {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn processing_mode_defaults_to_standard_high_quality() {
        let config = parse_config(json!({
            "image_paths": [],
            "output_dir": ".",
            "prefix": "test"
        }));

        assert_eq!(config.processing_mode, ProcessingMode::StandardHighQuality);
        assert!(!config.linear_light_resize());
    }

    #[test]
    fn maximum_quality_enables_linear_light_by_default() {
        let config = parse_config(json!({
            "image_paths": [],
            "processing_mode": "maximum_quality",
            "output_dir": ".",
            "prefix": "test"
        }));

        assert_eq!(config.processing_mode, ProcessingMode::MaximumQuality);
        assert!(config.linear_light_resize());
    }

    #[test]
    fn linear_light_setting_overrides_processing_mode_default() {
        let config = parse_config(json!({
            "image_paths": [],
            "processing_mode": "maximum_quality",
            "output_dir": ".",
            "prefix": "test",
            "output_settings": {
                "linear_light_resize": false
            }
        }));

        assert!(!config.linear_light_resize());
    }

    #[test]
    fn legacy_processing_mode_values_still_deserialize() {
        let high_quality = parse_config(json!({
            "image_paths": [],
            "processing_mode": "high_quality",
            "output_dir": ".",
            "prefix": "test"
        }));
        let standard = parse_config(json!({
            "image_paths": [],
            "processing_mode": "standard",
            "output_dir": ".",
            "prefix": "test"
        }));
        let fast = parse_config(json!({
            "image_paths": [],
            "processing_mode": "fast",
            "output_dir": ".",
            "prefix": "test"
        }));

        assert_eq!(high_quality.processing_mode, ProcessingMode::MaximumQuality);
        assert_eq!(standard.processing_mode, ProcessingMode::StandardHighQuality);
        assert_eq!(fast.processing_mode, ProcessingMode::FastPreview);
    }
}
