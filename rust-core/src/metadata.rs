use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;

use flate2::read::ZlibDecoder;
use image::DynamicImage;

use crate::error::AppError;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub fn read_orientation(path: &Path) -> Option<u16> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    read_orientation_from_reader(&mut reader)
}

pub fn read_orientation_from_bytes(data: &[u8]) -> Option<u16> {
    let mut reader = Cursor::new(data);
    read_orientation_from_reader(&mut reader)
}

fn read_orientation_from_reader<R: std::io::BufRead + std::io::Seek>(reader: &mut R) -> Option<u16> {
    let exif = exif::Reader::new().read_from_container(&mut *reader).ok()?;
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    field.value.get_uint(0).map(|value| value as u16)
}

pub fn apply_orientation(
    img: DynamicImage,
    orientation: Option<u16>,
    enabled: bool,
) -> DynamicImage {
    if !enabled {
        return img;
    }

    match orientation.unwrap_or(1) {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.fliph().rotate90(),
        6 => img.rotate90(),
        7 => img.fliph().rotate270(),
        8 => img.rotate270(),
        _ => img,
    }
}

pub fn extract_icc_profile(path: &Path) -> Result<Option<Vec<u8>>, AppError> {
    let data = std::fs::read(path)?;
    extract_icc_profile_from_bytes(&data)
}

pub fn extract_icc_profile_from_bytes(data: &[u8]) -> Result<Option<Vec<u8>>, AppError> {
    if data.starts_with(&[0xFF, 0xD8]) {
        return Ok(extract_jpeg_icc(&data));
    }
    if data.starts_with(PNG_SIGNATURE) {
        return extract_png_icc(&data);
    }
    Ok(None)
}

fn extract_jpeg_icc(data: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 2usize;
    let mut chunks: Vec<(u8, u8, Vec<u8>)> = Vec::new();

    while pos + 4 <= data.len() {
        if data[pos] != 0xFF {
            break;
        }
        while pos < data.len() && data[pos] == 0xFF {
            pos += 1;
        }
        if pos >= data.len() {
            break;
        }

        let marker = data[pos];
        pos += 1;
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        if pos + 2 > data.len() {
            break;
        }

        let segment_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if segment_len < 2 || pos + segment_len - 2 > data.len() {
            break;
        }

        let payload = &data[pos..pos + segment_len - 2];
        if marker == 0xE2 && payload.starts_with(b"ICC_PROFILE\0") && payload.len() > 14 {
            chunks.push((payload[12], payload[13], payload[14..].to_vec()));
        }
        pos += segment_len - 2;
    }

    if chunks.is_empty() {
        return None;
    }

    chunks.sort_by_key(|(seq, _, _)| *seq);
    let expected_count = chunks[0].1;
    if expected_count == 0 || chunks.len() != expected_count as usize {
        return None;
    }

    let mut icc = Vec::new();
    for (expected_seq, (seq, count, chunk)) in (1u8..).zip(chunks) {
        if seq != expected_seq || count != expected_count {
            return None;
        }
        icc.extend_from_slice(&chunk);
    }
    Some(icc)
}

fn extract_png_icc(data: &[u8]) -> Result<Option<Vec<u8>>, AppError> {
    let mut pos = PNG_SIGNATURE.len();

    while pos + 12 <= data.len() {
        let len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if pos + 4 + len + 4 > data.len() {
            break;
        }

        let chunk_type = &data[pos..pos + 4];
        pos += 4;
        let chunk_data = &data[pos..pos + len];
        pos += len + 4; // skip data and CRC

        if chunk_type == b"iCCP" {
            let Some(null_pos) = chunk_data.iter().position(|b| *b == 0) else {
                return Ok(None);
            };
            if null_pos + 2 > chunk_data.len() || chunk_data[null_pos + 1] != 0 {
                return Ok(None);
            }
            let compressed = &chunk_data[null_pos + 2..];
            let mut decoder = ZlibDecoder::new(compressed);
            let mut icc = Vec::new();
            decoder.read_to_end(&mut icc)?;
            return Ok(Some(icc));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer};

    #[test]
    fn apply_orientation_rotates_90_degrees() {
        let img = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2, 3, image::Rgb([1, 2, 3])));

        let out = apply_orientation(img, Some(6), true);

        assert_eq!(out.width(), 3);
        assert_eq!(out.height(), 2);
    }
}
