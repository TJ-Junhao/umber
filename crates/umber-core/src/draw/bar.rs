//! Bar drawing: category bands, the offsets of grouped bars and the segments of stacked bars.

use super::MarkContext;
use super::bar_shape::bar_outline;
use super::mapping::{AxisMap, AxisPoint, axis_y};
use crate::model::BarMark;
use crate::resolve::{AxisScale, ResolvedAxis};
use crate::scene::{FillRule, Item, Paint};
use crate::{CategoryScale, Rgba, Theme, Transform};

/// The share of its category band that a plain bar takes, centered in the band.
const BAR_SHARE: f64 = 0.72;
/// The smallest bar clip margin, in points.
const MIN_BAR_MARGIN: f64 = 2.0;

/// Where bars start on the y axis `y`, in axis space: the value 0 on a linear axis, the low end
/// of the domain (the bottom of the plot) on a log axis, which cannot show 0. `None` on a
/// category axis, which cannot place numeric heights.
fn bar_base(y: &ResolvedAxis, map: AxisMap) -> Option<f64> {
    match &y.scale {
        AxisScale::Continuous(scale) => Some(match scale.transform() {
            Transform::Linear => 0.0,
            Transform::Log(_) => map.lo,
        }),
        AxisScale::Category(_) => None,
    }
}

/// The theme's bar corner radius in points, kept finite and at least 0.
fn corner_radius(theme: &Theme) -> f64 {
    let radius = f64::from(theme.bar_corner_radius());
    if radius.is_finite() && radius > 0.0 {
        radius
    } else {
        0.0
    }
}

/// The x range `(left, right)` of the bar of category `index`, in axis space: [`BAR_SHARE`] of
/// the category's band, centered in it.
fn bar_span(categories: &CategoryScale, index: usize) -> (f64, f64) {
    let (start, end) = categories.band(index);
    let (center, half) = ((start + end) / 2.0, (end - start) * BAR_SHARE / 2.0);
    (center - half, center + half)
}

/// The scene items of one bar mark, one `Fill` per visible bar in `color`, in category order:
/// the order of the categories along the x axis, whatever order the mark lists them in. Nothing
/// is drawn on axes that cannot hold bars (the model rejects them).
///
/// Each bar runs from the base of [`bar_base`] to its height. It is clipped in axis space
/// against the y domain expanded by `max(r, 2)` pt, where `r` is the corner radius as
/// [`bar_outline`] clamps it to half the bar width; only the clipped ends are mapped to plot
/// points. The end away from the base is rounded only when the bar's true end lies inside that
/// expanded domain; a cut end is square and lies at least `r` beyond the plot area, outside
/// the marks clip. A NaN height, or one equal to the base, draws nothing.
pub(super) fn bar_items(
    context: &MarkContext<'_>,
    x: &ResolvedAxis,
    y: &ResolvedAxis,
    mark: &BarMark,
    color: Rgba,
    items: &mut Vec<Item>,
) {
    let MarkContext {
        theme, maps, plot, ..
    } = *context;
    let AxisScale::Category(categories) = &x.scale else {
        return;
    };
    let Some(base) = bar_base(y, maps.y) else {
        return;
    };
    // Every band has the same width, so every bar of the mark has the same width in points.
    let (left, right) = bar_span(categories, 0);
    let half_width = (right - left) * plot.width / 2.0;
    let radius = corner_radius(theme);
    let margin = radius.min(half_width).max(MIN_BAR_MARGIN);
    let (low, high) = maps.y.expanded(margin);
    // `max` then `min` never panics, unlike `clamp`, and keeps finite values within the range.
    let clip = |v: f64| v.max(low).min(high);

    // Each category appears once in a mark, so sorting by axis index gives a total order.
    let ends = axis_y(y, mark.heights());
    let mut bars: Vec<(usize, f64)> = mark
        .categories()
        .iter()
        .zip(ends)
        .filter_map(|(name, end)| categories.index_of(name).map(|index| (index, end)))
        .collect();
    bars.sort_unstable_by_key(|&(index, _)| index);
    for (index, end) in bars {
        if end.is_nan() || end == base {
            continue;
        }
        let (visible_base, visible_end) = (clip(base), clip(end));
        if visible_base == visible_end {
            // The whole bar lies beyond one end of the expanded domain.
            continue;
        }
        let round_end = (low..=high).contains(&end);
        let (left, right) = bar_span(categories, index);
        let at_base = maps.to_plot(AxisPoint::new(left, visible_base));
        let at_end = maps.to_plot(AxisPoint::new(right, visible_end));
        let path = bar_outline(at_base.x, at_end.x, at_base.y, at_end.y, radius, round_end);
        if !path.is_empty() {
            items.push(Item::Fill {
                path,
                paint: Paint::Solid(color),
                rule: FillRule::NonZero,
            });
        }
    }
}
