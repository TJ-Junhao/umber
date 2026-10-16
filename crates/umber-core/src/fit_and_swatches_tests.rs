//! Automatic bubble containment and the displayed colors of filled legend swatches.

use crate::pipeline::{plan_figure, try_plan_panels};
use crate::resolve::resolve_marks;
use crate::{
    AxisData, BandOptions, BubbleOptions, ColorSpec, Figure, FontLibrary, Grid, GridIndex, Item,
    LegendMode, LegendStyle, LineOptions, LogBase, Paint, Placement, Rgba, ScaleKind, Share,
    SpanOptions, Theme,
};

fn bubbles(f: &mut Figure, x: &[f64], y: &[f64], sizes: &[f64]) {
    f.single_axes_mut()
        .bubbles(x, y, sizes, BubbleOptions::default())
        .unwrap();
}

fn assert_bubbles_inside(f: &Figure) {
    let planned = try_plan_panels(f, FontLibrary::global()).unwrap();
    for (axes, (resolved, layout)) in f.panels().zip(&planned) {
        let marks = resolve_marks(axes.marks(), f.theme());
        for mark in marks
            .iter()
            .filter(|m| matches!(m, crate::resolve::ResolvedMark::Bubbles { .. }))
        {
            let Item::Group { items, .. } =
                crate::draw::marks(f.theme(), resolved, layout, std::slice::from_ref(mark))
                    .unwrap()
            else {
                panic!("mark group")
            };
            let crate::resolve::ResolvedMark::Bubbles {
                mark: data,
                maximum,
                radius,
                ..
            } = mark
            else {
                unreachable!()
            };
            let expected = data
                .x()
                .iter()
                .zip(data.y())
                .zip(data.sizes())
                .filter(|((x, y), size)| {
                    x.is_finite()
                        && y.is_finite()
                        && crate::bubble_scale::radius(**size, *maximum, *radius) > 0.0
                })
                .count();
            assert_eq!(items.len(), 2 * expected, "no circle may be culled");
            for item in items {
                let (path, half) = match item {
                    Item::Fill { path, .. } => (path, 0.0),
                    Item::Stroke { path, style } => (path, f64::from(style.width) / 2.0),
                    _ => panic!("bubble path"),
                };
                let (lo, hi) = path.bounds().unwrap();
                let p = layout.plot;
                let tolerance = 1e-4;
                assert!(
                    f64::from(lo.x) - half >= p.x - tolerance,
                    "left: {lo:?}, {p:?}"
                );
                assert!(
                    f64::from(hi.x) + half <= p.right() + tolerance,
                    "right: {hi:?}, {p:?}"
                );
                assert!(
                    f64::from(lo.y) - half >= p.y - tolerance,
                    "top: {lo:?}, {p:?}"
                );
                assert!(
                    f64::from(hi.y) + half <= p.bottom() + tolerance,
                    "bottom: {hi:?}, {p:?}"
                );
            }
        }
    }
}

#[test]
fn automatic_bubbles_fit_all_population_circles() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Cities")
        .subtitle("Marker area shows population, in thousands")
        .xlabel("Income")
        .ylabel("Output")
        .bubbles(
            &[12.0, 18.0, 24.0, 35.0, 42.0, 55.0, 67.0, 78.0],
            &[3.2, 4.8, 2.9, 6.4, 5.7, 7.1, 4.2, 8.3],
            &[80.0, 140.0, 220.0, 500.0, 330.0, 870.0, 190.0, 650.0],
            BubbleOptions {
                label: "Cities".into(),
                ..BubbleOptions::default()
            },
        )
        .unwrap();
    assert_bubbles_inside(&f);
}

#[test]
fn automatic_bubbles_fit_single_with_and_without_explicit_ticks() {
    for explicit_ticks in [false, true] {
        let mut f = Figure::new(3.0, 2.0, Theme::pigment()).unwrap();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        if explicit_ticks {
            a.xticks(&[0.0, 1.0]).unwrap().yticks(&[0.0, 1.0]).unwrap();
        }
        bubbles(&mut f, &[1.0], &[1.0], &[1.0]);
        assert_bubbles_inside(&f);
    }
}

#[test]
fn automatic_bubbles_fit_log_x_and_log_y() {
    for (x, y) in [
        (ScaleKind::Log { base: LogBase::TEN }, ScaleKind::Linear),
        (ScaleKind::Linear, ScaleKind::Log { base: LogBase::TEN }),
        (
            ScaleKind::Log {
                base: LogBase::new(2.0).unwrap(),
            },
            ScaleKind::Log { base: LogBase::TEN },
        ),
    ] {
        let mut f = Figure::new(3.0, 2.5, Theme::midnight()).unwrap();
        f.add_axes(x, y)
            .unwrap()
            .xticks(&[1.0, 100.0])
            .unwrap()
            .yticks(&[1.0, 100.0])
            .unwrap();
        bubbles(&mut f, &[1.0, 100.0], &[100.0, 1.0], &[1.0, 1.0]);
        assert_bubbles_inside(&f);
    }
}

#[test]
fn automatic_bubbles_fit_when_mixed_with_a_line() {
    let mut f = Figure::new(3.0, 2.5, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions::default(),
        )
        .unwrap();
    bubbles(&mut f, &[0.0, 1.0], &[1.0, 0.0], &[1.0, 1.0]);
    assert_bubbles_inside(&f);
}

#[test]
fn automatic_bubbles_fit_every_shared_panel() {
    let grid = Grid::new(1, 2)
        .unwrap()
        .with_share_x(Share::All)
        .with_share_y(Share::All);
    let mut f = Figure::with_grid(6.0, 2.5, Theme::pigment(), grid).unwrap();
    for col in 0..2 {
        f.add_axes_at(
            Placement {
                row: GridIndex::At(0),
                col: GridIndex::At(col),
                ..Placement::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap()
        .bubbles(
            &[col as f64],
            &[col as f64],
            &[1.0],
            BubbleOptions::default(),
        )
        .unwrap();
    }
    assert_bubbles_inside(&f);
    let plans = try_plan_panels(&f, FontLibrary::global()).unwrap();
    assert_eq!(plans[0].0.x, plans[1].0.x);
    assert_eq!(plans[0].0.y, plans[1].0.y);
}

#[test]
fn automatic_bubbles_leave_explicit_limits_and_clipping_unchanged() {
    let mut f = Figure::new(3.0, 2.5, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap();
    bubbles(&mut f, &[1.0], &[1.0], &[1.0]);
    let (r, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    assert_eq!(r.x.continuous().unwrap().domain(), (0.0, 1.0));
    assert_eq!(r.y.scale.domain(), (0.0, 1.0));
    let marks = resolve_marks(f.single_axes().marks(), f.theme());
    let Item::Group {
        items,
        clip: Some(clip),
    } = crate::draw::marks(f.theme(), &r, &layout, &marks).unwrap()
    else {
        panic!("clipped marks")
    };
    let Item::Fill { path, .. } = &items[0] else {
        panic!("bubble fill")
    };
    let (lo, hi) = path.bounds().unwrap();
    assert!(lo.y < clip.y && hi.x > clip.x + clip.width);
}

#[test]
fn automatic_bubbles_only_fit_the_axis_without_explicit_limits() {
    let mut f = Figure::new(0.75, 3.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .xticks(&[])
        .unwrap()
        .yticks(&[])
        .unwrap();
    bubbles(&mut f, &[1.0], &[1.0], &[1.0]);
    let (r, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    assert_eq!(r.x.continuous().unwrap().domain(), (0.0, 1.0));
    assert!(layout.plot.width < 36.8);
    let position = r.y.scale.normalize(1.0);
    assert!(position * layout.plot.height >= 18.4);
    assert!((1.0 - position) * layout.plot.height >= 18.4);
    assert!(f.to_scene(FontLibrary::global()).is_ok());
}

#[test]
fn automatic_bubbles_reject_a_diameter_larger_than_the_plot() {
    let mut f = Figure::new(0.75, 0.75, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .legend(LegendStyle::None)
        .xticks(&[])
        .unwrap()
        .yticks(&[])
        .unwrap();
    bubbles(&mut f, &[0.0], &[0.0], &[1.0]);
    let error = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(error, crate::Error::Style { .. }));
    assert!(error.to_string().contains("bubble"), "{error}");
    assert!(error.hint().is_some());
}

fn composite(foreground: Rgba, background: Rgba) -> Rgba {
    let a = u32::from(foreground.a);
    let channel = |f, b| ((u32::from(f) * a + u32::from(b) * (255 - a) + 127) / 255) as u8;
    Rgba::opaque(
        channel(foreground.r, background.r),
        channel(foreground.g, background.g),
        channel(foreground.b, background.b),
    )
}

#[test]
fn filled_swatches_cover_all_fill_kinds_and_preserve_series_opacity() {
    use crate::{
        AreaOptions, BarOptions, BoxOptions, FillStyle, HistOptions, Series, ViolinOptions,
    };
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for fill_style in [FillStyle::Solid, FillStyle::Gradient] {
            for style in [LegendStyle::Header, LegendStyle::Chips] {
                for alpha in [255, 128] {
                    let theme = theme
                        .with_fill_style(fill_style)
                        .unwrap()
                        .with_legend_style(style)
                        .unwrap();
                    let color = Rgba {
                        a: alpha,
                        ..Rgba::RED
                    };
                    let spec = ColorSpec::Fixed(color);
                    for name in [
                        "area",
                        "stacked-area",
                        "histogram",
                        "bar",
                        "box",
                        "violin",
                        "bubbles",
                        "band",
                        "hspan",
                        "vspan",
                    ] {
                        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
                        let category = matches!(name, "bar" | "box" | "violin");
                        let mut a = f
                            .add_axes(
                                if category {
                                    ScaleKind::Category
                                } else {
                                    ScaleKind::Linear
                                },
                                ScaleKind::Linear,
                            )
                            .unwrap();
                        let label = name.to_owned();
                        let opacity = match name {
                            "area" => {
                                a.area(
                                    AxisData::Numbers(vec![0.0, 1.0]),
                                    &[1.0, 2.0],
                                    AreaOptions {
                                        label,
                                        color: spec,
                                        ..AreaOptions::default()
                                    },
                                )
                                .unwrap();
                                0.32
                            }
                            "stacked-area" => {
                                a.stacked_area(
                                    &[0.0, 1.0],
                                    vec![Series {
                                        label,
                                        color: spec,
                                        values: vec![1.0, 2.0],
                                    }],
                                )
                                .unwrap();
                                0.32
                            }
                            "histogram" => {
                                a.hist(
                                    &[1.0, 2.0, 3.0],
                                    HistOptions {
                                        label,
                                        color: spec,
                                        bins: 2,
                                    },
                                )
                                .unwrap();
                                if fill_style == FillStyle::Gradient {
                                    0.95
                                } else {
                                    1.0
                                }
                            }
                            "bar" => {
                                a.bar(vec!["A".into()], &[1.0], BarOptions { label, color: spec })
                                    .unwrap();
                                1.0
                            }
                            "box" => {
                                a.box_plot(
                                    vec!["A".into()],
                                    vec![vec![1.0, 2.0, 3.0]],
                                    BoxOptions { label, color: spec },
                                )
                                .unwrap();
                                0.32
                            }
                            "violin" => {
                                a.violin(
                                    vec!["A".into()],
                                    vec![vec![1.0, 2.0, 3.0]],
                                    ViolinOptions { label, color: spec },
                                )
                                .unwrap();
                                0.32
                            }
                            "bubbles" => {
                                a.bubbles(
                                    &[0.5],
                                    &[0.5],
                                    &[1.0],
                                    BubbleOptions {
                                        label,
                                        color: spec,
                                        ..BubbleOptions::default()
                                    },
                                )
                                .unwrap();
                                theme.marker_alpha()
                            }
                            "band" => {
                                a.band(
                                    &[0.0, 1.0],
                                    &[1.0, 2.0],
                                    &[2.0, 3.0],
                                    BandOptions { label, color: spec },
                                )
                                .unwrap();
                                0.32
                            }
                            "hspan" => {
                                a.hspan(1.0, 2.0, SpanOptions { label, color: spec })
                                    .unwrap();
                                0.32
                            }
                            "vspan" => {
                                a.vspan(1.0, 2.0, SpanOptions { label, color: spec })
                                    .unwrap();
                                0.32
                            }
                            _ => unreachable!(),
                        };
                        let expected = composite(color.scale_alpha(opacity), theme.background());
                        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
                        for entry in layout.legend.entries() {
                            assert_eq!(
                                entry.swatch.fill, expected,
                                "{name}: {fill_style:?}, {style:?}, alpha {alpha}"
                            );
                            assert_eq!(entry.swatch.color, color);
                        }
                        let mut colors = Vec::new();
                        collect_colors(
                            &crate::draw::legend::legend(&theme, &layout).unwrap(),
                            &mut colors,
                        );
                        assert!(colors.contains(&expected), "{name}: {colors:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn filled_swatches_do_not_change_direct_area_labels_or_scatter() {
    use crate::{AreaOptions, ScatterOptions};
    let color = Rgba {
        a: 128,
        ..Rgba::RED
    };
    let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .area(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[1.0, 2.0],
            AreaOptions {
                label: "Area".into(),
                color: ColorSpec::Fixed(color),
                ..AreaOptions::default()
            },
        )
        .unwrap()
        .scatter(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[1.0, 2.0],
            ScatterOptions {
                label: "Scatter".into(),
                color: ColorSpec::Fixed(color),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    assert_eq!(layout.direct[0].color, color);
    assert_eq!(layout.legend.entries()[0].swatch.fill, color);
}

fn collect_colors(item: &Item, colors: &mut Vec<Rgba>) {
    match item {
        Item::Stroke { style, .. } => colors.push(style.color),
        Item::Markers {
            fill: Some(color), ..
        } => colors.push(*color),
        Item::Fill {
            paint: Paint::Solid(color),
            ..
        } => colors.push(*color),
        Item::Group { items, .. } => items.iter().for_each(|item| collect_colors(item, colors)),
        _ => {}
    }
}

#[test]
fn filled_swatches_match_band_and_spans_in_every_theme_and_legend_mode() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for mode in [LegendMode::PerPanel, LegendMode::Merged] {
            for style in [LegendStyle::Header, LegendStyle::Chips, LegendStyle::Direct] {
                let theme = theme.with_legend_style(style).unwrap();
                let mut f = Figure::with_grid(
                    8.0,
                    4.0,
                    theme.clone(),
                    Grid::new(1, 2).unwrap().with_legend(mode),
                )
                .unwrap();
                for col in 0..2 {
                    let mut a = f
                        .add_axes_at(
                            Placement {
                                row: GridIndex::At(0),
                                col: GridIndex::At(col),
                                ..Placement::default()
                            },
                            ScaleKind::Linear,
                            ScaleKind::Linear,
                        )
                        .unwrap();
                    a.band(
                        &[0.0, 1.0],
                        &[1.0, 2.0],
                        &[2.0, 3.0],
                        BandOptions {
                            label: "Band".into(),
                            color: ColorSpec::Fixed(Rgba::RED),
                        },
                    )
                    .unwrap();
                    a.hspan(
                        1.0,
                        2.0,
                        SpanOptions {
                            label: "Horizontal".into(),
                            color: ColorSpec::Fixed(Rgba::GREEN),
                        },
                    )
                    .unwrap();
                    a.vspan(
                        0.2,
                        0.8,
                        SpanOptions {
                            label: "Vertical".into(),
                            color: ColorSpec::Fixed(Rgba::BLUE),
                        },
                    )
                    .unwrap();
                }
                let scene = f.to_scene(FontLibrary::global()).unwrap();
                let mut colors = Vec::new();
                for item in &scene.items {
                    collect_colors(item, &mut colors);
                }
                for color in [Rgba::RED, Rgba::GREEN, Rgba::BLUE] {
                    let expected = composite(color.scale_alpha(0.32), theme.background());
                    assert!(
                        colors.contains(&expected),
                        "{mode:?}, {style:?}: missing {expected:?} in {colors:?}"
                    );
                }
            }
        }
    }
}
