//! Fixed-decimal automatic ticks must not silently disappear.

#![allow(clippy::unwrap_used)]

use umber_core::{
    AxisData, ColorBy, ColorbarOptions, ColorbarPosition, Error, Figure, FontLibrary, Grid,
    GridIndex, LineOptions, LogBase, Placement, PointColor, ScaleKind, ScatterOptions, Share,
    SvgText, Theme, TickFormat,
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
            let target = if axis == "colorbar" {
                "colorbar".to_owned()
            } else {
                format!("{axis} axis")
            };
            assert_eq!(
                error.to_string(),
                format!(
                    "{context}: the {target} runs from 0.005 to 0.009, but its tick format {name} cannot label any value in that range."
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

fn colorbar_figure(format: TickFormat, inherited: bool, shared: bool, hidden: bool) -> Figure {
    let grid = Grid::new(1, if shared { 3 } else { 1 })
        .unwrap()
        .with_share_color(if shared { Share::All } else { Share::None });
    let mut f = Figure::with_grid(12.0, 4.0, Theme::pigment(), grid).unwrap();
    let options = ColorbarOptions {
        format: if inherited {
            None
        } else {
            Some(format.clone())
        },
        position: hidden.then_some(ColorbarPosition::None),
        ..ColorbarOptions::default()
    };
    // The first color-mapped panel is neither the first inserted nor the first cell.
    for &col in if shared { &[2, 0, 1][..] } else { &[0][..] } {
        let mut a = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        if shared && col == 0 {
            a.line(
                AxisData::Numbers(vec![1.0, 2.0, 3.0]),
                &[0.0, 500.0, 1000.0],
                LineOptions::default(),
            )
            .unwrap()
            .ytick_format(TickFormat::fixed(1).unwrap())
            .unwrap();
        } else {
            a.scatter(
                AxisData::Numbers(vec![1.0, 2.0, 3.0]),
                &[0.0, 500.0, 1000.0],
                ScatterOptions {
                    color: PointColor::By(ColorBy::new(vec![0.005, 0.007, 0.009])),
                    ..ScatterOptions::default()
                },
            )
            .unwrap()
            .ytick_format(if shared && col == 2 {
                TickFormat::fixed(3).unwrap()
            } else {
                format.clone()
            })
            .unwrap();
            if !shared {
                a.colorbar(options.clone()).unwrap();
            }
        }
    }
    if shared {
        f.colorbar(options).unwrap();
    }
    f
}

#[test]
fn inherited_and_shared_colorbar_errors_name_the_format_source_and_setting() {
    for shared in [false, true] {
        for inherited in [false, true] {
            for (format, name) in formats() {
                let mut f = colorbar_figure(format, inherited, shared, false);
                let error = f.to_scene(FontLibrary::global()).unwrap_err();
                let target = if shared {
                    "shared colorbar"
                } else {
                    "colorbar"
                };
                let cell = if shared { "row 0, col 1: " } else { "" };
                let source = if inherited {
                    ", taken from the y tick format,"
                } else {
                    ""
                };
                assert_eq!(
                    error.to_string(),
                    format!(
                        "Figure.to_scene(): {cell}the {target} runs from 0.005 to 0.009, but its tick format {name}{source} cannot label any value in that range."
                    )
                );
                let constructor = if name.starts_with("percent") {
                    "percent"
                } else {
                    "fixed"
                };
                let hint = if inherited {
                    let owner = if shared { "fig" } else { "ax" };
                    format!(
                        "set the colorbar's own format with {owner}.colorbar(format=...), using more decimals with TickFormat.{constructor}()."
                    )
                } else {
                    format!("use more decimals with TickFormat.{constructor}().")
                };
                assert_eq!(error.hint(), Some(hint.as_str()));
                assert_eq!(error.parameter(), None);
                let options = ColorbarOptions {
                    format: Some(TickFormat::fixed(3).unwrap()),
                    ..ColorbarOptions::default()
                };
                if shared {
                    f.colorbar(options).unwrap();
                } else {
                    f.axes_at_mut(GridIndex::Auto, GridIndex::Auto)
                        .unwrap()
                        .colorbar(options)
                        .unwrap();
                }
                f.to_scene(FontLibrary::global()).unwrap();
            }
        }
    }
}

#[test]
fn hidden_colorbars_do_not_check_inherited_or_explicit_formats() {
    for shared in [false, true] {
        for inherited in [false, true] {
            for (format, _) in formats() {
                colorbar_figure(format, inherited, shared, true)
                    .to_scene(FontLibrary::global())
                    .unwrap();
            }
        }
    }
}

#[test]
fn shared_axes_check_the_final_combined_domain() {
    for axis in ["x", "y"] {
        for high in [0.8, 1.8] {
            let grid = Grid::new(1, 2).unwrap();
            let grid = if axis == "x" {
                grid.with_share_x(Share::All)
            } else {
                grid.with_share_y(Share::All)
            };
            let mut f = Figure::with_grid(10.0, 4.0, Theme::pigment(), grid).unwrap();
            for (col, values) in [(0, vec![0.2, 0.3]), (1, vec![0.7, high])] {
                let mut a = f
                    .add_axes_at(
                        Placement {
                            col: GridIndex::At(col),
                            ..Placement::default()
                        },
                        ScaleKind::Linear,
                        ScaleKind::Linear,
                    )
                    .unwrap();
                if axis == "x" {
                    a.line(
                        AxisData::Numbers(values),
                        &[1.0, 2.0],
                        LineOptions::default(),
                    )
                    .unwrap()
                    .xtick_format(TickFormat::fixed(0).unwrap())
                    .unwrap();
                } else {
                    a.line(
                        AxisData::Numbers(vec![1.0, 2.0]),
                        &values,
                        LineOptions::default(),
                    )
                    .unwrap()
                    .ytick_format(TickFormat::fixed(0).unwrap())
                    .unwrap();
                }
            }
            let result = f.to_scene(FontLibrary::global());
            if high == 0.8 {
                let error = result.unwrap_err();
                let bounds = if axis == "x" {
                    "0.2 to 0.8"
                } else {
                    "0.17 to 0.83"
                };
                assert_eq!(
                    error.to_string(),
                    format!(
                        "Figure.to_scene(): row 0, col 0: the {axis} axis runs from {bounds}, but its tick format fixed(0) cannot label any value in that range."
                    )
                );
                assert_eq!(
                    error.hint(),
                    Some("use more decimals with TickFormat.fixed(), or set the ticks explicitly.")
                );
            } else {
                result.unwrap();
            }
        }
    }
}

#[test]
fn error_bounds_use_at_most_six_significant_digits_without_trailing_zeros() {
    for (low, high, expected) in [
        (0.17, 0.8300000000000001, "0.17 to 0.83"),
        (0.0006000000000000001, 0.0094, "0.0006 to 0.0094"),
        (2.17, 2.8 + 0.03, "2.17 to 2.83"),
        (1.23456789, 1.87654321, "1.23457 to 1.87654"),
        (-0.8300000000000001, -0.17, "-0.83 to -0.17"),
        (5e-9, 9e-9, "5e-9 to 9e-9"),
    ] {
        let f = figure("y", TickFormat::fixed(0).unwrap(), low, high, false);
        let error = f.to_scene(FontLibrary::global()).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "Figure.to_scene(): the y axis runs from {expected}, but its tick format fixed(0) cannot label any value in that range."
            )
        );
    }
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
