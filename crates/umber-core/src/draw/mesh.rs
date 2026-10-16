//! Opaque, edge-spanning numeric cells. Geometry is clipped before conversion to scene points.

use super::MarkContext;
use super::mapping::{AxisPoint, axis_value, point};
use crate::color_scale::ResolvedColorScale;
use crate::model::{Hist2dMark, MeshMark};
use crate::resolve::Resolved;
use crate::{FillRule, Item, Paint, Path, Rgba};

/// Draws finite row-major values, leaving NaN cells empty.
pub(super) fn mesh_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &MeshMark,
    color: &ResolvedColorScale,
    items: &mut Vec<Item>,
) {
    let columns = mark.x_edges.len() - 1;
    for (i, &value) in mark.colors.values.iter().enumerate() {
        if let Some(fill) = color.color(value) {
            rectangle(
                context,
                resolved,
                [mark.x_edges[i % columns], mark.x_edges[i % columns + 1]],
                [mark.y_edges[i / columns], mark.y_edges[i / columns + 1]],
                fill,
                items,
            );
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
    for (&(col, row), &count) in mark.bins.cells.iter().zip(&mark.bins.counts) {
        if let Some(fill) = color.color(count) {
            rectangle(
                context,
                resolved,
                [mark.bins.x_edges[col], mark.bins.x_edges[col + 1]],
                [mark.bins.y_edges[row], mark.bins.y_edges[row + 1]],
                fill,
                items,
            );
        }
    }
}

/// Clips an axis-aligned cell exactly, keeping every finite projected coordinate bounded.
fn rectangle(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    x: [f64; 2],
    y: [f64; 2],
    color: Rgba,
    items: &mut Vec<Item>,
) {
    let x = x.map(|v| axis_value(&resolved.x, v).clamp(context.maps.x.lo, context.maps.x.hi));
    let y = y.map(|v| axis_value(&resolved.y, v).clamp(context.maps.y.lo, context.maps.y.hi));
    if x[0] >= x[1] || y[0] >= y[1] {
        return;
    }
    fill(
        context,
        &[
            AxisPoint::new(x[0], y[0]),
            AxisPoint::new(x[1], y[0]),
            AxisPoint::new(x[1], y[1]),
            AxisPoint::new(x[0], y[1]),
        ],
        color,
        items,
    );
}

/// Fills a clipped polygon, without an outline or an area-opacity multiplier.
pub(super) fn fill(
    context: &MarkContext<'_>,
    vertices: &[AxisPoint],
    color: Rgba,
    items: &mut Vec<Item>,
) {
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
