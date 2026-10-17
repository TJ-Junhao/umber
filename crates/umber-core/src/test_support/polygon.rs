//! Plane geometry of polygons given as lists of corners, for tests that check drawn outlines.

/// A point in plot points.
pub(crate) type Pt = (f64, f64);

/// The distance from `p` to the segment from `a` to `b`.
pub(crate) fn segment_distance(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// The edges of `polygon`, each from one corner to the next.
pub(crate) fn edges(polygon: &[Pt]) -> impl Iterator<Item = (Pt, Pt)> + '_ {
    (0..polygon.len()).map(|i| (polygon[i], polygon[(i + 1) % polygon.len()]))
}

/// The distance from `p` to the nearest edge of `polygon`.
pub(crate) fn edge_distance(polygon: &[Pt], p: Pt) -> f64 {
    edges(polygon)
        .map(|(a, b)| segment_distance(p, a, b))
        .fold(f64::INFINITY, f64::min)
}

/// Whether `p` lies inside `polygon` by the nonzero winding rule.
pub(crate) fn inside(polygon: &[Pt], p: Pt) -> bool {
    let mut winding = 0;
    for (a, b) in edges(polygon) {
        let side = (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1);
        if a.1 <= p.1 && b.1 > p.1 && side > 0.0 {
            winding += 1;
        } else if b.1 <= p.1 && a.1 > p.1 && side < 0.0 {
            winding -= 1;
        }
    }
    winding != 0
}

/// Whether `p` lies inside `polygon` or within `tolerance` of its outline.
pub(crate) fn covers(polygon: &[Pt], p: Pt, tolerance: f64) -> bool {
    inside(polygon, p) || edge_distance(polygon, p) <= tolerance
}

/// The left, right, top and bottom of `polygon`.
pub(crate) fn bounds(polygon: &[Pt]) -> [f64; 4] {
    polygon.iter().fold(
        [f64::MAX, f64::MIN, f64::MAX, f64::MIN],
        |[x0, x1, y0, y1], p| [x0.min(p.0), x1.max(p.0), y0.min(p.1), y1.max(p.1)],
    )
}

/// The corners of `polygon` and points at most `step` apart along its edges.
pub(crate) fn outline_samples(polygon: &[Pt], step: f64) -> Vec<Pt> {
    edges(polygon)
        .flat_map(|(a, b)| {
            let steps = ((b.0 - a.0).hypot(b.1 - a.1) / step).ceil().max(1.0) as usize;
            (0..steps).map(move |k| {
                let t = k as f64 / steps as f64;
                (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
            })
        })
        .collect()
}

/// The points of a grid `step` apart over the bounds of `polygon` that lie inside it.
pub(crate) fn inner_samples(polygon: &[Pt], step: f64) -> Vec<Pt> {
    let [x0, x1, y0, y1] = bounds(polygon);
    let (columns, rows) = (((x1 - x0) / step) as usize, ((y1 - y0) / step) as usize);
    (0..=columns)
        .flat_map(|i| (0..=rows).map(move |j| (x0 + i as f64 * step, y0 + j as f64 * step)))
        .filter(|&p| inside(polygon, p))
        .collect()
}

/// The area `polygon` encloses, by the shoelace formula: its filled area when it is simple.
pub(crate) fn area(polygon: &[Pt]) -> f64 {
    edges(polygon)
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum::<f64>()
        .abs()
        / 2.0
}

/// Whether `polygon` is simple: no edge has zero length, and no two edges meet or come within
/// `tolerance` of each other except neighbouring edges at their shared corner, so the outline
/// neither crosses nor retraces itself.
pub(crate) fn simple(polygon: &[Pt], tolerance: f64) -> bool {
    let n = polygon.len();
    let edge = |i: usize| (polygon[i], polygon[(i + 1) % n]);
    let cross = |o: Pt, p: Pt, q: Pt| (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0);
    if n < 3 || (0..n).any(|i| segment_distance(edge(i).0, edge(i).1, edge(i).1) <= tolerance) {
        return false;
    }
    (0..n).all(|i| {
        (i + 1..n).all(|j| {
            let ((a, b), (c, d)) = (edge(i), edge(j));
            if j == i + 1 {
                // They share the corner b = c: neither runs back along the other.
                segment_distance(d, a, b) > tolerance && segment_distance(a, c, d) > tolerance
            } else if i == 0 && j == n - 1 {
                // They share the corner a = d.
                segment_distance(c, a, b) > tolerance && segment_distance(b, c, d) > tolerance
            } else {
                let crossing =
                    cross(c, d, a) * cross(c, d, b) < 0.0 && cross(a, b, c) * cross(a, b, d) < 0.0;
                !crossing
                    && [(a, c, d), (b, c, d), (c, a, b), (d, a, b)]
                        .iter()
                        .all(|&(p, s, t)| segment_distance(p, s, t) > tolerance)
            }
        })
    })
}

/// The part of the convex `polygon` inside the rectangle from `(x0, y0)` to `(x1, y1)`.
pub(crate) fn clip_to_rectangle(polygon: &[Pt], [x0, y0, x1, y1]: [f64; 4]) -> Vec<Pt> {
    let mut out = polygon.to_vec();
    // Each side as the coordinate it bounds, its value, and whether points keep below it.
    for (axis, edge, below) in [(0, x0, false), (0, x1, true), (1, y0, false), (1, y1, true)] {
        let at = |p: Pt| if axis == 0 { p.0 } else { p.1 };
        let keep = |p: Pt| if below { at(p) <= edge } else { at(p) >= edge };
        let mut clipped = Vec::new();
        for (a, b) in edges(&out) {
            if keep(a) {
                clipped.push(a);
            }
            if keep(a) != keep(b) {
                let t = (edge - at(a)) / (at(b) - at(a));
                clipped.push((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)));
            }
        }
        out = clipped;
        if out.is_empty() {
            break;
        }
    }
    out
}
