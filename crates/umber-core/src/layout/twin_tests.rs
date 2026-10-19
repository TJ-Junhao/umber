use crate::pipeline::plan_panel_marks;
use crate::resolve::{ResolvedMark, resolve_figure_marks};
use crate::{
    Figure, FontLibrary, GridIndex, LegendPosition, LegendStyle, LineOptions, ScaleKind, SizeSpec,
    Theme,
};

fn figure(theme: Theme) -> Figure {
    let mut f = Figure::new(8.0, 5.0, theme).unwrap();
    let mut left = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    left.title("One panel")
        .xlabel("Shared x")
        .ylabel("Left units");
    left.line(
        crate::AxisData::Numbers(vec![0., 1.]),
        &[1., 2.],
        LineOptions::default(),
    )
    .unwrap();
    left.twin_y(ScaleKind::Linear)
        .unwrap()
        .ylabel_sized("Right units", SizeSpec::pt(18.).unwrap())
        .line(
            crate::AxisData::Numbers(vec![-2., 4.]),
            &[100., 200.],
            LineOptions::default(),
        )
        .unwrap();
    f
}

#[test]
fn twin_uses_one_resolved_x_and_one_plot() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let f = figure(theme);
        let planned = plan_panel_marks(&f, FontLibrary::global());
        let (left, a, _) = &planned[0];
        let (right, b, _) = &planned[1];
        assert_eq!(left.x, right.x);
        assert_ne!(left.y, right.y);
        assert_eq!(a.plot, b.plot);
        assert!(!a.x_tick_labels.is_empty());
        assert!(b.x_tick_labels.is_empty());
        assert!(b.x_grid.is_empty() && b.y_grid.is_empty());
        assert!(b.title.is_none() && b.subtitle.is_none() && b.xlabel.is_none());
        let yl = a.ylabel.as_ref().unwrap();
        let yr = b.ylabel.as_ref().unwrap();
        assert_eq!(yl.y, yr.y);
        assert!((yr.x + yr.width() - b.plot.right()).abs() < 1e-9);
        assert!(
            b.y_tick_labels
                .iter()
                .all(|label| label.x == b.plot.right() + super::super::Y_LABEL_GAP)
        );
        assert!(
            b.y_tick_labels
                .iter()
                .all(|label| label.x + label.width() <= b.frame.right())
        );
        let scale = left.x.continuous().unwrap();
        assert!(scale.domain().0 <= -2. && scale.domain().1 >= 4.);
    }
}

#[test]
fn twin_title_gap_and_room_error_rounding() {
    let f = figure(Theme::pigment());
    let planned = plan_panel_marks(&f, FontLibrary::global());
    let left = &planned[0].1;
    let mut right = planned[1].1.clone();
    let left_title = left.ylabel.as_ref().unwrap();
    let right_title = right.ylabel.as_mut().unwrap();
    right_title.x = left_title.x + left_title.width() + super::super::AXIS_LABEL_GAP - 0.25;
    let width = left_title.width() + super::super::AXIS_LABEL_GAP + right_title.width();
    let room = right_title.x + right_title.width() - left_title.x;
    let (width, room) = super::super::widths(width, room);
    let err = super::check_titles(
        left,
        &right,
        [SizeSpec::Auto, SizeSpec::pt(18.).unwrap()],
        "Figure.to_scene()",
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        format!(
            "Figure.to_scene(): the left y axis label \"Left units\" and right y axis label \"Right units\" are {width} pt wide with a gap; the panel has {room} pt."
        )
    );
    assert_eq!(
        err.hint(),
        Some("make the figure wider or pass a smaller size=.")
    );
    right.ylabel.as_mut().unwrap().x += 0.5;
    super::check_titles(
        left,
        &right,
        [SizeSpec::Auto, SizeSpec::pt(18.).unwrap()],
        "Figure.to_scene()",
    )
    .unwrap();
    right.ylabel = None;
    super::check_titles(
        left,
        &right,
        [SizeSpec::Auto, SizeSpec::pt(18.).unwrap()],
        "Figure.to_scene()",
    )
    .unwrap();
}

#[test]
fn unlabelled_palette_continues_and_labels_share_slots() {
    let mut f = figure(Theme::pigment());
    let check = |f: &Figure, expected: &[usize]| {
        let colors: Vec<_> = resolve_figure_marks(f)
            .into_iter()
            .flatten()
            .map(|mark| {
                let ResolvedMark::Line { color, .. } = mark else {
                    panic!("line expected")
                };
                color
            })
            .collect();
        assert_eq!(
            colors,
            expected
                .iter()
                .map(|&i| f.theme().palette()[i])
                .collect::<Vec<_>>()
        );
    };
    check(&f, &[0, 1]);
    for twin in [false, true] {
        let mut axes = if twin {
            f.twin_axes_at_mut(GridIndex::Auto, GridIndex::Auto)
                .unwrap()
        } else {
            f.axes_at_mut(GridIndex::Auto, GridIndex::Auto).unwrap()
        };
        axes.line(
            crate::AxisData::Numbers(vec![0., 1.]),
            &[2., 3.],
            LineOptions {
                label: "Same".into(),
                ..Default::default()
            },
        )
        .unwrap();
    }
    check(&f, &[1, 0, 2, 0]);
}

#[test]
fn one_row_legend_and_right_reserve_clear_ticks() {
    for position in [
        LegendPosition::TopLeft,
        LegendPosition::TopRight,
        LegendPosition::Right,
        LegendPosition::Bottom,
        LegendPosition::Best,
    ] {
        let mut f = figure(Theme::pigment());
        f.axes_at_mut(GridIndex::Auto, GridIndex::Auto)
            .unwrap()
            .line(
                crate::AxisData::Numbers(vec![0., 1.]),
                &[1., 2.],
                LineOptions {
                    label: "Left series".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        f.twin_axes_at_mut(GridIndex::Auto, GridIndex::Auto)
            .unwrap()
            .line(
                crate::AxisData::Numbers(vec![0., 1.]),
                &[100., 200.],
                LineOptions {
                    label: "Right series".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .legend_at(LegendStyle::Header, position, SizeSpec::Auto)
            .unwrap();
        let planned = plan_panel_marks(&f, FontLibrary::global());
        let a = &planned[0].1;
        let b = &planned[1].1;
        assert_eq!(a.legend.entries().len(), 2);
        assert!(b.legend.is_empty());
        if position == LegendPosition::Right {
            let ticks = b
                .y_tick_labels
                .iter()
                .map(|label| label.x + label.width())
                .fold(0., f64::max);
            assert!(
                a.legend
                    .entries()
                    .iter()
                    .all(|entry| entry.swatch.x >= ticks)
            );
        }
    }
}

fn wrapped_legend(
    theme: Theme,
    position: LegendPosition,
    twin: bool,
    right_ticks: bool,
    height: f64,
) -> Figure {
    let mut f = Figure::new(3.0, height, theme).unwrap();
    let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    ax.title("Series")
        .subtitle("Eight lines")
        .xlabel("Time")
        .ylabel("Left")
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .xticks(&[0.0, 1.0])
        .unwrap()
        .yticks_labeled(&[0.0, 1.0], vec!["Left".into(); 2])
        .unwrap();
    for i in 0..8 {
        ax.line(
            crate::AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions {
                label: format!("S{i}"),
                ..Default::default()
            },
        )
        .unwrap();
    }
    ax.legend_at(LegendStyle::Header, position, SizeSpec::Auto)
        .unwrap();
    if twin {
        let mut right = ax.twin_y(ScaleKind::Linear).unwrap();
        right.ylim(0.0, 1.0).unwrap();
        if right_ticks {
            right
                .ylabel("Right")
                .yticks_labeled(&[0.0, 1.0], vec!["Right".into(); 2])
                .unwrap();
        } else {
            right.yticks(&[]).unwrap();
        }
    }
    f
}

#[test]
fn wrapped_twin_legends_stay_in_their_reserved_bands() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for position in [
            LegendPosition::TopLeft,
            LegendPosition::TopRight,
            LegendPosition::Bottom,
        ] {
            let build = |height| wrapped_legend(theme.clone(), position, true, true, height);
            let planned =
                crate::pipeline::try_plan_panels(&build(3.0), FontLibrary::global()).unwrap();
            let left = &planned[0].1;
            let entries = left.legend.entries();
            assert_eq!(entries.len(), 8);
            assert!(entries.last().unwrap().label.y > entries[0].label.y);
            assert!(planned[1].1.legend.is_empty());
            let top = entries
                .iter()
                .map(|e| e.label.y - f64::from(e.label.layout.ascent()))
                .fold(f64::INFINITY, f64::min);
            let bottom = entries
                .iter()
                .map(|e| e.label.y + f64::from(e.label.layout.descent()))
                .fold(0.0, f64::max);
            if position == LegendPosition::Bottom {
                let xlabel = left.xlabel.as_ref().unwrap();
                assert!(top >= xlabel.y + f64::from(xlabel.layout.descent()));
                assert!(top >= left.plot.bottom());
                assert!(bottom <= 216.0 - super::super::PADDING + 1e-9);
            } else {
                assert!(bottom <= left.header_bottom);
                for (_, layout) in &planned {
                    let ylabel = layout.ylabel.as_ref().unwrap();
                    assert!(
                        left.header_bottom <= ylabel.y - f64::from(ylabel.layout.ascent()) + 1e-9
                    );
                    assert!(bottom < layout.plot.y);
                    for tick in &layout.y_tick_labels {
                        assert!(bottom < tick.y - f64::from(tick.layout.ascent()));
                    }
                }
            }
            // The height error must reserve the same wrapping as successful placement.
            let error = build(0.5)
                .to_scene(FontLibrary::global())
                .unwrap_err()
                .to_string();
            let needed: f64 = error
                .split_once("at least ")
                .unwrap()
                .1
                .split_once(" pt")
                .unwrap()
                .0
                .parse()
                .unwrap();
            build(needed / 72.0)
                .to_scene(FontLibrary::global())
                .unwrap();
        }
    }
}

#[test]
fn one_cell_twin_keeps_matching_wrapped_legend_placement() {
    for position in [
        LegendPosition::TopLeft,
        LegendPosition::TopRight,
        LegendPosition::Bottom,
    ] {
        let single = crate::pipeline::try_plan_panels(
            &wrapped_legend(Theme::pigment(), position, false, false, 3.0),
            FontLibrary::global(),
        )
        .unwrap();
        for right_ticks in [false, true] {
            if right_ticks && position == LegendPosition::TopRight {
                continue; // Only top-right rows wrap to the narrower plot beside right ticks.
            }
            let twin = crate::pipeline::try_plan_panels(
                &wrapped_legend(Theme::pigment(), position, true, right_ticks, 3.0),
                FontLibrary::global(),
            )
            .unwrap();
            for (a, b) in single[0]
                .1
                .legend
                .entries()
                .iter()
                .zip(twin[0].1.legend.entries())
            {
                assert_eq!((a.label.x, a.label.y), (b.label.x, b.label.y));
            }
            assert_eq!(single[0].1.header_bottom, twin[0].1.header_bottom);
        }
    }
}

#[test]
fn category_union_keeps_the_same_order_on_both_axes() {
    let mut f = Figure::new(8., 5., Theme::pigment()).unwrap();
    let mut left = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
    left.line(
        crate::AxisData::Categories(vec!["A".into(), "B".into()]),
        &[1., 2.],
        LineOptions::default(),
    )
    .unwrap();
    left.twin_y(ScaleKind::Linear)
        .unwrap()
        .line(
            crate::AxisData::Categories(vec!["B".into(), "C".into()]),
            &[100., 200.],
            LineOptions::default(),
        )
        .unwrap();
    let planned = plan_panel_marks(&f, FontLibrary::global());
    assert_eq!(planned[0].0.x, planned[1].0.x);
    assert_eq!(
        planned[0]
            .0
            .x
            .ticks
            .iter()
            .map(|t| t.label.as_str())
            .collect::<Vec<_>>(),
        ["A", "B", "C"]
    );
}

#[test]
fn shared_direct_column_clears_ticks_and_precedes_both_colorbars() {
    use crate::{AxisData, ColorBy, ColorbarOptions, ColorbarPosition, PointColor, ScatterOptions};

    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for bars in [[true, true], [true, false], [false, true], [false, false]] {
            let mut f = Figure::new(12., 6., theme.clone()).unwrap();
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .twin_y(ScaleKind::Linear)
                .unwrap();
            for (i, shown) in bars.into_iter().enumerate() {
                let mut axes = if i == 0 {
                    f.axes_at_mut(GridIndex::Auto, GridIndex::Auto).unwrap()
                } else {
                    f.twin_axes_at_mut(GridIndex::Auto, GridIndex::Auto)
                        .unwrap()
                };
                let (label, ys) = if i == 0 {
                    ("Left", [1., 2.])
                } else {
                    ("Right line with a wider label", [200., 100.])
                };
                axes.xlim(0., 1.).unwrap();
                axes.line(
                    AxisData::Numbers(vec![0., 1.]),
                    &ys,
                    LineOptions {
                        label: label.into(),
                        ..Default::default()
                    },
                )
                .unwrap();
                axes.legend(LegendStyle::Direct, SizeSpec::Auto).unwrap();
                if shown {
                    axes.scatter(
                        AxisData::Numbers(vec![0.2, 0.8]),
                        &ys,
                        ScatterOptions {
                            color: PointColor::By(ColorBy::new(vec![0., 10.])),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                    axes.colorbar(ColorbarOptions {
                        label: label.into(),
                        position: Some(ColorbarPosition::Right),
                        ..Default::default()
                    })
                    .unwrap();
                }
            }
            let planned = plan_panel_marks(&f, FontLibrary::global());
            let (left, right) = (&planned[0].1, &planned[1].1);
            let a = left.direct[0].label.as_ref().unwrap();
            let b = right.direct[0].label.as_ref().unwrap();
            assert_eq!(a.x, b.x);
            assert!(a.y < b.y);
            let ticks_right = right
                .y_tick_labels
                .iter()
                .map(|label| label.x + label.width())
                .fold(0., f64::max);
            assert!(a.x >= ticks_right + super::super::Y_LABEL_GAP);
            let bars: Vec<_> = [left, right]
                .into_iter()
                .filter_map(|layout| layout.colorbar.as_ref())
                .collect();
            for label in [a, b] {
                assert!(label.x + label.width() <= left.frame.right());
                assert!(bars.iter().all(|bar| label.x + label.width() < bar.bar.x));
            }
            if bars.len() == 2 {
                assert!(bars[0].bar.right() < bars[1].bar.x);
                assert!(
                    bars[0]
                        .ticks
                        .iter()
                        .all(|(_, label)| label.x + label.width() < bars[1].bar.x)
                );
            }
        }
    }
}

#[test]
fn category_direct_label_clears_right_tick_column() {
    let mut f = Figure::new(8., 5., Theme::graphite()).unwrap();
    let categories: Vec<String> = (1..=12).map(|i| i.to_string()).collect();
    let mut left = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
    left.line(
        crate::AxisData::Categories(categories.clone()),
        &[50.; 12],
        LineOptions::default(),
    )
    .unwrap();
    left.twin_y(ScaleKind::Linear)
        .unwrap()
        .line(
            crate::AxisData::Categories(categories),
            &[4., 5., 9., 13., 18., 22., 25., 24., 20., 14., 9., 5.],
            LineOptions {
                label: "Temperature".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let planned = plan_panel_marks(&f, FontLibrary::global());
    let layout = &planned[1].1;
    let ticks_right = layout
        .y_tick_labels
        .iter()
        .map(|label| label.x + label.width())
        .fold(0., f64::max);
    let label = layout.direct[0].label.as_ref().unwrap();
    assert!(
        label.x >= ticks_right + super::super::Y_LABEL_GAP,
        "direct label starts at {}, but the tick column ends at {ticks_right}",
        label.x
    );
    assert!(label.x + label.width() <= layout.frame.right());
    assert!(layout.direct[0].dot.unwrap().x < layout.plot.right());
}

#[test]
fn overlapping_y_labels_choose_hint_from_both_sizes() {
    let f = figure(Theme::pigment());
    let planned = plan_panel_marks(&f, FontLibrary::global());
    let left = &planned[0].1;
    let mut right = planned[1].1.clone();
    right.ylabel.as_mut().unwrap().x = left.ylabel.as_ref().unwrap().x;
    let points = SizeSpec::pt(18.0).unwrap();
    let times = SizeSpec::times(1.4).unwrap();
    for sizes in [
        [SizeSpec::Auto, times],
        [times, SizeSpec::Auto],
        [points, times],
        [times, points],
        [points, points],
    ] {
        let error = super::check_titles(left, &right, sizes, "Figure.to_scene()").unwrap_err();
        let hint = if sizes.iter().any(|size| matches!(size, SizeSpec::Points(_))) {
            "make the figure wider or pass a smaller size=."
        } else {
            "make the figure wider or lower the font scale with Theme.with_font_scale()."
        };
        assert_eq!(error.hint(), Some(hint));
    }
}
