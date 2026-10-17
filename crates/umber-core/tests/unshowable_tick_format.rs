//! Fixed-decimal automatic ticks must not silently disappear.

#![allow(clippy::unwrap_used)]

use umber_core::{
    AxisData, ColorBy, ColorbarOptions, Error, Figure, FontLibrary, GridIndex, LineOptions,
    LogBase, PointColor, ScaleKind, ScatterOptions, SvgText, Theme, TickFormat,
};

fn formats() -> [(TickFormat, &'static str); 3] {
    [
        (TickFormat::fixed(0).unwrap(), "fixed(0)"),
        (TickFormat::fixed(1).unwrap(), "fixed(1)"),
        (TickFormat::percent(0).unwrap(), "percent(0)"),
    ]
}

fn figure(axis: &str, format: TickFormat, low: f64, high: f64, log: bool) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let scale = if log {
        ScaleKind::Log { base: LogBase::TEN }
    } else {
        ScaleKind::Linear
    };
    let mut a = f.add_axes(scale, scale).unwrap();
    match axis {
        "x" => {
            a.line(
                AxisData::Numbers(vec![low, high]),
                &[1.0, 2.0],
                LineOptions::default(),
            )
            .unwrap()
            .xlim(low, high)
            .unwrap()
            .xtick_format(format)
            .unwrap();
        }
        "y" => {
            a.line(
                AxisData::Numbers(vec![1.0, 2.0]),
                &[low, high],
                LineOptions::default(),
            )
            .unwrap()
            .ylim(low, high)
            .unwrap()
            .ytick_format(format)
            .unwrap();
        }
        "colorbar" => {
            a.scatter(
                AxisData::Numbers(vec![1.0, 2.0]),
                &[1.0, 2.0],
                ScatterOptions {
                    color: PointColor::By(ColorBy::new(vec![low, high])),
                    ..ScatterOptions::default()
                },
            )
            .unwrap()
            .colorbar(ColorbarOptions {
                format: Some(format),
                ..ColorbarOptions::default()
            })
            .unwrap();
        }
        _ => unreachable!(),
    }
    f
}

fn assert_unshowable(axis: &str) {
    for (format, name) in formats() {
        let f = figure(axis, format, 0.005, 0.009, false);
        for (context, error) in [
            (
                "Figure.to_scene()",
                f.to_scene(FontLibrary::global()).unwrap_err(),
            ),
            ("Figure.to_svg()", f.to_svg(SvgText::Editable).unwrap_err()),
            ("Figure.to_png()", f.to_png(72).unwrap_err()),
        ] {
            assert!(matches!(error, Error::Style { .. }));
            assert_eq!(
                error.to_string(),
                format!(
                    "{context}: the {axis} tick format {name} cannot label any value between 0.005 and 0.009."
                )
            );
            let constructor = if name.starts_with("percent") {
                "percent"
            } else {
                "fixed"
            };
            let ending = if axis == "colorbar" {
                "."
            } else {
                ", or set the ticks explicitly."
            };
            assert_eq!(
                error.hint().unwrap(),
                format!("use more decimals with TickFormat.{constructor}(){ending}")
            );
            // Rendering has no tick-format argument to highlight.
            assert_eq!(error.parameter(), None);
        }
    }
}

#[test]
fn unshowable_x_tick_format() {
    assert_unshowable("x");
}

#[test]
fn unshowable_y_tick_format() {
    assert_unshowable("y");
}

#[test]
fn unshowable_colorbar_tick_format() {
    assert_unshowable("colorbar");
}

#[test]
fn explicit_ticks_still_render_with_unshowable_formats() {
    for axis in ["x", "y"] {
        for (format, _) in formats() {
            for positions in [&[0.007][..], &[][..]] {
                let mut f = figure(axis, format.clone(), 0.005, 0.009, false);
                let mut a = f.axes_at_mut(GridIndex::Auto, GridIndex::Auto).unwrap();
                if axis == "x" {
                    a.xticks(positions).unwrap();
                } else {
                    a.yticks(positions).unwrap();
                }
                f.to_scene(FontLibrary::global()).unwrap();
            }
        }
    }
}

#[test]
fn one_representable_value_keeps_one_tick() {
    for axis in ["x", "y", "colorbar"] {
        for (format, _) in formats() {
            for center in [-1.0, 0.0, 1.0] {
                let f = figure(
                    axis,
                    format.affix("only:", "").unwrap(),
                    center - 0.004,
                    center + 0.004,
                    false,
                );
                let svg = f.to_svg(SvgText::Editable).unwrap();
                assert_eq!(svg.matches(">only:").count(), 1, "{axis}: {svg}");
            }
        }
    }
}

#[test]
fn log_axes_keep_their_ticks_with_fixed_decimals() {
    for axis in ["x", "y"] {
        for (format, _) in formats() {
            figure(axis, format, 0.005, 0.009, true)
                .to_scene(FontLibrary::global())
                .unwrap();
        }
    }
}
