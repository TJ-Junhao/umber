//! Value axes of category charts keep the same limits, ticks and baseline when transposed.

use super::{Limits, Resolved, ValueAxis, resolve, resolve_axes_marks};
use crate::layout::{PlotSize, TickRoom, TickSpacing, tick_rooms};
use crate::{
    AxisData, BarOptions, BoxOptions, Distribution, ErrorBarOptions, Figure, LogBase, ScaleKind,
    ScatterOptions, Series, Theme, ViolinOptions,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
enum Mark {
    Box,
    GroupedBox,
    Violin,
    GroupedViolin,
    Bar,
    StackedBar,
    GroupedBar,
    ErrorBars,
    Dots,
}

const MARKS: [Mark; 9] = [
    Mark::Box,
    Mark::GroupedBox,
    Mark::Violin,
    Mark::GroupedViolin,
    Mark::Bar,
    Mark::StackedBar,
    Mark::GroupedBar,
    Mark::ErrorBars,
    Mark::Dots,
];

fn figure(
    mark: Mark,
    horizontal: bool,
    scale: ScaleKind,
    groups: &[Vec<f64>],
    limits: Option<(f64, f64)>,
) -> Figure {
    let mut f = Figure::new(5.0, 5.0, Theme::pigment()).unwrap();
    let (x, y) = if horizontal {
        (scale, ScaleKind::Category)
    } else {
        (ScaleKind::Category, scale)
    };
    let mut a = f.add_axes(x, y).unwrap();
    if let Some((lo, hi)) = limits {
        if horizontal {
            a.xlim(lo, hi).unwrap();
        } else {
            a.ylim(lo, hi).unwrap();
        }
    }
    let names = || ["A", "B", "C"].map(str::to_owned).to_vec();
    let low: Vec<_> = groups.iter().map(|g| g[0]).collect();
    let high: Vec<_> = groups.iter().map(|g| *g.last().unwrap()).collect();
    let distributions = || {
        [1.0, 1.1]
            .map(|factor| Distribution {
                groups: groups
                    .iter()
                    .map(|g| g.iter().map(|v| v * factor).collect())
                    .collect(),
                ..Distribution::default()
            })
            .to_vec()
    };
    let series = || {
        [&low, &high]
            .map(|values| Series {
                values: values.clone(),
                ..Series::default()
            })
            .to_vec()
    };
    match mark {
        Mark::Box => a.box_plot(names(), groups.to_vec(), BoxOptions::default()),
        Mark::GroupedBox => a.grouped_box_plot(names(), distributions()),
        Mark::Violin => a.violin(names(), groups.to_vec(), ViolinOptions::default()),
        Mark::GroupedViolin => a.grouped_violin(names(), distributions()),
        Mark::Bar => a.bar(names(), &high, BarOptions::default()),
        Mark::StackedBar => a.stacked_bar(names(), series()),
        Mark::GroupedBar => a.grouped_bar(names(), series()),
        Mark::ErrorBars if horizontal => a.x_error_bars_shared(
            AxisData::Categories(names()),
            Arc::from(low),
            Arc::from(high),
            ErrorBarOptions::default(),
        ),
        Mark::ErrorBars => a.y_error_bars(
            AxisData::Categories(names()),
            &low,
            &high,
            ErrorBarOptions::default(),
        ),
        Mark::Dots => {
            let categories = AxisData::Categories(names());
            let values = AxisData::Numbers(high);
            let (x, y) = if horizontal {
                (values, categories)
            } else {
                (categories, values)
            };
            a.scatter_shared(x, y, ScatterOptions::default())
        }
    }
    .unwrap();
    f
}

fn resolved(f: &Figure, horizontal: bool, spacing: Option<TickSpacing>) -> Resolved {
    let axes = f.single_axes();
    let marks = resolve_axes_marks(axes, f.theme());
    // The value axis is 240 pt in either orientation; the category axis is 300 pt.
    let plot = if horizontal {
        PlotSize {
            width: 240.0,
            height: 300.0,
        }
    } else {
        PlotSize {
            width: 300.0,
            height: 240.0,
        }
    };
    let (mut x, mut y) = match spacing {
        Some(spacing) => tick_rooms(plot, crate::test_support::tick_font(f.theme()), spacing),
        None => TickRoom::unlimited(
            6,
            6,
            crate::test_support::tick_font(&crate::Theme::pigment()),
        ),
    };
    // Compare the same target without changing the screen-direction density policy.
    x.target = 6;
    y.target = 6;
    resolve(&marks, &Limits::of(axes), &x, &y)
}

#[track_caller]
fn assert_mirrors(vertical: &Resolved, horizontal: &Resolved, context: &str) {
    assert_eq!(vertical.baseline.axis, ValueAxis::Y, "{context}");
    assert_eq!(horizontal.baseline.axis, ValueAxis::X, "{context}");
    assert_eq!(
        vertical.y.continuous_scale().domain(),
        horizontal.x.continuous_scale().domain(),
        "limits: {context}"
    );
    assert_eq!(vertical.y.ticks, horizontal.x.ticks, "ticks: {context}");
    assert_eq!(
        vertical.baseline.pos, horizontal.baseline.pos,
        "baseline: {context}"
    );
}

fn check(mark: Mark, spacing: Option<TickSpacing>) {
    let positive = vec![
        vec![100.0, 110.0, 120.0],
        vec![130.0, 140.0, 150.0],
        vec![170.0, 180.0, 190.0],
    ];
    let crossing = vec![
        vec![-30.0, -10.0, 5.0],
        vec![-10.0, 20.0, 40.0],
        vec![10.0, 30.0, 65.0],
    ];
    let logarithmic = vec![
        vec![1.0, 3.0, 10.0],
        vec![10.0, 30.0, 100.0],
        vec![100.0, 300.0, 1000.0],
    ];
    let log = ScaleKind::Log { base: LogBase::TEN };
    for (name, groups, scale, limits) in [
        ("positive", &positive, ScaleKind::Linear, None),
        ("crossing", &crossing, ScaleKind::Linear, None),
        ("log", &logarithmic, log, None),
        (
            "explicit",
            &positive,
            ScaleKind::Linear,
            Some((80.0, 220.0)),
        ),
        (
            "explicit crossing",
            &crossing,
            ScaleKind::Linear,
            Some((-20.0, 50.0)),
        ),
        ("explicit log", &logarithmic, log, Some((0.5, 2000.0))),
    ] {
        let vertical = figure(mark, false, scale, groups, limits);
        let horizontal = figure(mark, true, scale, groups, limits);
        let context = format!("{mark:?}, {name}, {spacing:?}");
        assert_mirrors(
            &resolved(&vertical, false, spacing),
            &resolved(&horizontal, true, spacing),
            &context,
        );
    }
}

#[test]
fn equal_tick_targets_mirror_all_category_marks() {
    for mark in MARKS {
        check(mark, None);
    }
}

macro_rules! mirror_test {
    ($name:ident, $mark:ident) => {
        #[test]
        fn $name() {
            for spacing in [
                TickSpacing::Target,
                TickSpacing::Deferred,
                TickSpacing::LabelsApart,
            ] {
                check(Mark::$mark, Some(spacing));
            }
        }
    };
}

mirror_test!(box_value_axis_mirrors_vertical, Box);
mirror_test!(grouped_box_value_axis_mirrors_vertical, GroupedBox);
mirror_test!(violin_value_axis_mirrors_vertical, Violin);
mirror_test!(grouped_violin_value_axis_mirrors_vertical, GroupedViolin);
mirror_test!(bar_value_axis_mirrors_vertical, Bar);
mirror_test!(stacked_bar_value_axis_mirrors_vertical, StackedBar);
mirror_test!(grouped_bar_value_axis_mirrors_vertical, GroupedBar);
mirror_test!(error_bars_value_axis_mirrors_vertical, ErrorBars);
mirror_test!(dot_plot_value_axis_mirrors_vertical, Dots);

#[test]
fn distribution_value_axes_mirror_in_a_four_panel_layout() {
    use crate::{Grid, GridIndex, Placement};

    let groups = vec![
        vec![11.5, 20.0, 30.0, 35.0, 40.0, 50.0, 52.5],
        vec![15.0, 30.0, 38.0, 42.0, 48.0, 60.0, 65.0],
        vec![13.0, 20.0, 25.0, 29.0, 33.0, 40.0, 46.0],
    ];
    let mut f = Figure::with_grid(16.0, 4.0, Theme::pigment(), Grid::new(1, 4).unwrap()).unwrap();
    for col in 0..4 {
        let (x, y) = if col % 2 == 0 {
            (ScaleKind::Category, ScaleKind::Linear)
        } else {
            (ScaleKind::Linear, ScaleKind::Category)
        };
        let mut a = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                x,
                y,
            )
            .unwrap();
        let names = ["A", "B", "C"].map(str::to_owned).to_vec();
        if col < 2 {
            a.title("Box")
                .box_plot(names, groups.clone(), BoxOptions::default())
                .unwrap();
        } else {
            a.title("Violin")
                .violin(names, groups.clone(), ViolinOptions::default())
                .unwrap();
        }
    }
    let panel = |col| {
        let axes = f.axes_at(GridIndex::At(0), GridIndex::At(col)).unwrap();
        let marks = resolve_axes_marks(axes, f.theme());
        let tick = crate::test_support::tick_font(&crate::Theme::pigment());
        let (x, y) = TickRoom::unlimited(6, 6, tick);
        resolve(&marks, &Limits::of(axes), &x, &y)
    };
    for col in [0, 2] {
        assert_mirrors(&panel(col), &panel(col + 1), "four panels, equal targets");
    }
}

#[test]
fn explicit_value_ticks_mirror_with_and_without_limits() {
    let groups = vec![vec![100.0, 110.0], vec![130.0, 150.0], vec![170.0, 190.0]];
    for mark in MARKS {
        for scale in [ScaleKind::Linear, ScaleKind::Log { base: LogBase::TEN }] {
            for limits in [None, Some((90.0, 180.0))] {
                let mut vertical = figure(mark, false, scale, &groups, limits);
                let mut horizontal = figure(mark, true, scale, &groups, limits);
                let ticks = [80.0, 100.0, 150.0, 200.0];
                vertical.single_axes_mut().yticks(&ticks).unwrap();
                horizontal.single_axes_mut().xticks(&ticks).unwrap();
                assert_mirrors(
                    &resolved(&vertical, false, Some(TickSpacing::LabelsApart)),
                    &resolved(&horizontal, true, Some(TickSpacing::LabelsApart)),
                    &format!("{mark:?}, {scale:?}, {limits:?}, explicit ticks"),
                );
            }
        }
    }
}
