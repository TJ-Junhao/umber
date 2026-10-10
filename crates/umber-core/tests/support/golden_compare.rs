//! Golden-image comparison shared by the integration tests.
//!
//! References live in `tests/golden/{name}.png`. Run with `UPDATE_GOLDEN=1` to write or
//! overwrite them. A failing comparison writes `tests/golden/{name}.diff.png`, a red overlay
//! on the differing pixels, next to the reference.

// clippy's allow-in-tests settings cover only `#[test]` functions, not the helpers here.
#![allow(clippy::unwrap_used, clippy::panic, dead_code)]

use std::path::{Path as FsPath, PathBuf};

/// A pixel counts as "different" when any channel differs by more than this.
const CHANNEL_TOLERANCE: u8 = 8;
/// At most this fraction of pixels may be different.
const MAX_DIFFERENT_FRACTION: f64 = 0.005;
/// No pixel may have a channel differing by more than this.
const MAX_CHANNEL_DELTA: u8 = 64;

#[path = "../../src/test_support/image.rs"]
mod image;

use image::{DecodedPng as Image, decode_rgba8 as decode};

fn write_png(path: &FsPath, image: &Image) -> Result<(), String> {
    let file = std::fs::File::create(path)
        .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&image.rgba)
        .map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}

pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

/// Compares `png_bytes` with `{reference_dir}/{name}.png`. On failure, writes
/// `{diff_dir}/{name}.diff.png` and returns a description of the difference.
pub fn compare_golden(
    name: &str,
    png_bytes: &[u8],
    reference_dir: &FsPath,
    diff_dir: &FsPath,
) -> Result<(), String> {
    let reference_path = reference_dir.join(format!("{name}.png"));
    let reference_bytes = std::fs::read(&reference_path).map_err(|_| {
        format!(
            "missing golden image {}; run with UPDATE_GOLDEN=1 to create it",
            reference_path.display()
        )
    })?;
    let actual = decode(png_bytes)?;
    let reference = decode(&reference_bytes)?;
    if (actual.width, actual.height) != (reference.width, reference.height) {
        return Err(format!(
            "{name}: size differs: got {}x{}, reference is {}x{}",
            actual.width, actual.height, reference.width, reference.height
        ));
    }
    let mut overlay = actual.rgba.clone();
    let mut different = 0usize;
    let mut largest = 0u8;
    for (i, (a, r)) in actual
        .rgba
        .chunks(4)
        .zip(reference.rgba.chunks(4))
        .enumerate()
    {
        let delta = a
            .iter()
            .zip(r)
            .map(|(a, r)| a.abs_diff(*r))
            .max()
            .unwrap_or(0);
        largest = largest.max(delta);
        if delta > CHANNEL_TOLERANCE {
            different += 1;
        }
        if delta > 0 {
            overlay[i * 4..i * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
        }
    }
    let total = (actual.width as usize) * (actual.height as usize);
    let fraction = different as f64 / total as f64;
    if fraction <= MAX_DIFFERENT_FRACTION && largest <= MAX_CHANNEL_DELTA {
        return Ok(());
    }
    let diff_path = diff_dir.join(format!("{name}.diff.png"));
    let overlay = Image {
        width: actual.width,
        height: actual.height,
        rgba: overlay,
        phys: None,
    };
    write_png(&diff_path, &overlay)?;
    Err(format!(
        "{name}: {different} of {total} pixels ({:.3}%) differ by more than {CHANNEL_TOLERANCE} \
         (limit {:.1}%), largest channel difference {largest} (limit {MAX_CHANNEL_DELTA}); \
         diff written to {}",
        fraction * 100.0,
        MAX_DIFFERENT_FRACTION * 100.0,
        diff_path.display()
    ))
}

pub fn assert_golden(name: &str, png_bytes: &[u8]) {
    let dir = golden_dir();
    if std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::create_dir_all(&dir).unwrap();
        // Write beside the reference and rename, so an interrupted run never leaves a
        // truncated reference.
        let temp = dir.join(format!(".{name}.png.tmp"));
        std::fs::write(&temp, png_bytes).unwrap();
        std::fs::rename(&temp, dir.join(format!("{name}.png"))).unwrap();
        return;
    }
    if let Err(message) = compare_golden(name, png_bytes, &dir, &dir) {
        panic!("{message}");
    }
}
