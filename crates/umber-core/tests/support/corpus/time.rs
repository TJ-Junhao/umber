//! Output-gate cases for time axes.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Scenes: a line on a time x axis over six spans (90 seconds, 6 hours, 10 days, 4 months, 3
//! years and 80 years), the themes rotating Pigment, Graphite and Midnight, which shows the
//! calendar tick steps and the concise labels; a time x axis with every mark it takes (band,
//! stacked area, step, scatter, stems, error bars, reference lines and spans), explicit limits
//! and ISO labels; and a time y axis with explicit ticks.
//!
//! In `errors/time.txt`: the errors of time data and time settings on time and numeric axes,
//! the marks a time axis refuses, and the state a failed call leaves.

use std::sync::Arc;

use umber_core::figure::{
    AreaOptions, AxisData, BandOptions, BarOptions, ErrorBarOptions, LineOptions, RuleOptions,
    ScatterOptions, SpanOptions, StemOptions, StepOptions,
};
use umber_core::{
    AxisValue, Dash, Figure, FontLibrary, Instant, ScaleKind, Series, Theme, TickFormat,
    times_from_micros,
};

use super::super::{Log, Output, axes, emit_scene, figure, linear, wobble};

/// The microseconds in a day.
const DAY: i64 = 86_400_000_000;

/// The days from 1970-01-01 to a civil date, written independently of the library.
fn days(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The instant at a civil date and time.
fn at(date: (i64, i64, i64), (hour, minute, second): (i64, i64, i64)) -> Instant {
    let clock = ((hour * 60 + minute) * 60 + second) * 1_000_000;
    Instant::from_micros(days(date.0, date.1, date.2) * DAY + clock).unwrap()
}

/// Midnight of a civil date.
fn date(year: i64, month: i64, day: i64) -> Instant {
    at((year, month, day), (0, 0, 0))
}

/// `n` times evenly spread from `low` to `high`, both included.
fn spread(low: Instant, high: Instant, n: usize) -> Vec<Option<Instant>> {
    let (low, high) = (low.micros(), high.micros());
    let steps = (n - 1) as i64;
    (0..n as i64)
        .map(|k| Some(Instant::from_micros(low + (high - low) / steps * k).unwrap()))
        .collect()
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib).unwrap(), sink);
    }
    sink(Output {
        kind: "errors",
        file: "time.txt".into(),
        bytes: time_errors().into_bytes(),
    });
}

/// Every time figure, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();
    let spans = [
        (
            "90s",
            "90 seconds",
            at((2026, 3, 3), (14, 32, 7)),
            at((2026, 3, 3), (14, 33, 37)),
            Theme::pigment(),
            180,
        ),
        (
            "6h",
            "6 hours",
            at((2026, 3, 3), (4, 40, 0)),
            at((2026, 3, 3), (10, 40, 0)),
            Theme::graphite(),
            144,
        ),
        (
            "10d",
            "10 days",
            at((2026, 2, 24), (8, 0, 0)),
            at((2026, 3, 6), (8, 0, 0)),
            Theme::midnight(),
            120,
        ),
        (
            "4mo",
            "4 months",
            date(2025, 11, 12),
            date(2026, 3, 12),
            Theme::pigment(),
            121,
        ),
        (
            "3y",
            "3 years",
            date(2023, 5, 20),
            date(2026, 5, 20),
            Theme::graphite(),
            157,
        ),
        (
            "80y",
            "80 years",
            date(1946, 4, 1),
            date(2026, 4, 1),
            Theme::midnight(),
            81,
        ),
    ];
    for (seed, (id, name, low, high, theme, n)) in spans.into_iter().enumerate() {
        let mut f = figure(6.0, 3.8, theme, ScaleKind::Time, linear());
        let (_, y) = wobble(n, 70 + seed as u32);
        axes(&mut f)
            .title(&format!("A span of {name}"))
            .ylabel("Value")
            .line_shared(
                AxisData::Times(spread(low, high, n)),
                AxisData::Numbers(y),
                LineOptions {
                    label: "Series".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        out.push((format!("time-{id}"), f));
    }

    // Every mark a time x axis takes, with explicit limits and ISO labels.
    let mut f = figure(7.0, 4.0, Theme::pigment(), ScaleKind::Time, linear());
    let x = spread(date(2026, 1, 1), date(2026, 1, 31), 31);
    let mut gapped = x.clone();
    gapped[12] = None;
    let (_, y) = wobble(31, 9);
    let level: Vec<f64> = y.iter().map(|v| v + 6.0).collect();
    let low: Vec<f64> = level.iter().map(|v| v - 1.0).collect();
    let high: Vec<f64> = level.iter().map(|v| v + 1.0).collect();
    axes(&mut f)
        .title("Marks on a time axis")
        .stacked_area_time(
            Arc::from(x.clone()),
            vec![Series {
                label: "Base".into(),
                values: vec![1.0; 31],
                ..Series::default()
            }],
        )
        .unwrap()
        .band_time(
            Arc::from(gapped.clone()),
            Arc::from(low),
            Arc::from(high),
            BandOptions {
                label: "Range".into(),
                ..BandOptions::default()
            },
        )
        .unwrap()
        .line_shared(
            AxisData::Times(gapped),
            AxisData::Numbers(level),
            LineOptions {
                label: "Level".into(),
                ..LineOptions::default()
            },
        )
        .unwrap()
        .step_shared(
            AxisData::Times(x.clone()),
            AxisData::Numbers(y.iter().map(|v| v + 3.0).collect()),
            StepOptions::default(),
        )
        .unwrap()
        .scatter_shared(
            AxisData::Times(x[..10].to_vec()),
            AxisData::Numbers(vec![9.5; 10]),
            ScatterOptions::default(),
        )
        .unwrap()
        .stem_shared(
            AxisData::Times(x[20..].to_vec()),
            AxisData::Numbers(vec![2.0; 11]),
            StemOptions::default(),
        )
        .unwrap()
        .y_error_bars(
            AxisData::Times(x[..3].to_vec()),
            &[4.0, 4.5, 5.0],
            &[5.0, 5.5, 6.0],
            ErrorBarOptions::default(),
        )
        .unwrap()
        .x_error_bars_time(
            AxisData::Numbers(vec![1.5]),
            Arc::from([Some(date(2026, 1, 16))]),
            Arc::from([Some(date(2026, 1, 25))]),
            ErrorBarOptions::default(),
        )
        .unwrap()
        .vspan_time(date(2026, 1, 10), date(2026, 1, 14), SpanOptions::default())
        .unwrap()
        .vline(
            AxisValue::Time(at((2026, 1, 20), (12, 0, 0))),
            RuleOptions {
                dash: Dash::Dashed,
                ..RuleOptions::default()
            },
        )
        .unwrap()
        .hline(8.0, RuleOptions::default())
        .unwrap()
        .xlim_time(date(2025, 12, 28), date(2026, 2, 3))
        .unwrap()
        .xtick_format(TickFormat::iso())
        .unwrap();
    out.push(("time-marks".to_owned(), f));

    // A time y axis with explicit ticks, a reference line and a span.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), ScaleKind::Time);
    let y = spread(date(2026, 1, 5), date(2026, 3, 30), 12);
    axes(&mut f)
        .title("Times along y")
        .line_shared(
            AxisData::Numbers((0..12).map(f64::from).collect()),
            AxisData::Times(y.clone()),
            LineOptions::default(),
        )
        .unwrap()
        .x_error_bars_shared(
            AxisData::Times(y[..4].to_vec()),
            Arc::from([0.5, 1.5, 2.5, 3.5]),
            Arc::from([1.5, 2.5, 3.5, 4.5]),
            ErrorBarOptions::default(),
        )
        .unwrap()
        .y_error_bars_time(
            AxisData::Numbers(vec![9.0, 10.0]),
            Arc::from([Some(date(2026, 1, 20)), Some(date(2026, 2, 1))]),
            Arc::from([Some(date(2026, 2, 5)), Some(date(2026, 3, 10))]),
            ErrorBarOptions::default(),
        )
        .unwrap()
        .hline_time(date(2026, 2, 14), RuleOptions::default())
        .unwrap()
        .hspan_time(date(2026, 3, 1), date(2026, 3, 8), SpanOptions::default())
        .unwrap()
        .yticks_time(&[date(2026, 1, 1), date(2026, 2, 1), date(2026, 3, 1)])
        .unwrap();
    out.push(("time-y-axis".to_owned(), f));
    out
}

/// The errors of time axes, each call with the state it leaves.
fn time_errors() -> String {
    let mut rec = Log::default();
    let times = |v: &[Instant]| AxisData::Times(v.iter().copied().map(Some).collect());
    let (jan, feb) = (date(2026, 1, 1), date(2026, 2, 1));

    rec.section("times_from_micros");
    let max = Instant::MAX.micros();
    for (label, micros) in [
        ("in range and missing", vec![0, i64::MIN, max]),
        ("after 9999", vec![0, max + 1]),
        ("before year 1", vec![Instant::MIN.micros() - 1]),
    ] {
        rec.result(label, times_from_micros("Axes.line()", "x", &micros), |t| {
            super::super::record::times(&t)
        });
    }

    rec.section("Axes on a time x axis");
    let mut f = rec.new_figure("time/linear", ScaleKind::Time, linear());
    rec.on_axes(&mut f, "line ok", |a| {
        a.line_shared(
            times(&[jan, feb]),
            AxisData::Numbers(vec![1.0, 2.0]),
            LineOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "line with numbers", |a| {
        a.line(AxisData::Numbers(vec![0.0]), &[1.0], LineOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "area with names", |a| {
        a.area(
            AxisData::Categories(vec!["a".into()]),
            &[1.0],
            AreaOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "step unsorted", |a| {
        a.step_shared(
            times(&[feb, jan]),
            AxisData::Numbers(vec![1.0, 2.0]),
            StepOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "xlim_time ok", |a| a.xlim_time(jan, feb).map(drop));
    rec.on_axes(&mut f, "xlim_time reversed", |a| {
        a.xlim_time(feb, jan).map(drop)
    });
    rec.on_axes(&mut f, "xlim numbers", |a| a.xlim(0.0, 1.0).map(drop));
    rec.on_axes(&mut f, "xticks numbers", |a| a.xticks(&[0.0]).map(drop));
    rec.on_axes(&mut f, "xticks_time repeated", |a| {
        a.xticks_time(&[jan, jan]).map(drop)
    });
    rec.on_axes(&mut f, "xticks_time ok", |a| {
        a.xticks_time(&[jan]).map(drop)
    });
    rec.on_axes(&mut f, "xtick_format fixed", |a| {
        a.xtick_format(TickFormat::fixed(1).unwrap()).map(drop)
    });
    rec.on_axes(&mut f, "xtick_format iso", |a| {
        a.xtick_format(TickFormat::iso()).map(drop)
    });
    rec.on_axes(&mut f, "vline number", |a| {
        a.vline(AxisValue::Number(1.0), RuleOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "vspan numbers", |a| {
        a.vspan(0.0, 1.0, SpanOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "vspan_time reversed", |a| {
        a.vspan_time(feb, jan, SpanOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "bar", |a| {
        a.bar(vec!["a".into()], &[1.0], BarOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "hist", |a| {
        a.hist(&[1.0], umber_core::HistOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "bubbles", |a| {
        a.bubbles(&[1.0], &[1.0], &[1.0], umber_core::BubbleOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "x_error_bars", |a| {
        a.x_error_bars(&[1.0], &[0.0], &[2.0], ErrorBarOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "x_error_bars_time ok", |a| {
        a.x_error_bars_time(
            AxisData::Numbers(vec![1.0]),
            Arc::from([Some(jan)]),
            Arc::from([Some(feb)]),
            ErrorBarOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "x_error_bars_time reversed", |a| {
        a.x_error_bars_time(
            AxisData::Numbers(vec![1.0]),
            Arc::from([Some(feb)]),
            Arc::from([Some(jan)]),
            ErrorBarOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "y_error_bars_time on a linear y", |a| {
        a.y_error_bars_time(
            times(&[jan]),
            Arc::from([Some(jan)]),
            Arc::from([Some(feb)]),
            ErrorBarOptions::default(),
        )
        .map(drop)
    });

    rec.section("Axes on a linear/time pair");
    let mut f = rec.new_figure("linear/time", linear(), ScaleKind::Time);
    rec.on_axes(&mut f, "hline number", |a| {
        a.hline(1.0, RuleOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "hline_time ok", |a| {
        a.hline_time(jan, RuleOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "y_error_bars", |a| {
        a.y_error_bars(
            AxisData::Numbers(vec![1.0]),
            &[0.0],
            &[2.0],
            ErrorBarOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "band", |a| {
        a.band(&[0.0], &[0.0], &[1.0], BandOptions::default())
            .map(drop)
    });

    rec.section("Times on numeric axes");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "line with times", |a| {
        a.line_shared(
            times(&[jan]),
            AxisData::Numbers(vec![1.0]),
            LineOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "xlim_time", |a| a.xlim_time(jan, feb).map(drop));
    rec.on_axes(&mut f, "xticks_time", |a| a.xticks_time(&[jan]).map(drop));
    rec.on_axes(&mut f, "vline time", |a| {
        a.vline(AxisValue::Time(jan), RuleOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "hline_time", |a| {
        a.hline_time(jan, RuleOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "band_time", |a| {
        a.band_time(
            Arc::from([Some(jan)]),
            Arc::from([0.0]),
            Arc::from([1.0]),
            BandOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "xtick_format iso", |a| {
        a.xtick_format(TickFormat::iso()).map(drop)
    });
    rec.out
}
