//! Scatter marks: one batch of markers, culled in axis space by the marker's true extent.

use super::MarkContext;
use super::clip::Bounds;
use super::mapping::AxisPoint;
use super::marker::{extent, marker_shape};
use crate::geometry::Point;
use crate::scene::{Item, LineCap, LineJoin, StrokeStyle};
use crate::{Marker, Rgba};

/// The scatter outline width, in points.
const MARKER_OUTLINE: f32 = 0.8;
/// Scatter points whose marker (shape extent plus outline) lies further than this many points
/// outside the plot area are dropped.
const MARKER_SLACK: f64 = 1.0;

/// The scene item of one scatter: one batch of `shape` markers of diameter `size` (the mark's
/// drawn size, see [`ResolvedMark::Scatter`](crate::resolve::ResolvedMark::Scatter)), filled
/// with `color` at the theme's marker alpha times the color's own, outlined 0.8 pt in the
/// background color. Points whose marker cannot reach the plot area are dropped.
pub(super) fn scatter_item(
    context: &MarkContext<'_>,
    xs: &[f64],
    ys: &[f64],
    color: Rgba,
    shape: Marker,
    size: f32,
) -> Option<Item> {
    let MarkContext { theme, maps, .. } = *context;
    let shape_path = marker_shape(shape, size);
    // Cull in axis space by the shape's own extent on each axis, plus the outline (whose miter
    // joins reach at most its width beyond the path for these shapes) and the slack.
    let (half_w, half_h) = extent(&shape_path);
    let reach = f64::from(MARKER_OUTLINE) + MARKER_SLACK;
    let ((left, right), (top, bottom)) = (
        maps.x.expanded(half_w + reach),
        maps.y.expanded(half_h + reach),
    );
    let bounds = Bounds {
        left,
        top,
        right,
        bottom,
    };
    let positions: Vec<Point> = xs
        .iter()
        .zip(ys)
        .map(|(&x, &y)| AxisPoint::new(x, y))
        .filter(|&p| bounds.contains(p))
        .map(|p| maps.to_plot(p).to_scene())
        .collect();
    if positions.is_empty() {
        return None;
    }
    let alpha = theme.marker_alpha();
    let alpha = if alpha.is_finite() {
        alpha.clamp(0.0, 1.0)
    } else {
        1.0
    };
    Some(Item::Markers {
        shape: shape_path,
        fill: Some(color.scale_alpha(alpha)),
        stroke: Some(StrokeStyle {
            color: theme.background(),
            width: MARKER_OUTLINE,
            dash: Vec::new(),
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }),
        positions,
    })
}
