use std::path::Path;

/// 向已保存的 JPEG 文件注入 DPI 信息（修改 JFIF APP0 段）
/// JFIF APP0 段格式：
///   FF E0 [length 2B] "JFIF\0" [version 2B] [units 1B] [Xdensity 2B] [Ydensity 2B] [Xthumbnail 1B] [Ythumbnail 1B]
/// units=1 表示 DPI（每英寸点数）
pub fn inject_dpi(path: &Path, dpi: u16) -> std::io::Result<()> {
    let data = std::fs::read(path)?;

    // 查找 JFIF APP0 marker (FF E0)
    if data.len() < 18 || data[0] != 0xFF || data[1] != 0xD8 {
        return Ok(()); // 不是合法 JPEG，跳过
    }

    let mut patched = data.clone();

    // APP0 紧跟在 SOI 之后，从字节 2 开始
    if patched[2] == 0xFF && patched[3] == 0xE0 {
        let marker_len = u16::from_be_bytes([patched[4], patched[5]]) as usize;
        // 确保是 JFIF APP0（identifier = "JFIF\0"）
        if marker_len >= 16 && &patched[6..11] == b"JFIF\0" {
            // units 字段在偏移 11（相对于文件开头）
            patched[11] = 1; // 1 = DPI
            // Xdensity 在偏移 12-13，Ydensity 在 14-15
            let dpi_bytes = dpi.to_be_bytes();
            patched[12] = dpi_bytes[0];
            patched[13] = dpi_bytes[1];
            patched[14] = dpi_bytes[0];
            patched[15] = dpi_bytes[1];
            std::fs::write(path, &patched)?;
        }
    }

    Ok(())
}
