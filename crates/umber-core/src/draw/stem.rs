//! Stems: one stroke from the baseline to every value, then the same marker batch as scatter.
//!
//! Points are not reduced: each is a separate datum, as with scatter and error bars. Stems are
//! clipped in axis space before mapping, and markers use scatter's extent-aware culling.

use super::MarkContext;
use super::clip::{Bounds, clip_segment};
use super::error_bars::bar_style;
use super::mapping::{AxisPoint, axis_values};
use super::scatter::scatter_item;
use crate::Rgba;
use crate::geometry::Path;
use crate::model::StemMark;
use crate::resolve::Resolved;
use crate::scene::Item;

/// Adds the visible stems in `color`, at the theme's stem width, then their scatter markers of
/// diameter `size`. A NaN in either coordinate skips the point. A stem exactly at an x limit
/// keeps its inside half; stems crossing a y limit stop there, while their markers are culled
/// separately. A zero-length stem adds only its marker.
pub(super) fn stem_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &StemMark,
    (color, size): (Rgba, f32),
    items: &mut Vec<Item>,
) {
    let xs = axis_values(&resolved.x, mark.x());
    let ys = axis_values(&resolved.y, mark.y());
    let width = context.theme.stem_width();
    let (left, right) = context.maps.x.expanded(f64::from(width) / 2.0);
    let bounds = Bounds {
        left,
        right,
        top: context.maps.y.lo,
        bottom: context.maps.y.hi,
    };
    let baseline = context.maps.y.at(context.baseline.pos);
    let mut path = Path::new();
    for (&x, &y) in xs.iter().zip(&ys) {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        if let Some((start, end)) =
            clip_segment(AxisPoint::new(x, baseline), AxisPoint::new(x, y), &bounds)
            && start != end
        {
            path.move_to(context.maps.to_plot(start).to_scene());
            path.line_to(context.maps.to_plot(end).to_scene());
        }
    }
    if !path.is_empty() {
        items.push(Item::Stroke {
            path,
            style: bar_style(width, color),
        });
    }
    items.extend(scatter_item(
        context,
        &xs,
        &ys,
        color,
        mark.options().marker,
        size,
    ));
}
