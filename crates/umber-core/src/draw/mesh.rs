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
//!
//! Drawn as one image ([`Render`](crate::Render)), the grid keeps only its rows and columns
//! with a shown part, one pixel per cell, and the image covers them whole: an outer cell that
//! an explicit limit cuts reaches past it, and a clip at the limits cuts it there, as the vector
//! cells are cut. A grid whose cut cells would reach more than [`MAX_IMAGE_REACH`] points past
//! the limits cannot be placed exactly in the scene's precision: with
//! [`Render::Auto`](crate::Render::Auto) it is drawn as vector shapes, and with
//! [`Render::Raster`](crate::Render::Raster) rendering fails ([`too_wide_image`]).

use super::MarkContext;
use super::cells::{CLEAR, MAX_IMAGE_REACH, Reach, cell_path, image_item, reach};
use super::mapping::{AxisMap, AxisMaps, axis_time, axis_value};
use crate::color_scale::ResolvedColorScale;
use crate::model::{Edges, Hist2dMark, MeshMark};
use crate::resolve::{Resolved, ResolvedAxis, ResolvedMark};
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
    // Each value is mapped once; the neighbours of a cell look their colors up.
    let colors: Vec<Option<Rgba>> = mark
        .colors
        .values
        .iter()
        .map(|&value| color.color(value))
        .collect();
    let grid = Grid {
        x: GridAxis::new(&context.maps.x, &axis_edges(&resolved.x, &mark.x_edges)),
        y: GridAxis::new(&context.maps.y, &axis_edges(&resolved.y, &mark.y_edges)),
        color: |column: usize, row: usize| colors.get(row * columns + column).copied().flatten(),
    };
    if mark
        .render
        .draws_image(columns.saturating_mul(rows), mark.evenly_spaced)
    {
        match grid.image() {
            Raster::Image(item) => {
                items.push(item);
                return;
            }
            Raster::Nothing => return,
            Raster::TooWide => {}
        }
    }
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
    // Each count is mapped once; the neighbours of a cell look their colors up.
    let colors: HashMap<(usize, usize), Rgba> = bins
        .cells
        .iter()
        .zip(&bins.counts)
        .filter_map(|(&cell, &count)| Some((cell, color.color(count)?)))
        .collect();
    let edges = |axis: &ResolvedAxis, edges: &[f64]| -> Vec<f64> {
        edges.iter().map(|&v| axis_value(axis, v)).collect()
    };
    let grid = Grid {
        x: GridAxis::new(&context.maps.x, &edges(&resolved.x, &bins.x_edges)),
        y: GridAxis::new(&context.maps.y, &edges(&resolved.y, &bins.y_edges)),
        color: |column: usize, row: usize| colors.get(&(column, row)).copied(),
    };
    let count = bins.x_edges.len().saturating_sub(1) * bins.y_edges.len().saturating_sub(1);
    if mark.render.draws_image(count, true) {
        match grid.image() {
            Raster::Image(item) => {
                items.push(item);
                return;
            }
            Raster::Nothing => return,
            Raster::TooWide => {}
        }
    }
    let mut cells = bins.cells.clone();
    cells.sort_unstable_by_key(|&(column, row)| (Reverse(row), column));
    for (column, row) in cells {
        grid.push(column, row, items);
    }
}

/// For a mesh or a 2-D histogram among `marks` that is drawn as one image because its options
/// say [`Render::Raster`](crate::Render::Raster), but whose cells cut by explicit limits reach
/// more than [`MAX_IMAGE_REACH`] points past the limits, the first such mark: its index among
/// `marks`, what it is (`"a mesh"`, `"a 2-D histogram"`), the axis (`"x"` or `"y"`) and how
/// far, in points, the cut cells reach. [`Render::Auto`](crate::Render::Auto) draws such marks
/// as vector shapes instead.
pub(super) fn too_wide_image(
    maps: &AxisMaps,
    resolved: &Resolved,
    marks: &[ResolvedMark<'_>],
) -> Option<(usize, &'static str, &'static str, f64)> {
    marks.iter().enumerate().find_map(|(i, mark)| {
        let (kind, x, y) = match mark {
            ResolvedMark::Mesh { mark, .. } if mark.render == crate::Render::Raster => (
                "a mesh",
                axis_edges(&resolved.x, &mark.x_edges),
                axis_edges(&resolved.y, &mark.y_edges),
            ),
            ResolvedMark::Hist2d { mark, .. } if mark.render == crate::Render::Raster => {
                let edges = |axis: &ResolvedAxis, edges: &[f64]| -> Vec<f64> {
                    edges.iter().map(|&v| axis_value(axis, v)).collect()
                };
                (
                    "a 2-D histogram",
                    edges(&resolved.x, &mark.bins.x_edges),
                    edges(&resolved.y, &mark.bins.y_edges),
                )
            }
            _ => return None,
        };
        if x.len() < 2 || y.len() < 2 {
            return None;
        }
        let (x, y) = (GridAxis::new(&maps.x, &x), GridAxis::new(&maps.y, &y));
        // Nothing is drawn when either axis shows no cell.
        x.shown_cells()?;
        y.shown_cells()?;
        let (axis, reach) = match (x.too_wide(), y.too_wide()) {
            (Some(reach), _) => ("x", reach),
            (None, Some(reach)) => ("y", reach),
            (None, None) => return None,
        };
        Some((i, kind, axis, reach))
    })
}

/// One axis of a grid of cells: where its edges lie on the plot once clipped to the axis
/// domain, and which of the cells between them are left.
struct GridAxis {
    /// The plot coordinate of each edge, clipped to the axis domain, in order.
    at: Vec<f64>,
    /// The plot coordinate of each edge before clipping, in order; far from the plot, or not
    /// finite, for an edge far beyond the domain.
    unclipped: Vec<f64>,
    /// For each cell, whether it keeps a positive extent once its edges are clipped.
    shown: Vec<bool>,
}

impl GridAxis {
    /// The axis of the increasing axis-space `edges` on `map`.
    fn new(map: &AxisMap, edges: &[f64]) -> GridAxis {
        let clipped: Vec<f64> = edges.iter().map(|v| v.clamp(map.lo, map.hi)).collect();
        GridAxis {
            at: clipped.iter().map(|&v| map.plot_at(v)).collect(),
            unclipped: edges.iter().map(|&v| map.plot_at(v)).collect(),
            shown: clipped.windows(2).map(|pair| pair[0] < pair[1]).collect(),
        }
    }

    /// The cells left after clipping, which are consecutive; `None` when there is none.
    fn shown_cells(&self) -> Option<std::ops::Range<usize>> {
        let first = self.shown.iter().position(|&shown| shown)?;
        let end = self.shown.iter().rposition(|&shown| shown)? + 1;
        Some(first..end)
    }

    /// The plot coordinates of the outer edges of the image of `cells`, its first and last
    /// edges before clipping, and of their shown part. A single cell needs no edge beyond the
    /// shown part, so both are the shown part. `Err` with how far, in points, an outer edge
    /// lies beyond the shown part when that is more than [`MAX_IMAGE_REACH`].
    fn image_span(&self, cells: &std::ops::Range<usize>) -> Result<([f64; 2], [f64; 2]), f64> {
        let shown = [self.at[cells.start], self.at[cells.end]];
        if cells.len() == 1 {
            return Ok((shown, shown));
        }
        let whole = [self.unclipped[cells.start], self.unclipped[cells.end]];
        let reach = whole
            .iter()
            .zip(shown)
            .map(|(edge, at)| (edge - at).abs())
            .fold(0.0_f64, f64::max);
        // A reach that is not a number (edges far beyond the domain) is too far too.
        if reach <= MAX_IMAGE_REACH {
            Ok((whole, shown))
        } else {
            Err(reach)
        }
    }

    /// How far, in points, the image of the shown cells would reach past the limits, when that
    /// is more than [`MAX_IMAGE_REACH`]; `None` when it can be drawn, or nothing is shown.
    fn too_wide(&self) -> Option<f64> {
        self.image_span(&self.shown_cells()?).err()
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

/// The cells between the edges of two grid axes, with `color(column, row)` looking up the color
/// a cell's value was mapped to, `None` for an empty cell. Columns count from the left and rows
/// from the bottom.
struct Grid<F> {
    /// The columns.
    x: GridAxis,
    /// The rows.
    y: GridAxis,
    /// The color each cell's value was mapped to, `None` when it is empty.
    color: F,
}

/// A grid drawn as one image, or why it is not.
enum Raster {
    /// The image, clipped to the shown part of the grid when explicit limits cut it.
    Image(Item),
    /// No cell is drawn.
    Nothing,
    /// A cut outer cell reaches too far past the limits to place exactly; the grid is drawn
    /// as vector shapes.
    TooWide,
}

impl<F: Fn(usize, usize) -> Option<Rgba>> Grid<F> {
    /// The cells left after clipping as one image, one pixel per cell (transparent for an
    /// empty cell), over the whole extent of those cells and clipped to their shown part.
    fn image(&self) -> Raster {
        let (Some(columns), Some(rows)) = (self.x.shown_cells(), self.y.shown_cells()) else {
            return Raster::Nothing;
        };
        let (Ok((x_whole, x_shown)), Ok((y_whole, y_shown))) =
            (self.x.image_span(&columns), self.y.image_span(&rows))
        else {
            return Raster::TooWide;
        };
        // Rows count from the bottom; the image starts at the top row.
        let pixels: Vec<Rgba> = rows
            .clone()
            .rev()
            .flat_map(|row| {
                columns
                    .clone()
                    .map(move |column| (self.color)(column, row).unwrap_or(CLEAR))
            })
            .collect();
        // The y axis points up, so its last edge is the top.
        let bounds = [x_whole[0], y_whole[1], x_whole[1], y_whole[0]];
        let visible = [x_shown[0], y_shown[1], x_shown[1], y_shown[0]];
        match image_item(bounds, visible, (columns.len(), rows.len()), pixels) {
            Some(item) => Raster::Image(item),
            None => Raster::Nothing,
        }
    }

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
