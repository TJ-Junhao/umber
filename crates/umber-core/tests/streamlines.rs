//! Streamlines are the same drawing in PNG, SVG and PDF, and the lines of a 300 by 300 grid at
//! density 3 stay two scene items.

#![allow(clippy::unwrap_used, clippy::panic)]

use umber_core::{
    ColorBy, Figure, FontLibrary, Item, PointColor, ScaleKind, StreamplotOptions, SvgText, Theme,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;
use pdf_check::image;

/// `count` values from `low` to `high`.
fn spread(low: f64, high: f64, count: u32) -> Vec<f64> {
    let step = (high - low) / f64::from(count - 1);
    (0..count).map(|i| low + step * f64::from(i)).collect()
}

/// A field on a grid, with its speed.
struct Field {
    x: Vec<f64>,
    y: Vec<f64>,
    u: Vec<f64>,
    v: Vec<f64>,
    speed: Vec<f64>,
}

/// A swirl on a grid of `count` by `count` points.
fn swirl(count: u32) -> Field {
    let (x, y) = (spread(-3.0, 3.0, count), spread(-2.0, 2.0, count));
    let (mut u, mut v, mut speed) = (Vec::new(), Vec::new(), Vec::new());
    for &y in &y {
        for &x in &x {
            let (a, b) = (-y + 0.4 * x * (1.0 - x * x / 9.0), x - 0.3 * y);
            u.push(a);
            v.push(b);
            speed.push((a * a + b * b).sqrt());
        }
    }
    Field { x, y, u, v, speed }
}

fn figure(theme: Theme, mapped: bool, (count, density): (u32, f64)) -> Figure {
    let Field { x, y, u, v, speed } = swirl(count);
    let mut figure = Figure::new(5.0, 4.0, theme).unwrap();
    let color = if mapped {
        PointColor::By(ColorBy::new(speed))
    } else {
        PointColor::AUTO
    };
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .streamplot(
            &x,
            &y,
            &u,
            &v,
            StreamplotOptions {
                color,
                density,
                ..StreamplotOptions::default()
            },
        )
        .unwrap();
    figure
}

/// The items of the clipped marks group of `figure`.
fn mark_items(figure: &Figure) -> Vec<Item> {
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    let Item::Group {
        clip: Some(_),
        items,
    } = &scene.items[1]
    else {
        panic!("the clipped marks group");
    };
    items.clone()
}

#[test]
fn streamlines_agree_in_png_svg_and_pdf() {
    let poppler = pdf_check::have("pdftoppm");
    if !poppler {
        eprintln!("skipped the PDF raster comparison: pdftoppm is not installed");
    }
    for (name, theme) in [
        ("pigment", Theme::pigment()),
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        for mapped in [false, true] {
            let figure = figure(theme.clone(), mapped, (41, 1.0));
            let scene = figure.to_scene(FontLibrary::global()).unwrap();
            for text in [SvgText::Editable, SvgText::Outlines] {
                let svg = figure.to_svg(text, 300).unwrap();
                assert!(svg.contains("<path"), "{name} {mapped}");
            }
            let dpi = 144;
            let png = figure.to_png(dpi).unwrap();
            assert!(image::decode_rgba8(&png).is_ok(), "{name} {mapped}");
            let pdf = figure.to_pdf(300).unwrap();
            assert_eq!(pdf, umber_core::render::render_pdf(&scene, 300).unwrap());
            if poppler {
                let dir = pdf_check::scratch(&format!("streamlines-{name}-{mapped}"));
                let path = pdf_check::write(&dir, "streamlines.pdf", &pdf);
                pdf_check::assert_matches_png(&path, &png, dpi as u32);
            }
        }
    }
}

#[test]
fn a_dense_grid_in_one_color_is_two_items() {
    let figure = figure(Theme::pigment(), false, (300, 3.0));
    let items = mark_items(&figure);
    assert_eq!(items.len(), 2);
    assert!(matches!(items[0], Item::Stroke { .. }));
    assert!(matches!(items[1], Item::Fill { .. }));
    for text in [SvgText::Editable, SvgText::Outlines] {
        assert!(figure.to_svg(text, 300).is_ok());
    }
    assert!(figure.to_png(72).is_ok());
    assert!(figure.to_pdf(300).is_ok());
}
