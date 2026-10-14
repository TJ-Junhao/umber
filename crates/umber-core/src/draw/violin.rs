//! Equal-maximum-width violin densities, with local-width quantile lines.

use super::MarkContext;
use super::area::AREA_ALPHA;
use super::bar::{GROUP_GAP, group_slots, in_axis_order};
use super::clip::{Bounds, clip_segment};
use super::error_bars::bar_style;
use super::mapping::{AxisPoint, axis_y_value};
use crate::Rgba;
use crate::geometry::Path;
use crate::resolve::{AxisScale, Resolved, ResolvedDistribution, ViolinStats};
use crate::scene::{FillRule, Item, Paint};

/// Draws categories in axis order and distributions in input order, using bar gap rules.
pub(super) fn violin_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    names: &[String],
    distributions: &[ResolvedDistribution<'_, ViolinStats>],
    width: f32,
    items: &mut Vec<Item>,
) {
    let AxisScale::Category(categories) = &resolved.x.scale else {
        return;
    };
    let (start, end) = categories.band(0);
    let total = (end - start) * f64::from(context.theme.violin_width());
    let (slot, gap) = group_slots(total, distributions.len(), GROUP_GAP / context.plot.width);
    for (index, i) in in_axis_order(categories, names) {
        let (start, end) = categories.band(index);
        let left = (start + end - total) / 2.0;
        for (j, distribution) in distributions.iter().enumerate() {
            let Some(Some(stats)) = distribution.groups.get(i) else {
                continue;
            };
            let center = left + j as f64 * (slot + gap) + slot / 2.0;
            violin_group(
                context,
                resolved,
                stats,
                (center, slot / 2.0),
                (distribution.color, width),
                items,
            );
        }
    }
}

/// Draws one density. Clipping happens before converting extreme data coordinates to f32.
fn violin_group(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    stats: &ViolinStats,
    (center, half): (f64, f64),
    (color, width): (Rgba, f32),
    items: &mut Vec<Item>,
) {
    let reach = f64::from(width).max(2.0);
    let (x0, x1) = context.maps.x.expanded(reach);
    let (y0, y1) = context.maps.y.expanded(reach);
    let bounds = Bounds {
        left: x0,
        right: x1,
        top: y0,
        bottom: y1,
    };
    if !stats.density.is_empty() {
        let points: Vec<_> =
            stats
                .density
                .iter()
                .map(|&(y, w)| AxisPoint::new(center + half * w, axis_y_value(&resolved.y, y)))
                .chain(
                    stats.density.iter().rev().map(|&(y, w)| {
                        AxisPoint::new(center - half * w, axis_y_value(&resolved.y, y))
                    }),
                )
                .collect();
        // Category x is always inside its band. Cut only y, beyond the scene clip so that
        // the polygon's closing edges cannot create visible horizontal outline strokes.
        let points = clip_y(&clip_y(&points, y0, true), y1, false);
        if let Some(&first) = points.first() {
            let mut path = Path::new();
            path.move_to(context.maps.to_plot(first).to_scene());
            for &p in &points[1..] {
                path.line_to(context.maps.to_plot(p).to_scene());
            }
            path.close();
            items.push(Item::Fill {
                path: path.clone(),
                paint: Paint::Solid(color.scale_alpha(AREA_ALPHA)),
                rule: FillRule::NonZero,
            });
            items.push(Item::Stroke {
                path,
                style: bar_style(width / 2.0, color),
            });
        }
    }
    let quantiles = if stats.density.is_empty() {
        vec![(stats.median, 1.0, width)]
    } else {
        vec![
            (stats.q1, 0.5, width / 2.0),
            (stats.q3, 0.5, width / 2.0),
            (stats.median, 1.0, width),
        ]
    };
    for (value, fraction, stroke) in quantiles {
        let half = half * local_width(&stats.density, value) * fraction;
        let y = axis_y_value(&resolved.y, value);
        if let Some((a, b)) = clip_segment(
            AxisPoint::new(center - half, y),
            AxisPoint::new(center + half, y),
            &bounds,
        ) {
            let mut path = Path::new();
            path.move_to(context.maps.to_plot(a).to_scene());
            path.line_to(context.maps.to_plot(b).to_scene());
            items.push(Item::Stroke {
                path,
                style: bar_style(stroke, color),
            });
        }
    }
}

/// Interpolates a relative half-width in data space; constant groups use the entire slot.
fn local_width(density: &[(f64, f64)], value: f64) -> f64 {
    let after = density.partition_point(|&(y, _)| y < value);
    let Some(&(b, wb)) = density.get(after) else {
        return density.last().map_or(1.0, |&(_, w)| w);
    };
    let Some(&(a, wa)) = after.checked_sub(1).and_then(|i| density.get(i)) else {
        return wb;
    };
    if a == b {
        return wb;
    }
    wa + (wb - wa) * ((value - a) / (b - a))
}

/// Clips a closed polygon to one horizontal boundary in axis space.
fn clip_y(points: &[AxisPoint], at: f64, above: bool) -> Vec<AxisPoint> {
    let inside = |p: AxisPoint| if above { p.y >= at } else { p.y <= at };
    let mut result = Vec::with_capacity(points.len() + 2);
    for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
        if inside(a) != inside(b) {
            let fraction = (at - a.y) / (b.y - a.y);
            result.push(AxisPoint::new(a.x + (b.x - a.x) * fraction, at));
        }
        if inside(b) {
            result.push(b);
        }
    }
    result
}
