//! Reference lines and spans: lines and filled regions that run across the whole plot along one
//! axis, drawn in two layers of the marks group, spans below the data marks and lines above them
//! (see [`span_layer`] and [`rule_layer`]).
//!
//! A reference mark sits at a value of one axis and covers the plot area on the other, so it is
//! clipped in axis space against the one axis it has a value on. Nothing is mapped to plot
//! points before it is known to be visible.

use super::MarkContext;
use super::area::AREA_ALPHA;
use super::chrome::segment;
use super::line::line_style;
use super::mapping::{AxisMap, PlotPoint, axis_x, axis_y_value};
use crate::geometry::Path;
use crate::model::{AxisData, AxisValue};
use crate::resolve::{Resolved, ResolvedMark};
use crate::scene::{FillRule, Item, LineCap, Paint};
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

/// The scene items of the spans among `marks`, in call order. They are the lowest layer of the
/// marks group, under the data marks.
pub(super) fn span_layer(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    marks: &[ResolvedMark<'_>],
    items: &mut Vec<Item>,
) {
    for mark in marks {
        match *mark {
            ResolvedMark::HSpan { mark, color } => {
                let ends = mark.bounds().iter().map(|&v| axis_y_value(&resolved.y, v));
                span_item(context, Along::Y, ends, color, items);
            }
            ResolvedMark::VSpan { mark, color } => {
                let ends = mark.bounds().iter().map(|&v| {
                    axis_x(&resolved.x, &AxisData::Numbers(vec![v]))
                        .first()
                        .copied()
                        .unwrap_or(f64::NAN)
                });
                span_item(context, Along::X, ends, color, items);
            }
            ResolvedMark::Line { .. }
            | ResolvedMark::Scatter { .. }
            | ResolvedMark::Stem { .. }
            | ResolvedMark::Area { .. }
            | ResolvedMark::StackedArea { .. }
            | ResolvedMark::Bar { .. }
            | ResolvedMark::Grouped { .. }
            | ResolvedMark::Stacked { .. }
            | ResolvedMark::Hist { .. }
            | ResolvedMark::Step { .. }
            | ResolvedMark::HLine { .. }
            | ResolvedMark::VLine { .. }
            | ResolvedMark::Band { .. }
            | ResolvedMark::ErrorBars { .. }
            | ResolvedMark::Box { .. } => {}
        }
    }
}

/// The scene items of the reference lines among `marks`, in call order. They are the highest
/// layer of the marks group, over the data marks.
pub(super) fn rule_layer(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    marks: &[ResolvedMark<'_>],
    items: &mut Vec<Item>,
) {
    for mark in marks {
        match *mark {
            ResolvedMark::HLine { mark, color, width } => {
                let at = axis_y_value(&resolved.y, mark.y());
                let style = rule_style(width, color, mark.options().dash);
                rule_item(context, Along::Y, at, &style, items);
            }
            ResolvedMark::VLine { mark, color, width } => {
                let at = vline_position(resolved, mark.x());
                let style = rule_style(width, color, mark.options().dash);
                rule_item(context, Along::X, at, &style, items);
            }
            ResolvedMark::Line { .. }
            | ResolvedMark::Scatter { .. }
            | ResolvedMark::Stem { .. }
            | ResolvedMark::Area { .. }
            | ResolvedMark::StackedArea { .. }
            | ResolvedMark::Bar { .. }
            | ResolvedMark::Grouped { .. }
            | ResolvedMark::Stacked { .. }
            | ResolvedMark::Hist { .. }
            | ResolvedMark::Step { .. }
            | ResolvedMark::HSpan { .. }
            | ResolvedMark::VSpan { .. }
            | ResolvedMark::Band { .. }
            | ResolvedMark::ErrorBars { .. }
            | ResolvedMark::Box { .. } => {}
        }
    }
}

/// The axis-space position of a vertical line at `x`: its value on a continuous axis, the band
/// centre of its category on a category axis. NaN for a name the axis does not have, which
/// rendering refuses before it gets here (see `resolve::check_references`).
fn vline_position(resolved: &Resolved, x: &AxisValue) -> f64 {
    let data = match x {
        AxisValue::Number(v) => AxisData::Numbers(vec![*v]),
        AxisValue::Category(name) => AxisData::Categories(vec![name.clone()]),
    };
    axis_x(&resolved.x, &data)
        .first()
        .copied()
        .unwrap_or(f64::NAN)
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
/// whole plot; nothing when no part of the stroke is visible. A line whose centre lies up to
/// half its width beyond the domain still shows its inner half, and is kept; one exactly at an
/// end of the domain is kept. A NaN `at` is not visible.
fn rule_item(
    context: &MarkContext<'_>,
    along: Along,
    at: f64,
    style: &crate::scene::StrokeStyle,
    items: &mut Vec<Item>,
) {
    let (low, high) = along.map(context).expanded(f64::from(style.width) / 2.0);
    if at.is_nan() || at < low || at > high {
        return;
    }
    let from = along.point(context, at, 0.0).to_scene();
    let to = along.point(context, at, 1.0).to_scene();
    items.push(Item::Stroke {
        path: segment(from, to),
        style: style.clone(),
    });
}

/// The fill of one reference span between the axis-space `ends` along `along`, across the whole
/// plot, in `color` at the area opacity. The span is clipped to the domain: an end beyond it
/// moves to the domain's end, so a span that covers the whole view fills the plot and one that
/// crosses an end fills up to it. Nothing is drawn when the clipped span is empty, as for a span
/// that only touches the domain at one end.
fn span_item(
    context: &MarkContext<'_>,
    along: Along,
    ends: impl Iterator<Item = f64>,
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
        along.point(context, low, 1.0),
        along.point(context, high, 1.0),
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
