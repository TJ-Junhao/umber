//! Contours on a rectilinear grid of values: the grid in index space, its iso lines by marching
//! squares, and the oriented segments that its bands ([`super::bands`]) are made from.
//! Everything here is specific to the grid.
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

use super::join;
use super::{ContourField, Polyline, Traced, bands};
use crate::ContourStyle;

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
pub(super) enum Side {
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
    pub(super) fn value(&self, i: usize, j: usize) -> f64 {
        if i >= self.columns {
            return f64::NAN;
        }
        j.checked_mul(self.columns)
            .and_then(|start| self.values.get(start + i))
            .copied()
            .unwrap_or(f64::NAN)
    }

    /// The cells in row-major order: `(i, j)` for the cell whose lower left corner is `(i, j)`.
    pub(super) fn cells(&self) -> impl Iterator<Item = (usize, usize)> + use<> {
        let (columns, rows) = (self.columns - 1, self.rows - 1);
        (0..rows).flat_map(move |j| (0..columns).map(move |i| (i, j)))
    }

    /// The corners of the cell `(i, j)` counter-clockwise from its lower left one: `(i, j)`,
    /// `(i + 1, j)`, `(i + 1, j + 1)`, `(i, j + 1)`.
    pub(super) fn corners(i: usize, j: usize) -> [(usize, usize); 4] {
        [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)]
    }

    /// The values at the corners of the cell `(i, j)`, in the order of [`Grid::corners`].
    pub(super) fn corner_values(&self, i: usize, j: usize) -> [f64; 4] {
        Self::corners(i, j).map(|(x, y)| self.value(x, y))
    }

    /// Whether the cell `(i, j)` lies in the grid and its four values are finite.
    pub(super) fn is_finite_cell(&self, i: usize, j: usize) -> bool {
        i + 1 < self.columns
            && j + 1 < self.rows
            && self.corner_values(i, j).iter().all(|v| v.is_finite())
    }

    /// The side `k` of the cell `(i, j)`, from its corner `k` to its corner `k + 1` (counting as
    /// [`Grid::corners`] does): the bottom, right, top and left side for `k` = 0, 1, 2, 3.
    pub(super) fn side(i: usize, j: usize, k: usize) -> Side {
        match k % 4 {
            0 => Side::Horizontal(i, j),
            1 => Side::Vertical(i + 1, j),
            2 => Side::Horizontal(i, j + 1),
            _ => Side::Vertical(i, j),
        }
    }

    /// The cell across the side `k` of the cell `(i, j)`, when it is a finite cell.
    pub(super) fn finite_neighbour(&self, i: usize, j: usize, k: usize) -> bool {
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
    pub(super) fn crossing(&self, side: Side, level: f64) -> GridPoint {
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

/// The grid traced as [`ContourField`] says, each level marched once for its lines and its
/// region alike.
impl ContourField for Grid<'_> {
    type Point = GridPoint;

    fn whole(&self) -> Vec<Vec<GridPoint>> {
        bands::region(self, None)
    }

    fn trace(&self, level: f64, style: ContourStyle) -> Traced<GridPoint> {
        let segments = segments(self, level);
        Traced {
            lines: match style {
                ContourStyle::Lines | ContourStyle::Both => lines_of(self, level, &segments),
                ContourStyle::Filled => Vec::new(),
            },
            rings: match style {
                ContourStyle::Filled | ContourStyle::Both => {
                    bands::region(self, Some((level, &segments)))
                }
                ContourStyle::Lines => Vec::new(),
            },
        }
    }
}

/// The point `(i, j)` of index space.
pub(super) fn grid_point((i, j): (usize, usize)) -> GridPoint {
    GridPoint {
        x: i as f64,
        y: j as f64,
    }
}

/// The oriented segments of one cell at a level, each from the side where the line leaves the
/// high corners behind it to the side where it reaches them again, as side indices of the cell
/// (see [`Grid::side`]): none, one, or two in a saddle.
#[derive(Debug, Clone, Copy)]
pub(super) struct CellSegments {
    /// The segments, the first `count` of them used.
    pairs: [(usize, usize); 2],
    /// How many there are.
    count: usize,
}

impl CellSegments {
    /// The segments, in order.
    pub(super) fn iter(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.pairs.iter().take(self.count).copied()
    }
}

/// The oriented segments of a cell whose corners hold `corners` at `level`.
pub(super) fn cell_segments(corners: [f64; 4], level: f64) -> CellSegments {
    let high = corners.map(|v| v >= level);
    // A side is an exit when its first corner is high and its second is not, and an entry the
    // other way round.
    let (mut exits, mut entries) = ([0; 4], [0; 4]);
    let (mut exit_count, mut entry_count) = (0, 0);
    for k in 0..4 {
        match (high[k], high[(k + 1) % 4]) {
            (true, false) => {
                exits[exit_count] = k;
                exit_count += 1;
            }
            (false, true) => {
                entries[entry_count] = k;
                entry_count += 1;
            }
            _ => {}
        }
    }
    match (exit_count, entry_count) {
        (1, 1) => CellSegments {
            pairs: [(exits[0], entries[0]), (0, 0)],
            count: 1,
        },
        (2, 2) => {
            // A saddle: at or above the level the centre joins the high corners, and each exit
            // turns to the next side; below it, back to the previous one.
            let mean = corners.iter().sum::<f64>() / 4.0;
            let turn = if mean >= level { 1 } else { 3 };
            let [first, second] = [exits[0], exits[1]];
            CellSegments {
                pairs: [(first, (first + turn) % 4), (second, (second + turn) % 4)],
                count: 2,
            }
        }
        _ => CellSegments {
            pairs: [(0, 0); 2],
            count: 0,
        },
    }
}

/// One oriented segment of a cell at a level: the cell and the sides it joins.
#[derive(Debug, Clone, Copy)]
pub(super) struct Segment {
    /// The cell `(i, j)`.
    pub(super) cell: (usize, usize),
    /// The side where the line enters the cell.
    pub(super) from: Side,
    /// The side where it leaves.
    pub(super) to: Side,
}

/// The oriented segments of every finite cell of `grid` at `level`, in row-major cell order:
/// what both the lines and the bands of the level are made from.
pub(super) fn segments(grid: &Grid<'_>, level: f64) -> Vec<Segment> {
    let mut found = Vec::new();
    for (i, j) in grid.cells() {
        if !grid.is_finite_cell(i, j) {
            continue;
        }
        for (from, to) in cell_segments(grid.corner_values(i, j), level).iter() {
            found.push(Segment {
                cell: (i, j),
                from: Grid::side(i, j, from),
                to: Grid::side(i, j, to),
            });
        }
    }
    found
}

/// The lines where the field of `grid` crosses `level`, through its finite cells, from the
/// level's `segments`: the segments joined where they share a side ([`join::lines`]), each line
/// oriented with the high side on its left. Each crossing is the exit of at most one cell and
/// the entry of at most one other, so open lines come first, in the order of their first
/// segment (row-major by cell), then closed ones in the same order, each starting at its first
/// segment in that order.
fn lines_of(grid: &Grid<'_>, level: f64, segments: &[Segment]) -> Vec<Polyline<GridPoint>> {
    let pieces: Vec<(Side, Side)> = segments
        .iter()
        .map(|segment| (segment.from, segment.to))
        .collect();
    join::lines(&pieces)
        .into_iter()
        .map(|chain| {
            let start = chain
                .pieces
                .first()
                .and_then(|&first| pieces.get(first))
                .map(|&(from, _)| grid.crossing(from, level));
            let ends = chain
                .pieces
                .iter()
                .filter_map(|&index| pieces.get(index))
                .map(|&(_, to)| grid.crossing(to, level));
            Polyline {
                points: start.into_iter().chain(ends).collect(),
                closed: chain.closed,
            }
        })
        .collect()
}
