//! Output-gate cases for histogram variants: explicit bin edges, step outlines, stacked groups,
//! fractions, densities and cumulative bars.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Covered: step outlines and stacked groups in the three built-in themes, with labels; uneven
//! edges with densities; cumulative fractions; stacked groups under a gradient fill and on a log
//! y axis; a step histogram with an empty group and limits that cut it; and every new error
//! branch of `Axes::hist` and `Axes::grouped_hist`, in the order the checks run, with the state a
//! failed call leaves.

use umber_core::figure::{HistOptions, Series};
use umber_core::{
    Bins, ColorSpec, FillStyle, FontLibrary, GroupedHistOptions, HistStat, HistStyle, LegendStyle,
    ScaleKind, Theme,
};

use super::super::{Log, Output, axes, emit_scene, figure, labelled, lcg, linear, log, themes};

/// `n` values around `center`, roughly normal with a spread of about `spread`: the sum of six
/// uniform draws, scaled. Integer and basic float arithmetic only.
fn bell(n: usize, seed: u32, center: f64, spread: f64) -> Vec<f64> {
    let mut next = lcg(seed);
    (0..n)
        .map(|_| {
            let sum: f64 = (0..6).map(|_| next()).sum();
            center + 2.0 * spread * (sum - 3.0)
        })
        .collect()
}

/// A labelled series of `values` in the automatic color.
fn series(label: &str, values: Vec<f64>) -> Series {
    Series {
        label: label.into(),
        values,
        color: ColorSpec::AUTO,
    }
}

/// Three groups of different sizes and centres.
fn groups() -> Vec<Series> {
    vec![
        series("North", bell(600, 3, 40.0, 8.0)),
        series("South", bell(300, 5, 55.0, 6.0)),
        series("East", bell(150, 7, 65.0, 10.0)),
    ]
}

fn grouped(style: HistStyle, stat: HistStat) -> GroupedHistOptions {
    GroupedHistOptions {
        bins: Bins::Count(24),
        style,
        stat,
        ..GroupedHistOptions::default()
    }
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (theme_name, theme) in themes() {
        // Outlines of three groups, each normalized on its own.
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Step outlines");
        axes(&mut f)
            .grouped_hist(groups(), grouped(HistStyle::Step, HistStat::Fraction))
            .unwrap();
        emit_scene(
            &format!("fig-hist-step-{theme_name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        // Three groups stacked in each bin, normalized together.
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Stacked groups");
        axes(&mut f)
            .grouped_hist(groups(), grouped(HistStyle::Stacked, HistStat::Count))
            .unwrap();
        emit_scene(
            &format!("fig-hist-stacked-{theme_name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        // Uneven edges with densities, so wide bins are as low as their share asks.
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Uneven edges");
        axes(&mut f)
            .hist(
                &bell(800, 11, 50.0, 12.0),
                HistOptions {
                    bins: Bins::Edges(vec![0.0, 30.0, 40.0, 45.0, 50.0, 55.0, 60.0, 70.0, 100.0]),
                    stat: HistStat::Density,
                    label: "Density".into(),
                    ..HistOptions::default()
                },
            )
            .unwrap();
        emit_scene(
            &format!("fig-hist-edges-{theme_name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );

        // Cumulative fractions, as bars and as an outline.
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        labelled(&mut f, "Cumulative fractions");
        let values = bell(500, 13, 20.0, 5.0);
        for (style, label) in [(HistStyle::Bars, "Bars"), (HistStyle::Step, "Outline")] {
            axes(&mut f)
                .hist(
                    &values,
                    HistOptions {
                        bins: Bins::Count(20),
                        style,
                        cumulative: true,
                        stat: HistStat::Fraction,
                        label: label.into(),
                        ..HistOptions::default()
                    },
                )
                .unwrap();
        }
        emit_scene(
            &format!("fig-hist-cumulative-{theme_name}"),
            &f.to_scene(lib).unwrap(),
            sink,
        );
    }

    // Stacked groups under a gradient fill: one ramp from the tallest stack for every group.
    let theme = Theme::pigment()
        .with_fill_style(FillStyle::Gradient)
        .unwrap();
    let mut f = figure(6.0, 4.0, theme, linear(), linear());
    labelled(&mut f, "Stacked under a gradient");
    axes(&mut f)
        .grouped_hist(groups(), grouped(HistStyle::Stacked, HistStat::Fraction))
        .unwrap();
    emit_scene("fig-hist-stacked-gradient", &f.to_scene(lib).unwrap(), sink);

    // Stacked counts on a log y axis: the first group grows from the baseline.
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), log(10.0));
    labelled(&mut f, "Stacked on a log axis");
    axes(&mut f)
        .grouped_hist(groups(), grouped(HistStyle::Stacked, HistStat::Count))
        .unwrap();
    emit_scene("fig-hist-stacked-log-y", &f.to_scene(lib).unwrap(), sink);

    // Outlines with an empty group, a direct legend and limits that cut the outlines.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    labelled(&mut f, "Cut outlines");
    axes(&mut f)
        .legend(LegendStyle::Direct)
        .grouped_hist(
            vec![
                series("Kept", bell(400, 17, 50.0, 10.0)),
                series("Empty", vec![f64::NAN]),
            ],
            grouped(HistStyle::Step, HistStat::Density),
        )
        .unwrap()
        .xlim(42.0, 58.0)
        .unwrap();
    emit_scene("fig-hist-step-cut", &f.to_scene(lib).unwrap(), sink);

    sink(Output {
        kind: "errors",
        file: "hist-variants.txt".into(),
        bytes: errors().into_bytes(),
    });
}

/// Every new error branch of `Axes::hist` and `Axes::grouped_hist`, in the order their checks
/// run, and the state after each call.
fn errors() -> String {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let mut rec = Log::default();
    let edges = |edges: Vec<f64>| HistOptions {
        bins: Bins::Edges(edges),
        ..HistOptions::default()
    };

    rec.section("Axes.hist edges, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "hist edges ok", |a| {
        a.hist(&[0.0, 1.0, 2.5, nan, 10.0], edges(vec![0.0, 2.5, 10.0]))
            .map(drop)
    });
    let cases: Vec<(&str, Vec<f64>, Vec<f64>)> = vec![
        ("no edges", vec![1.0], vec![]),
        ("one edge", vec![1.0], vec![1.0]),
        ("NaN edge", vec![1.0], vec![0.0, nan, 2.0]),
        (
            "infinite edge before a repeat",
            vec![1.0],
            vec![inf, 0.0, 0.0],
        ),
        ("huge edge", vec![1.0], vec![0.0, -1e301]),
        ("repeated edge", vec![1.0], vec![0.0, 1.0, 1.0]),
        ("decreasing edge", vec![1.0], vec![0.0, 2.0, 1.0]),
        ("narrow edges", vec![1.0], vec![1.0, 1.0 + 1e-14]),
        ("value below the edges", vec![-1.0, 1.0], vec![0.0, 2.0]),
        ("values above the edges", vec![3.0, inf], vec![0.0, 2.0]),
        ("values outside", vec![-1.0, 3.0, 4.0, 1.0], vec![0.0, 2.0]),
    ];
    for (name, values, bins) in cases {
        rec.on_axes(&mut f, &format!("hist {name}"), |a| {
            a.hist(&values, edges(bins)).map(drop)
        });
    }
    rec.on_axes(&mut f, "hist cumulative density", |a| {
        a.hist(
            &[1.0],
            HistOptions {
                cumulative: true,
                stat: HistStat::Density,
                ..HistOptions::default()
            },
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "hist cumulative density with bad edges", |a| {
        a.hist(
            &[1.0],
            HistOptions {
                bins: Bins::Edges(vec![1.0]),
                cumulative: true,
                stat: HistStat::Density,
                ..HistOptions::default()
            },
        )
        .map(drop)
    });

    rec.section("Axes.grouped_hist, linear/linear");
    rec.on_axes(&mut f, "grouped_hist ok", |a| {
        a.grouped_hist(
            vec![series("a", vec![1.0, 2.0]), series("b", vec![nan, 3.0])],
            GroupedHistOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "grouped_hist no series", |a| {
        a.grouped_hist(Vec::new(), GroupedHistOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "grouped_hist bars", |a| {
        a.grouped_hist(
            vec![series("a", vec![1.0])],
            GroupedHistOptions {
                style: HistStyle::Bars,
                ..GroupedHistOptions::default()
            },
        )
        .map(drop)
    });
    rec.on_axes(&mut f, "grouped_hist infinite value in series 1", |a| {
        a.grouped_hist(
            vec![series("a", vec![1.0]), series("b", vec![2.0, inf])],
            GroupedHistOptions::default(),
        )
        .map(drop)
    });
    rec.on_axes(
        &mut f,
        "grouped_hist value outside the edges in series 1",
        |a| {
            a.grouped_hist(
                vec![series("a", vec![1.0]), series("b", vec![2.0, 9.0])],
                GroupedHistOptions {
                    bins: Bins::Edges(vec![0.0, 5.0]),
                    ..GroupedHistOptions::default()
                },
            )
            .map(drop)
        },
    );

    rec.section("Axes.grouped_hist, log/linear");
    let mut f = rec.new_figure("log/linear", log(10.0), linear());
    rec.on_axes(&mut f, "grouped_hist on a log x axis", |a| {
        a.grouped_hist(vec![series("a", vec![1.0])], GroupedHistOptions::default())
            .map(drop)
    });

    rec.section("Axes.grouped_hist, linear/category");
    let mut f = rec.new_figure("linear/category", linear(), ScaleKind::Category);
    rec.on_axes(&mut f, "grouped_hist on a category y axis", |a| {
        a.grouped_hist(vec![series("a", vec![1.0])], GroupedHistOptions::default())
            .map(drop)
    });
    rec.out
}
