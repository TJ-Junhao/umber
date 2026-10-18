//! Panel legend placement and room checks.

use crate::layout::{Area, Layout};
use crate::pipeline::{plan_figure, try_plan_panels};
use crate::{
    AxisData, Figure, FontLibrary, Grid, GridIndex, LegendMode, LegendPosition, LegendStyle,
    LineOptions, Placement, ScaleKind, ScatterOptions, Theme,
};

fn figure(position: LegendPosition) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .legend_at(LegendStyle::Header, position, crate::SizeSpec::Auto)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .line(
            AxisData::Numbers(vec![f64::NAN]),
            &[f64::NAN],
            LineOptions {
                label: "Signal".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    f
}

fn layout(f: &Figure) -> Layout {
    plan_figure(f, (0, 0), FontLibrary::global()).1
}

fn box_of(f: &Figure) -> (Area, Area) {
    let l = layout(f);
    (l.plot, l.legend.background.unwrap())
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} != {b}");
}

#[test]
fn best_picks_the_empty_corner() {
    let corners = [(0.97, 0.95), (0.03, 0.95), (0.97, 0.05), (0.03, 0.05)];
    for empty in 0..4 {
        let mut f = figure(LegendPosition::Best);
        let points: Vec<_> = corners
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != empty)
            .map(|(_, p)| *p)
            .collect();
        f.single_axes_mut()
            .scatter(
                AxisData::Numbers(points.iter().map(|p| p.0).collect()),
                &points.iter().map(|p| p.1).collect::<Vec<_>>(),
                ScatterOptions::default(),
            )
            .unwrap();
        let (p, b) = box_of(&f);
        near(
            b.x,
            if empty % 2 == 0 {
                p.right() - 6.0 - b.width
            } else {
                p.x + 6.0
            },
        );
        near(
            b.y,
            if empty < 2 {
                p.y + 6.0
            } else {
                p.bottom() - 6.0 - b.height
            },
        );
    }
}

#[test]
fn best_ties_follow_the_order() {
    let f = figure(LegendPosition::Best);
    let (p, b) = box_of(&f);
    near(b.right(), p.right() - 6.0);
    near(b.y, p.y + 6.0);
    assert_eq!(
        f.to_svg(crate::SvgText::Outlines).unwrap(),
        f.to_svg(crate::SvgText::Outlines).unwrap()
    );
}

#[test]
fn best_falls_back_to_least_covered() {
    let mut f = figure(LegendPosition::Best);
    // The span covers every candidate equally. Only the top-right also covers a point.
    f.single_axes_mut()
        .hspan(0.0, 1.0, crate::SpanOptions::default())
        .unwrap()
        .scatter(
            AxisData::Numbers(vec![0.97]),
            &[0.95],
            ScatterOptions::default(),
        )
        .unwrap();
    let (p, b) = box_of(&f);
    near(b.x, p.x + 6.0);
    near(b.y, p.y + 6.0);
    assert!(b.x >= p.x && b.y >= p.y && b.right() <= p.right() && b.bottom() <= p.bottom());
}

#[test]
fn best_avoids_stacked_area_layers() {
    let mut f = figure(LegendPosition::Best);
    f.single_axes_mut()
        .stacked_area(
            &[0.5, 1.0],
            vec![
                crate::Series {
                    label: "Lower".into(),
                    values: vec![0.5, 0.5],
                    ..crate::Series::default()
                },
                crate::Series {
                    label: "Upper".into(),
                    values: vec![0.5, 0.5],
                    ..crate::Series::default()
                },
            ],
        )
        .unwrap();
    let (p, b) = box_of(&f);
    near(b.x, p.x + 6.0);
    near(b.y, p.y + 6.0);
}

#[test]
fn right_and_bottom_reserve_room_and_keep_plots_aligned() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = Figure::with_grid(
            10.0,
            7.0,
            theme,
            Grid::new(2, 2).unwrap().with_legend(LegendMode::PerPanel),
        )
        .unwrap();
        for (i, position) in [
            LegendPosition::Right,
            LegendPosition::Bottom,
            LegendPosition::TopRight,
            LegendPosition::Best,
        ]
        .into_iter()
        .enumerate()
        {
            f.add_axes_at(
                Placement {
                    row: GridIndex::At((i / 2) as i64),
                    col: GridIndex::At((i % 2) as i64),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap()
            .legend_at(LegendStyle::Header, position, crate::SizeSpec::Auto)
            .unwrap()
            .xlabel("Time")
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[0.0, 1.0],
                LineOptions {
                    label: "Signal".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        let panels = try_plan_panels(&f, FontLibrary::global()).unwrap();
        let ls: Vec<_> = panels.iter().map(|(_, l)| l).collect();
        for l in &ls {
            near(l.plot.width, ls[0].plot.width);
            near(l.plot.height, ls[0].plot.height);
        }
        near(ls[0].plot.x, ls[2].plot.x);
        near(ls[1].plot.x, ls[3].plot.x);
        near(ls[0].plot.y, ls[1].plot.y);
        near(ls[2].plot.y, ls[3].plot.y);
        near(
            ls[0].legend.entries()[0].swatch.x,
            ls[0].plot.right() + 12.0,
        );
        let entry = &ls[1].legend.entries()[0];
        near(entry.chip.map_or(entry.swatch.x, |c| c.x), ls[1].plot.x);
        let label = ls[1].xlabel.as_ref().unwrap();
        assert!(ls[1].legend.entries()[0].swatch.center_y > label.y + 8.0);
    }
    let base = figure(LegendPosition::TopLeft);
    let right = figure(LegendPosition::Right);
    let bottom = figure(LegendPosition::Bottom);
    assert!(layout(&right).plot.width < layout(&base).plot.width);
    assert!(layout(&bottom).plot.bottom() < layout(&base).plot.bottom());
}

#[test]
fn single_bottom_wraps_and_reserves_the_inner_canvas_width() {
    for style in [LegendStyle::Header, LegendStyle::Chips] {
        for xlabel in ["", "Time"] {
            let make = |position| {
                let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
                let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
                axes.legend_at(style, position, crate::SizeSpec::Auto)
                    .unwrap()
                    .xlabel(xlabel)
                    .yticks_labeled(&[0.0, 1.0], vec!["1234567890".repeat(3); 2])
                    .unwrap();
                for i in 0..5 {
                    axes.line(
                        AxisData::Numbers(vec![0.0, 1.0]),
                        &[0.0, 1.0],
                        LineOptions {
                            label: format!("Series {i}"),
                            ..LineOptions::default()
                        },
                    )
                    .unwrap();
                }
                f
            };
            let top = layout(&make(LegendPosition::TopLeft));
            let bottom = layout(&make(LegendPosition::Bottom));
            near(bottom.legend.height(), top.legend.height());
            assert!(bottom.legend.width() > bottom.plot.width);
            let first = &bottom.legend.entries()[0];
            near(first.chip.map_or(first.swatch.x, |c| c.x), 12.0);
            for entry in bottom.legend.entries() {
                let end = entry
                    .chip
                    .map_or(entry.label.x + entry.label.width(), |c| c.right());
                assert!(end <= bottom.frame.right() - 12.0);
            }
            near(
                first.swatch.center_y + bottom.legend.height() / 2.0,
                bottom.frame.bottom() - 12.0,
            );
            let axis_bottom = bottom.xlabel.as_ref().map_or_else(
                || bottom.x_tick_labels.iter().map(|l| l.y).fold(0.0, f64::max),
                |l| l.y,
            );
            assert!(first.swatch.center_y - bottom.legend.height() / 2.0 > axis_bottom + 8.0);
        }
    }
}

#[test]
fn direct_style_rejects_a_position() {
    for (position, name) in [
        (LegendPosition::TopLeft, "LegendPosition.TOP_LEFT"),
        (LegendPosition::TopRight, "LegendPosition.TOP_RIGHT"),
        (LegendPosition::Right, "LegendPosition.RIGHT"),
        (LegendPosition::Bottom, "LegendPosition.BOTTOM"),
        (LegendPosition::Best, "LegendPosition.BEST"),
    ] {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .legend_at(
                LegendStyle::Header,
                LegendPosition::Bottom,
                crate::SizeSpec::Auto,
            )
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[0.0, 1.0],
                LineOptions {
                    label: "Signal".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        let before = f.clone();
        let err = f
            .single_axes_mut()
            .legend_at(LegendStyle::Direct, position, crate::SizeSpec::Auto)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "Axes.legend(): position is {name}; direct labels sit at the ends of their lines and take no position."
            )
        );
        assert_eq!(err.parameter(), Some("position"));
        assert_eq!(
            err.hint(),
            Some("drop position=, or use another legend style.")
        );
        assert_eq!(f, before);
    }
}

#[test]
fn best_uses_all_eight_candidates_in_order() {
    let (p, b) = box_of(&figure(LegendPosition::Best));
    let left = p.x + 6.0;
    let right = p.right() - 6.0 - b.width;
    let top = p.y + 6.0;
    let bottom = p.bottom() - 6.0 - b.height;
    let middle_x = p.x + (p.width - b.width) / 2.0;
    let middle_y = p.y + (p.height - b.height) / 2.0;
    let candidates = [
        (right, top),
        (left, top),
        (right, bottom),
        (left, bottom),
        (middle_x, top),
        (right, middle_y),
        (middle_x, bottom),
        (left, middle_y),
    ];
    for (index, &(x, y)) in candidates.iter().enumerate() {
        let mut f = figure(LegendPosition::Best);
        let occupied = &candidates[..index];
        f.single_axes_mut()
            .scatter(
                AxisData::Numbers(
                    occupied
                        .iter()
                        .map(|&(x, _)| (x + b.width / 2.0 - p.x) / p.width)
                        .collect(),
                ),
                &occupied
                    .iter()
                    .map(|&(_, y)| (p.bottom() - y - b.height / 2.0) / p.height)
                    .collect::<Vec<_>>(),
                ScatterOptions::default(),
            )
            .unwrap();
        let (_, chosen) = box_of(&f);
        near(chosen.x, x);
        near(chosen.y, y);
    }
}

#[test]
fn theme_position_is_inherited_and_legend_resets_an_override() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        assert_eq!(theme.legend_position(), LegendPosition::TopLeft);
        assert_eq!(theme.legend_background_alpha(), 0.85);
        let theme = theme.with_legend_position(LegendPosition::Right).unwrap();
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .legend(LegendStyle::Header, crate::SizeSpec::Auto)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[0.0, 1.0],
                LineOptions {
                    label: "Signal".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        let inherited = f.to_svg(crate::SvgText::Outlines).unwrap();
        let l = layout(&f);
        near(l.legend.entries()[0].swatch.x, l.plot.right() + 12.0);
        f.single_axes_mut()
            .legend_at(
                LegendStyle::Header,
                LegendPosition::Bottom,
                crate::SizeSpec::Auto,
            )
            .unwrap();
        assert_ne!(f.to_svg(crate::SvgText::Outlines).unwrap(), inherited);
        f.single_axes_mut()
            .legend(LegendStyle::Header, crate::SizeSpec::Auto)
            .unwrap();
        assert_eq!(f.single_axes().legend_position(), None);
        assert_eq!(f.to_svg(crate::SvgText::Outlines).unwrap(), inherited);
    }
}

#[test]
fn merged_grid_ignores_panel_positions() {
    let render = |position| {
        let mut f =
            Figure::with_grid(8.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
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
            .legend_at(LegendStyle::Header, position, crate::SizeSpec::Auto)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[0.0, 1.0],
                LineOptions {
                    label: "Signal".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        f.to_svg(crate::SvgText::Outlines).unwrap()
    };
    let baseline = render(LegendPosition::TopLeft);
    for position in [
        LegendPosition::TopRight,
        LegendPosition::Right,
        LegendPosition::Bottom,
        LegendPosition::Best,
    ] {
        assert_eq!(render(position), baseline);
    }
}

fn room_error(
    position: LegendPosition,
    grid: bool,
    tall: bool,
    style: LegendStyle,
) -> crate::Error {
    let mut f = if grid {
        Figure::with_grid(
            6.0,
            4.0,
            Theme::pigment(),
            Grid::new(1, 2).unwrap().with_legend(LegendMode::PerPanel),
        )
        .unwrap()
    } else {
        Figure::new(6.0, 4.0, Theme::pigment()).unwrap()
    };
    let mut axes = f
        .add_axes_at(
            Placement {
                row: GridIndex::At(0),
                col: GridIndex::At(i64::from(grid)),
                ..Placement::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap();
    axes.legend_at(style, position, crate::SizeSpec::Auto)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap();
    for i in 0..if tall { 30 } else { 1 } {
        axes.line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions {
                label: if tall {
                    format!("Series {i}")
                } else {
                    "W".repeat(80)
                },
                ..LineOptions::default()
            },
        )
        .unwrap();
    }
    f.to_scene(FontLibrary::global()).unwrap_err()
}

#[test]
fn new_positions_reject_legends_that_cannot_fit() {
    for (style, grid, widths, height) in [
        (LegendStyle::Header, false, [362, 352, 384, 338], 461),
        (LegendStyle::Header, true, [158, 327, 158, 134], 461),
        (LegendStyle::Chips, false, [363, 353, 384, 339], 641),
        (LegendStyle::Chips, true, [159, 328, 159, 135], 641),
    ] {
        for (index, (position, tall)) in [
            (LegendPosition::TopRight, false),
            (LegendPosition::Right, false),
            (LegendPosition::Bottom, false),
            (LegendPosition::Best, false),
            (LegendPosition::Right, true),
            (LegendPosition::Best, true),
        ]
        .into_iter()
        .enumerate()
        {
            let err = room_error(position, grid, tall, style);
            let detail = if tall {
                let available = if position == LegendPosition::Right {
                    "the plot has 243 pt"
                } else {
                    "the plot leaves 219 pt for the legend"
                };
                format!("the legend is {height} pt tall; {available}.")
            } else {
                format!(
                    "the legend label \"WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW… is 610 pt wide; the panel has {} pt.",
                    widths[index]
                )
            };
            let prefix = if grid { "row 0, col 1: " } else { "" };
            let message = format!("{prefix}{detail}");
            assert_eq!(err.to_string(), format!("Figure.to_scene(): {message}"));
            assert!(
                matches!(&err, crate::Error::Style { context: "Figure.to_scene()", message: actual, .. } if actual == &message)
            );
            assert_eq!(err.parameter(), None);
            assert_eq!(
                err.hint(),
                Some(if tall {
                    "make the figure taller, or use fewer legend entries."
                } else {
                    "shorten the label or widen the figure."
                })
            );
        }
    }
}

#[test]
fn every_existing_mark_provides_drawn_coverage() {
    use crate::{
        AreaOptions, AxisValue, BandOptions, BarOptions, BoxOptions, Distribution, ErrorBarOptions,
        HistOptions, RuleOptions, Series, SpanOptions, StemOptions, StepOptions,
    };
    let mut numeric = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = numeric
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.legend_at(
        LegendStyle::Header,
        LegendPosition::Best,
        crate::SizeSpec::Auto,
    )
    .unwrap();
    let x = || AxisData::Numbers(vec![0.2, 0.8]);
    let y = &[0.3, 0.7];
    axes.line(
        x(),
        y,
        LineOptions {
            label: "Signal".into(),
            ..LineOptions::default()
        },
    )
    .unwrap();
    axes.scatter(x(), y, ScatterOptions::default()).unwrap();
    axes.area(x(), y, AreaOptions::default()).unwrap();
    axes.stacked_area(
        &[0.2, 0.8],
        vec![Series {
            values: y.to_vec(),
            ..Series::default()
        }],
    )
    .unwrap();
    axes.hist(&[0.2, 0.4, 0.6, 0.8], HistOptions::default())
        .unwrap();
    axes.step(x(), y, StepOptions::default()).unwrap();
    axes.stem(x(), y, StemOptions::default()).unwrap();
    axes.hline(0.5, RuleOptions::default()).unwrap();
    axes.vline(AxisValue::Number(0.5), RuleOptions::default())
        .unwrap();
    axes.hspan(0.2, 0.4, SpanOptions::default()).unwrap();
    axes.vspan(0.2, 0.4, SpanOptions::default()).unwrap();
    axes.band(
        &[0.2, 0.8],
        &[0.1, 0.2],
        &[0.7, 0.8],
        BandOptions::default(),
    )
    .unwrap();
    axes.y_error_bars(x(), &[0.1, 0.2], &[0.7, 0.8], ErrorBarOptions::default())
        .unwrap();
    axes.x_error_bars(
        &[0.2, 0.8],
        &[0.1, 0.2],
        &[0.7, 0.8],
        ErrorBarOptions::default(),
    )
    .unwrap();
    let mut categorical = Figure::new(6.0, 4.0, Theme::midnight()).unwrap();
    let mut axes = categorical
        .add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap();
    axes.legend_at(
        LegendStyle::Header,
        LegendPosition::Best,
        crate::SizeSpec::Auto,
    )
    .unwrap();
    let categories = || vec!["A".into(), "B".into()];
    let series = || {
        vec![Series {
            label: "Signal".into(),
            values: vec![1.0, 2.0],
            ..Series::default()
        }]
    };
    axes.bar(categories(), &[1.0, 2.0], BarOptions::default())
        .unwrap();
    axes.grouped_bar(categories(), series()).unwrap();
    axes.stacked_bar(categories(), series()).unwrap();
    let groups = || vec![vec![0.0, 1.0, 2.0, 3.0, 4.0, 12.0], vec![2.0]];
    axes.box_plot(categories(), groups(), BoxOptions::default())
        .unwrap();
    axes.grouped_box_plot(
        categories(),
        vec![Distribution {
            label: "Boxes".into(),
            groups: groups(),
            ..Default::default()
        }],
    )
    .unwrap();
    for f in [numeric, categorical] {
        let (resolved, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        let p = layout.plot;
        let inside = crate::layout::Frame {
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
        };
        let outside = crate::layout::Frame {
            x: p.right() + 1.0,
            ..inside
        };
        for mark in crate::resolve::resolve_marks(f.single_axes().marks(), f.theme()) {
            let item =
                crate::draw::marks(f.theme(), &resolved, &layout, std::slice::from_ref(&mark))
                    .unwrap();
            let prepared = mark.for_coverage(Some(crate::coverage::Geometry::of(&item)));
            assert!(prepared.coverage(&p, &inside) > 0.0, "{mark:?}");
            assert_eq!(prepared.coverage(&p, &outside), 0.0);
        }
    }
}
