//! Small PNG helpers for screenshots.

use crate::error::AppError;
use std::fs::File;
use std::io::{BufWriter, Read};
use std::path::Path;

/// Write tightly packed RGBA bytes (8 bits per channel) as a PNG file.
pub fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), AppError> {
    let expected = (width as usize) * (height as usize) * 4;
    if width == 0 || height == 0 || rgba.len() != expected {
        return Err(AppError::internal(
            "Screenshot pixel data has the wrong size",
        ));
    }
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| AppError::internal(format!("Cannot write PNG header: {e}")))?;
    writer
        .write_image_data(rgba)
        .map_err(|e| AppError::internal(format!("Cannot write PNG data: {e}")))
}

/// Width and height from a PNG file's header, without decoding the picture.
pub fn png_size(path: &Path) -> Result<(u32, u32), AppError> {
    let mut header = [0u8; 24];
    File::open(path)?.read_exact(&mut header)?;
    parse_png_header(&header)
}

pub fn parse_png_header(header: &[u8; 24]) -> Result<(u32, u32), AppError> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if header[..8] != SIGNATURE || &header[12..16] != b"IHDR" {
        return Err(AppError::internal("The screenshot file is not a PNG"));
    }
    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_png_reports_its_size() {
        let path = std::env::temp_dir().join(format!("ad-png-{}.png", std::process::id()));
        write_png(&path, 3, 2, &[255u8; 3 * 2 * 4]).unwrap();
        assert_eq!(png_size(&path).unwrap(), (3, 2));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn wrong_sized_pixels_are_refused() {
        let path = std::env::temp_dir().join("ad-png-bad.png");
        assert!(write_png(&path, 2, 2, &[0u8; 5]).is_err());
        assert!(write_png(&path, 0, 2, &[]).is_err());
    }

    #[test]
    fn non_png_data_is_refused() {
        assert!(parse_png_header(&[0u8; 24]).is_err());
    }
}
