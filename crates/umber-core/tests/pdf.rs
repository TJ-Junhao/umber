//! Tests of the PDF backend: determinism, page size, embedded fonts and text, raster cells, and
//! agreement with the PNG of the same scene.
//!
//! Some tests run PDF readers; see `support/pdf_check.rs` for when they are skipped.

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

#[path = "support/pdf_check.rs"]
mod pdf_check;

use pdf_check::*;

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
    // Each glyph and the text it stands for has a character code, mapped to its glyph.
    let map = references(&pdf, "/CIDToGIDMap");
    assert_eq!(map.len(), 1);
    assert_eq!(stream_data(&pdf, map[0]), [0, 0, 0, 1, 0, 2, 0, 3]);
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

/// `pdftotext`'s text of a one-line page, without the line end and page break it adds.
fn one_line(path: &FsPath) -> String {
    extracted_text(path)
        .trim_end_matches(['\n', '\u{c}'])
        .to_owned()
}

/// MuPDF's text of a one-line page, without the line ends it adds.
fn mupdf_line(path: &FsPath) -> String {
    let out = Command::new("mutool")
        .args(["draw", "-q", "-F", "txt", "-o", "-"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .trim_end_matches(['\n', '\u{c}'])
        .to_owned()
}

#[test]
fn pdf_text_extracts_each_occurrence() {
    let without_system = FontLibrary::with_fallbacks(Vec::new());
    let cff = FontLibrary::with_fallbacks(vec![fixture("outline-cff.otf")]);
    let dir = scratch("occurrences");
    // In TrueType fonts every glyph standing for each text has a code of its own, so the
    // `ToUnicode` map alone reads every occurrence, in pdf.js too, and nothing is marked. A CFF
    // font has one code per glyph, so an occurrence its map misreads is marked with
    // `/ActualText`, which pdf.js does not read.
    for (name, library, font, text, marked, pdfjs) in [
        // A combining cluster on its own.
        (
            "standalone",
            FontLibrary::global(),
            Font::SOURCE_SERIF_4,
            "x\u{301}",
            false,
            true,
        ),
        // A base glyph alone and inside different combining clusters.
        (
            "combining",
            FontLibrary::global(),
            Font::SOURCE_SERIF_4,
            "x x\u{301} x\u{302} x\u{301}\u{302}",
            false,
            true,
        ),
        // .notdef standing for different characters.
        (
            "notdef",
            &without_system,
            Font::INTER,
            "Missing 日本",
            false,
            true,
        ),
        // Ligatures, which stand for the same characters wherever they occur.
        (
            "ligatures",
            FontLibrary::global(),
            Font::INTER,
            "office ffi fl ffi",
            false,
            true,
        ),
        // The CFF font's .notdef standing for different marks.
        (
            "cff-marks",
            &cff,
            Font::INTER,
            "\u{E001}\u{301} \u{E002}\u{302}",
            true,
            false,
        ),
    ] {
        let pdf = render_pdf(&text_scene(library, &[font], text)).unwrap();
        let content = page_content(&pdf);
        assert_eq!(content.contains("/ActualText"), marked, "{name}: {content}");
        let path = write(&dir, &format!("{name}.pdf"), &pdf);
        assert_readers_parse(&path);
        if have("qpdf") {
            assert_qpdf_clean(&path);
        }
        if have("pdftotext") {
            assert_eq!(one_line(&path), text, "{name}");
        }
        if have("mutool") {
            assert_eq!(mupdf_line(&path), text, "{name} (MuPDF)");
        }
        if pdfjs && let Some(extracted) = pdfjs_text(&path) {
            assert_eq!(extracted, text, "{name} (pdf.js)");
        }
    }
}

#[test]
fn pdf_writes_tiny_text_validly() {
    let dir = scratch("tiny");
    let library = FontLibrary::with_fallbacks(Vec::new());
    let fonts = library.with_figure_fonts(&[Font::INTER]).unwrap();
    for size in [0.0004_f32, 1.0e-30, 0.0015, 0.002, 0.25] {
        let mut scene = Scene::new(60.0, 30.0, WHITE);
        scene
            .push_text(
                &fonts,
                "Tiny",
                style(0, size),
                Point { x: 5.0, y: 20.0 },
                INK,
            )
            .unwrap();
        let pdf = render_pdf(&scene).unwrap();
        let content = page_content(&pdf);
        assert!(
            !content.contains("NaN") && !content.contains("inf"),
            "{size}: {content}"
        );
        if size >= 0.001 {
            // Written as text, at its exact size.
            assert!(
                content.contains(&format!("/F0 {size} Tf")),
                "{size}: {content}"
            );
        } else {
            // Too small to write, and too small for PNG to draw.
            assert!(!content.contains("BT"), "{size}: {content}");
            let empty = Scene::new(60.0, 30.0, WHITE);
            assert_eq!(
                render_png(&scene, 72).unwrap(),
                render_png(&empty, 72).unwrap(),
                "{size}"
            );
        }
        assert_readers_parse(&write(&dir, &format!("size-{size}.pdf"), &pdf));
    }
    let mut figure =
        Figure::new(3.0, 2.0, Theme::pigment().with_font_scale(1e-30).unwrap()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Tiny");
    assert_readers_parse(&write(&dir, "font-scale.pdf", &figure.to_pdf().unwrap()));
}

#[test]
fn pdf_keeps_fonts_of_different_libraries_apart() {
    // Each library has its fallback font at the same index.
    let cff = FontLibrary::with_fallbacks(vec![fixture("outline-cff.otf")]);
    let variable = FontLibrary::with_fallbacks(vec![fixture("outline-variable.ttf")]);
    let mut scene = Scene::new(200.0, 70.0, WHITE);
    for (library, text, y) in [
        (&cff, "\u{E001}\u{E003}", 25.0),
        (&variable, "\u{E004}\u{E006}", 55.0),
    ] {
        let fonts = library.with_figure_fonts(&[Font::INTER]).unwrap();
        scene
            .push_text(&fonts, text, style(0, 18.0), Point { x: 10.0, y }, INK)
            .unwrap();
    }
    let pdf = render_pdf(&scene).unwrap();
    assert_eq!(references(&pdf, "/FontFile3").len(), 1, "the CFF font");
    assert_eq!(references(&pdf, "/FontFile2").len(), 1, "the variable font");
    let dir = scratch("libraries");
    let path = write(&dir, "two-libraries.pdf", &pdf);
    assert_readers_parse(&path);
    if have("pdftotext") {
        let text = extracted_text(&path);
        for c in ['\u{E001}', '\u{E003}', '\u{E004}', '\u{E006}'] {
            assert!(text.contains(c), "no U+{:04X} in {text:?}", u32::from(c));
        }
    }
    if have("pdftoppm") {
        assert_matches_png(&path, &render_png(&scene, 144).unwrap(), 144);
    }
}

#[test]
fn pdf_embeds_cff2_as_text() {
    let library = FontLibrary::with_fallbacks(vec![fixture("outline-cff2.otf")]);
    let text = "ABC \u{E007}\u{E009}";
    let scene = text_scene(&library, &[Font::INTER], text);
    let pdf = render_pdf(&scene).unwrap();
    let content = page_content(&pdf);
    // Both runs are text, so nothing is filled but the background.
    assert_eq!(
        content.lines().filter(|line| line.trim() == "f").count(),
        1,
        "{content}"
    );
    let dir = scratch("cff2");
    let path = write(&dir, "cff2.pdf", &pdf);
    assert_readers_parse(&path);
    if have("qpdf") {
        assert_qpdf_clean(&path);
    }
    if have("pdffonts") {
        let out = Command::new("pdffonts").arg(&path).output().unwrap();
        let listing = String::from_utf8_lossy(&out.stdout);
        let rows: Vec<&str> = listing.lines().skip(2).collect();
        assert_eq!(rows.len(), 2, "{listing}");
        assert!(
            rows.iter().all(|row| row.contains("yes yes yes")),
            "{listing}"
        );
    }
    if have("pdftotext") {
        assert_eq!(one_line(&path), text);
    }
    if have("pdftoppm") {
        assert_matches_png(&path, &render_png(&scene, 144).unwrap(), 144);
    }
}
