//! Dividers between categories and category names of several lines.
//!
//! Scenes, in the three built-in themes: yearly bars whose last category name has two lines,
//! with a dashed divider between two years and a span covering whole years. In single themes:
//! horizontal bars with row names of several lines, a divider and a span on the category y
//! axis; a highlighted category of two lines; crowded category names of two and three lines,
//! which the automatic labels thin by their whole blocks; a divider on a grid with a shared
//! category axis, between categories of different panels.
//!
//! In `errors/category-axis.txt`: every error branch of `Between` and of the methods that take
//! one, in the order the checks run, with the state after each failed call, and the failures of
//! rendering: a name that is not a category, two categories that are not neighbours, span ends
//! in the wrong order, and the same on a grid.

use umber_core::figure::{BarOptions, LineOptions, RuleOptions, ScatterOptions, SpanOptions};
use umber_core::{
    AxisData, AxisValue, Between, ColorSpec, Dash, Figure, FontLibrary, Grid, GridIndex, Placement,
    ScaleKind, Share, SvgText, Theme,
};

use super::super::{Log, Output, axes, emit_scene, figure, linear, log, themes};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|&name| name.to_owned()).collect()
}

fn between(first: &str, second: &str) -> Between {
    Between::new(first, second).unwrap()
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

/// The years of the yearly figures; the last one is partial and named on two lines.
const YEARS: [&str; 6] = ["2021", "2022", "2023", "2024", "2025", "2026\n(to Oct)"];

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib).unwrap(), sink);
    }
    sink(Output {
        kind: "errors",
        file: "category-axis.txt".into(),
        bytes: errors(lib).into_bytes(),
    });
}

/// Every figure, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();

    // Yearly bars, a divider between two years and a span covering whole years.
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme, ScaleKind::Category, linear());
        axes(&mut f)
            .title("Yearly output")
            .subtitle("Before and after the change")
            .bar(
                names(&YEARS),
                &[42.0, 45.0, 51.0, 58.0, 61.0, 49.0],
                BarOptions::default(),
            )
            .unwrap()
            .vline_between(
                between("2023", "2024"),
                RuleOptions {
                    dash: Dash::Dashed,
                    ..rule("New policy")
                },
            )
            .unwrap()
            .vspan_between(
                between("2024", "2025"),
                between("2026\n(to Oct)", "2025"),
                span("Partial year"),
            )
            .unwrap();
        out.push((format!("category-axis-divider-{theme_name}"), f));
    }

    // Horizontal bars with row names of several lines, a divider and a span on the category y
    // axis.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), ScaleKind::Category);
    axes(&mut f)
        .title("Regional sales")
        .bar(
            names(&[
                "North",
                "South\nand coast",
                "East",
                "West\nincluding\nislands",
            ]),
            &[3.0, 5.0, 2.0, 4.0],
            BarOptions::default(),
        )
        .unwrap()
        .hline_between(between("South\nand coast", "East"), rule("Split"))
        .unwrap()
        .hspan_between(
            between("North", "South\nand coast"),
            between("South\nand coast", "East"),
            SpanOptions::default(),
        )
        .unwrap();
    out.push(("category-axis-rows".to_owned(), f));

    // A highlighted category of two lines.
    let mut f = figure(6.0, 4.0, Theme::graphite(), ScaleKind::Category, linear());
    axes(&mut f)
        .bar(
            names(&YEARS),
            &[42.0, 45.0, 51.0, 58.0, 61.0, 49.0],
            BarOptions::default(),
        )
        .unwrap()
        .xtick_highlight(names(&["2026\n(to Oct)"]), ColorSpec::AUTO)
        .unwrap();
    out.push(("category-axis-highlight".to_owned(), f));

    // Crowded names of two lines on x and of three lines on y, thinned by their whole blocks.
    // A line and a dot plot keep the many categories light to draw.
    let crowded: Vec<String> = (0..40).map(|i| format!("Site {i:02}\nnorth")).collect();
    let values: Vec<f64> = (0..40).map(|i| f64::from(i % 7)).collect();
    let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    axes(&mut f)
        .line(
            AxisData::Categories(crowded),
            &values,
            LineOptions::default(),
        )
        .unwrap();
    out.push(("category-axis-crowded-x".to_owned(), f));
    let crowded: Vec<String> = (0..30).map(|i| format!("Row {i}\nA\nB")).collect();
    let values: Vec<f64> = (0..30).map(|i| f64::from(i % 5)).collect();
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), ScaleKind::Category);
    axes(&mut f)
        .scatter_shared(
            AxisData::Numbers(values),
            AxisData::Categories(crowded),
            ScatterOptions::default(),
        )
        .unwrap();
    out.push(("category-axis-crowded-y".to_owned(), f));

    // A divider on a grid with a shared category axis, between categories of two panels.
    let grid = Grid::new(1, 2).unwrap().with_share_x(Share::All);
    let mut f = Figure::with_grid(8.0, 3.5, Theme::pigment(), grid).unwrap();
    for (col, years, heights) in [
        (0, ["2021", "2022"], [3.0, 4.0]),
        (1, ["2023", "2024\n(to Oct)"], [6.0, 5.0]),
    ] {
        let mut a = f
            .add_axes_at(cell(col), ScaleKind::Category, linear())
            .unwrap();
        a.bar(names(&years), &heights, BarOptions::default())
            .unwrap();
        if col == 0 {
            a.vline_between(between("2022", "2023"), rule("Change"))
                .unwrap();
        }
    }
    out.push(("category-axis-shared-grid".to_owned(), f));

    out
}

fn cell(col: i64) -> Placement {
    Placement {
        row: GridIndex::At(0),
        col: GridIndex::At(col),
        row_span: 1,
        col_span: 1,
    }
}

/// The errors of `Between` and of the methods that take one, and of rendering such figures.
fn errors(lib: &FontLibrary) -> String {
    let mut rec = Log::default();

    rec.section("Between::new");
    for (first, second) in [
        ("2013", "2014"),
        ("2026\n(to Oct)", "2025"),
        ("2013", "2013"),
        ("a\tb", "c"),
        ("a", "b\r"),
        ("a", "b\u{2028}"),
        ("", "a"),
    ] {
        rec.result(
            &format!("Between {first:?} {second:?}"),
            Between::new(first, second),
            |b| b.to_string(),
        );
    }

    let vline =
        |a: &mut umber_core::AxesMut<'_>| a.vline_between(between("a", "b"), rule("v")).map(drop);
    let hline =
        |a: &mut umber_core::AxesMut<'_>| a.hline_between(between("a", "b"), rule("h")).map(drop);
    let vspan = |a: &mut umber_core::AxesMut<'_>| {
        a.vspan_between(between("a", "b"), between("b", "c"), span("vs"))
            .map(drop)
    };
    let hspan = |a: &mut umber_core::AxesMut<'_>| {
        a.hspan_between(between("a", "b"), between("b", "c"), span("hs"))
            .map(drop)
    };
    for (label, x, y) in [
        ("category/linear", ScaleKind::Category, linear()),
        ("linear/category", linear(), ScaleKind::Category),
        ("linear/linear", linear(), linear()),
        ("log10/log10", log(10.0), log(10.0)),
        ("time/time", ScaleKind::Time, ScaleKind::Time),
        (
            "category/category",
            ScaleKind::Category,
            ScaleKind::Category,
        ),
        ("time/category", ScaleKind::Time, ScaleKind::Category),
    ] {
        rec.section(&format!("Between on {label}"));
        for (name, call) in [
            (
                "vline",
                &vline as &dyn Fn(&mut umber_core::AxesMut<'_>) -> umber_core::Result<()>,
            ),
            ("hline", &hline),
            ("vspan", &vspan),
            ("hspan", &hspan),
        ] {
            let mut f = rec.new_figure(label, x, y);
            rec.on_axes(&mut f, name, call);
        }
    }

    rec.section("Numbers on a category axis");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    rec.on_axes(&mut f, "vspan numbers", |a| {
        a.vspan(0.0, 1.0, SpanOptions::default()).map(drop)
    });
    let mut f = rec.new_figure("linear/category", linear(), ScaleKind::Category);
    rec.on_axes(&mut f, "hline number", |a| {
        a.hline(1.0, RuleOptions::default()).map(drop)
    });
    rec.on_axes(&mut f, "hspan numbers", |a| {
        a.hspan(0.0, 1.0, SpanOptions::default()).map(drop)
    });

    rec.section("Category names of several lines");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    rec.on_axes(&mut f, "bar with a line break", |a| {
        a.bar(names(&["a", "b\nc"]), &[1.0, 2.0], BarOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "bar with a tab", |a| {
        a.bar(names(&["a\td"]), &[1.0], BarOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "vline at a name of two lines", |a| {
        a.vline(AxisValue::Category("b\nc".into()), RuleOptions::default())
            .map(drop)
    });
    rec.on_axes(&mut f, "line through a name of two lines", |a| {
        a.line(
            AxisData::Categories(names(&["b\nc"])),
            &[1.5],
            LineOptions::default(),
        )
        .map(drop)
    });
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "xticks_labeled with a line break", |a| {
        a.xticks_labeled(&[0.0], names(&["b\nc"])).map(drop)
    });

    rec.section("Rendering");
    let years = |add: &dyn Fn(&mut umber_core::AxesMut<'_>)| {
        let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
        let mut a = axes(&mut f);
        a.bar(
            names(&["2012", "2013", "2014", "2015"]),
            &[1.0, 2.0, 3.0, 4.0],
            BarOptions::default(),
        )
        .unwrap();
        add(&mut a);
        f
    };
    let renders: [(&str, Figure); 6] = [
        (
            "neighbours",
            years(&|a| {
                a.vline_between(between("2014", "2013"), rule("")).unwrap();
            }),
        ),
        (
            "unknown name",
            years(&|a| {
                a.vline_between(between("2015", "2016"), rule("")).unwrap();
            }),
        ),
        (
            "not neighbours",
            years(&|a| {
                a.vline_between(between("2012", "2014"), rule("")).unwrap();
            }),
        ),
        (
            "span ends swapped",
            years(&|a| {
                a.vspan_between(between("2014", "2015"), between("2012", "2013"), span(""))
                    .unwrap();
            }),
        ),
        (
            "span ends meeting",
            years(&|a| {
                a.vspan_between(between("2013", "2014"), between("2014", "2013"), span(""))
                    .unwrap();
            }),
        ),
        (
            "span with an unknown high end",
            years(&|a| {
                a.vspan_between(between("2012", "2013"), between("2099", "2015"), span(""))
                    .unwrap();
            }),
        ),
    ];
    for (label, f) in &renders {
        rec.result(&format!("to_scene, {label}"), f.to_scene(lib), |_| {
            String::new()
        });
        rec.result(&format!("to_png, {label}"), f.to_png(72), |_| String::new());
        rec.result(
            &format!("to_svg, {label}"),
            f.to_svg(SvgText::Editable, 300),
            |_| String::new(),
        );
    }
    rec.raw(&super::super::record::figure(&renders[2].1));

    // On a grid without a shared axis, the names are looked up in the axes' own categories.
    let mut f = Figure::with_grid(8.0, 3.5, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    for col in 0..2 {
        let mut a = f
            .add_axes_at(cell(col), ScaleKind::Category, linear())
            .unwrap();
        a.bar(
            names(if col == 0 {
                &["2012", "2013"]
            } else {
                &["2014", "2015"]
            }),
            &[1.0, 2.0],
            BarOptions::default(),
        )
        .unwrap();
        if col == 1 {
            a.hline(1.0, RuleOptions::default()).unwrap();
            a.vline_between(between("2013", "2014"), rule("")).unwrap();
        }
    }
    rec.result("to_scene, unshared grid", f.to_scene(lib), |_| {
        String::new()
    });
    rec.out
}
