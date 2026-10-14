//! Error bars: the stem and the caps of each bar in plot points, the width and the cap length,
//! NaN, categories, both orientations, and the edges of the domain, where a bar that touches a
//! limit, crosses it or lies beyond it each have a defined result.

use super::{mark_group, nums, plot_of, subpaths};
use crate::layout::Area;
use crate::model::{AxisData, ErrorBarOptions, Figure};
use crate::scene::{Item, LineCap, Scene};
use crate::test_support::scene::assert_scene_bounded;
use crate::{FontLibrary, LogBase, ScaleKind, SizeSpec, Theme};

const LIN: ScaleKind = ScaleKind::Linear;
const NAN: f64 = f64::NAN;

/// Linear axes limited to `0..=10` on both, holding nothing else.
fn figure_in(theme: Theme, (x, y): (ScaleKind, ScaleKind)) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(x, y).unwrap();
    f
}

fn figure() -> Figure {
    let mut f = figure_in(Theme::pigment(), (LIN, LIN));
    f.single_axes_mut()
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap();
    f
}

fn scene_of(figure: &Figure) -> Scene {
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    scene
}

/// The items of the marks group of `figure`; empty when nothing is visible.
fn items(figure: &Figure) -> Vec<Item> {
    let scene = scene_of(figure);
    if scene.items.len() == 2 {
        return Vec::new();
    }
    mark_group(&scene).1.to_vec()
}

type Segment = ((f64, f64), (f64, f64));

/// The one stroke of the error bars of `figure`, as its segments in drawing order, and its
/// style.
fn segments(figure: &Figure) -> (Vec<Segment>, crate::scene::StrokeStyle) {
    let items = items(figure);
    let [Item::Stroke { path, style }] = items.as_slice() else {
        panic!("one stroke expected, got {items:?}")
    };
    let parts = subpaths(path)
        .into_iter()
        .map(|part| {
            let [a, b] = part.as_slice() else {
                panic!("a segment expected, got {part:?}")
            };
            (
                (f64::from(a.x), f64::from(a.y)),
                (f64::from(b.x), f64::from(b.y)),
            )
        })
        .collect();
    (parts, style.clone())
}

fn x_at(plot: Area, v: f64) -> f64 {
    plot.x + v / 10.0 * plot.width
}

fn y_at(plot: Area, v: f64) -> f64 {
    plot.bottom() - v / 10.0 * plot.height
}

#[track_caller]
fn assert_near(got: f64, expected: f64) {
    assert!((got - expected).abs() < 1e-3, "{got} vs {expected}");
}

#[track_caller]
fn assert_segment(got: Segment, expected: Segment) {
    assert_near(got.0.0, expected.0.0);
    assert_near(got.0.1, expected.0.1);
    assert_near(got.1.0, expected.1.0);
    assert_near(got.1.1, expected.1.1);
}

fn vertical(f: &mut Figure, x: &[f64], low: &[f64], high: &[f64], options: ErrorBarOptions) {
    f.single_axes_mut()
        .y_error_bars(nums(x), low, high, options)
        .unwrap();
}

fn no_caps() -> ErrorBarOptions {
    ErrorBarOptions {
        caps: false,
        ..ErrorBarOptions::default()
    }
}

#[test]
fn a_vertical_bar_is_a_stem_with_a_cap_at_each_end() {
    let mut f = figure();
    vertical(&mut f, &[5.0], &[2.0], &[7.0], ErrorBarOptions::default());
    let plot = plot_of(&f);
    let (parts, style) = segments(&f);
    let (x, cap) = (x_at(plot, 5.0), f64::from(Theme::pigment().cap_width()));
    assert_eq!(parts.len(), 3);
    assert_segment(parts[0], ((x, y_at(plot, 2.0)), (x, y_at(plot, 7.0))));
    assert_segment(
        parts[1],
        (
            (x - cap / 2.0, y_at(plot, 2.0)),
            (x + cap / 2.0, y_at(plot, 2.0)),
        ),
    );
    assert_segment(
        parts[2],
        (
            (x - cap / 2.0, y_at(plot, 7.0)),
            (x + cap / 2.0, y_at(plot, 7.0)),
        ),
    );
    // Flat ends, solid, the theme's line width.
    assert_eq!(style.cap, LineCap::Butt);
    assert!(style.dash.is_empty());
    assert_eq!(style.width, Theme::pigment().line_width());
}

#[test]
fn a_horizontal_bar_is_a_stem_with_a_cap_at_each_end() {
    let mut f = figure();
    f.single_axes_mut()
        .x_error_bars(&[4.0], &[1.0], &[6.0], ErrorBarOptions::default())
        .unwrap();
    let plot = plot_of(&f);
    let (parts, _) = segments(&f);
    let (y, cap) = (y_at(plot, 4.0), f64::from(Theme::pigment().cap_width()));
    assert_eq!(parts.len(), 3);
    assert_segment(parts[0], ((x_at(plot, 1.0), y), (x_at(plot, 6.0), y)));
    assert_segment(
        parts[1],
        (
            (x_at(plot, 1.0), y - cap / 2.0),
            (x_at(plot, 1.0), y + cap / 2.0),
        ),
    );
    assert_segment(
        parts[2],
        (
            (x_at(plot, 6.0), y - cap / 2.0),
            (x_at(plot, 6.0), y + cap / 2.0),
        ),
    );
}

#[test]
fn caps_are_cap_width_wide() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = figure_in(theme.clone(), (LIN, LIN));
        f.single_axes_mut()
            .xlim(0.0, 10.0)
            .unwrap()
            .ylim(0.0, 10.0)
            .unwrap();
        vertical(&mut f, &[5.0], &[2.0], &[7.0], ErrorBarOptions::default());
        let (parts, _) = segments(&f);
        for cap in &parts[1..] {
            assert_near(cap.1.0 - cap.0.0, 8.0);
            assert_eq!(cap.0.1, cap.1.1);
        }
    }
    // The cap length does not follow the width of the line, and the bar's centre is the cap's.
    let mut f = figure();
    vertical(
        &mut f,
        &[5.0],
        &[2.0],
        &[7.0],
        ErrorBarOptions {
            width: SizeSpec::times(3.0).unwrap(),
            ..ErrorBarOptions::default()
        },
    );
    let (parts, _) = segments(&f);
    assert_near(parts[1].1.0 - parts[1].0.0, 8.0);
}

#[test]
fn no_caps_when_disabled() {
    let mut f = figure();
    vertical(&mut f, &[3.0, 6.0], &[1.0, 2.0], &[4.0, 8.0], no_caps());
    let (parts, _) = segments(&f);
    assert_eq!(parts.len(), 2);
    for (stem, x) in parts.iter().zip([3.0, 6.0]) {
        assert_near(stem.0.0, x_at(plot_of(&f), x));
        assert_eq!(stem.0.0, stem.1.0);
    }
}

#[test]
fn the_line_width_is_the_theme_line_width_times_the_size() {
    let theme = Theme::pigment();
    for (width, expected) in [
        (SizeSpec::Auto, theme.line_width()),
        (SizeSpec::times(2.0).unwrap(), 2.0 * theme.line_width()),
        (SizeSpec::pt(1.0).unwrap(), 1.0),
    ] {
        let mut f = figure();
        vertical(
            &mut f,
            &[5.0],
            &[2.0],
            &[7.0],
            ErrorBarOptions {
                width,
                ..ErrorBarOptions::default()
            },
        );
        let (_, style) = segments(&f);
        assert_near(f64::from(style.width), f64::from(expected));
    }
}

#[test]
fn nan_in_any_array_skips_that_bar_only() {
    let mut f = figure();
    vertical(
        &mut f,
        &[1.0, NAN, 3.0, 4.0, 5.0],
        &[1.0, 1.0, NAN, 1.0, 1.0],
        &[2.0, 2.0, 2.0, NAN, 2.0],
        no_caps(),
    );
    let plot = plot_of(&f);
    let (parts, _) = segments(&f);
    assert_eq!(parts.len(), 2);
    assert_near(parts[0].0.0, x_at(plot, 1.0));
    assert_near(parts[1].0.0, x_at(plot, 5.0));
    // Nothing but gaps draws nothing.
    let mut f = figure();
    vertical(&mut f, &[NAN], &[1.0], &[2.0], ErrorBarOptions::default());
    assert!(items(&f).is_empty());
}

#[test]
fn category_error_bars_at_band_centres() {
    let mut f = figure_in(Theme::pigment(), (ScaleKind::Category, LIN));
    f.single_axes_mut()
        .ylim(0.0, 10.0)
        .unwrap()
        .y_error_bars(
            AxisData::Categories(vec!["a".into(), "b".into(), "c".into(), "b".into()]),
            &[1.0, 2.0, 3.0, 4.0],
            &[2.0, 3.0, 4.0, 5.0],
            no_caps(),
        )
        .unwrap();
    let plot = plot_of(&f);
    let (parts, _) = segments(&f);
    // Three bands, the centres at 1/6, 3/6 and 5/6 of the plot; a repeated name has one centre.
    let centre = |k: f64| plot.x + k / 6.0 * plot.width;
    let xs: Vec<f64> = parts.iter().map(|s| s.0.0).collect();
    assert_eq!(xs.len(), 4);
    assert_near(xs[0], centre(1.0));
    assert_near(xs[1], centre(3.0));
    assert_near(xs[2], centre(5.0));
    assert_near(xs[3], centre(3.0));
}

#[test]
fn a_log_y_axis_maps_the_limits_logarithmically() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let mut f = figure_in(Theme::pigment(), (LIN, log));
    f.single_axes_mut()
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(1.0, 1000.0)
        .unwrap()
        .y_error_bars(nums(&[5.0]), &[10.0], &[100.0], no_caps())
        .unwrap();
    let plot = plot_of(&f);
    let (parts, _) = segments(&f);
    // 10 and 100 are one third and two thirds up the axis.
    assert_segment(
        parts[0],
        (
            (x_at(plot, 5.0), plot.bottom() - plot.height / 3.0),
            (x_at(plot, 5.0), plot.bottom() - 2.0 * plot.height / 3.0),
        ),
    );
}

#[test]
fn a_bar_that_runs_past_a_limit_is_cut_and_loses_the_cap_there() {
    let mut f = figure();
    vertical(&mut f, &[5.0], &[-3.0], &[7.0], ErrorBarOptions::default());
    let plot = plot_of(&f);
    let (parts, _) = segments(&f);
    // The stem runs from the bottom of the plot; only the cap at 7 is left.
    assert_eq!(parts.len(), 2);
    assert_segment(
        parts[0],
        (
            (x_at(plot, 5.0), plot.bottom()),
            (x_at(plot, 5.0), y_at(plot, 7.0)),
        ),
    );
    assert_near(parts[1].0.1, y_at(plot, 7.0));
    // Past both limits: a full-height stem and no caps.
    let mut f = figure();
    vertical(&mut f, &[5.0], &[-3.0], &[13.0], ErrorBarOptions::default());
    let (parts, _) = segments(&f);
    assert_eq!(parts.len(), 1);
    assert_segment(
        parts[0],
        ((x_at(plot, 5.0), plot.bottom()), (x_at(plot, 5.0), plot.y)),
    );
    // Past the high limit of a horizontal bar.
    let mut f = figure();
    f.single_axes_mut()
        .x_error_bars(&[5.0], &[2.0], &[14.0], ErrorBarOptions::default())
        .unwrap();
    let (parts, _) = segments(&f);
    assert_eq!(parts.len(), 2);
    assert_segment(
        parts[0],
        (
            (x_at(plot, 2.0), y_at(plot, 5.0)),
            (plot.right(), y_at(plot, 5.0)),
        ),
    );
    assert_near(parts[1].0.0, x_at(plot, 2.0));
}

#[test]
fn limits_exactly_on_the_domain_keep_their_caps() {
    let mut f = figure();
    vertical(&mut f, &[5.0], &[0.0], &[10.0], ErrorBarOptions::default());
    let plot = plot_of(&f);
    let (parts, _) = segments(&f);
    assert_eq!(parts.len(), 3);
    assert_near(parts[1].0.1, plot.bottom());
    assert_near(parts[2].0.1, plot.y);
    // A bar that only touches the domain with one end keeps that end's cap and has no stem.
    let mut f = figure();
    vertical(&mut f, &[5.0], &[-4.0], &[0.0], ErrorBarOptions::default());
    let (parts, _) = segments(&f);
    assert_eq!(parts.len(), 1);
    assert_near(parts[0].0.1, plot.bottom());
    let mut f = figure();
    vertical(&mut f, &[5.0], &[10.0], &[14.0], ErrorBarOptions::default());
    let (parts, _) = segments(&f);
    assert_eq!(parts.len(), 1);
    assert_near(parts[0].0.1, plot.y);
    // Without caps those touching bars have nothing to draw.
    for (low, high) in [(-4.0, 0.0), (10.0, 14.0)] {
        let mut f = figure();
        vertical(&mut f, &[5.0], &[low], &[high], no_caps());
        assert!(items(&f).is_empty());
    }
}

#[test]
fn a_bar_wholly_beyond_the_domain_draws_nothing() {
    for (position, low, high) in [
        (5.0, 11.0, 14.0),
        (5.0, -9.0, -1.0),
        (12.0, 2.0, 5.0),
        (-3.0, 2.0, 5.0),
    ] {
        let mut f = figure();
        vertical(
            &mut f,
            &[position],
            &[low],
            &[high],
            ErrorBarOptions::default(),
        );
        assert!(items(&f).is_empty(), "{position} {low} {high}");
    }
    // A bar at an x on the limit is kept; one a few points beyond is dropped, one whose cap
    // still reaches in is kept for the cap.
    let plot = plot_of(&figure());
    let points = |p: f64| p / plot.width * 10.0;
    for (x, kept) in [
        (0.0, true),
        (10.0, true),
        (-points(3.0), true),
        (-points(4.5), false),
        (10.0 + points(3.9), true),
        (10.0 + points(4.1), false),
    ] {
        let mut f = figure();
        vertical(&mut f, &[x], &[2.0], &[7.0], ErrorBarOptions::default());
        assert_eq!(!items(&f).is_empty(), kept, "x {x}");
    }
    // Without caps only half the stroke can reach in.
    let half = f64::from(Theme::pigment().line_width()) / 2.0;
    let mut f = figure();
    vertical(
        &mut f,
        &[-half / plot.width * 10.0 * 1.1],
        &[2.0],
        &[7.0],
        no_caps(),
    );
    assert!(items(&f).is_empty());
    let mut f = figure();
    vertical(
        &mut f,
        &[-half / plot.width * 10.0 * 0.9],
        &[2.0],
        &[7.0],
        no_caps(),
    );
    assert!(!items(&f).is_empty());
}

#[test]
fn huge_limits_do_not_overflow_the_coordinates() {
    let mut f = figure();
    vertical(
        &mut f,
        &[5.0, 6.0],
        &[-1e300, 2.0],
        &[1e300, 1e299],
        ErrorBarOptions::default(),
    );
    // The scene is bounded (checked by `items`), and the first bar spans the plot.
    let (parts, _) = segments(&f);
    // The first bar is a bare stem; the second keeps its stem and the cap at 2.
    assert_eq!(parts.len(), 3);
    assert_near(parts[0].0.1, plot_of(&f).bottom());
}

#[test]
fn the_mark_is_drawn_in_call_order_with_the_data_marks() {
    let mut f = figure();
    f.single_axes_mut()
        .line(
            nums(&[0.0, 10.0]),
            &[1.0, 9.0],
            crate::model::LineOptions::default(),
        )
        .unwrap();
    vertical(&mut f, &[5.0], &[2.0], &[7.0], ErrorBarOptions::default());
    f.single_axes_mut()
        .scatter(
            nums(&[5.0]),
            &[4.0],
            crate::model::ScatterOptions::default(),
        )
        .unwrap();
    let items = items(&f);
    assert!(matches!(
        items.as_slice(),
        [
            Item::Stroke { .. },
            Item::Stroke { .. },
            Item::Markers { .. }
        ]
    ));
}
