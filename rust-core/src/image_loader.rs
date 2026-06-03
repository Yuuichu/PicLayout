use std::path::Path;

use image::{error::LimitErrorKind, DynamicImage, ImageError, ImageReader, Limits};

use crate::error::AppError;

const DECODE_MAX_ALLOC_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub fn open_image(path: &Path) -> Result<DynamicImage, AppError> {
    let mut reader = ImageReader::open(path)?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(DECODE_MAX_ALLOC_BYTES);
    reader.limits(limits);
    reader.decode().map_err(|err| map_decode_error(path, err))
}

fn map_decode_error(path: &Path, err: ImageError) -> AppError {
    if let ImageError::Limits(limit) = &err {
        if matches!(limit.kind(), LimitErrorKind::InsufficientMemory) {
            return AppError::Processing(format!(
                "图片解码需要的内存超过安全上限（约 2GB）：{}。请降低单图边框大小、最终尺寸，或减少图片数量。",
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

        assert!(msg.contains("超过安全上限"));
        assert!(msg.contains("too-large.jpg"));
    }
}
