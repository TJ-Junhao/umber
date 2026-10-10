//! Area fills under lines: geometry, gradients and fills on both sides of the baseline.

use super::{Pixels, line_figure, mark_group, plot_of, subpaths};
use crate::model::{Figure, LineOptions};
use crate::resolve::NormalizedPos;
use crate::scene::{FillRule, Item, LinearGradient, Paint};
use crate::test_support::scene::assert_scene_bounded;
use crate::{AreaFill, FontLibrary, GridLines, Theme};

#[test]
fn area_fill_sits_under_line() {
    let theme = Theme::midnight();
    let f = line_figure(
        theme.clone(),
        &[0.0, 1.0, 2.0, 3.0],
        &[10.0, 40.0, 25.0, 30.0],
        LineOptions::default(),
    );
    let scene = f.to_scene(FontLibrary::global());
    assert_scene_bounded(&scene);
    let (_, items) = mark_group(&scene);
    let [
        Item::Fill {
            path: area,
            paint: Paint::Linear(gradient),
            rule: FillRule::NonZero,
        },
        Item::Stroke { path: line, style },
    ] = items
    else {
        panic!("expected an area fill under the line, got {items:?}")
    };
    let color = theme.palette()[0];
    assert_eq!(style.color, color);
    let (line_min, _) = line.bounds().unwrap();
    let (area_min, area_max) = area.bounds().unwrap();
    let (_, layout) = crate::pipeline::plan_figure(&f, FontLibrary::global());
    let baseline = layout.baseline_y as f32;
    assert_eq!(area_min.y, line_min.y);
    assert_eq!(area_max.y, baseline);
    // The area's outline is the line, then down to the baseline and back.
    let area_points = &subpaths(area)[0];
    let line_points = &subpaths(line)[0];
    assert_eq!(&area_points[..line_points.len()], line_points.as_slice());
    assert_eq!(area_points.len(), line_points.len() + 2);
    assert!(
        area_points[line_points.len()..]
            .iter()
            .all(|p| p.y == baseline)
    );
    // The gradient runs from the line's top at 0.32 alpha to the baseline at 0.
    assert_eq!(gradient.start.y, line_min.y);
    assert_eq!(gradient.end.y, baseline);
    assert_eq!(gradient.start.x, gradient.end.x);
    let alphas: Vec<(f32, u8)> = gradient
        .stops
        .iter()
        .map(|s| (s.offset, s.color.a))
        .collect();
    assert_eq!(alphas, [(0.0, 82), (1.0, 0)]);
    assert!(
        gradient
            .stops
            .iter()
            .all(|s| (s.color.r, s.color.g, s.color.b) == (color.r, color.g, color.b))
    );

    // Without `AreaFill::Gradient`, there is no fill.
    let f = line_figure(
        Theme::graphite(),
        &[0.0, 1.0],
        &[1.0, 2.0],
        LineOptions::default(),
    );
    let scene = f.to_scene(FontLibrary::global());
    assert!(matches!(mark_group(&scene).1, [Item::Stroke { .. }]));
}

/// The PNG pixel at data position `(x, y)` of a figure with limits 0..1 by -2..2.
fn data_pixel(f: &Figure, png: &[u8], x: f64, y: f64) -> [u8; 3] {
    let plot = plot_of(f);
    Pixels::decode(png).at_point(
        (
            plot.x_at(NormalizedPos::new(x)),
            plot.y_at(NormalizedPos::new((y + 2.0) / 4.0)),
        ),
        300.0,
    )
}

fn area_figure(theme: &Theme, x: &[f64], y: &[f64]) -> Figure {
    let mut f = line_figure(theme.clone(), x, y, LineOptions::default());
    f.axes_mut()
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(-2.0, 2.0)
        .unwrap();
    f
}

/// Whether the area fill shows at each data position: the pixel differs from the same
/// figure drawn without area fill.
fn area_shows(x: &[f64], y: &[f64], probes: &[(f64, f64)]) -> Vec<bool> {
    let theme = Theme::midnight().with_grid_lines(GridLines::None).unwrap();
    let with = area_figure(&theme, x, y);
    let without = area_figure(&theme.with_area_fill(AreaFill::None).unwrap(), x, y);
    let (a, b) = (with.to_png(300).unwrap(), without.to_png(300).unwrap());
    probes
        .iter()
        .map(|&(px, py)| data_pixel(&with, &a, px, py) != data_pixel(&without, &b, px, py))
        .collect()
}

#[test]
fn areas_crossing_the_baseline_fill_both_sides() {
    // One line from +1 down to -1: filled above the baseline on the left, below on the
    // right, and nowhere beyond the line.
    let shows = area_shows(
        &[0.0, 0.5, 1.0],
        &[1.0, 0.0, -1.0],
        &[(0.1, 0.4), (0.9, -0.4), (0.1, -0.4), (0.9, 0.4)],
    );
    assert_eq!(shows, [true, true, false, false]);

    // NaN-separated runs on opposite sides, each fading towards the baseline.
    let shows = area_shows(
        &[0.0, 0.4, f64::NAN, 0.6, 1.0],
        &[1.0, 1.0, f64::NAN, -1.0, -1.0],
        &[(0.2, 0.5), (0.8, -0.5), (0.2, -0.5), (0.8, 0.5)],
    );
    assert_eq!(shows, [true, true, false, false]);

    // Each side is its own fill, with its gradient from its extreme to the baseline.
    let theme = Theme::midnight();
    let f = area_figure(&theme, &[0.0, 0.5, 1.0], &[1.0, 0.0, -1.0]);
    let scene = f.to_scene(FontLibrary::global());
    assert_scene_bounded(&scene);
    let plot = plot_of(&f);
    let baseline = plot.y_at(NormalizedPos::new(0.5)) as f32;
    let gradients: Vec<&LinearGradient> = mark_group(&scene)
        .1
        .iter()
        .filter_map(|item| match item {
            Item::Fill {
                paint: Paint::Linear(g),
                ..
            } => Some(g),
            _ => None,
        })
        .collect();
    let [above, below] = gradients.as_slice() else {
        panic!("expected two area fills, got {gradients:?}")
    };
    assert!((above.start.y - plot.y_at(NormalizedPos::new(0.75)) as f32).abs() < 1e-3);
    assert!((below.start.y - plot.y_at(NormalizedPos::new(0.25)) as f32).abs() < 1e-3);
    for g in [above, below] {
        assert_eq!(g.end.y, baseline);
        assert_eq!((g.stops[0].color.a, g.stops[1].color.a), (82, 0));
    }
}

#[test]
fn opposite_runs_do_not_cancel_their_areas() {
    let shows = area_shows(
        &[0.0, 1.0, f64::NAN, 1.0, 0.0],
        &[1.0, 1.0, f64::NAN, 1.0, 1.0],
        &[(0.5, 0.5)],
    );
    assert_eq!(shows, [true]);
    // A reversed run alone is filled too.
    assert_eq!(area_shows(&[1.0, 0.0], &[1.0, 1.0], &[(0.5, 0.5)]), [true]);
}
