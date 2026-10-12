//! Output-gate cases for x tick labels at the ends of the axis.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Covered, in every theme: wide labels given to the end ticks of a line's exact x extent, so
//! the last label insets the plot's right edge; wide automatic labels from an affixed format on
//! a scatter's padded x axis, whose last tick sits inside the plot; a wide last category label,
//! centered in its band; and a wide last label next to direct labels, which already reserve
//! room on the right.
//!
//! Labels that cannot fit, in `errors/x-label-overflow.txt`: a last label wider than the
//! capped room in every theme, wide labels at both ends of a 1 in figure, the wider of two
//! overflowing labels, huge category names, and the figure after a failed render, whose
//! shortened label then fits just inside the cap (also as the scene
//! `fig-x-end-label-near-cap`). The section "former scene cases" holds the three figures whose
//! x tick labels were clipped before rendering failed for them: `fig-category-crowded-log`,
//! `fig-squeezed-layout` and `fig-text-truncated`. The last two covered more than their tick
//! labels, so each is also drawn without x ticks, as `fig-squeezed-layout-no-x-ticks` and
//! `fig-text-truncated-no-x-ticks`: a 1 in figure with every text far too wide, and a title,
//! subtitle and axis labels far wider than any figure, cut to the longest prefix that fits on
//! a character boundary (ASCII, two-byte characters, and characters drawn by the fallback
//! font).

use umber_core::figure::{AxisData, BarOptions, LineOptions, ScatterOptions};
use umber_core::{Figure, FontLibrary, LegendStyle, ScaleKind, Theme, TickFormat};

use super::super::{
    Log, Output, axes, emit_scene, figure, labelled, line as plain_line, linear, log, numbers,
    record, themes,
};

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib).unwrap(), sink);
    }
    let (errors, near_cap) = overflow_errors(lib);
    sink(Output {
        kind: "errors",
        file: "x-label-overflow.txt".into(),
        bytes: errors.into_bytes(),
    });
    emit_scene(
        "fig-x-end-label-near-cap",
        &near_cap.to_scene(lib).unwrap(),
        sink,
    );
    for (name, no_x_ticks) in [
        ("fig-squeezed-layout-no-x-ticks", squeezed()),
        ("fig-text-truncated-no-x-ticks", truncated_text(false)),
    ] {
        let mut f = no_x_ticks;
        axes(&mut f).xticks(&[]).unwrap();
        emit_scene(name, &f.to_scene(lib).unwrap(), sink);
    }
}

/// Fourteen long category names on a 3 in figure with a log y axis, which crowd each other.
fn crowded_category() -> Figure {
    let mut f = figure(3.0, 3.0, Theme::pigment(), ScaleKind::Category, log(10.0));
    labelled(&mut f, "Long category names");
    axes(&mut f)
        .line(
            AxisData::Categories(
                (0..14)
                    .map(|i| format!("category number {i} with a long name"))
                    .collect(),
            ),
            &(0..14).map(|i| 10f64.powi(i % 5)).collect::<Vec<_>>(),
            plain_line(),
        )
        .unwrap();
    f
}

/// A 1 in figure at font scale 3 whose every text is far too wide.
fn squeezed() -> Figure {
    let mut f = figure(
        1.0,
        1.0,
        Theme::pigment().with_font_scale(3.0).unwrap(),
        linear(),
        linear(),
    );
    axes(&mut f)
        .title(&"Wide title ".repeat(30))
        .subtitle("Sub")
        .xlabel("x label that is far too long for the figure")
        .ylabel("y label that is far too long for the figure");
    axes(&mut f)
        .line(numbers(vec![0.0, 1.0]), &[0.0, 1.0], plain_line())
        .unwrap();
    f
}

/// Texts far wider than any figure on a 2 in figure at font scale 10. With `categories`, the
/// x axis is a category axis whose first name is as wide; otherwise it is linear.
fn truncated_text(categories: bool) -> Figure {
    let theme = Theme::pigment().with_font_scale(10.0).unwrap();
    let x = if categories {
        ScaleKind::Category
    } else {
        linear()
    };
    let mut f = figure(2.0, 2.0, theme, x, linear());
    axes(&mut f)
        .title(&"W".repeat(20_000))
        .subtitle(&"\u{fc}".repeat(6_000))
        .xlabel(&"\u{E000}".repeat(6_000))
        .ylabel(&"y\u{2212}".repeat(4_000));
    let data = if categories {
        AxisData::Categories(vec!["M".repeat(20_000), "short".into()])
    } else {
        numbers(vec![0.0, 1.0])
    };
    axes(&mut f).line(data, &[1.0, 2.0], plain_line()).unwrap();
    f
}

/// A `width` by `height` in figure in `theme` on x 0..1 with ticks at 0 and 1 labelled
/// `first` and `last`; without y ticks when `no_y_ticks`.
fn end_labels(
    width: f64,
    height: f64,
    theme: Theme,
    [first, last]: [String; 2],
    no_y_ticks: bool,
) -> Figure {
    let mut f = figure(width, height, theme, linear(), linear());
    let a = axes(&mut f);
    a.xlim(0.0, 1.0)
        .unwrap()
        .xticks_labeled(&[0.0, 1.0], vec![first, last])
        .unwrap();
    if no_y_ticks {
        a.yticks(&[]).unwrap();
    }
    f
}

/// Records the result of rendering `f` with `lib`: its error, or `ok`.
fn render(rec: &mut Log, label: &str, f: &Figure, lib: &FontLibrary) {
    rec.result(label, f.to_scene(lib).map(drop), |()| String::new());
}

/// The errors of figures whose x tick labels cannot fit, and the figure that fits after a
/// failed render once its label is shortened.
fn overflow_errors(lib: &FontLibrary) -> (String, Figure) {
    let mut rec = Log::default();

    rec.section("Figure::to_scene, a last label wider than the capped room");
    for (theme_name, theme) in themes() {
        let f = end_labels(6.0, 4.0, theme, ["0".into(), "W".repeat(60)], false);
        render(&mut rec, &format!("{theme_name}, 60 W"), &f, lib);
    }

    rec.section("Figure::to_scene, wide labels at both ends of a 1 in figure");
    for (first, last) in [(10, 10), (12, 10), (10, 12)] {
        let f = end_labels(
            1.0,
            1.0,
            Theme::midnight(),
            ["W".repeat(first), "W".repeat(last)],
            true,
        );
        render(&mut rec, &format!("{first} W and {last} W"), &f, lib);
    }

    rec.section("Figure::to_scene, huge category names");
    let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    axes(&mut f)
        .line(
            AxisData::Categories(vec!["W".repeat(500), "Q2".into(), "V".repeat(500)]),
            &[1.0, 2.0, 3.0],
            LineOptions::default(),
        )
        .unwrap();
    render(&mut rec, "500 W, Q2, 500 V", &f, lib);

    rec.section("Figure::to_scene, the figure after a failed render");
    let mut f = end_labels(
        6.0,
        4.0,
        Theme::midnight(),
        ["0".into(), "W".repeat(43)],
        false,
    );
    render(&mut rec, "midnight, 43 W", &f, lib);
    rec.raw(&record::figure(&f));
    rec.on_axes(&mut f, "shortened to 42 W", |a| {
        a.xticks_labeled(&[0.0, 1.0], vec!["0".into(), "W".repeat(42)])
            .map(drop)
    });
    render(&mut rec, "midnight, 42 W", &f, lib);

    rec.section("Figure::to_scene, former scene cases");
    for (name, f) in [
        ("fig-category-crowded-log", crowded_category()),
        ("fig-squeezed-layout", squeezed()),
        ("fig-text-truncated", truncated_text(true)),
    ] {
        render(&mut rec, name, &f, lib);
    }
    (rec.out, f)
}

fn strings(labels: &[&str]) -> Vec<String> {
    labels.iter().map(|s| (*s).to_owned()).collect()
}

fn line(label: &str) -> LineOptions {
    LineOptions {
        label: label.into(),
        ..LineOptions::default()
    }
}

/// A day of load as one line, with the hours of the day labelled by name.
fn day(a: &mut umber_core::Axes, label: &str) {
    a.line(
        numbers(vec![0.0, 6.0, 12.0, 18.0, 24.0]),
        &[1.0, 3.0, 2.0, 4.0, 3.0],
        line(label),
    )
    .unwrap()
    .xticks_labeled(
        &[0.0, 6.0, 12.0, 18.0, 24.0],
        strings(&["midnight", "6 am", "noon", "6 pm", "midnight"]),
    )
    .unwrap();
}

/// Every figure, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();
    for (theme_name, theme) in themes() {
        // Named hours on a line's exact x extent: the last label sits on the right edge.
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        day(axes(&mut f), "load");
        out.push((format!("fig-x-end-label-hours-{theme_name}"), f));

        // An affixed format on a scatter's padded x axis.
        let mut f = figure(6.0, 4.0, theme.clone(), linear(), linear());
        axes(&mut f)
            .title("Affixed automatic labels")
            .scatter(
                numbers(vec![0.0, 35.0, 70.0, 100.0]),
                &[1.0, 3.0, 2.0, 4.0],
                ScatterOptions::default(),
            )
            .unwrap()
            .xtick_format(TickFormat::AUTO.affix("", " thousand units").unwrap())
            .unwrap();
        out.push((format!("fig-x-end-label-affix-{theme_name}"), f));

        // A wide last category, centered in its band. It reaches into the padding in pigment
        // and graphite and insets the plot in midnight, whose monospaced type is wider.
        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Category, linear());
        axes(&mut f)
            .title("Wide last category")
            .bar(
                strings(&["Q1", "Q2", "Q3", "Q4", "Full year, all stations"]),
                &[3.0, 5.5, 4.0, 4.5, 17.0],
                BarOptions::default(),
            )
            .unwrap();
        out.push((format!("fig-x-end-label-category-{theme_name}"), f));

        // Direct labels already reserve room on the right; the last tick label stays inside.
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        let a = axes(&mut f);
        a.title("Next to direct labels").legend(LegendStyle::Direct);
        day(a, "Station load");
        out.push((format!("fig-x-end-label-direct-{theme_name}"), f));
    }
    out
}
