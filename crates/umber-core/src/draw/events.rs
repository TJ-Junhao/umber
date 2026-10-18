//! Short vertical strokes at event positions in category rows.

use super::MarkContext;
use super::clip::{Bounds, clip_segment};
use super::error_bars::bar_style;
use super::mapping::{AxisPoint, axis_values};
use crate::Rgba;
use crate::geometry::Path;
use crate::model::EventsMark;
use crate::resolve::{AxisScale, Resolved};
use crate::scene::Item;

/// Draws every finite event once, clipping in axis space before narrowing coordinates.
pub(super) fn items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &EventsMark,
    (color, width): (Rgba, f32),
    items: &mut Vec<Item>,
) {
    let AxisScale::Category(rows) = &resolved.y.scale else {
        return;
    };
    let ((left, right), (top, bottom)) = (
        context.maps.x.expanded(f64::from(width)),
        context.maps.y.expanded(f64::from(width)),
    );
    let bounds = Bounds {
        left,
        right,
        top,
        bottom,
    };
    let mut path = Path::new();
    for (name, positions) in mark.rows().iter().zip(mark.positions()) {
        let Some(index) = rows.index_of(name) else {
            continue;
        };
        let (start, end) = rows.band(index);
        let center = (start + end) / 2.0;
        let half = (end - start) * mark.options().length / 2.0;
        for x in axis_values(&resolved.x, positions) {
            if !x.is_finite() {
                continue;
            }
            if let Some((a, b)) = clip_segment(
                AxisPoint::new(x, center - half),
                AxisPoint::new(x, center + half),
                &bounds,
            ) {
                path.move_to(context.maps.to_plot(a).to_scene());
                path.line_to(context.maps.to_plot(b).to_scene());
            }
        }
    }
    if !path.verbs().is_empty() {
        items.push(Item::Stroke {
            path,
            style: bar_style(width, color),
        });
    }
}
