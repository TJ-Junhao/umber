//! Line and point clipping in axis space.
//!
//! Line clipping must stay accurate for any finite `f64` endpoints, including data far outside
//! narrow limits. Each boundary crossing fixes the crossing coordinate to the boundary value
//! exactly. The other coordinate is found from a reference value `r` inside the clip range: the
//! line's implicit equation is evaluated at `r` exactly (error-free `f64` expansion arithmetic,
//! or exact binary fractions when the operands' magnitudes are too far apart for that), and one
//! division gives the offset from `r` to the crossing. The rounding error therefore scales
//! with that offset (at most the clip rectangle's size for an accepted crossing), not with the
//! endpoints' magnitude, even when both endpoints are astronomically far away.

use super::mapping::AxisPoint;
use crate::exact::{Dyadic, cross_difference, group_scale, two_sum};

/// Crossings whose computed coordinate is outside the clip rectangle by at most this share of
/// the rectangle's size (about 4e-10 pt on a 400 pt plot) count as on it; their rounding error
/// is far smaller. A segment that misses a corner by less than this is therefore reported as
/// touching it (a zero-length visible part). The clip rectangle is the plot area expanded by
/// the line margin, and that touch point lies outside the plot clip, so nothing visible is
/// drawn.
const CROSSING_TOLERANCE: f64 = 1.0e-12;

/// An axis-aligned clip rectangle in axis space. `left..=right` is the x range and
/// `top..=bottom` the y range, both increasing; on the y axis that is bottom up on the page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Bounds {
    pub(super) left: f64,
    pub(super) top: f64,
    pub(super) right: f64,
    pub(super) bottom: f64,
}

impl Bounds {
    pub(super) fn contains(&self, p: AxisPoint) -> bool {
        (self.left..=self.right).contains(&p.x) && (self.top..=self.bottom).contains(&p.y)
    }
}

/// Cohen–Sutherland outcode of `p`: bit 0 left, 1 right, 2 above, 3 below.
fn outcode(p: AxisPoint, bounds: &Bounds) -> u8 {
    let mut code = 0;
    if p.x < bounds.left {
        code |= 1;
    } else if p.x > bounds.right {
        code |= 2;
    }
    if p.y < bounds.top {
        code |= 4;
    } else if p.y > bounds.bottom {
        code |= 8;
    }
    code
}

/// The visible part `(start, end)` of the segment `a..b` inside `bounds`, in the direction
/// from `a` to `b`, or `None` when no part is visible. Endpoints inside `bounds` are returned
/// unchanged; every other returned point lies on the boundary of `bounds`. The endpoints must
/// be finite.
///
/// The candidates are the inside endpoints plus the crossings with the four boundary lines
/// that lie on the boundary, ordered along the segment; the first and last are the visible
/// part.
pub(super) fn clip_segment(
    a: AxisPoint,
    b: AxisPoint,
    bounds: &Bounds,
) -> Option<(AxisPoint, AxisPoint)> {
    if outcode(a, bounds) & outcode(b, bounds) != 0 {
        return None;
    }
    let (inside_a, inside_b) = (bounds.contains(a), bounds.contains(b));
    if inside_a && inside_b {
        return Some((a, b));
    }
    let mut candidates: Vec<AxisPoint> = Vec::with_capacity(6);
    if inside_a {
        candidates.push(a);
    }
    if inside_b {
        candidates.push(b);
    }
    let (lo_x, hi_x, lo_y, hi_y) = (bounds.left, bounds.right, bounds.top, bounds.bottom);
    let mid_x = lo_x / 2.0 + hi_x / 2.0;
    let mid_y = lo_y / 2.0 + hi_y / 2.0;
    let on = |v: f64, lo: f64, hi: f64| {
        let tolerance = CROSSING_TOLERANCE * (hi - lo);
        (lo - tolerance..=hi + tolerance).contains(&v)
    };
    for edge in [lo_x, hi_x] {
        if let Some(y) = crossing((a.x, a.y), (b.x, b.y), edge, mid_y)
            && on(y, lo_y, hi_y)
        {
            candidates.push(AxisPoint::new(edge, y.clamp(lo_y, hi_y)));
        }
    }
    for edge in [lo_y, hi_y] {
        if let Some(x) = crossing((a.y, a.x), (b.y, b.x), edge, mid_x)
            && on(x, lo_x, hi_x)
        {
            candidates.push(AxisPoint::new(x.clamp(lo_x, hi_x), edge));
        }
    }
    // Order along the segment. Two candidates are compared on the axis where they are further
    // apart relative to the clip rectangle's size: in axis space, one axis can be many orders
    // of magnitude finer than the other, so the axis of the larger coordinate change may not
    // separate them.
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (width, height) = (hi_x - lo_x, hi_y - lo_y);
    let along = |p: &AxisPoint, q: &AxisPoint| {
        let (ex, ey) = (p.x - q.x, p.y - q.y);
        let forward = if ex.abs() / width >= ey.abs() / height {
            ex * dx.signum()
        } else {
            ey * dy.signum()
        };
        forward.total_cmp(&0.0)
    };
    let start = candidates.iter().copied().min_by(along)?;
    let end = candidates.iter().copied().max_by(along)?;
    Some((start, end))
}

/// Where the segment `a..b` crosses the vertical line `x = edge`: the y there, or `None` when
/// the segment does not reach the line or runs along it.
///
/// This is the numeric kernel shared by every coordinate space, so it takes plain `(x, y)`
/// coordinate pairs: [`clip_segment`] passes axis-space points, swapped for the horizontal
/// edges, and the area fills pass plot points swapped to cross the baseline. Callers keep the
/// coordinates of one segment in one space.
///
/// With `h(y) = (edge - ax)(by - ay) - (y - ay)(bx - ax)`, the crossing is
/// `y* = r + h(r) / (bx - ax)` for any `r`. `h(r)` is evaluated exactly and rounded about once,
/// so the error is a few ulps of `y* - r`, whatever the magnitude of the endpoints.
///
/// The fast path scales the x operands (`ax`, `bx`, `edge`) and the y operands (`ay`, `by`,
/// `r`) each by a power of two into `2^-500..=2^500`, which keeps every product finite, and
/// uses error-free `f64` arithmetic. That is exact unless an operand underflows when scaled or
/// a product residual underflows; those cases (operands of one axis differing by more than
/// about `2^1000`, or tiny values of both axes) are detected and computed with exact
/// binary-fraction arithmetic instead.
pub(super) fn crossing(a: (f64, f64), b: (f64, f64), edge: f64, r: f64) -> Option<f64> {
    if a.0 == b.0 || edge < a.0.min(b.0) || edge > a.0.max(b.0) {
        return None;
    }
    let offset = crossing_offset_fast(a, b, edge, r).unwrap_or_else(|| {
        // h(r) and bx - ax exactly; the division rounds once more.
        let d = Dyadic::from_f64;
        let (ax, ay, bx, by) = (d(a.0), d(a.1), d(b.0), d(b.1));
        let dx = bx.sub(&ax);
        let h = d(edge)
            .sub(&ax)
            .mul(&by.sub(&ay))
            .sub(&d(r).sub(&ay).mul(&dx));
        h.div_to_f64(&dx).unwrap_or(f64::NAN)
    });
    let y = r + offset;
    y.is_finite().then_some(y)
}

/// `h(r) / (bx - ax)` from [`crossing`] in scaled error-free `f64` arithmetic, or `None` when
/// that would not be exact.
pub(super) fn crossing_offset_fast(a: (f64, f64), b: (f64, f64), edge: f64, r: f64) -> Option<f64> {
    let sx = group_scale(a.0.abs().max(b.0.abs()).max(edge.abs()));
    let sy = group_scale(a.1.abs().max(b.1.abs()).max(r.abs()));
    // Scaling by a power of two is exact unless the result underflows.
    let scaled = |v: f64, s: f64| {
        let t = v * s;
        (t / s == v).then_some(t)
    };
    let (ax, bx, edge_s) = (scaled(a.0, sx)?, scaled(b.0, sx)?, scaled(edge, sx)?);
    let (ay, by, r_s) = (scaled(a.1, sy)?, scaled(b.1, sy)?, scaled(r, sy)?);
    let h = cross_difference(
        two_sum(edge_s, -ax),
        two_sum(by, -ay),
        two_sum(r_s, -ay),
        two_sum(bx, -ax),
    )?;
    // `h` carries `sx * sy` and `bx - ax` carries `sx`: undo `sy`.
    Some(h / (bx - ax) / sy)
}

/// The closed ring `ring` (its first point not repeated) clipped to `bounds`, one side of the
/// rectangle at a time (Sutherland–Hodgman): the points inside are kept, and a point is added
/// where an edge crosses the side, computed as [`crossing`] computes it. Inside `bounds` the
/// result winds round every point as often as the ring does, so it fills the same part of the
/// rectangle under either fill rule; where the ring leaves and comes back, the result runs along
/// the rectangle's edge, enclosing nothing there. Empty when no part of the ring is inside.
pub(super) fn clip_ring(ring: &[AxisPoint], bounds: &Bounds) -> Vec<AxisPoint> {
    /// The side of the rectangle that one pass clips against.
    #[derive(Clone, Copy)]
    enum Edge {
        Left,
        Right,
        Top,
        Bottom,
    }
    let mid_x = bounds.left / 2.0 + bounds.right / 2.0;
    let mid_y = bounds.top / 2.0 + bounds.bottom / 2.0;
    let mut points = ring.to_vec();
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        let inside = |p: AxisPoint| match edge {
            Edge::Left => p.x >= bounds.left,
            Edge::Right => p.x <= bounds.right,
            Edge::Top => p.y >= bounds.top,
            Edge::Bottom => p.y <= bounds.bottom,
        };
        // Where the edge from `a` to `b`, one end inside and one outside, crosses the side.
        let cross = |a: AxisPoint, b: AxisPoint| match edge {
            Edge::Left | Edge::Right => {
                let x = if matches!(edge, Edge::Left) {
                    bounds.left
                } else {
                    bounds.right
                };
                let y = crossing((a.x, a.y), (b.x, b.y), x, mid_y).unwrap_or(a.y);
                AxisPoint::new(x, y)
            }
            Edge::Top | Edge::Bottom => {
                let y = if matches!(edge, Edge::Top) {
                    bounds.top
                } else {
                    bounds.bottom
                };
                let x = crossing((a.y, a.x), (b.y, b.x), y, mid_x).unwrap_or(a.x);
                AxisPoint::new(x, y)
            }
        };
        let n = points.len();
        let mut kept = Vec::with_capacity(n + 4);
        for (i, &current) in points.iter().enumerate() {
            let Some(&previous) = points.get((i + n - 1) % n) else {
                continue;
            };
            match (inside(previous), inside(current)) {
                (true, true) => kept.push(current),
                (true, false) => kept.push(cross(previous, current)),
                (false, true) => {
                    kept.push(cross(previous, current));
                    kept.push(current);
                }
                (false, false) => {}
            }
        }
        points = kept;
        if points.is_empty() {
            break;
        }
    }
    points
}

/// Splits the polyline through `(xs[i], ys[i])` at NaN and clips it to `bounds`. Each returned
/// subpath has at least one point; a single point is an isolated data point inside `bounds`.
/// A segment that leaves `bounds` ends its subpath, and the part that re-enters starts a new
/// one.
pub(super) fn clip_polyline(xs: &[f64], ys: &[f64], bounds: &Bounds) -> Vec<Vec<AxisPoint>> {
    let points: Vec<AxisPoint> = xs
        .iter()
        .copied()
        .zip(ys.iter().copied())
        .map(|(x, y)| AxisPoint::new(x, y))
        .collect();
    let mut out = Vec::new();
    for run in points.split(|p| p.x.is_nan() || p.y.is_nan()) {
        match run {
            [] => {}
            [p] => {
                if bounds.contains(*p) {
                    out.push(vec![*p]);
                }
            }
            _ => clip_run(run, bounds, &mut out),
        }
    }
    out
}

/// Clips one NaN-free run of at least two points, appending its visible subpaths to `out`.
fn clip_run(run: &[AxisPoint], bounds: &Bounds, out: &mut Vec<Vec<AxisPoint>>) {
    let mut current: Vec<AxisPoint> = Vec::new();
    for pair in run.windows(2) {
        let [a, b] = pair else { continue };
        let (a, b) = (*a, *b);
        match clip_segment(a, b, bounds) {
            None => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            Some((start, end)) => {
                // A segment continues the current subpath only if it starts at its own
                // (unchanged, inside) first point, which the previous segment ended at.
                if current.is_empty() || start != a {
                    if !current.is_empty() {
                        out.push(std::mem::take(&mut current));
                    }
                    current.push(start);
                }
                current.push(end);
                if end != b {
                    out.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
}

/// A point of a clamped line (see [`clamp_polyline`]): where it is drawn, inside the bounds,
/// and the y of the line itself there, which lies beyond the bounds where the line does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ClampedPoint {
    /// The point, with its y clamped into the bounds.
    pub(super) at: AxisPoint,
    /// The line's own y at `at.x`, before clamping.
    pub(super) line_y: f64,
}

/// Splits the polyline through `(xs[i], ys[i])` at NaN and turns each run into the boundary of
/// the region between the line and any level inside the y range of `bounds`, as far as `bounds`
/// shows it. Each segment is cut to the x range of `bounds` as [`clip_segment`] cuts it, then
/// clamped into the y range, with a point inserted where it crosses the top or the bottom
/// edge, so a part of the line beyond the y range runs along that edge instead of being
/// dropped. Each point keeps the line's own y as well. A piece ends only where the line leaves
/// the x range; a single point is never a piece.
///
/// Clamping the line is exact for that region: for a level `b` inside the y range, the region
/// between `y = f(x)` and `b`, cut to the y range, is the region between `clamp(f(x))` and `b`.
/// Segments inside `bounds` are returned unchanged, and a segment that only crosses a side edge
/// gets the crossing [`clip_segment`] computes, so a line inside the y range gives the points
/// of [`clip_polyline`].
pub(super) fn clamp_polyline(xs: &[f64], ys: &[f64], bounds: &Bounds) -> Vec<Vec<ClampedPoint>> {
    let points: Vec<AxisPoint> = xs
        .iter()
        .copied()
        .zip(ys.iter().copied())
        .map(|(x, y)| AxisPoint::new(x, y))
        .collect();
    let mut out = Vec::new();
    for run in points.split(|p| p.x.is_nan() || p.y.is_nan()) {
        let mut current: Vec<ClampedPoint> = Vec::new();
        for pair in run.windows(2) {
            let [a, b] = pair else { continue };
            let Some(along) = clamp_segment(*a, *b, bounds) else {
                if current.len() > 1 {
                    out.push(std::mem::take(&mut current));
                }
                current.clear();
                continue;
            };
            let (Some(&start), Some(&end)) = (along.first(), along.last()) else {
                continue;
            };
            // A segment continues the current piece only if it starts where the previous one
            // ended: at its own first point, inside the x range.
            if current.last() != Some(&start) {
                if current.len() > 1 {
                    out.push(std::mem::take(&mut current));
                }
                current.clear();
                current.push(start);
            }
            current.extend(along.iter().skip(1));
            // An end outside the x range was cut there: the line leaves the region.
            if end.at.x != b.x {
                if current.len() > 1 {
                    out.push(std::mem::take(&mut current));
                }
                current.clear();
            }
        }
        if current.len() > 1 {
            out.push(current);
        }
    }
    out
}

/// The segment `a..b` cut to the x range of `bounds` and clamped into its y range, as the
/// points along it: the cut ends, with their y clamped, and between them the crossings with the
/// top and bottom edges. `None` when no part lies inside the x range.
fn clamp_segment(a: AxisPoint, b: AxisPoint, bounds: &Bounds) -> Option<Vec<ClampedPoint>> {
    let (lo_x, hi_x, lo_y, hi_y) = (bounds.left, bounds.right, bounds.top, bounds.bottom);
    if (a.x < lo_x && b.x < lo_x) || (a.x > hi_x && b.x > hi_x) {
        return None;
    }
    let mid_x = lo_x / 2.0 + hi_x / 2.0;
    let mid_y = lo_y / 2.0 + hi_y / 2.0;
    // The cut ends: an end outside the x range moves to its crossing with that side edge.
    let cut = |p: AxisPoint| {
        let edge = if p.x < lo_x {
            lo_x
        } else if p.x > hi_x {
            hi_x
        } else {
            return Some(p);
        };
        crossing((a.x, a.y), (b.x, b.y), edge, mid_y).map(|y| AxisPoint::new(edge, y))
    };
    let (start, end) = (cut(a)?, cut(b)?);
    let clamp = |p: AxisPoint| ClampedPoint {
        at: AxisPoint::new(p.x, p.y.clamp(lo_y, hi_y)),
        line_y: p.y,
    };
    let mut along = vec![clamp(start)];
    // The edges strictly between the ends' y values, in order along the segment.
    let (low, high) = (start.y.min(end.y), start.y.max(end.y));
    let edges = if start.y <= end.y {
        [lo_y, hi_y]
    } else {
        [hi_y, lo_y]
    };
    let (min_x, max_x) = (start.x.min(end.x), start.x.max(end.x));
    for edge in edges {
        if low < edge
            && edge < high
            && let Some(x) = crossing((a.y, a.x), (b.y, b.x), edge, mid_x)
        {
            let p = clamp(AxisPoint::new(x.clamp(min_x, max_x), edge));
            if along.last().map(|q| q.at) != Some(p.at) {
                along.push(p);
            }
        }
    }
    // The end replaces a crossing at its own position, keeping the line's y beyond the edge.
    let last = clamp(end);
    match along.as_mut_slice() {
        [_, .., previous] if previous.at == last.at => *previous = last,
        _ => along.push(last),
    }
    Some(along)
}
