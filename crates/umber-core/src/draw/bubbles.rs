//! Bubbles in data order, with the scatter fill, outline and culling rules.

use super::MarkContext;
use super::mapping::axis_value;
use super::scatter::{PointFill, markers_item};
use crate::model::BubbleMark;
use crate::resolve::Resolved;
use crate::{FillRule, Item, Paint};

/// Appends each bubble's fill and outline together, each bubble filled as `fill` says (a mapped
/// color opaque, see [`PointFill::at`]); a bubble whose value maps to no color is not drawn.
/// Individual paths also let inside-legend placement measure the visible area of a bubble,
/// rather than only its centre.
pub(super) fn bubble_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &BubbleMark,
    (fill, maximum, radius): (PointFill<'_>, f64, f32),
    items: &mut Vec<Item>,
) {
    let points = mark.x().iter().zip(mark.y()).zip(mark.sizes()).enumerate();
    for (index, ((&x, &y), &value)) in points {
        if value == 0.0 {
            continue;
        }
        let Some(color) = fill.at(context.theme, index) else {
            continue;
        };
        let diameter = 2.0 * crate::bubble_scale::radius(value, maximum, radius);
        if diameter == 0.0 {
            continue;
        }
        let Some(Item::Markers {
            shape,
            fill: marker_fill,
            stroke,
            positions,
        }) = markers_item(
            context,
            (&[axis_value(&resolved.x, x)], &[axis_value(&resolved.y, y)]),
            color,
            mark.options().marker,
            diameter,
        )
        else {
            continue;
        };
        for position in positions {
            let path = shape.translated(position.x, position.y);
            if let Some(marker_fill) = marker_fill {
                items.push(Item::Fill {
                    path: path.clone(),
                    paint: Paint::Solid(marker_fill),
                    rule: FillRule::NonZero,
                });
            }
            if let Some(style) = &stroke {
                items.push(Item::Stroke {
                    path,
                    style: style.clone(),
                });
            }
        }
    }
}
