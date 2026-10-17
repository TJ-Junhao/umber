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
//!
//! A grid drawn as one image instead ([`Render`](crate::Render)) needs none of this: its cells
//! are the pixels of one [`Item::Image`], with transparent pixels for empty cells, and the
//! renderers meet them without seams.

use super::mapping::point;
use crate::Rgba;
use crate::geometry::Path;
use crate::scene::{Item, Rect};

/// A transparent pixel: an empty cell of an image.
pub(super) const CLEAR: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 0,
};

/// How far, in points, the image of a grid may reach past the part of it that is shown. Cells
/// cut by explicit limits are drawn whole and clipped, so an image reaches past the limits by
/// at most one cell; a cell wider than this, which only limits inside a few cells give, is too
/// wide to place exactly in the scene's precision, and such a grid is drawn as vector shapes.
pub(super) const MAX_IMAGE_REACH: f64 = 10_000.0;

/// The grid of `width` by `height` cells whose colors are `pixels` (row by row from the top,
/// [`CLEAR`] for an empty cell) as one image over `bounds`, the left, top, right and bottom
/// edges of its outer cells in plot points. When explicit limits cut its outer cells, `visible`
/// is the part of `bounds` shown, and the image is clipped to it; otherwise `visible` equals
/// `bounds`. `None` when every cell is empty.
pub(super) fn image_item(
    bounds: [f64; 4],
    visible: [f64; 4],
    (width, height): (usize, usize),
    pixels: Vec<Rgba>,
) -> Option<Item> {
    if pixels.iter().all(|c| c.a == 0) {
        return None;
    }
    let rect = |[left, top, right, bottom]: [f64; 4]| {
        let (x0, x1) = (left.min(right), left.max(right));
        let (y0, y1) = (top.min(bottom), top.max(bottom));
        Rect {
            x: x0 as f32,
            y: y0 as f32,
            width: (x1 - x0) as f32,
            height: (y1 - y0) as f32,
        }
    };
    let image = Item::Image {
        rect: rect(bounds),
        width: u32::try_from(width).ok()?,
        height: u32::try_from(height).ok()?,
        pixels,
    };
    if visible == bounds {
        return Some(image);
    }
    Some(Item::Group {
        clip: Some(rect(visible)),
        items: vec![image],
    })
}

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
