//! Level labels of filled contours on grids and meshes with holes, colored for the bands they
//! lie across and kept inside the field, drawn alike in PNG, both SVG text modes and PDF.

#![allow(clippy::unwrap_used, clippy::panic)]

use umber_core::scene::{Item, Scene, TextItem};
use umber_core::{
    ContourLevels, ContourOptions, ContourStyle, Figure, FontLibrary, Rgba, ScaleKind, SvgText,
    Theme, Triangles, delaunay,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;

/// A bump on a 41 × 31 grid with a NaN hole on its slope, filled with lines and labels.
fn grid(theme: Theme) -> Figure {
    let x: Vec<f64> = (0..41).map(|i| f64::from(i) / 10.0 - 2.0).collect();
    let y: Vec<f64> = (0..31).map(|j| f64::from(j) / 10.0 - 1.5).collect();
    let z: Vec<f64> = y
        .iter()
        .flat_map(|&y| {
            x.iter().map(move |&x| {
                if (x - 0.5).hypot(y - 0.2) < 0.25 {
                    f64::NAN
                } else {
                    (-(x * x) / 1.5 - y * y).exp()
                }
            })
        })
        .collect();
    let mut f = Figure::new(5.0, 3.5, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .contour(&x, &y, &z, labelled())
        .unwrap();
    f
}

/// An annulus of scattered points around a hole, its triangles across the hole left out,
/// filled with lines and labels of a field rising to one side.
fn annulus(theme: Theme) -> Figure {
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for ring in 0..9 {
        let radius = 1.0 + f64::from(ring) * 0.25;
        let count = 24 + ring * 6;
        for k in 0..count {
            let angle = std::f64::consts::TAU * f64::from(k) / f64::from(count);
            x.push(radius * angle.cos());
            y.push(radius * angle.sin());
        }
    }
    let triangles: Vec<[i64; 3]> = delaunay(&x, &y)
        .unwrap()
        .into_iter()
        .filter(|corners| {
            let (cx, cy) = corners.iter().fold((0.0, 0.0), |(cx, cy), &k| {
                (cx + x[k as usize] / 3.0, cy + y[k as usize] / 3.0)
            });
            cx.hypot(cy) > 1.0
        })
        .collect();
    let z: Vec<f64> = x
        .iter()
        .zip(&y)
        .map(|(&x, &y)| ((x + 0.6 * y) / 3.0 + 0.5).clamp(0.0, 1.0))
        .collect();
    let mut f = Figure::new(5.0, 3.5, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .tricontour(&x, &y, Triangles::Given(&triangles), &z, labelled())
        .unwrap()
        .equal_aspect()
        .unwrap();
    f
}

/// Bands and lines at nine levels from 0.1 to 0.9, with labels.
fn labelled() -> ContourOptions {
    ContourOptions {
        levels: ContourLevels::Values((1..10).map(|k| f64::from(k) / 10.0).collect()),
        style: ContourStyle::Both,
        annotate: true,
        ..ContourOptions::default()
    }
}

/// The level labels of `scene`.
fn labels(scene: &Scene) -> Vec<&TextItem> {
    let mut found = Vec::new();
    let mut pending: Vec<&Item> = scene.items.iter().collect();
    while let Some(item) = pending.pop() {
        match item {
            Item::Group { items, .. } => pending.extend(items),
            Item::RotatedText { text, .. } => found.push(text),
            _ => {}
        }
    }
    found
}

/// The color `c` as SVG writes a fill.
fn fill(c: Rgba) -> String {
    format!("fill=\"#{:02X}{:02X}{:02X}\"", c.r, c.g, c.b)
}

/// Whether the PDF page `content` sets the fill color `c`, its channels kept to three decimal
/// places.
fn sets_fill(content: &str, c: Rgba) -> bool {
    let rgb = [c.r, c.g, c.b].map(|v| (f32::from(v) / 255.0 * 1000.0).round() / 1000.0);
    content
        .lines()
        .filter(|line| line.ends_with(" rg"))
        .any(|line| {
            let values: Vec<f32> = line
                .split_whitespace()
                .take(3)
                .filter_map(|v| v.parse().ok())
                .collect();
            values.len() == 3 && values.iter().zip(rgb).all(|(a, b)| (a - b).abs() < 1e-4)
        })
}

#[test]
fn level_labels_agree_in_png_svg_and_pdf() {
    let poppler = pdf_check::have("pdftoppm");
    if !poppler {
        eprintln!("skipped the PDF raster comparison: pdftoppm is not installed");
    }
    for (name, theme) in [
        ("pigment", Theme::pigment()),
        ("midnight", Theme::midnight()),
    ] {
        for (kind, f) in [
            ("grid", grid(theme.clone())),
            ("mesh", annulus(theme.clone())),
        ] {
            let scene = f.to_scene(FontLibrary::global()).unwrap();
            let found = labels(&scene);
            assert!(found.len() >= 4, "{name} {kind}: {}", found.len());
            let colors: Vec<Rgba> = found.iter().map(|text| text.color).collect();
            let (text, background) = (theme.text_color(), theme.background());
            assert!(
                colors.iter().all(|&c| c == text || c == background),
                "{name} {kind}: {colors:?}"
            );
            assert!(colors.contains(&text), "{name} {kind}: {colors:?}");
            assert!(colors.contains(&background), "{name} {kind}: {colors:?}");
            for mode in [SvgText::Editable, SvgText::Outlines] {
                let svg = f.to_svg(mode).unwrap();
                assert_eq!(svg, umber_core::render::render_svg(&scene, mode));
                for &color in &[text, background] {
                    assert!(svg.contains(&fill(color)), "{name} {kind} {mode:?}");
                }
            }
            // As editable text, each label is a text element in its own color.
            let svg = f.to_svg(SvgText::Editable).unwrap();
            for label in &found {
                let element = svg
                    .lines()
                    .filter(|line| line.starts_with("<text") && line.contains(&fill(label.color)))
                    .filter(|line| line.ends_with(&format!(">{}</text>", label.text)))
                    .count();
                let same = found
                    .iter()
                    .filter(|other| other.text == label.text && other.color == label.color)
                    .count();
                assert_eq!(element, same, "{name} {kind}: {}", label.text);
            }
            let png = f.to_png(144).unwrap();
            assert_eq!(png, umber_core::render::render_png(&scene, 144).unwrap());
            let pdf = f.to_pdf().unwrap();
            assert_eq!(pdf, umber_core::render::render_pdf(&scene).unwrap());
            let content = pdf_check::page_content(&pdf);
            for &color in &[text, background] {
                assert!(sets_fill(&content, color), "{name} {kind}: {color:?}");
            }
            if poppler {
                let dir = pdf_check::scratch(&format!("contour-labels-{name}-{kind}"));
                let path = pdf_check::write(&dir, "labels.pdf", &pdf);
                pdf_check::assert_matches_png(&path, &png, 144);
            }
        }
    }
}
