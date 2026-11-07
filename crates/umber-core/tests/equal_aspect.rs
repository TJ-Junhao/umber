//! Equal aspect through the public API: the scales read off the drawn scene after the fits of
//! value labels, and the same figure in every output format.

#![allow(clippy::unwrap_used, clippy::panic)]

use umber_core::render::{SvgText, render_pdf, render_png, render_svg};
use umber_core::scene::{Item, Scene, TextItem};
use umber_core::{
    AxesMut, AxisData, Figure, FontLibrary, LineOptions, Place, PointLabels, ScaleKind,
    ScatterOptions, Theme,
};

fn render(figure: &Figure) -> Scene {
    figure.to_scene(FontLibrary::global()).unwrap()
}

fn leaves(items: &[Item]) -> Vec<&Item> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Item::Group { items, .. } => out.extend(leaves(items)),
            other => out.push(other),
        }
    }
    out
}

fn text<'a>(scene: &'a Scene, content: &str) -> &'a TextItem {
    leaves(&scene.items)
        .into_iter()
        .find_map(|item| match item {
            Item::Text(t) if t.text == content => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text {content:?}"))
}

/// Tick labels `x0`, `x1` at x 0 and 1 and `y0`, `y1` at y 0 and 1, to measure a data unit by.
fn unit_ticks(axes: &mut AxesMut<'_>) {
    axes.xticks_labeled(&[0.0, 1.0], vec!["x0".into(), "x1".into()])
        .unwrap()
        .yticks_labeled(&[0.0, 1.0], vec!["y0".into(), "y1".into()])
        .unwrap();
}

/// The length of a data unit on x and on y, read off the tick labels of [`unit_ticks`].
fn unit_lengths(scene: &Scene) -> (f64, f64) {
    let centre = |label: &TextItem| f64::from(label.origin.x) + f64::from(label.width) / 2.0;
    let baseline = |label: &TextItem| f64::from(label.origin.y);
    (
        centre(text(scene, "x1")) - centre(text(scene, "x0")),
        baseline(text(scene, "y0")) - baseline(text(scene, "y1")),
    )
}

#[track_caller]
fn assert_equal_lengths(scene: &Scene) {
    let (x, y) = unit_lengths(scene);
    assert!((x / y - 1.0).abs() < 1e-4, "{x} pt on x, {y} pt on y");
}

/// Points along the top edge of the data, each with its value, where `annotate` asks, in a
/// figure of the given size. Their labels leave the plot above, unless it grows for them.
fn labelled_points(width: f64, height: f64, place: Place, annotate: bool, equal: bool) -> Figure {
    let mut f = Figure::new(width, height, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    axes.scatter(
        AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0, 4.0]),
        &[0.0, 1.0, 2.0, 2.0, 2.0],
        ScatterOptions {
            labels: PointLabels {
                annotate,
                place: if annotate { place } else { Place::Auto },
                ..PointLabels::default()
            },
            ..ScatterOptions::default()
        },
    )
    .unwrap();
    unit_ticks(&mut axes);
    if equal {
        axes.equal_aspect().unwrap();
    }
    f
}

#[test]
fn scatter_labels_that_change_the_limits_keep_the_scales_equal() {
    for (width, height) in [(8.0, 2.5), (2.5, 8.0), (5.0, 5.0)] {
        for place in [Place::Auto, Place::Above, Place::Right] {
            let without = render(&labelled_points(width, height, place, false, true));
            let scene = render(&labelled_points(width, height, place, true, true));
            assert_equal_lengths(&without);
            assert_equal_lengths(&scene);
            let again = render(&labelled_points(width, height, place, true, true));
            assert_eq!(unit_lengths(&again), unit_lengths(&scene));
        }
    }
}

#[test]
fn fixed_labels_widen_the_limits_they_need() {
    // The labels above the top row take room, so a data unit is shorter than without them.
    let without = render(&labelled_points(8.0, 2.5, Place::Above, false, true));
    let scene = render(&labelled_points(8.0, 2.5, Place::Above, true, true));
    assert!(unit_lengths(&scene).1 < unit_lengths(&without).1);
}

fn circle(equal: bool) -> Figure {
    let mut f = Figure::new(8.0, 3.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    let (x, y): (Vec<f64>, Vec<f64>) = (0..=60)
        .map(|i| {
            let angle = std::f64::consts::TAU * f64::from(i) / 60.0;
            (angle.cos(), angle.sin())
        })
        .unzip();
    axes.line(AxisData::Numbers(x), &y, LineOptions::default())
        .unwrap();
    axes.title("Domain").xlabel("x (m)").ylabel("y (m)");
    if equal {
        axes.equal_aspect().unwrap();
    }
    f
}

#[test]
fn every_format_draws_the_same_scene() {
    let figure = circle(true);
    let scene = render(&figure);
    let plain = circle(false);
    let editable = render_svg(&scene, SvgText::Editable);
    assert_eq!(figure.to_svg(SvgText::Editable).unwrap(), editable);
    assert_ne!(plain.to_svg(SvgText::Editable).unwrap(), editable);
    let outlines = render_svg(&scene, SvgText::Outlines);
    assert_eq!(figure.to_svg(SvgText::Outlines).unwrap(), outlines);
    let png = render_png(&scene, 72).unwrap();
    assert_eq!(figure.to_png(72).unwrap(), png);
    assert_ne!(plain.to_png(72).unwrap(), png);
    let pdf = render_pdf(&scene).unwrap();
    assert_eq!(figure.to_pdf().unwrap(), pdf);
    assert_ne!(plain.to_pdf().unwrap(), pdf);
}
