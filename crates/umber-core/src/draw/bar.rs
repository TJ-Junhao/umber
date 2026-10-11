//! Bar drawing: category bands, the offsets of grouped bars and the segments of stacked bars.

use super::MarkContext;
use super::bar_shape::bar_outline;
use super::mapping::{AxisMap, AxisMaps, AxisPoint, axis_y_value};
use crate::model::{BarGroupMark, BarMark, BarSeriesView};
use crate::resolve::{AxisScale, ResolvedAxis};
use crate::scene::{FillRule, Item, Paint};
use crate::stacking::Stacks;
use crate::{CategoryScale, Rgba, Theme, Transform};

/// The share of its category band that a plain bar, a stack or a group of bars takes, centered
/// in the band.
const BAR_SHARE: f64 = 0.72;
/// The smallest bar clip margin, in points.
const MIN_BAR_MARGIN: f64 = 2.0;
/// The gap between neighbouring bars of a group, in points.
const GROUP_GAP: f64 = 2.0;

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

/// The x range `(left, right)` of the bar, stack or group of category `index`, in axis space:
/// [`BAR_SHARE`] of the category's band, centered in it.
fn bar_span(categories: &CategoryScale, index: usize) -> (f64, f64) {
    let (start, end) = categories.band(index);
    let (center, half) = ((start + end) / 2.0, (end - start) * BAR_SHARE / 2.0);
    (center - half, center + half)
}

/// The categories of a mark in axis order: for each category the axis knows, its index on the
/// axis and its index in the mark, sorted by the axis index. Each category appears once in a
/// mark, so the order is total. Bars are drawn in this order, whatever order the mark lists
/// its categories in.
fn in_axis_order(categories: &CategoryScale, names: &[String]) -> Vec<(usize, usize)> {
    let mut order: Vec<(usize, usize)> = names
        .iter()
        .enumerate()
        .filter_map(|(i, name)| categories.index_of(name).map(|index| (index, i)))
        .collect();
    order.sort_unstable_by_key(|&(index, _)| index);
    order
}

/// The width of each bar of a group of `count` bars and the gap between them, in axis units:
/// the group spans `group`, and neighbours are `gap` apart, unless that leaves bars narrower
/// than the gap; then the gap is 0 and the bars touch.
fn group_slots(group: f64, count: usize, gap: f64) -> (f64, f64) {
    let n = count.max(1) as f64;
    let width = (group - (n - 1.0) * gap) / n;
    if width < gap {
        (group / n, 0.0)
    } else {
        (width, gap)
    }
}

/// What the bars of one mark share: the axis maps, where bars start, the corner radius and the
/// axis-space range bars are clipped to.
///
/// Bars are clipped in axis space against the y domain expanded by `max(r, 2)` pt, where `r`
/// is the corner radius as [`bar_outline`] clamps it to half the bar width; only the clipped
/// ends are mapped to plot points. An end is rounded only when it may be rounded and its true
/// value lies inside that expanded domain; a cut end is square and lies at least `r` beyond the
/// plot area, outside the marks clip.
struct Bars {
    /// The axis maps.
    maps: AxisMaps,
    /// Where bars start, in y axis space; see [`bar_base`].
    base: f64,
    /// The theme's corner radius, in points.
    radius: f64,
    /// The y range, in axis space, that bar ends are clipped to.
    low: f64,
    /// See `low`.
    high: f64,
}

impl Bars {
    /// The shared geometry of bars `width` axis units wide on the y axis `y`; `None` on axes
    /// that cannot hold bars (the model rejects them).
    fn new(context: &MarkContext<'_>, y: &ResolvedAxis, width: f64) -> Option<Bars> {
        let MarkContext {
            theme, maps, plot, ..
        } = *context;
        let base = bar_base(y, maps.y)?;
        let radius = corner_radius(theme);
        let margin = radius.min(width * plot.width / 2.0).max(MIN_BAR_MARGIN);
        let (low, high) = maps.y.expanded(margin);
        Some(Bars {
            maps,
            base,
            radius,
            low,
            high,
        })
    }

    /// One bar across `(left, right)` in x axis space, from `from` to `to` in y axis space, as
    /// a `Fill` in `color`. The end at `to` is rounded when `round` holds and `to` lies inside
    /// the clip range. Nothing is drawn when either end is NaN, the ends are equal, or the
    /// whole bar lies beyond one end of the clip range.
    fn push(
        &self,
        (left, right): (f64, f64),
        from: f64,
        to: f64,
        round: bool,
        color: Rgba,
        items: &mut Vec<Item>,
    ) {
        if from.is_nan() || to.is_nan() || from == to {
            return;
        }
        // `max` then `min` never panics, unlike `clamp`, and keeps finite values within the
        // range.
        let clip = |v: f64| v.max(self.low).min(self.high);
        let (visible_from, visible_to) = (clip(from), clip(to));
        if visible_from == visible_to {
            return;
        }
        let round_end = round && (self.low..=self.high).contains(&to);
        let at_from = self.maps.to_plot(AxisPoint::new(left, visible_from));
        let at_to = self.maps.to_plot(AxisPoint::new(right, visible_to));
        let path = bar_outline(
            at_from.x,
            at_to.x,
            at_from.y,
            at_to.y,
            self.radius,
            round_end,
        );
        if !path.is_empty() {
            items.push(Item::Fill {
                path,
                paint: Paint::Solid(color),
                rule: FillRule::NonZero,
            });
        }
    }
}

/// The scene items of one bar mark, one `Fill` per visible bar in `color`, in category order
/// (see [`in_axis_order`]). Each bar runs from the base of [`bar_base`] to its height, clipped
/// and rounded as [`Bars`] describes. A NaN height, or one equal to the base, draws nothing.
pub(super) fn bar_items(
    context: &MarkContext<'_>,
    x: &ResolvedAxis,
    y: &ResolvedAxis,
    mark: &BarMark,
    color: Rgba,
    items: &mut Vec<Item>,
) {
    let AxisScale::Category(categories) = &x.scale else {
        return;
    };
    // Every band has the same width, so every bar of the mark has the same width.
    let (left, right) = bar_span(categories, 0);
    let Some(bars) = Bars::new(context, y, right - left) else {
        return;
    };
    for (index, i) in in_axis_order(categories, mark.categories()) {
        let Some(&height) = mark.heights().get(i) else {
            continue;
        };
        let end = axis_y_value(y, height);
        bars.push(
            bar_span(categories, index),
            bars.base,
            end,
            true,
            color,
            items,
        );
    }
}

/// The scene items of one grouped bar mark, one `Fill` per visible bar, in category order (see
/// [`in_axis_order`]) and within a category in series order, each in its series' color.
///
/// The group takes the span of a plain bar ([`BAR_SHARE`] of the band); its bars are
/// [`GROUP_GAP`] apart, or touch when that would leave them narrower than the gap (see
/// [`group_slots`]). Each bar runs from the base to its height, clipped and rounded as
/// [`Bars`] describes; a NaN height, or one equal to the base, draws nothing.
pub(super) fn grouped_items(
    context: &MarkContext<'_>,
    x: &ResolvedAxis,
    y: &ResolvedAxis,
    mark: &BarGroupMark,
    series: &[(BarSeriesView<'_>, Rgba)],
    items: &mut Vec<Item>,
) {
    let AxisScale::Category(categories) = &x.scale else {
        return;
    };
    let (start, end) = bar_span(categories, 0);
    let (width, gap) = group_slots(end - start, series.len(), GROUP_GAP / context.plot.width);
    let Some(bars) = Bars::new(context, y, width) else {
        return;
    };
    for (index, i) in in_axis_order(categories, mark.categories()) {
        let (group_left, _) = bar_span(categories, index);
        for (j, (view, color)) in series.iter().enumerate() {
            let Some(&height) = view.heights().get(i) else {
                continue;
            };
            let left = group_left + j as f64 * (width + gap);
            let end = axis_y_value(y, height);
            bars.push((left, left + width), bars.base, end, true, *color, items);
        }
    }
}

/// The scene items of one stacked bar mark, one `Fill` per visible segment, in category order
/// (see [`in_axis_order`]) and within a category in series order, each in its series' color.
///
/// A stack takes the span of a plain bar. Each segment of `stacks` runs from its bottom to its
/// top; a segment that starts at 0 starts at the base of [`bar_base`] instead, so on a log axis
/// the first segment starts at the bottom of the plot. Only the outermost segment on each side
/// of 0 may have its end rounded. Clipping and rounding are as [`Bars`] describes.
pub(super) fn stacked_items(
    context: &MarkContext<'_>,
    x: &ResolvedAxis,
    y: &ResolvedAxis,
    mark: &BarGroupMark,
    series: &[(BarSeriesView<'_>, Rgba)],
    stacks: &Stacks,
    items: &mut Vec<Item>,
) {
    let AxisScale::Category(categories) = &x.scale else {
        return;
    };
    let (left, right) = bar_span(categories, 0);
    let Some(bars) = Bars::new(context, y, right - left) else {
        return;
    };
    for (index, i) in in_axis_order(categories, mark.categories()) {
        let span = bar_span(categories, index);
        for (j, (_, color)) in series.iter().enumerate() {
            let Some(segment) = stacks.segment(i, j) else {
                continue;
            };
            let from = if segment.bottom == 0.0 {
                bars.base
            } else {
                axis_y_value(y, segment.bottom)
            };
            let to = axis_y_value(y, segment.top);
            bars.push(span, from, to, segment.outermost, *color, items);
        }
    }
}
