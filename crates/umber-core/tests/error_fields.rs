//! The parameter and the hint of errors raised by the public API.

// clippy's allow-in-tests settings cover only `#[test]` functions, not the helpers here.
#![allow(clippy::expect_used)]

use umber_core::{
    Alpha, AxisData, BarOptions, BarSeries, Error, Figure, HistOptions, LineOptions, LogBase,
    Result, Rgba, ScaleKind, ScatterOptions, Theme,
};

const LOG: ScaleKind = ScaleKind::Log { base: LogBase::TEN };

fn figure() -> Figure {
    Figure::new(6.0, 4.0, Theme::pigment()).expect("a valid size")
}

fn nums(values: &[f64]) -> AxisData {
    AxisData::Numbers(values.to_vec())
}

fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).to_owned()).collect()
}

fn series(heights: &[f64]) -> BarSeries {
    BarSeries {
        heights: heights.to_vec(),
        ..BarSeries::default()
    }
}

/// Runs `call` on the axes of a new figure with the given scale kinds.
fn on_axes(
    x: ScaleKind,
    y: ScaleKind,
    call: impl FnOnce(&mut umber_core::Axes) -> Result<()>,
) -> Error {
    let mut figure = figure();
    let axes = figure.add_axes(x, y).expect("the first axes");
    call(axes).expect_err("the call should fail")
}

#[test]
fn errors_carry_their_parameter_and_hint() {
    let inf = f64::INFINITY;
    let linear = ScaleKind::Linear;
    let cat = ScaleKind::Category;
    let cases: Vec<(&str, Error, Option<&str>, Option<&str>)> = vec![
        (
            "line, infinite y",
            on_axes(linear, linear, |a| {
                a.line(nums(&[1.0, 2.0]), &[1.0, inf], LineOptions::default())
                    .map(|_| ())
            }),
            Some("y"),
            Some("use NaN to leave a gap in the line."),
        ),
        (
            "line, log y with zero",
            on_axes(linear, LOG, |a| {
                a.line(nums(&[1.0, 2.0]), &[1.0, 0.0], LineOptions::default())
                    .map(|_| ())
            }),
            Some("y"),
            Some("use a linear axis, or remove values that are not positive."),
        ),
        (
            "line, lengths differ",
            on_axes(linear, linear, |a| {
                a.line(nums(&[1.0, 2.0]), &[1.0], LineOptions::default())
                    .map(|_| ())
            }),
            None,
            None,
        ),
        (
            "line, numbers on a category x axis",
            on_axes(cat, linear, |a| {
                a.line(nums(&[1.0]), &[1.0], LineOptions::default())
                    .map(|_| ())
            }),
            None,
            Some("to plot numbers, use a linear or log x axis."),
        ),
        (
            "line, category y axis",
            on_axes(linear, cat, |a| {
                a.line(nums(&[1.0]), &[1.0], LineOptions::default())
                    .map(|_| ())
            }),
            None,
            Some("create the axes with a linear or log y scale."),
        ),
        (
            "bar, non-category x axis",
            on_axes(linear, linear, |a| {
                a.bar(names(&["a"]), &[1.0], BarOptions::default())
                    .map(|_| ())
            }),
            None,
            Some("create the axes with a category x scale."),
        ),
        (
            "stacked bar, category y axis",
            on_axes(cat, cat, |a| {
                a.stacked_bar(names(&["a"]), vec![series(&[1.0])])
                    .map(|_| ())
            }),
            None,
            Some("create the axes with a linear or log y scale."),
        ),
        (
            "hist, category x axis",
            on_axes(cat, linear, |a| {
                a.hist(&[1.0], HistOptions::default()).map(|_| ())
            }),
            None,
            Some("create the axes with a linear x scale."),
        ),
        (
            "hist, log x axis",
            on_axes(LOG, linear, |a| {
                a.hist(&[1.0], HistOptions::default()).map(|_| ())
            }),
            None,
            Some("create the axes with a linear x scale; histograms do not support log x."),
        ),
        (
            "hist, category y axis",
            on_axes(linear, cat, |a| {
                a.hist(&[1.0], HistOptions::default()).map(|_| ())
            }),
            None,
            Some("create the axes with a linear or log y scale."),
        ),
        (
            "scatter, infinite x",
            on_axes(linear, linear, |a| {
                a.scatter(nums(&[inf]), &[1.0], ScatterOptions::default())
                    .map(|_| ())
            }),
            Some("x"),
            Some("use NaN to leave a point out."),
        ),
        (
            "bar, infinite height",
            on_axes(cat, linear, |a| {
                a.bar(names(&["a"]), &[inf], BarOptions::default())
                    .map(|_| ())
            }),
            Some("heights"),
            Some("use NaN to leave a category without a bar."),
        ),
        (
            "bar, repeated category",
            on_axes(cat, linear, |a| {
                a.bar(names(&["a", "a"]), &[1.0, 2.0], BarOptions::default())
                    .map(|_| ())
            }),
            Some("categories"),
            Some("add up the heights of a repeated category before passing them."),
        ),
        (
            "grouped bar, no series",
            on_axes(cat, linear, |a| {
                a.grouped_bar(names(&["a"]), Vec::new()).map(|_| ())
            }),
            Some("series"),
            None,
        ),
        (
            "grouped bar, nested infinite height",
            on_axes(cat, linear, |a| {
                a.grouped_bar(names(&["a"]), vec![series(&[1.0]), series(&[inf])])
                    .map(|_| ())
            }),
            Some("series"),
            Some("use NaN to leave a category without a bar."),
        ),
        (
            "stacked bar, infinite height",
            on_axes(cat, linear, |a| {
                a.stacked_bar(names(&["a"]), vec![series(&[inf])])
                    .map(|_| ())
            }),
            Some("series"),
            Some("use NaN to leave a category without a bar."),
        ),
        (
            "hist, bins out of range",
            on_axes(linear, linear, |a| {
                a.hist(
                    &[1.0],
                    HistOptions {
                        bins: 0,
                        ..HistOptions::default()
                    },
                )
                .map(|_| ())
            }),
            Some("bins"),
            None,
        ),
        (
            "hist, infinite value",
            on_axes(linear, linear, |a| {
                a.hist(&[inf], HistOptions::default()).map(|_| ())
            }),
            Some("values"),
            Some("a histogram ignores NaN, so use it to leave a value out."),
        ),
        (
            "xlim, inverted",
            on_axes(linear, linear, |a| a.xlim(2.0, 1.0).map(|_| ())),
            None,
            None,
        ),
        (
            "xlim, log with a non-positive lower limit",
            on_axes(LOG, linear, |a| a.xlim(0.0, 10.0).map(|_| ())),
            Some("low"),
            Some("use a linear axis if the range includes 0."),
        ),
        (
            "xlim, category axis",
            on_axes(cat, linear, |a| a.xlim(0.0, 1.0).map(|_| ())),
            None,
            None,
        ),
        (
            "second add_axes",
            {
                let mut f = figure();
                f.add_axes(linear, linear).expect("the first axes");
                f.add_axes(linear, linear)
                    .map(|_| ())
                    .expect_err("a second axes")
            },
            None,
            Some("create a new Figure for another chart."),
        ),
        (
            "Figure width",
            Figure::new(0.0, 4.0, Theme::pigment()).expect_err("a zero width"),
            Some("width"),
            None,
        ),
        (
            "Color.hex",
            Rgba::hex("#12").expect_err("a short code"),
            Some("code"),
            None,
        ),
        (
            "Color.rgb",
            Rgba::from_ints(0, 256, 0).expect_err("a channel above 255"),
            Some("g"),
            None,
        ),
        (
            "Color.alpha",
            Alpha::new(1.5).expect_err("an alpha above 1"),
            Some("a"),
            None,
        ),
        (
            "Scale.log",
            LogBase::new(1.0).expect_err("a base of 1"),
            Some("base"),
            None,
        ),
        (
            "Theme.with_line_width",
            Theme::pigment()
                .with_line_width(-1.0)
                .expect_err("a negative width"),
            Some("value"),
            None,
        ),
        (
            "Theme.with_palette",
            Theme::pigment()
                .with_palette(&[])
                .expect_err("an empty palette"),
            Some("colors"),
            None,
        ),
        (
            "Figure.to_png, dpi",
            figure().to_png(1).expect_err("a dpi below 10"),
            Some("dpi"),
            None,
        ),
        (
            "Figure.to_png, size",
            Figure::new(200.0, 200.0, Theme::pigment())
                .expect("a valid size")
                .to_png(2400)
                .expect_err("an oversized image"),
            None,
            None,
        ),
    ];
    for (name, err, parameter, hint) in cases {
        assert_eq!(err.parameter(), parameter, "{name}: {err}");
        assert_eq!(err.hint(), hint, "{name}: {err}");
        // The hint stays out of Display.
        if let Some(hint) = hint {
            assert!(!err.to_string().contains(hint), "{name}: {err}");
        }
    }
}

#[test]
fn display_is_unchanged_by_the_parameter_and_hint() {
    let err = on_axes(ScaleKind::Linear, ScaleKind::Linear, |a| {
        a.line(nums(&[1.0]), &[f64::INFINITY], LineOptions::default())
            .map(|_| ())
    });
    assert_eq!(
        err.to_string(),
        "Axes.line(): y[0] is inf; values must be finite."
    );
}
