//! Reading PDFs and running PDF readers, shared by the PDF integration tests.
//!
//! Tests run the PDF validators `qpdf`, Poppler (`pdftotext`, `pdftoppm`, `pdffonts`), MuPDF
//! (`mutool`) and pdf.js (Firefox's PDF reader, run with `node`; the `pdfjs-dist` package is
//! looked up at `UMBER_PDFJS_DIST`, else at `~/.local/share/pdfjs/node_modules/pdfjs-dist`).
//! Where a tool is not installed a test skips its use of it with a note on stderr, unless the
//! environment variable `UMBER_REQUIRE_PDF_TOOLS` is set, which makes a missing tool a failure.

// clippy's allow-in-tests settings cover only `#[test]` functions, not the helpers here; each
// includer uses a different part of this module.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, dead_code)]

use std::path::{Path as FsPath, PathBuf};
use std::process::Command;

#[path = "../../src/test_support/image.rs"]
mod image;

// --- Reading the PDF ---------------------------------------------------------------------------

/// The dictionary text of object `number`, up to its `stream` keyword or `endobj`.
pub fn object_dict(pdf: &[u8], number: u32) -> String {
    let start = find(pdf, format!("\n{number} 0 obj\n").as_bytes())
        .unwrap_or_else(|| panic!("no object {number}"));
    let rest = &pdf[start..];
    let end = [b"stream\n".as_slice(), b"endobj".as_slice()]
        .iter()
        .filter_map(|marker| find(rest, marker))
        .min()
        .unwrap();
    String::from_utf8_lossy(&rest[..end]).into_owned()
}

/// The decoded data of the Flate stream of object `number`.
pub fn stream_data(pdf: &[u8], number: u32) -> Vec<u8> {
    let dict = object_dict(pdf, number);
    assert!(dict.contains("/Filter /FlateDecode"), "{dict}");
    let length: usize = after(&dict, "/Length ")
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let start = find(pdf, format!("\n{number} 0 obj\n").as_bytes()).unwrap();
    let data_start = start + find(&pdf[start..], b"stream\n").unwrap() + b"stream\n".len();
    miniz_oxide::inflate::decompress_to_vec_zlib(&pdf[data_start..data_start + length]).unwrap()
}

pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// The text after the first `marker` in `s`.
pub fn after<'a>(s: &'a str, marker: &str) -> &'a str {
    let at = s
        .find(marker)
        .unwrap_or_else(|| panic!("no {marker} in {s}"));
    &s[at + marker.len()..]
}

/// Every object number that `key` refers to (`key N 0 R`), in file order.
pub fn references(pdf: &[u8], key: &str) -> Vec<u32> {
    let text = String::from_utf8_lossy(pdf);
    text.match_indices(&format!("{key} "))
        .filter_map(|(at, m)| {
            let rest = &text[at + m.len()..];
            let mut parts = rest.split(' ');
            let number = parts.next()?.parse().ok()?;
            (parts.next()? == "0" && parts.next()?.starts_with('R')).then_some(number)
        })
        .collect()
}

// --- External tools ------------------------------------------------------------------------------

/// Whether `tool` can be run. When it cannot, the test is skipped with a note, or fails when
/// `UMBER_REQUIRE_PDF_TOOLS` is set.
pub fn have(tool: &str) -> bool {
    let found = Command::new(tool).arg("-v").output().is_ok();
    if !found {
        assert!(
            std::env::var_os("UMBER_REQUIRE_PDF_TOOLS").is_none(),
            "{tool} is required but cannot be run"
        );
        eprintln!("skipped: {tool} is not installed");
    }
    found
}

/// A fresh scratch directory for one test.
pub fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("umber-pdf-tests")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn write(dir: &FsPath, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// Runs `qpdf --check` and asserts that it finds nothing to warn about.
pub fn assert_qpdf_clean(path: &FsPath) {
    let out = Command::new("qpdf")
        .arg("--check")
        .arg(path)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stderr.trim().is_empty() && !stdout.contains("WARNING"),
        "qpdf --check {}: {}\n{stdout}\n{stderr}",
        path.display(),
        out.status
    );
    assert!(
        stdout.contains("No syntax or stream encoding errors"),
        "{stdout}"
    );
}

/// The text `pdftotext` extracts.
pub fn extracted_text(path: &FsPath) -> String {
    let out = Command::new("pdftotext")
        .arg("-enc")
        .arg("UTF-8")
        .arg(path)
        .arg("-")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// An RGB raster.
pub struct Raster {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
}

/// The page rasterized by `pdftoppm` at `dpi`, as RGB.
pub fn poppler_raster(path: &FsPath, dpi: u32) -> Raster {
    let prefix = path.with_extension("");
    let out = Command::new("pdftoppm")
        .args(["-r", &dpi.to_string(), "-singlefile"])
        .arg(path)
        .arg(&prefix)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ppm = std::fs::read(prefix.with_extension("ppm")).unwrap();
    // P6, width, height, 255, then one whitespace byte before the samples.
    let mut fields = Vec::new();
    let mut at = 0;
    while fields.len() < 4 {
        while ppm[at].is_ascii_whitespace() {
            at += 1;
        }
        let start = at;
        while !ppm[at].is_ascii_whitespace() {
            at += 1;
        }
        fields.push(String::from_utf8_lossy(&ppm[start..at]).into_owned());
    }
    assert_eq!((fields[0].as_str(), fields[3].as_str()), ("P6", "255"));
    let width: usize = fields[1].parse().unwrap();
    let height: usize = fields[2].parse().unwrap();
    let rgb = ppm[at + 1..].to_vec();
    assert_eq!(rgb.len(), width * height * 3);
    Raster { width, height, rgb }
}

/// The PNG rendered at `dpi`, composited over white and reduced to RGB.
pub fn png_raster(png: &[u8]) -> Raster {
    let decoded = image::decode_rgba8(png).unwrap();
    let rgb = decoded
        .rgba
        .chunks_exact(4)
        .flat_map(|px| {
            let a = u32::from(px[3]);
            [0, 1, 2].map(|c| ((u32::from(px[c]) * a + 255 * (255 - a) + 127) / 255) as u8)
        })
        .collect();
    Raster {
        width: decoded.width as usize,
        height: decoded.height as usize,
        rgb,
    }
}

/// How far two rasters of the same size are apart: the mean absolute channel difference, and
/// the share of pixels with a channel more than 64 apart.
pub fn raster_difference(a: &Raster, b: &Raster) -> (f64, f64) {
    assert_eq!((a.width, a.height), (b.width, b.height));
    let mut total = 0_u64;
    let mut far = 0_usize;
    for (pa, pb) in a.rgb.chunks_exact(3).zip(b.rgb.chunks_exact(3)) {
        let deltas = [0, 1, 2].map(|c| pa[c].abs_diff(pb[c]));
        total += deltas.iter().map(|&d| u64::from(d)).sum::<u64>();
        if deltas.iter().any(|&d| d > 64) {
            far += 1;
        }
    }
    let pixels = a.width * a.height;
    (
        total as f64 / (pixels * 3) as f64,
        far as f64 / pixels as f64,
    )
}

/// The largest mean absolute channel difference between the Poppler raster of a PDF and the PNG
/// of the same scene; antialiasing and glyph rasterization differ between the two rasterizers.
pub const MAX_MEAN_DIFFERENCE: f64 = 2.0;
/// The largest share of pixels with a channel more than 64 apart.
pub const MAX_FAR_SHARE: f64 = 0.01;

pub fn assert_matches_png(pdf_path: &FsPath, png: &[u8], dpi: u32) {
    let (mean, far) = raster_difference(&poppler_raster(pdf_path, dpi), &png_raster(png));
    eprintln!(
        "{}: mean difference {mean:.4}, far share {far:.5}",
        pdf_path.display()
    );
    assert!(
        mean <= MAX_MEAN_DIFFERENCE && far <= MAX_FAR_SHARE,
        "{}: mean difference {mean:.4} (at most {MAX_MEAN_DIFFERENCE}), far share {far:.5} \
         (at most {MAX_FAR_SHARE})",
        pdf_path.display()
    );
}

/// The page content stream, object 4 of every document the backend writes, as text.
pub fn page_content(pdf: &[u8]) -> String {
    String::from_utf8_lossy(&stream_data(pdf, 4)).into_owned()
}

/// Asserts that the content-stream readers of Poppler and MuPDF draw the page without a
/// message. `qpdf --check` does not parse content streams, so it cannot show this. MuPDF's
/// notice that it was built without color management is not about the document and is ignored.
pub fn assert_readers_parse(path: &FsPath) {
    if have("pdftoppm") {
        let out = Command::new("pdftoppm")
            .args(["-r", "36", "-singlefile"])
            .arg(path)
            .arg(path.with_extension("parse"))
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success() && stderr.trim().is_empty(),
            "pdftoppm {}: {}\n{stderr}",
            path.display(),
            out.status
        );
    }
    if have("mutool") {
        let out = Command::new("mutool")
            .args(["draw", "-q", "-r", "36", "-o"])
            .arg(path.with_extension("mupdf.png"))
            .arg(path)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        let messages: Vec<&str> = stderr
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.contains("ICC support"))
            .collect();
        assert!(
            out.status.success() && messages.is_empty(),
            "mutool draw {}: {}\n{stderr}",
            path.display(),
            out.status
        );
    }
}

/// The script that prints the text pdf.js extracts from page 1: its arguments are the path of
/// pdf.js's `pdf.mjs` and the path of the PDF.
const PDFJS_TEXT: &str = r#"
import fs from 'node:fs';
import { pathToFileURL } from 'node:url';
const pdfjs = await import(pathToFileURL(process.argv[2]).href);
const data = new Uint8Array(fs.readFileSync(process.argv[3]));
const doc = await pdfjs.getDocument({ data, useSystemFonts: false, verbosity: 0 }).promise;
const page = await doc.getPage(1);
const content = await page.getTextContent({ disableNormalization: true });
process.stdout.write(content.items.filter((item) => 'str' in item).map((item) => item.str).join(''));
"#;

/// The text pdf.js extracts from page 1 of `path`, or `None` (with a note, or a failure when
/// the tools are required) when node or pdf.js is not installed.
pub fn pdfjs_text(path: &FsPath) -> Option<String> {
    let dist = std::env::var_os("UMBER_PDFJS_DIST")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".local/share/pdfjs/node_modules/pdfjs-dist"))
        });
    let module = dist
        .map(|dist| dist.join("legacy/build/pdf.mjs"))
        .filter(|module| module.is_file());
    let Some(module) = module.filter(|_| have("node")) else {
        assert!(
            std::env::var_os("UMBER_REQUIRE_PDF_TOOLS").is_none(),
            "pdf.js is required but not installed"
        );
        eprintln!("skipped: pdf.js is not installed");
        return None;
    };
    let script = path.with_extension("pdfjs.mjs");
    std::fs::write(&script, PDFJS_TEXT).unwrap();
    let out = Command::new("node")
        .arg(&script)
        .arg(&module)
        .arg(path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "pdf.js {}: {}",
        path.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    Some(String::from_utf8(out.stdout).unwrap())
}
