//! Output-gate cases for figures with several axes on a grid.
//!
//! The cases write only new output files, so no existing manifest entry changes. See
//! `scripts/check-refactor-output.sh`.
//!
//! Scenes, in the three built-in themes, at the automatic size of 6 in per column by 4 in per
//! row: a 1 × 2 grid of a line and bars; a 2 × 2 grid whose top axes spans both columns over a
//! scatter and a histogram; a 3 × 1 grid of labelled lines with header legends, one on a log y
//! axis; a 2 × 2 grid with an empty cell. Only axes in the first column have labelled marks:
//! the legend row is measured for the canvas width.
//!
//! In `errors/grid.txt`: every error branch of `Grid::new`, `Grid::cells`,
//! `Figure::add_axes_at` (in the order the checks run, with the figure's state after each
//! failed call), `Figure::axes_at`, `Figure::axes_at_mut` and `Axes::check_scales`; the axes
//! found at each cell; and layout errors on a grid, which name the axes' top-left cell.

use umber_core::figure::{AreaOptions, AxisData, BarOptions, HistOptions, LineOptions};
use umber_core::{
    Figure, FontLibrary, Grid, GridIndex, LegendMode, LogBase, Placement, ScaleKind, Share, Theme,
};

use super::super::{Log, Output, emit_scene, line, linear, log, record, scatter, themes, wobble};

/// Passes every output of this area to `sink`, in a fixed order.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (theme_name, theme) in themes() {
        for (name, figure) in [
            ("grid-1x2", one_by_two(theme.clone())),
            ("grid-2x2-span", spanning(theme.clone())),
            ("grid-3x1", three_by_one(theme.clone())),
            ("grid-2x2-empty", with_empty_cell(theme.clone())),
        ] {
            emit_scene(
                &format!("{name}-{theme_name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
    sink(Output {
        kind: "errors",
        file: "grid.txt".into(),
        bytes: grid_errors(lib).into_bytes(),
    });
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

/// The placement of an axes at `row`, `col` spanning `row_span × col_span` cells.
fn at(row: i64, col: i64, row_span: i64, col_span: i64) -> Placement {
    Placement {
        row: GridIndex::At(row),
        col: GridIndex::At(col),
        row_span,
        col_span,
    }
}

/// An empty figure on a `rows × cols` grid at the grid's automatic size.
fn grid_figure(rows: i64, cols: i64, theme: Theme) -> Figure {
    let grid = Grid::new(rows, cols).unwrap();
    let (width, height) = grid.auto_size_in();
    Figure::with_grid(width, height, theme, grid).unwrap()
}

fn labelled_line(label: &str) -> LineOptions {
    LineOptions {
        label: label.into(),
        ..LineOptions::default()
    }
}

/// A line beside bars.
fn one_by_two(theme: Theme) -> Figure {
    let mut f = grid_figure(1, 2, theme);
    let (x, y) = wobble(40, 3);
    f.add_axes_at(at(0, 0, 1, 1), linear(), linear())
        .unwrap()
        .title("Signal")
        .xlabel("Step")
        .ylabel("Level")
        .line(AxisData::Numbers(x), &y, line())
        .unwrap();
    f.add_axes_at(at(0, 1, 1, 1), ScaleKind::Category, linear())
        .unwrap()
        .title("Totals")
        .subtitle("By region")
        .bar(
            names(&["North", "East", "South", "West"]),
            &[4.0, 7.5, 3.25, 6.0],
            BarOptions::default(),
        )
        .unwrap();
    f
}

/// A labelled line chart across the top row over a scatter and a histogram.
fn spanning(theme: Theme) -> Figure {
    let mut f = grid_figure(2, 2, theme);
    let mut top = f.add_axes_at(at(0, 0, 1, 2), linear(), linear()).unwrap();
    top.title("Load across the year").ylabel("MW");
    for (seed, label) in [(11, "North"), (12, "South")] {
        let (x, y) = wobble(120, seed);
        top.line(AxisData::Numbers(x), &y, labelled_line(label))
            .unwrap();
    }
    let (x, y) = wobble(60, 21);
    f.add_axes_at(at(1, 0, 1, 1), linear(), linear())
        .unwrap()
        .title("Spread")
        .scatter(AxisData::Numbers(x), &y, scatter())
        .unwrap();
    let (_, values) = wobble(400, 22);
    f.add_axes_at(at(1, 1, 1, 1), linear(), linear())
        .unwrap()
        .title("Distribution")
        .xlabel("Level")
        .hist(
            &values,
            HistOptions {
                bins: 12,
                ..HistOptions::default()
            },
        )
        .unwrap();
    f
}

/// Three rows of labelled lines, the last on a log y axis.
fn three_by_one(theme: Theme) -> Figure {
    let mut f = grid_figure(3, 1, theme);
    for (row, title) in [(0, "North"), (1, "South")] {
        let mut axes = f.add_axes_at(at(row, 0, 1, 1), linear(), linear()).unwrap();
        axes.title(title);
        for (seed, label) in [(31, "Day"), (32, "Night")] {
            let (x, y) = wobble(48, seed + 2 * row as u32);
            axes.line(AxisData::Numbers(x), &y, labelled_line(label))
                .unwrap();
        }
    }
    let x: Vec<f64> = (0..24).map(f64::from).collect();
    let y: Vec<f64> = x.iter().map(|v| 2.0 * 1.35_f64.powf(*v)).collect();
    f.add_axes_at(at(2, 0, 1, 1), linear(), log(10.0))
        .unwrap()
        .title("Growth")
        .xlabel("Hour")
        .line(AxisData::Numbers(x), &y, labelled_line("Total"))
        .unwrap();
    f
}

/// Three axes and an empty cell at the bottom right.
fn with_empty_cell(theme: Theme) -> Figure {
    let mut f = grid_figure(2, 2, theme);
    let (x, y) = wobble(30, 41);
    f.add_axes_at(at(0, 0, 1, 1), linear(), linear())
        .unwrap()
        .title("Line")
        .line(AxisData::Numbers(x), &y, line())
        .unwrap();
    f.add_axes_at(at(0, 1, 1, 1), ScaleKind::Category, linear())
        .unwrap()
        .title("Bars")
        .bar(
            names(&["a", "b", "c"]),
            &[3.0, -1.0, 2.0],
            BarOptions::default(),
        )
        .unwrap();
    let (x, y) = wobble(30, 42);
    let y: Vec<f64> = y.iter().map(|v| v.abs() + 1.0).collect();
    f.add_axes_at(at(1, 0, 1, 1), linear(), linear())
        .unwrap()
        .title("Area")
        .area(AxisData::Numbers(x), &y, AreaOptions::default())
        .unwrap();
    f
}

/// The axes found at a cell, by its cells and title.
fn found(axes: &umber_core::Axes) -> String {
    let c = axes.cells();
    format!(
        "cells row={} col={} row_span={} col_span={} title={}",
        c.row(),
        c.col(),
        c.row_span(),
        c.col_span(),
        record::text(axes.title_text())
    )
}

/// A grid index as the records write it.
fn index(i: GridIndex) -> String {
    match i {
        GridIndex::Auto => "AUTO".to_owned(),
        GridIndex::At(i) => i.to_string(),
    }
}

/// The results of the grid calls, and the state each failed call leaves.
fn grid_errors(lib: &FontLibrary) -> String {
    let mut rec = Log::default();

    rec.section("Grid::new");
    for (rows, cols) in [
        (1, 1),
        (32, 32),
        (0, 1),
        (-1, 0),
        (33, 1),
        (1, 0),
        (1, 33),
        (i64::MIN, i64::MAX),
        (2, i64::MAX),
    ] {
        rec.result(
            &format!("Grid::new({rows}, {cols})"),
            Grid::new(rows, cols),
            |g| {
                let (width, height) = g.auto_size_in();
                format!(
                    "rows={} cols={} auto_size_in={}x{}",
                    g.rows(),
                    g.cols(),
                    record::f64(width),
                    record::f64(height)
                )
            },
        );
    }
    let shared = Grid::new(2, 2)
        .unwrap()
        .with_share_x(Share::Col)
        .with_share_y(Share::All)
        .with_legend(LegendMode::PerPanel);
    let settings = (shared.share_x(), shared.share_y(), shared.legend());
    let expected = (Share::Col, Share::All, LegendMode::PerPanel);
    rec.raw(&format!("Grid settings kept: {}\n", settings == expected));

    rec.section("Grid::cells");
    let grid = Grid::new(2, 3).unwrap();
    for count in [1, 4, 6, 0, 7, -1, i64::MIN] {
        rec.result(
            &format!("2 × 3 cells({count})"),
            grid.cells(count),
            |cells| {
                cells
                    .iter()
                    .map(|c| format!("({}, {})", c.row, c.col))
                    .collect::<Vec<_>>()
                    .join(" ")
            },
        );
    }

    rec.section("Figure::add_axes_at, in the order of its checks");
    let mut f = grid_figure(3, 3, Theme::pigment());
    let result = f
        .add_axes_at(at(1, 0, 1, 2), linear(), linear())
        .map(|mut a| {
            a.title("held");
        });
    rec.result("add_axes_at(1, 0, 1, 2)", result, |()| String::new());
    rec.raw(&record::figure(&f));
    let auto = Placement::default();
    let category = ScaleKind::Category;
    for (label, place, y) in [
        ("auto row", auto, category),
        (
            "auto col",
            Placement {
                row: GridIndex::At(0),
                ..auto
            },
            category,
        ),
        ("row out of range", at(3, -1, 0, 0), category),
        ("negative row", at(-1, 0, 1, 1), linear()),
        ("col out of range", at(0, 3, 0, 0), category),
        ("row_span 0", at(0, 0, 0, -1), category),
        ("col_span -1", at(0, 0, 1, -1), category),
        ("row_span past the grid", at(2, 0, 2, 9), category),
        ("col_span past the grid", at(0, 1, 1, 3), category),
        ("col_span i64::MAX", at(0, 2, 1, i64::MAX), category),
        ("category y", at(1, 0, 1, 1), category),
        ("taken cell", at(0, 1, 2, 2), linear()),
        ("taken top-left cell", at(1, 0, 1, 1), linear()),
    ] {
        let result = f.add_axes_at(place, linear(), y).map(drop);
        rec.result(label, result, |()| String::new());
        rec.raw(&record::figure(&f));
    }
    for (label, place, x) in [
        ("add_axes_at(0, 0, 1, 3)", at(0, 0, 1, 3), category),
        ("add_axes_at(2, 2, 1, 1)", at(2, 2, 1, 1), linear()),
    ] {
        let result = f.add_axes_at(place, x, linear()).map(drop);
        rec.result(label, result, |()| String::new());
    }
    rec.raw(&record::figure(&f));
    rec.result(
        "add_axes on a 3 × 3 grid",
        f.add_axes(linear(), linear()).map(drop),
        |()| String::new(),
    );

    rec.section("Figure::axes_at and Figure::axes_at_mut");
    for (row, col) in [
        (GridIndex::At(0), GridIndex::At(0)),
        (GridIndex::At(1), GridIndex::At(0)),
        (GridIndex::At(2), GridIndex::At(2)),
        (GridIndex::At(0), GridIndex::At(2)),
        (GridIndex::At(1), GridIndex::At(1)),
        (GridIndex::At(2), GridIndex::At(1)),
        (GridIndex::Auto, GridIndex::At(0)),
        (GridIndex::At(0), GridIndex::Auto),
        (GridIndex::At(3), GridIndex::At(0)),
        (GridIndex::At(0), GridIndex::At(-1)),
    ] {
        rec.result(
            &format!("axes_at({}, {})", index(row), index(col)),
            f.axes_at(row, col),
            found,
        );
        let result = f.axes_at_mut(row, col).map(|a| found(&a));
        rec.result(
            &format!("axes_at_mut({}, {})", index(row), index(col)),
            result,
            |s| s,
        );
    }

    rec.section("Axes::check_scales");
    let log2 = log(2.0);
    let ten = ScaleKind::Log { base: LogBase::TEN };
    let mut scaled = grid_figure(1, 2, Theme::pigment());
    scaled
        .add_axes_at(at(0, 1, 1, 1), ScaleKind::Category, log2)
        .unwrap();
    let axes = scaled.axes_at(GridIndex::Auto, GridIndex::At(1)).unwrap();
    for (label, x, y) in [
        ("category/log2", ScaleKind::Category, log2),
        ("linear/log2", linear(), log2),
        ("log10/log2", ten, log2),
        ("category/log10", ScaleKind::Category, ten),
        ("category/linear", ScaleKind::Category, linear()),
        (
            "category/category",
            ScaleKind::Category,
            ScaleKind::Category,
        ),
    ] {
        rec.result(label, axes.check_scales(x, y), |()| String::new());
    }

    rec.section("Figure::to_scene, layout errors name the axes' top-left cell");
    let mut wide = grid_figure(1, 3, Theme::midnight());
    wide.add_axes_at(at(0, 0, 1, 1), linear(), linear())
        .unwrap();
    wide.add_axes_at(at(0, 1, 1, 2), linear(), linear())
        .unwrap()
        .xticks_labeled(&[0.0, 1.0], vec!["0".into(), "W".repeat(200)])
        .unwrap();
    rec.result(
        "an x tick label wider than its cells",
        wide.to_scene(lib).map(drop),
        |()| String::new(),
    );
    let mut short =
        Figure::with_grid(4.0, 1.0, Theme::graphite(), Grid::new(2, 1).unwrap()).unwrap();
    short
        .add_axes_at(at(1, 0, 1, 1), linear(), linear())
        .unwrap()
        .title("Too short for its cell")
        .subtitle("Twice over");
    rec.result(
        "a cell too short for its axes",
        short.to_scene(lib).map(drop),
        |()| String::new(),
    );
    let mut narrow = grid_figure(1, 3, Theme::pigment());
    narrow
        .add_axes_at(at(0, 2, 1, 1), linear(), linear())
        .unwrap()
        .yticks_labeled(&[0.0, 1.0], vec!["0".into(), "W".repeat(60)])
        .unwrap();
    rec.result(
        "y tick labels wider than their cell",
        narrow.to_scene(lib).map(drop),
        |()| String::new(),
    );
    let empty = grid_figure(4, 5, Theme::pigment());
    rec.result(
        "an empty 4 × 5 grid",
        empty
            .to_scene(lib)
            .map(|s| format!("items={}", s.items.len())),
        |s| s,
    );
    rec.out
}
