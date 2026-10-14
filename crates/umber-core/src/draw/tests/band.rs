//! Band marks: the polygon of each run, NaN gaps, the flat fill in every fill style, clipping
//! at the limits, a log y axis, and downsampling.

use super::super::area::AREA_ALPHA;
use super::super::line::downsample_columns;
use super::{mark_group, nums, plot_of, subpaths};
use crate::geometry::PathVerb;
use crate::model::{BandOptions, Figure, LineOptions};
use crate::scene::{FillRule, Item, Paint};
use crate::test_support::scene::assert_scene_bounded;
use crate::{FillStyle, FontLibrary, LogBase, ScaleKind, Theme};

const LIN: ScaleKind = ScaleKind::Linear;
const NAN: f64 = f64::NAN;

fn band_figure(theme: Theme, y: ScaleKind, (x, low, high): (&[f64], &[f64], &[f64])) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(LIN, y)
        .unwrap()
        .band(x, low, high, BandOptions::default())
        .unwrap();
    f
}

/// The mark items of `figure`.
fn items(figure: &Figure) -> Vec<Item> {
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    match scene.items.as_slice() {
        [_, _] => Vec::new(),
        _ => mark_group(&scene).1.to_vec(),
    }
}

/// The polygons of the one fill of `figure`'s marks, as lists of `(x, y)`, each checked to be
/// closed.
fn polygons(figure: &Figure) -> Vec<Vec<(f64, f64)>> {
    let items = items(figure);
    let [Item::Fill { path, .. }] = items.as_slice() else {
        panic!("expected one fill, got {items:?}")
    };
    let closes = path
        .verbs()
        .iter()
        .filter(|v| matches!(v, PathVerb::Close))
        .count();
    let parts = subpaths(path);
    assert_eq!(closes, parts.len(), "every polygon is closed");
    parts
        .into_iter()
        .map(|part| {
            part.iter()
                .map(|p| (f64::from(p.x), f64::from(p.y)))
                .collect()
        })
        .collect()
}

#[track_caller]
fn assert_points(got: &[(f64, f64)], expected: &[(f64, f64)]) {
    assert_eq!(got.len(), expected.len(), "{got:?} vs {expected:?}");
    for (g, e) in got.iter().zip(expected) {
        assert!(
            (g.0 - e.0).abs() < 1e-3 && (g.1 - e.1).abs() < 1e-3,
            "{got:?} vs {expected:?}"
        );
    }
}

#[test]
fn band_runs_along_the_upper_curve_and_back_along_the_lower() {
    let mut f = band_figure(
        Theme::pigment(),
        LIN,
        (&[0.0, 1.0, 2.0], &[1.0, 2.0, 1.0], &[3.0, 4.0, 3.0]),
    );
    f.single_axes_mut()
        .xlim(0.0, 2.0)
        .unwrap()
        .ylim(0.0, 4.0)
        .unwrap();
    let plot = plot_of(&f);
    let at = |x: f64, y: f64| {
        (
            plot.x + x / 2.0 * plot.width,
            plot.bottom() - y / 4.0 * plot.height,
        )
    };
    let parts = polygons(&f);
    assert_eq!(parts.len(), 1);
    assert_points(
        &parts[0],
        &[
            at(0.0, 3.0),
            at(1.0, 4.0),
            at(2.0, 3.0),
            at(2.0, 1.0),
            at(1.0, 2.0),
            at(0.0, 1.0),
        ],
    );
}

#[test]
fn band_nan_breaks_into_two_polygons() {
    let x = [0.0, 1.0, 2.0, 3.0, 4.0];
    let (low, high) = ([0.0, 1.0, 1.0, 2.0, 1.0], [2.0, 3.0, 3.0, 4.0, 3.0]);
    // A gap in any of the three leaves the same two polygons, each of two points per curve.
    let gapped = [
        (vec![0.0, 1.0, NAN, 3.0, 4.0], low.to_vec(), high.to_vec()),
        (x.to_vec(), vec![0.0, 1.0, NAN, 2.0, 1.0], high.to_vec()),
        (x.to_vec(), low.to_vec(), vec![2.0, 3.0, NAN, 4.0, 3.0]),
    ];
    for (x, low, high) in gapped {
        let mut f = band_figure(Theme::graphite(), LIN, (&x, &low, &high));
        f.single_axes_mut()
            .xlim(0.0, 4.0)
            .unwrap()
            .ylim(0.0, 4.0)
            .unwrap();
        let plot = plot_of(&f);
        let at = |x: f64, y: f64| {
            (
                plot.x + x / 4.0 * plot.width,
                plot.bottom() - y / 4.0 * plot.height,
            )
        };
        let parts = polygons(&f);
        assert_eq!(parts.len(), 2, "{x:?} {low:?} {high:?}");
        assert_points(
            &parts[0],
            &[at(0.0, 2.0), at(1.0, 3.0), at(1.0, 1.0), at(0.0, 0.0)],
        );
        assert_points(
            &parts[1],
            &[at(3.0, 4.0), at(4.0, 3.0), at(4.0, 1.0), at(3.0, 2.0)],
        );
    }
}

#[test]
fn a_single_point_between_gaps_and_a_band_of_no_height_draw_nothing() {
    let f = band_figure(
        Theme::pigment(),
        LIN,
        (&[0.0, 1.0, 2.0], &[0.0, NAN, 0.0], &[1.0, 2.0, 1.0]),
    );
    assert_eq!(items(&f), []);
    let mut f = band_figure(
        Theme::pigment(),
        LIN,
        (&[0.0, 1.0, 2.0], &[2.0, 2.0, 2.0], &[2.0, 2.0, 2.0]),
    );
    f.single_axes_mut().ylim(0.0, 4.0).unwrap();
    assert_eq!(items(&f), []);
}

#[test]
fn band_ignores_fill_style() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let color = theme.palette()[0];
        for style in [FillStyle::Solid, FillStyle::Gradient] {
            let theme = theme.with_fill_style(style).unwrap();
            let f = band_figure(
                theme,
                LIN,
                (&[0.0, 1.0, 2.0], &[1.0, 2.0, 1.0], &[3.0, 4.0, 3.0]),
            );
            // One flat fill and nothing else: no outline, no gradient.
            let items = items(&f);
            let [Item::Fill { paint, rule, .. }] = items.as_slice() else {
                panic!("expected one fill, got {items:?}")
            };
            assert_eq!(*rule, FillRule::NonZero);
            assert_eq!(
                *paint,
                Paint::Solid(color.scale_alpha(AREA_ALPHA)),
                "{style:?}"
            );
        }
    }
}

#[test]
fn band_is_drawn_in_call_order_with_the_data_marks() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(LIN, LIN)
        .unwrap()
        .band(
            &[0.0, 1.0],
            &[0.0, 0.0],
            &[1.0, 2.0],
            BandOptions::default(),
        )
        .unwrap()
        .line(nums(&[0.0, 1.0]), &[0.5, 1.0], LineOptions::default())
        .unwrap();
    let kinds: Vec<&str> = items(&f)
        .iter()
        .map(|item| match item {
            Item::Fill { .. } => "fill",
            Item::Stroke { .. } => "stroke",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["fill", "stroke"]);
}

#[test]
fn explicit_limits_cut_the_band_at_the_plot_and_at_its_clip_margin() {
    let (x, low, high) = ([0.0, 1.0, 2.0, 3.0], [1.0; 4], [5.0; 4]);
    // The plot shows x from 1 to 2 and y from 2 to 4; the band covers all of it, so the fill
    // reaches the clip margin of 2 pt on every side, which the marks group clips to the plot.
    let mut f = band_figure(Theme::pigment(), LIN, (&x, &low, &high));
    f.single_axes_mut()
        .xlim(1.0, 2.0)
        .unwrap()
        .ylim(2.0, 4.0)
        .unwrap();
    let plot = plot_of(&f);
    let parts = polygons(&f);
    assert_eq!(parts.len(), 1);
    let xs = parts[0].iter().map(|p| p.0);
    let ys = parts[0].iter().map(|p| p.1);
    let close = |got: f64, want: f64| assert!((got - want).abs() < 1e-3, "{got} vs {want}");
    close(xs.clone().fold(f64::INFINITY, f64::min), plot.x - 2.0);
    close(xs.fold(f64::NEG_INFINITY, f64::max), plot.right() + 2.0);
    close(ys.clone().fold(f64::INFINITY, f64::min), plot.y - 2.0);
    close(ys.fold(f64::NEG_INFINITY, f64::max), plot.bottom() + 2.0);

    // A band that ends exactly at the low limit has nothing inside the plot; one that starts
    // exactly at the high limit likewise.
    for (x, xlim) in [([0.0, 1.0], (1.0, 2.0)), ([2.0, 3.0], (1.0, 2.0))] {
        let mut f = band_figure(Theme::pigment(), LIN, (&x, &[1.0; 2], &[3.0; 2]));
        f.single_axes_mut()
            .xlim(xlim.0, xlim.1)
            .unwrap()
            .ylim(0.0, 4.0)
            .unwrap();
        let plot = plot_of(&f);
        for part in polygons(&f) {
            let inside = part
                .iter()
                .any(|p| p.0 > plot.x + 1e-3 && p.0 < plot.right() - 1e-3);
            assert!(!inside, "{part:?}");
        }
    }
    // A band entirely beyond a limit is not drawn.
    for (low, high) in [(5.0, 6.0), (-3.0, -1.0)] {
        let mut f = band_figure(Theme::pigment(), LIN, (&[0.0, 1.0], &[low; 2], &[high; 2]));
        f.single_axes_mut().ylim(0.0, 4.0).unwrap();
        assert_eq!(items(&f), [], "{low} {high}");
    }
}

#[test]
fn band_on_a_log_y_axis_is_placed_in_log_space() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let mut f = band_figure(
        Theme::pigment(),
        log,
        (&[0.0, 1.0], &[1.0, 10.0], &[10.0, 100.0]),
    );
    f.single_axes_mut()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(1.0, 100.0)
        .unwrap();
    let plot = plot_of(&f);
    let mid = plot.bottom() - plot.height / 2.0;
    let parts = polygons(&f);
    assert_eq!(parts.len(), 1);
    assert_points(
        &parts[0],
        &[
            (plot.x, mid),
            (plot.right(), plot.y),
            (plot.right(), mid),
            (plot.x, plot.bottom()),
        ],
    );
}

#[test]
fn band_downsampled_keeps_the_envelope() {
    let n = 200_000;
    let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let noise = |i: usize| {
        let h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40;
        2.0 * h as f64 / f64::from(1_u32 << 24) - 1.0
    };
    let mut low: Vec<f64> = (0..n).map(|i| -1.0 + 0.2 * noise(i)).collect();
    let mut high: Vec<f64> = (0..n).map(|i| 1.0 + 0.2 * noise(i)).collect();
    // The extremes that must survive, and a gap.
    high[12_345] = 3.9;
    low[150_001] = -3.9;
    high[99_999] = NAN;
    let mut f = band_figure(Theme::graphite(), LIN, (&xs, &low, &high));
    f.single_axes_mut()
        .xlim(0.0, n as f64)
        .unwrap()
        .ylim(-4.0, 4.0)
        .unwrap();
    let plot = plot_of(&f);
    let columns = downsample_columns(plot.width);
    let parts = polygons(&f);
    assert_eq!(parts.len(), 2, "the gap splits the band");
    let vertices: usize = parts.iter().map(Vec::len).sum();
    // Per column and run at most six points are kept, and each is a vertex of both curves.
    assert!(
        vertices <= 2 * 6 * (columns + 1),
        "{vertices} vertices for {columns} columns"
    );
    assert!(vertices < n / 4, "{vertices} vertices");
    let y_of = |v: f64| plot.bottom() - (v + 4.0) / 8.0 * plot.height;
    let near = |target: f64| parts.iter().flatten().any(|p| (p.1 - target).abs() < 1e-3);
    assert!(near(y_of(3.9)), "the highest high is kept");
    assert!(near(y_of(-3.9)), "the lowest low is kept");
    // The first and last point of the band are kept.
    let first = parts[0][0];
    assert!((first.0 - plot.x).abs() < 1e-3, "{first:?}");
    let ends: Vec<f64> = parts.iter().flatten().map(|p| p.0).collect();
    let right = ends.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (right - plot.right()).abs() < plot.width / n as f64 * 2.0 + 1e-3,
        "{right}"
    );
}
