//! Histogram drawing: bins to rectangles, the gap between them, the gradient fill and the top
//! edge.

use super::MarkContext;
use super::area::baseline_fade;
use super::bar_shape::BarFrame;
use super::mapping::{AxisPoint, axis_value};
use crate::binning::Histogram;
use crate::geometry::Path;
use crate::resolve::ResolvedAxis;
use crate::scene::{FillRule, Item, LineCap, LineJoin, Paint, StrokeStyle};
use crate::{AreaFill, Rgba};

/// The gradient's opacity at the top of the tallest bin, as a share of the color's own.
const TOP_ALPHA: f32 = 0.95;
/// The width of the solid top edge of each bin under a gradient, in points.
const TOP_EDGE_WIDTH: f32 = 1.6;

/// The scene items of one histogram: a single `Fill` that holds every visible bin, followed,
/// under [`AreaFill::Gradient`], by a single `Stroke` that holds their top edges. Nothing when
/// no bin is visible.
///
/// Each non-empty bin is a rectangle with square corners from the base of its [`BarFrame`] (0
/// on a linear y axis, the bottom of the plot on a log one) to its count, across its edges inset
/// by half the theme's `hist_gap` on each side; the gap is at most half the bin's width. A bin
/// with a count of 0 draws nothing. Both ends of a bin are clipped in axis space as the frame
/// describes: the count against the y domain and the edges against the x domain, each expanded
/// by the frame's margin, so a bin far outside narrow limits stays within a few points of the
/// plot area.
///
/// Without a gradient the fill is the solid `color`. With [`AreaFill::Gradient`] it is one
/// vertical gradient for the whole histogram, from 0.95 of the color's opacity at the top of
/// the tallest bin to transparent at the base, both clipped as the bins are, so shorter bins
/// look dimmer; and each bin whose top lies inside the clip range gets a 1.6 pt top edge in the
/// solid color, drawn over the fill.
pub(super) fn hist_items(
    context: &MarkContext<'_>,
    x: &ResolvedAxis,
    y: &ResolvedAxis,
    histogram: &Histogram,
    color: Rgba,
    items: &mut Vec<Item>,
) {
    let Some(frame) = BarFrame::square(context.maps, y) else {
        return;
    };
    let gradient = context.theme.area_fill() == AreaFill::Gradient;
    // `max` keeps a NaN gap from spreading; the theme only holds finite gaps of at least 0.
    let gap = context
        .maps
        .x
        .length(f64::from(context.theme.hist_gap()).max(0.0));
    let mut fill = Path::new();
    let mut edges = Path::new();
    for (i, &count) in histogram.counts().iter().enumerate() {
        if count == 0 {
            continue;
        }
        let (low, high) = (
            axis_value(x, histogram.edge(i)),
            axis_value(x, histogram.edge(i + 1)),
        );
        let inset = gap.min((high - low) / 2.0) / 2.0;
        let Some((left, right)) = frame.clip_x((low + inset, high - inset)) else {
            continue;
        };
        let top = axis_value(y, count as f64);
        let Some(outline) = frame.outline((left, right), frame.base(), top, false) else {
            continue;
        };
        fill.extend(&outline);
        if gradient && frame.contains_y(top) {
            let maps = context.maps;
            edges.move_to(maps.to_plot(AxisPoint::new(left, top)).to_scene());
            edges.line_to(maps.to_plot(AxisPoint::new(right, top)).to_scene());
        }
    }
    if fill.is_empty() {
        return;
    }
    let paint = if gradient {
        let tallest = axis_value(y, histogram.max_count() as f64);
        let (top_y, base_y) = (frame.plot_y(tallest), frame.plot_y(frame.base()));
        baseline_fade(color, TOP_ALPHA, top_y, base_y)
    } else {
        Paint::Solid(color)
    };
    items.push(Item::Fill {
        path: fill,
        paint,
        rule: FillRule::NonZero,
    });
    if !edges.is_empty() {
        items.push(Item::Stroke {
            path: edges,
            style: StrokeStyle {
                color,
                width: TOP_EDGE_WIDTH,
                dash: Vec::new(),
                cap: LineCap::Butt,
                join: LineJoin::Miter,
            },
        });
    }
}
