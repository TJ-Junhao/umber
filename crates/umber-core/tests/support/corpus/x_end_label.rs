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

use umber_core::figure::{BarOptions, LineOptions, ScatterOptions};
use umber_core::{Figure, FontLibrary, LegendStyle, ScaleKind, TickFormat};

use super::super::{Output, axes, emit_scene, figure, linear, numbers, themes};

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib), sink);
    }
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
