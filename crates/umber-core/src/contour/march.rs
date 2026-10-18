//! Iso lines by marching squares: the segments of each finite cell, joined across shared sides
//! into polylines.

use std::collections::HashMap;

use super::{Grid, GridPoint, Side};

/// One line where the field crosses a level: crossings in order along the line, which keeps the
/// high side of the field on its left (counter-clockwise around the high regions, in index
/// space with `y` up). A closed line, one that returns to where it starts, repeats its first
/// point at the end; an open line ends on the edge of the grid or of a hole.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Polyline {
    /// The points, at least two.
    pub points: Vec<GridPoint>,
    /// Whether the line returns to its first point.
    pub closed: bool,
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

/// The lines where the field of `grid` crosses `level`, through its finite cells: the segments
/// of the cells joined where they share a side, each line oriented with the high side on its
/// left. Open lines come first, in the order of their first segment (row-major by cell), then
/// closed ones in the same order, each starting at its first segment in that order.
pub(crate) fn iso_lines(grid: &Grid<'_>, level: f64) -> Vec<Polyline> {
    lines_of(grid, level, &segments(grid, level))
}

/// The lines of `grid` at `level`, as [`iso_lines`] gives them, from its `segments`.
pub(super) fn lines_of(grid: &Grid<'_>, level: f64, segments: &[Segment]) -> Vec<Polyline> {
    // The segment that starts on each side, and whether one ends there. Each crossing is the
    // exit of at most one cell and the entry of at most one other.
    let starting: HashMap<Side, usize> = segments
        .iter()
        .enumerate()
        .map(|(index, segment)| (segment.from, index))
        .collect();
    let ending: HashMap<Side, usize> = segments
        .iter()
        .enumerate()
        .map(|(index, segment)| (segment.to, index))
        .collect();
    let mut used = vec![false; segments.len()];
    let mut lines = Vec::new();
    let follow = |first: usize, used: &mut [bool]| {
        let mut points = Vec::new();
        let mut current = first;
        if let Some(segment) = segments.get(first) {
            points.push(grid.crossing(segment.from, level));
        }
        while let Some(&Segment { to, .. }) = segments.get(current) {
            if let Some(flag) = used.get_mut(current) {
                *flag = true;
            }
            points.push(grid.crossing(to, level));
            match starting.get(&to) {
                Some(&next) if !used.get(next).copied().unwrap_or(true) => current = next,
                Some(&next) => return (points, next == first),
                None => return (points, false),
            }
        }
        (points, false)
    };
    // Open lines start at a segment that no other segment leads into.
    for first in 0..segments.len() {
        let opens = segments
            .get(first)
            .is_some_and(|segment| !ending.contains_key(&segment.from));
        if opens && used.get(first) == Some(&false) {
            let (points, _) = follow(first, &mut used);
            lines.push(Polyline {
                points,
                closed: false,
            });
        }
    }
    for first in 0..segments.len() {
        if used.get(first) == Some(&false) {
            let (points, closed) = follow(first, &mut used);
            lines.push(Polyline { points, closed });
        }
    }
    lines
}
