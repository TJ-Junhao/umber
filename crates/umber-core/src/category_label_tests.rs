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

/// Checks the smallest fitting 1/2/5 step on this plot, including category zero and band centers.
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

fn bars_with_wide_last_label(n: usize, width: f64, letters: usize) -> Figure {
    let mut figure = Figure::new(width, 4.0, Theme::pigment()).unwrap();
    let mut categories = names(n);
    categories[n - 1] = "W".repeat(letters);
    figure
        .add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .bar(categories, &vec![1.0; n], BarOptions::default())
        .unwrap();
    figure
}

/// Tries each x step on the plot its own labels produce, keeping the other axis fixed.
fn smallest_self_fitting_step(
    figure: &Figure,
    resolved: &Resolved,
    lib: &FontLibrary,
) -> (usize, Vec<AxisTick>) {
    let all = all_ticks(resolved, Side::X);
    with_layout_input(figure, (0, 0), lib, |input, grid| {
        for power in 0..10 {
            for factor in [1, 2, 5] {
                let k = factor * 10_usize.pow(power);
                let mut candidate = resolved.clone();
                candidate.x.ticks = all.iter().step_by(k).cloned().collect();
                let bands = layout::measure_panel(
                    input,
                    &candidate,
                    grid.span(input.axes.cells()),
                    [true, true],
                );
                if let Ok(solution) = layout::solve_grid(grid, &[bands])
                    && Side::X
                        .room(solution.panels[0].plot.size(), figure.theme(), lib)
                        .slack(&candidate.x.ticks)
                        >= 0.0
                {
                    return (k, candidate.x.ticks);
                }
            }
        }
        panic!("no self-fitting category step")
    })
}

#[test]
fn category_labels_converge_with_wide_end_labels() {
    let lib = FontLibrary::bundled();
    let mut failures = Vec::new();
    for (n, width, letters, expected_k) in [
        (2, 1.75, 8, 2),
        (2, 2.0, 10, 2),
        (2, 3.0, 20, 2),
        (3, 2.0, 12, 5),
    ] {
        let figure = bars_with_wide_last_label(n, width, letters);
        let (resolved, layout) = try_plan_figure(&figure, (0, 0), &lib).unwrap();
        let slack = Side::X
            .room(layout.plot.size(), figure.theme(), &lib)
            .slack(&resolved.x.ticks);
        let (k, expected) = smallest_self_fitting_step(&figure, &resolved, &lib);
        assert_eq!(k, expected_k);
        if slack < 0.0 || resolved.x.ticks != expected {
            failures.push((n, width, letters, slack, k));
        }
        assert_eq!(layout.x_tick_labels[0].text, "c0");
        figure.to_scene(&lib).unwrap();
    }
    assert!(
        failures.is_empty(),
        "(n, width, letters, slack, expected k): {failures:?}"
    );
}

#[test]
fn category_labels_wide_end_sweep_fits_and_is_minimal() {
    let lib = FontLibrary::bundled();
    let mut rendered = 0;
    let mut errors = 0;
    let mut overlaps = Vec::new();
    let mut nonminimal = Vec::new();
    for n in 2..=15 {
        for quarters in 6..=16 {
            let width = f64::from(quarters) / 4.0;
            for letters in 1..=39 {
                let figure = bars_with_wide_last_label(n, width, letters);
                let (resolved, layout) = match try_plan_figure(&figure, (0, 0), &lib) {
                    Ok(planned) => planned,
                    Err(error) => {
                        assert!(matches!(error, crate::Error::Style { .. }), "{error}");
                        errors += 1;
                        continue;
                    }
                };
                figure.to_scene(&lib).unwrap();
                rendered += 1;
                let slack = Side::X
                    .room(layout.plot.size(), figure.theme(), &lib)
                    .slack(&resolved.x.ticks);
                if slack < 0.0 {
                    overlaps.push((n, width, letters, slack));
                }
                let (k, expected) = smallest_self_fitting_step(&figure, &resolved, &lib);
                if resolved.x.ticks != expected {
                    nonminimal.push((n, width, letters, k));
                }
                assert_eq!(layout.x_tick_labels[0].text, "c0");
            }
        }
    }
    eprintln!(
        "wide end sweep: {rendered} rendered, {errors} layout errors, {} overlaps, {} nonminimal",
        overlaps.len(),
        nonminimal.len()
    );
    assert_eq!(rendered + errors, 6006);
    assert!(rendered >= 5679);
    assert!(overlaps.is_empty(), "overlaps: {overlaps:?}");
    assert!(nonminimal.is_empty(), "nonminimal: {nonminimal:?}");
}

#[test]
fn category_labels_keep_an_overwide_first_label_error() {
    let lib = FontLibrary::bundled();
    for side in Side::BOTH {
        let mut figure = Figure::new(2.0, 4.0, Theme::pigment()).unwrap();
        let (x, y) = match side {
            Side::X => (ScaleKind::Category, ScaleKind::Linear),
            Side::Y => (ScaleKind::Linear, ScaleKind::Category),
        };
        figure
            .add_axes(x, y)
            .unwrap()
            .bar(
                vec!["W".repeat(500), "c1".into()],
                &[1.0; 2],
                BarOptions::default(),
            )
            .unwrap();
        let error = try_plan_figure(&figure, (0, 0), &lib).unwrap_err();
        assert!(matches!(error, crate::Error::Style { .. }));
        let text = error.to_string();
        assert!(text.contains("tick label"), "{text}");
        assert!(
            text.contains(&crate::error::quoted(&"W".repeat(500))),
            "{text}"
        );
    }
}

#[test]
fn category_labels_are_minimal_for_varied_names_and_sizes() {
    let lib = FontLibrary::bundled();
    let mut combinations = 0;
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
                    combinations += 1;
                }
            }
        }
    }
    assert_eq!(combinations, 90);
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
