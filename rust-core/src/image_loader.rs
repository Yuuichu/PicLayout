use std::path::Path;

use image::{error::LimitErrorKind, DynamicImage, ImageError, ImageReader, Limits};

use crate::{error::AppError, metadata, ultrahdr_output};

const HEIC_DECODE_MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

const DECODE_MAX_ALLOC_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const RGBA_BYTES_PER_PIXEL: u64 = 4;

pub struct LoadedImage {
    /// 8-bit SDR base image (tone-mapped if source was HDR)
    pub image: DynamicImage,
    pub orientation: Option<u16>,
    pub icc_profile: Option<Vec<u8>>,
    /// Per-image warnings (missing ICC, HDR tone-mapped, etc.)
    pub warnings: Vec<String>,
    /// Gain map data for Ultra HDR JPEG output (only populated for HDR HEIC sources)
    pub gain_map: Option<ultrahdr_output::GainMapData>,
}

impl LoadedImage {
    fn new(image: DynamicImage, orientation: Option<u16>, icc_profile: Option<Vec<u8>>) -> Self {
        Self {
            image,
            orientation,
            icc_profile,
            warnings: Vec::new(),
            gain_map: None,
        }
    }
}

pub fn load_image(path: &Path) -> Result<LoadedImage, AppError> {
    if is_jpeg_path(path) {
        if let Ok(loaded) = load_jpeg_turbo(path) {
            return Ok(loaded);
        }
    }

    if is_heif_path(path) {
        return load_heif(path);
    }

    let image = open_image(path)?;
    Ok(LoadedImage::new(
        image,
        metadata::read_orientation(path),
        metadata::extract_icc_profile(path)?,
    ))
}

pub fn open_image(path: &Path) -> Result<DynamicImage, AppError> {
    if is_jpeg_path(path) {
        if let Ok(loaded) = load_jpeg_turbo(path) {
            return Ok(loaded.image);
        }
    }

    if is_heif_path(path) {
        return load_heif(path).map(|loaded| loaded.image);
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

fn is_heif_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "heic" | "heif"))
        .unwrap_or(false)
}

fn load_heif(path: &Path) -> Result<LoadedImage, AppError> {
    let data = std::fs::read(path).map_err(|e| {
        AppError::Processing(format!(
            "failed to read HEIC file {}: {}",
            path.display(),
            e
        ))
    })?;

    if data.len() as u64 > HEIC_DECODE_MAX_FILE_BYTES {
        return Err(AppError::Processing(format!(
            "HEIC file {} is {:.1}MB, above the {:.1}MB safety limit",
            path.display(),
            data.len() as f64 / (1024.0 * 1024.0),
            HEIC_DECODE_MAX_FILE_BYTES as f64 / (1024.0 * 1024.0),
        )));
    }

    // Probe bit depth before full decode
    let info = heif::probe(&data).map_err(|e| {
        AppError::Processing(format!(
            "failed to probe HEIC file {}: {}",
            path.display(),
            e
        ))
    })?;

    let is_hdr = info.bit_depth != heif::BitDepth::Eight;
    let bit_depth = match info.bit_depth {
        heif::BitDepth::Eight => 8u32,
        heif::BitDepth::Ten => 10u32,
        heif::BitDepth::Twelve => 12u32,
        _ => 8u32,
    };

    // Decode: heif-rs returns 16-bit DynamicImage when source is >8-bit
    let decoded = heif::decode(&data).map_err(|e| {
        AppError::Processing(format!(
            "failed to decode HEIC file {}: {}",
            path.display(),
            e
        ))
    })?;

    let (w, h) = (decoded.width(), decoded.height());
    ensure_rgba_allocation_safe(w, h, "HEIC decode")?;

    // Tone-map HDR to SDR if needed, and compute gain map for Ultra HDR output
    let (image, mut warnings, gain_map) = if is_hdr {
        let mut hdr_warnings = vec![format!(
            "{}: {}-bit HDR HEIC tone-mapped to 8-bit SDR via Reinhard",
            path.file_name().unwrap_or_default().to_string_lossy(),
            bit_depth,
        )];

        // Extract HDR raw bytes before tone-mapping (for gain map computation)
        let hdr_rgb16 = extract_rgb16_bytes(&decoded);

        // Tone-map to SDR
        let sdr = tonemap_reinhard_16_to_8(decoded);

        // Extract SDR raw bytes (RGBA8)
        let sdr_rgba8 = extract_rgba8_bytes(&sdr);

        // Compute gain map from HDR/SDR pair
        let gm = hdr_rgb16.and_then(|hdr_bytes| {
            sdr_rgba8.and_then(|sdr_bytes| {
                ultrahdr_output::compute_gainmap_from_pair(&hdr_bytes, &sdr_bytes, w, h).ok()
            })
        });

        if gm.is_some() {
            hdr_warnings.push(format!(
                "{}: Ultra HDR gain map computed (ready for HDR output)",
                path.file_name().unwrap_or_default().to_string_lossy(),
            ));
        }

        (sdr, hdr_warnings, gm)
    } else {
        (decoded, Vec::new(), None)
    };

    // Read EXIF orientation (kamadak-exif 0.6.1+ supports HEIF containers)
    let orientation = metadata::read_orientation_from_bytes(&data);
    if orientation.is_none() {
        warnings.push(format!(
            "{}: no EXIF orientation found",
            path.file_name().unwrap_or_default().to_string_lossy(),
        ));
    }

    // ICC profile (HEIF colr box parsing — future enhancement)
    let icc_profile = metadata::extract_icc_profile_from_bytes(&data).unwrap_or(None);
    if icc_profile.is_none() && is_hdr {
        warnings.push(format!(
            "{}: no ICC profile in HEIC; processing as sRGB",
            path.file_name().unwrap_or_default().to_string_lossy(),
        ));
    }

    let mut loaded = LoadedImage::new(image, orientation, icc_profile);
    loaded.warnings = warnings;
    loaded.gain_map = gain_map;
    Ok(loaded)
}

/// Apply Reinhard tone mapping to 16-bit HDR images, producing an 8-bit SDR result.
///
/// Uses luminance-preserving Reinhard: `L' = L / (1 + L)` with BT.709 luminance
/// coefficients, then scales RGB by `L' / L` to preserve saturation.
fn tonemap_reinhard_16_to_8(img: DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageRgb16(buf) => {
            let (w, h) = (buf.width(), buf.height());
            let mut out = image::RgbaImage::new(w, h);
            for (x, y, p) in buf.enumerate_pixels() {
                let r = p[0] as f32 / 65535.0;
                let g = p[1] as f32 / 65535.0;
                let b = p[2] as f32 / 65535.0;
                let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                let mapped = lum / (1.0 + lum);
                let scale = if lum > 0.001 { mapped / lum } else { 1.0 };
                out.put_pixel(
                    x,
                    y,
                    image::Rgba([
                        (r * scale * 255.0).clamp(0.0, 255.0) as u8,
                        (g * scale * 255.0).clamp(0.0, 255.0) as u8,
                        (b * scale * 255.0).clamp(0.0, 255.0) as u8,
                        255u8,
                    ]),
                );
            }
            DynamicImage::ImageRgba8(out)
        }
        DynamicImage::ImageRgba16(buf) => {
            let (w, h) = (buf.width(), buf.height());
            let mut out = image::RgbaImage::new(w, h);
            for (x, y, p) in buf.enumerate_pixels() {
                let r = p[0] as f32 / 65535.0;
                let g = p[1] as f32 / 65535.0;
                let b = p[2] as f32 / 65535.0;
                let a = (p[3] >> 8) as u8;
                let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                let mapped = lum / (1.0 + lum);
                let scale = if lum > 0.001 { mapped / lum } else { 1.0 };
                out.put_pixel(
                    x,
                    y,
                    image::Rgba([
                        (r * scale * 255.0).clamp(0.0, 255.0) as u8,
                        (g * scale * 255.0).clamp(0.0, 255.0) as u8,
                        (b * scale * 255.0).clamp(0.0, 255.0) as u8,
                        a,
                    ]),
                );
            }
            DynamicImage::ImageRgba8(out)
        }
        other => other,
    }
}

/// Extract raw 16-bit RGB pixel bytes from a DynamicImage (for gain map computation).
/// Returns packed little-endian Rgb16 bytes: [R_lo, R_hi, G_lo, G_hi, B_lo, B_hi, ...]
fn extract_rgb16_bytes(img: &DynamicImage) -> Option<Vec<u8>> {
    match img {
        DynamicImage::ImageRgb16(buf) => {
            let raw = buf.as_raw();
            let mut rgb = Vec::with_capacity(raw.len() * 2);
            for channel in raw {
                rgb.extend_from_slice(&channel.to_le_bytes());
            }
            Some(rgb)
        }
        DynamicImage::ImageRgba16(buf) => {
            let rgb_len = buf.width() as usize * buf.height() as usize * 3 * 2;
            let mut rgb = Vec::with_capacity(rgb_len);
            for pixel in buf.pixels() {
                for channel in &pixel.0[..3] {
                    rgb.extend_from_slice(&channel.to_le_bytes());
                }
            }
            Some(rgb)
        }
        _ => None,
    }
}

/// Extract raw 8-bit RGBA pixel bytes from a DynamicImage.
fn extract_rgba8_bytes(img: &DynamicImage) -> Option<Vec<u8>> {
    match img {
        DynamicImage::ImageRgba8(buf) => Some(buf.clone().into_raw()),
        DynamicImage::ImageRgb8(buf) => {
            let raw = buf.clone().into_raw();
            // Convert RGB → RGBA (add alpha=255)
            let pixel_count = raw.len() / 3;
            let mut rgba = Vec::with_capacity(pixel_count * 4);
            for chunk in raw.chunks(3) {
                rgba.extend_from_slice(chunk);
                rgba.push(255u8);
            }
            Some(rgba)
        }
        _ => None,
    }
}

fn load_jpeg_turbo(path: &Path) -> Result<LoadedImage, AppError> {
    let data = std::fs::read(path)?;
    let header = turbojpeg::read_header(&data)
        .map_err(|e| AppError::Processing(format!("turbojpeg read header failed: {}", e)))?;
    ensure_rgba_allocation_safe(header.width as u32, header.height as u32, "JPEG decode")?;

    let image: image::RgbaImage = turbojpeg::decompress_image(&data)
        .map_err(|e| AppError::Processing(format!("turbojpeg decode failed: {}", e)))?;

    let orientation = metadata::read_orientation_from_bytes(&data);
    let icc_profile = metadata::extract_icc_profile_from_bytes(&data)?;

    let mut loaded = LoadedImage::new(DynamicImage::ImageRgba8(image), orientation, icc_profile);
    if loaded.orientation.is_none() {
        loaded
            .warnings
            .push("no EXIF orientation in JPEG; auto-rotate disabled".into());
    }
    if loaded.icc_profile.is_none() {
        loaded
            .warnings
            .push("no ICC profile in JPEG; processing as sRGB".into());
    }
    Ok(loaded)
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

    #[test]
    fn heif_path_detected_by_extension() {
        assert!(is_heif_path(Path::new("photo.heic")));
        assert!(is_heif_path(Path::new("photo.heif")));
        assert!(is_heif_path(Path::new("PHOTO.HEIC")));
        assert!(is_heif_path(Path::new("photo.HEIF")));
        assert!(!is_heif_path(Path::new("photo.jpg")));
        assert!(!is_heif_path(Path::new("photo.png")));
        assert!(!is_heif_path(Path::new("photo")));
    }

    #[test]
    fn tonemap_reinhard_preserves_dimensions() {
        use image::ImageBuffer;
        let buf: ImageBuffer<image::Rgb<u16>, Vec<u16>> = ImageBuffer::from_fn(2, 2, |x, y| {
            image::Rgb([(x as u16) * 10000, (y as u16) * 20000, 32768])
        });
        let img = DynamicImage::ImageRgb16(buf);
        let result = tonemap_reinhard_16_to_8(img);
        assert_eq!(result.width(), 2);
        assert_eq!(result.height(), 2);
        assert!(matches!(result, DynamicImage::ImageRgba8(_)));
    }

    #[test]
    fn tonemap_reinhard_handles_zero_luminance() {
        use image::ImageBuffer;
        let buf: ImageBuffer<image::Rgb<u16>, Vec<u16>> =
            ImageBuffer::from_pixel(1, 1, image::Rgb([0u16, 0, 0]));
        let img = DynamicImage::ImageRgb16(buf);
        let result = tonemap_reinhard_16_to_8(img);
        assert!(matches!(result, DynamicImage::ImageRgba8(_)));
    }
}
