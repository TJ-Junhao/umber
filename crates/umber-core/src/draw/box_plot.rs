//! Boxes, Tukey whiskers and outliers, clipped in axis space before mapping to scene points.

use super::MarkContext;
use super::area::AREA_ALPHA;
use super::bar::{GROUP_GAP, group_slots, in_axis_order};
use super::clip::{Bounds, clip_segment};
use super::error_bars::bar_style;
use super::mapping::{AxisPoint, axis_y_value};
use super::scatter::scatter_item;
use crate::geometry::Path;
use crate::resolve::{AxisScale, Resolved, ResolvedDistribution};
use crate::scene::{FillRule, Item, Paint};
use crate::stats::BoxStats;
use crate::{Marker, Rgba, SizeSpec};

/// Draws distributions in axis-category order, then distribution order within a category.
/// Group widths use the theme's box fraction; slot widths and gap fallback are shared with bars.
pub(super) fn box_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    names: &[String],
    distributions: &[ResolvedDistribution<'_>],
    width: f32,
    items: &mut Vec<Item>,
) {
    let AxisScale::Category(categories) = &resolved.x.scale else {
        return;
    };
    let (start, end) = categories.band(0);
    let total = (end - start) * f64::from(context.theme.box_width());
    let (slot, gap) = group_slots(total, distributions.len(), GROUP_GAP / context.plot.width);
    for (index, i) in in_axis_order(categories, names) {
        let (start, end) = categories.band(index);
        let left = (start + end - total) / 2.0;
        for (j, distribution) in distributions.iter().enumerate() {
            let Some(Some(stats)) = distribution.groups.get(i) else {
                continue;
            };
            let x0 = left + j as f64 * (slot + gap);
            box_group(
                context,
                resolved,
                stats,
                (x0, x0 + slot),
                (distribution.color, width),
                items,
            );
        }
    }
}

/// Draws one summary. A constant group has only its median; empty groups never reach here.
fn box_group(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    stats: &BoxStats,
    (left, right): (f64, f64),
    (color, width): (Rgba, f32),
    items: &mut Vec<Item>,
) {
    let y = |v| axis_y_value(&resolved.y, v);
    let (q1, q3, median) = (y(stats.q1), y(stats.q3), y(stats.median));
    let center = (left + right) / 2.0;
    let half_cap = (right - left) / 4.0;
    // All cuts lie beyond the scene clip, so they cannot create a visible boundary stroke.
    let reach = f64::from(width).max(2.0);
    let (x0, x1) = context.maps.x.expanded(reach);
    let (y0, y1) = context.maps.y.expanded(reach);
    let bounds = Bounds {
        left: x0,
        right: x1,
        top: y0,
        bottom: y1,
    };
    let mut outline = Path::new();
    let segment = |path: &mut Path, a: (f64, f64), b: (f64, f64)| {
        if let Some((a, b)) =
            clip_segment(AxisPoint::new(a.0, a.1), AxisPoint::new(b.0, b.1), &bounds)
        {
            path.move_to(context.maps.to_plot(a).to_scene());
            path.line_to(context.maps.to_plot(b).to_scene());
        }
    };
    if stats.low_whisker != stats.high_whisker {
        for (quartile, whisker) in [(q1, y(stats.low_whisker)), (q3, y(stats.high_whisker))] {
            segment(&mut outline, (center, quartile), (center, whisker));
            segment(
                &mut outline,
                (center - half_cap, whisker),
                (center + half_cap, whisker),
            );
        }
        let (a, b, low, high) = (left.max(x0), right.min(x1), q1.max(y0), q3.min(y1));
        if a < b && low < high {
            let mut path = Path::new();
            for (i, (x, y)) in [(a, low), (b, low), (b, high), (a, high)]
                .into_iter()
                .enumerate()
            {
                let point = context.maps.to_plot(AxisPoint::new(x, y)).to_scene();
                if i == 0 {
                    path.move_to(point);
                } else {
                    path.line_to(point);
                }
            }
            path.close();
            outline.extend(&path);
            items.push(Item::Fill {
                path,
                paint: Paint::Solid(color.scale_alpha(AREA_ALPHA)),
                rule: FillRule::NonZero,
            });
        }
        if !outline.is_empty() {
            items.push(Item::Stroke {
                path: outline,
                style: bar_style(width / 2.0, color),
            });
        }
    }
    let mut line = Path::new();
    segment(&mut line, (left, median), (right, median));
    if !line.is_empty() {
        items.push(Item::Stroke {
            path: line,
            style: bar_style(width, color),
        });
    }
    let xs = vec![center; stats.outliers.len()];
    let ys: Vec<f64> = stats.outliers.iter().map(|&v| y(v)).collect();
    items.extend(scatter_item(
        context,
        &xs,
        &ys,
        color,
        Marker::Circle,
        context.theme.drawn_marker_size(SizeSpec::Auto) * 0.8,
    ));
}
