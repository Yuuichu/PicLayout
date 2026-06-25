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
    #[serde(default)]
    pub tile_border_px: Option<u32>,
    #[serde(default)]
    pub gap_x_px: u32,
    #[serde(default)]
    pub gap_y_px: u32,
    #[serde(default)]
    pub outer_border_px: Option<u32>,
    #[serde(default, flatten)]
    pub layout_percent: LayoutPercentConfig,
    #[serde(default = "default_final_size")]
    pub final_size: u32,
    #[serde(default)]
    pub target_aspect_ratio: Option<AspectRatioConfig>,
    #[serde(default = "default_dpi")]
    pub dpi: u32,
    #[serde(default)]
    pub background_color: BackgroundColor,
    pub watermark: Option<WatermarkConfig>,
    pub text_block: Option<TextBlockConfig>,
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

    pub fn tile_size(&self) -> Option<u32> {
        self.resolved_layout().map(|layout| layout.tile_size_px)
    }

    pub fn resolved_layout(&self) -> Option<ResolvedLayout> {
        let content_long_edge_px = self.resolved_content_long_edge_px()?;
        let tile_size_px = self.resolved_tile_size_px(content_long_edge_px)?;
        let gap_x_px = self.resolved_gap_x_px()?;
        let gap_y_px = self.resolved_gap_y_px()?;

        Some(ResolvedLayout {
            content_long_edge_px,
            tile_size_px,
            gap_x_px,
            gap_y_px,
        })
    }

    pub fn explicit_outer_border_px(&self) -> Option<u32> {
        self.layout_percent
            .outer_border_percent
            .and_then(|percent| percent_to_px(self.final_size, percent))
            .or(self.outer_border_px)
    }

    pub fn target_canvas_dimensions(&self) -> Option<(u32, u32)> {
        self.target_aspect_ratio
            .and_then(|ratio| ratio.canvas_dimensions(self.final_size))
    }

    fn resolved_content_long_edge_px(&self) -> Option<u32> {
        self.layout_percent
            .content_long_edge_percent
            .and_then(|percent| percent_to_px(self.final_size, percent))
            .or(Some(self.resample_size))
    }

    fn resolved_tile_size_px(&self, content_long_edge_px: u32) -> Option<u32> {
        if let Some(percent) = self.layout_percent.tile_border_percent {
            let border_px = percent_to_px(self.final_size, percent)?;
            return border_px
                .checked_mul(2)
                .and_then(|padding| content_long_edge_px.checked_add(padding));
        }

        match self.tile_border_px {
            Some(border_px) => border_px
                .checked_mul(2)
                .and_then(|padding| content_long_edge_px.checked_add(padding)),
            None => Some(self.border_size),
        }
    }

    fn resolved_gap_x_px(&self) -> Option<u32> {
        self.layout_percent
            .gap_x_percent
            .and_then(|percent| percent_to_px(self.final_size, percent))
            .or(Some(self.gap_x_px))
    }

    fn resolved_gap_y_px(&self) -> Option<u32> {
        self.layout_percent
            .gap_y_percent
            .and_then(|percent| percent_to_px(self.final_size, percent))
            .or(Some(self.gap_y_px))
    }

    pub fn has_text_block(&self) -> bool {
        self.text_block
            .as_ref()
            .is_some_and(|block| !block.text.trim().is_empty())
    }

    pub fn has_overlay(&self) -> bool {
        self.watermark.is_some() || self.has_text_block()
    }
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
pub struct LayoutPercentConfig {
    #[serde(default)]
    pub content_long_edge_percent: Option<f32>,
    #[serde(default)]
    pub tile_border_percent: Option<f32>,
    #[serde(default)]
    pub gap_x_percent: Option<f32>,
    #[serde(default)]
    pub gap_y_percent: Option<f32>,
    #[serde(default)]
    pub outer_border_percent: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedLayout {
    pub content_long_edge_px: u32,
    pub tile_size_px: u32,
    pub gap_x_px: u32,
    pub gap_y_px: u32,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
pub struct AspectRatioConfig {
    pub width: f32,
    pub height: f32,
}

impl AspectRatioConfig {
    pub fn normalized_ratio(self) -> Option<f64> {
        if !self.width.is_finite() || !self.height.is_finite() {
            return None;
        }
        if self.width <= 0.0 || self.height <= 0.0 {
            return None;
        }

        Some(self.width as f64 / self.height as f64)
    }

    pub fn canvas_dimensions(self, final_size: u32) -> Option<(u32, u32)> {
        let ratio = self.normalized_ratio()?;
        if final_size == 0 {
            return None;
        }

        if ratio >= 1.0 {
            let height = (final_size as f64 / ratio).round();
            return Some((final_size, height.max(1.0).min(u32::MAX as f64) as u32));
        }

        let width = (final_size as f64 * ratio).round();
        Some((width.max(1.0).min(u32::MAX as f64) as u32, final_size))
    }
}

pub fn percent_to_px(base_px: u32, percent: f32) -> Option<u32> {
    if !percent.is_finite() || percent < 0.0 {
        return None;
    }

    let value = base_px as f64 * percent as f64 / 100.0;
    if value > u32::MAX as f64 {
        return None;
    }

    Some(value.round() as u32)
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
    #[serde(default)]
    pub position_reference: PositionReference,
    #[serde(default = "default_watermark_x")]
    pub position_x_percent: f32,
    #[serde(default = "default_watermark_y")]
    pub position_y_percent: f32,
}

#[derive(Debug, Deserialize)]
pub struct TextBlockConfig {
    pub text: String,
    #[serde(default = "default_text_font_family")]
    pub font_family: String,
    #[serde(default = "default_text_font_weight")]
    pub font_weight: u16,
    #[serde(default)]
    pub font_style: TextFontStyle,
    #[serde(default = "default_text_font_size")]
    pub font_size_px: f32,
    #[serde(default = "default_text_line_height")]
    pub line_height_px: f32,
    #[serde(default = "default_text_max_width")]
    pub max_width_percent: f32,
    #[serde(default)]
    pub align: TextAlign,
    #[serde(default = "default_text_rgba")]
    pub text_rgba: [u8; 4],
    #[serde(default = "default_text_background_rgba")]
    pub background_rgba: [u8; 4],
    #[serde(default)]
    pub padding_px: u32,
    #[serde(default)]
    pub position_reference: PositionReference,
    #[serde(default = "default_text_x")]
    pub position_x_percent: f32,
    #[serde(default = "default_text_y")]
    pub position_y_percent: f32,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PositionReference {
    #[default]
    Canvas,
    Content,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextFontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
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
fn default_text_font_family() -> String {
    "sans-serif".into()
}
fn default_text_font_weight() -> u16 {
    400
}
fn default_text_font_size() -> f32 {
    120.0
}
fn default_text_line_height() -> f32 {
    144.0
}
fn default_text_max_width() -> f32 {
    60.0
}
fn default_text_rgba() -> [u8; 4] {
    [255, 255, 255, 255]
}
fn default_text_background_rgba() -> [u8; 4] {
    [0, 0, 0, 0]
}
fn default_text_x() -> f32 {
    50.0
}
fn default_text_y() -> f32 {
    92.0
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
        assert_eq!(
            standard.processing_mode,
            ProcessingMode::StandardHighQuality
        );
        assert_eq!(fast.processing_mode, ProcessingMode::FastPreview);
    }

    #[test]
    fn tile_size_uses_legacy_border_size_when_tile_border_missing() {
        let config = parse_config(json!({
            "image_paths": [],
            "output_dir": ".",
            "prefix": "test",
            "resample_size": 4000,
            "border_size": 4200
        }));

        assert_eq!(config.tile_size(), Some(4200));
    }

    #[test]
    fn tile_size_uses_explicit_tile_border_when_present() {
        let config = parse_config(json!({
            "image_paths": [],
            "output_dir": ".",
            "prefix": "test",
            "resample_size": 4000,
            "border_size": 4200,
            "tile_border_px": 250
        }));

        assert_eq!(config.tile_size(), Some(4500));
    }

    #[test]
    fn percent_layout_fields_resolve_against_final_size() {
        let config = parse_config(json!({
            "image_paths": [],
            "output_dir": ".",
            "prefix": "test",
            "final_size": 10000,
            "content_long_edge_percent": 40,
            "tile_border_percent": 1,
            "gap_x_percent": 2.5,
            "gap_y_percent": 3
        }));

        assert_eq!(
            config.resolved_layout(),
            Some(ResolvedLayout {
                content_long_edge_px: 4000,
                tile_size_px: 4200,
                gap_x_px: 250,
                gap_y_px: 300,
            })
        );
    }

    #[test]
    fn outer_border_percent_overrides_legacy_px() {
        let config = parse_config(json!({
            "image_paths": [],
            "output_dir": ".",
            "prefix": "test",
            "final_size": 20000,
            "outer_border_px": 100,
            "outer_border_percent": 10
        }));

        assert_eq!(config.explicit_outer_border_px(), Some(2000));
    }

    #[test]
    fn target_aspect_ratio_deserializes_and_uses_final_size_as_long_edge() {
        let config = parse_config(json!({
            "image_paths": [],
            "output_dir": ".",
            "prefix": "test",
            "final_size": 1000,
            "target_aspect_ratio": {
                "width": 3,
                "height": 4
            }
        }));

        assert_eq!(
            config.target_aspect_ratio,
            Some(AspectRatioConfig {
                width: 3.0,
                height: 4.0
            })
        );
        assert_eq!(config.target_canvas_dimensions(), Some((750, 1000)));
    }

    #[test]
    fn overlay_position_reference_defaults_to_canvas_for_legacy_protocol() {
        let config = parse_config(json!({
            "image_paths": [],
            "output_dir": ".",
            "prefix": "test",
            "watermark": {
                "path": "watermark.png"
            },
            "text_block": {
                "text": "caption"
            }
        }));

        assert_eq!(
            config.watermark.unwrap().position_reference,
            PositionReference::Canvas
        );
        assert_eq!(
            config.text_block.unwrap().position_reference,
            PositionReference::Canvas
        );
    }

    #[test]
    fn overlay_position_reference_accepts_content_mode() {
        let config = parse_config(json!({
            "image_paths": [],
            "output_dir": ".",
            "prefix": "test",
            "watermark": {
                "path": "watermark.png",
                "position_reference": "content"
            },
            "text_block": {
                "text": "caption",
                "position_reference": "content"
            }
        }));

        assert_eq!(
            config.watermark.unwrap().position_reference,
            PositionReference::Content
        );
        assert_eq!(
            config.text_block.unwrap().position_reference,
            PositionReference::Content
        );
    }

    #[test]
    fn landscape_target_aspect_ratio_uses_width_as_long_edge() {
        let config = parse_config(json!({
            "image_paths": [],
            "output_dir": ".",
            "prefix": "test",
            "final_size": 1000,
            "target_aspect_ratio": {
                "width": 4,
                "height": 3
            }
        }));

        assert_eq!(config.target_canvas_dimensions(), Some((1000, 750)));
    }
}
