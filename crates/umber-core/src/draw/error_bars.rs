//! Error bars: a line from the lower to the upper limit of each bar, with a cap across each end.
//! All the bars of a mark are one stroke.
//!
//! Bars are not reduced: each is a separate datum, and a bar that cannot reach the plot area is
//! dropped before it is mapped, so a long mark costs what its visible bars cost. Everything is
//! clipped in axis space, before it is mapped to plot points: a bar whose position is outside the
//! plot loses its stem, a bar that runs past a limit of the value axis is cut at it, and a cap
//! is kept only when its end is on the plot.

use super::MarkContext;
use super::mapping::{AxisPoint, axis_shared, axis_values};
use crate::Rgba;
use crate::geometry::{Path, Point};
use crate::model::{ErrorBarMark, ErrorBarOrientation};
use crate::resolve::{Orientation, Resolved};
use crate::scene::{Item, LineCap, LineJoin, StrokeStyle};

/// The stroke of error bars and their caps: solid, `width` pt wide, with flat ends, so a bar
/// ends exactly at its limit and a cap is exactly as long as the theme says.
pub(super) fn bar_style(width: f32, color: Rgba) -> StrokeStyle {
    StrokeStyle {
        color,
        width,
        dash: Vec::new(),
        cap: LineCap::Butt,
        join: LineJoin::Miter,
    }
}

/// Adds to `path` the cap across the bar at `at`: a line of `length` centred on it, across a bar
/// that runs along y when `vertical` and along x otherwise.
pub(super) fn push_cap(path: &mut Path, vertical: bool, at: Point, length: f32) {
    let half = length / 2.0;
    let (from, to) = if vertical {
        (at.translated(-half, 0.0), at.translated(half, 0.0))
    } else {
        (at.translated(0.0, -half), at.translated(0.0, half))
    };
    path.move_to(from);
    path.line_to(to);
}

/// Adds to `path` the bar from `start` to `end`, and a cap of `cap` pt across each end when it is
/// given. The bar runs along y when `vertical` and along x otherwise.
pub(super) fn push_bar(
    path: &mut Path,
    vertical: bool,
    (start, end): (Point, Point),
    cap: Option<f32>,
) {
    path.move_to(start);
    path.line_to(end);
    if let Some(length) = cap {
        push_cap(path, vertical, start, length);
        push_cap(path, vertical, end, length);
    }
}

/// The scene items of one error bar mark in `color`, its lines `width` pt wide: one stroke of all
/// the visible bars, and nothing when none is visible.
///
/// A bar is skipped when its position or either limit is NaN. A bar whose position is farther
/// beyond the plot than half the width of its stroke, or of its caps when it has them, is
/// dropped. The stem is cut at the limits of the value axis and drawn only when something is
/// left of it; a cap is kept when its end is on the plot or within half a stroke width of it,
/// as a reference line is, so the cap of an end beyond the plot is lost and the bar keeps its
/// inside part. A bar with equal limits has only caps, or nothing without them.
pub(super) fn error_bar_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &ErrorBarMark,
    (color, width): (Rgba, f32),
    items: &mut Vec<Item>,
) {
    // Vertical bars stand along x with their limits on y, horizontal ones the other way round;
    // both are drawn in the frame of their orientation (see `super::mapping`), whose first axis
    // is the one the bars are placed along and whose second is the one their limits are on.
    let orientation = match mark.orientation() {
        ErrorBarOrientation::Vertical => Orientation::Vertical,
        ErrorBarOrientation::Horizontal => Orientation::Horizontal,
    };
    let vertical = orientation == Orientation::Vertical;
    let cap = mark.options().caps.then(|| context.theme.cap_width());
    let maps = context.maps.oriented(orientation);
    let (along, values) = resolved.along_and_values(orientation);
    let positions = axis_values(along, mark.positions());
    let (low, high) = mark.limits();
    let (lows, highs) = (axis_shared(values, low), axis_shared(values, high));
    let (position_map, value_map) = (maps.x, maps.y);
    let half_stroke = f64::from(width) / 2.0;
    let reach = cap.map_or(half_stroke, |c| half_stroke.max(f64::from(c) / 2.0));
    let (near, far) = position_map.expanded(reach);
    let (ends_low, ends_high) = value_map.expanded(half_stroke);
    let (domain_low, domain_high) = (value_map.lo, value_map.hi);
    let plot_point = |position: f64, value: f64| -> Point {
        maps.to_plot(AxisPoint::new(position, value)).to_scene()
    };
    let mut path = Path::new();
    for ((&position, &low), &high) in positions.iter().zip(&lows).zip(&highs) {
        if position.is_nan() || low.is_nan() || high.is_nan() || position < near || position > far {
            continue;
        }
        // The value axis runs the same way in axis space whatever its direction on the page, so
        // the order of the limits is the order in axis space.
        let (start, end) = (low.max(domain_low), high.min(domain_high));
        if start < end {
            path.move_to(plot_point(position, start));
            path.line_to(plot_point(position, end));
        }
        let Some(length) = cap else { continue };
        for limit in [low, high] {
            if (ends_low..=ends_high).contains(&limit) {
                push_cap(&mut path, vertical, plot_point(position, limit), length);
            }
        }
    }
    if !path.is_empty() {
        items.push(Item::Stroke {
            path,
            style: bar_style(width, color),
        });
    }
}
