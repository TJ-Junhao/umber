//! Reference lines and spans: lines and filled regions that run across the whole plot along one
//! axis, drawn in two layers of the marks group, spans below the data marks and lines above them
//! (see [`span_layer`] and [`rule_layer`]).
//!
//! A reference mark sits at a value of one axis and covers the plot area on the other, so it is
//! clipped in axis space against the one axis it has a value on. Nothing is mapped to plot
//! points before it is known to be visible.
//!
//! When the baseline sits at the y axis's lower limit, only padding lies below it, so nothing of
//! a reference mark is drawn there: a vertical one ends at the baseline, and the layers are
//! clipped at it (see [`baseline_clip`]), which also cuts the half-stroke of a horizontal line
//! on the baseline and the round cap of a vertical one. When the baseline is at y = 0 inside
//! the domain, they cover the whole plot height. A chart whose values lie along x (a category y
//! axis beside a numeric x axis) mirrors this: its baseline is vertical, the layers are clipped
//! left of it when it sits at the x axis's lower limit, and its vertical lines and spans, which
//! run along the baseline, cover the whole plot height.

use super::MarkContext;
use super::chrome::segment;
use super::fill::AREA_ALPHA;
use super::line::line_style;
use super::mapping::{AxisMap, PlotPoint, axis_position, axis_time, axis_value};
use crate::geometry::Path;
use crate::resolve::{Resolved, ResolvedMark, ValueAxis};
use crate::scene::{FillRule, Item, LineCap, Paint, Rect};
use crate::{Dash, Rgba};

/// The axis a reference mark has its value on; it runs across the plot along the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Along {
    /// The value is an x: a vertical line, or a span between two x values.
    X,
    /// The value is a y: a horizontal line, or a span between two y values.
    Y,
}

impl Along {
    /// The map of the axis the value is on.
    fn map(self, context: &MarkContext<'_>) -> AxisMap {
        match self {
            Along::X => context.maps.x,
            Along::Y => context.maps.y,
        }
    }

    /// The fraction of the plot area, from its start on the other axis (left, or top), that a
    /// mark of this kind covers across it: all of it, except for a vertical mark on a chart with
    /// its values on y, which stops at the baseline when that sits at the y axis's lower limit.
    /// A mark whose value lies on the value axis runs along the baseline and covers it all. The
    /// model refuses horizontal marks beside a category y axis, the one chart with its baseline
    /// on x, so no mark there would stop at the baseline.
    fn reach(self, context: &MarkContext<'_>, resolved: &Resolved) -> f64 {
        match (self, context.baseline.axis) {
            (Along::Y, _) | (Along::X, ValueAxis::X) => 1.0,
            (Along::X, ValueAxis::Y) if zero_in_domain(context, resolved) => 1.0,
            (Along::X, ValueAxis::Y) => {
                ((context.baseline_at - context.plot.y) / context.plot.height).clamp(0.0, 1.0)
            }
        }
    }

    /// The plot point at `at` along the value's axis and at `across` along the other: `across`
    /// is `0` at the start of the plot area on the other axis (left, or top) and `1` at its end.
    fn point(self, context: &MarkContext<'_>, at: f64, across: f64) -> PlotPoint {
        let plot = context.plot;
        match self {
            Along::X => PlotPoint {
                x: context.maps.x.plot_at(at),
                y: plot.y + across * plot.height,
            },
            Along::Y => PlotPoint {
                x: plot.x + across * plot.width,
                y: context.maps.y.plot_at(at),
            },
        }
    }
}

/// Whether the baseline sits at 0 inside the domain of a linear value axis, rather than at the
/// lower limit.
fn zero_in_domain(context: &MarkContext<'_>, resolved: &Resolved) -> bool {
    match context.baseline.axis {
        ValueAxis::X => resolved.x.holds_zero(),
        ValueAxis::Y => resolved.y.holds_zero(),
    }
}

/// The clip of the reference layers: `marks_clip` cut at the baseline when that sits at the
/// value axis's lower limit inside the plot (below its top on y, right of its left edge on x),
/// `None` when nothing lies beyond it to cut.
pub(super) fn baseline_clip(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    marks_clip: Rect,
) -> Option<Rect> {
    if zero_in_domain(context, resolved) {
        return None;
    }
    let plot = context.plot;
    match context.baseline.axis {
        ValueAxis::Y => {
            let height = context.baseline_at - plot.y;
            (height < plot.height - 1e-6).then(|| Rect {
                height: height.max(0.0) as f32,
                ..marks_clip
            })
        }
        ValueAxis::X => {
            let left = context.baseline_at - plot.x;
            (left > 1e-6).then(|| {
                let right = f64::from(marks_clip.x) + f64::from(marks_clip.width);
                let x = context.baseline_at.min(right);
                Rect {
                    x: x as f32,
                    width: (right - x) as f32,
                    ..marks_clip
                }
            })
        }
    }
}

/// Adds `layer` to `items`, as one group clipped to `clip` when there is one.
fn push_layer(items: &mut Vec<Item>, clip: Option<Rect>, layer: Vec<Item>) {
    if layer.is_empty() {
        return;
    }
    match clip {
        Some(clip) => items.push(Item::Group {
            clip: Some(clip),
            items: layer,
        }),
        None => items.extend(layer),
    }
}

/// The scene items of the spans among `marks`, in call order. They are the lowest layer of the
/// marks group, under the data marks.
pub(super) fn span_layer(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    marks: &[ResolvedMark<'_>],
    clip: Option<Rect>,
    items: &mut Vec<Item>,
) {
    let mut layer = Vec::new();
    for mark in marks {
        match *mark {
            ResolvedMark::HSpan { mark, color, .. } => {
                let ends = mark.bounds().iter().map(|&v| axis_value(&resolved.y, v));
                let times = mark
                    .time_bounds()
                    .iter()
                    .map(|&t| axis_time(&resolved.y, t));
                span_item(context, Along::Y, ends.chain(times), 1.0, color, &mut layer);
            }
            ResolvedMark::VSpan { mark, color, .. } => {
                let ends = mark.bounds().iter().map(|&v| axis_value(&resolved.x, v));
                let times = mark
                    .time_bounds()
                    .iter()
                    .map(|&t| axis_time(&resolved.x, t));
                let reach = Along::X.reach(context, resolved);
                span_item(
                    context,
                    Along::X,
                    ends.chain(times),
                    reach,
                    color,
                    &mut layer,
                );
            }
            ResolvedMark::Line { .. }
            | ResolvedMark::Scatter { .. }
            | ResolvedMark::Bubbles { .. }
            | ResolvedMark::Stem { .. }
            | ResolvedMark::Area { .. }
            | ResolvedMark::StackedArea { .. }
            | ResolvedMark::Bar { .. }
            | ResolvedMark::Grouped { .. }
            | ResolvedMark::Stacked { .. }
            | ResolvedMark::Hist { .. }
            | ResolvedMark::Mesh { .. }
            | ResolvedMark::Hist2d { .. }
            | ResolvedMark::Hexbin { .. }
            | ResolvedMark::Contour { .. }
            | ResolvedMark::Step { .. }
            | ResolvedMark::HLine { .. }
            | ResolvedMark::VLine { .. }
            | ResolvedMark::Band { .. }
            | ResolvedMark::ErrorBars { .. }
            | ResolvedMark::Box { .. }
            | ResolvedMark::Violin { .. }
            | ResolvedMark::Heatmap { .. } => {}
        }
    }
    push_layer(items, clip, layer);
}

/// The scene items of the reference lines among `marks`, in call order. They are the highest
/// layer of the marks group, over the data marks.
pub(super) fn rule_layer(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    marks: &[ResolvedMark<'_>],
    clip: Option<Rect>,
    items: &mut Vec<Item>,
) {
    let mut layer = Vec::new();
    for mark in marks {
        match *mark {
            ResolvedMark::HLine {
                mark, color, width, ..
            } => {
                let at = axis_position(&resolved.y, mark.y());
                let style = rule_style(width, color, mark.options().dash);
                rule_item(context, Along::Y, at, 1.0, &style, &mut layer);
            }
            ResolvedMark::VLine {
                mark, color, width, ..
            } => {
                let at = axis_position(&resolved.x, mark.x());
                let style = rule_style(width, color, mark.options().dash);
                let reach = Along::X.reach(context, resolved);
                rule_item(context, Along::X, at, reach, &style, &mut layer);
            }
            ResolvedMark::Line { .. }
            | ResolvedMark::Scatter { .. }
            | ResolvedMark::Bubbles { .. }
            | ResolvedMark::Stem { .. }
            | ResolvedMark::Area { .. }
            | ResolvedMark::StackedArea { .. }
            | ResolvedMark::Bar { .. }
            | ResolvedMark::Grouped { .. }
            | ResolvedMark::Stacked { .. }
            | ResolvedMark::Hist { .. }
            | ResolvedMark::Mesh { .. }
            | ResolvedMark::Hist2d { .. }
            | ResolvedMark::Hexbin { .. }
            | ResolvedMark::Contour { .. }
            | ResolvedMark::Step { .. }
            | ResolvedMark::HSpan { .. }
            | ResolvedMark::VSpan { .. }
            | ResolvedMark::Band { .. }
            | ResolvedMark::ErrorBars { .. }
            | ResolvedMark::Box { .. }
            | ResolvedMark::Violin { .. }
            | ResolvedMark::Heatmap { .. } => {}
        }
    }
    push_layer(items, clip, layer);
}

/// The stroke of a reference line of `color` and `dash`, `width` pt wide. A line runs exactly
/// from edge to edge of the plot, so a solid one ends square; dashes and dots end as they do on
/// any line.
fn rule_style(width: f32, color: Rgba, dash: Dash) -> crate::scene::StrokeStyle {
    let mut style = line_style(width, color, dash);
    if dash == Dash::Solid {
        style.cap = LineCap::Butt;
    }
    style
}

/// The stroke of one reference line at the axis-space position `at` along `along`, across the
/// plot as far as the fraction `reach` of it (see [`Along::reach`]); nothing when no part of the stroke is visible. A line whose centre lies up to
/// half its width beyond the domain still shows its inner half, and is kept; one exactly at an
/// end of the domain is kept. A NaN `at` is not visible.
fn rule_item(
    context: &MarkContext<'_>,
    along: Along,
    at: f64,
    reach: f64,
    style: &crate::scene::StrokeStyle,
    items: &mut Vec<Item>,
) {
    let (low, high) = along.map(context).expanded(f64::from(style.width) / 2.0);
    if at.is_nan() || at < low || at > high {
        return;
    }
    let from = along.point(context, at, 0.0).to_scene();
    let to = along.point(context, at, reach).to_scene();
    items.push(Item::Stroke {
        path: segment(from, to),
        style: style.clone(),
    });
}

/// The fill of one reference span between the axis-space `ends` along `along`, across the plot
/// as far as the fraction `reach` of it (see [`Along::reach`]), in `color` at the area
/// opacity. The span is clipped to the domain: an end beyond it moves to the domain's end, so a span that covers the whole view fills the plot and one that
/// crosses an end fills up to it. Nothing is drawn when the clipped span is empty, as for a span
/// that only touches the domain at one end.
fn span_item(
    context: &MarkContext<'_>,
    along: Along,
    ends: impl Iterator<Item = f64>,
    reach: f64,
    color: Rgba,
    items: &mut Vec<Item>,
) {
    let map = along.map(context);
    let mut ends = ends;
    let (Some(low), Some(high)) = (ends.next(), ends.next()) else {
        return;
    };
    if low.is_nan() || high.is_nan() {
        return;
    }
    let (low, high) = (low.max(map.lo), high.min(map.hi));
    if low >= high {
        return;
    }
    let corners = [
        along.point(context, low, 0.0),
        along.point(context, low, reach),
        along.point(context, high, reach),
        along.point(context, high, 0.0),
    ];
    let mut path = Path::new();
    path.move_to(corners[0].to_scene());
    for corner in &corners[1..] {
        path.line_to(corner.to_scene());
    }
    path.close();
    items.push(Item::Fill {
        path,
        paint: Paint::Solid(color.scale_alpha(AREA_ALPHA)),
        rule: FillRule::NonZero,
    });
}
