use image::Rgba;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct CollageConfig {
    pub image_paths: Vec<PathBuf>,
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

fn default_resample_size() -> u32 { 4000 }
fn default_border_size() -> u32 { 4200 }
fn default_final_size() -> u32 { 10000 }
fn default_dpi() -> u32 { 300 }
fn default_watermark_scale() -> f32 { 100.0 }
fn default_watermark_x() -> f32 { 50.0 }
fn default_watermark_y() -> f32 { 95.0 }
