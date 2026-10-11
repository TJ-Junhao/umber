//! Histogram drawing: bins to rectangles, the gap between them, the gradient fill and the top
//! edge.

use super::MarkContext;
use super::area::{FadeStop, vertical_fade};
use super::bar_shape::BarFrame;
use super::mapping::axis_value;
use crate::binning::Histogram;
use crate::geometry::Path;
use crate::resolve::ResolvedAxis;
use crate::scene::{FillRule, Item, Paint};
use crate::{AreaFill, Rgba};

/// The gradient's opacity at the top of the tallest bin, as a share of the color's own.
const TOP_ALPHA: f32 = 0.95;
/// The depth of the solid top edge of each bin under a gradient, in points.
const TOP_EDGE_DEPTH: f64 = 1.6;

/// The scene items of one histogram: a single `Fill` that holds every visible bin, followed,
/// under [`AreaFill::Gradient`], by a single solid `Fill` that holds their top edges. Nothing
/// when no bin is visible.
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
/// the tallest bin to transparent at the base, so shorter bins look dimmer (see [`ramp`]); and
/// each bin whose top lies inside the clip range gets a top edge in the solid color over the
/// fill, from its top down 1.6 pt or to its base, whichever is nearer, so the edge never paints
/// beyond the bin.
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
    let edge_depth = context.maps.y.length(TOP_EDGE_DEPTH);
    let base = frame.base();
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
        let Some(span) = frame.clip_x((low + inset, high - inset)) else {
            continue;
        };
        let top = axis_value(y, count as f64);
        let Some(outline) = frame.outline(span, base, top, false) else {
            continue;
        };
        fill.extend(&outline);
        if gradient && frame.contains_y(top) {
            // From the top towards the base, but never past it.
            let inner = if top > base {
                (top - edge_depth).max(base)
            } else {
                (top + edge_depth).min(base)
            };
            if let Some(edge) = frame.outline(span, inner, top, false) {
                edges.extend(&edge);
            }
        }
    }
    if fill.is_empty() {
        return;
    }
    let paint = if gradient {
        let tallest = axis_value(y, histogram.max_count() as f64);
        let at = |v: f64| {
            let v = frame.clip_y(v);
            FadeStop {
                y: frame.plot_y(v),
                share: TOP_ALPHA * ramp(v, base, tallest),
            }
        };
        vertical_fade(color, at(tallest), at(base))
    } else {
        Paint::Solid(color)
    };
    items.push(Item::Fill {
        path: fill,
        paint,
        rule: FillRule::NonZero,
    });
    if !edges.is_empty() {
        items.push(Item::Fill {
            path: edges,
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        });
    }
}

/// Where the y axis-space value `v` lies on the gradient ramp of a histogram, as a share of its
/// top opacity: 1 at `tallest`, the top of the tallest bin, 0 at `base`, linear in between and
/// kept within `0..=1`; 0 everywhere when the two coincide.
///
/// The gradient is anchored at these true ends. When clipping moves an end of the drawn
/// gradient, that end takes the ramp's opacity at the clipped value, so the opacity at every
/// visible count is the one the unclipped gradient would give it.
fn ramp(v: f64, base: f64, tallest: f64) -> f32 {
    if tallest == base {
        return 0.0;
    }
    let share = (v - base) / (tallest - base);
    if share.is_nan() {
        0.0
    } else {
        share.clamp(0.0, 1.0) as f32
    }
}
