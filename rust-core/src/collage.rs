use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::{imageops, DynamicImage, GenericImageView, Rgba, RgbaImage};

use crate::{
    config::CollageConfig, error::AppError, image_loader::open_image,
    jpeg_output::save_user_jpeg_view, progress,
};

const MAX_JPEG_DIMENSION: u32 = u16::MAX as u32;

#[derive(Debug)]
pub struct ProcessedTile {
    pub image: RgbaImage,
    pub x: u32,
    pub y: u32,
}

pub fn create_final_collage_image(
    tiles: &[ProcessedTile],
    config: &CollageConfig,
    layout: &FinalCollageLayout,
) -> Result<DynamicImage, AppError> {
    if tiles.is_empty() {
        return Err(AppError::NoImagesProcessed);
    }

    let [r, g, b] = config.background_color.to_rgb();
    let bg_pixel = Rgba([r, g, b, 255]);
    let mut canvas = RgbaImage::from_pixel(layout.canvas_width, layout.canvas_height, bg_pixel);

    for tile in tiles {
        if tile.image.width() == 0 || tile.image.height() == 0 {
            continue;
        }
        imageops::overlay(&mut canvas, &tile.image, tile.x as i64, tile.y as i64);
    }

    Ok(DynamicImage::ImageRgba8(canvas))
}

pub struct FinalCollageLayout {
    pub grid_cols: u32,
    pub scale: f64,
    pub outer_border: u32,
    pub border_size: u32,
    pub canvas_width: u32,
    pub canvas_height: u32,
}

pub struct TilePlacement {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl FinalCollageLayout {
    pub fn new(
        tile_count: u32,
        config: &CollageConfig,
        outer_border: u32,
    ) -> Result<Self, AppError> {
        if tile_count == 0 {
            return Err(AppError::NoImagesProcessed);
        }
        let grid_cols = (tile_count as f64).sqrt().ceil() as u32;
        let grid_rows = (tile_count as f64 / grid_cols as f64).ceil() as u32;
        let collage_width = grid_cols
            .checked_mul(config.border_size)
            .ok_or_else(|| AppError::Processing("collage width calculation overflowed".into()))?;
        let collage_height = grid_rows
            .checked_mul(config.border_size)
            .ok_or_else(|| AppError::Processing("collage height calculation overflowed".into()))?;
        if collage_width > MAX_JPEG_DIMENSION || collage_height > MAX_JPEG_DIMENSION {
            return Err(AppError::Processing(format!(
                "collage dimensions {}x{} px exceed JPEG limit {} px",
                collage_width, collage_height, MAX_JPEG_DIMENSION
            )));
        }

        let inner_size = config.final_size.saturating_sub(2 * outer_border).max(1);
        let scale = inner_size as f64 / collage_width.max(collage_height) as f64;
        let scaled_width = (collage_width as f64 * scale).round().max(1.0) as u32;
        let scaled_height = (collage_height as f64 * scale).round().max(1.0) as u32;
        let canvas_width = scaled_width
            .checked_add(2 * outer_border)
            .ok_or_else(|| AppError::Processing("final width calculation overflowed".into()))?;
        let canvas_height = scaled_height
            .checked_add(2 * outer_border)
            .ok_or_else(|| AppError::Processing("final height calculation overflowed".into()))?;

        Ok(Self {
            grid_cols,
            scale,
            outer_border,
            border_size: config.border_size,
            canvas_width,
            canvas_height,
        })
    }

    pub fn tile_placement(&self, index: u32, image_width: u32, image_height: u32) -> TilePlacement {
        let col = index % self.grid_cols;
        let row = index / self.grid_cols;
        let tile_x = col * self.border_size;
        let tile_y = row * self.border_size;
        let offset_x = self.border_size.saturating_sub(image_width) / 2;
        let offset_y = self.border_size.saturating_sub(image_height) / 2;

        let x0 = scale_coord(tile_x + offset_x, self.scale) + self.outer_border;
        let y0 = scale_coord(tile_y + offset_y, self.scale) + self.outer_border;
        let x1 = scale_coord(tile_x + offset_x + image_width, self.scale) + self.outer_border;
        let y1 = scale_coord(tile_y + offset_y + image_height, self.scale) + self.outer_border;

        TilePlacement {
            x: x0,
            y: y0,
            width: x1.saturating_sub(x0).max(1),
            height: y1.saturating_sub(y0).max(1),
        }
    }
}

fn scale_coord(value: u32, scale: f64) -> u32 {
    (value as f64 * scale).round() as u32
}

#[allow(dead_code)]
pub fn create_collage_image(
    tiles: &[ProcessedTile],
    config: &CollageConfig,
) -> Result<DynamicImage, AppError> {
    if tiles.is_empty() {
        return Err(AppError::NoImagesProcessed);
    }

    let num = tiles.len() as u32;
    let grid_cols = (num as f64).sqrt().ceil() as u32;
    let grid_rows = (num as f64 / grid_cols as f64).ceil() as u32;
    let bs = config.border_size;
    let total_width = grid_cols
        .checked_mul(bs)
        .ok_or_else(|| AppError::Processing("collage width calculation overflowed".into()))?;
    let total_height = grid_rows
        .checked_mul(bs)
        .ok_or_else(|| AppError::Processing("collage height calculation overflowed".into()))?;
    if total_width > MAX_JPEG_DIMENSION || total_height > MAX_JPEG_DIMENSION {
        return Err(AppError::Processing(format!(
            "collage dimensions {}x{} px exceed JPEG limit {} px",
            total_width, total_height, MAX_JPEG_DIMENSION
        )));
    }

    let [r, g, b] = config.background_color.to_rgb();
    let mut canvas = RgbaImage::from_pixel(total_width, total_height, Rgba([r, g, b, 255]));
    for (index, tile) in tiles.iter().enumerate() {
        if tile.image.width() != bs || tile.image.height() != bs {
            return Err(AppError::Processing(
                "processed tile size does not match border_size".into(),
            ));
        }
        let index = index as u32;
        let x = (index % grid_cols) * bs;
        let y = (index / grid_cols) * bs;
        imageops::overlay(&mut canvas, &tile.image, x as i64, y as i64);
    }

    progress::send(&progress::ProgressMessage::StageChanged {
        stage: progress::Stage::CreatingCollage,
        message: format!("Collage image created ({}x{} grid)", grid_cols, grid_rows),
        elapsed_ms: 0,
    });

    Ok(DynamicImage::ImageRgba8(canvas))
}

#[allow(dead_code)]
pub fn create_collage(
    images: &[PathBuf],
    output_path: &Path,
    config: &CollageConfig,
    icc_profile: Option<&[u8]>,
) -> Result<(), AppError> {
    if images.is_empty() {
        return Err(AppError::NoImagesProcessed);
    }

    let num = images.len() as u32;
    let grid_cols = (num as f64).sqrt().ceil() as u32;
    let grid_rows = (num as f64 / grid_cols as f64).ceil() as u32;
    let bs = config.border_size;
    let total_width = grid_cols
        .checked_mul(bs)
        .ok_or_else(|| AppError::Processing("collage width calculation overflowed".into()))?;
    let total_height = grid_rows
        .checked_mul(bs)
        .ok_or_else(|| AppError::Processing("collage height calculation overflowed".into()))?;
    if total_width > MAX_JPEG_DIMENSION || total_height > MAX_JPEG_DIMENSION {
        return Err(AppError::Processing(format!(
            "collage dimensions {}x{} px exceed JPEG limit {} px",
            total_width, total_height, MAX_JPEG_DIMENSION
        )));
    }

    let view = CollageView::new(images, grid_cols, grid_rows, bs, &config.background_color)?;
    save_user_jpeg_view(&view, output_path, config, icc_profile)?;

    progress::send(&progress::ProgressMessage::StageChanged {
        stage: progress::Stage::CreatingCollage,
        message: format!("Collage image created ({}x{} grid)", grid_cols, grid_rows),
        elapsed_ms: 0,
    });

    Ok(())
}

struct CachedRow {
    row: u32,
    tiles: HashMap<usize, RgbaImage>,
}

struct CollageView<'a> {
    image_paths: &'a [PathBuf],
    grid_cols: u32,
    border_size: u32,
    width: u32,
    height: u32,
    bg_pixel: Rgba<u8>,
    cache: RefCell<CachedRow>,
}

impl<'a> CollageView<'a> {
    fn new(
        image_paths: &'a [PathBuf],
        grid_cols: u32,
        grid_rows: u32,
        border_size: u32,
        bg: &crate::config::BackgroundColor,
    ) -> Result<Self, AppError> {
        for path in image_paths {
            let img = open_image(path)?;
            if img.width() != border_size || img.height() != border_size {
                return Err(AppError::Processing(format!(
                    "temporary collage tile size mismatch: {}",
                    path.display()
                )));
            }
        }

        let [r, g, b] = bg.to_rgb();
        Ok(Self {
            image_paths,
            grid_cols,
            border_size,
            width: grid_cols * border_size,
            height: grid_rows * border_size,
            bg_pixel: Rgba([r, g, b, 255]),
            cache: RefCell::new(CachedRow {
                row: u32::MAX,
                tiles: HashMap::new(),
            }),
        })
    }

    fn load_tile_pixel(&self, index: usize, row: u32, x: u32, y: u32) -> Rgba<u8> {
        let mut cache = self.cache.borrow_mut();
        if cache.row != row {
            cache.row = row;
            cache.tiles.clear();
        }
        cache.tiles.entry(index).or_insert_with(|| {
            open_image(&self.image_paths[index])
                .expect("processed collage tile should remain readable")
                .to_rgba8()
        });

        *cache
            .tiles
            .get(&index)
            .expect("collage tile cache should be populated")
            .get_pixel(x, y)
    }
}

impl GenericImageView for CollageView<'_> {
    type Pixel = Rgba<u8>;

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        let col = x / self.border_size;
        let row = y / self.border_size;
        let index = (row * self.grid_cols + col) as usize;
        if index >= self.image_paths.len() {
            return self.bg_pixel;
        }

        self.load_tile_pixel(index, row, x % self.border_size, y % self.border_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        BackgroundColor, CollageConfig, ColorManagementConfig, OutputSettings, ProcessingMode,
        RenderingIntent, TargetProfileMode,
    };
    use std::fs;

    fn save_tile(path: &Path, size: u32, color: [u8; 4]) {
        let img = RgbaImage::from_pixel(size, size, Rgba(color));
        img.save(path).unwrap();
    }

    #[test]
    fn collage_view_keeps_loaded_tiles_available_after_switching_tiles() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.png");
        let second = dir.path().join("second.png");
        save_tile(&first, 4, [10, 20, 30, 255]);
        save_tile(&second, 4, [40, 50, 60, 255]);
        let paths = vec![first.clone(), second];
        let view = CollageView::new(&paths, 2, 1, 4, &BackgroundColor::White).unwrap();

        assert_eq!(view.get_pixel(0, 0).0, [10, 20, 30, 255]);
        assert_eq!(view.get_pixel(4, 0).0, [40, 50, 60, 255]);
        fs::remove_file(first).unwrap();

        assert_eq!(view.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn final_collage_overlays_pre_resized_tiles_without_scaling() {
        let config = CollageConfig {
            image_paths: vec![],
            image_rotations: Default::default(),
            processing_mode: ProcessingMode::StandardHighQuality,
            output_dir: std::path::PathBuf::new(),
            prefix: "test".into(),
            resample_size: 4,
            border_size: 10,
            final_size: 10,
            dpi: 300,
            background_color: BackgroundColor::White,
            watermark: None,
            overwrite: false,
            output_settings: OutputSettings::default(),
            color_management: ColorManagementConfig {
                enabled: false,
                target_profile: TargetProfileMode::Srgb,
                target_profile_path: None,
                rendering_intent: RenderingIntent::Perceptual,
            },
        };
        let layout = FinalCollageLayout {
            grid_cols: 1,
            scale: 1.0,
            outer_border: 0,
            border_size: 10,
            canvas_width: 10,
            canvas_height: 10,
        };
        let tile = ProcessedTile {
            image: RgbaImage::from_pixel(2, 2, Rgba([200, 0, 0, 255])),
            x: 3,
            y: 4,
        };

        let out = create_final_collage_image(&[tile], &config, &layout)
            .unwrap()
            .to_rgba8();

        assert_eq!(out.get_pixel(3, 4).0, [200, 0, 0, 255]);
        assert_eq!(out.get_pixel(4, 5).0, [200, 0, 0, 255]);
        assert_eq!(out.get_pixel(5, 5).0, [255, 255, 255, 255]);
    }
}
