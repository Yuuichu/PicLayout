use std::path::Path;

use image::{error::LimitErrorKind, DynamicImage, ImageError, ImageReader, Limits};

use crate::{error::AppError, metadata};

const DECODE_MAX_ALLOC_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const RGBA_BYTES_PER_PIXEL: u64 = 4;

pub struct LoadedImage {
    pub image: DynamicImage,
    pub orientation: Option<u16>,
    pub icc_profile: Option<Vec<u8>>,
}

pub fn load_image(path: &Path) -> Result<LoadedImage, AppError> {
    if is_jpeg_path(path) {
        if let Ok(loaded) = load_jpeg_turbo(path) {
            return Ok(loaded);
        }
    }

    let image = open_image(path)?;
    Ok(LoadedImage {
        image,
        orientation: metadata::read_orientation(path),
        icc_profile: metadata::extract_icc_profile(path)?,
    })
}

pub fn open_image(path: &Path) -> Result<DynamicImage, AppError> {
    if is_jpeg_path(path) {
        if let Ok(loaded) = load_jpeg_turbo(path) {
            return Ok(loaded.image);
        }
    }

    let mut reader = ImageReader::open(path)?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(DECODE_MAX_ALLOC_BYTES);
    reader.limits(limits);
    reader.decode().map_err(|err| map_decode_error(path, err))
}

pub fn ensure_rgba_allocation_safe(width: u32, height: u32, label: &str) -> Result<(), AppError> {
    let bytes = width as u64 * height as u64 * RGBA_BYTES_PER_PIXEL;
    if bytes > DECODE_MAX_ALLOC_BYTES {
        return Err(AppError::Processing(format!(
            "{} requires {:.2}GB RGBA memory, above the {:.2}GB safety limit",
            label,
            bytes as f64 / 1024.0 / 1024.0 / 1024.0,
            DECODE_MAX_ALLOC_BYTES as f64 / 1024.0 / 1024.0 / 1024.0
        )));
    }
    Ok(())
}

fn is_jpeg_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg"))
        .unwrap_or(false)
}

fn load_jpeg_turbo(path: &Path) -> Result<LoadedImage, AppError> {
    let data = std::fs::read(path)?;
    let header = turbojpeg::read_header(&data)
        .map_err(|e| AppError::Processing(format!("turbojpeg read header failed: {}", e)))?;
    ensure_rgba_allocation_safe(header.width as u32, header.height as u32, "JPEG decode")?;

    let image: image::RgbaImage = turbojpeg::decompress_image(&data)
        .map_err(|e| AppError::Processing(format!("turbojpeg decode failed: {}", e)))?;
    Ok(LoadedImage {
        image: DynamicImage::ImageRgba8(image),
        orientation: metadata::read_orientation_from_bytes(&data),
        icc_profile: metadata::extract_icc_profile_from_bytes(&data)?,
    })
}

fn map_decode_error(path: &Path, err: ImageError) -> AppError {
    if let ImageError::Limits(limit) = &err {
        if matches!(limit.kind(), LimitErrorKind::InsufficientMemory) {
            return AppError::Processing(format!(
                "image decode memory exceeds the safety limit (about 2GB): {}. Reduce tile/final size or image count.",
                path.display()
            ));
        }
    }

    AppError::Image(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::error::LimitError;

    #[test]
    fn maps_decode_memory_limit_to_actionable_error() {
        let err = ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory));

        let msg = map_decode_error(Path::new("too-large.jpg"), err).to_string();

        assert!(msg.contains("safety limit"));
        assert!(msg.contains("too-large.jpg"));
    }

    #[test]
    fn rejects_unsafe_rgba_allocation() {
        let err = ensure_rgba_allocation_safe(100_000, 100_000, "test")
            .unwrap_err()
            .to_string();

        assert!(err.contains("RGBA memory"));
    }
}
