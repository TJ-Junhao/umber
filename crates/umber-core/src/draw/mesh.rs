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
//! cells are cut. The image's pixels are evenly spaced, so before it is drawn every edge is
//! mapped to the final plot and compared with its place among them. A grid whose cut cells
//! would reach more than [`MAX_IMAGE_REACH`] points past the limits, or one with an edge more
//! than [`MAX_EDGE_OFFSET`] points from its place (edges evenly spaced to the call's tolerance
//! that the limits zoom in on), cannot be drawn exactly as one image: with
//! [`Render::Auto`](crate::Render::Auto) it is drawn as vector shapes, and with
//! [`Render::Raster`](crate::Render::Raster) rendering fails ([`unplaceable_image`]).

use super::MarkContext;
use super::cells::{CLEAR, MAX_EDGE_OFFSET, MAX_IMAGE_REACH, Reach, cell_path, image_item, reach};
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

/// The grid of `mark` on `maps`, its cells colored through `color`.
fn mesh_grid(
    maps: &AxisMaps,
    resolved: &Resolved,
    mark: &MeshMark,
    color: &ResolvedColorScale,
) -> Grid<impl Fn(usize, usize) -> Option<Rgba> + use<>> {
    let columns = mark.x_edges.len() - 1;
    // Each value is mapped once; the neighbours of a cell look their colors up.
    let colors: Vec<Option<Rgba>> = mark
        .colors
        .values
        .iter()
        .map(|&value| color.color(value))
        .collect();
    Grid {
        x: GridAxis::new(&maps.x, &axis_edges(&resolved.x, &mark.x_edges)),
        y: GridAxis::new(&maps.y, &axis_edges(&resolved.y, &mark.y_edges)),
        color: move |column: usize, row: usize| {
            colors.get(row * columns + column).copied().flatten()
        },
    }
}

/// The grid of the bins of `mark` on `maps`, its non-empty bins colored through `color`.
fn hist2d_grid(
    maps: &AxisMaps,
    resolved: &Resolved,
    mark: &Hist2dMark,
    color: &ResolvedColorScale,
) -> Grid<impl Fn(usize, usize) -> Option<Rgba> + use<>> {
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
    Grid {
        x: GridAxis::new(&maps.x, &edges(&resolved.x, &bins.x_edges)),
        y: GridAxis::new(&maps.y, &edges(&resolved.y, &bins.y_edges)),
        color: move |column: usize, row: usize| colors.get(&(column, row)).copied(),
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
    let (columns, rows) = (mark.x_edges.len() - 1, mark.y_edges.len() - 1);
    let grid = mesh_grid(&context.maps, resolved, mark, color);
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
            Raster::Unplaceable => {}
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
    let grid = hist2d_grid(&context.maps, resolved, mark, color);
    let count = bins.x_edges.len().saturating_sub(1) * bins.y_edges.len().saturating_sub(1);
    if mark.render.draws_image(count, true) {
        match grid.image() {
            Raster::Image(item) => {
                items.push(item);
                return;
            }
            Raster::Nothing => return,
            Raster::Unplaceable => {}
        }
    }
    let mut cells = bins.cells.clone();
    cells.sort_unstable_by_key(|&(column, row)| (Reverse(row), column));
    for (column, row) in cells {
        grid.push(column, row, items);
    }
}

/// Why a grid cannot be drawn as one image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Unplaceable {
    /// A cell cut by explicit limits reaches this many points past them, more than
    /// [`MAX_IMAGE_REACH`].
    Reach(f64),
    /// The edge `index` (counted from the first edge of the grid) lies `offset` points from its
    /// place in the image, more than [`MAX_EDGE_OFFSET`].
    Edge {
        /// The edge, counted from the first edge of the grid.
        index: usize,
        /// How far it lies from its place in the image, in points.
        offset: f64,
    },
}

/// A mesh or a 2-D histogram among `marks` that draws something, whose options say
/// [`Render::Raster`](crate::Render::Raster), but whose cells cannot be drawn as one image on
/// `maps` ([`Unplaceable`]): the first such mark, as its index among `marks`, what it is
/// (`"a mesh"`, `"a 2-D histogram"`), the axis (`"x"` or `"y"`) and why.
/// [`Render::Auto`](crate::Render::Auto) draws such marks as vector shapes instead.
pub(super) fn unplaceable_image(
    maps: &AxisMaps,
    resolved: &Resolved,
    marks: &[ResolvedMark<'_>],
) -> Option<(usize, &'static str, &'static str, Unplaceable)> {
    // Without a color scale no cell is drawn.
    let color = resolved.color.as_ref()?;
    marks.iter().enumerate().find_map(|(i, mark)| {
        let (kind, found) = match mark {
            ResolvedMark::Mesh { mark, .. } if mark.render == crate::Render::Raster => (
                "a mesh",
                mesh_grid(maps, resolved, mark, color).unplaceable(),
            ),
            ResolvedMark::Hist2d { mark, .. }
                if mark.render == crate::Render::Raster && !mark.bins.x_edges.is_empty() =>
            {
                (
                    "a 2-D histogram",
                    hist2d_grid(maps, resolved, mark, color).unplaceable(),
                )
            }
            _ => return None,
        };
        let (axis, why) = found?;
        Some((i, kind, axis, why))
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
    /// shown part, so both are the shown part. `Err` when an outer edge lies more than
    /// [`MAX_IMAGE_REACH`] points beyond the shown part, or an edge between the cells more than
    /// [`MAX_EDGE_OFFSET`] points from where the image's evenly spaced pixels put it.
    fn image_span(
        &self,
        cells: &std::ops::Range<usize>,
    ) -> Result<([f64; 2], [f64; 2]), Unplaceable> {
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
        if reach.is_nan() || reach > MAX_IMAGE_REACH {
            return Err(Unplaceable::Reach(reach));
        }
        // Each edge on the final plot, against its place among the image's pixels.
        let size = (whole[1] - whole[0]) / cells.len() as f64;
        for index in cells.start + 1..cells.end {
            let place = whole[0] + size * (index - cells.start) as f64;
            let offset = (self.unclipped[index] - place).abs();
            if offset.is_nan() || offset > MAX_EDGE_OFFSET {
                return Err(Unplaceable::Edge { index, offset });
            }
        }
        Ok((whole, shown))
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
    /// The image cannot place the cells exactly ([`Unplaceable`]); the grid is drawn as vector
    /// shapes.
    Unplaceable,
}

impl<F: Fn(usize, usize) -> Option<Rgba>> Grid<F> {
    /// The axis, `"x"` or `"y"`, along which the shown cells cannot be drawn as one image and
    /// why, when at least one shown cell is drawn; `None` when they can, or nothing is drawn.
    fn unplaceable(&self) -> Option<(&'static str, Unplaceable)> {
        let (columns, rows) = (self.x.shown_cells()?, self.y.shown_cells()?);
        let found = match (self.x.image_span(&columns), self.y.image_span(&rows)) {
            (Err(why), _) => ("x", why),
            (Ok(_), Err(why)) => ("y", why),
            (Ok(_), Ok(_)) => return None,
        };
        let drawn = rows.clone().any(|row| {
            columns
                .clone()
                .any(|column| (self.color)(column, row).is_some())
        });
        drawn.then_some(found)
    }

    /// The cells left after clipping as one image, one pixel per cell (transparent for an
    /// empty cell), over the whole extent of those cells and clipped to their shown part.
    fn image(&self) -> Raster {
        let (Some(columns), Some(rows)) = (self.x.shown_cells(), self.y.shown_cells()) else {
            return Raster::Nothing;
        };
        let (x_whole, x_shown) = match self.x.image_span(&columns) {
            Ok(span) => span,
            Err(_) => return Raster::Unplaceable,
        };
        let (y_whole, y_shown) = match self.y.image_span(&rows) {
            Ok(span) => span,
            Err(_) => return Raster::Unplaceable,
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
