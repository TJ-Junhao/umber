//! Time axes: time data on either axis, their limits, ticks, labels and reference marks, the
//! marks a time axis refuses, and the errors of mixing times with numbers.

use std::sync::Arc;

use crate::geometry::PathVerb;
use crate::pipeline::plan_figure;
use crate::resolve::{AxisScale, AxisTick, Resolved};
use crate::scene::{Item, Scene};
use crate::test_support::scene::assert_scene_bounded;
use crate::time::{concise_labels, iso_label, time_ticks};
use crate::{
    AreaOptions, AxisData, AxisValue, BandOptions, BarOptions, BoxOptions, BubbleOptions,
    Distribution, Error, ErrorBarOptions, ExplicitTicks, Figure, FontLibrary, Grid, GridIndex,
    HistOptions, Instant, LegendStyle, LineOptions, Placement, Rgba, RuleOptions, ScaleKind,
    ScatterOptions, Series, Share, SpanOptions, StemOptions, StepOptions, Theme, TickFormat,
    ViolinOptions, times_from_micros,
};

const LIN: ScaleKind = ScaleKind::Linear;
const TIME: ScaleKind = ScaleKind::Time;

/// The microseconds in a day.
const DAY: i64 = 86_400_000_000;

/// The instant at a civil date and time.
fn at(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Instant {
    let days = crate::time::days_from_civil(i64::from(year), i64::from(month), i64::from(day));
    let clock = (i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second)) * 1_000_000;
    Instant::from_micros(days * DAY + clock).unwrap()
}

/// Midnight of a civil date.
fn date(year: i32, month: u8, day: u8) -> Instant {
    at(year, month, day, 0, 0, 0)
}

/// `t` moved by `us` microseconds.
fn plus(t: Instant, us: i64) -> Instant {
    Instant::from_micros(t.micros() + us).unwrap()
}

/// `n + 1` instants evenly spread from `low` to `high`, both included.
fn spread(low: Instant, high: Instant, n: i64) -> Vec<Instant> {
    let span = high.micros() - low.micros();
    (0..=n).map(|k| plus(low, span / n * k)).collect()
}

/// The times as a series without gaps.
fn times(values: &[Instant]) -> AxisData {
    AxisData::Times(values.iter().copied().map(Some).collect())
}

/// `n` numbers that wobble around 10.
fn wobble(n: usize) -> AxisData {
    AxisData::Numbers((0..n).map(|k| 10.0 + (k as f64 * 0.7).sin()).collect())
}

fn figure(theme: Theme, x: ScaleKind, y: ScaleKind) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(x, y).unwrap();
    f
}

fn resolved(f: &Figure) -> Resolved {
    plan_figure(f, (0, 0), FontLibrary::global()).0
}

fn labels(ticks: &[AxisTick]) -> Vec<&str> {
    ticks.iter().map(|tick| tick.label.as_str()).collect()
}

/// The time scale of the axis, or a panic.
#[track_caller]
fn time_scale(scale: &AxisScale) -> &crate::scale::TimeScale {
    match scale {
        AxisScale::Time(scale) => scale,
        other => panic!("expected a time axis, got {other:?}"),
    }
}

/// A call on an axes, for the calls that must fail.
type Call = Box<dyn Fn(&mut crate::AxesMut<'_>) -> crate::Result<()>>;

/// The error of a call on `f` that must fail, with the figure unchanged by it.
#[track_caller]
fn refused(
    f: &mut Figure,
    call: impl FnOnce(&mut crate::AxesMut<'_>) -> crate::Result<()>,
) -> Error {
    let before = f.clone();
    let err = call(&mut f.single_axes_mut()).expect_err("the call must fail");
    assert_eq!(*f, before, "a failed call changed the figure");
    err
}

/// The kind, display text, parameter and hint of an error.
fn parts(
    err: &Error,
) -> (
    &'static str,
    String,
    Option<&'static str>,
    Option<&'static str>,
) {
    let kind = match err {
        Error::Data { .. } => "data",
        Error::Style { .. } => "style",
        Error::Usage { .. } => "usage",
        _ => "other",
    };
    (kind, err.to_string(), err.parameter(), err.hint())
}

/// The strokes of `color` in `scene`, in drawing order.
fn strokes_of(scene: &Scene, color: Rgba) -> Vec<Vec<PathVerb>> {
    fn walk(items: &[Item], color: Rgba, out: &mut Vec<Vec<PathVerb>>) {
        for item in items {
            match item {
                Item::Stroke { path, style } if style.color == color => {
                    out.push(path.verbs().to_vec());
                }
                Item::Group { items, .. } => walk(items, color, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&scene.items, color, &mut out);
    out
}

fn move_tos(verbs: &[PathVerb]) -> usize {
    verbs
        .iter()
        .filter(|v| matches!(v, PathVerb::MoveTo(_)))
        .count()
}

#[test]
fn time_line_ticks_match_time_module() {
    // Six hours of a line: the automatic ticks are the time module's ticks over the exact data
    // range, with its concise labels, and the line runs edge to edge.
    let (low, high) = (at(2026, 3, 3, 4, 40, 0), at(2026, 3, 3, 10, 40, 0));
    let x = spread(low, high, 144);
    let mut f = figure(Theme::graphite(), TIME, LIN);
    f.single_axes_mut()
        .line_shared(times(&x), wobble(x.len()), LineOptions::default())
        .unwrap();
    let r = resolved(&f);
    let scale = time_scale(&r.x.scale);
    assert_eq!(scale.normalize(low), 0.0);
    assert_eq!(scale.normalize(high), 1.0);
    let (step, ticks) = time_ticks(low, high);
    assert_eq!(labels(&r.x.ticks), concise_labels(&ticks, step));
    assert_eq!(
        labels(&r.x.ticks),
        ["Mar 3 05:00", "06:00", "07:00", "08:00", "09:00", "10:00"]
    );
    for (tick, &t) in r.x.ticks.iter().zip(&ticks) {
        assert_eq!(tick.pos.get(), scale.normalize(t));
    }
    // The y axis is numeric as before.
    assert!(matches!(r.y.scale, AxisScale::Continuous(_)));
}

#[test]
fn time_limits_follow_the_numeric_rules() {
    let (low, high) = (date(2026, 2, 24), date(2026, 3, 6));
    let x = spread(low, high, 10);
    // Points are padded by 5 % of the span on each side; the ticks stay inside the domain.
    let mut f = figure(Theme::pigment(), TIME, LIN);
    f.single_axes_mut()
        .scatter_shared(times(&x), wobble(x.len()), ScatterOptions::default())
        .unwrap();
    let r = resolved(&f);
    let scale = time_scale(&r.x.scale);
    let share = 0.05 / 1.1;
    assert!((scale.normalize(low) - share).abs() < 1e-12);
    assert!((scale.normalize(high) - (1.0 - share)).abs() < 1e-12);
    assert!(r.x.ticks.iter().all(|t| (0.0..=1.0).contains(&t.pos.get())));
    // Explicit limits replace the automatic ones and are not padded.
    let (left, right) = (date(2026, 2, 1), date(2026, 4, 1));
    f.single_axes_mut().xlim_time(left, right).unwrap();
    assert_eq!(f.single_axes().x_time_limits(), Some((left, right)));
    assert_eq!(f.single_axes().x_limits(), None);
    let r = resolved(&f);
    let scale = time_scale(&r.x.scale);
    assert_eq!((scale.normalize(left), scale.normalize(right)), (0.0, 1.0));
    let (step, ticks) = time_ticks(left, right);
    assert_eq!(labels(&r.x.ticks), concise_labels(&ticks, step));
    // Two months give weekly ticks on Mondays, the first with its month.
    assert_eq!(labels(&r.x.ticks)[..3], ["Feb 2", "9", "16"]);
}

#[test]
fn time_xlim_and_vline_accept_times() {
    let (low, high) = (date(2026, 1, 1), date(2026, 1, 31));
    let event = date(2026, 2, 10);
    let mut f = figure(Theme::pigment(), TIME, LIN);
    f.single_axes_mut()
        .line_shared(times(&[low, high]), wobble(2), LineOptions::default())
        .unwrap()
        .vline(AxisValue::Time(event), RuleOptions::default())
        .unwrap()
        .vspan_time(date(2026, 1, 10), date(2026, 1, 12), SpanOptions::default())
        .unwrap();
    // A reference line extends the automatic domain; lines keep the exact extent.
    let r = resolved(&f);
    let scale = time_scale(&r.x.scale);
    assert_eq!((scale.normalize(low), scale.normalize(event)), (0.0, 1.0));
    // With limits the line, the span and the ticks keep to them.
    f.single_axes_mut().xlim_time(low, high).unwrap();
    let r = resolved(&f);
    let scale = time_scale(&r.x.scale);
    assert_eq!(scale.normalize(high), 1.0);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    // A span the wrong way round is a style error, a number on a time axis a data error.
    let err = refused(&mut f, |a| {
        a.vspan_time(date(2026, 1, 12), date(2026, 1, 10), SpanOptions::default())
            .map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "style",
            "Axes.vspan(): low is 2026-01-12, high is 2026-01-10; low must be below high.".into(),
            Some("high"),
            Some("swap the two values.")
        )
    );
    let err = refused(&mut f, |a| {
        a.vline(AxisValue::Number(3.0), RuleOptions::default())
            .map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "data",
            "Axes.vline(): x holds a number but the x axis is a time axis; pass a time.".into(),
            Some("x"),
            Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().")
        )
    );
    let err = refused(&mut f, |a| a.xlim(0.0, 1.0).map(drop));
    assert_eq!(
        parts(&err).1,
        "Axes.xlim(): this is a time axis; its limits must be times."
    );
    let err = refused(&mut f, |a| a.xlim_time(high, low).map(drop));
    assert_eq!(
        parts(&err),
        (
            "style",
            "Axes.xlim(): the limits are 2026-01-31 and 2026-01-01; the lower limit must be below the upper limit.".into(),
            None,
            None
        )
    );
    // Times on a numeric axis are refused as well.
    let mut numeric = figure(Theme::pigment(), LIN, LIN);
    let err = refused(&mut numeric, |a| a.xlim_time(low, high).map(drop));
    assert_eq!(
        parts(&err).1,
        "Axes.xlim(): this is a linear or log axis; its limits must be numbers."
    );
    let err = refused(&mut numeric, |a| {
        a.vline(AxisValue::Time(event), RuleOptions::default())
            .map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "data",
            "Axes.vline(): x holds a time but the x axis is not a time axis; pass a number.".into(),
            Some("x"),
            Some("pass x=Scale.time() to Figure.add_axes().")
        )
    );
}

#[test]
fn nat_leaves_gap() {
    let x = spread(date(2026, 1, 1), date(2026, 1, 9), 8);
    let mut with_gap: Vec<Option<Instant>> = x.iter().copied().map(Some).collect();
    with_gap[4] = None;
    let f = {
        let mut f = figure(Theme::pigment(), TIME, LIN);
        f.single_axes_mut()
            .line_shared(AxisData::Times(with_gap), wobble(9), LineOptions::default())
            .unwrap();
        f
    };
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let color = f.theme().palette()[0];
    let lines = strokes_of(&scene, color);
    assert_eq!(lines.len(), 1);
    assert_eq!(move_tos(&lines[0]), 2, "the gap splits the line in two");
    // All NaT: nothing to draw and the fallback range, without an error.
    let mut empty = figure(Theme::pigment(), TIME, LIN);
    empty
        .single_axes_mut()
        .line_shared(
            AxisData::Times(vec![None; 3]),
            wobble(3),
            LineOptions::default(),
        )
        .unwrap();
    let scene = empty.to_scene(FontLibrary::global()).unwrap();
    assert!(strokes_of(&scene, color).is_empty());
    let r = resolved(&empty);
    assert_eq!(time_scale(&r.x.scale).normalize(date(1970, 1, 1)), 0.0);
    assert_eq!(time_scale(&r.x.scale).normalize(date(1970, 1, 2)), 1.0);
}

#[test]
fn time_y_axis() {
    let y = spread(date(2025, 11, 12), date(2026, 3, 12), 4);
    let mut f = figure(Theme::midnight(), LIN, TIME);
    f.single_axes_mut()
        .line_shared(
            AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0, 4.0]),
            times(&y),
            LineOptions::default(),
        )
        .unwrap()
        .hline_time(date(2026, 1, 1), RuleOptions::default())
        .unwrap()
        .hspan_time(date(2026, 1, 1), date(2026, 2, 1), SpanOptions::default())
        .unwrap()
        .x_error_bars_shared(
            times(&y[..2]),
            Arc::from([0.5, 1.5]),
            Arc::from([1.0, 2.0]),
            ErrorBarOptions::default(),
        )
        .unwrap();
    let r = resolved(&f);
    let scale = time_scale(&r.y.scale);
    // y is the value axis here; its data is padded as a numeric value axis is.
    assert!(scale.normalize(y[0]) > 0.0 && scale.normalize(y[4]) < 1.0);
    assert_eq!(labels(&r.y.ticks), ["Dec 2025", "Jan 2026", "Feb", "Mar"]);
    // Explicit y limits and ticks take times.
    f.single_axes_mut()
        .ylim_time(date(2025, 11, 1), date(2026, 4, 1))
        .unwrap()
        .yticks_time(&[date(2025, 12, 1), date(2026, 2, 1)])
        .unwrap();
    let r = resolved(&f);
    assert_eq!(labels(&r.y.ticks), ["Dec 2025", "Feb 2026"]);
    assert_eq!(
        f.single_axes().y_ticks(),
        Some(&ExplicitTicks::TimePositions(vec![
            date(2025, 12, 1),
            date(2026, 2, 1)
        ]))
    );
    assert_scene_bounded(&f.to_scene(FontLibrary::global()).unwrap());
    // Marks whose values are numbers on y refuse a time y axis.
    let err = refused(&mut f, |a| {
        a.y_error_bars(
            AxisData::Numbers(vec![0.0]),
            &[1.0],
            &[2.0],
            ErrorBarOptions::default(),
        )
        .map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "usage",
            "Axes.y_error_bars(): y is a time scale; Axes.y_error_bars() needs a numeric y axis."
                .into(),
            Some("y"),
            Some("use a linear or log y axis.")
        )
    );
    let err = refused(&mut f, |a| a.hline(1.0, RuleOptions::default()).map(drop));
    assert_eq!(parts(&err).0, "usage");
}

#[test]
fn bar_on_time_axis_rejected() {
    let names = || vec!["a".to_owned(), "b".to_owned()];
    let hint = Some("use a category axis with formatted labels.");
    let mut f = figure(Theme::pigment(), TIME, LIN);
    let series = |label: &str| Series {
        label: label.into(),
        values: vec![1.0, 2.0],
        ..Series::default()
    };
    let distribution = || Distribution {
        label: "d".into(),
        groups: vec![vec![1.0], vec![2.0]],
        ..Distribution::default()
    };
    let calls: Vec<(&str, Call)> = vec![
        (
            "Axes.bar()",
            Box::new(move |a| a.bar(names(), &[1.0, 2.0], BarOptions::default()).map(drop)),
        ),
        (
            "Axes.grouped_bar()",
            Box::new(move |a| a.grouped_bar(names(), vec![series("s")]).map(drop)),
        ),
        (
            "Axes.stacked_bar()",
            Box::new(move |a| a.stacked_bar(names(), vec![series("s")]).map(drop)),
        ),
        (
            "Axes.box()",
            Box::new(move |a| {
                a.box_plot(names(), vec![vec![1.0], vec![2.0]], BoxOptions::default())
                    .map(drop)
            }),
        ),
        (
            "Axes.grouped_box()",
            Box::new(move |a| a.grouped_box_plot(names(), vec![distribution()]).map(drop)),
        ),
        (
            "Axes.violin()",
            Box::new(move |a| {
                a.violin(
                    names(),
                    vec![vec![1.0], vec![2.0]],
                    ViolinOptions::default(),
                )
                .map(drop)
            }),
        ),
        (
            "Axes.grouped_violin()",
            Box::new(move |a| a.grouped_violin(names(), vec![distribution()]).map(drop)),
        ),
    ];
    for (context, call) in &calls {
        let err = refused(&mut f, |a| call(a));
        assert_eq!(
            parts(&err),
            (
                "usage",
                format!("{context}: x is a time scale; {context} needs a category x axis."),
                Some("x"),
                hint
            )
        );
    }
    let err = refused(&mut f, |a| {
        a.hist(&[1.0, 2.0], HistOptions::default()).map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "usage",
            "Axes.hist(): x is a time scale; Axes.hist() needs a linear x axis.".into(),
            Some("x"),
            hint
        )
    );
    let err = refused(&mut f, |a| {
        a.bubbles(&[1.0], &[1.0], &[1.0], BubbleOptions::default())
            .map(drop)
    });
    assert_eq!(
        parts(&err).1,
        "Axes.bubbles(): x is a time scale; Axes.bubbles() needs a linear or log x axis."
    );
    // On a time y axis the same marks need a numeric y axis.
    let mut f = figure(Theme::pigment(), ScaleKind::Category, TIME);
    let err = refused(&mut f, |a| {
        a.bar(names(), &[1.0, 2.0], BarOptions::default()).map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "usage",
            "Axes.bar(): y is a time scale; Axes.bar() needs a numeric y axis.".into(),
            Some("y"),
            Some("use a linear or log y axis.")
        )
    );
}

#[test]
fn numbers_and_times_do_not_mix() {
    let mut f = figure(Theme::pigment(), TIME, LIN);
    let err = refused(&mut f, |a| {
        a.line(AxisData::Numbers(vec![0.0]), &[1.0], LineOptions::default())
            .map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "data",
            "Axes.line(): x holds numbers but the x axis is a time axis; pass times.".into(),
            None,
            Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().")
        )
    );
    let err = refused(&mut f, |a| {
        a.scatter(
            AxisData::Categories(vec!["a".into()]),
            &[1.0],
            ScatterOptions::default(),
        )
        .map(drop)
    });
    assert_eq!(
        parts(&err).1,
        "Axes.scatter(): x holds category names but the x axis is a time axis; pass times."
    );
    let err = refused(&mut f, |a| a.xticks(&[1.0]).map(drop));
    assert_eq!(
        parts(&err).1,
        "Axes.xticks(): this is a time axis; its tick positions must be times."
    );
    let err = refused(&mut f, |a| a.xtick_format(TickFormat::fixed(1)?).map(drop));
    assert_eq!(
        parts(&err),
        (
            "usage",
            "Axes.xtick_format(): this is a time axis; number formats label linear and log axes only.".into(),
            Some("format"),
            Some("use TickFormat.AUTO or TickFormat.iso() on a time axis.")
        )
    );
    // Times on a linear axis.
    let mut numeric = figure(Theme::pigment(), LIN, LIN);
    let err = refused(&mut numeric, |a| {
        a.line_shared(
            AxisData::Numbers(vec![1.0]),
            times(&[date(2026, 1, 1)]),
            LineOptions::default(),
        )
        .map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "data",
            "Axes.line(): y holds times but the y axis is not a time axis; pass numbers.".into(),
            None,
            Some("pass y=Scale.time() to Figure.add_axes().")
        )
    );
    let err = refused(&mut numeric, |a| {
        a.xticks_time(&[date(2026, 1, 1)]).map(drop)
    });
    assert_eq!(
        parts(&err).1,
        "Axes.xticks(): this is a linear or log axis; its tick positions must be numbers."
    );
    // Tick positions must increase.
    let err = refused(&mut f, |a| {
        a.xticks_time(&[date(2026, 1, 2), date(2026, 1, 1)])
            .map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "style",
            "Axes.xticks(): positions[1] is 2026-01-01 but positions[0] is 2026-01-02; tick positions must be strictly increasing.".into(),
            Some("positions"),
            Some("sort the positions and remove repeats; they are used in the order given.")
        )
    );
}

#[test]
fn times_from_micros_names_the_first_out_of_range_value() {
    let max = Instant::MAX.micros();
    let got = times_from_micros("Axes.line()", "x", &[0, i64::MIN, max]).unwrap();
    assert_eq!(
        got,
        [
            Some(Instant::from_micros(0).unwrap()),
            None,
            Some(Instant::MAX)
        ]
    );
    let err = times_from_micros("Axes.line()", "x", &[0, 1, 2, max + 1]).unwrap_err();
    assert_eq!(
        parts(&err),
        (
            "data",
            "Axes.line(): x[3] is 10000-01-01; times must lie between 0001-01-01 and 9999-12-31."
                .into(),
            Some("x"),
            None
        )
    );
    let err = times_from_micros("Axes.xlim()", "low", &[Instant::MIN.micros() - 1]).unwrap_err();
    assert_eq!(
        parts(&err).1,
        "Axes.xlim(): low[0] is 0000-12-31; times must lie between 0001-01-01 and 9999-12-31."
    );
}

#[test]
fn time_and_numeric_axes_cannot_share() {
    let grid = Grid::new(1, 2).unwrap().with_share_x(Share::All);
    let mut f = Figure::with_grid(8.0, 4.0, Theme::pigment(), grid).unwrap();
    let at = |col| Placement {
        row: GridIndex::At(0),
        col: GridIndex::At(col),
        ..Placement::default()
    };
    f.add_axes_at(at(0), LIN, LIN).unwrap();
    let before = f.clone();
    let err = f.add_axes_at(at(1), TIME, LIN).map(drop).unwrap_err();
    assert_eq!(f, before);
    assert_eq!(
        err.to_string(),
        "Figure.add_axes(): x is Scale.time(); the axes at row 0, col 0 shares its x axis and has Scale.linear()."
    );
    // Shared time axes share their limits.
    let grid = Grid::new(1, 2).unwrap().with_share_x(Share::All);
    let mut f = Figure::with_grid(8.0, 4.0, Theme::pigment(), grid).unwrap();
    f.add_axes_at(at(0), TIME, LIN)
        .unwrap()
        .xlim_time(date(2026, 1, 1), date(2026, 2, 1))
        .unwrap();
    let mut right = f.add_axes_at(at(1), TIME, LIN).unwrap();
    assert_eq!(
        right.x_time_limits(),
        Some((date(2026, 1, 1), date(2026, 2, 1)))
    );
    let err = right
        .xlim_time(date(2026, 1, 1), date(2026, 3, 1))
        .map(drop)
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Axes.xlim(): x limits are shared with row 0, col 0, which set (2026-01-01, 2026-02-01)."
    );
}

#[test]
fn explicit_time_ticks_take_labels_of_their_unit() {
    let x = spread(date(2025, 12, 15), date(2026, 3, 15), 6);
    let mut f = figure(Theme::pigment(), TIME, LIN);
    f.single_axes_mut()
        .line_shared(times(&x), wobble(x.len()), LineOptions::default())
        .unwrap()
        .xticks_time(&[date(2026, 1, 1), date(2026, 2, 1), date(2026, 3, 1)])
        .unwrap();
    assert_eq!(labels(&resolved(&f).x.ticks), ["Jan 2026", "Feb", "Mar"]);
    f.single_axes_mut().xtick_format(TickFormat::iso()).unwrap();
    assert_eq!(
        labels(&resolved(&f).x.ticks),
        ["2026-01", "2026-02", "2026-03"]
    );
    // Clock times label with their clock; ISO labels keep a prefix and a suffix.
    f.single_axes_mut()
        .xtick_format(TickFormat::iso().affix("", " UTC").unwrap())
        .unwrap()
        .xticks_time(&[at(2026, 1, 1, 6, 30, 0), at(2026, 2, 1, 12, 0, 0)])
        .unwrap();
    assert_eq!(
        labels(&resolved(&f).x.ticks),
        ["2026-01-01T06:30 UTC", "2026-02-01T12:00 UTC"]
    );
    // Labels given with their positions are drawn as given.
    f.single_axes_mut()
        .xtick_format(TickFormat::AUTO)
        .unwrap()
        .xticks_labeled_time(&[date(2026, 1, 1)], vec!["launch".into()])
        .unwrap();
    assert_eq!(labels(&resolved(&f).x.ticks), ["launch"]);
    assert_eq!(
        f.single_axes().x_ticks(),
        Some(&ExplicitTicks::TimeLabeled(vec![(
            date(2026, 1, 1),
            "launch".to_owned()
        )]))
    );
}

#[test]
fn automatic_iso_labels_follow_the_step() {
    let (low, high) = (date(2025, 11, 12), date(2026, 3, 12));
    let mut f = figure(Theme::pigment(), TIME, LIN);
    f.single_axes_mut()
        .line_shared(times(&[low, high]), wobble(2), LineOptions::default())
        .unwrap()
        .xtick_format(TickFormat::iso())
        .unwrap();
    let (step, ticks) = time_ticks(low, high);
    let expected: Vec<String> = ticks.iter().map(|&t| iso_label(t, step)).collect();
    assert_eq!(labels(&resolved(&f).x.ticks), expected);
    assert_eq!(expected, ["2025-12", "2026-01", "2026-02", "2026-03"]);
}

#[test]
fn awkward_times_render() {
    let cases: Vec<(&str, Vec<Option<Instant>>)> = vec![
        ("single point", vec![Some(date(2026, 1, 1))]),
        ("all missing", vec![None, None]),
        ("duplicates", vec![Some(date(2026, 1, 1)); 4]),
        (
            "one microsecond",
            vec![Some(date(2026, 1, 1)), Some(plus(date(2026, 1, 1), 1))],
        ),
        (
            "500 years",
            vec![Some(date(1600, 1, 1)), Some(date(2100, 1, 1))],
        ),
        ("year 1", vec![Some(Instant::MIN), Some(date(1, 3, 1))]),
        (
            "year 9999",
            vec![Some(date(9999, 10, 1)), Some(Instant::MAX)],
        ),
        ("whole range", vec![Some(Instant::MIN), Some(Instant::MAX)]),
    ];
    for (name, x) in cases {
        for scatter in [false, true] {
            let mut f = figure(Theme::graphite(), TIME, LIN);
            let n = x.len();
            let y = wobble(n);
            let mut axes = f.single_axes_mut();
            if scatter {
                axes.scatter_shared(AxisData::Times(x.clone()), y, ScatterOptions::default())
            } else {
                axes.line_shared(AxisData::Times(x.clone()), y, LineOptions::default())
            }
            .unwrap();
            let scene = f
                .to_scene(FontLibrary::global())
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_scene_bounded(&scene);
            let r = resolved(&f);
            assert!(!r.x.ticks.is_empty(), "{name}: no ticks");
            let positions: Vec<f64> = r.x.ticks.iter().map(|t| t.pos.get()).collect();
            assert!(
                positions.windows(2).all(|w| w[0] < w[1]),
                "{name}: {positions:?}"
            );
            assert!(
                positions.iter().all(|p| (0.0..=1.0).contains(p)),
                "{name}: {positions:?}"
            );
        }
    }
}

#[test]
fn long_time_lines_are_downsampled() {
    let n = 200_000;
    let low = date(2026, 1, 1);
    let x: Vec<Instant> = (0..n).map(|k| plus(low, k * 1_000_000)).collect();
    let y = AxisData::Numbers((0..n).map(|k| (k as f64 * 0.01).sin()).collect());
    let mut f = figure(Theme::pigment(), TIME, LIN);
    f.single_axes_mut()
        .line_shared(times(&x), y, LineOptions::default())
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let lines = strokes_of(&scene, f.theme().palette()[0]);
    assert_eq!(lines.len(), 1);
    // At most four points per device column of a plot narrower than the 6 in figure.
    let columns = (6.0_f64 * 300.0).ceil() as usize;
    assert!(lines[0].len() <= 4 * columns, "{}", lines[0].len());
}

#[test]
fn bands_steps_areas_stems_and_stacked_areas_take_time_x() {
    let x = spread(date(2026, 1, 1), date(2026, 1, 11), 10);
    let mut gapped: Vec<Option<Instant>> = x.iter().copied().map(Some).collect();
    gapped[3] = None;
    let n = x.len();
    let values: Vec<f64> = (0..n).map(|k| 2.0 + k as f64 * 0.1).collect();
    let mut f = figure(Theme::midnight(), TIME, LIN);
    f.single_axes_mut()
        .band_time(
            Arc::from(gapped.clone()),
            Arc::from(vec![1.0; n]),
            Arc::from(vec![3.0; n]),
            BandOptions::default(),
        )
        .unwrap()
        .step_shared(times(&x), wobble(n), StepOptions::default())
        .unwrap()
        .area_shared(times(&x), wobble(n), AreaOptions::default())
        .unwrap()
        .stem_shared(times(&x), wobble(n), StemOptions::default())
        .unwrap()
        .stacked_area_time(
            Arc::from(gapped.clone()),
            vec![Series {
                label: "a".into(),
                values: values.clone(),
                ..Series::default()
            }],
        )
        .unwrap()
        .y_error_bars(
            times(&x[..2]),
            &[1.0, 2.0],
            &[2.0, 3.0],
            ErrorBarOptions::default(),
        )
        .unwrap();
    assert_scene_bounded(&f.to_scene(FontLibrary::global()).unwrap());
    let r = resolved(&f);
    let scale = time_scale(&r.x.scale);
    // Every mark is drawn along a line or is a band, except the stems and error bars: padded.
    assert!(scale.normalize(x[0]) > 0.0);
    // Unsorted times are refused by the marks that need sorted x.
    let mut unsorted: Vec<Option<Instant>> = x.iter().copied().map(Some).collect();
    unsorted.swap(1, 2);
    let err = refused(&mut f, |a| {
        a.band_time(
            Arc::from(unsorted.clone()),
            Arc::from(vec![1.0; n]),
            Arc::from(vec![3.0; n]),
            BandOptions::default(),
        )
        .map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "data",
            "Axes.band(): x[2] is 2026-01-02, below x[1] = 2026-01-03; band needs x in non-decreasing order.".into(),
            Some("x"),
            Some("sort the points by x first.")
        )
    );
    let err = refused(&mut f, |a| {
        a.step_shared(
            AxisData::Times(unsorted.clone()),
            wobble(n),
            StepOptions::default(),
        )
        .map(drop)
    });
    assert_eq!(
        parts(&err).1,
        "Axes.step(): x[2] is 2026-01-02, below x[1] = 2026-01-03; step needs x in non-decreasing order."
    );
    // A band and a stacked area need numbers along y.
    let mut timed_y = figure(Theme::pigment(), LIN, TIME);
    let err = refused(&mut timed_y, |a| {
        a.stacked_area(&[0.0], vec![Series::default()]).map(drop)
    });
    assert_eq!(
        parts(&err).1,
        "Axes.stacked_area(): y is a time scale; Axes.stacked_area() needs a numeric y axis."
    );
}

#[test]
fn time_lines_take_direct_labels() {
    let x = spread(date(2026, 1, 1), date(2026, 1, 11), 10);
    let mut f = figure(Theme::graphite(), TIME, LIN);
    assert_eq!(f.theme().legend_style(), LegendStyle::Direct);
    f.single_axes_mut()
        .line_shared(
            times(&x),
            wobble(x.len()),
            LineOptions {
                label: "Load".into(),
                ..LineOptions::default()
            },
        )
        .unwrap()
        .xlim_time(date(2026, 1, 1), at(2026, 1, 6, 12, 0, 0))
        .unwrap();
    let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    assert_eq!(layout.direct.len(), 1);
    // The line leaves the plot at the upper limit, so its label has no end dot.
    assert!(layout.direct[0].dot.is_none());
    // A last point on the upper limit ends the line there, with its dot.
    f.single_axes_mut()
        .xlim_time(date(2026, 1, 1), date(2026, 1, 6))
        .unwrap();
    let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    assert!(layout.direct[0].dot.is_some());
}

#[test]
fn failed_time_calls_change_nothing() {
    let mut f = figure(Theme::pigment(), TIME, LIN);
    let x = spread(date(2026, 1, 1), date(2026, 1, 3), 2);
    f.single_axes_mut()
        .line_shared(times(&x), wobble(3), LineOptions::default())
        .unwrap()
        .xlim_time(date(2026, 1, 1), date(2026, 1, 3))
        .unwrap();
    let svg = f.to_svg(crate::SvgText::Outlines).unwrap();
    let calls: Vec<Call> = vec![
        Box::new(|a| a.xlim_time(date(2026, 1, 3), date(2026, 1, 1)).map(drop)),
        Box::new(|a| {
            a.xticks_time(&[date(2026, 1, 2), date(2026, 1, 2)])
                .map(drop)
        }),
        Box::new(|a| a.xtick_format(TickFormat::scientific()).map(drop)),
        Box::new(|a| {
            a.bar(vec!["a".into()], &[1.0], BarOptions::default())
                .map(drop)
        }),
        Box::new(|a| {
            a.line(AxisData::Numbers(vec![1.0]), &[1.0], LineOptions::default())
                .map(drop)
        }),
        Box::new(|a| {
            a.vspan_time(date(2026, 1, 2), date(2026, 1, 1), SpanOptions::default())
                .map(drop)
        }),
    ];
    for call in &calls {
        refused(&mut f, |a| call(a));
    }
    assert_eq!(f.to_svg(crate::SvgText::Outlines).unwrap(), svg);
}

#[test]
fn a_step_label_at_a_midpoint_keeps_the_midpoint() {
    // A step that changes level halfway between two times one microsecond apart ends there:
    // the midpoint lies half a microsecond after the first time, in the middle of the plot.
    let t = date(2026, 1, 1);
    let mut f = figure(Theme::graphite(), TIME, LIN);
    f.single_axes_mut()
        .step_shared(
            times(&[t, plus(t, 1)]),
            AxisData::Numbers(vec![1.0, f64::NAN]),
            StepOptions {
                position: crate::Step::Mid,
                label: "series".into(),
                ..StepOptions::default()
            },
        )
        .unwrap();
    let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    let dot = layout.direct[0].dot.expect("the step ends inside the plot");
    let middle = layout.plot.x + layout.plot.width / 2.0;
    assert!((dot.x - middle).abs() < 1e-6, "{} vs {middle}", dot.x);
}

#[test]
fn explicit_time_ticks_show_the_larger_units_that_change() {
    let (a, b) = (date(2025, 1, 2), date(2026, 1, 2));
    for y in [false, true] {
        let mut f = if y {
            figure(Theme::pigment(), LIN, TIME)
        } else {
            figure(Theme::pigment(), TIME, LIN)
        };
        let mut axes = f.single_axes_mut();
        if y {
            axes.yticks_time(&[a, b]).unwrap();
        } else {
            axes.xticks_time(&[a, b]).unwrap();
        }
        let r = resolved(&f);
        let ticks = if y { &r.y.ticks } else { &r.x.ticks };
        assert_eq!(labels(ticks), ["Jan 2 2025", "Jan 2 2026"]);
    }
    // Within one year the concise labels need no year after the first tick.
    let mut f = figure(Theme::pigment(), TIME, LIN);
    f.single_axes_mut()
        .xticks_time(&[date(2026, 1, 2), date(2026, 1, 9), date(2026, 2, 2)])
        .unwrap();
    assert_eq!(labels(&resolved(&f).x.ticks), ["Jan 2", "9", "Feb 2"]);
    // Clock ticks across days show the date where it changes.
    f.single_axes_mut()
        .xticks_time(&[at(2026, 1, 1, 22, 0, 0), at(2026, 1, 2, 2, 0, 0)])
        .unwrap();
    assert_eq!(
        labels(&resolved(&f).x.ticks),
        ["Jan 1 22:00", "Jan 2 02:00"]
    );
}

#[test]
fn the_ticks_of_exact_limits_reach_both_limits() {
    let origin = date(2026, 1, 1);
    let (low, high) = (plus(origin, 1), plus(origin, 250));
    let mut f = Figure::new(20.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(TIME, LIN).unwrap().xlim_time(low, high).unwrap();
    let r = resolved(&f);
    let (step, ticks) = time_ticks(low, high);
    assert_eq!(labels(&r.x.ticks), concise_labels(&ticks, step));
    assert_eq!(ticks.last(), Some(&high));
    assert_eq!(r.x.ticks.last().map(|t| t.pos.get()), Some(1.0));
}

#[test]
fn error_bars_take_time_limits_on_a_time_value_axis() {
    let (a, b, c) = (date(2026, 1, 1), date(2026, 1, 3), date(2026, 1, 9));
    // Vertical bars between times on a time y axis extend its domain to their limits.
    let mut f = figure(Theme::pigment(), LIN, TIME);
    f.single_axes_mut()
        .y_error_bars_time(
            AxisData::Numbers(vec![1.0, 2.0, 3.0]),
            Arc::from([Some(a), None, Some(b)]),
            Arc::from([Some(b), Some(c), Some(c)]),
            ErrorBarOptions::default(),
        )
        .unwrap();
    let r = resolved(&f);
    let scale = time_scale(&r.y.scale);
    let share = 0.05 / 1.1;
    assert!((scale.normalize(a) - share).abs() < 1e-12);
    assert!((scale.normalize(c) - (1.0 - share)).abs() < 1e-12);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let bars = strokes_of(&scene, f.theme().palette()[0]);
    // The bar with a missing limit is skipped: two stems with their caps.
    assert_eq!(bars.len(), 1);
    assert_eq!(move_tos(&bars[0]), 2 * 3);
    // Horizontal bars between times on a time x axis.
    let mut f = figure(Theme::pigment(), TIME, LIN);
    f.single_axes_mut()
        .x_error_bars_time(
            AxisData::Numbers(vec![1.0]),
            Arc::from([Some(a)]),
            Arc::from([Some(c)]),
            ErrorBarOptions::default(),
        )
        .unwrap();
    let r = resolved(&f);
    assert!(time_scale(&r.x.scale).normalize(c) < 1.0);
    // Limits in the wrong order, of the wrong count, or on a numeric value axis are refused.
    let err = refused(&mut f, |ax| {
        ax.x_error_bars_time(
            AxisData::Numbers(vec![1.0]),
            Arc::from([Some(c)]),
            Arc::from([Some(b)]),
            ErrorBarOptions::default(),
        )
        .map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "data",
            "Axes.x_error_bars(): low[0] is 2026-01-09, above high[0] = 2026-01-03; low must not exceed high.".into(),
            Some("low"),
            Some("check that low and high are not swapped.")
        )
    );
    let err = refused(&mut f, |ax| {
        ax.x_error_bars_time(
            AxisData::Numbers(vec![1.0, 2.0]),
            Arc::from([Some(a)]),
            Arc::from([Some(b)]),
            ErrorBarOptions::default(),
        )
        .map(drop)
    });
    assert_eq!(
        parts(&err).1,
        "Axes.x_error_bars(): y has 2 values but low has 1; y, low and high must have the same length."
    );
    let err = refused(&mut f, |ax| {
        ax.y_error_bars_time(
            AxisData::Times(vec![Some(a)]),
            Arc::from([Some(a)]),
            Arc::from([Some(b)]),
            ErrorBarOptions::default(),
        )
        .map(drop)
    });
    assert_eq!(
        parts(&err),
        (
            "usage",
            "Axes.y_error_bars(): the y axis is not a time axis; error bars between times need one.".into(),
            None,
            Some("pass y=Scale.time() to Figure.add_axes().")
        )
    );
}
