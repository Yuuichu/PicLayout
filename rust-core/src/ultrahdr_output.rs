//! Ultra HDR JPEG gain-map computation and ISO 21496-1 packaging.

use image::{codecs::jpeg::JpegEncoder, GrayImage, Luma};
use ultrahdr_rs::{encode_ultrahdr, ColorGamut, GainMapMetadata};

use crate::error::AppError;

const GAIN_MAP_SCALE: u32 = 4;
const MIN_BOOST_LOG2: f32 = -3.0;
const MAX_BOOST_LOG2: f32 = 3.0;
pub const NEUTRAL_GAIN_VALUE: u8 = 128;

/// A raw gain map that can still be transformed with its source image.
#[derive(Debug, Clone)]
pub struct GainMapData {
    pub image: GrayImage,
    pub metadata: GainMapMetadata,
}

impl GainMapData {
    pub fn new(image: GrayImage) -> Self {
        Self {
            image,
            metadata: gain_map_metadata(),
        }
    }
}

/// Compute a quarter-resolution luminance gain map from an HDR/SDR pixel pair.
pub fn compute_gainmap_from_pair(
    hdr_rgb16: &[u8],
    sdr_rgba8: &[u8],
    width: u32,
    height: u32,
) -> Result<GainMapData, AppError> {
    let pixel_count = width as usize * height as usize;
    let expected_hdr_len = pixel_count
        .checked_mul(6)
        .ok_or_else(|| AppError::Processing("HDR gain map size overflowed".into()))?;
    let expected_sdr_len = pixel_count
        .checked_mul(4)
        .ok_or_else(|| AppError::Processing("SDR gain map size overflowed".into()))?;
    if hdr_rgb16.len() < expected_hdr_len || sdr_rgba8.len() < expected_sdr_len {
        return Err(AppError::Processing(
            "HDR/SDR buffers are smaller than their declared dimensions".into(),
        ));
    }

    let map_width = width.div_ceil(GAIN_MAP_SCALE);
    let map_height = height.div_ceil(GAIN_MAP_SCALE);
    let mut gain_map = GrayImage::new(map_width, map_height);

    for map_y in 0..map_height {
        for map_x in 0..map_width {
            let mut log_gain_sum = 0.0f32;
            let mut sample_count = 0u32;
            let y_start = map_y * GAIN_MAP_SCALE;
            let y_end = (y_start + GAIN_MAP_SCALE).min(height);
            let x_start = map_x * GAIN_MAP_SCALE;
            let x_end = (x_start + GAIN_MAP_SCALE).min(width);

            for y in y_start..y_end {
                for x in x_start..x_end {
                    let pixel_index = y as usize * width as usize + x as usize;
                    let hdr_index = pixel_index * 6;
                    let sdr_index = pixel_index * 4;
                    let hdr = [
                        u16::from_le_bytes([hdr_rgb16[hdr_index], hdr_rgb16[hdr_index + 1]]) as f32
                            / u16::MAX as f32,
                        u16::from_le_bytes([hdr_rgb16[hdr_index + 2], hdr_rgb16[hdr_index + 3]])
                            as f32
                            / u16::MAX as f32,
                        u16::from_le_bytes([hdr_rgb16[hdr_index + 4], hdr_rgb16[hdr_index + 5]])
                            as f32
                            / u16::MAX as f32,
                    ];
                    let sdr = [
                        sdr_rgba8[sdr_index] as f32 / u8::MAX as f32,
                        sdr_rgba8[sdr_index + 1] as f32 / u8::MAX as f32,
                        sdr_rgba8[sdr_index + 2] as f32 / u8::MAX as f32,
                    ];
                    let hdr_luminance = luminance(hdr);
                    let sdr_luminance = luminance(sdr);
                    let gain = if hdr_luminance <= f32::EPSILON {
                        1.0
                    } else {
                        hdr_luminance / sdr_luminance.max(1.0 / u8::MAX as f32)
                    };
                    log_gain_sum += gain.log2().clamp(MIN_BOOST_LOG2, MAX_BOOST_LOG2);
                    sample_count += 1;
                }
            }

            let average_log_gain = if sample_count == 0 {
                0.0
            } else {
                log_gain_sum / sample_count as f32
            };
            gain_map.put_pixel(map_x, map_y, Luma([encode_log_gain(average_log_gain)]));
        }
    }

    Ok(GainMapData::new(gain_map))
}

pub fn assemble_ultrahdr_jpeg(
    sdr_jpeg: &[u8],
    gain_map: &GainMapData,
) -> Result<Vec<u8>, AppError> {
    let gain_map_jpeg = encode_gainmap_to_jpeg(&gain_map.image)?;
    encode_ultrahdr(
        sdr_jpeg,
        &gain_map_jpeg,
        &gain_map.metadata,
        ColorGamut::Bt709,
    )
    .map_err(|error| AppError::Processing(format!("Ultra HDR assembly failed: {error}")))
}

fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

fn encode_log_gain(log_gain: f32) -> u8 {
    let normalized = (log_gain.clamp(MIN_BOOST_LOG2, MAX_BOOST_LOG2) - MIN_BOOST_LOG2)
        / (MAX_BOOST_LOG2 - MIN_BOOST_LOG2);
    (normalized * u8::MAX as f32).round() as u8
}

fn encode_gainmap_to_jpeg(image: &GrayImage) -> Result<Vec<u8>, AppError> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err(AppError::Processing("gain map cannot be empty".into()));
    }

    let mut jpeg_bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg_bytes, 85)
        .encode(image, width, height, image::ColorType::L8.into())
        .map_err(|error| AppError::Processing(format!("gain map JPEG encode failed: {error}")))?;
    Ok(jpeg_bytes)
}

fn gain_map_metadata() -> GainMapMetadata {
    let mut metadata = GainMapMetadata::new();
    metadata.gain_map_max = [MAX_BOOST_LOG2 as f64; 3];
    metadata.gain_map_min = [MIN_BOOST_LOG2 as f64; 3];
    metadata.gamma = [1.0; 3];
    metadata.base_offset = [1.0 / 64.0; 3];
    metadata.alternate_offset = [1.0 / 64.0; 3];
    metadata.base_hdr_headroom = 0.0;
    metadata.alternate_hdr_headroom = MAX_BOOST_LOG2 as f64;
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};
    use ultrahdr_rs::Decoder;

    fn uniform_hdr_sdr_pair(width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
        let mut hdr = Vec::with_capacity(width as usize * height as usize * 6);
        let mut sdr = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..width * height {
            for channel in [u16::MAX, u16::MAX, u16::MAX] {
                hdr.extend_from_slice(&channel.to_le_bytes());
            }
            sdr.extend_from_slice(&[128, 128, 128, 255]);
        }
        (hdr, sdr)
    }

    #[test]
    fn preserves_non_square_gain_map_dimensions() {
        let (hdr, sdr) = uniform_hdr_sdr_pair(9, 5);
        let gain_map = compute_gainmap_from_pair(&hdr, &sdr, 9, 5).unwrap();

        assert_eq!(gain_map.image.dimensions(), (3, 2));
        let jpeg = encode_gainmap_to_jpeg(&gain_map.image).unwrap();
        let decoded = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (3, 2));
    }

    #[test]
    fn equal_normalized_luminance_encodes_neutral_gain() {
        let mut hdr = Vec::new();
        for channel in [32896u16, 32896, 32896] {
            hdr.extend_from_slice(&channel.to_le_bytes());
        }
        let sdr = [128, 128, 128, 255];
        let gain_map = compute_gainmap_from_pair(&hdr, &sdr, 1, 1).unwrap();

        assert_eq!(gain_map.image.get_pixel(0, 0)[0], NEUTRAL_GAIN_VALUE);
    }

    #[test]
    fn assembled_output_round_trips_as_ultra_hdr() {
        let base = RgbImage::from_pixel(9, 5, Rgb([96, 128, 160]));
        let mut base_jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut base_jpeg, 90)
            .encode_image(&base)
            .unwrap();
        let gain_map = GainMapData::new(GrayImage::from_pixel(3, 2, Luma([192])));

        let output = assemble_ultrahdr_jpeg(&base_jpeg, &gain_map).unwrap();
        let decoder = Decoder::new(&output).unwrap();

        assert!(decoder.is_ultrahdr());
        assert!(decoder.metadata().is_some());
        let decoded_base = image::load_from_memory(decoder.primary_jpeg().unwrap()).unwrap();
        let decoded_gain_map = image::load_from_memory(decoder.gainmap_jpeg().unwrap()).unwrap();
        assert_eq!((decoded_base.width(), decoded_base.height()), (9, 5));
        assert_eq!(
            (decoded_gain_map.width(), decoded_gain_map.height()),
            (3, 2)
        );
    }
}
