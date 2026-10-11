//! Line marks: M4 downsampling over x, clipping in axis space, then the stroke and, with
//! `AreaFill::Gradient`, the area fill under it.

use super::MarkContext;
use super::area::area_fills;
use super::clip::{Bounds, clip_polyline};
use super::mapping::PlotPoint;
use super::stroke::stroke_width;
use crate::geometry::Path;
use crate::layout::Area;
use crate::m4_indices;
use crate::scene::{Item, LineCap, LineJoin, StrokeStyle};
use crate::{AreaFill, Dash, Rgba, Theme};

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
    stroke_width(theme.line_width())
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

/// The scene items of one line, given in axis space: the area fill (with
/// `AreaFill::Gradient`), then the stroke. The line is clipped in axis space and only the
/// clipped points are mapped to plot coordinates.
pub(super) fn line_items(
    context: &MarkContext<'_>,
    xs: &[f64],
    ys: &[f64],
    color: Rgba,
    dash: Dash,
    items: &mut Vec<Item>,
) {
    let MarkContext {
        theme,
        maps,
        plot,
        baseline_y,
    } = *context;
    let margin = f64::from(line_width(theme)).max(MIN_LINE_MARGIN);
    let ((left, right), (top, bottom)) = (maps.x.expanded(margin), maps.y.expanded(margin));
    let bounds = Bounds {
        left,
        top,
        right,
        bottom,
    };
    // Clipped points are inside `bounds`, so they map to within `margin` of the plot area. The
    // clamp only guards against rounding in the map.
    let limit = PlotLimit::around(plot, margin);
    let subpaths: Vec<Vec<PlotPoint>> = clip_polyline(xs, ys, &bounds)
        .into_iter()
        .map(|subpath| {
            subpath
                .into_iter()
                .map(|p| limit.clamp(maps.to_plot(p)))
                .collect()
        })
        .collect();
    if subpaths.is_empty() {
        return;
    }

    if theme.area_fill() == AreaFill::Gradient {
        for subpath in &subpaths {
            items.extend(area_fills(subpath, baseline_y, color));
        }
    }

    let mut path = Path::new();
    for subpath in &subpaths {
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
    items.push(Item::Stroke {
        path,
        style: line_style(theme, color, dash),
    });
}
