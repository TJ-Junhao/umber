//! Tests of the PDF backend: determinism, page size, embedded fonts and text, raster cells, and
//! agreement with the PNG of the same scene.
//!
//! Some tests run the PDF validators `qpdf` and Poppler (`pdftotext`, `pdftoppm`, `pdffonts`).
//! Where a tool is not installed they are skipped with a note on stderr, unless the environment
//! variable `UMBER_REQUIRE_PDF_TOOLS` is set, which makes a missing tool a failure.

// clippy's allow-in-tests settings cover only `#[test]` functions, not the helpers here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path as FsPath, PathBuf};
use std::process::Command;

use read_fonts::{FontRef, TableProvider};
use umber_core::figure::{AreaOptions, AxisData, BarOptions, LineOptions, ScatterOptions};
use umber_core::render::{render_pdf, render_png};
use umber_core::scene::{Item, Scene};
use umber_core::{
    Alpha, ColorSpec, FaceId, Figure, FillStyle, Font, FontLibrary, Grid, GridIndex,
    HeatmapOptions, Placement, Point, PointColor, Render, Rgba, ScaleKind, TextStyle, Theme,
    Weight,
};

#[path = "../src/test_support/image.rs"]
mod image;

const WHITE: Rgba = Rgba {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};
const INK: Rgba = Rgba {
    r: 20,
    g: 24,
    b: 31,
    a: 255,
};

/// A test font file of `tests/fixtures`.
fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn style(face: usize, size: f32) -> TextStyle {
    TextStyle {
        face: FaceId::new(face),
        weight: Weight::Regular,
        size,
        tabular_figures: false,
    }
}

/// A line chart with a title, labels and a legend.
fn line_figure() -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Quarterly revenue")
        .xlabel("Quarter")
        .ylabel("Revenue")
        .line(
            AxisData::Numbers(vec![1.0, 2.0, 3.0, 4.0]),
            &[3.0, 4.5, 4.0, 6.5],
            LineOptions {
                label: "North".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    figure
}

/// The figure of every scene feature at once: a 2 × 2 grid with a gradient area under a line,
/// translucent markers, a heatmap drawn as an image with its colorbar, and bars, titled in
/// Latin and CJK text.
fn all_features_figure() -> Figure {
    let theme = Theme::pigment()
        .with_fill_style(FillStyle::Gradient)
        .unwrap();
    let grid = Grid::new(2, 2).unwrap();
    let (width, height) = grid.auto_size_in();
    let mut figure = Figure::with_grid(width, height, theme, grid).unwrap();
    let at = |row: i64, col: i64| Placement {
        row: GridIndex::At(row),
        col: GridIndex::At(col),
        row_span: 1,
        col_span: 1,
    };
    let x: Vec<f64> = (0..40).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|v| (v / 6.0).sin() * 3.0 + 4.0).collect();
    figure
        .add_axes_at(at(0, 0), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Area under a line")
        .area(AxisData::Numbers(x.clone()), &y, AreaOptions::default())
        .unwrap();
    let translucent = PointColor::Uniform(ColorSpec::Auto(Alpha::new(0.5).unwrap()));
    figure
        .add_axes_at(at(0, 1), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Translucent points")
        .scatter(
            AxisData::Numbers(x.clone()),
            &y.iter().map(|v| 8.0 - v).collect::<Vec<_>>(),
            ScatterOptions {
                color: translucent,
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    let names = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    figure
        .add_axes_at(at(1, 0), ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .title("Cells as an image")
        .heatmap(
            &names(&["Mon", "Tue", "Wed", "Thu", "Fri"]),
            &names(&["Morning", "Noon", "Night"]),
            &[
                4.0, 7.0, 3.0, 8.0, 6.0, 9.0, 2.0, 5.0, 7.0, 1.0, 3.0, 6.0, 8.0, 4.0, 2.0,
            ],
            HeatmapOptions {
                render: Render::Raster,
                ..HeatmapOptions::default()
            },
        )
        .unwrap();
    figure
        .add_axes_at(at(1, 1), ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .title("温度 by region")
        .bar(
            names(&["North", "East", "South"]),
            &[4.0, 7.5, 3.25],
            BarOptions::default(),
        )
        .unwrap();
    figure
}

/// A scene with one text item set in `library` with `fonts`.
fn text_scene(library: &FontLibrary, fonts: &[Font], text: &str) -> Scene {
    let fonts = library.with_figure_fonts(fonts).unwrap();
    let mut scene = Scene::new(200.0, 60.0, WHITE);
    scene
        .push_text(
            &fonts,
            text,
            style(0, 18.0),
            Point { x: 10.0, y: 40.0 },
            INK,
        )
        .unwrap();
    scene
}

// --- Reading the PDF ---------------------------------------------------------------------------

/// The dictionary text of object `number`, up to its `stream` keyword or `endobj`.
fn object_dict(pdf: &[u8], number: u32) -> String {
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
fn stream_data(pdf: &[u8], number: u32) -> Vec<u8> {
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

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// The text after the first `marker` in `s`.
fn after<'a>(s: &'a str, marker: &str) -> &'a str {
    let at = s
        .find(marker)
        .unwrap_or_else(|| panic!("no {marker} in {s}"));
    &s[at + marker.len()..]
}

/// Every object number that `key` refers to (`key N 0 R`), in file order.
fn references(pdf: &[u8], key: &str) -> Vec<u32> {
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
fn have(tool: &str) -> bool {
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
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("umber-pdf-tests")
        .join(format!("{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &FsPath, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// Runs `qpdf --check` and asserts that it finds nothing to warn about.
fn assert_qpdf_clean(path: &FsPath) {
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
fn extracted_text(path: &FsPath) -> String {
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
struct Raster {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
}

/// The page rasterized by `pdftoppm` at `dpi`, as RGB.
fn poppler_raster(path: &FsPath, dpi: u32) -> Raster {
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
fn png_raster(png: &[u8]) -> Raster {
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
fn raster_difference(a: &Raster, b: &Raster) -> (f64, f64) {
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
const MAX_MEAN_DIFFERENCE: f64 = 2.0;
/// The largest share of pixels with a channel more than 64 apart.
const MAX_FAR_SHARE: f64 = 0.01;

fn assert_matches_png(pdf_path: &FsPath, png: &[u8], dpi: u32) {
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

// --- Tests ---------------------------------------------------------------------------------------

#[test]
fn pdf_is_deterministic() {
    let figure = line_figure();
    let first = figure.to_pdf().unwrap();
    let second = figure.to_pdf().unwrap();
    assert!(first == second, "two renders of one figure differ");
    assert!(first.starts_with(b"%PDF-1.7\n"));
    assert!(first.ends_with(b"%%EOF") || first.ends_with(b"%%EOF\n"));
    let text = String::from_utf8_lossy(&first);
    assert!(!text.contains("/CreationDate") && !text.contains("/ModDate"));
    assert!(
        text.contains(&format!("/Producer (umber {})", umber_core::VERSION)),
        "no producer"
    );
    assert!(text.contains("/ID ["), "no document ID");
    // A different figure has a different document ID.
    let mut other = line_figure();
    other
        .axes_at_mut(GridIndex::Auto, GridIndex::Auto)
        .unwrap()
        .title("Another title");
    let other = other.to_pdf().unwrap();
    let id = |pdf: &[u8]| after(&String::from_utf8_lossy(pdf), "/ID [").to_owned();
    assert_ne!(id(&first), id(&other));
}

#[test]
fn pdf_page_size_matches_figure() {
    for ((width, height), media_box) in [
        ((6.0, 4.0), "/MediaBox [0 0 432 288]"),
        ((3.5, 2.25), "/MediaBox [0 0 252 162]"),
        ((200.0, 1.0), "/MediaBox [0 0 14400 72]"),
    ] {
        let mut figure = Figure::new(width, height, Theme::graphite()).unwrap();
        figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        let pdf = figure.to_pdf().unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(
            text.contains(media_box),
            "{width} x {height}: no {media_box}"
        );
        assert_eq!(
            text.matches("/Type /Page\n").count() + text.matches("/Type /Page ").count(),
            1
        );
    }
}

#[test]
fn pdf_rejects_pages_it_cannot_write() {
    for (width, height) in [
        (f32::NAN, 10.0),
        (10.0, 0.0),
        (14_401.0, 10.0),
        (10.0, -1.0),
    ] {
        let err = render_pdf(&Scene::new(width, height, WHITE)).unwrap_err();
        assert!(matches!(err, umber_core::Error::Style { .. }), "{err}");
        assert!(
            err.to_string().starts_with("Figure.to_pdf(): the page is "),
            "{err}"
        );
        assert_eq!(
            err.hint(),
            Some("use a width and height above 0 and at most 200 in.")
        );
    }
    assert!(render_pdf(&Scene::new(14_400.0, 0.5, WHITE)).is_ok());
}

#[test]
fn figure_errors_name_to_pdf() {
    let mut figure = Figure::new(1.0, 1.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xticks_labeled(&[0.0, 1.0], vec!["0".into(), "W".repeat(60)])
        .unwrap();
    let err = figure.to_pdf().unwrap_err();
    assert!(err.to_string().starts_with("Figure.to_pdf(): "), "{err}");
}

#[test]
fn pdf_text_extractable() {
    let dir = scratch("text");
    let path = write(&dir, "line.pdf", &line_figure().to_pdf().unwrap());
    if !have("pdftotext") {
        return;
    }
    let text = extracted_text(&path);
    for expected in ["Quarterly revenue", "Quarter", "Revenue", "North"] {
        assert!(text.contains(expected), "no {expected:?} in {text:?}");
    }
}

#[test]
fn pdf_embeds_only_used_glyphs() {
    let library = FontLibrary::with_fallbacks(Vec::new());
    let pdf = render_pdf(&text_scene(&library, &[Font::INTER], "abcab")).unwrap();
    let files = references(&pdf, "/FontFile2");
    assert_eq!(files.len(), 1, "one embedded font");
    let subset = stream_data(&pdf, files[0]);
    let font = FontRef::new(&subset).unwrap();
    // .notdef, a, b and c.
    assert_eq!(font.maxp().unwrap().num_glyphs(), 4);
    assert!(font.cmap().is_err(), "the subset keeps no cmap of its own");
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("/CIDFontType2"), "TrueType outlines");
    assert!(text.contains("/CIDToGIDMap /Identity"));
    assert!(text.contains("/Encoding /Identity-H"));
    // The base font is the PostScript name with a six-letter subset tag.
    let base = after(&text, "/BaseFont /");
    let (tag, name) = base.split_once('+').unwrap();
    assert!(
        tag.len() == 6 && tag.bytes().all(|b| b.is_ascii_uppercase()),
        "{tag}"
    );
    assert!(name.starts_with("Inter"), "{name}");
    // The ToUnicode map names the three letters.
    let to_unicode = references(&pdf, "/ToUnicode");
    assert_eq!(to_unicode.len(), 1);
    let cmap = String::from_utf8(stream_data(&pdf, to_unicode[0])).unwrap();
    for letter in ['a', 'b', 'c'] {
        assert!(
            cmap.contains(&format!("<{:04X}>", u32::from(letter))),
            "{cmap}"
        );
    }
    // Fewer distinct glyphs give a smaller subset.
    let fewer = render_pdf(&text_scene(&library, &[Font::INTER], "aaa")).unwrap();
    let subset = stream_data(&fewer, references(&fewer, "/FontFile2")[0]);
    assert_eq!(
        FontRef::new(&subset).unwrap().maxp().unwrap().num_glyphs(),
        2
    );
}

#[test]
fn pdf_embeds_cff_and_variable_fonts() {
    let library = FontLibrary::with_fallbacks(vec![
        fixture("outline-cff.otf"),
        fixture("outline-variable.ttf"),
    ]);
    // Inter, then two glyphs of the CFF font and two of the variable font.
    let text = "Ab \u{E001}\u{E002} \u{E004}\u{E006}";
    let scene = text_scene(&library, &[Font::INTER], text);
    let pdf = render_pdf(&scene).unwrap();
    let body = String::from_utf8_lossy(&pdf);
    assert_eq!(
        body.matches("/Type /Font\n").count() + body.matches("/Type /Font ").count(),
        6
    );
    assert!(body.contains("/CIDFontType0\n") || body.contains("/CIDFontType0 "));
    assert!(
        body.contains("/Subtype /CIDFontType0C"),
        "the CFF table is embedded bare"
    );

    // The CFF font program: a bare CFF table holding .notdef and the two glyphs.
    let cff = references(&pdf, "/FontFile3");
    assert_eq!(cff.len(), 1);
    let program = stream_data(&pdf, cff[0]);
    assert_eq!(program[0], 1, "CFF major version 1");

    // The variable font is embedded at its default instance, without its variation tables.
    let truetype = references(&pdf, "/FontFile2");
    assert_eq!(truetype.len(), 2, "Inter and the variable font");
    let variable = truetype
        .iter()
        .map(|&n| stream_data(&pdf, n))
        .find(|data| FontRef::new(data).unwrap().maxp().unwrap().num_glyphs() == 3)
        .expect("a subset of .notdef and two glyphs");
    let font = FontRef::new(&variable).unwrap();
    assert!(font.fvar().is_err() && font.gvar().is_err());

    let dir = scratch("cff-variable");
    let path = write(&dir, "fonts.pdf", &pdf);
    if have("pdffonts") {
        let out = Command::new("pdffonts").arg(&path).output().unwrap();
        let listing = String::from_utf8_lossy(&out.stdout);
        let rows: Vec<&str> = listing.lines().skip(2).collect();
        assert_eq!(rows.len(), 3, "{listing}");
        for row in &rows {
            // Every font is embedded, subset and has a ToUnicode map.
            assert!(row.contains("yes yes yes"), "{listing}");
        }
        assert!(listing.contains("CID Type 0C"), "{listing}");
        assert!(listing.contains("CID TrueType"), "{listing}");
    }
    if have("qpdf") {
        assert_qpdf_clean(&path);
    }
    if have("pdftotext") {
        let extracted = extracted_text(&path);
        for c in ['\u{E001}', '\u{E002}', '\u{E004}', '\u{E006}'] {
            assert!(
                extracted.contains(c),
                "no U+{:04X} in {extracted:?}",
                u32::from(c)
            );
        }
    }
    if have("pdftoppm") {
        assert_matches_png(&path, &render_png(&scene, 144).unwrap(), 144);
    }
}

#[test]
fn pdf_all_features_grid() {
    let figure = all_features_figure();
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    let pdf = figure.to_pdf().unwrap();
    assert_eq!(pdf, render_pdf(&scene).unwrap());
    let dir = scratch("all-features");
    let path = write(&dir, "grid.pdf", &pdf);
    let body = String::from_utf8_lossy(&pdf);
    for (feature, needle) in [
        ("an axial shading", "/ShadingType 2"),
        ("a soft mask for the fading area", "/SMask"),
        ("translucent paint", "/ca 0."),
        ("an image of the cells", "/Subtype /Image"),
        ("a form for the markers", "/Subtype /Form"),
    ] {
        assert!(
            body.contains(needle),
            "no {feature} ({needle}) in {}",
            path.display()
        );
    }
    if have("qpdf") {
        assert_qpdf_clean(&path);
    }
    if have("pdftotext") {
        let text = extracted_text(&path);
        for title in [
            "Area under a line",
            "Translucent points",
            "Cells as an image",
            "by region",
        ] {
            assert!(text.contains(title), "no {title:?} in {text:?}");
        }
    }
    if have("pdftoppm") {
        assert_matches_png(&path, &figure.to_png(144).unwrap(), 144);
    }
}

#[test]
fn raster_cells_not_interpolated() {
    let mut figure = Figure::new(4.0, 3.0, Theme::graphite()).unwrap();
    let names = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    figure
        .add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &names(&["a", "b", "c"]),
            &names(&["x", "y"]),
            &[1.0, 2.0, 3.0, 4.0, f64::NAN, 6.0],
            HeatmapOptions {
                render: Render::Raster,
                ..HeatmapOptions::default()
            },
        )
        .unwrap();
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    let image_count = {
        fn count(items: &[Item]) -> usize {
            items
                .iter()
                .map(|item| match item {
                    Item::Image { .. } => 1,
                    Item::Group { items, .. } => count(items),
                    _ => 0,
                })
                .sum()
        }
        count(&scene.items)
    };
    assert_eq!(image_count, 1);
    let pdf = render_pdf(&scene).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    let image = after(&text, "/Subtype /Image");
    let dict = &image[..image.find(">>").unwrap()];
    for entry in [
        "/Width 3",
        "/Height 2",
        "/Interpolate false",
        "/BitsPerComponent 8",
    ] {
        assert!(dict.contains(entry), "no {entry} in {dict}");
    }
    assert!(!text.contains("/Interpolate true"));
    // The missing cell is transparent, so the image has a soft mask.
    assert!(dict.contains("/SMask"), "{dict}");
}
