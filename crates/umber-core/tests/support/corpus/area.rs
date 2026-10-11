//! Output-gate cases for area figures.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Covered: areas next to lines in the three built-in themes, so under their legend styles
//! (header, direct labels and chips); each theme's fill style switched to the other; NaN gaps
//! and lines that cross the baseline; a log y axis, where areas fill down to the baseline; an
//! area next to a scatter, which gives the x axis nice limits; a category x axis; explicit
//! limits that leave 0 outside the y axis and cut the line ends, with direct labels at the edge
//! and at end dots; lines that run above and below the y axis, on a linear and on a log y axis,
//! whose fills still show between the clip edge and the baseline; chips set on the axes;
//! 200 000 points, downsampled; and every error branch
//! of `Axes::area`, including which fault is reported when an input has several, and the state
//! a failed call leaves.

use umber_core::figure::{AreaOptions, AxisData, LineOptions, ScatterOptions};
use umber_core::{Alpha, ColorSpec, Dash, FillStyle, FontLibrary, LegendStyle, ScaleKind, Theme};

use super::super::{
    Log, Output, axes, emit_scene, figure, labelled, linear, log, numbers, themes, wobble,
};

fn labelled_area(label: &str) -> AreaOptions {
    AreaOptions {
        label: label.into(),
        ..AreaOptions::default()
    }
}

fn labelled_line(label: &str, dash: Dash) -> LineOptions {
    LineOptions {
        label: label.into(),
        dash,
        ..LineOptions::default()
    }
}

/// A ramp of `n` points with a deterministic wobble, lifted by `lift`.
fn lifted(n: usize, seed: u32, lift: f64) -> (Vec<f64>, Vec<f64>) {
    let (x, y) = wobble(n, seed);
    (x, y.iter().map(|v| v + lift).collect())
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib), sink);
    }
    sink(Output {
        kind: "errors",
        file: "area.txt".into(),
        bytes: area_errors().into_bytes(),
    });
}

/// Every area figure, by name, in a fixed order.
fn figures() -> Vec<(String, umber_core::Figure)> {
    let mut out = Vec::new();

    // An area under a dashed line and a solid line, in every theme and its legend style.
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Area and lines");
        let (x, y) = lifted(40, 11, 6.0);
        let (x2, y2) = lifted(40, 12, 9.0);
        let (x3, y3) = lifted(40, 13, 3.0);
        axes(&mut f)
            .area(numbers(x), &y, labelled_area("Demand"))
            .unwrap()
            .line(numbers(x2), &y2, labelled_line("Forecast", Dash::Dashed))
            .unwrap()
            .area(numbers(x3), &y3, labelled_area("Base load"))
            .unwrap();
        out.push((format!("fig-area-{theme_name}"), f));
    }

    // Each theme's fill style switched to the other.
    for (name, theme, style) in [
        (
            "fig-area-gradient-pigment",
            Theme::pigment(),
            FillStyle::Gradient,
        ),
        (
            "fig-area-gradient-graphite",
            Theme::graphite(),
            FillStyle::Gradient,
        ),
        (
            "fig-area-solid-midnight",
            Theme::midnight(),
            FillStyle::Solid,
        ),
    ] {
        let mut f = figure(
            6.0,
            4.0,
            theme.with_fill_style(style).unwrap(),
            linear(),
            linear(),
        );
        labelled(&mut f, "Fill style");
        let (x, y) = lifted(30, 21, 4.0);
        axes(&mut f)
            .area(numbers(x), &y, labelled_area("Output"))
            .unwrap();
        out.push((name.to_owned(), f));
    }

    // NaN gaps, an isolated point between two gaps, and runs on both sides of the baseline.
    let nan = f64::NAN;
    for (theme_name, theme) in [
        ("pigment", Theme::pigment()),
        ("midnight", Theme::midnight()),
    ] {
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Gaps and sign changes");
        axes(&mut f)
            .area(
                numbers((0..12).map(f64::from).collect()),
                &[
                    2.0, 3.5, nan, 1.5, nan, -1.0, -2.5, 1.0, 2.0, nan, -3.0, -1.5,
                ],
                labelled_area("Balance"),
            )
            .unwrap()
            .line(
                numbers((0..12).map(f64::from).collect()),
                &[1.0, 1.5, 2.0, 1.0, 0.5, nan, -0.5, 0.0, 1.5, 2.5, 1.0, 0.5],
                labelled_line("Trend", Dash::Solid),
            )
            .unwrap();
        out.push((format!("fig-area-nan-gaps-{theme_name}"), f));
    }

    // A log y axis: the area fills down to the baseline, below the data.
    for (theme_name, theme) in [
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        let mut f = figure(6.0, 4.0, theme, linear(), log(10.0));
        labelled(&mut f, "Areas on a log axis");
        let x: Vec<f64> = (0..25).map(f64::from).collect();
        let y: Vec<f64> = x.iter().map(|v| 3.0 * 1.3_f64.powf(*v)).collect();
        let y2: Vec<f64> = x.iter().map(|v| 40.0 + 5.0 * v).collect();
        axes(&mut f)
            .area(numbers(x.clone()), &y, labelled_area("Growth"))
            .unwrap()
            .area(numbers(x), &y2, labelled_area("Linear"))
            .unwrap();
        out.push((format!("fig-area-log-y-{theme_name}"), f));
    }

    // Next to a scatter, the x axis gets nice limits instead of the exact extent.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut f, "Area and points");
    let (x, y) = lifted(20, 31, 5.0);
    let (x2, y2) = lifted(15, 32, 6.0);
    axes(&mut f)
        .area(numbers(x), &y, labelled_area("Range"))
        .unwrap()
        .scatter(
            numbers(x2),
            &y2,
            ScatterOptions {
                label: "Samples".into(),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    out.push(("fig-area-with-scatter".to_owned(), f));

    // A category x axis.
    let months: Vec<String> = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"]
        .map(String::from)
        .to_vec();
    let mut f = figure(6.0, 4.0, Theme::graphite(), ScaleKind::Category, linear());
    labelled(&mut f, "Categories");
    axes(&mut f)
        .area(
            AxisData::Categories(months.clone()),
            &[4.0, 6.5, 5.0, 8.0, 7.5, 9.0],
            labelled_area("Wind"),
        )
        .unwrap()
        .area(
            AxisData::Categories(months),
            &[1.0, 2.0, 3.5, 4.0, 3.0, 2.5],
            AreaOptions {
                label: "Solar".into(),
                color: ColorSpec::Auto(Alpha::new(0.6).unwrap()),
                dash: Dash::Dotted,
                ..AreaOptions::default()
            },
        )
        .unwrap();
    out.push(("fig-area-category".to_owned(), f));

    // Explicit limits: 0 lies below the y axis, so the areas fill to the lower limit; the x
    // limit cuts one area (a direct label at the edge, no end dot) and not the other (a label
    // at its end dot); the line of a third area lies above the y axis, so it shows no stroke
    // but fills the plot from the baseline up to the top of the clip.
    for (theme_name, theme) in [
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Explicit limits");
        let (x, y) = lifted(30, 41, 10.0);
        let (x2, y2) = lifted(20, 42, 9.0);
        axes(&mut f)
            .area(numbers(x), &y, labelled_area("Cut"))
            .unwrap()
            .area(numbers(x2), &y2, labelled_area("Ends"))
            .unwrap()
            .area(
                numbers(vec![0.0, 10.0, 20.0]),
                &[40.0, 45.0, 42.0],
                labelled_area("Above"),
            )
            .unwrap()
            .xlim(0.0, 24.5)
            .unwrap()
            .ylim(6.0, 14.0)
            .unwrap()
            .legend(LegendStyle::Direct);
        out.push((format!("fig-area-direct-limits-{theme_name}"), f));
    }

    // Lines that leave the y axis: entering from above, dipping below, and wholly above. The
    // fills run along the clip edge where the lines are beyond it.
    for (theme_name, theme, y_scale, ylim, ys) in [
        (
            "midnight",
            Theme::midnight(),
            linear(),
            (0.0, 10.0),
            [
                vec![25.0, 4.0, 6.0, 30.0],
                vec![2.0, -3.0, -1.0, 3.0],
                vec![12.0, 15.0, 11.0, 14.0],
            ],
        ),
        (
            "pigment-log",
            Theme::pigment(),
            log(10.0),
            (1.0, 100.0),
            [
                vec![1e4, 20.0, 50.0, 1e6],
                vec![5.0, 0.01, 0.1, 8.0],
                vec![300.0, 1e3, 500.0, 200.0],
            ],
        ),
    ] {
        let mut f = figure(6.0, 4.0, theme, linear(), y_scale);
        labelled(&mut f, "Beyond the y axis");
        for (label, y) in ["Enters", "Dips", "Above"].into_iter().zip(ys) {
            axes(&mut f)
                .area(numbers(vec![0.0, 1.0, 2.0, 3.0]), &y, labelled_area(label))
                .unwrap();
        }
        axes(&mut f).ylim(ylim.0, ylim.1).unwrap();
        out.push((format!("fig-area-beyond-y-{theme_name}"), f));
    }

    // Chips set on the axes, over a header theme.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut f, "Chips");
    for (k, label) in ["North", "South"].into_iter().enumerate() {
        let (x, y) = lifted(25, 51 + k as u32, 4.0 + 3.0 * k as f64);
        axes(&mut f)
            .area(numbers(x), &y, labelled_area(label))
            .unwrap();
    }
    let (x, y) = lifted(25, 53, 6.0);
    axes(&mut f)
        .line(numbers(x), &y, labelled_line("Mean", Dash::Dashed))
        .unwrap()
        .legend(LegendStyle::Chips);
    out.push(("fig-area-chips".to_owned(), f));

    // Many points: the line and its fill are downsampled together.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    let (x, y) = lifted(200_000, 61, 50.0);
    axes(&mut f)
        .area(numbers(x), &y, AreaOptions::default())
        .unwrap();
    out.push(("fig-area-200k-points".to_owned(), f));

    out
}

/// Every error branch of `Axes::area`, in the order its checks run, and the state after each
/// call.
fn area_errors() -> String {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let nums = |v: &[f64]| AxisData::Numbers(v.to_vec());
    let cats = |v: &[&str]| AxisData::Categories(v.iter().map(|s| (*s).to_owned()).collect());
    let area = AreaOptions::default;
    let mut rec = Log::default();

    rec.section("Axes.area, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "area ok", |a| {
        a.area(nums(&[0.0, 1.0]), &[1.0, 2.0], area()).map(drop)
    });
    let cases: Vec<(&str, AxisData, Vec<f64>)> = vec![
        ("length mismatch", nums(&[0.0]), vec![1.0, 2.0]),
        (
            "length mismatch and infinite y",
            nums(&[inf]),
            vec![inf, 2.0],
        ),
        ("infinite y", nums(&[0.0]), vec![inf]),
        ("negative infinite x", nums(&[f64::NEG_INFINITY]), vec![1.0]),
        ("huge x", nums(&[1.0e301]), vec![1.0]),
        ("huge negative y", nums(&[0.0]), vec![-1.0e301]),
        (
            "huge y[1] before infinite y[2]",
            nums(&[0.0, 1.0, 2.0]),
            vec![nan, 1.0e301, inf],
        ),
        ("categories on number axis", cats(&["a"]), vec![1.0]),
    ];
    for (name, x, y) in cases {
        rec.on_axes(&mut f, &format!("area {name}"), |a| {
            a.area(x, &y, area()).map(drop)
        });
    }
    // Gaps with non-canonical NaN payloads and signed zeros: storage keeps every bit.
    let p0 = f64::from_bits(0x7ff8_0000_0000_0001);
    let p1 = f64::from_bits(0xfff0_0000_dead_beef);
    rec.on_axes(&mut f, "area with NaN payloads", |a| {
        a.area(nums(&[0.0, p0, 2.0, -0.0]), &[p1, 1.0, 0.0, -0.0], area())
            .map(drop)
    });
    rec.on_axes(&mut f, "area with options", |a| {
        a.area(
            nums(&[umber_core::MAX_MAGNITUDE, -umber_core::MAX_MAGNITUDE]),
            &[-1.0, 1.0],
            AreaOptions {
                label: "edge".into(),
                color: ColorSpec::Palette {
                    index: 21,
                    alpha: Alpha::new(0.25).unwrap(),
                },
                dash: Dash::Dashed,
                ..AreaOptions::default()
            },
        )
        .map(drop)
    });

    rec.section("Axes.area, category/log10");
    let mut f = rec.new_figure("category/log10", ScaleKind::Category, log(10.0));
    for (name, x, y) in [
        ("numbers on category axis", nums(&[1.0]), vec![1.0]),
        ("zero y", cats(&["a"]), vec![0.0]),
        ("negative y", cats(&["a", "b"]), vec![5.0, -1.0]),
        ("tiny y", cats(&["a"]), vec![1.0e-301]),
        ("smallest y", cats(&["a"]), vec![1.0e-300]),
        ("NaN y", cats(&["a", "b"]), vec![nan, 3.0]),
    ] {
        rec.on_axes(&mut f, &format!("area {name}"), |a| {
            a.area(x, &y, area()).map(drop)
        });
    }

    // The y axis is never a category axis: the axes refuse it, so no area meets one.
    rec.section("Axes.area, category y");
    rec.new_figure("linear/category", linear(), ScaleKind::Category);

    rec.section("Axes.area, log2/linear");
    let mut f = rec.new_figure("log2/linear", log(2.0), linear());
    for (name, x, y) in [
        ("zero x", nums(&[0.0]), vec![1.0]),
        ("negative x, infinite y", nums(&[-1.0]), vec![inf]),
        ("negative y is fine", nums(&[1.0]), vec![-1.0]),
    ] {
        rec.on_axes(&mut f, &format!("area {name}"), |a| {
            a.area(x, &y, area()).map(drop)
        });
    }
    rec.out
}
