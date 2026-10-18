//! Output-gate cases for the plot's minimum height.
//!
//! The cases write only new output files. See `scripts/check-refactor-output.sh`.
//!
//! Figures too short for the plot, in `errors/plot-min-height.txt`: a figure without ticks or a
//! header 0.3 in tall, and one just tall enough (25 pt); in every theme, a 6 in figure with a
//! title, a subtitle, axis labels, automatic ticks and three labelled lines in the theme's own
//! legend style, 1 in tall, as tall as it needs and 1 pt shorter; every combination of a
//! title, a subtitle and a header legend row, whose hint names only the parts the figure has;
//! and the former scene cases that drew their plot clamped to 1 pt tall, at their former
//! sizes. The figures as tall as they need are also the scenes `fig-plot-min-height-<theme>`.

use umber_core::figure::{AxisData, LineOptions};
use umber_core::{Figure, FontLibrary, LegendStyle, Theme};

use super::super::{Log, Output, axes, emit_scene, figure, font_scale_figure, linear, themes};
use super::legend::{fifty_lines, font_scale_max};
use super::x_end_label::{squeezed, truncated_text};

/// The heights, in points, that the full figure of [`full`] needs in each theme, in the order
/// of `themes()`: pigment shows a header row, graphite direct labels and midnight chips.
const FULL_NEEDS: [f64; 3] = [132.0, 114.0, 137.0];

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    sink(Output {
        kind: "errors",
        file: "plot-min-height.txt".into(),
        bytes: min_height_errors(lib).into_bytes(),
    });
    for ((theme_name, theme), need) in themes().into_iter().zip(FULL_NEEDS) {
        emit_scene(
            &format!("fig-plot-min-height-{theme_name}"),
            &full(theme, need / 72.0).to_scene(lib).unwrap(),
            sink,
        );
    }
}

fn render(rec: &mut Log, label: &str, f: &Figure, lib: &FontLibrary) {
    rec.result(label, f.to_scene(lib).map(drop), |()| String::new());
}

/// A `width` by `height` in midnight figure without ticks, with the header parts asked for: a
/// title, a subtitle, and a header legend row of one labelled line.
fn bare(width: f64, height: f64, [title, subtitle, legend]: [bool; 3]) -> Figure {
    let mut f = figure(width, height, Theme::midnight(), linear(), linear());
    let mut a = axes(&mut f);
    a.xticks(&[])
        .unwrap()
        .yticks(&[])
        .unwrap()
        .legend(LegendStyle::Header, umber_core::SizeSpec::Auto)
        .unwrap();
    if title {
        a.title("Load");
    }
    if subtitle {
        a.subtitle("Daily mean");
    }
    if legend {
        a.line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[1.0, 2.0],
            LineOptions {
                label: "North".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    }
    f
}

/// A 6 in wide figure `height` in tall in `theme` with a title, a subtitle, axis labels,
/// automatic ticks and three labelled lines in the theme's own legend style.
fn full(theme: Theme, height: f64) -> Figure {
    let mut f = figure(6.0, height, theme, linear(), linear());
    let mut a = axes(&mut f);
    a.title("Station output")
        .subtitle("Daily mean, MW")
        .xlabel("Day")
        .ylabel("MW");
    for (i, label) in ["North", "South", "East"].into_iter().enumerate() {
        let i = i as f64;
        a.line(
            AxisData::Numbers(vec![0.0, 30.0]),
            &[i, 100.0 - i],
            LineOptions {
                label: label.into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    }
    f
}

/// The results of figures that leave the plot less than 1 pt tall, or just enough.
fn min_height_errors(lib: &FontLibrary) -> String {
    let mut rec = Log::default();

    rec.section("Figure::to_scene, figures without ticks or a header");
    render(&mut rec, "0.3 in", &bare(1.0, 0.3, [false; 3]), lib);
    render(&mut rec, "25 pt", &bare(1.0, 25.0 / 72.0, [false; 3]), lib);

    rec.section("Figure::to_scene, a full figure 1 in tall, as tall as it needs and 1 pt less");
    for ((theme_name, theme), need) in themes().into_iter().zip(FULL_NEEDS) {
        for (label, height) in [("1 in", 72.0), ("needed", need), ("1 pt less", need - 1.0)] {
            let f = full(theme.clone(), height / 72.0);
            render(&mut rec, &format!("{theme_name}, {label}"), &f, lib);
        }
    }

    rec.section("Figure::to_scene, the header parts named in the hint");
    for parts in [
        [true, false, false],
        [false, true, false],
        [false, false, true],
        [true, true, false],
        [true, false, true],
        [false, true, true],
        [true, true, true],
    ] {
        let named: Vec<&str> = ["title", "subtitle", "legend"]
            .into_iter()
            .zip(parts)
            .filter_map(|(name, on)| on.then_some(name))
            .collect();
        render(&mut rec, &named.join(" and "), &bare(2.0, 0.3, parts), lib);
    }

    rec.section("Figure::to_scene, former scene cases");
    for (name, f) in [
        ("fig-font-scale-max", font_scale_figure(10.0, 4.0)),
        (
            "fig-legend-tiny-50-entries",
            fifty_lines(Theme::pigment(), "Entry", 1.0),
        ),
        (
            "fig-legend-font-scale-max",
            font_scale_max(Theme::pigment(), 4.0),
        ),
        (
            "fig-legend-chips-font-scale-max",
            font_scale_max(Theme::midnight(), 4.0),
        ),
        (
            "fig-legend-chips-tiny-50-entries",
            fifty_lines(Theme::midnight(), "Entry", 1.0),
        ),
        (
            "fig-direct-font-scale-max",
            font_scale_max(Theme::graphite(), 4.0),
        ),
        (
            "fig-direct-tiny-50-lines",
            fifty_lines(Theme::graphite(), "Line", 1.0),
        ),
        ("fig-squeezed-layout-no-x-ticks", {
            let mut f = squeezed(1.0);
            axes(&mut f).xticks(&[]).unwrap();
            f
        }),
        ("fig-text-truncated-no-ticks", {
            let mut f = truncated_text(false, 2.0);
            axes(&mut f).xticks(&[]).unwrap().yticks(&[]).unwrap();
            f
        }),
    ] {
        render(&mut rec, name, &f, lib);
    }
    rec.out
}
