//! Scatter marks: one batch of markers, culled in axis space by the marker's true extent.

use super::MarkContext;
use super::clip::Bounds;
use super::fill::marker_alpha;
use super::mapping::AxisPoint;
use super::marker::{extent, marker_shape};
use crate::geometry::Point;
use crate::scene::{Item, LineCap, LineJoin, StrokeStyle};
use crate::{Marker, Rgba};

/// The scatter outline width, in points.
pub(crate) const MARKER_OUTLINE: f32 = 0.8;
/// Scatter points whose marker (shape extent plus outline) lies further than this many points
/// outside the plot area are dropped.
const MARKER_SLACK: f64 = 1.0;

/// How far a marker of `shape` and nominal `diameter` reaches from its centre along x and along
/// y, outline included, in points: the shape's extent on each axis plus half the outline for a
/// circle, or the whole outline for a polygon, whose miter joins reach at most that far past a
/// vertex for these shapes (a triangle's 60° apex reaches exactly that far).
pub(crate) fn marker_reach(shape: Marker, diameter: f32) -> (f64, f64) {
    let (half_w, half_h) = extent(&marker_shape(shape, diameter));
    let outline = f64::from(MARKER_OUTLINE);
    let join = match shape {
        Marker::Circle => outline / 2.0,
        Marker::Square | Marker::Triangle | Marker::Diamond | Marker::Cross => outline,
    };
    (half_w + join, half_h + join)
}

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
    Some(Item::Markers {
        shape: shape_path,
        fill: Some(color.scale_alpha(marker_alpha(theme))),
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
