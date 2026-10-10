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

mod expansion;

use crate::exact::Dyadic;
use crate::layout::Area;
use expansion::{cross_difference, group_scale, two_sum};

/// Crossings whose computed coordinate is outside the clip rectangle by at most this share of
/// the rectangle's size (about 4e-10 pt on a 400 pt plot) count as on it; their rounding error
/// is far smaller. A segment that misses a corner by less than this is therefore reported as
/// touching it (a zero-length visible part). The clip rectangle is the plot area expanded by
/// the line margin, and that touch point lies outside the plot clip, so nothing visible is
/// drawn.
const CROSSING_TOLERANCE: f64 = 1.0e-12;

/// An axis-aligned rectangle in `f64` points, for clipping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Bounds {
    pub(super) left: f64,
    pub(super) top: f64,
    pub(super) right: f64,
    pub(super) bottom: f64,
}

impl Bounds {
    /// `area` expanded by `margin` on every side.
    pub(super) fn around(area: Area, margin: f64) -> Bounds {
        Bounds {
            left: area.x - margin,
            top: area.y - margin,
            right: area.right() + margin,
            bottom: area.bottom() + margin,
        }
    }

    pub(super) fn contains(&self, (x, y): (f64, f64)) -> bool {
        (self.left..=self.right).contains(&x) && (self.top..=self.bottom).contains(&y)
    }
}

/// A point as `(x, y)`, in axis space or in plot points.
pub(super) type Pt = (f64, f64);

/// Cohen–Sutherland outcode of `p`: bit 0 left, 1 right, 2 above, 3 below.
fn outcode(p: Pt, bounds: &Bounds) -> u8 {
    let mut code = 0;
    if p.0 < bounds.left {
        code |= 1;
    } else if p.0 > bounds.right {
        code |= 2;
    }
    if p.1 < bounds.top {
        code |= 4;
    } else if p.1 > bounds.bottom {
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
pub(super) fn clip_segment(a: Pt, b: Pt, bounds: &Bounds) -> Option<(Pt, Pt)> {
    if outcode(a, bounds) & outcode(b, bounds) != 0 {
        return None;
    }
    let (inside_a, inside_b) = (bounds.contains(a), bounds.contains(b));
    if inside_a && inside_b {
        return Some((a, b));
    }
    let mut candidates: Vec<Pt> = Vec::with_capacity(6);
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
        if let Some(y) = crossing(a, b, edge, mid_y)
            && on(y, lo_y, hi_y)
        {
            candidates.push((edge, y.clamp(lo_y, hi_y)));
        }
    }
    let swap = |p: Pt| (p.1, p.0);
    for edge in [lo_y, hi_y] {
        if let Some(x) = crossing(swap(a), swap(b), edge, mid_x)
            && on(x, lo_x, hi_x)
        {
            candidates.push((x.clamp(lo_x, hi_x), edge));
        }
    }
    // Order along the segment. Two candidates are compared on the axis where they are further
    // apart relative to the clip rectangle's size: in axis space, one axis can be many orders
    // of magnitude finer than the other, so the axis of the larger coordinate change may not
    // separate them.
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (width, height) = (hi_x - lo_x, hi_y - lo_y);
    let along = |p: &Pt, q: &Pt| {
        let (ex, ey) = (p.0 - q.0, p.1 - q.1);
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
pub(super) fn crossing(a: Pt, b: Pt, edge: f64, r: f64) -> Option<f64> {
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
pub(super) fn crossing_offset_fast(a: Pt, b: Pt, edge: f64, r: f64) -> Option<f64> {
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

/// Splits the polyline through `(xs[i], ys[i])` at NaN and clips it to `bounds`. Each returned
/// subpath has at least one point; a single point is an isolated data point inside `bounds`.
/// A segment that leaves `bounds` ends its subpath, and the part that re-enters starts a new
/// one.
pub(super) fn clip_polyline(xs: &[f64], ys: &[f64], bounds: &Bounds) -> Vec<Vec<Pt>> {
    let points: Vec<Pt> = xs.iter().copied().zip(ys.iter().copied()).collect();
    let mut out = Vec::new();
    for run in points.split(|p| p.0.is_nan() || p.1.is_nan()) {
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
fn clip_run(run: &[Pt], bounds: &Bounds, out: &mut Vec<Vec<Pt>>) {
    let mut current: Vec<Pt> = Vec::new();
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
