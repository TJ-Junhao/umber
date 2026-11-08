//! The filled regions of a contour on a grid: for each band, the boundary of the finite cells
//! where the field is at or above the band's lower level ([`super::BandPolygons`]).
//!
//! A region's boundary is made of the iso segments of its level inside the finite cells and of
//! the parts of their sides that face no finite cell (the edge of the grid or of a hole) and
//! lie at or above the level. Each piece keeps the region on its left, so the pieces join into
//! closed rings: counter-clockwise around the region and clockwise around its holes, in index
//! space with `y` up. A side shared by two finite cells is never part of a boundary.

use super::join;
use super::march::{Grid, GridPoint, Segment, Side, grid_point};

/// A corner of a region's boundary: a grid point, or the crossing of the level on a side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Vertex {
    /// The grid point `(i, j)`.
    Point(usize, usize),
    /// The crossing of the region's level on the side.
    Crossing(Side),
}

/// One piece of a region's boundary, from `from` (at `at`) to `to`.
#[derive(Debug, Clone, Copy)]
struct Edge {
    /// Where the piece starts.
    from: Vertex,
    /// Where it ends.
    to: Vertex,
    /// The position of `from`.
    at: GridPoint,
}

/// The boundary rings of the finite cells of `grid` where the field is at or above a level,
/// given with the oriented segments of the finite cells there ([`super::march::segments`]),
/// or of every finite cell for `None`.
pub(super) fn region(grid: &Grid<'_>, level: Option<(f64, &[Segment])>) -> Vec<Vec<GridPoint>> {
    let (level, segments) = match level {
        Some((level, segments)) => (Some(level), segments),
        None => (None, &[][..]),
    };
    let high = |(i, j): (usize, usize)| level.is_none_or(|level| grid.value(i, j) >= level);
    let position = |vertex: Vertex, level: f64| match vertex {
        Vertex::Point(i, j) => grid_point((i, j)),
        Vertex::Crossing(side) => grid.crossing(side, level),
    };
    let mut edges: Vec<Edge> = Vec::new();
    let mut push = |from: Vertex, to: Vertex| {
        edges.push(Edge {
            from,
            to,
            at: position(from, level.unwrap_or(f64::NAN)),
        });
    };
    // The segments come in row-major cell order, each cell's together.
    let mut next = segments.iter().peekable();
    for (i, j) in grid.cells() {
        if !grid.is_finite_cell(i, j) {
            continue;
        }
        while let Some(segment) = next.next_if(|segment| segment.cell == (i, j)) {
            push(Vertex::Crossing(segment.from), Vertex::Crossing(segment.to));
        }
        let corners = Grid::corners(i, j);
        for (k, (&a, &b)) in corners
            .iter()
            .zip(corners.iter().cycle().skip(1))
            .enumerate()
        {
            if grid.finite_neighbour(i, j, k) {
                continue;
            }
            let crossing = Vertex::Crossing(Grid::side(i, j, k));
            let (start, end) = (Vertex::Point(a.0, a.1), Vertex::Point(b.0, b.1));
            match (high(a), high(b)) {
                (true, true) => push(start, end),
                (true, false) => push(start, crossing),
                (false, true) => push(crossing, end),
                (false, false) => {}
            }
        }
    }
    rings(&edges)
}

/// The closed rings that `edges` form, each vertex having as many pieces leaving it as
/// reaching it ([`join::rings`]), each simplified ([`simplified`]); those left with fewer than
/// three points are dropped.
fn rings(edges: &[Edge]) -> Vec<Vec<GridPoint>> {
    let pieces: Vec<(Vertex, Vertex)> = edges.iter().map(|edge| (edge.from, edge.to)).collect();
    join::rings(&pieces)
        .into_iter()
        .map(|ring| {
            simplified(
                ring.iter()
                    .filter_map(|&index| edges.get(index))
                    .map(|edge| edge.at)
                    .collect(),
            )
        })
        .filter(|ring| ring.len() >= 3)
        .collect()
}

/// `ring` simplified without changing the region it bounds: first without repeated points
/// (distinct corners, such as a grid point and a crossing of a level equal to its value, can
/// lie at one position), then without the points that lie on a grid line between their two
/// neighbours, such as the grid points along the edge of the grid. A point is dropped only when
/// it lies on the segment that joins its neighbours, so no corner of the ring is lost.
fn simplified(ring: Vec<GridPoint>) -> Vec<GridPoint> {
    let mut distinct: Vec<GridPoint> = Vec::with_capacity(ring.len());
    for point in ring {
        if distinct.last() != Some(&point) {
            distinct.push(point);
        }
    }
    while distinct.len() > 1 && distinct.first() == distinct.last() {
        distinct.pop();
    }
    let mut kept: Vec<GridPoint> = Vec::with_capacity(distinct.len());
    for point in distinct {
        while let [.., before, last] = kept.as_slice()
            && between(*before, *last, point)
        {
            kept.pop();
        }
        kept.push(point);
    }
    // The ring closes on itself: its last and first points have neighbours across the start.
    loop {
        let n = kept.len();
        if n < 3 {
            break;
        }
        if between(kept[n - 2], kept[n - 1], kept[0]) {
            kept.pop();
        } else if between(kept[n - 1], kept[0], kept[1]) {
            kept.remove(0);
        } else {
            break;
        }
    }
    kept
}

/// Whether `point` lies on the segment from `a` to `b` along one grid line: all three share
/// their x or their y, and `point` is between the other two.
fn between(a: GridPoint, point: GridPoint, b: GridPoint) -> bool {
    let inside = |p: f64, a: f64, b: f64| a.min(b) <= p && p <= a.max(b);
    (a.x == point.x && point.x == b.x && inside(point.y, a.y, b.y))
        || (a.y == point.y && point.y == b.y && inside(point.x, a.x, b.x))
}
