//! Output-gate cases for reference lines and spans.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Scenes, in the three built-in themes: a labelled line with a dashed horizontal rule, a
//! vertical rule and a horizontal and a vertical span, which Graphite labels directly for the
//! line and in the legend row for the references. In single themes: marks called in an order
//! unlike their layers; references on a category x axis, at a band centre; on log axes; against
//! explicit limits, with values at a limit, spans that cross one and spans that cover the
//! whole view; references alone, which pad the axes; styles, widths and colors; each legend
//! style; references on a grid.
//!
//! In `errors/rules.txt`: every error branch of the four methods in the order the checks run,
//! the state after each failed call, and the failures of rendering: a vertical rule at an
//! unknown category in each of the three render calls, and on a grid.

use umber_core::figure::{
    AxisData, BarOptions, LineOptions, RuleOptions, ScatterOptions, SpanOptions,
};
use umber_core::{
    Alpha, AxisValue, ColorSpec, Dash, Figure, FontLibrary, Grid, GridIndex, LegendStyle,
    Placement, Rgba, ScaleKind, SizeSpec, SvgText, Theme,
};

use super::super::{Log, Output, axes, emit_scene, figure, labelled, linear, log, themes, wobble};

fn palette(index: usize) -> ColorSpec {
    ColorSpec::Palette {
        index,
        alpha: Alpha::new(1.0).unwrap(),
    }
}

fn rule(label: &str) -> RuleOptions {
    RuleOptions {
        label: label.into(),
        ..RuleOptions::default()
    }
}

fn span(label: &str) -> SpanOptions {
    SpanOptions {
        label: label.into(),
        ..SpanOptions::default()
    }
}

fn at(x: f64) -> AxisValue {
    AxisValue::Number(x)
}

fn category(name: &str) -> AxisValue {
    AxisValue::Category(name.into())
}

fn names(list: &[&str]) -> AxisData {
    AxisData::Categories(list.iter().map(|s| (*s).to_owned()).collect())
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib).unwrap(), sink);
    }
    sink(Output {
        kind: "errors",
        file: "rules.txt".into(),
        bytes: rules_errors(lib).into_bytes(),
    });
}

/// A labelled signal over 0..=60 with a slow wave and a deterministic wobble.
fn signal() -> (Vec<f64>, Vec<f64>) {
    let (_, noise) = wobble(241, 7);
    let t: Vec<f64> = (0..241).map(|i| f64::from(i) * 0.25).collect();
    let y = t
        .iter()
        .zip(&noise)
        .map(|(t, n)| 50.0 + 12.0 * (t / 7.0).sin() + n)
        .collect();
    (t, y)
}

/// Every reference figure, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();

    // The reference board: a line, a threshold, an event, a normal range and a window.
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Reference lines and spans");
        let (t, y) = signal();
        axes(&mut f)
            .line(
                AxisData::Numbers(t),
                &y,
                LineOptions {
                    label: "Signal".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .hline(
                66.0,
                RuleOptions {
                    dash: Dash::Dashed,
                    ..rule("Threshold")
                },
            )
            .unwrap()
            .vline(at(31.0), rule("Event"))
            .unwrap()
            .hspan(42.0, 58.0, span("Normal range"))
            .unwrap()
            .vspan(40.0, 50.0, span("Window"))
            .unwrap();
        out.push((format!("rules-reference-{theme_name}"), f));
    }

    // Called in the order rule, span, line, scatter; drawn spans, line and scatter, rules.
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    labelled(&mut f, "Layers");
    let x: Vec<f64> = (0..12).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|v| 3.0 + (v * 0.7).sin() * 2.0).collect();
    axes(&mut f)
        .vline(at(4.0), rule("Rule"))
        .unwrap()
        .hspan(2.0, 4.0, span("Span"))
        .unwrap()
        .line(
            AxisData::Numbers(x.clone()),
            &y,
            LineOptions {
                label: "Line".into(),
                ..LineOptions::default()
            },
        )
        .unwrap()
        .scatter(
            AxisData::Numbers(x),
            &y,
            ScatterOptions {
                label: "Points".into(),
                ..ScatterOptions::default()
            },
        )
        .unwrap()
        .hline(3.0, rule("Mean"))
        .unwrap();
    out.push(("rules-layers".to_owned(), f));

    // A category x axis: rules at band centres, a horizontal span over the bars.
    for (theme_name, theme) in [
        ("pigment", Theme::pigment()),
        ("graphite", Theme::graphite()),
    ] {
        let mut f = figure(6.0, 4.0, theme, ScaleKind::Category, linear());
        labelled(&mut f, "Quarters");
        axes(&mut f)
            .bar(
                ["Q1", "Q2", "Q3", "Q4"].map(String::from).to_vec(),
                &[4.0, 6.0, 5.0, 8.0],
                BarOptions {
                    label: "Revenue".into(),
                    ..BarOptions::default()
                },
            )
            .unwrap()
            .vline(category("Q3"), rule("Launch"))
            .unwrap()
            .vline(category("Q1"), rule(""))
            .unwrap()
            .hline(
                5.5,
                RuleOptions {
                    dash: Dash::Dotted,
                    ..rule("Target")
                },
            )
            .unwrap()
            .hspan(3.0, 4.0, span("Floor"))
            .unwrap();
        out.push((format!("rules-category-{theme_name}"), f));
    }
    // A scatter on a category axis, a rule at a category added by a later mark.
    let mut f = figure(6.0, 4.0, Theme::midnight(), ScaleKind::Category, linear());
    axes(&mut f)
        .vline(category("Late"), rule("Late"))
        .unwrap()
        .scatter(
            names(&["Early", "Mid"]),
            &[1.0, 3.0],
            ScatterOptions::default(),
        )
        .unwrap()
        .scatter(names(&["Late"]), &[2.0], ScatterOptions::default())
        .unwrap();
    out.push(("rules-category-added-later".to_owned(), f));

    // Log axes: positions in log space, a span between two decades.
    let mut f = figure(6.0, 4.0, Theme::pigment(), log(10.0), log(10.0));
    labelled(&mut f, "Log axes");
    axes(&mut f)
        .line(
            AxisData::Numbers(vec![1.0, 10.0, 100.0, 1000.0]),
            &[2.0, 20.0, 40.0, 800.0],
            LineOptions::default(),
        )
        .unwrap()
        .vspan(10.0, 100.0, span("Decades"))
        .unwrap()
        .hline(100.0, rule("Cap"))
        .unwrap()
        .vline(at(300.0), rule(""))
        .unwrap()
        .hspan(1.0, 5.0, span(""))
        .unwrap();
    out.push(("rules-log-axes".to_owned(), f));

    // Explicit limits: a rule at each limit, one just inside the stroke's reach and one far
    // outside; spans that cross a limit, cover the view, end at a limit and lie outside it.
    for (theme_name, theme) in [
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Explicit limits");
        axes(&mut f)
            .hline(0.0, rule("At low"))
            .unwrap()
            .hline(10.0, rule("At high"))
            .unwrap()
            .hline(10.1, rule("Past high"))
            .unwrap()
            .hline(50.0, rule("Far past"))
            .unwrap()
            .vline(at(0.0), rule(""))
            .unwrap()
            .vline(at(12.0), rule(""))
            .unwrap()
            .vspan(-4.0, 1.5, span("Crosses low"))
            .unwrap()
            .vspan(8.0, 14.0, span("Crosses high"))
            .unwrap()
            .hspan(-5.0, 15.0, span("Covers"))
            .unwrap()
            .hspan(10.0, 20.0, span("Starts at high"))
            .unwrap()
            .hspan(-3.0, 0.0, span("Ends at low"))
            .unwrap()
            .vspan(20.0, 30.0, span("Outside"))
            .unwrap()
            .xlim(0.0, 10.0)
            .unwrap()
            .ylim(0.0, 10.0)
            .unwrap();
        out.push((format!("rules-explicit-limits-{theme_name}"), f));
    }

    // References alone: the axes are padded, not fitted to the references.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut f, "References alone");
    axes(&mut f)
        .hspan(20.0, 40.0, span("Band"))
        .unwrap()
        .vspan(2.0, 6.0, span("Window"))
        .unwrap()
        .hline(50.0, rule("Top"))
        .unwrap();
    out.push(("rules-alone".to_owned(), f));
    // A single rule alone, one value on each axis.
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    axes(&mut f)
        .hline(3.0, RuleOptions::default())
        .unwrap()
        .vline(at(3.0), RuleOptions::default())
        .unwrap();
    out.push(("rules-single-values".to_owned(), f));

    // Styles: dashes, a thick rule, palette and fixed colors, and a repeated label.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut f, "Styles");
    axes(&mut f)
        .hline(
            2.0,
            RuleOptions {
                dash: Dash::Dashed,
                width: SizeSpec::times(2.0).unwrap(),
                color: palette(3),
                ..rule("Dashed, thick")
            },
        )
        .unwrap()
        .hline(
            4.0,
            RuleOptions {
                dash: Dash::Dotted,
                ..rule("Dotted")
            },
        )
        .unwrap()
        .vline(
            at(3.0),
            RuleOptions {
                width: SizeSpec::pt(0.5).unwrap(),
                color: ColorSpec::Fixed(Rgba {
                    r: 200,
                    g: 40,
                    b: 40,
                    a: 255,
                }),
                ..rule("Fine, fixed")
            },
        )
        .unwrap()
        .vspan(
            5.0,
            7.0,
            SpanOptions {
                color: palette(5),
                ..span("Palette span")
            },
        )
        .unwrap()
        .vspan(
            8.0,
            9.0,
            SpanOptions {
                color: ColorSpec::Auto(Alpha::new(0.5).unwrap()),
                ..span("Half alpha")
            },
        )
        .unwrap()
        .hline(6.0, rule("Dotted"))
        .unwrap()
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 8.0)
        .unwrap();
    out.push(("rules-styles".to_owned(), f));

    // Each legend style over the same labelled references.
    for (name, style) in [
        ("header", LegendStyle::Header),
        ("chips", LegendStyle::Chips),
        ("direct", LegendStyle::Direct),
        ("none", LegendStyle::None),
    ] {
        let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
        labelled(&mut f, "Legend styles");
        let (t, y) = signal();
        axes(&mut f)
            .line(
                AxisData::Numbers(t),
                &y,
                LineOptions {
                    label: "Signal".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .hline(66.0, rule("Threshold"))
            .unwrap()
            .vspan(40.0, 50.0, span("Window"))
            .unwrap()
            .legend(style, Default::default())
            .unwrap();
        out.push((format!("rules-legend-{name}"), f));
    }

    // Several axes: rules on a log panel and spans on a linear one, each in its own cell.
    let grid = Grid::new(1, 2).unwrap();
    let (width, height) = grid.auto_size_in();
    let mut f = Figure::with_grid(width, height, Theme::midnight(), grid).unwrap();
    for col in 0..2_i64 {
        let at_cell = Placement {
            row: GridIndex::At(0),
            col: GridIndex::At(col),
            row_span: 1,
            col_span: 1,
        };
        let y_scale = if col == 1 { log(10.0) } else { linear() };
        let mut cell = f.add_axes_at(at_cell, linear(), y_scale).unwrap();
        cell.title(if col == 1 { "Log" } else { "Linear" })
            .line(
                AxisData::Numbers(vec![0.0, 5.0, 10.0]),
                &[2.0, 20.0, 9.0],
                LineOptions {
                    label: "Level".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .hline(10.0, rule("Limit"))
            .unwrap()
            .vspan(3.0, 6.0, span("Window"))
            .unwrap();
    }
    out.push(("rules-grid".to_owned(), f));

    out
}

/// Every error branch of the four methods, in the order their checks run, the state after each
/// call, and what rendering refuses.
fn rules_errors(lib: &FontLibrary) -> String {
    let (inf, nan) = (f64::INFINITY, f64::NAN);
    let mut rec = Log::default();

    rec.section("Axes.hline, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "hline ok", |a| {
        a.hline(66.0, rule("Threshold")).map(drop)
    });
    for (name, y) in [
        ("infinite", inf),
        ("negative infinite", f64::NEG_INFINITY),
        ("NaN", nan),
        ("huge", 1.0e301),
        ("huge negative", -1.0e301),
        ("at the largest magnitude", umber_core::MAX_MAGNITUDE),
        ("negative zero", -0.0),
    ] {
        rec.on_axes(&mut f, &format!("hline {name}"), |a| {
            a.hline(y, RuleOptions::default()).map(drop)
        });
    }
    rec.on_axes(&mut f, "hline with options", |a| {
        a.hline(
            1.0,
            RuleOptions {
                label: "edge".into(),
                color: ColorSpec::Palette {
                    index: 21,
                    alpha: Alpha::new(0.25).unwrap(),
                },
                dash: Dash::Dashed,
                width: SizeSpec::pt(3.0).unwrap(),
            },
        )
        .map(drop)
    });

    rec.section("Axes.hline, linear/log10");
    let mut f = rec.new_figure("linear/log10", linear(), log(10.0));
    for (name, y) in [
        ("zero", 0.0),
        ("negative", -1.0),
        ("below the log minimum", 1.0e-301),
        ("positive", 0.5),
    ] {
        rec.on_axes(&mut f, &format!("hline {name}"), |a| {
            a.hline(y, RuleOptions::default()).map(drop)
        });
    }

    rec.section("Axes.vline, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    for (name, x) in [
        ("ok", at(31.0)),
        ("infinite", at(inf)),
        ("negative infinite", at(f64::NEG_INFINITY)),
        ("NaN", at(nan)),
        ("huge", at(1.0e301)),
        ("a category name", category("Q3")),
    ] {
        rec.on_axes(&mut f, &format!("vline {name}"), |a| {
            a.vline(x, rule("Event")).map(drop)
        });
    }

    rec.section("Axes.vline, category/linear");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    for (name, x) in [
        ("a name", category("Q3")),
        ("an empty name", category("")),
        ("a number", at(1.0)),
        ("a NaN", at(nan)),
    ] {
        rec.on_axes(&mut f, &format!("vline {name}"), |a| {
            a.vline(x, RuleOptions::default()).map(drop)
        });
    }

    rec.section("Axes.vline, log2/linear");
    let mut f = rec.new_figure("log2/linear", log(2.0), linear());
    for (name, x) in [("zero", 0.0), ("negative", -3.0), ("positive", 8.0)] {
        rec.on_axes(&mut f, &format!("vline {name}"), |a| {
            a.vline(at(x), RuleOptions::default()).map(drop)
        });
    }

    rec.section("Axes.hspan and Axes.vspan, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    let cases: Vec<(&str, f64, f64)> = vec![
        ("ok", 42.0, 58.0),
        ("low above high", 5.0, 2.0),
        ("low equal to high", 2.0, 2.0),
        ("infinite low", inf, 1.0),
        ("infinite high", 1.0, inf),
        ("NaN low", nan, 1.0),
        ("NaN high", 1.0, nan),
        ("NaN both", nan, nan),
        ("infinite low and swapped", inf, -1.0),
        ("swapped large", 1.0e7, 1.0e6),
        ("huge low", -1.0e301, 0.0),
        ("huge high", 0.0, 1.0e301),
        ("tiny span", 0.0, 5.0e-324),
    ];
    for (name, low, high) in cases {
        rec.on_axes(&mut f, &format!("hspan {name}"), |a| {
            a.hspan(low, high, span("Range")).map(drop)
        });
        rec.on_axes(&mut f, &format!("vspan {name}"), |a| {
            a.vspan(low, high, span("Window")).map(drop)
        });
    }

    rec.section("Axes.hspan and Axes.vspan, category/log10");
    let mut f = rec.new_figure("category/log10", ScaleKind::Category, log(10.0));
    for (name, low, high) in [
        ("ok", 1.0, 10.0),
        ("zero low", 0.0, 10.0),
        ("negative high", 1.0, -10.0),
        ("below the log minimum", 1.0e-301, 1.0),
    ] {
        rec.on_axes(&mut f, &format!("hspan {name}"), |a| {
            a.hspan(low, high, SpanOptions::default()).map(drop)
        });
        // A category x axis refuses a vspan before it looks at the values.
        rec.on_axes(&mut f, &format!("vspan {name}"), |a| {
            a.vspan(low, high, SpanOptions::default()).map(drop)
        });
    }

    rec.section("Axes.vspan, log10/linear");
    let mut f = rec.new_figure("log10/linear", log(10.0), linear());
    for (name, low, high) in [
        ("ok", 1.0, 10.0),
        ("zero low", 0.0, 10.0),
        ("negative low", -2.0, -1.0),
        ("below the log minimum", 1.0e-301, 1.0),
    ] {
        rec.on_axes(&mut f, &format!("vspan {name}"), |a| {
            a.vspan(low, high, SpanOptions::default()).map(drop)
        });
    }

    // The categories of a vertical rule are looked up when the figure is rendered.
    rec.section("Figure rendering, vline at a category");
    let build = |name: &str, extra: Option<&str>| {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        {
            let mut a = f.add_axes(ScaleKind::Category, linear()).unwrap();
            a.scatter(names(&["Q1", "Q2"]), &[1.0, 2.0], ScatterOptions::default())
                .unwrap()
                .vline(category(name), RuleOptions::default())
                .unwrap();
            if let Some(extra) = extra {
                a.scatter(names(&[extra]), &[3.0], ScatterOptions::default())
                    .unwrap();
            }
        }
        f
    };
    let known = build("Q2", None);
    rec.result("to_scene, known category", known.to_scene(lib), |_| {
        String::new()
    });
    rec.result("to_png, known category", known.to_png(72), |_| {
        String::new()
    });
    rec.result(
        "to_svg, known category",
        known.to_svg(SvgText::Editable),
        |_| String::new(),
    );
    let unknown = build("Q5", None);
    rec.result("to_scene, unknown category", unknown.to_scene(lib), |_| {
        String::new()
    });
    rec.result("to_png, unknown category", unknown.to_png(72), |_| {
        String::new()
    });
    rec.result(
        "to_svg, unknown category",
        unknown.to_svg(SvgText::Editable),
        |_| String::new(),
    );
    rec.raw(&super::super::record::figure(&unknown));
    let brought_later = build("Q5", Some("Q5"));
    rec.result(
        "to_scene, category brought by a later mark",
        brought_later.to_scene(lib),
        |_| String::new(),
    );
    let long = build(&"Q".repeat(60), None);
    rec.result("to_scene, long unknown name", long.to_scene(lib), |_| {
        String::new()
    });

    // On a grid, the message names the axes' cell.
    let grid = Grid::new(1, 2).unwrap();
    let (width, height) = grid.auto_size_in();
    let mut f = Figure::with_grid(width, height, Theme::pigment(), grid).unwrap();
    let cell = |col| Placement {
        row: GridIndex::At(0),
        col: GridIndex::At(col),
        row_span: 1,
        col_span: 1,
    };
    f.add_axes_at(cell(0), linear(), linear()).unwrap();
    f.add_axes_at(cell(1), ScaleKind::Category, linear())
        .unwrap()
        .vline(category("Q5"), RuleOptions::default())
        .unwrap();
    rec.result(
        "to_scene, unknown category on a grid",
        f.to_scene(lib),
        |_| String::new(),
    );
    rec.out
}
