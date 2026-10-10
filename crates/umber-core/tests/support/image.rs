//! PNG decoding and pixel sampling shared by the unit tests and the integration tests.
//!
//! This file is included with `#[path]` from `src/test_support/mod.rs` and from the integration
//! tests, so it may only use the `png` crate and `std`.

// Each includer uses a different part of this module.
#![allow(dead_code)]

/// A decoded 8-bit RGBA PNG.
pub struct DecodedPng {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA, four bytes per pixel.
    pub rgba: Vec<u8>,
    /// The `pHYs` chunk, if the file has one.
    pub phys: Option<::png::PixelDimensions>,
}

impl DecodedPng {
    /// The RGBA bytes of the pixel at column `x`, row `y`.
    pub fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

/// Decodes `bytes`, which must be an 8-bit RGBA PNG.
pub fn decode_rgba8(bytes: &[u8]) -> Result<DecodedPng, String> {
    let mut reader = ::png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| format!("cannot read PNG header: {e}"))?;
    let size = reader
        .output_buffer_size()
        .ok_or("PNG is too large to decode")?;
    let mut buf = vec![0; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("cannot decode PNG: {e}"))?;
    if info.color_type != ::png::ColorType::Rgba || info.bit_depth != ::png::BitDepth::Eight {
        return Err(format!(
            "expected 8-bit RGBA, got {:?} {:?}",
            info.color_type, info.bit_depth
        ));
    }
    buf.truncate(info.buffer_size());
    Ok(DecodedPng {
        width: info.width,
        height: info.height,
        rgba: buf,
        phys: reader.info().pixel_dims,
    })
}
