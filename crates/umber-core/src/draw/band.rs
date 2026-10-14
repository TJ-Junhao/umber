//! Band marks: the region between a lower and an upper curve, filled flat at the area opacity
//! with no outline, whatever the theme's fill style. The curves are reduced per device column,
//! clipped in axis space like the line of an area, and each run of points that no gap breaks is
//! one polygon.

use super::MarkContext;
use super::area::AREA_ALPHA;
use super::clip::{ClampedPoint, clamp_polyline};
use super::line::{LineClip, downsample_columns};
use super::mapping::{PlotPoint, axis_x_value, axis_y};
use crate::Rgba;
use crate::geometry::Path;
use crate::m4_indices;
use crate::model::BandMark;
use crate::resolve::Resolved;
use crate::scene::{FillRule, Item, Paint};

/// The overlap with the plot, in points, at or below which a polygon only touches it.
const TOUCH: f64 = 1e-6;

/// The scene item of one band in `color`: the fill of all its polygons as one path with a
/// closed subpath each, and nothing when no part of the band is visible.
///
/// A polygon runs along the upper curve from left to right and back along the lower curve. A
/// NaN in x, low or high leaves no point there, so it ends one polygon and starts the next; a
/// run of a single point has no area and is not drawn. Both curves are clamped into the clip
/// as an area's line is, so a part of the band beyond the top or bottom of the plot still
/// fills the plot up to that edge, and a band that leaves the plot across a side starts a new
/// polygon where it comes back. A band of no height is not drawn.
///
/// On a continuous x axis the points are first reduced, per device column of the plot (see
/// [`kept_points`]), so a long band costs what a line of the same size costs.
pub(super) fn band_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &BandMark,
    color: Rgba,
    items: &mut Vec<Item>,
) {
    let (xs, lows, highs) = gapped_points(resolved, mark);
    let (xs, lows, highs) = kept_points(context, xs, lows, highs);
    let clip = LineClip::of(context, 0.0);
    let uppers = clamp_polyline(&xs, &highs, &clip.bounds);
    let lowers = clamp_polyline(&xs, &lows, &clip.bounds);
    let plot = context.plot;
    let mut path = Path::new();
    for (upper, lower) in uppers.iter().zip(&lowers) {
        // Both curves are cut where x leaves the clip, so their pieces pair up; a pair that
        // does not span the same x would not be a polygon.
        if !same_span(upper, lower) || is_flat(upper, lower) {
            continue;
        }
        let mut points: Vec<PlotPoint> = upper
            .iter()
            .chain(lower.iter().rev())
            .map(|p| clip.plot_point(p.at))
            .collect();
        // A polygon that only touches the plot, or lies in the margin around it, shows nothing.
        let (x0, x1) = extent(points.iter().map(|p| p.x));
        let (y0, y1) = extent(points.iter().map(|p| p.y));
        if x1.min(plot.right()) - x0.max(plot.x) <= TOUCH
            || y1.min(plot.bottom()) - y0.max(plot.y) <= TOUCH
        {
            continue;
        }
        // Every polygon is wound the same way, so that polygons of separate runs never cancel
        // each other under the non-zero rule, and where they overlap the fill is drawn once.
        if signed_area(&points) < 0.0 {
            points.reverse();
        }
        let mut scene = points.iter().map(|p| p.to_scene());
        let Some(first) = scene.next() else {
            continue;
        };
        path.move_to(first);
        scene.for_each(|p| path.line_to(p));
        path.close();
    }
    if !path.is_empty() {
        items.push(Item::Fill {
            path,
            paint: Paint::Solid(color.scale_alpha(AREA_ALPHA)),
            rule: FillRule::NonZero,
        });
    }
}

/// The smallest and largest of `values`.
fn extent(values: impl Iterator<Item = f64>) -> (f64, f64) {
    values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
        (lo.min(v), hi.max(v))
    })
}

/// Twice the signed area of the closed polygon through `points`; its sign is its winding.
fn signed_area(points: &[PlotPoint]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum()
}

/// The x, low and high values of `mark` in axis space, with NaN in all three wherever any of
/// them is NaN, so the three break at the same places.
fn gapped_points(resolved: &Resolved, mark: &BandMark) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut xs: Vec<f64> = mark
        .x()
        .iter()
        .map(|&v| axis_x_value(&resolved.x, v))
        .collect();
    let mut lows = axis_y(&resolved.y, mark.low());
    let mut highs = axis_y(&resolved.y, mark.high());
    for ((x, low), high) in xs.iter_mut().zip(&mut lows).zip(&mut highs) {
        if x.is_nan() || low.is_nan() || high.is_nan() {
            (*x, *low, *high) = (f64::NAN, f64::NAN, f64::NAN);
        }
    }
    (xs, lows, highs)
}

/// The points of the band `(xs, lows, highs)` that keep its envelope over the plot's device
/// columns; all of them when the band is short or its x is not sorted.
///
/// On a sorted x, the points inside the x domain are reduced, and of the points outside it
/// only the neighbour on each side is kept, so the segments that cross the limits stay exact
/// and points out of view never share a column with the first or last points in view.
///
/// M4 downsampling keeps, for each column and each run that no gap breaks, the first and last
/// point and the points of the lowest and the highest value of one curve. Applied to the
/// lower and to the upper curve, which break at the same places, the union of the points kept
/// holds the first and last point of every column and run, the point of the lowest low and the
/// point of the highest high, which is the envelope of the band at the resolution of the plot.
/// The union also holds the highest low and the lowest high, which lie inside the envelope.
fn kept_points(
    context: &MarkContext<'_>,
    xs: Vec<f64>,
    lows: Vec<f64>,
    highs: Vec<f64>,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let columns = downsample_columns(context.plot.width);
    let (lo, hi) = (context.maps.x.lo, context.maps.x.hi);
    let n = xs.len();
    let sorted = xs
        .iter()
        .filter(|x| !x.is_nan())
        .zip(xs.iter().filter(|x| !x.is_nan()).skip(1))
        .all(|(a, b)| a <= b);
    if n <= 4 * columns || !sorted {
        return (xs, lows, highs);
    }
    let first = xs.iter().position(|&x| x >= lo).unwrap_or(n);
    let end = xs.iter().position(|&x| x > hi).unwrap_or(n);
    let (first, end) = (first.min(end), end);
    let mut kept: Vec<usize> = first.checked_sub(1).into_iter().collect();
    let (sx, sl, sh) = (&xs[first..end], &lows[first..end], &highs[first..end]);
    let mut inside = m4_indices(sx, sl, (lo, hi), columns);
    inside.extend(m4_indices(sx, sh, (lo, hi), columns));
    inside.sort_unstable();
    inside.dedup();
    kept.extend(inside.into_iter().map(|i| first + i));
    kept.extend((end < n).then_some(end));
    let pick = |v: &[f64]| -> Vec<f64> { kept.iter().map(|&i| v[i]).collect() };
    (pick(&xs), pick(&lows), pick(&highs))
}

/// Whether two clamped pieces run over the same x, from the first point to the last.
fn same_span(upper: &[ClampedPoint], lower: &[ClampedPoint]) -> bool {
    let ends = |piece: &[ClampedPoint]| {
        piece
            .first()
            .zip(piece.last())
            .map(|(first, last)| (first.at.x, last.at.x))
    };
    ends(upper).is_some() && ends(upper) == ends(lower)
}

/// Whether every point of both pieces lies at one y: a polygon of no height, as a band
/// entirely beyond the top or the bottom of the plot is once clamped.
fn is_flat(upper: &[ClampedPoint], lower: &[ClampedPoint]) -> bool {
    let Some(first) = upper.first() else {
        return true;
    };
    upper.iter().chain(lower).all(|p| p.at.y == first.at.y)
}
