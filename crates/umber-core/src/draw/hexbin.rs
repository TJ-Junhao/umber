//! Hexbins: one opaque pointy-top hexagon per non-empty cell, in the color its count maps to
//! on the axes' color scale, clipped to the axis domains in axis space before projection.
//!
//! The hexagons meet without a seam. Two antialiased fills that only touch along an edge both
//! cover the pixels on it partly, so the background would show between them; instead, each
//! edge a hexagon shares with a neighbour drawn after it is pushed out under that neighbour,
//! whose own edge then covers it. The cells are drawn in the order of their lattice positions,
//! row by row from the bottom, each row from left to right, so a hexagon pushes its edges
//! toward the cells to its right, upper right and upper left when they are drawn.
//!
//! All hexagons are translates of one centrally symmetric hexagon in axis space, and so in
//! plot points too. At each vertex of a hexagon a third edge leaves it, the edge between the two
//! neighbours that meet there; it is parallel to the hexagon's edge from the next vertex but
//! one to the next vertex. A pushed edge from vertex `a` to vertex `b` becomes the segment
//! between the points a fraction `t` of the way along the third edges at `a` and `b`. So the
//! pushed part is a strip of the neighbour along the shared edge, bounded at each end by the
//! neighbour's own edges, and it never reaches into a third cell, empty or not; where two
//! pushed edges meet, their ends are the same point, on the third edge between them. A hexagon
//! keeps its exact outline next to an empty cell and wherever the neighbour is drawn before
//! it. The fraction `t` makes the strip [`reach`] deep at most: one
//! [`OVERLAP`](super::cells::OVERLAP) or half the neighbour across that edge, the most a
//! heatmap cell reaches.

use super::MarkContext;
use super::cells::reach;
use super::clip::crossing;
use super::mapping::{AxisMaps, AxisPoint, axis_value, point};
use crate::bin2d::{HexBins, hex_neighbours};
use crate::color_scale::ResolvedColorScale;
use crate::resolve::Resolved;
use crate::{FillRule, Item, Paint, Path, Rgba};
use std::collections::HashMap;

/// Draws non-empty cells through the common axes color scale.
pub(super) fn hexbin_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    bins: &HexBins,
    color: &ResolvedColorScale,
    items: &mut Vec<Item>,
) {
    let HexBins::Occupied(bins) = bins else {
        return;
    };
    let colors: Vec<Option<Rgba>> = bins.counts.iter().map(|&n| color.color(n)).collect();
    // The drawing index of each drawn cell, by lattice position.
    let drawn: HashMap<[i64; 2], usize> = bins
        .positions
        .iter()
        .zip(&colors)
        .enumerate()
        .filter(|(_, (_, color))| color.is_some())
        .map(|(i, (&position, _))| (position, i))
        .collect();
    for (i, (&position, fill)) in bins.positions.iter().zip(&colors).enumerate() {
        let Some(fill) = *fill else {
            continue;
        };
        let vertices = bins
            .vertices(position)
            .map(|[x, y]| AxisPoint::new(axis_value(&resolved.x, x), axis_value(&resolved.y, y)));
        let pushed =
            hex_neighbours(position).map(|n| drawn.get(&n).is_some_and(|&later| later > i));
        let outline = pushed_outline(&vertices, pushed, share(context.maps, &vertices));
        polygon(context, &clip(context, outline), fill, items);
    }
}

/// The fraction of the third edges at its vertices (see the module documentation) by which the
/// hexagon `vertices`, in axis space, pushes its edges out: the least that keeps every edge
/// [`reach`] deep at most, in plot points; 0 for a hexagon without area on the plot.
fn share(maps: AxisMaps, vertices: &[AxisPoint; 6]) -> f64 {
    let plot = |a: AxisPoint, b: AxisPoint| (maps.x.points(b.x - a.x), maps.y.points(b.y - a.y));
    (0..6)
        .map(|i| {
            let (a, b, c) = (vertices[i], vertices[(i + 1) % 6], vertices[(i + 2) % 6]);
            let edge = plot(a, b);
            let third = plot(c, b);
            let length = edge.0.hypot(edge.1);
            // How far the far end of the third edge lies from the edge: half the neighbour
            // across it.
            let depth = (edge.0 * third.1 - edge.1 * third.0).abs() / length;
            if depth > 0.0 && depth.is_finite() {
                reach(2.0 * depth) / depth
            } else {
                0.0
            }
        })
        .fold(1.0, f64::min)
}

/// The outline of the hexagon `vertices`, each edge `i`, from vertex `i` to vertex `i + 1`,
/// pushed out by `share` of the third edges at its ends when `pushed[i]` (see the module
/// documentation).
fn pushed_outline(vertices: &[AxisPoint; 6], pushed: [bool; 6], share: f64) -> Vec<AxisPoint> {
    let mut outline = Vec::with_capacity(12);
    for i in 0..6 {
        let v = vertices[i];
        // The third edge at `v` is parallel to the edge from vertex i + 2 to vertex i + 1.
        let (to, from) = (vertices[(i + 1) % 6], vertices[(i + 2) % 6]);
        let reached = AxisPoint::new(v.x + share * (to.x - from.x), v.y + share * (to.y - from.y));
        match (pushed[(i + 5) % 6], pushed[i]) {
            (false, false) => outline.push(v),
            (true, false) => outline.extend([reached, v]),
            (false, true) => outline.extend([v, reached]),
            (true, true) => outline.push(reached),
        }
    }
    outline
}

/// Fills a clipped polygon, without an outline or an area-opacity multiplier.
fn polygon(context: &MarkContext<'_>, vertices: &[AxisPoint], color: Rgba, items: &mut Vec<Item>) {
    if vertices.len() < 3 {
        return;
    }
    let mut path = Path::new();
    for (i, &v) in vertices.iter().enumerate() {
        let p = context.maps.to_plot(v);
        let p = point(p.x, p.y);
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    path.close();
    items.push(Item::Fill {
        path,
        paint: Paint::Solid(color),
        rule: FillRule::NonZero,
    });
}

/// Sutherland–Hodgman clipping against the four plot edges, using the shared robust crossing
/// kernel so an explicit narrow domain is safe even beside very large coordinates.
fn clip(context: &MarkContext<'_>, mut polygon: Vec<AxisPoint>) -> Vec<AxisPoint> {
    let x = context.maps.x;
    let y = context.maps.y;
    for (vertical, edge, lower) in [
        (true, x.lo, true),
        (true, x.hi, false),
        (false, y.lo, true),
        (false, y.hi, false),
    ] {
        let value = |p: AxisPoint| if vertical { p.x } else { p.y };
        let inside = |p| {
            if lower {
                value(p) >= edge
            } else {
                value(p) <= edge
            }
        };
        let Some(&last) = polygon.last() else {
            break;
        };
        let mut previous = last;
        let mut clipped = Vec::with_capacity(polygon.len() + 1);
        for current in polygon {
            if inside(current) != inside(previous) {
                let v = if vertical {
                    crossing(
                        (previous.x, previous.y),
                        (current.x, current.y),
                        edge,
                        y.lo / 2.0 + y.hi / 2.0,
                    )
                    .map(|v| AxisPoint::new(edge, v))
                } else {
                    crossing(
                        (previous.y, previous.x),
                        (current.y, current.x),
                        edge,
                        x.lo / 2.0 + x.hi / 2.0,
                    )
                    .map(|v| AxisPoint::new(v, edge))
                };
                if let Some(v) = v {
                    clipped.push(v);
                }
            }
            if inside(current) {
                clipped.push(current);
            }
            previous = current;
        }
        polygon = clipped;
    }
    polygon
}
