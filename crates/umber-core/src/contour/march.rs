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

/// The oriented segments of the cell `(i, j)` at `level`, each from the side where the line
/// leaves the high corners behind it to the side where it reaches them again, as side indices
/// of the cell (see [`Grid::side`]): none, one, or two in a saddle.
pub(super) fn cell_segments(corners: [f64; 4], level: f64) -> Vec<(usize, usize)> {
    let high = corners.map(|v| v >= level);
    // A side is an exit when its first corner is high and its second is not, and an entry the
    // other way round.
    let exits: Vec<usize> = (0..4).filter(|&k| high[k] && !high[(k + 1) % 4]).collect();
    let entries: Vec<usize> = (0..4).filter(|&k| !high[k] && high[(k + 1) % 4]).collect();
    match (exits.as_slice(), entries.as_slice()) {
        ([exit], [entry]) => vec![(*exit, *entry)],
        ([first, second], [_, _]) => {
            // A saddle: at or above the level the centre joins the high corners, and each exit
            // turns to the next side; below it, back to the previous one.
            let mean = corners.iter().sum::<f64>() / 4.0;
            let turn = if mean >= level { 1 } else { 3 };
            vec![(*first, (first + turn) % 4), (*second, (second + turn) % 4)]
        }
        _ => Vec::new(),
    }
}

/// The oriented segments of every finite cell of `grid` at `level`, in row-major cell order,
/// as the sides they join.
pub(super) fn segments(grid: &Grid<'_>, level: f64) -> Vec<(Side, Side)> {
    grid.cells()
        .filter(|&(i, j)| grid.is_finite_cell(i, j))
        .flat_map(|(i, j)| {
            cell_segments(grid.corner_values(i, j), level)
                .into_iter()
                .map(move |(from, to)| (Grid::side(i, j, from), Grid::side(i, j, to)))
        })
        .collect()
}

/// The lines where the field of `grid` crosses `level`, through its finite cells: the segments
/// of the cells joined where they share a side, each line oriented with the high side on its
/// left. Open lines come first, in the order of their first segment (row-major by cell), then
/// closed ones in the same order, each starting at its first segment in that order.
pub(crate) fn iso_lines(grid: &Grid<'_>, level: f64) -> Vec<Polyline> {
    let segments = segments(grid, level);
    // The segment that starts on each side, and whether one ends there. Each crossing is the
    // exit of at most one cell and the entry of at most one other.
    let starting: HashMap<Side, usize> = segments
        .iter()
        .enumerate()
        .map(|(index, &(from, _))| (from, index))
        .collect();
    let ending: HashMap<Side, usize> = segments
        .iter()
        .enumerate()
        .map(|(index, &(_, to))| (to, index))
        .collect();
    let mut used = vec![false; segments.len()];
    let mut lines = Vec::new();
    let follow = |first: usize, used: &mut [bool]| {
        let mut points = Vec::new();
        let mut current = first;
        if let Some(&(from, _)) = segments.get(first) {
            points.push(grid.crossing(from, level));
        }
        while let Some(&(_, to)) = segments.get(current) {
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
            .is_some_and(|(from, _)| !ending.contains_key(from));
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
