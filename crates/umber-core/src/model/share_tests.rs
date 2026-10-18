//! Shared axes: group settings, domains, ticks and outer labels.

use super::*;
use crate::pipeline::try_plan_panels;
use crate::resolve::AxisScale;
use crate::{FontLibrary, ScaleKind, Theme};

fn at(row: i64, col: i64) -> Placement {
    Placement {
        row: GridIndex::At(row),
        col: GridIndex::At(col),
        ..Placement::default()
    }
}

fn figure(rows: i64, cols: i64, x: Share, y: Share) -> Figure {
    Figure::with_grid(
        6.0 * cols as f64,
        4.0 * rows as f64,
        Theme::graphite(),
        Grid::new(rows, cols)
            .unwrap()
            .with_share_x(x)
            .with_share_y(y),
    )
    .unwrap()
}

fn add(f: &mut Figure, row: i64, col: i64, high: f64) {
    f.add_axes_at(at(row, col), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Numbers(vec![0.0, high]),
            &[1.0, high],
            LineOptions::default(),
        )
        .unwrap();
}

#[test]
fn shared_x_has_one_domain_and_ticks() {
    let mut f = figure(2, 1, Share::Col, Share::None);
    add(&mut f, 0, 0, 3.0);
    add(&mut f, 1, 0, 10.0);
    let panels = try_plan_panels(&f, &FontLibrary::bundled()).unwrap();
    assert_eq!(panels[0].0.x, panels[1].0.x);
    assert_eq!(panels[0].0.x.continuous().unwrap().domain(), (0.0, 10.0));
}

#[test]
fn shared_error_bar_domains_and_ticks_match_their_endpoints() {
    let lib = FontLibrary::bundled();
    let modes = [Share::None, Share::Row, Share::Col, Share::All];
    for x in modes {
        for y in modes {
            for kind in [
                ScaleKind::Linear,
                ScaleKind::Log {
                    base: crate::LogBase::TEN,
                },
            ] {
                let mut bars = figure(2, 2, x, y);
                let mut endpoints = figure(2, 2, x, y);
                for (row, col) in [(1, 1), (0, 1), (1, 0), (0, 0)] {
                    let k = (1 + row * 2 + col) as f64;
                    let positions = [k, k * 2.0];
                    let low = [k * 0.1, k * 0.5];
                    let high = [k * 100.0, k * 200.0];
                    let mut a = bars.add_axes_at(at(row, col), kind, kind).unwrap();
                    let mut b = endpoints.add_axes_at(at(row, col), kind, kind).unwrap();
                    let positions_twice = [positions, positions].concat();
                    let bounds = [low, high].concat();
                    if row == col {
                        a.y_error_bars(
                            AxisData::Numbers(positions.to_vec()),
                            &low,
                            &high,
                            ErrorBarOptions::default(),
                        )
                        .unwrap();
                        b.scatter(
                            AxisData::Numbers(positions_twice),
                            &bounds,
                            ScatterOptions::default(),
                        )
                        .unwrap();
                    } else {
                        a.x_error_bars(&positions, &low, &high, ErrorBarOptions::default())
                            .unwrap();
                        b.scatter(
                            AxisData::Numbers(bounds),
                            &positions_twice,
                            ScatterOptions::default(),
                        )
                        .unwrap();
                    }
                    for axes in [&mut a, &mut b] {
                        axes.line(
                            AxisData::Numbers(vec![1.0, 2.0]),
                            &[1.0, 2.0],
                            LineOptions::default(),
                        )
                        .unwrap();
                    }
                }
                let actual = try_plan_panels(&bars, &lib).unwrap();
                let expected = try_plan_panels(&endpoints, &lib).unwrap();
                let groups = ShareGroups::new(&bars);
                for (i, ((actual, _), (expected, _))) in actual.iter().zip(&expected).enumerate() {
                    assert_eq!(actual, expected, "{x:?} {y:?} {kind:?} panel {i}");
                }
                for (i, (resolved, _)) in actual.iter().enumerate() {
                    assert_eq!(resolved.x, actual[groups.x[i]].0.x);
                    assert_eq!(resolved.y, actual[groups.y[i]].0.y);
                }
            }
        }
    }
}

#[test]
fn shared_error_bar_categories_follow_panel_order_and_allow_references() {
    let mut f = figure(1, 2, Share::Row, Share::Row);
    for (col, names, low, high) in [
        (1, ["a", "b"], [10.0, 20.0], [30.0, 40.0]),
        (0, ["b", "c"], [-10.0, -5.0], [1.0, 2.0]),
    ] {
        f.add_axes_at(at(0, col), ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .y_error_bars(
                AxisData::Categories(names.map(str::to_owned).to_vec()),
                &low,
                &high,
                ErrorBarOptions::default(),
            )
            .unwrap();
    }
    f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
        .unwrap()
        .vline(AxisValue::Category("a".into()), RuleOptions::default())
        .unwrap();
    let lib = FontLibrary::bundled();
    let panels = try_plan_panels(&f, &lib).unwrap();
    assert_eq!(panels[0].0, panels[1].0);
    let AxisScale::Category(scale) = &panels[0].0.x.scale else {
        panic!("category scale")
    };
    assert_eq!(scale.categories(), ["b", "c", "a"]);
    let (low, high) = panels[0].0.y.continuous_scale().domain();
    assert!(low <= -10.0 && high >= 40.0);
    assert!(f.to_scene(&lib).is_ok());
}

#[test]
fn shared_x_labels_on_lowest_occupied_panel() {
    let mut f = figure(3, 2, Share::Col, Share::None);
    for (r, c) in [(0, 0), (0, 1), (1, 0), (1, 1), (2, 0)] {
        add(&mut f, r, c, 10.0);
    }
    let panels = try_plan_panels(&f, &FontLibrary::bundled()).unwrap();
    for (i, (_, layout)) in panels.iter().enumerate() {
        assert_eq!(!layout.x_tick_labels.is_empty(), i == 3 || i == 4);
        assert!(!layout.x_grid.is_empty());
    }
}

#[test]
fn conflicting_shared_limits_leave_group_unchanged() {
    let mut f = figure(2, 1, Share::Col, Share::None);
    add(&mut f, 0, 0, 3.0);
    f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
        .unwrap()
        .xlim(0.0, 10.0)
        .unwrap();
    add(&mut f, 1, 0, 10.0);
    assert_eq!(
        f.axes_at(GridIndex::At(1), GridIndex::At(0))
            .unwrap()
            .x_limits(),
        Some((0.0, 10.0))
    );
    let before = f.clone();
    let lib = FontLibrary::bundled();
    let svg =
        crate::render::render_svg(&f.to_scene(&lib).unwrap(), crate::render::SvgText::Editable);
    let err = f
        .axes_at_mut(GridIndex::At(1), GridIndex::At(0))
        .unwrap()
        .xlim(0.0, 5.0)
        .unwrap_err();
    assert!(matches!(err, crate::Error::Usage { .. }));
    assert_eq!(
        err.to_string(),
        "Axes.xlim(): x limits are shared with row 0, col 0, which set (0, 10)."
    );
    assert_eq!(
        err.hint(),
        Some("set it once for the group, or share fewer axes.")
    );
    assert_eq!(f, before);
    assert_eq!(
        svg,
        crate::render::render_svg(&f.to_scene(&lib).unwrap(), crate::render::SvgText::Editable)
    );
    f.axes_at_mut(GridIndex::At(1), GridIndex::At(0))
        .unwrap()
        .xlim(0.0, 10.0)
        .unwrap();
}

#[test]
fn shared_groups_reject_mixed_scales_and_cross_group_spans() {
    let mut f = figure(2, 2, Share::Row, Share::None);
    add(&mut f, 0, 0, 3.0);
    let before = f.clone();
    let err = f
        .add_axes_at(at(0, 1), ScaleKind::Category, ScaleKind::Linear)
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Figure.add_axes(): x is Scale.category(); the axes at row 0, col 0 shares its x axis and has Scale.linear()."
    );
    assert_eq!(err.parameter(), Some("x"));
    assert_eq!(
        err.hint(),
        Some("axes that share an axis need the same scale.")
    );
    assert_eq!(f, before);
    let err = f
        .add_axes_at(
            Placement {
                row_span: 2,
                ..at(0, 1)
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Figure.add_axes(): the axes spans rows 0 to 1, which share x in different groups."
    );
    assert_eq!(err.parameter(), Some("row_span"));
    assert_eq!(
        err.hint(),
        Some("span within one shared group, or share by ALL or NONE.")
    );
    assert_eq!(f, before);
}

#[test]
fn shared_categories_follow_row_major_first_appearance() {
    let mut f = figure(1, 2, Share::Row, Share::None);
    for (col, names) in [(1, vec!["a", "b"]), (0, vec!["b", "c"])] {
        f.add_axes_at(at(0, col), ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .bar(
                names.into_iter().map(str::to_owned).collect(),
                &[1.0, 2.0],
                BarOptions::default(),
            )
            .unwrap();
    }
    let panels = try_plan_panels(&f, &FontLibrary::bundled()).unwrap();
    for (r, _) in panels {
        let AxisScale::Category(scale) = r.x.scale else {
            panic!("category scale")
        };
        assert_eq!(scale.categories(), ["b", "c", "a"]);
    }
}

fn set(a: &mut AxesMut<'_>, operation: usize, other: bool) -> crate::Result<()> {
    let high = if other { 5.0 } else { 10.0 };
    match operation {
        0 => a.xlim(0.0, high),
        1 => a.ylim(0.0, high),
        2 => a.xticks(&[0.0, high]),
        3 => a.yticks(&[0.0, high]),
        4 => a.xticks_labeled(
            &[0.0, 10.0],
            vec!["start".into(), if other { "other" } else { "end" }.into()],
        ),
        5 => a.yticks_labeled(
            &[0.0, 10.0],
            vec!["start".into(), if other { "other" } else { "end" }.into()],
        ),
        6 => a.xtick_format(crate::TickFormat::fixed(if other { 2 } else { 1 })?),
        _ => a.ytick_format(crate::TickFormat::fixed(if other { 2 } else { 1 })?),
    }
    .map(|_| ())
}

#[test]
fn group_settings_share_their_allocation_after_setters_inheritance_and_clones() {
    let modes = [Share::None, Share::Row, Share::Col, Share::All];
    for x in modes {
        for y in modes {
            for operation in 0..8 {
                let mut f = figure(2, 2, x, y);
                add(&mut f, 1, 1, 10.0);
                f.assert_group_settings_shared();
                set(
                    &mut f.axes_at_mut(GridIndex::At(1), GridIndex::At(1)).unwrap(),
                    operation,
                    false,
                )
                .unwrap();
                f.assert_group_settings_shared();
                for (row, col) in [(0, 1), (1, 0), (0, 0)] {
                    add(&mut f, row, col, 3.0);
                    f.assert_group_settings_shared();
                }
                let before = f.clone();
                let mut copy = f.clone();
                before.assert_group_settings_shared();
                copy.assert_group_settings_shared();
                set(
                    &mut copy
                        .axes_at_mut(GridIndex::At(1), GridIndex::At(1))
                        .unwrap(),
                    operation,
                    true,
                )
                .unwrap();
                copy.assert_group_settings_shared();
                f.assert_group_settings_shared();
                assert_eq!(f, before);
                assert_ne!(copy, f);
                for (row, col) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    set(
                        &mut f
                            .axes_at_mut(GridIndex::At(row), GridIndex::At(col))
                            .unwrap(),
                        operation,
                        false,
                    )
                    .unwrap();
                    f.assert_group_settings_shared();
                }
                copy.assert_group_settings_shared();
                before.assert_group_settings_shared();
            }
        }
    }
}

#[test]
fn every_shared_setter_checks_other_members_and_preserves_copies() {
    for operation in 0..8 {
        let mut f = figure(2, 2, Share::All, Share::All);
        add(&mut f, 1, 1, 10.0);
        set(
            &mut f.axes_at_mut(GridIndex::At(1), GridIndex::At(1)).unwrap(),
            operation,
            false,
        )
        .unwrap();
        // Inserting an earlier panel must not change the identity of the member that set it.
        add(&mut f, 0, 0, 3.0);
        let axes: Vec<_> = f.panels().collect();
        assert!(std::sync::Arc::ptr_eq(
            &axes[0].x_settings,
            &axes[1].x_settings
        ));
        assert!(std::sync::Arc::ptr_eq(
            &axes[0].y_settings,
            &axes[1].y_settings
        ));
        let snapshot = axes[0].clone();
        let before = f.clone();
        let mut copy = f.clone();
        set(
            &mut copy
                .axes_at_mut(GridIndex::At(1), GridIndex::At(1))
                .unwrap(),
            operation,
            true,
        )
        .unwrap();
        assert_eq!(f, before);
        assert_eq!(
            snapshot,
            *f.axes_at(GridIndex::At(0), GridIndex::At(0)).unwrap()
        );
        assert_ne!(copy, f);
        let err = set(
            &mut f.axes_at_mut(GridIndex::At(0), GridIndex::At(0)).unwrap(),
            operation,
            true,
        )
        .unwrap_err();
        assert!(matches!(err, crate::Error::Usage { .. }));
        assert!(err.to_string().contains("shared with row 1, col 1"));
        assert_eq!(
            err.hint(),
            Some("set it once for the group, or share fewer axes.")
        );
        assert_eq!(f, before);
        set(
            &mut f.axes_at_mut(GridIndex::At(0), GridIndex::At(0)).unwrap(),
            operation,
            false,
        )
        .unwrap();
        // Both members have now set the value, so neither may silently change the other's.
        let before = f.clone();
        assert!(
            set(
                &mut f.axes_at_mut(GridIndex::At(1), GridIndex::At(1)).unwrap(),
                operation,
                true
            )
            .is_err()
        );
        assert_eq!(f, before);
    }
}

#[test]
fn labeled_ticks_and_formats_validate_against_the_group() {
    let mut f = figure(2, 1, Share::All, Share::All);
    add(&mut f, 0, 0, 10.0);
    add(&mut f, 1, 0, 10.0);
    f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
        .unwrap()
        .xticks_labeled(&[0.0, 10.0], vec!["low".into(), "high".into()])
        .unwrap()
        .ytick_format(crate::TickFormat::fixed(1).unwrap())
        .unwrap();
    let before = f.clone();
    let mut a = f.axes_at_mut(GridIndex::At(1), GridIndex::At(0)).unwrap();
    assert!(
        a.xtick_format(crate::TickFormat::fixed(1).unwrap())
            .is_err()
    );
    assert!(a.yticks_labeled(&[0.0], vec!["zero".into()]).is_err());
    assert!(a.xticks(&[0.0, 10.0]).is_err());
    assert_eq!(f, before);
}

#[test]
fn explicit_auto_format_is_a_group_setting() {
    let mut f = figure(2, 1, Share::All, Share::All);
    add(&mut f, 0, 0, 10.0);
    add(&mut f, 1, 0, 10.0);
    f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
        .unwrap()
        .xtick_format(crate::TickFormat::AUTO)
        .unwrap();
    let before = f.clone();
    let err = f
        .axes_at_mut(GridIndex::At(1), GridIndex::At(0))
        .unwrap()
        .xtick_format(crate::TickFormat::fixed(1).unwrap())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Axes.xtick_format(): x tick format is shared with row 0, col 0, which set another format."
    );
    assert_eq!(f, before);
}

#[test]
fn groups_accept_only_matching_log_bases_and_spans() {
    let log = |base| ScaleKind::Log {
        base: crate::LogBase::new(base).unwrap(),
    };
    for x_axis in [true, false] {
        let mut f = figure(2, 2, Share::All, Share::All);
        f.add_axes_at(at(0, 0), log(10.0), log(10.0)).unwrap();
        let before = f.clone();
        let (x, y) = if x_axis {
            (log(2.0), log(10.0))
        } else {
            (log(10.0), log(2.0))
        };
        let err = f.add_axes_at(at(1, 1), x, y).unwrap_err();
        assert_eq!(err.parameter(), Some(if x_axis { "x" } else { "y" }));
        assert!(err.to_string().contains("Scale.log(2)"));
        assert_eq!(f, before);
    }
    for (x, y, rows, parameter, dimension, axis) in [
        (Share::Row, Share::None, true, "row_span", "rows", "x"),
        (Share::Col, Share::None, false, "col_span", "columns", "x"),
        (Share::None, Share::Row, true, "row_span", "rows", "y"),
        (Share::None, Share::Col, false, "col_span", "columns", "y"),
    ] {
        let mut f = figure(2, 2, x, y);
        let before = f.clone();
        let p = if rows {
            Placement {
                row_span: 2,
                ..at(0, 0)
            }
        } else {
            Placement {
                col_span: 2,
                ..at(0, 0)
            }
        };
        let err = f
            .add_axes_at(p, ScaleKind::Linear, ScaleKind::Linear)
            .unwrap_err();
        assert_eq!(err.parameter(), Some(parameter));
        assert_eq!(
            err.to_string(),
            format!(
                "Figure.add_axes(): the axes spans {dimension} 0 to 1, which share {axis} in different groups."
            )
        );
        assert_eq!(f, before);
    }
}

#[test]
fn spanning_panels_join_groups_but_only_hide_labels_with_matching_spans() {
    let mut f = figure(3, 3, Share::All, Share::All);
    f.add_axes_at(
        Placement {
            col_span: 2,
            ..at(0, 0)
        },
        ScaleKind::Linear,
        ScaleKind::Linear,
    )
    .unwrap()
    .xlabel("Time")
    .ylabel("Level")
    .line(
        AxisData::Numbers(vec![0.0, 20.0]),
        &[0.0, 30.0],
        LineOptions::default(),
    )
    .unwrap();
    add(&mut f, 0, 2, 3.0);
    f.add_axes_at(
        Placement {
            row_span: 2,
            ..at(1, 0)
        },
        ScaleKind::Linear,
        ScaleKind::Linear,
    )
    .unwrap();
    add(&mut f, 1, 1, 5.0);
    add(&mut f, 2, 2, 7.0);
    let lib = FontLibrary::bundled();
    let panels = try_plan_panels(&f, &lib).unwrap();
    let visibility: Vec<_> = panels
        .iter()
        .map(|(_, l)| [!l.x_tick_labels.is_empty(), !l.y_tick_labels.is_empty()])
        .collect();
    assert_eq!(
        visibility,
        [
            [true, true],
            [false, false],
            [true, true],
            [true, true],
            [true, true]
        ]
    );
    assert!(panels[0].1.xlabel.is_some() && panels[0].1.ylabel.is_some());
    for (r, l) in &panels {
        assert_eq!(r, &panels[0].0);
        assert!(!l.x_grid.is_empty());
    }
}

#[test]
fn equal_spans_hide_labels_only_at_matching_starts_across_empty_cells() {
    for span in [1, 2] {
        for (dimension, places) in [
            (0, [(0, 0, 1, span), (2, 0, 1, span), (3, 1, 1, span)]),
            (1, [(0, 3, span, 1), (0, 1, span, 1), (1, 0, span, 1)]),
        ] {
            let mut f = figure(4, 4, Share::All, Share::All);
            for (row, col, row_span, col_span) in places.into_iter().rev() {
                f.add_axes_at(
                    Placement {
                        row_span,
                        col_span,
                        ..at(row, col)
                    },
                    ScaleKind::Linear,
                    ScaleKind::Linear,
                )
                .unwrap();
            }
            for ((row, col, _, _), visible) in places.into_iter().zip([false, true, true]) {
                let axes = f.axes_at(GridIndex::At(row), GridIndex::At(col)).unwrap();
                assert_eq!(
                    share::label_visibility(&f, axes.cells())[dimension],
                    visible
                );
            }
        }
    }
}

#[test]
fn category_reference_can_use_another_group_members_category() {
    let mut f = figure(1, 2, Share::Row, Share::None);
    f.add_axes_at(at(0, 0), ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .vline(AxisValue::Category("other".into()), RuleOptions::default())
        .unwrap();
    f.add_axes_at(at(0, 1), ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .bar(vec!["other".into()], &[2.0], BarOptions::default())
        .unwrap();
    assert!(f.to_scene(&FontLibrary::bundled()).is_ok());
}

#[test]
fn shared_x_ticks_fit_the_labeled_narrow_panels_above_a_wide_panel() {
    let mut f = figure(2, 2, Share::All, Share::None);
    for place in [
        at(0, 0),
        at(0, 1),
        Placement {
            col_span: 2,
            ..at(1, 0)
        },
    ] {
        f.add_axes_at(place, ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 10000.0]),
                &[0.0, 10.0],
                LineOptions::default(),
            )
            .unwrap()
            .yticks(&[0.0, 10.0])
            .unwrap();
    }
    f.axes_at_mut(GridIndex::At(1), GridIndex::At(0))
        .unwrap()
        .xtick_format(
            crate::TickFormat::fixed(0)
                .unwrap()
                .affix("elapsed time ", " s")
                .unwrap(),
        )
        .unwrap();
    let lib = FontLibrary::bundled();
    let fonts = crate::layout::figure_fonts(&lib, f.theme()).unwrap();
    let panels = try_plan_panels(&f, &lib).unwrap();
    for (_, layout) in &panels {
        assert!(!layout.x_tick_labels.is_empty());
    }
    let axes = f.axes_at(GridIndex::At(1), GridIndex::At(0)).unwrap();
    let marks = crate::resolve::resolve_marks(axes.marks(), f.theme());
    let limits = crate::resolve::Limits::of(axes);
    let ticks_for = |plot| {
        let (x, _) = crate::layout::tick_rooms(
            plot,
            f.theme(),
            &fonts,
            crate::layout::TickSpacing::LabelsApart,
        );
        let (axis, _) = crate::resolve::resolve_axis(
            marks.iter(),
            crate::resolve::ValueAxis::X,
            crate::resolve::AxisRole::of_group(
                crate::resolve::ValueAxis::X,
                std::iter::once((limits.orientation(), marks.as_slice())),
            ),
            limits.x,
            &x,
            None,
        );
        axis
    };
    let wide = ticks_for(panels[2].1.plot.size());
    let narrow = ticks_for(panels[0].1.plot.size());
    assert!(wide.ticks.len() > narrow.ticks.len());
    for (resolved, _) in &panels {
        assert_eq!(resolved.x, narrow);
    }
}

#[test]
fn shared_automatic_ticks_fit_the_final_shortest_plot() {
    let mut f = figure(2, 3, Share::All, Share::All);
    f.add_axes_at(
        Placement {
            col_span: 2,
            ..at(0, 0)
        },
        ScaleKind::Linear,
        ScaleKind::Linear,
    )
    .unwrap()
    .xtick_format(
        crate::TickFormat::fixed(0)
            .unwrap()
            .affix("elapsed time ", " s")
            .unwrap(),
    )
    .unwrap();
    for (r, c) in [(0, 2), (1, 0), (1, 1), (1, 2)] {
        add(&mut f, r, c, 10000.0);
    }
    let lib = FontLibrary::bundled();
    let fonts = crate::layout::figure_fonts(&lib, f.theme()).unwrap();
    let panels = try_plan_panels(&f, &lib).unwrap();
    for (r, l) in &panels {
        assert_eq!(r, &panels[0].0);
        let (x, y) = crate::layout::tick_rooms(
            l.plot.size(),
            f.theme(),
            &fonts,
            crate::layout::TickSpacing::LabelsApart,
        );
        assert!(x.slack(&r.x.ticks) >= -1e-9);
        assert!(y.slack(&r.y.ticks) >= -1e-9);
    }
}

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn shared_group_scales_and_ticks_are_bit_identical(
        rows in 1_i64..5, cols in 1_i64..5,
        sx in 0_usize..4, sy in 0_usize..4,
        log_x in any::<bool>(), log_y in any::<bool>(),
        seed in any::<u64>(), theme in 0_usize..3,
    ) {
        let modes = [Share::None, Share::Row, Share::Col, Share::All];
        let themes = [Theme::pigment(), Theme::graphite(), Theme::midnight()];
        let grid = Grid::new(rows, cols).unwrap().with_share_x(modes[sx]).with_share_y(modes[sy]);
        let mut f = Figure::with_grid(4.0 * cols as f64, 3.0 * rows as f64, themes[theme].clone(), grid).unwrap();
        let kind = |log| if log { ScaleKind::Log { base: crate::LogBase::TEN } } else { ScaleKind::Linear };
        for i in (0..rows * cols).rev() {
            if (seed >> i) & 1 == 0 { continue; }
            let v = (seed.rotate_left(i as u32) % 1000 + 2) as f64;
            f.add_axes_at(at(i / cols, i % cols), kind(log_x), kind(log_y)).unwrap()
                .line(AxisData::Numbers(vec![1.0, v]), &[1.0, v / 2.0], LineOptions::default()).unwrap();
        }
        let groups = ShareGroups::new(&f);
        let all = try_plan_panels(&f, &FontLibrary::bundled()).unwrap();
        let bits = |scale: &crate::ContinuousScale| { let (lo, hi) = scale.domain(); (lo.to_bits(), hi.to_bits()) };
        let ticks = |t: &[crate::resolve::AxisTick]| t.iter().map(|t| (t.pos.get().to_bits(), t.label.clone())).collect::<Vec<_>>();
        for (i, (r, _)) in all.iter().enumerate() {
            let x = &all[groups.x[i]].0.x;
            let y = &all[groups.y[i]].0.y;
            prop_assert_eq!(bits(r.x.continuous().unwrap()), bits(x.continuous().unwrap()));
            prop_assert_eq!(bits(r.y.continuous_scale()), bits(y.continuous_scale()));
            prop_assert_eq!(ticks(&r.x.ticks), ticks(&x.ticks));
            prop_assert_eq!(ticks(&r.y.ticks), ticks(&y.ticks));
        }
    }
}

/// The x domain of every panel of a 2 × 1 grid sharing x, in panel order, with a line with
/// explicit x ticks in row `line_row` and an empty axes with a category y axis in the other.
fn x_domains_beside_an_empty_category_y(line_row: i64) -> Vec<(f64, f64)> {
    let mut f = figure(2, 1, Share::All, Share::None);
    f.add_axes_at(at(1 - line_row, 0), ScaleKind::Linear, ScaleKind::Category)
        .unwrap();
    f.add_axes_at(at(line_row, 0), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Numbers(vec![2.0, 5.0, 10.0]),
            &[3.0, 6.0, 9.0],
            LineOptions::default(),
        )
        .unwrap()
        .xticks(&[2.0, 5.0, 10.0])
        .unwrap();
    try_plan_panels(&f, &FontLibrary::bundled())
        .unwrap()
        .iter()
        .map(|(resolved, _)| resolved.x.continuous_scale().domain())
        .collect()
}

#[test]
fn an_empty_member_does_not_change_how_a_shared_axis_fits_the_data() {
    // The line keeps the exact extent of its x values, whichever row the empty axes with a
    // category y axis, whose value axis is x, takes.
    for line_row in [0, 1] {
        assert_eq!(
            x_domains_beside_an_empty_category_y(line_row),
            [(2.0, 10.0), (2.0, 10.0)],
            "line in row {line_row}"
        );
    }
    // Members without data share the role of the others: all of them decide it.
    use crate::resolve::{AxisRole, Orientation, ValueAxis};
    let none: &[crate::resolve::ResolvedMark<'_>] = &[];
    let vertical = (Orientation::Vertical, none);
    let horizontal = (Orientation::Horizontal, none);
    for members in [[vertical, horizontal], [horizontal, vertical]] {
        assert_eq!(
            AxisRole::of_group(ValueAxis::X, members.into_iter()),
            AxisRole::Positions
        );
    }
    assert_eq!(
        AxisRole::of_group(ValueAxis::X, [horizontal, horizontal].into_iter()),
        AxisRole::Value
    );
}
