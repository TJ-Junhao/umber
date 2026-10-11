//! Line marks: M4 downsampling over x, clipping in axis space, then the stroke. Lines are never
//! filled; the [`area`](super::area) mark draws its line with these same steps.

use super::MarkContext;
use super::clip::{Bounds, clip_polyline};
use super::mapping::{AxisMaps, AxisPoint, PlotPoint, axis_x, axis_y};
use crate::geometry::Path;
use crate::layout::Area;
use crate::m4_indices;
use crate::model::LineMark;
use crate::resolve::Resolved;
use crate::scene::{Item, LineCap, LineJoin, StrokeStyle};
use crate::{Dash, Rgba, Theme};

/// The smallest line clip margin, in points.
const MIN_LINE_MARGIN: f64 = 2.0;
/// The nominal device resolution that sets the downsampling column count.
const DOWNSAMPLE_DPI: f64 = 300.0;

/// The number of device columns the downsampler works with for a plot `width` pt wide.
pub(super) fn downsample_columns(width: f64) -> usize {
    let columns = (width / 72.0 * DOWNSAMPLE_DPI).ceil();
    if columns.is_finite() && columns >= 1.0 {
        // Figures are at most 200 in wide, so this is far below `usize::MAX`.
        columns as usize
    } else {
        1
    }
}

/// The points of `line` in axis space, reduced by [`downsample`] on a continuous x axis.
pub(super) fn line_points(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    line: &LineMark,
) -> (Vec<f64>, Vec<f64>) {
    let xs = axis_x(&resolved.x, line.x());
    let ys = axis_y(&resolved.y, line.y());
    if resolved.x.is_category() {
        (xs, ys)
    } else {
        downsample(context, xs, ys)
    }
}

/// The axis-space line `(xs, ys)` reduced to the points that M4 downsampling keeps over the
/// plot's device columns; unchanged when every point is kept. Only for a continuous x axis: it
/// works over x in axis space, which maps to plot x monotonically and affinely.
pub(super) fn downsample(
    context: &MarkContext<'_>,
    xs: Vec<f64>,
    ys: Vec<f64>,
) -> (Vec<f64>, Vec<f64>) {
    let columns = downsample_columns(context.plot.width);
    let xm = context.maps.x;
    let kept = m4_indices(&xs, &ys, (xm.lo, xm.hi), columns);
    if kept.len() < xs.len() {
        let pick =
            |v: &[f64]| -> Vec<f64> { kept.iter().filter_map(|&i| v.get(i).copied()).collect() };
        (pick(&xs), pick(&ys))
    } else {
        (xs, ys)
    }
}

/// A rectangle in plot points that mapped line points are clamped into.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PlotLimit {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl PlotLimit {
    /// `area` expanded by `margin` on every side.
    fn around(area: Area, margin: f64) -> PlotLimit {
        PlotLimit {
            left: area.x - margin,
            top: area.y - margin,
            right: area.right() + margin,
            bottom: area.bottom() + margin,
        }
    }

    /// `p` clamped into this rectangle.
    fn clamp(&self, p: PlotPoint) -> PlotPoint {
        PlotPoint {
            x: p.x.clamp(self.left, self.right),
            y: p.y.clamp(self.top, self.bottom),
        }
    }
}

/// The stroke width of a line mark, in points: the theme's line width, kept finite.
pub(super) fn line_width(theme: &Theme) -> f32 {
    theme.drawn_line_width()
}

/// The stroke of a line mark of `color` and `dash` at the theme's line width.
pub(super) fn line_style(theme: &Theme, color: Rgba, dash: Dash) -> StrokeStyle {
    let w = line_width(theme);
    let (dash, cap) = match dash {
        Dash::Solid => (Vec::new(), LineCap::Round),
        Dash::Dashed => (vec![4.0 * w, 3.0 * w], LineCap::Butt),
        Dash::Dotted => (vec![0.0, 2.5 * w], LineCap::Round),
    };
    StrokeStyle {
        color,
        width: w,
        dash,
        cap,
        join: LineJoin::Round,
    }
}

/// The scene item of one line, given in axis space: its stroke, and nothing when no part of
/// the line is visible. See [`plot_subpaths`] for the clipping.
pub(super) fn line_items(
    context: &MarkContext<'_>,
    xs: &[f64],
    ys: &[f64],
    color: Rgba,
    dash: Dash,
    items: &mut Vec<Item>,
) {
    let subpaths = plot_subpaths(context, xs, ys);
    if !subpaths.is_empty() {
        items.push(stroke(context.theme, &subpaths, color, dash));
    }
}

/// The visible runs of the axis-space line `(xs, ys)` in plot points, one subpath per run that
/// NaN or the clip leaves. The line is clipped in axis space and only the clipped points are
/// mapped to plot coordinates.
pub(super) fn plot_subpaths(
    context: &MarkContext<'_>,
    xs: &[f64],
    ys: &[f64],
) -> Vec<Vec<PlotPoint>> {
    let clip = LineClip::of(context);
    clip.to_plot(clip_polyline(xs, ys, &clip.bounds))
}

/// The axis-space clip of a line, the domain expanded by `max(line width, 2)` pt, with the map
/// of clipped pieces to plot points.
pub(super) struct LineClip {
    /// The clip rectangle in axis space.
    pub(super) bounds: Bounds,
    /// The axis maps.
    maps: AxisMaps,
    /// The plot area expanded by the same margin, which mapped points are clamped into.
    limit: PlotLimit,
}

impl LineClip {
    /// The line clip of `context`.
    pub(super) fn of(context: &MarkContext<'_>) -> LineClip {
        let MarkContext {
            theme, maps, plot, ..
        } = *context;
        let margin = f64::from(line_width(theme)).max(MIN_LINE_MARGIN);
        let ((left, right), (top, bottom)) = (maps.x.expanded(margin), maps.y.expanded(margin));
        LineClip {
            bounds: Bounds {
                left,
                top,
                right,
                bottom,
            },
            maps,
            limit: PlotLimit::around(plot, margin),
        }
    }

    /// The plot point of `p`, a point inside the bounds. Such points map to within the margin
    /// of the plot area; the clamp only guards against rounding in the map.
    pub(super) fn plot_point(&self, p: AxisPoint) -> PlotPoint {
        self.limit.clamp(self.maps.to_plot(p))
    }

    /// `pieces`, clipped to the bounds, in plot points.
    fn to_plot(&self, pieces: Vec<Vec<AxisPoint>>) -> Vec<Vec<PlotPoint>> {
        pieces
            .into_iter()
            .map(|piece| piece.into_iter().map(|p| self.plot_point(p)).collect())
            .collect()
    }
}

/// The stroke through the plot-point `subpaths`, in `color` and `dash` at the theme's line
/// width.
pub(super) fn stroke(theme: &Theme, subpaths: &[Vec<PlotPoint>], color: Rgba, dash: Dash) -> Item {
    let mut path = Path::new();
    for subpath in subpaths {
        let mut points = subpath.iter().map(|p| p.to_scene());
        let Some(first) = points.next() else {
            continue;
        };
        path.move_to(first);
        match points.next() {
            // An isolated point: a zero-length subpath, which the round cap draws as a dot.
            None => path.line_to(first),
            Some(second) => {
                path.line_to(second);
                points.for_each(|p| path.line_to(p));
            }
        }
    }
    Item::Stroke {
        path,
        style: line_style(theme, color, dash),
    }
}
