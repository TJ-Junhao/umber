//! Opaque rectangular cells that meet without a seam, shared by heatmaps, meshes and 2-D
//! histograms.
//!
//! Two antialiased fills that only touch along an edge both cover the pixels on that edge
//! partly, so the background shows through between them. Cells are therefore drawn in rows from
//! the top of the plot down, each row from left to right, and a cell reaches up to [`OVERLAP`]
//! under its neighbours to the right and below, which are drawn after it and cover the reach
//! with their own edges. A cell reaches only under a drawn neighbour, never past the outer
//! edges of its grid or into an empty cell, so every edge that faces the background stays
//! exactly where it is. Where a cell reaches both right and down but the cell diagonally below
//! it to the right is empty, the corner where the two reaches meet is cut out of it.

use super::mapping::point;
use crate::geometry::Path;

/// How far, in points, a cell reaches under a neighbour drawn after it; at most half the
/// neighbour's size along that axis.
pub(super) const OVERLAP: f64 = 1.0;

/// How far a cell reaches under the neighbour drawn after it across one edge, whose size
/// across that edge is `size` points: [`OVERLAP`], or half the neighbour when it is smaller.
pub(super) fn reach(size: f64) -> f64 {
    OVERLAP.min(size.abs() / 2.0)
}

/// How far a rectangular cell reaches under the neighbours drawn after it, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Reach {
    /// How far it reaches right, under the next cell in its row: 0 without one.
    pub(super) right: f64,
    /// How far it reaches down, under the next cell in its column: 0 without one.
    pub(super) down: f64,
    /// Whether the cell diagonally below it to the right is drawn, so that the corner where
    /// both reaches meet lies under a cell too.
    pub(super) corner: bool,
}

/// The outline of the cell whose left, top, right and bottom edges are `bounds`, in plot
/// points, widened by `reach` to the right and down. When it reaches both ways but no cell lies
/// diagonally below it, the corner where the two reaches meet is cut out, so nothing of the
/// cell shows over the empty cell there.
pub(super) fn cell_path([left, top, right, bottom]: [f64; 4], reach: Reach) -> Path {
    let (far_right, far_bottom) = (right + reach.right, bottom + reach.down);
    let mut path = Path::new();
    path.move_to(point(left, top));
    path.line_to(point(far_right, top));
    if reach.right > 0.0 && reach.down > 0.0 && !reach.corner {
        path.line_to(point(far_right, bottom));
        path.line_to(point(right, bottom));
        path.line_to(point(right, far_bottom));
    } else {
        path.line_to(point(far_right, far_bottom));
    }
    path.line_to(point(left, far_bottom));
    path.close();
    path
}
