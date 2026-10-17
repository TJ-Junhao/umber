//! Meshes and 2-D histograms: one opaque rectangle per cell between numeric or time edges, in
//! the color its value maps to on the axes' color scale.
//!
//! Each cell is clipped to the axis domains in axis space before it is mapped to plot points:
//! a cell beyond an explicit limit is left out and a cell across one ends at it. The cells meet
//! without a seam, as [`cells`](super::cells) describes. Edges increase and continuous and time
//! axes never run backwards, so the columns of a grid run left to right and its rows bottom to
//! top; the rows are drawn from the top down, each from left to right, and a cell reaches under
//! its drawn neighbours to the right and below. It never reaches into an empty cell (a NaN
//! value, an empty bin or a cell clipped away) or past the outer edges of the grid.

use super::MarkContext;
use super::cells::{Reach, cell_path, reach};
use super::mapping::{AxisMap, axis_time, axis_value};
use crate::color_scale::ResolvedColorScale;
use crate::model::{Edges, Hist2dMark, MeshMark};
use crate::resolve::{Resolved, ResolvedAxis};
use crate::{FillRule, Item, Paint, Rgba};
use std::cmp::Reverse;
use std::collections::HashMap;

/// The edges in axis space: numbers through the axis' scale, times as seconds from its origin.
fn axis_edges(axis: &ResolvedAxis, edges: &Edges) -> Vec<f64> {
    match edges {
        Edges::Numbers(edges) => edges.iter().map(|&v| axis_value(axis, v)).collect(),
        Edges::Times(edges) => edges.iter().map(|&t| axis_time(axis, t)).collect(),
    }
}

/// Draws finite row-major values, leaving NaN cells empty.
pub(super) fn mesh_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &MeshMark,
    color: &ResolvedColorScale,
    items: &mut Vec<Item>,
) {
    let columns = mark.x_edges.len() - 1;
    let rows = mark.y_edges.len() - 1;
    let grid = Grid {
        x: GridAxis::new(&context.maps.x, &axis_edges(&resolved.x, &mark.x_edges)),
        y: GridAxis::new(&context.maps.y, &axis_edges(&resolved.y, &mark.y_edges)),
        color: |column: usize, row: usize| {
            let value = *mark.colors.values.get(row * columns + column)?;
            color.color(value)
        },
    };
    for row in (0..rows).rev() {
        for column in 0..columns {
            grid.push(column, row, items);
        }
    }
}

/// Draws only the non-empty count cells.
pub(super) fn hist2d_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &Hist2dMark,
    color: &ResolvedColorScale,
    items: &mut Vec<Item>,
) {
    let bins = &mark.bins;
    let counts: HashMap<(usize, usize), f64> = bins
        .cells
        .iter()
        .copied()
        .zip(bins.counts.iter().copied())
        .collect();
    let edges = |axis: &ResolvedAxis, edges: &[f64]| -> Vec<f64> {
        edges.iter().map(|&v| axis_value(axis, v)).collect()
    };
    let grid = Grid {
        x: GridAxis::new(&context.maps.x, &edges(&resolved.x, &bins.x_edges)),
        y: GridAxis::new(&context.maps.y, &edges(&resolved.y, &bins.y_edges)),
        color: |column: usize, row: usize| color.color(*counts.get(&(column, row))?),
    };
    let mut cells = bins.cells.clone();
    cells.sort_unstable_by_key(|&(column, row)| (Reverse(row), column));
    for (column, row) in cells {
        grid.push(column, row, items);
    }
}

/// One axis of a grid of cells: where its edges lie on the plot once clipped to the axis
/// domain, and which of the cells between them are left.
struct GridAxis {
    /// The plot coordinate of each edge, clipped to the axis domain, in order.
    at: Vec<f64>,
    /// For each cell, whether it keeps a positive extent once its edges are clipped.
    shown: Vec<bool>,
}

impl GridAxis {
    /// The axis of the increasing axis-space `edges` on `map`.
    fn new(map: &AxisMap, edges: &[f64]) -> GridAxis {
        let clipped: Vec<f64> = edges.iter().map(|v| v.clamp(map.lo, map.hi)).collect();
        GridAxis {
            at: clipped.iter().map(|&v| map.plot_at(v)).collect(),
            shown: clipped.windows(2).map(|pair| pair[0] < pair[1]).collect(),
        }
    }

    /// Whether cell `i` exists and is left after clipping.
    fn shown(&self, i: usize) -> bool {
        self.shown.get(i).copied().unwrap_or(false)
    }

    /// The size of cell `i`, which must exist, in points.
    fn size(&self, i: usize) -> f64 {
        (self.at[i + 1] - self.at[i]).abs()
    }
}

/// The cells between the edges of two grid axes, with `color(column, row)` the color of a cell
/// or `None` for an empty one. Columns count from the left and rows from the bottom.
struct Grid<F> {
    /// The columns.
    x: GridAxis,
    /// The rows.
    y: GridAxis,
    /// The color of each cell, `None` when it is empty.
    color: F,
}

impl<F: Fn(usize, usize) -> Option<Rgba>> Grid<F> {
    /// The color of the cell at `column` and `row` when it is drawn: when it exists, is left
    /// after clipping, and is not empty.
    fn drawn(&self, column: usize, row: usize) -> Option<Rgba> {
        if self.x.shown(column) && self.y.shown(row) {
            (self.color)(column, row)
        } else {
            None
        }
    }

    /// Appends the fill of the cell at `column` and `row` to `items` when it is drawn, reaching
    /// under its drawn neighbours to the right and below, which are drawn after it.
    fn push(&self, column: usize, row: usize, items: &mut Vec<Item>) {
        let Some(color) = self.drawn(column, row) else {
            return;
        };
        let right = self.drawn(column + 1, row).is_some();
        let below = row
            .checked_sub(1)
            .filter(|&below| self.drawn(column, below).is_some());
        let corner = row
            .checked_sub(1)
            .is_some_and(|below| self.drawn(column + 1, below).is_some());
        let reaching = Reach {
            right: if right {
                reach(self.x.size(column + 1))
            } else {
                0.0
            },
            down: below.map_or(0.0, |below| reach(self.y.size(below))),
            corner,
        };
        let bounds = [
            self.x.at[column],
            self.y.at[row + 1],
            self.x.at[column + 1],
            self.y.at[row],
        ];
        items.push(Item::Fill {
            path: cell_path(bounds, reaching),
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        });
    }
}
