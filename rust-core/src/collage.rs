use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::{GenericImageView, Rgba, RgbaImage};

use crate::{
    config::CollageConfig, error::AppError, image_loader::open_image,
    jpeg_output::save_user_jpeg_view,
    progress,
};

const MAX_JPEG_DIMENSION: u32 = u16::MAX as u32;

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
    let total_width = grid_cols.checked_mul(bs).ok_or_else(|| {
        AppError::Processing("拼贴图尺寸过大，宽度计算溢出".into())
    })?;
    let total_height = grid_rows.checked_mul(bs).ok_or_else(|| {
        AppError::Processing("拼贴图尺寸过大，高度计算溢出".into())
    })?;
    if total_width > MAX_JPEG_DIMENSION || total_height > MAX_JPEG_DIMENSION {
        return Err(AppError::Processing(format!(
            "拼贴图尺寸 {}×{} px 超过 JPEG 支持上限 {} px",
            total_width, total_height, MAX_JPEG_DIMENSION
        )));
    }

    let view = CollageView::new(images, grid_cols, grid_rows, bs, &config.background_color)?;
    save_user_jpeg_view(&view, output_path, config, icc_profile)?;

    progress::send(&progress::ProgressMessage::StageChanged {
        stage: progress::Stage::CreatingCollage,
        message: format!("拼贴图已创建（{}×{} 网格）", grid_cols, grid_rows),
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
                    "临时拼贴图块尺寸不匹配: {}",
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
    use crate::config::BackgroundColor;
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
}
