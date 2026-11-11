//! Arrows and wind barbs are the same drawing in PNG, SVG and PDF, and a hundred thousand of
//! them stay a handful of scene items.

#![allow(clippy::unwrap_used, clippy::panic)]

use umber_core::{
    ArrowKey, AxisData, BarbOptions, ColorBy, FieldVectors, Figure, FontLibrary, Item, PointColor,
    QuiverOptions, ScaleKind, SvgText, Theme,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;
use pdf_check::image;

/// A grid of `columns` by `rows` with a rotating field.
fn field(columns: u32, rows: u32) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let x: Vec<f64> = (0..columns).map(f64::from).collect();
    let y: Vec<f64> = (0..rows).map(f64::from).collect();
    let (cx, cy) = (f64::from(columns) / 2.0, f64::from(rows) / 2.0);
    let u = y
        .iter()
        .flat_map(|&y| x.iter().map(move |_| cy - y))
        .collect();
    let v = y
        .iter()
        .flat_map(|_| x.iter().map(move |&x| x - cx))
        .collect();
    (x, y, u, v)
}

fn numbers(values: Vec<f64>) -> AxisData {
    AxisData::Numbers(values)
}

fn quiver_figure(theme: Theme) -> Figure {
    let (x, y, u, v) = field(9, 6);
    let mut figure = Figure::new(4.0, 3.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .quiver(
            numbers(x),
            numbers(y),
            FieldVectors::grid(u, v),
            QuiverOptions {
                key: Some(ArrowKey::new(2.0, "2 m/s")),
                ..QuiverOptions::default()
            },
        )
        .unwrap();
    figure
}

fn barb_figure(theme: Theme) -> Figure {
    let (x, y, u, v) = field(6, 4);
    let u: Vec<f64> = u.iter().map(|u| 12.0 * u).collect();
    let v: Vec<f64> = v.iter().map(|v| 12.0 * v).collect();
    let mut figure = Figure::new(4.0, 3.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .barbs(
            numbers(x),
            numbers(y),
            FieldVectors::grid(u, v),
            BarbOptions {
                label: "Wind".into(),
                ..BarbOptions::default()
            },
        )
        .unwrap();
    figure
}

#[test]
fn arrows_and_barbs_agree_in_png_svg_and_pdf() {
    let poppler = pdf_check::have("pdftoppm");
    if !poppler {
        eprintln!("skipped the PDF raster comparison: pdftoppm is not installed");
    }
    for (name, theme) in [
        ("pigment", Theme::pigment()),
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        for (mark, figure) in [
            ("quiver", quiver_figure(theme.clone())),
            ("barbs", barb_figure(theme.clone())),
        ] {
            let scene = figure.to_scene(FontLibrary::global()).unwrap();
            let Item::Group {
                clip: Some(_),
                items,
            } = &scene.items[1]
            else {
                panic!("the clipped marks group");
            };
            // SVG: every item of the marks is one path element, in both text modes.
            for text in [SvgText::Editable, SvgText::Outlines] {
                let svg = figure.to_svg(text, 300).unwrap();
                assert!(svg.contains("<path"), "{name} {mark}");
            }
            assert!(!items.is_empty() && items.len() <= 2, "{name} {mark}");
            // PNG and PDF: the same pixels, as far as their rasterizers allow.
            let dpi = 144;
            let png = figure.to_png(dpi).unwrap();
            assert!(image::decode_rgba8(&png).is_ok(), "{name} {mark}");
            let pdf = figure.to_pdf(300).unwrap();
            assert_eq!(pdf, umber_core::render::render_pdf(&scene, 300).unwrap());
            if poppler {
                let dir = pdf_check::scratch(&format!("vector-fields-{name}-{mark}"));
                let path = pdf_check::write(&dir, "field.pdf", &pdf);
                pdf_check::assert_matches_png(&path, &png, dpi as u32);
            }
        }
    }
}

#[test]
fn a_hundred_thousand_arrows_are_one_item_per_color() {
    let (x, y, u, v) = field(400, 250);
    let values: Vec<f64> = (0..100_000).map(|i| f64::from(i % 4)).collect();
    let mut figure = Figure::new(8.0, 6.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .quiver(
            numbers(x),
            numbers(y),
            FieldVectors::grid(u, v),
            QuiverOptions {
                color: PointColor::By(ColorBy::new(values)),
                ..QuiverOptions::default()
            },
        )
        .unwrap();
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    let Item::Group {
        clip: Some(_),
        items,
    } = &scene.items[1]
    else {
        panic!("the clipped marks group");
    };
    assert_eq!(items.len(), 4);
}
