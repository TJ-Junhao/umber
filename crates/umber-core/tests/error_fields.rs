//! The parameter and the hint of errors raised by the public API.

// clippy's allow-in-tests settings cover only `#[test]` functions, not the helpers here.
#![allow(clippy::expect_used)]

use umber_core::{
    Alpha, AreaOptions, AxisData, BarOptions, Error, Figure, HistOptions, LineOptions, LogBase,
    Result, Rgba, ScaleKind, ScatterOptions, Series, SizeSpec, Theme,
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

fn series(values: &[f64]) -> Series {
    Series {
        values: values.to_vec(),
        ..Series::default()
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
            "area, infinite y",
            on_axes(linear, linear, |a| {
                a.area(nums(&[1.0, 2.0]), &[1.0, inf], AreaOptions::default())
                    .map(|_| ())
            }),
            Some("y"),
            Some("use NaN to leave a gap in the area."),
        ),
        (
            "area, infinite x",
            on_axes(linear, linear, |a| {
                a.area(nums(&[-inf]), &[1.0], AreaOptions::default())
                    .map(|_| ())
            }),
            Some("x"),
            Some("use NaN to leave a gap in the area."),
        ),
        (
            "area, huge y",
            on_axes(linear, linear, |a| {
                a.area(nums(&[1.0]), &[1e301], AreaOptions::default())
                    .map(|_| ())
            }),
            Some("y"),
            None,
        ),
        (
            "area, log y with zero",
            on_axes(linear, LOG, |a| {
                a.area(nums(&[1.0, 2.0]), &[1.0, 0.0], AreaOptions::default())
                    .map(|_| ())
            }),
            Some("y"),
            Some("use a linear axis, or remove values that are not positive."),
        ),
        (
            "area, lengths differ",
            on_axes(linear, linear, |a| {
                a.area(nums(&[1.0, 2.0]), &[1.0], AreaOptions::default())
                    .map(|_| ())
            }),
            None,
            None,
        ),
        (
            "area, numbers on a category x axis",
            on_axes(cat, linear, |a| {
                a.area(nums(&[1.0]), &[1.0], AreaOptions::default())
                    .map(|_| ())
            }),
            None,
            Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."),
        ),
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
            Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."),
        ),
        (
            "bar, non-category x axis",
            on_axes(linear, linear, |a| {
                a.bar(names(&["a"]), &[1.0], BarOptions::default())
                    .map(|_| ())
            }),
            None,
            Some("pass x=Scale.category() to Figure.add_axes()."),
        ),
        (
            "hist, category x axis",
            on_axes(cat, linear, |a| {
                a.hist(&[1.0], HistOptions::default()).map(|_| ())
            }),
            None,
            Some("pass x=Scale.linear() to Figure.add_axes()."),
        ),
        (
            "hist, log x axis",
            on_axes(LOG, linear, |a| {
                a.hist(&[1.0], HistOptions::default()).map(|_| ())
            }),
            None,
            Some("pass x=Scale.linear() to Figure.add_axes()."),
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
            Some("combine repeated categories and sum their heights."),
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
            Some("use NaN to omit that height."),
        ),
        (
            "stacked bar, infinite height",
            on_axes(cat, linear, |a| {
                a.stacked_bar(names(&["a"]), vec![series(&[inf])])
                    .map(|_| ())
            }),
            Some("series"),
            Some("use NaN to omit that height."),
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
            Some("use at least one bin."),
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
            Some("use a linear axis if either limit is not positive."),
        ),
        (
            "xlim, category axis",
            on_axes(cat, linear, |a| a.xlim(0.0, 1.0).map(|_| ())),
            None,
            Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."),
        ),
        (
            "grouped bar, repeated category",
            on_axes(cat, linear, |a| {
                a.grouped_bar(names(&["a", "a"]), vec![series(&[1.0, 2.0])])
                    .map(|_| ())
            }),
            Some("categories"),
            Some("combine repeated categories and sum their heights within each series."),
        ),
        (
            "xlim, non-finite low",
            on_axes(linear, linear, |a| a.xlim(f64::NAN, 1.0).map(|_| ())),
            Some("low"),
            None,
        ),
        (
            "xlim, non-finite high",
            on_axes(linear, linear, |a| a.xlim(0.0, inf).map(|_| ())),
            Some("high"),
            None,
        ),
        (
            "xlim, low beyond the magnitude",
            on_axes(linear, linear, |a| a.xlim(-1e301, 0.0).map(|_| ())),
            Some("low"),
            None,
        ),
        (
            "xlim, high beyond the magnitude",
            on_axes(linear, linear, |a| a.xlim(0.0, 1e301).map(|_| ())),
            Some("high"),
            None,
        ),
        (
            "xlim, low below the log range",
            on_axes(LOG, linear, |a| a.xlim(1e-301, 1.0).map(|_| ())),
            Some("low"),
            None,
        ),
        (
            "xlim, high above the log range",
            on_axes(LOG, linear, |a| a.xlim(1.0, 1e301).map(|_| ())),
            Some("high"),
            None,
        ),
        (
            "ylim, non-finite low",
            on_axes(linear, linear, |a| a.ylim(f64::NAN, 1.0).map(|_| ())),
            Some("low"),
            None,
        ),
        (
            "ylim, non-finite high",
            on_axes(linear, linear, |a| a.ylim(0.0, inf).map(|_| ())),
            Some("high"),
            None,
        ),
        (
            "ylim, low beyond the magnitude",
            on_axes(linear, linear, |a| a.ylim(-1e301, 0.0).map(|_| ())),
            Some("low"),
            None,
        ),
        (
            "ylim, high beyond the magnitude",
            on_axes(linear, linear, |a| a.ylim(0.0, 1e301).map(|_| ())),
            Some("high"),
            None,
        ),
        (
            "ylim, low below the log range",
            on_axes(linear, LOG, |a| a.ylim(1e-301, 1.0).map(|_| ())),
            Some("low"),
            None,
        ),
        (
            "ylim, high above the log range",
            on_axes(linear, LOG, |a| a.ylim(1.0, 1e301).map(|_| ())),
            Some("high"),
            None,
        ),
        (
            "xlim, both limits non-finite",
            on_axes(linear, linear, |a| a.xlim(f64::NAN, inf).map(|_| ())),
            None,
            None,
        ),
        (
            "xlim, too narrow",
            on_axes(linear, linear, |a| a.xlim(1.0, 1.0 + 1e-12).map(|_| ())),
            None,
            None,
        ),
        (
            "ylim, log with both limits below the log range",
            on_axes(linear, LOG, |a| a.ylim(1e-302, 1e-301).map(|_| ())),
            None,
            None,
        ),
        (
            "add_axes, category y",
            figure()
                .add_axes(linear, cat)
                .map(|_| ())
                .expect_err("a category y axis"),
            Some("y"),
            Some("pass y=Scale.linear() or y=Scale.log()."),
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
        (
            "Size.times, zero",
            SizeSpec::times(0.0).expect_err("a zero factor"),
            Some("factor"),
            Some("use Size.AUTO for the theme's size, or a factor below 1 for a smaller one."),
        ),
        (
            "Size.times, above 10",
            SizeSpec::times(10.000_001).expect_err("a factor above 10"),
            Some("factor"),
            Some("use Size.pt() for a size in points."),
        ),
        (
            "Size.times, NaN",
            SizeSpec::times(f64::NAN).expect_err("a NaN factor"),
            Some("factor"),
            None,
        ),
        (
            "Size.pt, negative",
            SizeSpec::pt(-1.0).expect_err("negative points"),
            Some("points"),
            Some("use Size.AUTO for the theme's size."),
        ),
        (
            "Size.pt, above 72",
            SizeSpec::pt(72.1).expect_err("points above 72"),
            Some("points"),
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

#[test]
fn area_errors_name_the_area() {
    for (y, message) in [
        (
            f64::INFINITY,
            "Axes.area(): y[0] is inf; values must be finite.",
        ),
        (
            -1e301,
            "Axes.area(): y[0] is -1e301; magnitudes must not exceed 1e300.",
        ),
    ] {
        let err = on_axes(ScaleKind::Linear, ScaleKind::Linear, |a| {
            a.area(nums(&[1.0]), &[y], AreaOptions::default())
                .map(|_| ())
        });
        assert_eq!(err.to_string(), message);
        assert!(matches!(
            err,
            Error::Data {
                context: "Axes.area()",
                ..
            }
        ));
    }
}

#[test]
fn size_errors_name_the_bound() {
    for (err, message) in [
        (
            SizeSpec::times(0.0),
            "Size.times(): factor is 0; a size factor must be greater than 0 and at most 10.",
        ),
        (
            SizeSpec::times(-2.5),
            "Size.times(): factor is -2.5; a size factor must be greater than 0 and at most 10.",
        ),
        (
            SizeSpec::times(1e301),
            "Size.times(): factor is 1e301; a size factor must be greater than 0 and at most 10.",
        ),
        (
            SizeSpec::pt(0.0),
            "Size.pt(): points is 0; a size in points must be greater than 0 and at most 72.",
        ),
        (
            SizeSpec::pt(f64::INFINITY),
            "Size.pt(): points is inf; a size in points must be greater than 0 and at most 72.",
        ),
    ] {
        let err = err.expect_err("out of range");
        assert_eq!(err.to_string(), message);
        assert!(matches!(err, Error::Style { .. }));
    }
}
