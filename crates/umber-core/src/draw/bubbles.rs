//! Bubbles in data order, with the scatter fill, outline and culling rules.

use super::MarkContext;
use super::mapping::axis_value;
use super::scatter::scatter_item;
use crate::model::BubbleMark;
use crate::resolve::Resolved;
use crate::{FillRule, Item, Paint, Rgba};

/// Appends each bubble's fill and outline together. Individual paths also let inside-legend
/// placement measure the visible area of a bubble, rather than only its centre.
pub(super) fn bubble_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &BubbleMark,
    (color, maximum, radius): (Rgba, f64, f32),
    items: &mut Vec<Item>,
) {
    for ((&x, &y), &value) in mark.x().iter().zip(mark.y()).zip(mark.sizes()) {
        if value == 0.0 {
            continue;
        }
        let diameter = 2.0 * crate::bubble_scale::radius(value, maximum, radius);
        if diameter == 0.0 {
            continue;
        }
        let Some(Item::Markers {
            shape,
            fill,
            stroke,
            positions,
        }) = scatter_item(
            context,
            &[axis_value(&resolved.x, x)],
            &[axis_value(&resolved.y, y)],
            color,
            mark.options().marker,
            diameter,
        )
        else {
            continue;
        };
        for position in positions {
            let path = shape.translated(position.x, position.y);
            if let Some(fill) = fill {
                items.push(Item::Fill {
                    path: path.clone(),
                    paint: Paint::Solid(fill),
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
