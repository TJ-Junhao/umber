//! Output-gate cases for integer ticks on integer data and for limits fixed at one end.
//!
//! The cases write only new output files. See `scripts/check-refactor-output.sh`.
//!
//! Scenes: a line over the years 2015 to 2026 in each theme, whose x ticks take whole steps;
//! counts as bars, whose y ticks take whole steps; integer data from 0 to 1 with integer ticks;
//! a line whose y axis starts at 0 with an automatic top, in each theme; a scatter with only its
//! upper x limit fixed; a log y axis with only its lower limit fixed; bars with only their upper
//! limit fixed; and a time x axis with only its start fixed.
//!
//! In `errors/axis-limits.txt`: the errors of a limit fixed at one end, when it is set and when
//! the figure renders, and the state a failed call leaves.

use std::fmt::Write as _;

use umber_core::figure::{AxisData, BarOptions, LineOptions, ScatterOptions};
use umber_core::{
    Figure, FontLibrary, Grid, GridIndex, Instant, LimitEnd, Placement, ScaleKind, Share, Theme,
};

use super::super::{Output, axes, emit_scene, figure, linear, log, numbers, record, themes};

/// The microseconds in a day.
const DAY: i64 = 86_400_000_000;

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib).unwrap(), sink);
    }
    sink(Output {
        kind: "errors",
        file: "axis-limits.txt".into(),
        bytes: errors(lib).into_bytes(),
    });
}

/// Every figure of this area, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();
    let years: Vec<f64> = (2015..=2026).map(f64::from).collect();
    let rates = [3.1, 3.4, 3.0, 3.8, 4.2, 2.9, 3.6, 4.4, 4.1, 4.8, 5.0, 4.7];
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 3.6, theme, linear(), linear());
        axes(&mut f)
            .title("Integer years take whole steps")
            .ylabel("Rate (%)")
            .line(numbers(years.clone()), &rates, LineOptions::default())
            .unwrap();
        out.push((format!("integer-ticks-years-{theme_name}"), f));
    }

    let mut counts = figure(5.0, 3.6, Theme::pigment(), ScaleKind::Category, linear());
    axes(&mut counts)
        .title("Counts take whole steps")
        .bar(
            ["Mon", "Tue", "Wed", "Thu"].map(String::from).to_vec(),
            &[1.0, 3.0, 2.0, 3.0],
            BarOptions::default(),
        )
        .unwrap();
    out.push(("integer-ticks-counts".into(), counts));

    let mut binary = figure(5.0, 3.6, Theme::pigment(), linear(), linear());
    axes(&mut binary)
        .title("Integer data from 0 to 1")
        .scatter(
            numbers(vec![0.0, 1.0, 2.0, 3.0, 4.0]),
            &[0.0, 1.0, 1.0, 0.0, 1.0],
            ScatterOptions::default(),
        )
        .unwrap();
    out.push(("integer-ticks-zero-one".into(), binary));

    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 3.6, theme, linear(), linear());
        axes(&mut f)
            .title("The y axis starts at 0")
            .line(
                numbers(vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5]),
                &[41.5, 44.0, 43.2, 47.8, 46.1, 49.3],
                LineOptions::default(),
            )
            .unwrap()
            .ylim_end(LimitEnd::Low, 0.0)
            .unwrap();
        out.push((format!("limit-end-low-{theme_name}"), f));
    }

    let mut high_x = figure(5.0, 3.6, Theme::graphite(), linear(), linear());
    axes(&mut high_x)
        .title("Only the upper x limit is fixed")
        .scatter(
            numbers(vec![0.4, 1.3, 2.2, 3.7, 5.1, 6.6]),
            &[2.0, 2.6, 2.1, 3.3, 3.9, 3.4],
            ScatterOptions::default(),
        )
        .unwrap()
        .xlim_end(LimitEnd::High, 5.0)
        .unwrap();
    out.push(("limit-end-high-x".into(), high_x));

    let mut log_low = figure(5.0, 3.6, Theme::midnight(), linear(), log(10.0));
    axes(&mut log_low)
        .title("A log y axis from 1")
        .line(
            numbers(vec![0.0, 1.0, 2.0, 3.0, 4.0]),
            &[12.0, 140.0, 900.0, 4_100.0, 23_000.0],
            LineOptions::default(),
        )
        .unwrap()
        .ylim_end(LimitEnd::Low, 1.0)
        .unwrap();
    out.push(("limit-end-log-low".into(), log_low));

    let mut bars = figure(5.0, 3.6, Theme::pigment(), ScaleKind::Category, linear());
    axes(&mut bars)
        .title("Bars under a fixed top")
        .bar(
            ["A", "B", "C"].map(String::from).to_vec(),
            &[12.5, -4.0, 7.25],
            BarOptions::default(),
        )
        .unwrap()
        .ylim_end(LimitEnd::High, 20.0)
        .unwrap();
    out.push(("limit-end-bars-high".into(), bars));

    let mut time = figure(6.0, 3.6, Theme::graphite(), ScaleKind::Time, linear());
    let start = Instant::from_micros(20_454 * DAY).unwrap();
    let days: Vec<Option<Instant>> = (3..=9)
        .map(|k| Some(Instant::from_micros((20_454 + k) * DAY).unwrap()))
        .collect();
    axes(&mut time)
        .title("A time axis from a fixed start")
        .line_shared(
            AxisData::Times(days),
            numbers(vec![5.0, 6.5, 6.0, 7.5, 7.0, 8.5, 9.0]),
            LineOptions::default(),
        )
        .unwrap()
        .xlim_time_end(LimitEnd::Low, start)
        .unwrap();
    out.push(("limit-end-time-low".into(), time));
    out
}

/// The errors of a limit fixed at one end, each with its parameter and hint and whether the
/// failed call left the figure unchanged.
fn errors(lib: &FontLibrary) -> String {
    let mut out = String::new();
    let mut note = |label: &str, figure: &Figure, before: &Figure, err: &umber_core::Error| {
        let _ = writeln!(
            out,
            "{label}: {}\nparameter={:?} hint={:?}\nstate unchanged={}",
            record::error(err),
            err.parameter(),
            err.hint(),
            figure == before,
        );
    };

    let calls: [(&str, ScaleKind, ScaleKind, LimitEnd, f64); 5] = [
        (
            "infinite low",
            linear(),
            linear(),
            LimitEnd::Low,
            f64::INFINITY,
        ),
        ("NaN high", linear(), linear(), LimitEnd::High, f64::NAN),
        ("huge low", linear(), linear(), LimitEnd::Low, 1e301),
        ("zero on log", linear(), log(10.0), LimitEnd::Low, 0.0),
        ("tiny on log", linear(), log(10.0), LimitEnd::High, 1e-301),
    ];
    for (label, x, y, end, value) in calls {
        let mut f = figure(4.0, 3.0, Theme::pigment(), x, y);
        let before = f.clone();
        let err = axes(&mut f).ylim_end(end, value).unwrap_err();
        note(label, &f, &before, &err);
    }

    let mut category = figure(4.0, 3.0, Theme::pigment(), ScaleKind::Category, linear());
    let before = category.clone();
    let err = axes(&mut category)
        .xlim_end(LimitEnd::Low, 0.0)
        .unwrap_err();
    note("category x", &category, &before, &err);

    let mut time = figure(4.0, 3.0, Theme::pigment(), ScaleKind::Time, linear());
    let before = time.clone();
    let err = axes(&mut time).xlim_end(LimitEnd::Low, 0.0).unwrap_err();
    note("number on a time axis", &time, &before, &err);
    let mut numeric = figure(4.0, 3.0, Theme::pigment(), linear(), linear());
    let before = numeric.clone();
    let err = axes(&mut numeric)
        .xlim_time_end(LimitEnd::Low, Instant::from_micros(0).unwrap())
        .unwrap_err();
    note("time on a linear axis", &numeric, &before, &err);

    let grid = Grid::new(1, 2).unwrap().with_share_x(Share::All);
    let mut shared = Figure::with_grid(6.0, 3.0, Theme::pigment(), grid).unwrap();
    let at = |col| Placement {
        row: GridIndex::At(0),
        col: GridIndex::At(col),
        ..Placement::default()
    };
    shared
        .add_axes_at(at(0), linear(), linear())
        .unwrap()
        .xlim_end(LimitEnd::Low, 0.0)
        .unwrap();
    shared.add_axes_at(at(1), linear(), linear()).unwrap();
    let before = shared.clone();
    let err = shared
        .axes_at_mut(GridIndex::At(0), GridIndex::At(1))
        .unwrap()
        .xlim_end(LimitEnd::High, 9.0)
        .unwrap_err();
    note("shared x", &shared, &before, &err);

    let mut past = figure(4.0, 3.0, Theme::pigment(), linear(), linear());
    axes(&mut past)
        .line(numbers(vec![0.0, 1.0]), &[2.0, 3.0], LineOptions::default())
        .unwrap()
        .ylim_end(LimitEnd::Low, 10.0)
        .unwrap();
    let err = past.to_scene(lib).unwrap_err();
    note("data below a fixed low", &past, &past, &err);

    let mut past_time = figure(4.0, 3.0, Theme::pigment(), ScaleKind::Time, linear());
    axes(&mut past_time)
        .line_shared(
            AxisData::Times(vec![Some(Instant::from_micros(0).unwrap())]),
            numbers(vec![1.0]),
            LineOptions::default(),
        )
        .unwrap()
        .xlim_time_end(LimitEnd::High, Instant::from_micros(-DAY).unwrap())
        .unwrap();
    let err = past_time.to_scene(lib).unwrap_err();
    note("times above a fixed high", &past_time, &past_time, &err);
    out
}
