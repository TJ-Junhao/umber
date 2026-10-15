//! Visible-column envelopes of long lines, areas and steps under explicit limits.

use super::super::line::downsample_columns;
use super::{line_strokes, mark_group, nums, plot_of, subpaths};
use crate::model::{AreaOptions, BandOptions, Figure, LineOptions, Series, StepOptions};
use crate::scene::Item;
use crate::{FontLibrary, ScaleKind, Step, Theme};

#[derive(Clone, Copy, Debug)]
enum Kind {
    Line,
    Area,
    Step(Step),
    Band,
    StackedArea,
}

fn figure(kind: Kind, x: &[f64], y: &[f64], limits: (f64, f64)) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    a.xlim(limits.0, limits.1).unwrap().ylim(0.0, 10.0).unwrap();
    match kind {
        Kind::Line => {
            a.line(nums(x), y, LineOptions::default()).unwrap();
        }
        Kind::Area => {
            a.area(nums(x), y, AreaOptions::default()).unwrap();
        }
        Kind::Band => {
            a.band(x, &vec![0.0; x.len()], y, BandOptions::default())
                .unwrap();
        }
        Kind::StackedArea => {
            a.stacked_area(
                x,
                vec![Series {
                    values: y.to_vec(),
                    ..Series::default()
                }],
            )
            .unwrap();
        }
        Kind::Step(position) => {
            a.step(
                nums(x),
                y,
                StepOptions {
                    position,
                    ..StepOptions::default()
                },
            )
            .unwrap();
        }
    }
    f
}

/// The full path, with step corners constructed independently of the drawing code.
fn full_path(kind: Kind, x: &[f64], y: &[f64]) -> Vec<(f64, f64)> {
    let mut out = vec![(x[0], y[0])];
    for i in 1..x.len() {
        if let Kind::Step(position) = kind {
            let turn = match position {
                Step::Post => x[i],
                Step::Pre => x[i - 1],
                Step::Mid => (x[i - 1] + x[i]) / 2.0,
            };
            out.extend([(turn, y[i - 1]), (turn, y[i])]);
        }
        out.push((x[i], y[i]));
    }
    out
}

/// The envelope over a closed column, including its intersections with segments.
fn over(path: &[(f64, f64)], lo: f64, hi: f64) -> (f64, f64) {
    let start = path.partition_point(|p| p.0 < lo).saturating_sub(1);
    let end = (path.partition_point(|p| p.0 <= hi) + 1).min(path.len());
    let mut extent = (f64::INFINITY, f64::NEG_INFINITY);
    let mut add = |y: f64| {
        extent.0 = extent.0.min(y);
        extent.1 = extent.1.max(y);
    };
    for &(x, y) in &path[start..end] {
        if x >= lo && x <= hi {
            add(y);
        }
    }
    for pair in path[start..end].windows(2) {
        let (a, b) = (pair[0], pair[1]);
        for edge in [lo, hi] {
            if a.0 < edge && edge < b.0 {
                add(a.1 + (b.1 - a.1) * (edge - a.0) / (b.0 - a.0));
            }
        }
    }
    extent
}

#[track_caller]
fn assert_envelope(kind: Kind, x: &[f64], y: &[f64], limits: (f64, f64)) {
    let f = figure(kind, x, y, limits);
    let plot = plot_of(&f);
    let raw: Vec<_> = full_path(kind, x, y)
        .into_iter()
        .map(|(x, y)| {
            (
                plot.x + (x - limits.0) / (limits.1 - limits.0) * plot.width,
                plot.bottom() - y / 10.0 * plot.height,
            )
        })
        .collect();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let strokes = line_strokes(&scene);
    assert_eq!(strokes.len(), 1);
    let parts = subpaths(strokes[0].0);
    assert_eq!(parts.len(), 1);
    let drawn: Vec<_> = parts[0]
        .iter()
        .map(|p| (f64::from(p.x), f64::from(p.y)))
        .collect();
    let fills: Vec<_> = mark_group(&scene)
        .1
        .iter()
        .filter_map(|item| match item {
            Item::Fill { path, .. } => Some(subpaths(path)),
            _ => None,
        })
        .flatten()
        .collect();
    if matches!(kind, Kind::Area) {
        assert_eq!(fills.len(), 1);
    }
    let columns = downsample_columns(plot.width);
    for c in 0..columns {
        let lo = plot.x + c as f64 / columns as f64 * plot.width;
        let hi = plot.x + (c + 1) as f64 / columns as f64 * plot.width;
        let (got, want) = (over(&drawn, lo, hi), over(&raw, lo, hi));
        assert!(
            (got.0 - want.0).abs() < 0.05 && (got.1 - want.1).abs() < 0.05,
            "{kind:?}, column {c}: {got:?} vs {want:?}"
        );
        for fill in &fills {
            // The boundary follows the line, then returns along the baseline.
            let edge: Vec<_> = fill[..fill.len() - 2]
                .iter()
                .map(|p| (f64::from(p.x), f64::from(p.y)))
                .collect();
            let got = over(&edge, lo, hi);
            assert!(
                (got.0 - want.0).abs() < 0.05 && (got.1 - want.1).abs() < 0.05,
                "area fill, column {c}: {got:?} vs {want:?}"
            );
        }
    }
}

fn spike(kind: Kind) {
    let mut x: Vec<f64> = (-10_000..-1).map(f64::from).collect();
    x.extend([-0.002, -0.001, 0.03]);
    x.extend((1..=100).map(f64::from));
    let mut y = vec![1.0; x.len()];
    y[x.iter().position(|&v| v == -0.002).unwrap()] = 9.0;
    assert_envelope(kind, &x, &y, (0.0, 100.0));
}

fn with_wide_neighbour(mut f: Figure) -> Figure {
    f.single_axes_mut()
        .line(
            nums(&[25.0, 75.0]),
            &[2.0, 2.0],
            LineOptions {
                width: crate::SizeSpec::pt(8.0).unwrap(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    f
}

fn half_stroke_strip(kind: Kind) {
    for right in [false, true] {
        let mut x: Vec<_> = (0..10_000)
            .map(|i| -0.5 + f64::from(i) / 25_000.0)
            .collect();
        if right {
            x = x.into_iter().rev().map(|x| 100.0 - x).collect();
            x.splice(0..0, [0.0, 100.0]);
        } else {
            x.extend([0.0, 100.0]);
        }
        let mut y = vec![1.0; x.len()];
        y[2_500] = 8.0;
        let f = with_wide_neighbour(figure(kind, &x, &y, (0.0, 100.0)));
        let plot = plot_of(&f);
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let (clip, items) = mark_group(&scene);
        let path = match &items[0] {
            Item::Fill { path, .. } | Item::Stroke { path, .. } => path,
            item => panic!("unexpected item: {item:?}"),
        };
        let peak = plot.bottom() - 0.8 * plot.height;
        assert!(
            subpaths(path).iter().flatten().any(|p| {
                let px = f64::from(p.x);
                (f64::from(p.y) - peak).abs() < 0.001
                    && px >= f64::from(clip.x)
                    && px <= f64::from(clip.x + clip.width)
                    && if right {
                        px > plot.right()
                    } else {
                        px < plot.x
                    }
            }),
            "{kind:?}, right={right}: missing peak in the half-stroke strip"
        );
    }
}

#[test]
fn line_keeps_extrema_in_the_half_stroke_strip() {
    half_stroke_strip(Kind::Line);
}
#[test]
fn area_keeps_extrema_in_the_half_stroke_strip() {
    half_stroke_strip(Kind::Area);
}
#[test]
fn step_keeps_extrema_in_the_half_stroke_strip() {
    for position in [Step::Post, Step::Pre, Step::Mid] {
        half_stroke_strip(Kind::Step(position));
    }
}
#[test]
fn band_keeps_extrema_in_the_half_stroke_strip() {
    half_stroke_strip(Kind::Band);
}
#[test]
fn stacked_area_keeps_extrema_in_the_half_stroke_strip() {
    half_stroke_strip(Kind::StackedArea);
}

fn step_gap_at_view_edge(position: Step) {
    for right in [false, true] {
        for gap_x in [false, true] {
            let mut x = vec![-2.0, -1.2, -1.1, -0.5, -0.3, -0.1, 0.1, 1.0, 100.0];
            let mut y = vec![
                f64::NAN,
                4.0,
                f64::NAN,
                5.0,
                f64::NAN,
                8.0,
                f64::NAN,
                1.0,
                1.0,
            ];
            if gap_x {
                x[2] = f64::NAN;
            }
            let (long_x, long_y) = if right {
                x = x.into_iter().rev().map(|x| 100.0 - x).collect();
                y.reverse();
                let mut long_x = x.clone();
                long_x.extend((103..10_103).map(f64::from));
                let mut long_y = y.clone();
                long_y.resize(long_x.len(), f64::NAN);
                (long_x, long_y)
            } else {
                let mut long_x: Vec<_> = (-10_002..-2).map(f64::from).collect();
                let mut long_y = vec![f64::NAN; long_x.len()];
                long_x.extend(&x);
                long_y.extend(&y);
                (long_x, long_y)
            };
            let make = |x: &[f64], y: &[f64]| {
                let f = with_wide_neighbour(figure(Kind::Step(position), x, y, (0.0, 100.0)));
                let scene = f.to_scene(FontLibrary::global()).unwrap();
                subpaths(line_strokes(&scene)[0].0)
            };
            assert_eq!(
                make(&long_x, &long_y),
                make(&x, &y),
                "{position:?}, right={right}, gap_x={gap_x}"
            );
        }
    }
}

#[test]
fn step_post_keeps_holds_next_to_a_nan_gap_at_the_view_edge() {
    step_gap_at_view_edge(Step::Post);
}
#[test]
fn step_pre_keeps_holds_next_to_a_nan_gap_at_the_view_edge() {
    step_gap_at_view_edge(Step::Pre);
}
#[test]
fn step_mid_keeps_holds_next_to_a_nan_gap_at_the_view_edge() {
    step_gap_at_view_edge(Step::Mid);
}

#[test]
fn line_offscreen_spike_stays_outside() {
    spike(Kind::Line);
}
#[test]
fn area_offscreen_spike_stays_outside() {
    spike(Kind::Area);
}
#[test]
fn step_post_offscreen_spike_stays_outside() {
    spike(Kind::Step(Step::Post));
}
#[test]
fn step_mid_offscreen_spike_stays_outside() {
    spike(Kind::Step(Step::Mid));
}
#[test]
fn step_pre_offscreen_spike_stays_outside() {
    spike(Kind::Step(Step::Pre));
}

fn zoomed(kind: Kind) {
    let n = 200_000;
    let x: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let mut y: Vec<_> = (0..n)
        .map(|i| {
            let h = (i as u64 ^ 9).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40;
            1.0 + 0.05 * h as f64 / f64::from(1_u32 << 24)
        })
        .collect();
    for i in [40_000, 59_999, 140_001, 160_000] {
        y[i] = 9.0;
    }
    assert_envelope(kind, &x, &y, (60_000.25, 139_999.75));
}

#[test]
fn zoomed_line_keeps_every_column_envelope() {
    zoomed(Kind::Line);
}
#[test]
fn zoomed_area_keeps_every_column_envelope() {
    zoomed(Kind::Area);
}
#[test]
fn zoomed_step_post_keeps_every_column_envelope() {
    zoomed(Kind::Step(Step::Post));
}
#[test]
fn zoomed_step_mid_keeps_every_column_envelope() {
    zoomed(Kind::Step(Step::Mid));
}
#[test]
fn zoomed_step_pre_keeps_every_column_envelope() {
    zoomed(Kind::Step(Step::Pre));
}
