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
