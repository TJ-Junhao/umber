//! Output-gate cases for legend figures.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Covered: the header row with 1, 3 and 20 entries (wrapping), with and without a title and
//! subtitle, at the largest font scale, in a 1 by 1 in figure with 50 entries, in every theme;
//! a 300-character label; labels drawn by the in-memory fallback font and labels with
//! characters no font draws; unlabelled series mixed in; line swatches in every dash, dot
//! swatches, the square swatch of a bar next to a line and of every series of a stacked bar;
//! the legend switch (`LegendStyle::None` in the theme, and `Axes::legend` turning the row off
//! and on); and the theme records with `LegendStyle::None`.
//!
//! Chips: 3 and 20 entries (wrapping), a 300-character label, fallback and undrawable labels,
//! a faded series color, lines, points and bars (every chip holds a dot), the largest font
//! scale and a 1 by 1 in figure with 50 entries, on the dark Midnight background and on the
//! light Pigment and Graphite backgrounds.

use std::fmt::Write as _;

use umber_core::figure::{AxisData, BarOptions, LineOptions, ScatterOptions};
use umber_core::{BarSeries, ColorSpec, Dash, Figure, FontLibrary, LegendStyle, ScaleKind, Theme};

use super::super::{Output, axes, emit_scene, figure, labelled, linear, numbers, record, wobble};

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(name, &figure.to_scene(lib), sink);
    }
    sink(Output {
        kind: "api",
        file: "legend.txt".into(),
        bytes: api_cases().into_bytes(),
    });
}

fn labelled_line(label: &str, dash: Dash) -> LineOptions {
    LineOptions {
        label: label.into(),
        dash,
        ..LineOptions::default()
    }
}

fn labelled_scatter(label: &str) -> ScatterOptions {
    ScatterOptions {
        label: label.into(),
        ..ScatterOptions::default()
    }
}

/// A 6 by 4 in figure in `theme` with a title, subtitle and axis labels, and one line labelled
/// with each of `labels` (an empty label leaves its line unlabelled).
fn lines(theme: Theme, title: &str, labels: &[String]) -> Figure {
    let mut f = figure(6.0, 4.0, theme, linear(), linear());
    labelled(&mut f, title);
    add_lines(&mut f, labels);
    f
}

fn add_lines(f: &mut Figure, labels: &[String]) {
    for (k, label) in labels.iter().enumerate() {
        let (x, y) = wobble(12, 300 + k as u32);
        let y: Vec<f64> = y.iter().map(|v| v + k as f64).collect();
        axes(f)
            .line(numbers(x), &y, labelled_line(label, Dash::Solid))
            .unwrap();
    }
}

fn names(prefix: &str, n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("{prefix} {i}")).collect()
}

/// Every legend figure, by name, in a fixed order.
fn figures() -> Vec<(&'static str, Figure)> {
    let mut out = Vec::new();

    out.push((
        "fig-legend-header-1",
        lines(Theme::pigment(), "One entry", &names("Series", 1)),
    ));

    // Line swatches in every dash, and a dot swatch.
    let mut kinds = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut kinds, "Swatch kinds");
    for (k, (label, dash)) in [
        ("Solid", Dash::Solid),
        ("Dashed", Dash::Dashed),
        ("Dotted", Dash::Dotted),
    ]
    .into_iter()
    .enumerate()
    {
        let (x, y) = wobble(20, 40 + k as u32);
        axes(&mut kinds)
            .line(numbers(x), &y, labelled_line(label, dash))
            .unwrap();
    }
    let (x, y) = wobble(20, 50);
    axes(&mut kinds)
        .scatter(numbers(x), &y, labelled_scatter("Points"))
        .unwrap();
    out.push(("fig-legend-swatch-kinds", kinds));

    out.push((
        "fig-legend-header-3",
        lines(Theme::pigment(), "Three entries", &names("Region", 3)),
    ));
    out.push((
        "fig-legend-header-20-wrapping",
        lines(Theme::pigment(), "Twenty entries", &names("Station", 20)),
    ));

    // A label far wider than the row, followed by a short one: each gets a row of its own.
    let long: String = "Long legend label ".repeat(17).chars().take(300).collect();
    out.push((
        "fig-legend-long-label",
        lines(Theme::pigment(), "Long label", &[long, "Short".into()]),
    ));

    // U+E000 is drawn only by the in-memory fallback font; the CJK characters by no font.
    out.push((
        "fig-legend-text-fallback",
        lines(
            Theme::pigment(),
            "Fallback labels",
            &[
                "Fallback \u{E000}\u{E000}".into(),
                "Missing \u{65e5}\u{672c}".into(),
            ],
        ),
    ));

    let mixed: Vec<String> = ["First", "", "Third", "", ""]
        .into_iter()
        .map(String::from)
        .collect();
    out.push((
        "fig-legend-unlabelled-mixed",
        lines(Theme::pigment(), "Some series unlabelled", &mixed),
    ));

    // No title and no subtitle: the row starts at the top padding.
    let mut bare = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    add_lines(&mut bare, &names("Series", 3));
    out.push(("fig-legend-without-title", bare));

    let mut tiny = figure(1.0, 1.0, Theme::pigment(), linear(), linear());
    labelled(&mut tiny, "Tiny");
    add_lines(&mut tiny, &names("Entry", 50));
    out.push(("fig-legend-tiny-50-entries", tiny));

    out.push((
        "fig-legend-font-scale-max",
        lines(
            Theme::pigment().with_font_scale(10.0).unwrap(),
            "Scaled",
            &names("Series", 3),
        ),
    ));

    // The legend switch.
    let none = Theme::pigment()
        .with_legend_style(LegendStyle::None)
        .unwrap();
    out.push((
        "fig-legend-theme-none",
        lines(none.clone(), "Theme without legend", &names("Series", 3)),
    ));
    let mut off = lines(Theme::pigment(), "Legend off", &names("Series", 3));
    axes(&mut off).legend(LegendStyle::None);
    out.push(("fig-legend-axes-none", off));
    let mut on = lines(none, "Legend on", &names("Series", 3));
    axes(&mut on).legend(LegendStyle::Header);
    out.push(("fig-legend-axes-header", on));
    for (name, theme) in [
        ("fig-legend-graphite-header", Theme::graphite()),
        ("fig-legend-midnight-header", Theme::midnight()),
    ] {
        let mut f = lines(theme, "Header row", &names("Series", 3));
        let (x, y) = wobble(20, 77);
        axes(&mut f)
            .scatter(
                numbers(x),
                &y,
                ScatterOptions {
                    label: "Faded points".into(),
                    color: ColorSpec::Auto(umber_core::Alpha::new(0.5).unwrap()),
                    ..ScatterOptions::default()
                },
            )
            .unwrap()
            .legend(LegendStyle::Header);
        out.push((name, f));
    }

    // A category axis.
    let mut category = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    labelled(&mut category, "Categories");
    for (k, label) in ["Wind", "Solar"].into_iter().enumerate() {
        axes(&mut category)
            .line(
                AxisData::Categories(vec!["Jan".into(), "Feb".into(), "Mar".into()]),
                &[1.0 + k as f64, 3.0, 2.0 - k as f64],
                labelled_line(label, Dash::Solid),
            )
            .unwrap();
    }
    out.push(("fig-legend-category", category));

    // A bar next to a line: the bar's square swatch and the line's swatch in one row.
    let quarters: Vec<String> = ["Q1", "Q2", "Q3", "Q4"].map(String::from).to_vec();
    let mut bars = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    labelled(&mut bars, "Bar and line");
    axes(&mut bars)
        .bar(
            quarters.clone(),
            &[12.0, 18.0, 15.0, 22.0],
            BarOptions {
                label: "Revenue".into(),
                ..BarOptions::default()
            },
        )
        .unwrap()
        .line(
            AxisData::Categories(quarters),
            &[10.0, 14.0, 16.0, 19.0],
            labelled_line("Target", Dash::Dashed),
        )
        .unwrap();
    out.push(("fig-legend-bar-and-line", bars));

    // A stacked bar with three labelled series: one square swatch per series, in series order.
    let mut stacked = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
    labelled(&mut stacked, "Stacked series");
    let series = [
        ("2024", [12.0, 18.0, 15.0, 22.0]),
        ("2025", [9.0, 14.0, 19.0, 17.0]),
        ("2026", [5.0, 8.0, 11.0, 14.0]),
    ]
    .map(|(label, heights)| BarSeries {
        label: label.into(),
        heights: heights.to_vec(),
        color: ColorSpec::AUTO,
    })
    .to_vec();
    axes(&mut stacked)
        .stacked_bar(["Q1", "Q2", "Q3", "Q4"].map(String::from).to_vec(), series)
        .unwrap();
    out.push(("fig-legend-stacked-bar", stacked));

    out.extend(chips_figures());
    out
}

/// The chips legend figures, by name, in a fixed order.
fn chips_figures() -> Vec<(&'static str, Figure)> {
    let mut out = Vec::new();
    let chips = |theme: Theme| theme.with_legend_style(LegendStyle::Chips).unwrap();

    // Lines, a faded scatter and a dashed line: every chip holds a dot, tinted by its color.
    for (name, theme) in [
        ("fig-legend-chips-midnight", Theme::midnight()),
        ("fig-legend-chips-pigment", chips(Theme::pigment())),
        ("fig-legend-chips-graphite", chips(Theme::graphite())),
    ] {
        let mut f = lines(theme, "Chips", &names("Series", 2));
        let (x, y) = wobble(20, 78);
        axes(&mut f)
            .scatter(
                numbers(x),
                &y,
                ScatterOptions {
                    label: "Faded points".into(),
                    color: ColorSpec::Auto(umber_core::Alpha::new(0.5).unwrap()),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        let (x, y) = wobble(20, 79);
        axes(&mut f)
            .line(numbers(x), &y, labelled_line("Dashed", Dash::Dashed))
            .unwrap();
        out.push((name, f));
    }

    out.push((
        "fig-legend-chips-20-wrapping",
        lines(Theme::midnight(), "Twenty chips", &names("Station", 20)),
    ));
    out.push((
        "fig-legend-chips-20-wrapping-light",
        lines(
            chips(Theme::pigment()),
            "Twenty chips",
            &names("Station", 20),
        ),
    ));

    let long: String = "Long legend label ".repeat(17).chars().take(300).collect();
    out.push((
        "fig-legend-chips-long-label",
        lines(Theme::midnight(), "Long chip", &[long, "Short".into()]),
    ));

    // U+E000 is drawn only by the in-memory fallback font; the CJK characters by no font.
    let fallback: [String; 2] = [
        "Fallback \u{E000}\u{E000}".into(),
        "Missing \u{65e5}\u{672c}".into(),
    ];
    out.push((
        "fig-legend-chips-text-fallback",
        lines(Theme::midnight(), "Fallback chips", &fallback),
    ));
    out.push((
        "fig-legend-chips-text-fallback-light",
        lines(chips(Theme::graphite()), "Fallback chips", &fallback),
    ));

    out.push((
        "fig-legend-chips-font-scale-max",
        lines(
            Theme::midnight().with_font_scale(10.0).unwrap(),
            "Scaled",
            &names("Series", 3),
        ),
    ));

    let mut tiny = figure(1.0, 1.0, Theme::midnight(), linear(), linear());
    labelled(&mut tiny, "Tiny");
    add_lines(&mut tiny, &names("Entry", 50));
    out.push(("fig-legend-chips-tiny-50-entries", tiny));

    // A grouped bar: bars get a dot in a chip, not a square.
    let series = [("North", 1.0), ("South", 2.0), ("East", 3.0)]
        .map(|(label, k)| BarSeries {
            label: label.into(),
            heights: vec![10.0 + k, 14.0 - k, 9.0 + 2.0 * k],
            color: ColorSpec::AUTO,
        })
        .to_vec();
    let mut grouped = figure(6.0, 4.0, Theme::midnight(), ScaleKind::Category, linear());
    labelled(&mut grouped, "Grouped chips");
    axes(&mut grouped)
        .grouped_bar(["Q1", "Q2", "Q3"].map(String::from).to_vec(), series)
        .unwrap();
    out.push(("fig-legend-chips-grouped-bar", grouped));

    out
}

/// The legend style records: every variant, every built-in theme with `LegendStyle::None`, and
/// the axes override as `Axes::legend` sets and replaces it.
fn api_cases() -> String {
    let mut out = String::new();
    for style in [
        LegendStyle::Header,
        LegendStyle::Direct,
        LegendStyle::Chips,
        LegendStyle::None,
    ] {
        let _ = writeln!(out, "legend_style {}", record::legend(style));
    }
    for (name, theme) in [
        ("pigment", Theme::pigment()),
        ("graphite", Theme::graphite()),
        ("midnight", Theme::midnight()),
    ] {
        let _ = writeln!(out, "theme {name} with_legend_style(none)");
        out.push_str(&record::theme(
            &theme.with_legend_style(LegendStyle::None).unwrap(),
        ));
    }
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    let override_of = |f: &mut Figure| {
        axes(f)
            .legend_style()
            .map_or_else(|| "unset".to_owned(), |s| record::legend(s).to_owned())
    };
    let _ = writeln!(out, "axes legend_style {}", override_of(&mut f));
    for style in [LegendStyle::Chips, LegendStyle::None, LegendStyle::Header] {
        axes(&mut f).legend(style);
        let _ = writeln!(
            out,
            "axes legend({}) legend_style {}",
            record::legend(style),
            override_of(&mut f)
        );
    }
    out
}
