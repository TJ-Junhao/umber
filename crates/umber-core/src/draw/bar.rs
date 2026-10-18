//! Bar drawing: category bands, the offsets of grouped bars and the segments of stacked bars.
//!
//! Bars are drawn in the frame of their orientation (see [`super::mapping`]): the axis they
//! lie along first, as x, and their value axis second, as y. Horizontal bars are the same code
//! with the axes swapped.

use super::MarkContext;
use super::bar_shape::BarFrame;
use super::mapping::axis_value;
use crate::geometry::Path;
use crate::model::{BarGroupMark, BarMark, SeriesView};
use crate::resolve::{AxisScale, ResolvedAxis};
use crate::scene::{FillRule, Item, Paint};
use crate::stacking::{Segment, Stacks};
use crate::{CategoryScale, Rgba, Theme};

/// The share of its category band that a plain bar, a stack or a group of bars takes, centered
/// in the band.
const BAR_SHARE: f64 = 0.72;
/// The gap between neighbouring bars of a group, in points.
pub(super) const GROUP_GAP: f64 = 2.0;
/// The gap between neighbouring segments on one side of a stack, in points: the background
/// shows through it, so adjacent fills never meet in an antialiased seam.
const STACK_GAP: f64 = 1.0;

/// The theme's bar corner radius in points, kept finite and at least 0.
fn corner_radius(theme: &Theme) -> f64 {
    let radius = f64::from(theme.bar_corner_radius());
    if radius.is_finite() && radius > 0.0 {
        radius
    } else {
        0.0
    }
}

/// The range `(left, right)` across the bar, stack or group of category `index`, along the
/// category axis in axis space: [`BAR_SHARE`] of the category's band, centered in it. On a
/// category y axis `left` is the top.
pub(super) fn bar_span(categories: &CategoryScale, index: usize) -> (f64, f64) {
    let (start, end) = categories.band(index);
    let (center, half) = ((start + end) / 2.0, (end - start) * BAR_SHARE / 2.0);
    (center - half, center + half)
}

/// The categories of a mark in axis order: for each category the axis knows, its index on the
/// axis and its index in the mark, sorted by the axis index. Each category appears once in a
/// mark, so the order is total. Bars are drawn in this order, whatever order the mark lists
/// its categories in.
pub(super) fn in_axis_order(categories: &CategoryScale, names: &[String]) -> Vec<(usize, usize)> {
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
pub(super) fn group_slots(group: f64, count: usize, gap: f64) -> (f64, f64) {
    let n = count.max(1) as f64;
    let width = (group - (n - 1.0) * gap) / n;
    if width < gap {
        (group / n, 0.0)
    } else {
        (width, gap)
    }
}

/// The [`BarFrame`] of bars `width` category axis units wide, rounded by the theme's corner
/// radius, whose value axis is `values`.
pub(super) fn frame(context: &MarkContext<'_>, values: &ResolvedAxis, width: f64) -> BarFrame {
    let radius = corner_radius(context.theme);
    BarFrame::new(
        context.maps,
        values,
        context.baseline.pos,
        radius,
        context.maps.x.points(width),
    )
}

/// Pushes the bar `outline`, if any, as a `Fill` in `color`.
fn push_bar(outline: Option<Path>, color: Rgba, items: &mut Vec<Item>) {
    if let Some(path) = outline {
        items.push(Item::Fill {
            path,
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        });
    }
}

/// The scene items of one bar mark lying along the category axis `along` with its values on
/// `values`, drawn in the frame of `context`: one `Fill` per visible bar in `color`, in category
/// order (see [`in_axis_order`]). Each bar runs from the base of its [`BarFrame`] to its height,
/// clipped and rounded as the frame describes. A NaN height, or one equal to the base, draws
/// nothing.
pub(super) fn bar_items(
    context: &MarkContext<'_>,
    along: &ResolvedAxis,
    values: &ResolvedAxis,
    mark: &BarMark,
    color: Rgba,
    items: &mut Vec<Item>,
) {
    let AxisScale::Category(categories) = &along.scale else {
        return;
    };
    // Every band has the same width, so every bar of the mark has the same width.
    let (left, right) = bar_span(categories, 0);
    let bars = frame(context, values, right - left);
    for (index, i) in in_axis_order(categories, mark.categories()) {
        let Some(&height) = mark.heights().get(i) else {
            continue;
        };
        let end = axis_value(values, height);
        let outline = bars.outline(bar_span(categories, index), bars.base(), end, true);
        push_bar(outline, color, items);
    }
}

/// The scene items of one grouped bar mark, along `along` with its values on `values` as in
/// [`bar_items`]: one `Fill` per visible bar, in category order (see [`in_axis_order`]) and
/// within a category in series order, from the left, or on a category y axis from the top,
/// each in its series' color.
///
/// The group takes the span of a plain bar ([`BAR_SHARE`] of the band); its bars are
/// [`GROUP_GAP`] apart, or touch when that would leave them narrower than the gap (see
/// [`group_slots`]). Each bar runs from the base to its height, clipped and rounded as its
/// [`BarFrame`] describes; a NaN height, or one equal to the base, draws nothing.
pub(super) fn grouped_items(
    context: &MarkContext<'_>,
    along: &ResolvedAxis,
    values: &ResolvedAxis,
    mark: &BarGroupMark,
    series: &[(SeriesView<'_>, Rgba)],
    items: &mut Vec<Item>,
) {
    let AxisScale::Category(categories) = &along.scale else {
        return;
    };
    let slots = GroupSlots::new(context, categories, series.len());
    let bars = frame(context, values, slots.width);
    for (index, i) in in_axis_order(categories, mark.categories()) {
        for (j, (view, color)) in series.iter().enumerate() {
            let Some(&height) = view.values().get(i) else {
                continue;
            };
            let end = axis_value(values, height);
            let outline = bars.outline(slots.span(categories, index, j), bars.base(), end, true);
            push_bar(outline, *color, items);
        }
    }
}

/// The bars of one group of grouped bars: their width and the gap between them, in category
/// axis units (see [`group_slots`]).
#[derive(Debug, Clone, Copy)]
pub(super) struct GroupSlots {
    /// The width of each bar.
    pub(super) width: f64,
    /// The gap between neighbouring bars.
    gap: f64,
}

impl GroupSlots {
    /// The slots of `count` bars in the span of a plain bar ([`BAR_SHARE`] of a band of
    /// `categories`), [`GROUP_GAP`] apart in the frame of `context`.
    pub(super) fn new(context: &MarkContext<'_>, categories: &CategoryScale, count: usize) -> Self {
        let (start, end) = bar_span(categories, 0);
        let gap = context.maps.x.length(GROUP_GAP);
        let (width, gap) = group_slots(end - start, count, gap);
        GroupSlots { width, gap }
    }

    /// The range `(left, right)` across bar `j` of the group of category `index`, in category
    /// axis space; on a category y axis `left` is the top.
    pub(super) fn span(self, categories: &CategoryScale, index: usize, j: usize) -> (f64, f64) {
        let (group_left, _) = bar_span(categories, index);
        let left = group_left + j as f64 * (self.width + self.gap);
        (left, left + self.width)
    }
}

/// The scene items of one stacked bar mark, along `along` with its values on `values` as in
/// [`bar_items`]: one `Fill` per visible segment, in category order (see [`in_axis_order`]) and
/// within a category in series order, each in its series' color.
///
/// A stack takes the span of a plain bar. Each segment of `stacks` runs from its bottom to its
/// top; a segment that starts at 0 starts at the base of its [`BarFrame`] instead, so on a log
/// axis the first segment starts at the baseline. Neighbouring segments on one side of
/// 0 are [`STACK_GAP`] apart (see [`inset_segment`]); segments on opposite sides meet at the
/// base. Only the outermost segment on each side of 0 may have its end rounded. Clipping and
/// rounding are as the frame describes.
pub(super) fn stacked_items(
    context: &MarkContext<'_>,
    along: &ResolvedAxis,
    values: &ResolvedAxis,
    mark: &BarGroupMark,
    series: &[(SeriesView<'_>, Rgba)],
    stacks: &Stacks,
    items: &mut Vec<Item>,
) {
    let AxisScale::Category(categories) = &along.scale else {
        return;
    };
    let (left, right) = bar_span(categories, 0);
    let bars = frame(context, values, right - left);
    let half_gap = stack_half_gap(context);
    for (index, i) in in_axis_order(categories, mark.categories()) {
        let span = bar_span(categories, index);
        for (j, (_, color)) in series.iter().enumerate() {
            let Some(segment) = stacks.segment(i, j) else {
                continue;
            };
            let Some((from, to)) = segment_ends(&bars, values, segment, half_gap) else {
                continue;
            };
            let outline = bars.outline(span, from, to, segment.outermost);
            push_bar(outline, *color, items);
        }
    }
}

/// Half of [`STACK_GAP`] in value axis units of the frame of `context`.
pub(super) fn stack_half_gap(context: &MarkContext<'_>) -> f64 {
    context.maps.y.length(STACK_GAP / 2.0)
}

/// The ends of the drawn stacked `segment` in value axis space (`values`), from its bottom to
/// its top: a segment that starts at 0 starts at the base of `bars`, and each inner end is moved
/// in by `half_gap` (see [`inset_segment`]). `None` when no height is left.
pub(super) fn segment_ends(
    bars: &BarFrame,
    values: &ResolvedAxis,
    segment: Segment,
    half_gap: f64,
) -> Option<(f64, f64)> {
    let from = if segment.bottom == 0.0 {
        bars.base()
    } else {
        axis_value(values, segment.bottom)
    };
    let to = axis_value(values, segment.top);
    let inner = (segment.bottom != 0.0, !segment.outermost);
    inset_segment(from, to, inner, half_gap)
}

/// The segment from `from` to `to` (value axis space) with each inner end, one shared with a
/// neighbouring segment on the same side of 0, moved inwards by `half_gap` (axis units); `inner`
/// says whether the `from` and the `to` end are inner. The base at 0 and the outermost end stay
/// where they are. The insets are taken from the true ends, before clipping, so two neighbours
/// are always exactly two half gaps apart. `None` when no height is left.
fn inset_segment(from: f64, to: f64, inner: (bool, bool), half_gap: f64) -> Option<(f64, f64)> {
    let away = (to - from).signum();
    let from = if inner.0 {
        from + away * half_gap
    } else {
        from
    };
    let to = if inner.1 { to - away * half_gap } else { to };
    ((to - from) * away > 0.0).then_some((from, to))
}
