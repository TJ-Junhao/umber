//! Contour geometry on a rectilinear grid of values: the lines where the field crosses a level
//! ([`iso_lines`], by marching squares) and the regions of the field at or above each level
//! ([`bands()`]), both in grid index space; and where a level label sits on a drawn line
//! ([`labels`]).
//!
//! A grid holds `columns × rows` values, row by row: the value of row `j`, column `i` sits at
//! the point `(i, j)` of index space. A cell is the unit square between four neighbouring
//! points. Only a cell whose four values are finite takes part, so a NaN value cuts the cells
//! around it out of every line and region: a hole.
//!
//! Along each side of a cell the field is linear between the values at its ends, and a point
//! is *high* when its value is at or above the level. A level crosses a side whose ends differ
//! (one high, one not), at the linearly interpolated point. The crossing is computed from the
//! side alone, its ends taken in index order, so the two cells that share a side agree on it
//! bit for bit. Inside a cell the line runs straight from crossing to crossing, with the high
//! corners on its left. A cell whose corners alternate high and low around it (a saddle) has
//! four crossings; the mean of its four values decides how they pair: at or above the level the
//! high corners are joined across the centre, below it they are cut apart. So every result is a
//! function of the values alone, and the same input always gives the same geometry, point for
//! point and in the same order.

mod bands;
pub(crate) mod labels;
mod march;

pub(crate) use bands::{BandPolygons, bands};
pub(crate) use march::{Polyline, iso_lines};

use crate::{ContourMark, ContourStyle};

/// The geometry of one contour in grid index space: the lines along each of its levels, when
/// it is drawn with lines, and its bands, when it is filled.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ContourShapes {
    /// The lines of each level, in the order of the levels; none for a contour without lines.
    pub lines: Vec<Vec<Polyline>>,
    /// The bands, one more than the levels ([`bands()`]); none for a contour without fill.
    pub bands: Vec<BandPolygons>,
}

impl ContourShapes {
    /// The lines and bands that the style of `mark` draws.
    pub(crate) fn of(mark: &ContourMark) -> ContourShapes {
        let levels = mark.levels();
        let (lines, filled) = match mark.options().style {
            ContourStyle::Lines => (true, false),
            ContourStyle::Filled => (false, true),
            ContourStyle::Both => (true, true),
        };
        // The model checked the shape of the grid.
        let Some(grid) = Grid::new(mark.z(), mark.x().len(), mark.y().len()) else {
            return ContourShapes {
                lines: Vec::new(),
                bands: Vec::new(),
            };
        };
        ContourShapes {
            lines: if lines {
                levels
                    .iter()
                    .map(|&level| iso_lines(&grid, level))
                    .collect()
            } else {
                Vec::new()
            },
            bands: if filled {
                bands(&grid, &levels)
            } else {
                Vec::new()
            },
        }
    }
}

/// A point of grid index space: `x` counts columns and `y` rows from the grid's first value, so
/// the value of column `i`, row `j` sits at `(i, j)` and a cell spans one unit along each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GridPoint {
    /// The column position.
    pub x: f64,
    /// The row position.
    pub y: f64,
}

/// The values of a rectilinear grid, row by row, with at least two rows and two columns.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Grid<'a> {
    /// The values, `columns` per row.
    values: &'a [f64],
    /// The number of values in each row.
    columns: usize,
    /// The number of rows.
    rows: usize,
}

/// A side of a cell, between two neighbouring grid points: `Horizontal(i, j)` joins `(i, j)` to
/// `(i + 1, j)`, `Vertical(i, j)` joins `(i, j)` to `(i, j + 1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Side {
    /// From `(i, j)` to `(i + 1, j)`.
    Horizontal(usize, usize),
    /// From `(i, j)` to `(i, j + 1)`.
    Vertical(usize, usize),
}

impl<'a> Grid<'a> {
    /// The grid of `values`, `columns` per row: `None` unless there are at least two columns
    /// and two rows and `values` holds exactly `columns * rows` of them.
    pub(crate) fn new(values: &'a [f64], columns: usize, rows: usize) -> Option<Grid<'a>> {
        (columns >= 2 && rows >= 2 && columns.checked_mul(rows) == Some(values.len())).then_some(
            Grid {
                values,
                columns,
                rows,
            },
        )
    }

    /// The value at the point `(i, j)`; NaN outside the grid.
    fn value(&self, i: usize, j: usize) -> f64 {
        if i >= self.columns {
            return f64::NAN;
        }
        j.checked_mul(self.columns)
            .and_then(|start| self.values.get(start + i))
            .copied()
            .unwrap_or(f64::NAN)
    }

    /// The cells in row-major order: `(i, j)` for the cell whose lower left corner is `(i, j)`.
    fn cells(&self) -> impl Iterator<Item = (usize, usize)> + use<> {
        let (columns, rows) = (self.columns - 1, self.rows - 1);
        (0..rows).flat_map(move |j| (0..columns).map(move |i| (i, j)))
    }

    /// The corners of the cell `(i, j)` counter-clockwise from its lower left one: `(i, j)`,
    /// `(i + 1, j)`, `(i + 1, j + 1)`, `(i, j + 1)`.
    fn corners(i: usize, j: usize) -> [(usize, usize); 4] {
        [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)]
    }

    /// The values at the corners of the cell `(i, j)`, in the order of [`Grid::corners`].
    fn corner_values(&self, i: usize, j: usize) -> [f64; 4] {
        Self::corners(i, j).map(|(x, y)| self.value(x, y))
    }

    /// Whether the cell `(i, j)` lies in the grid and its four values are finite.
    fn is_finite_cell(&self, i: usize, j: usize) -> bool {
        i + 1 < self.columns
            && j + 1 < self.rows
            && self.corner_values(i, j).iter().all(|v| v.is_finite())
    }

    /// The side `k` of the cell `(i, j)`, from its corner `k` to its corner `k + 1` (counting as
    /// [`Grid::corners`] does): the bottom, right, top and left side for `k` = 0, 1, 2, 3.
    fn side(i: usize, j: usize, k: usize) -> Side {
        match k % 4 {
            0 => Side::Horizontal(i, j),
            1 => Side::Vertical(i + 1, j),
            2 => Side::Horizontal(i, j + 1),
            _ => Side::Vertical(i, j),
        }
    }

    /// The cell across the side `k` of the cell `(i, j)`, when it is a finite cell.
    fn finite_neighbour(&self, i: usize, j: usize, k: usize) -> bool {
        let across = match k % 4 {
            0 => j.checked_sub(1).map(|below| (i, below)),
            1 => Some((i + 1, j)),
            2 => Some((i, j + 1)),
            _ => i.checked_sub(1).map(|left| (left, j)),
        };
        across.is_some_and(|(x, y)| self.is_finite_cell(x, y))
    }

    /// Where `level` crosses `side`, whose ends are one high and one not: interpolated linearly
    /// from the end with the lower index, so both cells that share the side get the same point.
    fn crossing(&self, side: Side, level: f64) -> GridPoint {
        let ((i, j), (x, y)) = match side {
            Side::Horizontal(i, j) => ((i, j), (i + 1, j)),
            Side::Vertical(i, j) => ((i, j), (i, j + 1)),
        };
        let (from, to) = (self.value(i, j), self.value(x, y));
        // The ends differ, one at or above the level and one below it, so `to - from` is not
        // zero; both are at most 1e300 in magnitude, so it is finite.
        let t = ((level - from) / (to - from)).clamp(0.0, 1.0);
        match side {
            Side::Horizontal(..) => GridPoint {
                x: i as f64 + t,
                y: j as f64,
            },
            Side::Vertical(..) => GridPoint {
                x: i as f64,
                y: j as f64 + t,
            },
        }
    }
}

/// The point `(i, j)` of index space.
fn grid_point((i, j): (usize, usize)) -> GridPoint {
    GridPoint {
        x: i as f64,
        y: j as f64,
    }
}

#[cfg(test)]
mod tests;
