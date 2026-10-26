//! Bubbles in data order, with the scatter fill, outline and culling rules.

use super::MarkContext;
use super::mapping::axis_shared;
use super::scatter::{MappedOutlines, MarkerGeometry, Outline, PointFill};
use crate::model::BubbleMark;
use crate::resolve::Resolved;
use crate::{FillRule, Item, Paint};

/// Appends each bubble's fill and outline together, each bubble filled as `fill` says (a mapped
/// color opaque, see [`PointFill::at`]); a bubble whose value maps to no color is not drawn.
/// Mapped fills use the scatter's contrasting 0.5 pt outline; uniform fills keep the 0.8 pt
/// background outline. Neutral legend swatches and size references are not color-mapped.
/// Individual paths also let inside-legend placement measure the visible area of a bubble,
/// rather than only its centre.
pub(super) fn bubble_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &BubbleMark,
    (fill, maximum, radius): (PointFill<'_>, f64, f32),
    items: &mut Vec<Item>,
) {
    let xs = axis_shared(&resolved.x, mark.x_values());
    let ys = axis_shared(&resolved.y, mark.y_values());
    let mut outlines = match fill {
        PointFill::Uniform(_) => None,
        PointFill::Mapped { .. } => Some(MappedOutlines::new(context.theme)),
    };
    let mut previous: Option<(f32, MarkerGeometry)> = None;
    let points = xs.iter().zip(&ys).zip(mark.sizes()).enumerate();
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
        let stroke = match &mut outlines {
            Some(outlines) => Outline::Contrasting(outlines.color(color)),
            None => Outline::Background,
        }
        .style(context.theme);
        if previous.as_ref().is_none_or(|(size, _)| *size != diameter) {
            previous = Some((
                diameter,
                MarkerGeometry::new(context, mark.options().marker, diameter, stroke.width),
            ));
        }
        let Some((_, geometry)) = &previous else {
            continue;
        };
        let Some(position) = geometry.position(context, x, y) else {
            continue;
        };
        let path = geometry.shape.translated(position.x, position.y);
        items.push(Item::Fill {
            path: path.clone(),
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        });
        items.push(Item::Stroke {
            path,
            style: stroke,
        });
    }
}
