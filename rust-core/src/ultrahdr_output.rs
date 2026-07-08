//! Ultra HDR JPEG assembly — gain map computation and ISO 21496-1 packaging.
//!
//! Produces backwards-compatible JPEGs that display as SDR on legacy viewers
//! and as HDR on Instagram / Xiaohongshu / Chrome / Android 15+.

use image::codecs::jpeg::JpegEncoder;
use image::ImageBuffer;
use ultrahdr_rs::encode_ultrahdr;
use ultrahdr_rs::GainMapMetadata;

use crate::error::AppError;

/// Pre-computed gain map data carried alongside a loaded image.
#[derive(Debug, Clone)]
pub struct GainMapData {
    /// JPEG-encoded gain map (grayscale, single-channel).
    pub gainmap_jpeg: Vec<u8>,
    /// Gain map metadata (boost range, offsets, gamma).
    pub metadata: GainMapMetadata,
}

/// Compute a gain map from an HDR/SDR pixel pair and encode it as a grayscale JPEG.
///
/// `hdr_rgb16`: packed little-endian 16-bit RGB pixels (from HeifDecoder).
/// `sdr_rgba8`: 8-bit RGBA pixels (SDR base, e.g. from Reinhard tone-mapping).
///
/// Uses single-channel luminance gain map with BT.709 coefficients,
/// log2 quantized, downscaled 1/4.
pub fn encode_gainmap_from_pair(
    hdr_rgb16: &[u8],
    sdr_rgba8: &[u8],
    width: u32,
    height: u32,
) -> Result<GainMapData, AppError> {
    let w = width as usize;
    let h = height as usize;

    // Compute the gain map
    let gainmap = compute_simple_gainmap(hdr_rgb16, sdr_rgba8, w, h)?;

    // Encode as grayscale JPEG
    let gainmap_jpeg = encode_gainmap_to_jpeg(&gainmap)?;

    // Build metadata
    let metadata = make_gainmap_metadata(&gainmap);

    Ok(GainMapData {
        gainmap_jpeg,
        metadata,
    })
}

/// Simple single-channel luminance gain map computation.
///
/// For each pixel:
///   L_hdr = 0.2126*R16 + 0.7152*G16 + 0.0722*B16  (from 16-bit LE)
///   L_sdr = 0.2126*R8  + 0.7152*G8  + 0.0722*B8   (from RGBA8, skip A)
///   ratio = L_hdr / L_sdr  (clamped to [0.125, 8.0])
///   encoded = log2(ratio) scaled to [0, 255]
///
/// Downscaled 1/4 (average of 4x4 blocks).
#[allow(clippy::needless_range_loop)]
fn compute_simple_gainmap(
    hdr_rgb16: &[u8],
    sdr_rgba8: &[u8],
    width: usize,
    height: usize,
) -> Result<Vec<u8>, AppError> {
    let gm_width = (width + 3) / 4;
    let gm_height = (height + 3) / 4;
    let mut gainmap = vec![0u8; gm_width * gm_height];

    // Constrain ratio to a reasonable range
    let min_ratio: f32 = 1.0 / 8.0; // log2 = -3.0
    let max_ratio: f32 = 8.0;       // log2 = 3.0

    for gy in 0..gm_height {
        for gx in 0..gm_width {
            let mut sum = 0.0f32;
            let mut count = 0u32;

            let y_start = gy * 4;
            let y_end = (y_start + 4).min(height);
            let x_start = gx * 4;
            let x_end = (x_start + 4).min(width);

            for y in y_start..y_end {
                for x in x_start..x_end {
                    let hdr_idx = (y * width + x) * 6;
                    let sdr_idx = (y * width + x) * 4;

                    if hdr_idx + 6 > hdr_rgb16.len() || sdr_idx + 4 > sdr_rgba8.len() {
                        continue;
                    }

                    // HDR: 16-bit LE → f32
                    let hr = u16::from_le_bytes([hdr_rgb16[hdr_idx], hdr_rgb16[hdr_idx + 1]]) as f32;
                    let hg = u16::from_le_bytes([hdr_rgb16[hdr_idx + 2], hdr_rgb16[hdr_idx + 3]]) as f32;
                    let hb = u16::from_le_bytes([hdr_rgb16[hdr_idx + 4], hdr_rgb16[hdr_idx + 5]]) as f32;

                    // SDR: 8-bit → f32
                    let sr = sdr_rgba8[sdr_idx] as f32;
                    let sg = sdr_rgba8[sdr_idx + 1] as f32;
                    let sb = sdr_rgba8[sdr_idx + 2] as f32;

                    // BT.709 luminance
                    let l_hdr = 0.2126 * hr + 0.7152 * hg + 0.0722 * hb;
                    let l_sdr = 0.2126 * sr + 0.7152 * sg + 0.0722 * sb;

                    if l_sdr > 1.0 {
                        let ratio = (l_hdr / l_sdr).clamp(min_ratio, max_ratio);
                        sum += ratio.ln();
                        count += 1;
                    }
                }
            }

            if count > 0 {
                // Average log ratio, scale to [0, 255]
                let avg_log = sum / count as f32;
                let min_log = min_ratio.ln();
                let max_log = max_ratio.ln();
                let scaled = (avg_log - min_log) / (max_log - min_log);
                gainmap[gy * gm_width + gx] = (scaled * 255.0).clamp(0.0, 255.0) as u8;
            }
        }
    }

    Ok(gainmap)
}

/// Encode a single-channel gain map as a grayscale JPEG.
fn encode_gainmap_to_jpeg(data: &[u8]) -> Result<Vec<u8>, AppError> {
    // Determine dimensions (gain map is 1/4 of original, but we just have raw data)
    // For encoding, we need to know width and height. These should match what
    // compute_simple_gainmap produced.
    let gm_width = (data.len() as f64).sqrt().ceil() as u32;
    let gm_height = if gm_width > 0 {
        data.len() as u32 / gm_width
    } else {
        0
    };

    if gm_width == 0 || gm_height == 0 {
        // Fallback: 1x1 gain map (essentially no gain map)
        let buf: ImageBuffer<image::Luma<u8>, Vec<u8>> = ImageBuffer::from_pixel(1, 1, image::Luma([0u8]));
        let mut jpeg_bytes = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, 85);
        encoder
            .encode(&buf, 1, 1, image::ColorType::L8.into())
            .map_err(|e| AppError::Processing(format!("gain map JPEG encode failed: {}", e)))?;
        return Ok(jpeg_bytes);
    }

    let padded_len = (gm_width * gm_height) as usize;
    let padded_data = if padded_len > data.len() {
        let mut p = data.to_vec();
        p.resize(padded_len, 0u8);
        p
    } else {
        data[..padded_len].to_vec()
    };

    let buf: ImageBuffer<image::Luma<u8>, Vec<u8>> = ImageBuffer::from_raw(gm_width, gm_height, padded_data)
        .ok_or_else(|| AppError::Processing("invalid gain map dimensions".into()))?;

    let mut jpeg_bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, 85);
    encoder
        .encode(&buf, gm_width, gm_height, image::ColorType::L8.into())
        .map_err(|e| AppError::Processing(format!("gain map JPEG encode failed: {}", e)))?;

    Ok(jpeg_bytes)
}

/// Build gain map metadata from the computed gain map.
fn make_gainmap_metadata(gainmap: &[u8]) -> GainMapMetadata {
    let min_val = gainmap.iter().copied().min().unwrap_or(0);
    let max_val = gainmap.iter().copied().max().unwrap_or(255);

    let min_boost_log2 = (min_val as f64 / 255.0) * 6.0 - 3.0;
    let max_boost_log2 = (max_val as f64 / 255.0) * 6.0 - 3.0;

    let mut meta = GainMapMetadata::new();
    meta.gain_map_max = [max_boost_log2; 3];
    meta.gain_map_min = [min_boost_log2; 3];
    meta.gamma = [1.0; 3];
    meta.base_offset = [1.0 / 64.0; 3];
    meta.alternate_offset = [1.0 / 64.0; 3];
    meta.base_hdr_headroom = 0.0;
    meta.alternate_hdr_headroom = max_boost_log2;
    meta
}

/// Assemble an Ultra HDR JPEG from pre-encoded components.
pub fn assemble_ultrahdr_jpeg(
    sdr_jpeg: &[u8],
    gainmap_data: &GainMapData,
) -> Result<Vec<u8>, AppError> {
    use ultrahdr_rs::ColorGamut;
    encode_ultrahdr(
        sdr_jpeg,
        &gainmap_data.gainmap_jpeg,
        &gainmap_data.metadata,
        ColorGamut::Bt709,
    )
    .map_err(|_e| AppError::Processing("Ultra HDR assembly failed".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_gainmap_for_simple_pair() {
        let w = 8u32;
        let h = 8u32;
        // HDR: uniform bright pixels (16-bit LE, 65535 = max)
        let hdr: Vec<u8> = (0..(w * h * 3 * 2) as usize)
            .map(|i| if i % 2 == 0 { 0xFF } else { 0x00 })
            .collect();
        // SDR: uniform dark pixels (RGBA8, value=128)
        let sdr: Vec<u8> = (0..(w * h * 4) as usize)
            .map(|i| if i % 4 == 3 { 255 } else { 128 })
            .collect();

        let result = encode_gainmap_from_pair(&hdr, &sdr, w, h);
        assert!(result.is_ok());
        let data = result.unwrap();
        assert!(!data.gainmap_jpeg.is_empty());
    }
}
