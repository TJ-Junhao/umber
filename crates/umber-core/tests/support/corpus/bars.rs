//! Output-gate cases for bar and grouped or stacked bar figures.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Covered: plain bars in the three built-in themes; negative heights; NaN and zero heights; a
//! huge height under narrow limits; a log y axis, with explicit limits that leave one bar below
//! the plot; bar marks that share categories with category lines and with each other, and miss
//! some of the lines'; every error branch of `Axes::bar`, including which fault is reported
//! when an input has several, and the state a failed call leaves.
//!
//! Grouped and stacked bars: both in the three built-in themes; mixed signs with a NaN
//! segment; one and twelve series; fixed and palette colors between automatic ones; stacked
//! totals at the magnitude bound; a log y axis; huge stacks under narrow limits; and every
//! error branch of `Axes::grouped_bar` and `Axes::stacked_bar`, in a file of their own.
//!
//! Marks carry no labels, so no legend is drawn.

use umber_core::figure::{AxisData, BarOptions, BarSeries};
use umber_core::{Alpha, ColorSpec, FontLibrary, Rgba, ScaleKind, Theme};

use super::super::{Log, Output, axes, emit_scene, figure, labelled, line, linear, log, themes};

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

fn bar() -> BarOptions {
    BarOptions::default()
}

/// An unlabelled series of `heights` in the automatic color.
fn series(heights: &[f64]) -> BarSeries {
    BarSeries {
        heights: heights.to_vec(),
        ..BarSeries::default()
    }
}

/// An unlabelled series of `heights` in `color`.
fn colored(color: ColorSpec, heights: &[f64]) -> BarSeries {
    BarSeries {
        color,
        ..series(heights)
    }
}

/// How a bar group arranges its series, and the name of its cases.
#[derive(Clone, Copy)]
enum Group {
    Grouped,
    Stacked,
}

impl Group {
    const BOTH: [Group; 2] = [Group::Grouped, Group::Stacked];

    fn name(self) -> &'static str {
        match self {
            Group::Grouped => "grouped",
            Group::Stacked => "stacked",
        }
    }

    fn add(
        self,
        axes: &mut umber_core::Axes,
        categories: Vec<String>,
        series: Vec<BarSeries>,
    ) -> umber_core::Result<&mut umber_core::Axes> {
        match self {
            Group::Grouped => axes.grouped_bar(categories, series),
            Group::Stacked => axes.stacked_bar(categories, series),
        }
    }
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let regions = names(&["North", "East", "South", "West", "Central"]);
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme, ScaleKind::Category, linear());
        labelled(&mut f, "Bars");
        axes(&mut f)
            .bar(regions.clone(), &[12.0, 31.5, 22.0, 8.25, 27.0], bar())
            .unwrap();
        emit_scene(&format!("fig-bars-{theme_name}"), &f.to_scene(lib), sink);
    }

    // Every bar below 0: the domain ends at 0, the bars grow downwards from the baseline at the
    // top and round their bottom end.
    for (theme_name, theme) in [
        ("pigment", Theme::pigment()),
        ("midnight", Theme::midnight()),
    ] {
        let mut f = figure(6.0, 4.0, theme, ScaleKind::Category, linear());
        labelled(&mut f, "Negative bars");
        axes(&mut f)
            .bar(regions.clone(), &[-12.0, -31.5, -22.0, -8.25, -27.0], bar())
            .unwrap();
        emit_scene(
            &format!("fig-bars-negative-{theme_name}"),
            &f.to_scene(lib),
            sink,
        );
    }

    // Both signs, with NaN and 0, which draw nothing.
    let mut f = figure(6.0, 4.0, Theme::midnight(), ScaleKind::Category, linear());
    labelled(&mut f, "Mixed signs");
    axes(&mut f)
        .bar(
            names(&["a", "b", "c", "d", "e", "f"]),
            &[14.0, -6.5, f64::NAN, 0.0, -0.0, 3.0],
            bar(),
        )
        .unwrap();
    emit_scene("fig-bars-mixed-nan-zero", &f.to_scene(lib), sink);

    // A huge height under limits 1e-10 wide: clipped in axis space, cut square above the plot.
    let mut f = figure(6.0, 4.0, Theme::midnight(), ScaleKind::Category, linear());
    axes(&mut f)
        .bar(
            names(&["huge", "inside", "below"]),
            &[1e300, 5e-11, -1e300],
            bar(),
        )
        .unwrap()
        .ylim(1e-11, 1e-10)
        .unwrap();
    emit_scene("fig-bars-huge-narrow-limits", &f.to_scene(lib), sink);

    // Log y: bars grow from the bottom of the plot.
    for (theme_name, theme) in [
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        let mut f = figure(6.0, 4.0, theme, ScaleKind::Category, log(10.0));
        labelled(&mut f, "Bars on a log axis");
        axes(&mut f)
            .bar(regions.clone(), &[3.0, 420.0, 18.0, 9500.0, 61.0], bar())
            .unwrap();
        emit_scene(
            &format!("fig-bars-log-y-{theme_name}"),
            &f.to_scene(lib),
            sink,
        );
    }
    let mut f = figure(6.0, 4.0, Theme::graphite(), ScaleKind::Category, log(10.0));
    axes(&mut f)
        .bar(
            names(&["huge", "tiny", "inside"]),
            &[1e300, 1e-300, 5.0],
            bar(),
        )
        .unwrap()
        .ylim(1.0, 10.0)
        .unwrap();
    emit_scene("fig-bars-log-y-limits", &f.to_scene(lib), sink);

    // Categories shared with lines and between two bar marks, which overlap in insertion
    // order. The first line sets the category order; the bars miss Q1 and Q3 and add Q5. The
    // first line is drawn under the bars, the second, added after them, over them.
    let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    labelled(&mut f, "Bars and lines");
    let quarters = |v: &[&str]| AxisData::Categories(names(v));
    axes(&mut f)
        .line(
            quarters(&["Q1", "Q2", "Q3", "Q4"]),
            &[10.0, 16.0, 21.0, 19.0],
            line(),
        )
        .unwrap()
        .bar(names(&["Q2", "Q4", "Q5"]), &[18.0, 25.0, 9.0], bar())
        .unwrap()
        .bar(
            names(&["Q4", "Q2"]),
            &[12.0, 7.5],
            BarOptions {
                color: ColorSpec::Palette {
                    index: 4,
                    alpha: umber_core::Alpha::new(0.7).unwrap(),
                },
                ..bar()
            },
        )
        .unwrap()
        .line(quarters(&["Q2", "Q3", "Q5"]), &[4.0, 11.0, 14.0], line())
        .unwrap();
    emit_scene("fig-bars-with-category-lines", &f.to_scene(lib), sink);

    sink(Output {
        kind: "errors",
        file: "bars.txt".into(),
        bytes: bar_errors().into_bytes(),
    });

    group_cases(lib, sink);
}

/// The figures of grouped and stacked bars, then their errors.
fn group_cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    let regions = names(&["North", "East", "South", "West", "Central"]);
    let three = || {
        vec![
            series(&[12.0, 31.5, 22.0, 8.25, 27.0]),
            series(&[9.0, 14.0, 18.5, 11.0, 6.0]),
            series(&[4.5, 10.0, 7.0, 15.5, 12.0]),
        ]
    };
    for group in Group::BOTH {
        let kind = group.name();
        for (theme_name, theme) in themes() {
            let mut f = figure(6.0, 4.0, theme, ScaleKind::Category, linear());
            labelled(&mut f, &format!("{} bars", capitalized(kind)));
            group.add(axes(&mut f), regions.clone(), three()).unwrap();
            emit_scene(
                &format!("fig-{kind}-bars-{theme_name}"),
                &f.to_scene(lib),
                sink,
            );
        }

        // Both signs, with a NaN segment in the middle series and a 0, which add nothing: a
        // stack continues from the segment below the gap.
        for (theme_name, theme) in [
            ("pigment", Theme::pigment()),
            ("midnight", Theme::midnight()),
        ] {
            let mut f = figure(6.0, 4.0, theme, ScaleKind::Category, linear());
            labelled(&mut f, "Mixed signs");
            group
                .add(
                    axes(&mut f),
                    names(&["a", "b", "c", "d"]),
                    vec![
                        series(&[14.0, -6.5, 8.0, -3.0]),
                        series(&[f64::NAN, -4.0, 0.0, 5.0]),
                        series(&[6.0, 3.0, -9.0, -2.5]),
                    ],
                )
                .unwrap();
            emit_scene(
                &format!("fig-{kind}-bars-mixed-signs-{theme_name}"),
                &f.to_scene(lib),
                sink,
            );
        }

        // One series: the same geometry as a plain bar.
        let mut f = figure(6.0, 4.0, Theme::graphite(), ScaleKind::Category, linear());
        group
            .add(
                axes(&mut f),
                regions.clone(),
                vec![series(&[3.0, 5.0, -2.0, 4.0, 1.0])],
            )
            .unwrap();
        emit_scene(
            &format!("fig-{kind}-bars-one-series"),
            &f.to_scene(lib),
            sink,
        );

        // Twelve series: the palette wraps; grouped bars in a narrow band touch.
        let mut f = figure(6.0, 4.0, Theme::midnight(), ScaleKind::Category, linear());
        labelled(&mut f, "Twelve series");
        let twelve: Vec<BarSeries> = (0..12)
            .map(|j| {
                let j = f64::from(j);
                series(&[1.0 + j, 12.0 - j, 2.0 + (j * 7.0) % 5.0, 3.0])
            })
            .collect();
        group
            .add(axes(&mut f), names(&["Q1", "Q2", "Q3", "Q4"]), twelve)
            .unwrap();
        emit_scene(
            &format!("fig-{kind}-bars-12-series"),
            &f.to_scene(lib),
            sink,
        );

        // Fixed and palette colors between automatic ones, which take slots 0, 1 and 2.
        let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
        group
            .add(
                axes(&mut f),
                names(&["a", "b", "c"]),
                vec![
                    series(&[4.0, 6.0, 3.0]),
                    colored(ColorSpec::Fixed(Rgba::opaque(20, 20, 20)), &[2.0, 3.0, 5.0]),
                    series(&[5.0, 1.0, 2.0]),
                    colored(
                        ColorSpec::Palette {
                            index: 0,
                            alpha: Alpha::new(0.5).unwrap(),
                        },
                        &[3.0, 4.0, 4.0],
                    ),
                    series(&[1.0, 2.0, 6.0]),
                ],
            )
            .unwrap();
        emit_scene(&format!("fig-{kind}-bars-colors"), &f.to_scene(lib), sink);

        // Log y: bars and the first segment of each stack grow from the bottom of the plot.
        for (theme_name, theme) in [
            ("graphite", Theme::graphite()),
            ("midnight", Theme::midnight()),
        ] {
            let mut f = figure(6.0, 4.0, theme, ScaleKind::Category, log(10.0));
            labelled(&mut f, "Bars on a log axis");
            group
                .add(
                    axes(&mut f),
                    regions.clone(),
                    vec![
                        series(&[3.0, 420.0, 18.0, 9500.0, 61.0]),
                        series(&[30.0, 80.0, f64::NAN, 2000.0, 5.0]),
                    ],
                )
                .unwrap();
            emit_scene(
                &format!("fig-{kind}-bars-log-y-{theme_name}"),
                &f.to_scene(lib),
                sink,
            );
        }

        // Huge heights under limits 1e-10 wide: clipped in axis space.
        let mut f = figure(6.0, 4.0, Theme::midnight(), ScaleKind::Category, linear());
        group
            .add(
                axes(&mut f),
                names(&["up", "down", "inside"]),
                vec![
                    series(&[5e299, -5e299, 2e-11]),
                    series(&[5e299, -5e299, 3e-11]),
                ],
            )
            .unwrap()
            .ylim(1e-11, 1e-10)
            .unwrap();
        emit_scene(
            &format!("fig-{kind}-bars-huge-narrow-limits"),
            &f.to_scene(lib),
            sink,
        );
    }

    // Stacked totals exactly at the bound, on both sides.
    let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    axes(&mut f)
        .stacked_bar(
            names(&["a", "b"]),
            vec![series(&[6e299, -7.5e299]), series(&[4e299, -2.5e299])],
        )
        .unwrap();
    emit_scene("fig-stacked-bars-totals-at-bound", &f.to_scene(lib), sink);

    sink(Output {
        kind: "errors",
        file: "grouped-bars.txt".into(),
        bytes: group_errors().into_bytes(),
    });
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
        .unwrap_or_default()
}

/// Every error branch of `Axes::grouped_bar` and `Axes::stacked_bar`, in the order their
/// checks run, and the state after each call.
fn group_errors() -> String {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let mut rec = Log::default();
    let list =
        |heights: &[&[f64]]| -> Vec<BarSeries> { heights.iter().map(|h| series(h)).collect() };

    for group in Group::BOTH {
        let kind = group.name();
        rec.section(&format!("Axes.{kind}_bar, category/linear"));
        let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
        rec.on_axes(&mut f, &format!("{kind} ok"), |a| {
            group
                .add(
                    a,
                    names(&["a", "b"]),
                    vec![
                        BarSeries {
                            label: "kept".into(),
                            heights: vec![1.0, -2.0],
                            color: ColorSpec::Fixed(Rgba::opaque(1, 2, 3)),
                        },
                        series(&[f64::from_bits(0x7ff8_0000_0000_0001), -0.0]),
                    ],
                )
                .map(drop)
        });
        rec.on_axes(&mut f, &format!("{kind} no categories"), |a| {
            group.add(a, Vec::new(), list(&[&[], &[]])).map(drop)
        });
        let cases: Vec<(&str, Vec<String>, Vec<BarSeries>)> = vec![
            ("empty series", names(&["a"]), Vec::new()),
            (
                "empty series before duplicates",
                names(&["a", "a"]),
                Vec::new(),
            ),
            (
                "series[1] too short",
                names(&["a", "b"]),
                list(&[&[1.0, 2.0], &[1.0], &[inf]]),
            ),
            (
                "series[0] too long before duplicates",
                names(&["a", "a"]),
                list(&[&[1.0, 2.0, 3.0]]),
            ),
            (
                "duplicate category before infinite height",
                names(&["x", "y", "x"]),
                list(&[&[inf, inf, inf]]),
            ),
            (
                "infinite height in series[1]",
                names(&["a", "b"]),
                list(&[&[1.0, nan], &[2.0, inf]]),
            ),
            (
                "huge height in series[0] before series[1]",
                names(&["a"]),
                list(&[&[-1e301], &[inf]]),
            ),
            (
                "positive totals beyond the bound",
                names(&["a", "b"]),
                list(&[&[1.0, 6e299], &[2.0, 5e299]]),
            ),
            (
                "negative totals beyond the bound",
                names(&["a"]),
                list(&[&[-1e300], &[1e300], &[-1e299]]),
            ),
        ];
        for (name, categories, series) in cases {
            rec.on_axes(&mut f, &format!("{kind} {name}"), |a| {
                group.add(a, categories, series).map(drop)
            });
        }

        rec.section(&format!("Axes.{kind}_bar, linear/linear"));
        let mut f = rec.new_figure("linear/linear", linear(), linear());
        rec.on_axes(&mut f, &format!("{kind} on a number x axis"), |a| {
            group
                .add(a, names(&["a", "a"]), list(&[&[1.0, 2.0]]))
                .map(drop)
        });

        rec.section(&format!("Axes.{kind}_bar, category/log"));
        let mut f = rec.new_figure("category/log", ScaleKind::Category, log(10.0));
        rec.on_axes(&mut f, &format!("{kind} ok"), |a| {
            group
                .add(a, names(&["a"]), list(&[&[1e-300], &[5e299], &[5e299]]))
                .map(drop)
        });
        rec.on_axes(&mut f, &format!("{kind} zero height in series[1]"), |a| {
            group
                .add(a, names(&["a"]), list(&[&[nan], &[0.0]]))
                .map(drop)
        });
        rec.on_axes(&mut f, &format!("{kind} negative height"), |a| {
            group.add(a, names(&["a"]), list(&[&[-3.0]])).map(drop)
        });
        rec.on_axes(
            &mut f,
            &format!("{kind} height below the log range in series[1]"),
            |a| {
                group
                    .add(a, names(&["a", "b"]), list(&[&[1.0, 2.0], &[3.0, 1e-301]]))
                    .map(drop)
            },
        );

        rec.section(&format!("Axes.{kind}_bar, category/category"));
        let mut f = rec.new_figure(
            "category/category",
            ScaleKind::Category,
            ScaleKind::Category,
        );
        rec.on_axes(&mut f, &format!("{kind} on a category y axis"), |a| {
            group.add(a, names(&["a"]), list(&[&[1.0]])).map(drop)
        });
        rec.on_axes(
            &mut f,
            &format!("{kind} heights summing past the bound on a category y axis"),
            |a| {
                group
                    .add(a, names(&["a"]), list(&[&[1e300], &[1e300]]))
                    .map(drop)
            },
        );
    }
    rec.out
}

/// Every error branch of `Axes::bar`, in the order its checks run, and the state after each
/// call.
fn bar_errors() -> String {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let mut rec = Log::default();

    rec.section("Axes.bar, category/linear");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    rec.on_axes(&mut f, "bar ok", |a| {
        a.bar(names(&["a", "b"]), &[1.0, -2.0], bar()).map(drop)
    });
    rec.on_axes(&mut f, "bar empty", |a| {
        a.bar(Vec::new(), &[], bar()).map(drop)
    });
    // Non-canonical NaN payloads and signed zeros are stored bit for bit.
    let gaps = [
        f64::from_bits(0x7ff8_0000_0000_0001),
        f64::from_bits(0xfff0_0000_dead_beef),
    ];
    rec.on_axes(&mut f, "bar NaN and signed zeros", |a| {
        a.bar(
            names(&["a", "c", "d", "e"]),
            &[gaps[0], -0.0, 0.0, gaps[1]],
            BarOptions {
                label: "kept".into(),
                color: ColorSpec::Fixed(umber_core::Rgba::opaque(1, 2, 3)),
            },
        )
        .map(drop)
    });
    let cases: Vec<(&str, Vec<String>, Vec<f64>)> = vec![
        ("length mismatch", names(&["a"]), vec![1.0, 2.0]),
        (
            "length mismatch, duplicate and infinite",
            names(&["a", "a"]),
            vec![inf],
        ),
        ("no categories, one height", Vec::new(), vec![1.0]),
        (
            "duplicate category",
            names(&["a", "b", "a"]),
            vec![1.0, 2.0, 3.0],
        ),
        (
            "duplicate category before infinite height",
            names(&["x", "x"]),
            vec![inf, inf],
        ),
        (
            "duplicate empty name",
            names(&["", "b", ""]),
            vec![1.0, 2.0, 3.0],
        ),
        (
            "duplicate name with quotes",
            names(&["say \"hi\"", "say \"hi\""]),
            vec![1.0, 2.0],
        ),
        ("infinite height", names(&["a", "b"]), vec![1.0, inf]),
        (
            "negative infinite height",
            names(&["a"]),
            vec![f64::NEG_INFINITY],
        ),
        ("huge height", names(&["a"]), vec![1.0e301]),
        ("huge negative height", names(&["a"]), vec![-1.0e301]),
        (
            "huge height[1] before infinite height[2]",
            names(&["a", "b", "c"]),
            vec![nan, 1.0e301, inf],
        ),
    ];
    for (name, categories, heights) in cases {
        rec.on_axes(&mut f, &format!("bar {name}"), |a| {
            a.bar(categories, &heights, bar()).map(drop)
        });
    }

    rec.section("Axes.bar, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "bar on a number x axis", |a| {
        a.bar(names(&["a"]), &[1.0], bar()).map(drop)
    });
    rec.on_axes(&mut f, "bar on a number x axis, duplicate", |a| {
        a.bar(names(&["a", "a"]), &[1.0, 2.0], bar()).map(drop)
    });
    rec.on_axes(&mut f, "bar on a number x axis, length mismatch", |a| {
        a.bar(names(&["a"]), &[], bar()).map(drop)
    });

    rec.section("Axes.bar, category/log");
    let mut f = rec.new_figure("category/log", ScaleKind::Category, log(10.0));
    rec.on_axes(&mut f, "bar ok", |a| {
        a.bar(names(&["a", "b"]), &[1e-300, 1e300], bar()).map(drop)
    });
    for (name, height) in [
        ("zero height", 0.0),
        ("negative zero height", -0.0),
        ("negative height", -3.0),
        ("height below the log range", 1e-301),
        ("height above the log range", 1e301),
    ] {
        rec.on_axes(&mut f, &format!("bar {name}"), |a| {
            a.bar(names(&["a", "b"]), &[nan, height], bar()).map(drop)
        });
    }
    rec.on_axes(&mut f, "bar duplicate before a zero height", |a| {
        a.bar(names(&["a", "a"]), &[0.0, 0.0], bar()).map(drop)
    });

    rec.section("Axes.bar, category/category");
    let mut f = rec.new_figure(
        "category/category",
        ScaleKind::Category,
        ScaleKind::Category,
    );
    rec.on_axes(&mut f, "bar on a category y axis", |a| {
        a.bar(names(&["a"]), &[1.0], bar()).map(drop)
    });
    rec.on_axes(&mut f, "bar on a category y axis, infinite height", |a| {
        a.bar(names(&["a"]), &[inf], bar()).map(drop)
    });
    rec.out
}
