use std::path::Path;

const JFIF_APP0: &[u8] = b"JFIF\0";
const ICC_APP2_HEADER: &[u8] = b"ICC_PROFILE\0";
const MAX_ICC_CHUNK: usize = 65_533 - 14;

#[allow(dead_code)]
pub fn inject_dpi(path: &Path, dpi: u16) -> std::io::Result<()> {
    let data = std::fs::read(path)?;
    let patched = inject_dpi_into_jpeg(data, dpi);
    std::fs::write(path, patched)
}

pub fn inject_dpi_into_jpeg(mut data: Vec<u8>, dpi: u16) -> Vec<u8> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return data;
    }

    if has_jfif_app0_at(&data, 2) {
        let dpi_bytes = dpi.to_be_bytes();
        data[13] = 1;
        data[14] = dpi_bytes[0];
        data[15] = dpi_bytes[1];
        data[16] = dpi_bytes[0];
        data[17] = dpi_bytes[1];
        return data;
    }

    let mut segment = Vec::with_capacity(18);
    segment.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
    segment.extend_from_slice(JFIF_APP0);
    segment.extend_from_slice(&[0x01, 0x01, 0x01]);
    segment.extend_from_slice(&dpi.to_be_bytes());
    segment.extend_from_slice(&dpi.to_be_bytes());
    segment.extend_from_slice(&[0x00, 0x00]);
    data.splice(2..2, segment);
    data
}

pub fn inject_icc_into_jpeg(data: Vec<u8>, icc_profile: Option<&[u8]>) -> Vec<u8> {
    let Some(icc) = icc_profile else {
        return data;
    };
    if icc.is_empty() || data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return data;
    }

    let chunk_count = icc.len().div_ceil(MAX_ICC_CHUNK);
    if chunk_count == 0 || chunk_count > u8::MAX as usize {
        return data;
    }

    let insert_at = metadata_insert_position(&data);
    let mut segments = Vec::with_capacity(icc.len() + chunk_count * 18);
    for (idx, chunk) in icc.chunks(MAX_ICC_CHUNK).enumerate() {
        let payload_len = ICC_APP2_HEADER.len() + 2 + chunk.len();
        let segment_len = (payload_len + 2) as u16;
        segments.extend_from_slice(&[0xFF, 0xE2]);
        segments.extend_from_slice(&segment_len.to_be_bytes());
        segments.extend_from_slice(ICC_APP2_HEADER);
        segments.push((idx + 1) as u8);
        segments.push(chunk_count as u8);
        segments.extend_from_slice(chunk);
    }

    let mut out = Vec::with_capacity(data.len() + segments.len());
    out.extend_from_slice(&data[..insert_at]);
    out.extend_from_slice(&segments);
    out.extend_from_slice(&data[insert_at..]);
    out
}

fn has_jfif_app0_at(data: &[u8], pos: usize) -> bool {
    if pos + 18 > data.len() || data[pos] != 0xFF || data[pos + 1] != 0xE0 {
        return false;
    }
    let len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
    len >= 16 && pos + 4 + len - 2 <= data.len() && &data[pos + 4..pos + 9] == JFIF_APP0
}

fn metadata_insert_position(data: &[u8]) -> usize {
    let mut pos = 2usize;
    while pos + 4 <= data.len() && data[pos] == 0xFF {
        let marker = data[pos + 1];
        if marker == 0xE0 || marker == 0xE1 {
            let len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
            if len < 2 || pos + 2 + len > data.len() {
                break;
            }
            pos += 2 + len;
            continue;
        }
        break;
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_jpeg_without_app0() -> Vec<u8> {
        vec![0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02, 0xFF, 0xD9]
    }

    #[test]
    fn inserts_jfif_dpi_when_missing() {
        let out = inject_dpi_into_jpeg(minimal_jpeg_without_app0(), 300);

        assert_eq!(&out[0..4], &[0xFF, 0xD8, 0xFF, 0xE0]);
        assert_eq!(&out[6..11], b"JFIF\0");
        assert_eq!(out[13], 1);
        assert_eq!(u16::from_be_bytes([out[14], out[15]]), 300);
    }

    #[test]
    fn inserts_icc_app2_segments() {
        let out = inject_icc_into_jpeg(inject_dpi_into_jpeg(minimal_jpeg_without_app0(), 300), Some(&[1, 2, 3]));

        assert!(out.windows(12).any(|w| w == b"ICC_PROFILE\0"));
    }
}
