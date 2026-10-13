//! Step marks: the vertices of each position in plot points, NaN breaks, category and log
//! axes, the stroke, and downsampling.

use super::super::line::downsample_columns;
use super::{line_strokes, nums, plot_of, subpaths};
use crate::layout::Area;
use crate::model::{AxisData, Figure, Step, StepOptions};
use crate::scene::{LineCap, LineJoin};
use crate::test_support::scene::assert_scene_bounded;
use crate::{Dash, FontLibrary, LogBase, ScaleKind, Theme};

fn step_figure(
    x: AxisData,
    y: &[f64],
    options: StepOptions,
    kinds: (ScaleKind, ScaleKind),
) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    f.add_axes(kinds.0, kinds.1)
        .unwrap()
        .step(x, y, options)
        .unwrap();
    f
}

fn at_position(position: Step) -> StepOptions {
    StepOptions {
        position,
        ..StepOptions::default()
    }
}

/// The points of every subpath of the one stroke of `figure`'s marks, as `(x, y)` pairs.
fn paths(figure: &Figure) -> Vec<Vec<(f64, f64)>> {
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let strokes = line_strokes(&scene);
    let [(path, _)] = strokes.as_slice() else {
        panic!("expected one stroke, got {strokes:?}")
    };
    subpaths(path)
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

/// The plot point of the data point `(x, y)` on linear axes limited to `0..=xmax` and `0..=ymax`.
fn plot_at(plot: Area, (xmax, ymax): (f64, f64), (x, y): (f64, f64)) -> (f64, f64) {
    (
        plot.x + x / xmax * plot.width,
        plot.bottom() - y / ymax * plot.height,
    )
}

#[test]
fn step_post_pre_mid_vertices() {
    let (x, y) = ([0.0, 1.0, 3.0], [1.0, 2.0, 0.0]);
    let cases: [(Step, &[(f64, f64)]); 3] = [
        (
            Step::Post,
            &[(0.0, 1.0), (1.0, 1.0), (1.0, 2.0), (3.0, 2.0), (3.0, 0.0)],
        ),
        (
            Step::Pre,
            &[(0.0, 1.0), (0.0, 2.0), (1.0, 2.0), (1.0, 0.0), (3.0, 0.0)],
        ),
        (
            Step::Mid,
            &[
                (0.0, 1.0),
                (0.5, 1.0),
                (0.5, 2.0),
                (2.0, 2.0),
                (2.0, 0.0),
                (3.0, 0.0),
            ],
        ),
    ];
    for (position, vertices) in cases {
        let mut f = step_figure(
            nums(&x),
            &y,
            at_position(position),
            (ScaleKind::Linear, ScaleKind::Linear),
        );
        f.single_axes_mut()
            .xlim(0.0, 3.0)
            .unwrap()
            .ylim(0.0, 2.0)
            .unwrap();
        let plot = plot_of(&f);
        let expected: Vec<(f64, f64)> = vertices
            .iter()
            .map(|&v| plot_at(plot, (3.0, 2.0), v))
            .collect();
        let parts = paths(&f);
        assert_eq!(parts.len(), 1, "{position:?}");
        assert_points(&parts[0], &expected);
    }
}

#[test]
fn step_nan_breaks() {
    let nan = f64::NAN;
    let mut f = step_figure(
        nums(&[0.0, 1.0, 2.0, 3.0, 4.0]),
        &[1.0, 2.0, nan, 4.0, 5.0],
        StepOptions::default(),
        (ScaleKind::Linear, ScaleKind::Linear),
    );
    f.single_axes_mut()
        .xlim(0.0, 4.0)
        .unwrap()
        .ylim(0.0, 5.0)
        .unwrap();
    let plot = plot_of(&f);
    let at = |x, y| plot_at(plot, (4.0, 5.0), (x, y));
    let parts = paths(&f);
    assert_eq!(parts.len(), 2);
    assert_points(
        &parts[0],
        &[at(0.0, 1.0), at(1.0, 1.0), at(1.0, 2.0), at(2.0, 2.0)],
    );
    assert_points(&parts[1], &[at(3.0, 4.0), at(4.0, 4.0), at(4.0, 5.0)]);

    // A point with no neighbour to hold its level to is a dot, as in a line.
    let f = step_figure(
        nums(&[0.0, 1.0, nan]),
        &[nan, 1.0, 2.0],
        StepOptions::default(),
        (ScaleKind::Linear, ScaleKind::Linear),
    );
    let parts = paths(&f);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].len(), 2);
    assert_eq!(parts[0][0], parts[0][1]);
}

#[test]
fn step_on_a_category_axis_runs_between_band_centres() {
    let names = AxisData::Categories(vec!["a".into(), "b".into(), "c".into(), "d".into()]);
    let f = step_figure(
        names,
        &[1.0, 3.0, 2.0, 4.0],
        at_position(Step::Post),
        (ScaleKind::Category, ScaleKind::Linear),
    );
    let plot = plot_of(&f);
    let parts = paths(&f);
    assert_eq!(parts.len(), 1);
    let xs: Vec<f64> = parts[0].iter().map(|p| p.0).collect();
    // Vertices: (a, 1), (b, 1), (b, 3), (c, 3), (c, 2), (d, 2), (d, 4).
    assert_eq!(xs.len(), 7);
    let band = plot.width / 4.0;
    for (got, centre) in [xs[0], xs[1], xs[3], xs[5]]
        .into_iter()
        .zip([0.5, 1.5, 2.5, 3.5])
    {
        assert!((got - (plot.x + centre * band)).abs() < 1e-3, "{xs:?}");
    }
    // Every level change is a vertical segment.
    assert_eq!((xs[1], xs[3], xs[5]), (xs[2], xs[4], xs[6]));
}

#[test]
fn step_mid_on_a_log_axis_changes_level_midway_in_log_space() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let mut f = step_figure(
        nums(&[1.0, 100.0]),
        &[1.0, 2.0],
        at_position(Step::Mid),
        (log, ScaleKind::Linear),
    );
    f.single_axes_mut()
        .xlim(1.0, 100.0)
        .unwrap()
        .ylim(0.0, 2.0)
        .unwrap();
    let plot = plot_of(&f);
    let parts = paths(&f);
    assert_eq!(parts.len(), 1);
    let xs: Vec<f64> = parts[0].iter().map(|p| p.0).collect();
    // 10 is the middle of 1..=100 on a log axis.
    assert_eq!(xs.len(), 4);
    assert!((xs[1] - (plot.x + plot.width / 2.0)).abs() < 1e-3, "{xs:?}");
    assert_eq!(xs[1], xs[2]);
}

#[test]
fn step_is_stroked_as_a_line_is() {
    let theme = Theme::graphite();
    for (dash, pattern, cap) in [
        (Dash::Solid, vec![], LineCap::Round),
        (Dash::Dashed, vec![4.0 * 1.8, 3.0 * 1.8], LineCap::Butt),
        (Dash::Dotted, vec![0.0, 2.5 * 1.8], LineCap::Round),
    ] {
        let f = step_figure(
            nums(&[0.0, 1.0]),
            &[0.0, 1.0],
            StepOptions {
                dash,
                ..StepOptions::default()
            },
            (ScaleKind::Linear, ScaleKind::Linear),
        );
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let strokes = line_strokes(&scene);
        let style = strokes[0].1;
        assert_eq!(style.dash, pattern, "{dash:?}");
        assert_eq!(
            (style.cap, style.join, style.width, style.color),
            (cap, LineJoin::Round, theme.line_width(), theme.palette()[0])
        );
    }
}

#[test]
fn step_downsampled_keeps_extremes() {
    let n = 200_000;
    let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
    // Deterministic noise within -1..1, and two spikes that must survive.
    let mut ys: Vec<f64> = (0..n)
        .map(|i| {
            let h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40;
            2.0 * h as f64 / f64::from(1_u32 << 24) - 1.0
        })
        .collect();
    ys[12_345] = 1.9;
    ys[150_001] = -1.9;
    ys[99_999] = f64::NAN;
    let mut f = step_figure(
        nums(&xs),
        &ys,
        StepOptions::default(),
        (ScaleKind::Linear, ScaleKind::Linear),
    );
    f.single_axes_mut()
        .xlim(0.0, n as f64)
        .unwrap()
        .ylim(-2.0, 2.0)
        .unwrap();
    let plot = plot_of(&f);
    let columns = downsample_columns(plot.width);
    let parts = paths(&f);
    assert_eq!(parts.len(), 2, "the gap splits the path");
    let points: usize = parts.iter().map(Vec::len).sum();
    // Each kept point makes at most two vertices.
    assert!(
        points <= 8 * columns + 16,
        "{points} vertices for {columns} columns"
    );
    assert!(points < n / 4, "{points} vertices");
    let y_of = |v: f64| plot.bottom() - (v + 2.0) / 4.0 * plot.height;
    let near = |target: f64| parts.iter().flatten().any(|p| (p.1 - target).abs() < 1e-3);
    assert!(near(y_of(1.9)), "the highest point is kept");
    assert!(near(y_of(-1.9)), "the lowest point is kept");
}
