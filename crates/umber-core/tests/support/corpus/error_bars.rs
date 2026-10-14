//! Output-gate cases for error bars.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Scenes, in the three built-in themes: means on a category axis with vertical bars next to a
//! numeric axis with vertical and horizontal bars, on a one-by-two grid. In single themes: each
//! axis alone; bars without caps; widths; explicit limits that cut bars at their ends and at
//! their sides, with limits exactly on bar ends; a log y axis and a log x axis; NaN in each
//! array; bars beside a line, which pad the axes; each legend style (the bars take a row entry
//! under all of them); and 200 000 bars, most of them outside the limits.
//!
//! In `errors/error_bars.txt`: every error branch of the two methods in the order its checks
//! run, which fault is reported when an input has several, and the state a failed call leaves.
//!
//! In `api/constants-error-bars.txt`: the cap width of each built-in theme.

use std::fmt::Write as _;

use umber_core::figure::{AxisData, ErrorBarOptions, LineOptions, ScatterOptions};
use umber_core::{
    Alpha, AxesMut, ColorSpec, Figure, FontLibrary, Grid, GridIndex, LegendStyle, Placement,
    ScaleKind, SizeSpec, Theme,
};

use super::super::{Log, Output, axes, emit_scene, figure, labelled, linear, log, themes, wobble};

/// A named input of `y_error_bars`: its x, low and high values.
type Case = (&'static str, Vec<f64>, Vec<f64>, Vec<f64>);

fn palette(index: usize) -> ColorSpec {
    ColorSpec::Palette {
        index,
        alpha: Alpha::new(1.0).unwrap(),
    }
}

fn bars(label: &str, index: usize) -> ErrorBarOptions {
    ErrorBarOptions {
        label: label.into(),
        color: palette(index),
        ..ErrorBarOptions::default()
    }
}

fn means(label: &str) -> ScatterOptions {
    ScatterOptions {
        label: label.into(),
        color: palette(0),
        ..ScatterOptions::default()
    }
}

fn names(list: &[&str]) -> AxisData {
    AxisData::Categories(list.iter().map(|s| (*s).to_owned()).collect())
}

fn numbers(values: &[f64]) -> AxisData {
    AxisData::Numbers(values.to_vec())
}

fn around(centre: &[f64], half: &[f64]) -> (Vec<f64>, Vec<f64>) {
    (
        centre.iter().zip(half).map(|(c, h)| c - h).collect(),
        centre.iter().zip(half).map(|(c, h)| c + h).collect(),
    )
}

const GROUPS: [&str; 4] = ["Control", "Low", "Medium", "High"];
const MEANS: [f64; 4] = [4.2, 5.1, 6.8, 7.4];
const SPREAD: [f64; 4] = [0.6, 0.8, 0.7, 1.1];

/// Six measurements with an uncertainty in each direction.
fn measurements() -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let x = vec![1.0, 2.0, 3.0, 4.5, 6.0, 7.5];
    let noise = [0.12, -0.31, 0.24, -0.17, 0.33, -0.09];
    let y = x
        .iter()
        .zip(noise)
        .map(|(x, n)| 1.5 + 0.9 * x + n)
        .collect();
    let y_error = x.iter().map(|x| 0.4 + 0.1 * x).collect();
    let x_error = vec![0.2, 0.3, 0.25, 0.4, 0.35, 0.5];
    (x, y, y_error, x_error)
}

/// Draws the category panel of the reference board on `panel`.
fn category_panel(panel: &mut AxesMut<'_>, index: usize) {
    let (low, high) = around(&MEANS, &SPREAD);
    panel
        .y_error_bars(names(&GROUPS), &low, &high, bars("y interval", index))
        .unwrap()
        .scatter(names(&GROUPS), &MEANS, means("Mean"))
        .unwrap();
}

/// Draws the numeric panel of the reference board on `panel`.
fn numeric_panel(panel: &mut AxesMut<'_>, index: usize) {
    let (x, y, y_error, x_error) = measurements();
    let (y_low, y_high) = around(&y, &y_error);
    let (x_low, x_high) = around(&x, &x_error);
    panel
        .y_error_bars(numbers(&x), &y_low, &y_high, bars("y interval", index))
        .unwrap()
        .x_error_bars(&y, &x_low, &x_high, bars("x interval", index))
        .unwrap()
        .scatter(numbers(&x), &y, means("Mean"))
        .unwrap();
}

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, figure) in figures() {
        emit_scene(&name, &figure.to_scene(lib).unwrap(), sink);
    }
    sink(Output {
        kind: "errors",
        file: "error_bars.txt".into(),
        bytes: error_bar_errors().into_bytes(),
    });
    sink(Output {
        kind: "api",
        file: "constants-error-bars.txt".into(),
        bytes: cap_widths().into_bytes(),
    });
}

/// The cap width of every built-in theme.
fn cap_widths() -> String {
    let mut out = String::new();
    for (name, theme) in themes() {
        let _ = writeln!(out, "{name} cap_width {:?}", theme.cap_width());
    }
    out
}

/// Every error bar figure, by name, in a fixed order.
fn figures() -> Vec<(String, Figure)> {
    let mut out = Vec::new();

    // The reference board: category means on the left, measurements on the right.
    for (theme_name, theme) in themes() {
        let grid = Grid::new(1, 2).unwrap();
        let (width, height) = grid.auto_size_in();
        let mut f = Figure::with_grid(width, height, theme, grid).unwrap();
        for (col, title) in [(0_i64, "Category x"), (1, "Numeric x")] {
            let at = Placement {
                row: GridIndex::At(0),
                col: GridIndex::At(col),
                row_span: 1,
                col_span: 1,
            };
            let kind = if col == 0 {
                ScaleKind::Category
            } else {
                linear()
            };
            let mut panel = f.add_axes_at(at, kind, linear()).unwrap();
            panel.title(title);
            if col == 0 {
                category_panel(&mut panel, 0);
            } else {
                numeric_panel(&mut panel, 0);
            }
        }
        out.push((format!("error-bars-{theme_name}"), f));
    }

    // Each axis on its own figure, in the three themes.
    for (theme_name, theme) in themes() {
        let mut f = figure(6.0, 4.0, theme.clone(), ScaleKind::Category, linear());
        labelled(&mut f, "Means with 95% intervals");
        category_panel(&mut axes(&mut f), 0);
        out.push((format!("error-bars-category-{theme_name}"), f));

        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Measurements with uncertainty");
        numeric_panel(&mut axes(&mut f), 1);
        out.push((format!("error-bars-numeric-{theme_name}"), f));
    }

    // Without caps, with widths, with a fixed width.
    for (name, options) in [
        (
            "no-caps",
            ErrorBarOptions {
                caps: false,
                ..bars("Interval", 0)
            },
        ),
        (
            "thick",
            ErrorBarOptions {
                width: SizeSpec::times(2.5).unwrap(),
                ..bars("Thick", 1)
            },
        ),
        (
            "thin",
            ErrorBarOptions {
                width: SizeSpec::pt(0.5).unwrap(),
                ..bars("Thin", 2)
            },
        ),
    ] {
        let mut f = figure(6.0, 4.0, Theme::pigment(), ScaleKind::Category, linear());
        labelled(&mut f, name);
        let (low, high) = around(&MEANS, &SPREAD);
        axes(&mut f)
            .y_error_bars(names(&GROUPS), &low, &high, options)
            .unwrap();
        out.push((format!("error-bars-{name}"), f));
    }

    // Explicit limits cut bars at their ends: the caps there are lost, the insides stay.
    for (theme_name, theme) in [
        ("pigment", Theme::pigment()),
        ("midnight", Theme::midnight()),
    ] {
        let (x, y, y_error, x_error) = measurements();
        let (y_low, y_high) = around(&y, &y_error.iter().map(|e| e * 6.0).collect::<Vec<_>>());
        let (x_low, x_high) = around(&x, &x_error.iter().map(|e| e * 6.0).collect::<Vec<_>>());
        let mut f = figure(6.0, 4.0, theme, linear(), linear());
        labelled(&mut f, "Explicit limits");
        axes(&mut f)
            .y_error_bars(numbers(&x), &y_low, &y_high, bars("y interval", 0))
            .unwrap()
            .x_error_bars(&y, &x_low, &x_high, bars("x interval", 1))
            .unwrap()
            .xlim(2.0, 6.0)
            .unwrap()
            .ylim(3.0, 8.0)
            .unwrap();
        out.push((format!("error-bars-limits-{theme_name}"), f));
    }

    // Limits exactly on bar ends and on bar positions, and bars that only touch a limit.
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    labelled(&mut f, "Limits on the ends");
    axes(&mut f)
        .y_error_bars(
            numbers(&[2.0, 3.0, 4.0, 5.0, 6.0]),
            &[1.0, 2.0, 3.0, -1.0, 4.0],
            &[3.0, 4.0, 5.0, 2.0, 7.0],
            bars("Bars", 0),
        )
        .unwrap()
        .xlim(2.0, 6.0)
        .unwrap()
        .ylim(2.0, 4.0)
        .unwrap();
    out.push(("error-bars-limits-exact".to_owned(), f));

    // Bars wholly outside the limits next to bars inside.
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    labelled(&mut f, "Outside the limits");
    axes(&mut f)
        .y_error_bars(
            numbers(&[1.0, 2.0, 3.0, 8.0]),
            &[8.0, 1.0, -5.0, 1.0],
            &[9.0, 3.0, -4.0, 3.0],
            bars("Bars", 0),
        )
        .unwrap()
        .xlim(0.0, 4.0)
        .unwrap()
        .ylim(0.0, 4.0)
        .unwrap();
    out.push(("error-bars-outside-limits".to_owned(), f));

    // A log y axis: the limits are placed in log space.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), log(10.0));
    labelled(&mut f, "Log y");
    let x: Vec<f64> = (0..8).map(f64::from).collect();
    let centre: Vec<f64> = x.iter().map(|x| 10.0_f64.powf(0.3 * x + 1.0)).collect();
    let low: Vec<f64> = centre.iter().map(|c| c / 2.0).collect();
    let high: Vec<f64> = centre.iter().map(|c| c * 3.0).collect();
    axes(&mut f)
        .y_error_bars(AxisData::Numbers(x.clone()), &low, &high, bars("Range", 0))
        .unwrap()
        .scatter(AxisData::Numbers(x), &centre, means("Centre"))
        .unwrap();
    out.push(("error-bars-log-y".to_owned(), f));

    // A log x axis, with horizontal bars.
    let mut f = figure(6.0, 4.0, Theme::midnight(), log(10.0), linear());
    labelled(&mut f, "Log x");
    axes(&mut f)
        .x_error_bars(
            &[1.0, 2.0, 3.0, 4.0],
            &[1.0, 5.0, 20.0, 100.0],
            &[8.0, 30.0, 90.0, 800.0],
            bars("Spread", 0),
        )
        .unwrap();
    out.push(("error-bars-log-x".to_owned(), f));

    // NaN in the positions, in low and in high skips only that bar.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut f, "Gaps");
    axes(&mut f)
        .y_error_bars(
            numbers(&[1.0, f64::NAN, 3.0, 4.0, 5.0, 6.0]),
            &[1.0, 1.0, f64::NAN, 1.0, 1.0, 1.0],
            &[2.0, 2.0, 2.0, f64::NAN, 2.0, 2.0],
            bars("Bars", 0),
        )
        .unwrap();
    out.push(("error-bars-gaps".to_owned(), f));

    // Beside a line, which pads the axes the line alone would not.
    let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
    labelled(&mut f, "Bars and a line");
    let (x, y) = wobble(30, 55);
    let y: Vec<f64> = y.iter().map(|v| v + 6.0).collect();
    let (low, high) = around(&y, &vec![0.7; 30]);
    axes(&mut f)
        .line(
            AxisData::Numbers(x.clone()),
            &y,
            LineOptions {
                label: "Series".into(),
                ..LineOptions::default()
            },
        )
        .unwrap()
        .y_error_bars(AxisData::Numbers(x), &low, &high, bars("Error", 1))
        .unwrap();
    out.push(("error-bars-with-line".to_owned(), f));

    // Bars that give an axis nothing leave its extent as the line has it.
    let mut f = figure(6.0, 4.0, Theme::pigment(), linear(), linear());
    labelled(&mut f, "Bars without values");
    axes(&mut f)
        .line(
            AxisData::Numbers(vec![0.0, 10.0]),
            &[1.0, 2.0],
            LineOptions::default(),
        )
        .unwrap()
        .y_error_bars(AxisData::Numbers(vec![]), &[], &[], bars("None", 1))
        .unwrap()
        .x_error_bars(&[f64::NAN], &[f64::NAN], &[f64::NAN], bars("Gaps", 2))
        .unwrap();
    out.push(("error-bars-without-values".to_owned(), f));

    // Each legend style: the bars are in the legend row under all of them.
    for (style_name, style) in [
        ("none", LegendStyle::None),
        ("header", LegendStyle::Header),
        ("chips", LegendStyle::Chips),
        ("direct", LegendStyle::Direct),
    ] {
        let mut f = figure(6.0, 4.0, Theme::graphite(), linear(), linear());
        labelled(&mut f, "Legend styles");
        let (x, y, y_error, x_error) = measurements();
        let (y_low, y_high) = around(&y, &y_error);
        let (x_low, x_high) = around(&x, &x_error);
        axes(&mut f)
            .y_error_bars(numbers(&x), &y_low, &y_high, bars("y interval", 0))
            .unwrap()
            .x_error_bars(&y, &x_low, &x_high, bars("x interval", 1))
            .unwrap()
            .line(
                AxisData::Numbers(x),
                &y,
                LineOptions {
                    label: "Trend".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .legend(style);
        out.push((format!("error-bars-legend-{style_name}"), f));
    }

    // Many bars, most of them outside the limits.
    let n = 200_000;
    let (x, y) = wobble(n, 77);
    let (low, high) = around(&y, &vec![0.5; n]);
    let mut f = figure(6.0, 4.0, Theme::midnight(), linear(), linear());
    axes(&mut f)
        .y_error_bars(
            AxisData::Numbers(x),
            &low,
            &high,
            ErrorBarOptions::default(),
        )
        .unwrap()
        .xlim(1000.0, 1060.0)
        .unwrap();
    out.push(("error-bars-200k-bars".to_owned(), f));

    out
}

/// Every error branch of `Axes::y_error_bars` and `Axes::x_error_bars`, in the order the checks
/// run, and the state after each call.
fn error_bar_errors() -> String {
    let inf = f64::INFINITY;
    let nan = f64::NAN;
    let options = ErrorBarOptions::default;
    let mut rec = Log::default();

    rec.section("Axes.y_error_bars, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "y_error_bars ok", |a| {
        a.y_error_bars(numbers(&[0.0, 1.0]), &[1.0, 2.0], &[3.0, 4.0], options())
            .map(drop)
    });
    let cases: Vec<Case> = vec![
        (
            "x longer than low",
            vec![0.0, 1.0],
            vec![0.0],
            vec![1.0, 2.0],
        ),
        (
            "x longer than high",
            vec![0.0, 1.0],
            vec![0.0, 1.0],
            vec![2.0],
        ),
        (
            "x shorter than both",
            vec![0.0],
            vec![0.0, 1.0],
            vec![2.0, 3.0],
        ),
        ("length mismatch and infinite x", vec![inf], vec![], vec![]),
        ("infinite x", vec![0.0, inf], vec![0.0, 1.0], vec![2.0, 3.0]),
        (
            "negative infinite low",
            vec![0.0, 1.0],
            vec![f64::NEG_INFINITY, 1.0],
            vec![2.0, 3.0],
        ),
        ("infinite high", vec![0.0], vec![0.0], vec![inf]),
        ("huge x", vec![1.0e301], vec![0.0], vec![1.0]),
        ("huge low", vec![0.0], vec![-1.0e301], vec![1.0]),
        ("huge high", vec![0.0], vec![0.0], vec![1.0e301]),
        (
            "infinite low and infinite x",
            vec![inf],
            vec![inf],
            vec![0.0],
        ),
        (
            "infinite low and infinite high",
            vec![0.0],
            vec![inf],
            vec![inf],
        ),
        (
            "swapped and infinite high",
            vec![0.0, 1.0],
            vec![5.0, 5.0],
            vec![1.0, inf],
        ),
        ("low above high", vec![0.0], vec![2.0], vec![1.0]),
        (
            "first of two swapped",
            vec![0.0, 1.0, 2.0],
            vec![0.0, 5.0, 9.0],
            vec![1.0, 4.0, 2.0],
        ),
        (
            "swapped by one ulp",
            vec![0.0],
            vec![1.0 + f64::EPSILON],
            vec![1.0],
        ),
        (
            "swapped after gaps",
            vec![0.0, 1.0, 2.0, 3.0],
            vec![nan, 3.0, 9.0, 8.0],
            vec![0.0, nan, 2.0, 1.0],
        ),
        (
            "unsorted x is fine",
            vec![3.0, 0.0, 2.0],
            vec![0.0; 3],
            vec![1.0; 3],
        ),
        ("swapped, large", vec![0.0], vec![1.0e7], vec![1.0e6]),
    ];
    for (name, x, low, high) in cases {
        rec.on_axes(&mut f, &format!("y_error_bars {name}"), |a| {
            a.y_error_bars(numbers(&x), &low, &high, options())
                .map(drop)
        });
    }
    // Names on a numeric axis, equal limits, gaps in each array, empty data, with
    // non-canonical NaN payloads and signed zeros: storage keeps every bit.
    let p0 = f64::from_bits(0x7ff8_0000_0000_0001);
    let p1 = f64::from_bits(0xfff0_0000_dead_beef);
    rec.on_axes(&mut f, "y_error_bars names on a numeric axis", |a| {
        a.y_error_bars(names(&["a"]), &[0.0], &[1.0], options())
            .map(drop)
    });
    rec.on_axes(&mut f, "y_error_bars with equal limits", |a| {
        a.y_error_bars(numbers(&[0.0, 1.0]), &[2.0, 2.0], &[2.0, 2.0], options())
            .map(drop)
    });
    rec.on_axes(
        &mut f,
        "y_error_bars with NaN payloads and signed zeros",
        |a| {
            a.y_error_bars(
                numbers(&[-0.0, 0.0, p0, 2.0]),
                &[p1, -0.0, 1.0, 0.0],
                &[1.0, 0.0, 2.0, p0],
                options(),
            )
            .map(drop)
        },
    );
    rec.on_axes(&mut f, "y_error_bars all NaN", |a| {
        a.y_error_bars(numbers(&[nan, nan]), &[nan, nan], &[nan, nan], options())
            .map(drop)
    });
    rec.on_axes(&mut f, "y_error_bars empty", |a| {
        a.y_error_bars(numbers(&[]), &[], &[], options()).map(drop)
    });
    rec.on_axes(&mut f, "y_error_bars with options", |a| {
        a.y_error_bars(
            numbers(&[umber_core::MAX_MAGNITUDE, -umber_core::MAX_MAGNITUDE]),
            &[-umber_core::MAX_MAGNITUDE, 0.0],
            &[0.0, umber_core::MAX_MAGNITUDE],
            ErrorBarOptions {
                label: "edge".into(),
                color: ColorSpec::Palette {
                    index: 21,
                    alpha: Alpha::new(0.25).unwrap(),
                },
                width: SizeSpec::times(2.0).unwrap(),
                caps: false,
            },
        )
        .map(drop)
    });

    rec.section("Axes.y_error_bars, category/linear");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    rec.on_axes(&mut f, "y_error_bars names", |a| {
        a.y_error_bars(names(&["a", "b", "a"]), &[0.0; 3], &[1.0; 3], options())
            .map(drop)
    });
    rec.on_axes(&mut f, "y_error_bars numbers on a category axis", |a| {
        a.y_error_bars(numbers(&[0.0, 1.0]), &[0.0, 1.0], &[1.0, 2.0], options())
            .map(drop)
    });
    rec.on_axes(&mut f, "y_error_bars empty names", |a| {
        a.y_error_bars(names(&[]), &[], &[], options()).map(drop)
    });
    rec.on_axes(&mut f, "y_error_bars names, length mismatch", |a| {
        a.y_error_bars(names(&["a"]), &[], &[], options()).map(drop)
    });
    rec.on_axes(&mut f, "y_error_bars names, swapped", |a| {
        a.y_error_bars(names(&["a"]), &[2.0], &[1.0], options())
            .map(drop)
    });

    rec.section("Axes.y_error_bars, linear/log10");
    let mut f = rec.new_figure("linear/log10", linear(), log(10.0));
    for (name, x, low, high) in [
        ("zero low", vec![0.0, 1.0], vec![1.0, 0.0], vec![2.0, 3.0]),
        ("negative high", vec![0.0], vec![1.0], vec![-1.0]),
        ("positive", vec![0.0, 1.0], vec![1.0, 2.0], vec![3.0, 4.0]),
        ("NaN low", vec![0.0, 1.0], vec![nan, 2.0], vec![3.0, 4.0]),
        ("swapped", vec![0.0], vec![4.0], vec![3.0]),
        ("negative x is fine", vec![-5.0], vec![1.0], vec![2.0]),
    ] {
        rec.on_axes(&mut f, &format!("y_error_bars {name}"), |a| {
            a.y_error_bars(numbers(&x), &low, &high, options())
                .map(drop)
        });
    }

    rec.section("Axes.y_error_bars, log2/linear");
    let mut f = rec.new_figure("log2/linear", log(2.0), linear());
    for (name, x, low, high) in [
        ("zero x", vec![0.0], vec![1.0], vec![2.0]),
        ("negative x", vec![-1.0], vec![1.0], vec![2.0]),
        ("positive x", vec![2.0, 4.0], vec![1.0, 2.0], vec![3.0, 4.0]),
        ("zero y is fine", vec![2.0], vec![0.0], vec![0.0]),
    ] {
        rec.on_axes(&mut f, &format!("y_error_bars {name}"), |a| {
            a.y_error_bars(numbers(&x), &low, &high, options())
                .map(drop)
        });
    }

    rec.section("Axes.x_error_bars, linear/linear");
    let mut f = rec.new_figure("linear/linear", linear(), linear());
    rec.on_axes(&mut f, "x_error_bars ok", |a| {
        a.x_error_bars(&[0.0, 1.0], &[1.0, 2.0], &[3.0, 4.0], options())
            .map(drop)
    });
    let cases: Vec<Case> = vec![
        (
            "y longer than low",
            vec![0.0, 1.0],
            vec![0.0],
            vec![1.0, 2.0],
        ),
        (
            "y longer than high",
            vec![0.0, 1.0],
            vec![0.0, 1.0],
            vec![2.0],
        ),
        (
            "y shorter than both",
            vec![0.0],
            vec![0.0, 1.0],
            vec![2.0, 3.0],
        ),
        ("length mismatch and infinite y", vec![inf], vec![], vec![]),
        ("infinite y", vec![0.0, inf], vec![0.0, 1.0], vec![2.0, 3.0]),
        (
            "negative infinite low",
            vec![0.0, 1.0],
            vec![f64::NEG_INFINITY, 1.0],
            vec![2.0, 3.0],
        ),
        ("infinite high", vec![0.0], vec![0.0], vec![inf]),
        ("huge y", vec![1.0e301], vec![0.0], vec![1.0]),
        ("huge low", vec![0.0], vec![-1.0e301], vec![1.0]),
        ("huge high", vec![0.0], vec![0.0], vec![1.0e301]),
        (
            "infinite low and infinite y",
            vec![inf],
            vec![inf],
            vec![0.0],
        ),
        ("low above high", vec![0.0], vec![2.0], vec![1.0]),
        (
            "first of two swapped",
            vec![0.0, 1.0, 2.0],
            vec![0.0, 5.0, 9.0],
            vec![1.0, 4.0, 2.0],
        ),
        (
            "swapped after gaps",
            vec![0.0, 1.0, 2.0, 3.0],
            vec![nan, 3.0, 9.0, 8.0],
            vec![0.0, nan, 2.0, 1.0],
        ),
        (
            "unsorted y is fine",
            vec![3.0, 0.0, 2.0],
            vec![0.0; 3],
            vec![1.0; 3],
        ),
    ];
    for (name, y, low, high) in cases {
        rec.on_axes(&mut f, &format!("x_error_bars {name}"), |a| {
            a.x_error_bars(&y, &low, &high, options()).map(drop)
        });
    }
    rec.on_axes(&mut f, "x_error_bars all NaN", |a| {
        a.x_error_bars(&[nan], &[nan], &[nan], options()).map(drop)
    });
    rec.on_axes(&mut f, "x_error_bars empty", |a| {
        a.x_error_bars(&[], &[], &[], options()).map(drop)
    });
    rec.on_axes(&mut f, "x_error_bars with options", |a| {
        a.x_error_bars(
            &[1.0],
            &[0.0],
            &[2.0],
            ErrorBarOptions {
                label: "across".into(),
                color: palette(3),
                width: SizeSpec::pt(1.5).unwrap(),
                caps: false,
            },
        )
        .map(drop)
    });

    rec.section("Axes.x_error_bars, category/linear");
    let mut f = rec.new_figure("category/linear", ScaleKind::Category, linear());
    for (name, y, low, high) in [
        ("category x", vec![0.0, 1.0], vec![0.0, 1.0], vec![1.0, 2.0]),
        ("category x, empty", vec![], vec![], vec![]),
        ("category x, length mismatch", vec![0.0], vec![], vec![]),
        ("category x, infinite y", vec![inf], vec![], vec![]),
        ("category x, swapped", vec![0.0], vec![2.0], vec![1.0]),
    ] {
        rec.on_axes(&mut f, &format!("x_error_bars {name}"), |a| {
            a.x_error_bars(&y, &low, &high, options()).map(drop)
        });
    }

    rec.section("Axes.x_error_bars, log10/log10");
    let mut f = rec.new_figure("log10/log10", log(10.0), log(10.0));
    for (name, y, low, high) in [
        ("positive", vec![1.0, 2.0], vec![1.0, 2.0], vec![3.0, 4.0]),
        ("zero y", vec![0.0], vec![1.0], vec![2.0]),
        ("zero low", vec![1.0], vec![0.0], vec![2.0]),
        ("negative high", vec![1.0], vec![1.0], vec![-1.0]),
        ("NaN high", vec![1.0], vec![1.0], vec![nan]),
        ("swapped", vec![1.0], vec![4.0], vec![3.0]),
    ] {
        rec.on_axes(&mut f, &format!("x_error_bars {name}"), |a| {
            a.x_error_bars(&y, &low, &high, options()).map(drop)
        });
    }

    rec.section("Axes.x_error_bars, linear/log2");
    let mut f = rec.new_figure("linear/log2", linear(), log(2.0));
    for (name, y, low, high) in [
        ("zero y", vec![0.0], vec![1.0], vec![2.0]),
        (
            "negative x limits are fine",
            vec![2.0],
            vec![-3.0],
            vec![-1.0],
        ),
    ] {
        rec.on_axes(&mut f, &format!("x_error_bars {name}"), |a| {
            a.x_error_bars(&y, &low, &high, options()).map(drop)
        });
    }
    rec.out
}
