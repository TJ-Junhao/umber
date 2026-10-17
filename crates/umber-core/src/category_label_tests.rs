//! Automatic category labels, their spacing and their shared-axis positions.

use super::*;
use crate::model::{BarOptions, Grid, GridIndex, HeatmapOptions, Placement};
use crate::resolve::{AxisScale, AxisTick};
use crate::{ScaleKind, Share};

fn names(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("c{i}")).collect()
}

fn bars(n: usize, width: f64, height: f64, side: Side) -> Figure {
    let mut figure = Figure::new(width, height, Theme::pigment()).unwrap();
    let (x, y) = match side {
        Side::X => (ScaleKind::Category, ScaleKind::Linear),
        Side::Y => (ScaleKind::Linear, ScaleKind::Category),
    };
    figure
        .add_axes(x, y)
        .unwrap()
        .bar(names(n), &vec![1.0; n], BarOptions::default())
        .unwrap();
    figure
}

fn all_ticks(resolved: &Resolved, side: Side) -> Vec<AxisTick> {
    let axis = match side {
        Side::X => &resolved.x,
        Side::Y => &resolved.y,
    };
    let AxisScale::Category(scale) = &axis.scale else {
        panic!("category scale")
    };
    scale
        .categories()
        .iter()
        .enumerate()
        .map(|(i, label)| AxisTick {
            pos: NormalizedPos::new(match side {
                Side::X => scale.center(i),
                Side::Y => 1.0 - scale.center(i),
            }),
            label: label.clone(),
        })
        .collect()
}

/// Checks the smallest fitting 1/2/5 step, including category zero and the band centers.
fn assert_step(figure: &Figure, resolved: &Resolved, layout: &Layout, side: Side) -> usize {
    let lib = FontLibrary::bundled();
    let room = side.room(layout.plot.size(), figure.theme(), &lib);
    let all = all_ticks(resolved, side);
    for power in 0..10 {
        for factor in [1, 2, 5] {
            let k = factor * 10_usize.pow(power);
            let expected: Vec<_> = all.iter().step_by(k).cloned().collect();
            if room.slack(&expected) >= 0.0 {
                assert_eq!(
                    side.ticks(resolved),
                    expected,
                    "{side:?}, k={k}, {:?}",
                    layout.plot
                );
                let labels = match side {
                    Side::X => &layout.x_tick_labels,
                    Side::Y => &layout.y_tick_labels,
                };
                if !labels.is_empty() {
                    assert_eq!(labels.len(), expected.len());
                    assert_eq!(labels[0].text, all[0].label);
                }
                return k;
            }
        }
    }
    panic!("no fitting category step")
}

#[test]
fn category_labels_use_smallest_round_step_on_x() {
    let lib = FontLibrary::bundled();
    for (n, width, k) in [
        (3, 6.0, 1),
        (20, 6.0, 1),
        (40, 6.0, 2),
        (200, 6.0, 20),
        (200, 12.0, 10),
        (1000, 6.0, 100),
        (1000, 12.0, 50),
    ] {
        let figure = bars(n, width, 4.0, Side::X);
        let (resolved, layout) = try_plan_figure(&figure, (0, 0), &lib).unwrap();
        assert_eq!(assert_step(&figure, &resolved, &layout, Side::X), k);
    }
}

#[test]
fn category_labels_use_smallest_round_step_on_y() {
    let lib = FontLibrary::bundled();
    for (n, height, k) in [
        (3, 4.0, 1),
        (20, 4.0, 2),
        (200, 4.0, 20),
        (200, 8.0, 10),
        (1000, 4.0, 100),
        (1000, 8.0, 50),
    ] {
        let figure = bars(n, 6.0, height, Side::Y);
        let (resolved, layout) = try_plan_figure(&figure, (0, 0), &lib).unwrap();
        assert_eq!(assert_step(&figure, &resolved, &layout, Side::Y), k);
    }
}

#[test]
fn category_labels_thin_a_thousand_by_thousand_heatmap() {
    let lib = FontLibrary::bundled();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &names(1000),
            &names(1000),
            &vec![1.0; 1_000_000],
            HeatmapOptions::default(),
        )
        .unwrap();
    let (resolved, layout) = try_plan_figure(&figure, (0, 0), &lib).unwrap();
    for side in Side::BOTH {
        assert_eq!(assert_step(&figure, &resolved, &layout, side), 100);
    }
}

#[test]
fn category_labels_keep_explicit_numeric_labels_unchanged() {
    let lib = FontLibrary::bundled();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let positions: Vec<_> = (0..100).map(f64::from).collect();
    let labels = names(100);
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xticks_labeled(&positions, labels.clone())
        .unwrap()
        .yticks_labeled(&positions, labels.clone())
        .unwrap();
    let (resolved, layout) = try_plan_figure(&figure, (0, 0), &lib).unwrap();
    for side in Side::BOTH {
        assert_eq!(
            side.ticks(&resolved)
                .iter()
                .map(|t| &t.label)
                .collect::<Vec<_>>(),
            labels.iter().collect::<Vec<_>>()
        );
        assert!(
            side.room(layout.plot.size(), figure.theme(), &lib)
                .slack(side.ticks(&resolved))
                < 0.0
        );
    }
}

#[test]
fn category_labels_agree_across_shared_panels() {
    let lib = FontLibrary::bundled();
    for side in Side::BOTH {
        let grid = Grid::new(2, 2).unwrap();
        let grid = match side {
            Side::X => grid.with_share_x(Share::All),
            Side::Y => grid.with_share_y(Share::All),
        };
        let mut figure = Figure::with_grid(8.0, 6.0, Theme::pigment(), grid).unwrap();
        for (row, col) in [(1, 1), (0, 1), (1, 0), (0, 0)] {
            let (x, y) = match side {
                Side::X => (ScaleKind::Category, ScaleKind::Linear),
                Side::Y => (ScaleKind::Linear, ScaleKind::Category),
            };
            figure
                .add_axes_at(
                    Placement {
                        row: GridIndex::At(row),
                        col: GridIndex::At(col),
                        ..Placement::default()
                    },
                    x,
                    y,
                )
                .unwrap()
                .bar(names(200), &vec![1.0; 200], BarOptions::default())
                .unwrap();
        }
        let planned = try_plan_panels(&figure, &lib).unwrap();
        let first = side.ticks(&planned[0].0);
        assert!(first.len() < 200);
        for (resolved, layout) in &planned {
            assert_eq!(side.ticks(resolved), first);
            assert_step(&figure, resolved, layout, side);
        }
    }
}

#[test]
fn category_labels_can_skip_an_overwide_last_label() {
    let lib = FontLibrary::bundled();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut categories = names(20);
    categories[19] = "W".repeat(500);
    figure
        .add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .bar(categories, &[1.0; 20], BarOptions::default())
        .unwrap();
    let (resolved, layout) = try_plan_figure(&figure, (0, 0), &lib).unwrap();
    assert_eq!(assert_step(&figure, &resolved, &layout, Side::X), 2);
}

#[test]
fn category_labels_are_minimal_for_varied_names_and_sizes() {
    let lib = FontLibrary::bundled();
    for side in Side::BOTH {
        for width in [2.0, 3.0, 4.0, 6.0, 9.0] {
            for height in [1.5, 2.0, 4.0] {
                for n in [7, 25, 60] {
                    let mut figure = Figure::new(width, height, Theme::pigment()).unwrap();
                    let (x, y) = match side {
                        Side::X => (ScaleKind::Category, ScaleKind::Linear),
                        Side::Y => (ScaleKind::Linear, ScaleKind::Category),
                    };
                    let categories: Vec<_> = (0..n)
                        .map(|i| {
                            if i % 3 == 0 {
                                format!("category {i}")
                            } else {
                                format!("c{i}")
                            }
                        })
                        .collect();
                    figure
                        .add_axes(x, y)
                        .unwrap()
                        .bar(categories, &vec![1.0; n], BarOptions::default())
                        .unwrap();
                    let (resolved, layout) = try_plan_figure(&figure, (0, 0), &lib).unwrap();
                    assert_step(&figure, &resolved, &layout, side);
                }
            }
        }
    }
}

#[test]
fn category_labels_stay_minimal_when_y_labels_release_width() {
    let lib = FontLibrary::bundled();
    for nx in [15, 20, 25] {
        for ny in 24..40 {
            let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let mut rows = names(ny);
            rows[2] = "a wider category label on an intermediate step".into();
            figure
                .add_axes(ScaleKind::Category, ScaleKind::Category)
                .unwrap()
                .heatmap(
                    &names(nx),
                    &rows,
                    &vec![1.0; nx * ny],
                    HeatmapOptions::default(),
                )
                .unwrap();
            let (resolved, layout) = try_plan_figure(&figure, (0, 0), &lib).unwrap();
            for side in Side::BOTH {
                assert_step(&figure, &resolved, &layout, side);
            }
        }
    }
}

#[test]
fn category_labels_preserve_unrelated_size_errors() {
    let lib = FontLibrary::bundled();
    for (width, height) in [(0.2, 4.0), (6.0, 0.2)] {
        let figure = bars(200, width, height, Side::X);
        assert!(matches!(
            try_plan_figure(&figure, (0, 0), &lib),
            Err(crate::Error::Style { .. })
        ));
    }
}
