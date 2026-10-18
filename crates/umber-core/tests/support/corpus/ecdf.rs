//! Output-gate cases for ECDFs: step lines from 0 to 1 over numbers and times.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Covered: two labelled ECDFs and a complementary one in the three built-in themes; ECDFs on a
//! time axis and on a log x axis; direct labels at the end of ECDFs; an ECDF beside a line,
//! whose y axis is padded as usual; tied values, gaps and a single value; and every error branch
//! of `Axes::ecdf`, in the order the checks run, with the state a failed call leaves.

use umber_core::figure::LineOptions;
use umber_core::{
    AxisData, ColorSpec, Dash, EcdfOptions, FontLibrary, Instant, LegendStyle, ScaleKind, SizeSpec,
    Theme,
};

use super::super::{Log, Output, axes, emit_scene, figure, labelled, lcg, linear, log, themes};

/// `n` values with a long right tail, from `low`: integer and basic float arithmetic only.
fn latencies(n: usize, seed: u32, low: f64) -> Vec<f64> {
    let mut next = lcg(seed);
    (0..n)
        .map(|_| {
            let u = next();
            low + 40.0 * u * u * u + 5.0 * next()
        })
        .collect()
}

fn numbers(values: Vec<f64>) -> AxisData {
    AxisData::Numbers(values)
}

fn labelled_ecdf(label: &str) -> EcdfOptions {
    EcdfOptions {
        label: label.into(),
        ..EcdfOptions::default()
    }
}

/// The instant `hours` hours after 2026-01-05T00:00.
fn hour(hours: i64) -> Option<Instant> {
    const START: i64 = 1_767_571_200_000_000;
    Some(Instant::from_micros(START + hours * 3_600_000_000).unwrap())
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Response times");
        axes(&mut f)
            .ecdf(numbers(latencies(300, 3, 10.0)), labelled_ecdf("Before"))
            .unwrap()
            .ecdf(numbers(latencies(200, 5, 4.0)), labelled_ecdf("After"))
            .unwrap();
        emit_scene(
            &format!("fig-ecdf-{theme_name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Share still waiting");
        axes(&mut f)
            .ecdf(
                numbers(latencies(150, 7, 2.0)),
                EcdfOptions {
                    complementary: true,
                    label: "Waiting".into(),
                    dash: Dash::Dashed,
                    width: SizeSpec::times(1.5).unwrap(),
                    ..EcdfOptions::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("fig-ecdf-complementary-{theme_name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }

    // Times: arrivals over a week, with ties and a missing time.
    let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Time, linear());
    labelled(&mut f, "Arrivals");
    let mut next = lcg(11);
    let mut arrivals: Vec<Option<Instant>> =
        (0..80).map(|_| hour((next() * 168.0) as i64)).collect();
    arrivals.extend([None, hour(0), hour(0)]);
    axes(&mut f)
        .ecdf(AxisData::Times(arrivals), labelled_ecdf("Arrivals"))
        .unwrap();
    emit_scene("fig-ecdf-time", &f.to_scene(lib).unwrap(), sink);

    // A log x axis, with direct labels at the ends of the lines.
    let mut f = figure(6.0, 4.0, Theme::midnight(), log(10.0), linear());
    labelled(&mut f, "File sizes");
    let mut next = lcg(13);
    let sizes: Vec<f64> = (0..200).map(|_| 10f64.powf(1.0 + 4.0 * next())).collect();
    let small: Vec<f64> = sizes.iter().map(|v| v / 10.0).collect();
    axes(&mut f)
        .legend(LegendStyle::Direct)
        .ecdf(numbers(sizes), labelled_ecdf("Images"))
        .unwrap()
        .ecdf(numbers(small), labelled_ecdf("Thumbnails"))
        .unwrap();
    emit_scene("fig-ecdf-log-x", &f.to_scene(lib).unwrap(), sink);

    // Beside a line, the y axis takes its usual padding; ties, gaps and one value.
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    labelled(&mut f, "Beside a line");
    axes(&mut f)
        .ecdf(
            numbers(vec![2.0, 2.0, f64::NAN, 3.0, 1.0, 2.0]),
            labelled_ecdf("Ties"),
        )
        .unwrap()
        .ecdf(numbers(vec![2.5]), labelled_ecdf("One value"))
        .unwrap()
        .line(
            numbers(vec![0.0, 4.0]),
            &[0.2, 1.3],
            LineOptions {
                label: "Line".into(),
                color: ColorSpec::AUTO,
                ..LineOptions::default()
            },
        )
        .unwrap();
    emit_scene("fig-ecdf-beside-a-line", &f.to_scene(lib).unwrap(), sink);

    sink(Output {
        kind: "errors",
        file: "ecdf.txt".into(),
        bytes: errors().into_bytes(),
    });
}

/// Every error branch of `Axes::ecdf`, in the order its checks run, and the state after each
/// call.
fn errors() -> String {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let mut rec = Log::default();
    let plain = EcdfOptions::default;

    rec.section("Axes.ecdf, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "ecdf ok", |a| {
        a.ecdf(numbers(vec![1.0, nan, -0.0, 0.0]), labelled_ecdf("kept"))
            .map(drop)
    });
    for (name, values) in [
        ("empty", vec![]),
        ("only NaN", vec![nan, nan]),
        ("infinite value", vec![1.0, inf]),
        ("huge value", vec![nan, -1e301]),
    ] {
        rec.on_axes(&mut f, &format!("ecdf {name}"), |a| {
            a.ecdf(numbers(values), plain()).map(drop)
        });
    }
    rec.on_axes(&mut f, "ecdf names", |a| {
        a.ecdf(AxisData::Categories(vec!["a".into()]), plain())
            .map(drop)
    });
    rec.on_axes(&mut f, "ecdf times", |a| {
        a.ecdf(AxisData::Times(vec![hour(1)]), plain()).map(drop)
    });

    rec.section("Axes.ecdf, log/linear");
    let mut f = rec.new_figure("log/linear", log(10.0), linear());
    rec.on_axes(&mut f, "ecdf not positive", |a| {
        a.ecdf(numbers(vec![1.0, 0.0]), plain()).map(drop)
    });

    rec.section("Axes.ecdf, time/linear");
    let mut f = rec.new_figure("time/linear", ScaleKind::Time, linear());
    rec.on_axes(&mut f, "ecdf times ok", |a| {
        a.ecdf(AxisData::Times(vec![hour(2), None, hour(1)]), plain())
            .map(drop)
    });
    rec.on_axes(&mut f, "ecdf only missing times", |a| {
        a.ecdf(AxisData::Times(vec![None]), plain()).map(drop)
    });
    rec.on_axes(&mut f, "ecdf numbers on a time axis", |a| {
        a.ecdf(numbers(vec![1.0]), plain()).map(drop)
    });

    rec.section("Axes.ecdf, category/linear");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    rec.on_axes(&mut f, "ecdf on a category x axis", |a| {
        a.ecdf(numbers(vec![1.0]), plain()).map(drop)
    });

    rec.section("Axes.ecdf, linear/log");
    let mut f = rec.new_figure("linear/log", linear(), log(10.0));
    rec.on_axes(&mut f, "ecdf on a log y axis", |a| {
        a.ecdf(numbers(vec![1.0, inf]), plain()).map(drop)
    });

    rec.section("Axes.ecdf, linear/time");
    let mut f = rec.new_figure("linear/time", linear(), ScaleKind::Time);
    rec.on_axes(&mut f, "ecdf on a time y axis", |a| {
        a.ecdf(numbers(vec![1.0]), plain()).map(drop)
    });
    rec.out
}
