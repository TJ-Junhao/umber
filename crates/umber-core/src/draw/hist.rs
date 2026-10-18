//! Histogram drawing: bins to rectangles, the gap between them, the gradient fill and the top
//! edge, stacked series, and the outlines of step histograms.

use super::MarkContext;
use super::area::{FadeStop, vertical_fade};
use super::bar_shape::BarFrame;
use super::fill::HISTOGRAM_TOP_ALPHA;
use super::line::{line_items, line_style};
use super::mapping::axis_value;
use crate::binning::HistBins;
use crate::geometry::Path;
use crate::model::HistStyle;
use crate::resolve::ResolvedAxis;
use crate::scene::{FillRule, Item, Paint};
use crate::{Dash, FillStyle, Rgba};

/// The depth of the solid top edge of each bin under a gradient, in points.
const TOP_EDGE_DEPTH: f64 = 1.6;

/// The scene items of one histogram whose series are `layers` (label and color, in order),
/// binned in `bins`, drawn in `style`: bars ([`bar_items`]) for [`HistStyle::Bars`] and
/// [`HistStyle::Stacked`], whose series are stacked in order (one series is plain bars), and
/// one outline per series ([`outline_items`]), `width` pt wide, for [`HistStyle::Step`].
pub(super) fn hist_items(
    context: &MarkContext<'_>,
    (x, y): (&ResolvedAxis, &ResolvedAxis),
    bins: &HistBins,
    layers: &[(&str, Rgba)],
    (style, width): (HistStyle, f32),
    items: &mut Vec<Item>,
) {
    match style {
        HistStyle::Bars | HistStyle::Stacked => bar_items(context, (x, y), bins, layers, items),
        HistStyle::Step => {
            for (g, &(_, color)) in layers.iter().enumerate() {
                outline_items(context, (x, y), bins, g, (color, width), items);
            }
        }
    }
}

/// The bars of a histogram whose series are `layers`, in order: for each series, a single
/// `Fill` that holds every visible bar of that series, followed, under [`FillStyle::Gradient`],
/// by a single solid `Fill` that holds their top edges. Nothing for a series with no visible bar.
///
/// In each bin the series are stacked in order: each bar runs from the top of the bars below it
/// to that plus its height, and the first from the base of its [`BarFrame`] (0 on a linear y
/// axis, the baseline on a log one). With one series every bar starts at the base. A bar of
/// height 0 draws nothing. Each bar spans its bin's edges inset by half the theme's `hist_gap`
/// on each side; the gap is at most half the bin's width. Bars have square corners. Both ends
/// of a bar are clipped in axis space as the frame describes: its top and bottom against the y
/// domain and its edges against the x domain, each expanded by the frame's margin, so a bin far
/// outside narrow limits stays within a few points of the plot area.
///
/// Under [`FillStyle::Solid`] each series' fill is its solid color. Under [`FillStyle::Gradient`]
/// it is one vertical gradient in the series color, the same for every series, from 0.95 of the
/// color's opacity at the top of the tallest stack to transparent at the base, so lower bars look
/// dimmer (see [`ramp`]); and each bar whose top lies inside the clip range gets a top edge in
/// the solid color over the fill, from its top down 1.6 pt or to its bottom, whichever is nearer,
/// so the edge never paints beyond the bar.
fn bar_items(
    context: &MarkContext<'_>,
    (x, y): (&ResolvedAxis, &ResolvedAxis),
    bins: &HistBins,
    layers: &[(&str, Rgba)],
    items: &mut Vec<Item>,
) {
    let frame = BarFrame::square(context.maps, y, context.baseline.pos);
    let gradient = context.theme.fill_style() == FillStyle::Gradient;
    // `max` keeps a NaN gap from spreading; the theme only holds finite gaps of at least 0.
    let gap = context
        .maps
        .x
        .length(f64::from(context.theme.hist_gap()).max(0.0));
    let edge_depth = context.maps.y.length(TOP_EDGE_DEPTH);
    let base = frame.base();
    let tallest = axis_value(y, bins.tops().fold(0.0, f64::max));
    let mut bottoms = vec![0.0; bins.bins()];
    for (g, &(_, color)) in layers.iter().enumerate() {
        let mut fill = Path::new();
        let mut edges = Path::new();
        for (i, &height) in bins.heights(g).iter().enumerate() {
            let bottom = bottoms[i];
            bottoms[i] = bottom + height;
            if height == 0.0 {
                continue;
            }
            let (low, high) = (axis_value(x, bins.edge(i)), axis_value(x, bins.edge(i + 1)));
            let inset = gap.min((high - low) / 2.0) / 2.0;
            let Some(span) = frame.clip_x((low + inset, high - inset)) else {
                continue;
            };
            let from = if bottom == 0.0 {
                base
            } else {
                axis_value(y, bottom)
            };
            let top = axis_value(y, bottom + height);
            let Some(outline) = frame.outline(span, from, top, false) else {
                continue;
            };
            fill.extend(&outline);
            if gradient && frame.contains_y(top) {
                // From the top towards the bottom, but never past it.
                let inner = if top > from {
                    (top - edge_depth).max(from)
                } else {
                    (top + edge_depth).min(from)
                };
                if let Some(edge) = frame.outline(span, inner, top, false) {
                    edges.extend(&edge);
                }
            }
        }
        if fill.is_empty() {
            continue;
        }
        let paint = if gradient {
            let at = |v: f64| {
                let v = frame.clip_y(v);
                FadeStop {
                    y: frame.plot_y(v),
                    share: HISTOGRAM_TOP_ALPHA * ramp(v, base, tallest),
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
}

/// The outline of series `g` of a step histogram, in `color`, `width` pt wide: one solid line
/// from the base of its [`BarFrame`] at the low edge of the first bin, up to the first bar's
/// height, along the top of every bin and down at each edge to the next bin's height, and down
/// to the base at the high edge of the last bin. An empty bin's level is the base. The line is
/// clipped in axis space and stroked as a line is. Nothing when the series has no height.
fn outline_items(
    context: &MarkContext<'_>,
    (x, y): (&ResolvedAxis, &ResolvedAxis),
    bins: &HistBins,
    g: usize,
    (color, width): (Rgba, f32),
    items: &mut Vec<Item>,
) {
    let heights = bins.heights(g);
    if heights.iter().all(|&height| height == 0.0) {
        return;
    }
    let base = BarFrame::square(context.maps, y, context.baseline.pos).base();
    let level = |height: f64| {
        if height > 0.0 {
            axis_value(y, height)
        } else {
            base
        }
    };
    let n = heights.len();
    let mut xs = Vec::with_capacity(2 * n + 2);
    let mut ys = Vec::with_capacity(2 * n + 2);
    xs.push(axis_value(x, bins.edge(0)));
    ys.push(base);
    for (i, &height) in heights.iter().enumerate() {
        let level = level(height);
        xs.extend([axis_value(x, bins.edge(i)), axis_value(x, bins.edge(i + 1))]);
        ys.extend([level, level]);
    }
    xs.push(axis_value(x, bins.edge(n)));
    ys.push(base);
    line_items(
        context,
        &xs,
        &ys,
        &line_style(width, color, Dash::Solid),
        items,
    );
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
