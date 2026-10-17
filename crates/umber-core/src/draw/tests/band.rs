//! Band marks: the polygon of each run, NaN gaps, the flat fill in every fill style, clipping
//! at the limits, a log y axis, and downsampling.

use super::super::fill::AREA_ALPHA;
use super::super::line::downsample_columns;
use super::{nums, plot_of, subpaths};
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
    super::mark_items(&scene).to_vec()
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
        assert_eq!(items(&f), [], "{x:?}");
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

/// Twice the signed area of a polygon given as `(x, y)` points.
fn signed_area(part: &[(f64, f64)]) -> f64 {
    part.iter()
        .zip(part.iter().cycle().skip(1))
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum()
}

/// A figure of linear axes with the limits given and a red band, or none for `None`.
fn limited(xlim: (f64, f64), ylim: (f64, f64), band: Option<(&[f64], &[f64], &[f64])>) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(LIN, LIN)
        .unwrap()
        .xlim(xlim.0, xlim.1)
        .unwrap()
        .ylim(ylim.0, ylim.1)
        .unwrap();
    if let Some((x, low, high)) = band {
        f.single_axes_mut()
            .band(
                x,
                low,
                high,
                BandOptions {
                    color: crate::ColorSpec::Fixed(crate::Rgba::opaque(214, 73, 51)),
                    ..BandOptions::default()
                },
            )
            .unwrap();
    }
    f
}

#[test]
fn every_polygon_is_wound_the_same_way_and_separate_runs_do_not_cancel() {
    // Runs side by side, a pinch where low meets high, and a band cut by the limits: with x
    // sorted and low not above high, each polygon has the winding of the others.
    let nan = NAN;
    let f = limited(
        (0.0, 8.0),
        (-1.0, 3.0),
        Some((
            &[0.0, 2.0, nan, 3.0, 4.0, 5.0, nan, 6.0, 12.0],
            &[0.0, 0.0, nan, 1.0, 1.0, 0.5, nan, -4.0, -4.0],
            &[1.0, 1.0, nan, 2.0, 1.0, 2.0, nan, 5.0, 5.0],
        )),
    );
    let parts = polygons(&f);
    assert_eq!(parts.len(), 3);
    let signs: Vec<bool> = parts.iter().map(|p| signed_area(p) > 0.0).collect();
    assert!(signs.iter().all(|&s| s == signs[0]), "{signs:?}");
    let empty = limited((0.0, 8.0), (-1.0, 3.0), None);
    assert_ne!(f.to_png(72).unwrap(), empty.to_png(72).unwrap());
}

#[test]
fn a_band_that_only_touches_a_limit_leaves_no_pixel() {
    #[allow(clippy::type_complexity)]
    let cases: [(&[f64], &[f64], &[f64]); 7] = [
        // Above the high y limit, below the low one, left of the low x limit, right of the high.
        (&[1.0, 2.0], &[2.0, 2.0], &[3.0, 3.0]),
        (&[1.0, 2.0], &[0.0, 0.0], &[1.0, 1.0]),
        (&[0.0, 1.0], &[1.0, 1.0], &[2.0, 2.0]),
        (&[2.0, 3.0], &[1.0, 1.0], &[2.0, 2.0]),
        // Touching the top edge, with a tail out of view whose bounding box overlaps the plot.
        (&[0.0, 1.0, 2.0], &[1.0, 2.0, 2.0], &[2.0, 3.0, 3.0]),
        // Touching the plot only at a corner.
        (&[0.0, 2.0], &[1.0, 3.0], &[2.0, 4.0]),
        (&[2.0, 4.0], &[0.0, -2.0], &[1.0, -1.0]),
    ];
    let empty = limited((1.0, 2.0), (1.0, 2.0), None).to_png(72).unwrap();
    for (x, low, high) in cases {
        let f = limited((1.0, 2.0), (1.0, 2.0), Some((x, low, high)));
        assert_eq!(f.to_png(72).unwrap(), empty, "{x:?} {low:?} {high:?}");
        assert_eq!(items(&f), [], "{x:?} {low:?} {high:?}");
    }
}

#[test]
fn a_thin_band_beside_a_limit_is_still_drawn() {
    let tiny = 1e-3;
    for (x, low, high) in [
        // Just inside the top edge, and just inside the left edge.
        (&[1.0, 2.0][..], &[2.0 - tiny; 2][..], &[3.0; 2][..]),
        (&[0.0, 1.0 + tiny][..], &[1.0; 2][..], &[2.0; 2][..]),
    ] {
        let f = limited((1.0, 2.0), (1.0, 2.0), Some((x, low, high)));
        assert_eq!(polygons(&f).len(), 1, "{x:?} {low:?} {high:?}");
    }
}

/// The polyline `chain` over the x range `a..=b`: its lowest and highest y there.
fn over(chain: &[(f64, f64)], a: f64, b: f64) -> Option<(f64, f64)> {
    let at = |x: f64| {
        let k = chain.partition_point(|p| p.0 < x);
        match (k.checked_sub(1).and_then(|i| chain.get(i)), chain.get(k)) {
            (Some(p), Some(q)) if q.0 > p.0 => Some(p.1 + (q.1 - p.1) * (x - p.0) / (q.0 - p.0)),
            (_, Some(q)) if q.0 == x => Some(q.1),
            _ => None,
        }
    };
    let from = chain.partition_point(|p| p.0 <= a);
    let to = chain.partition_point(|p| p.0 < b);
    let mut ys: Vec<f64> = chain
        .get(from..to.max(from))
        .unwrap_or(&[])
        .iter()
        .map(|p| p.1)
        .collect();
    ys.extend(at(a));
    ys.extend(at(b));
    (!ys.is_empty()).then(|| {
        (
            ys.iter().copied().fold(f64::INFINITY, f64::min),
            ys.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    })
}

/// Checks that the one polygon of `f`, a band over the raw `(x, low, high)`, has in every device
/// column of the plot the highest and lowest upper edge, and the highest and lowest lower
/// edge, of the polygon through all the raw points.
#[track_caller]
fn assert_envelope_kept(
    f: &Figure,
    (x, low, high): (&[f64], &[f64], &[f64]),
    xlim: (f64, f64),
    ylim: (f64, f64),
) {
    let plot = plot_of(f);
    let px = |v: f64| plot.x + (v - xlim.0) / (xlim.1 - xlim.0) * plot.width;
    let py = |v: f64| plot.bottom() - (v - ylim.0) / (ylim.1 - ylim.0) * plot.height;
    let raw = |ys: &[f64]| -> Vec<(f64, f64)> {
        x.iter().zip(ys).map(|(&x, &y)| (px(x), py(y))).collect()
    };
    let (raw_up, raw_low) = (raw(high), raw(low));
    let parts = polygons(f);
    assert_eq!(parts.len(), 1);
    let part = &parts[0];
    // The upper edge runs to the right-most vertex, the lower edge comes back from it.
    let right = part.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let split = part.iter().position(|p| p.0 == right).unwrap();
    let up: Vec<(f64, f64)> = part[..=split].to_vec();
    let mut lo: Vec<(f64, f64)> = part[split..].to_vec();
    lo.reverse();
    let columns = downsample_columns(plot.width);
    let width = plot.width / columns as f64;
    for c in 0..columns {
        let (a, b) = (plot.x + c as f64 * width, plot.x + (c + 1) as f64 * width);
        for (got, want) in [(&up, &raw_up), (&lo, &raw_low)] {
            let (g, w) = (over(got, a, b), over(want, a, b));
            let (Some(g), Some(w)) = (g, w) else {
                assert_eq!(g.is_some(), w.is_some(), "column {c}");
                continue;
            };
            assert!(
                (g.0 - w.0).abs() < 0.05 && (g.1 - w.1).abs() < 0.05,
                "column {c}: {g:?} vs {w:?}"
            );
        }
    }
}

fn noisy(n: usize, seed: u64, amplitude: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let noise = |i: usize| {
        let h = (i as u64 ^ seed).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40;
        amplitude * h as f64 / f64::from(1_u32 << 24)
    };
    let x = (0..n).map(|i| i as f64).collect();
    let low = (0..n).map(|i| 1.0 + noise(i)).collect();
    let high = (0..n).map(|i| 3.0 + noise(i + n)).collect();
    (x, low, high)
}

#[test]
fn a_long_band_keeps_its_envelope_in_every_column() {
    let n = 200_000;
    let (x, low, high) = noisy(n, 5, 1.0);
    let (xlim, ylim) = ((0.0, n as f64 - 1.0), (0.0, 5.0));
    let f = limited(xlim, ylim, Some((&x, &low, &high)));
    assert_envelope_kept(&f, (&x, &low, &high), xlim, ylim);
}

#[test]
fn a_zoomed_band_keeps_its_envelope_and_ignores_the_points_out_of_view() {
    // Points far out of view on both sides, with spikes among them: only the neighbours of
    // the view take part, and the first and last columns hold what is in view.
    let n = 200_000;
    let (x, low, mut high) = noisy(n, 9, 0.05);
    high[40_000] = 4.9;
    high[160_000] = 4.9;
    high[59_999] = 4.9;
    high[140_001] = 4.9;
    let (xlim, ylim) = ((60_000.0, 140_000.0), (0.0, 5.0));
    let f = limited(xlim, ylim, Some((&x, &low, &high)));
    assert_envelope_kept(&f, (&x, &low, &high), xlim, ylim);
}

#[test]
fn a_spike_beside_the_low_limit_does_not_reach_the_first_visible_column() {
    let mut x: Vec<f64> = (-10_000..-1).map(f64::from).collect();
    x.extend([-0.002, -0.001, 0.03]);
    x.extend((1..=100).map(f64::from));
    let low = vec![0.0; x.len()];
    let mut high = vec![1.0; x.len()];
    high[x.iter().position(|&v| v == -0.002).unwrap()] = 100.0;
    let (xlim, ylim) = ((0.0, 100.0), (-1.0, 10.0));
    let f = limited(xlim, ylim, Some((&x, &low, &high)));
    assert_envelope_kept(&f, (&x, &low, &high), xlim, ylim);
}
