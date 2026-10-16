//! Pointy-top hexagonal cells, clipped in axis space before projection.

use super::MarkContext;
use super::clip::crossing;
use super::mapping::{AxisPoint, axis_value};
use crate::Item;
use crate::bin2d::HexBins;
use crate::color_scale::ResolvedColorScale;
use crate::resolve::Resolved;

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
    for (&center, &count) in bins.centers.iter().zip(&bins.counts) {
        let Some(fill) = color.color(count) else {
            continue;
        };
        let vertices = bins
            .vertices(center)
            .map(|[x, y]| AxisPoint::new(axis_value(&resolved.x, x), axis_value(&resolved.y, y)));
        let vertices = clip(context, vertices.to_vec());
        super::mesh::fill(context, &vertices, fill, items);
    }
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
