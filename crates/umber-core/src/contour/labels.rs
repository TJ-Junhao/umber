//! Level labels on drawn contour lines: where a label sits on a line, on the page, and the parts
//! of the line left to draw around it.
//!
//! A label sits at the middle of the line's length, turned along the line and kept upright,
//! and the line is broken under it with a gap on each side. Only a line at least
//! [`MIN_LENGTH_FACTOR`] times as long as its label carries one.

/// How many times as long as its label a line must be to carry it.
pub(crate) const MIN_LENGTH_FACTOR: f64 = 3.0;

/// A point on the page, in points, `y` down.
pub(crate) type PagePoint = (f64, f64);

/// Where a label sits: the centre of its box, and the angle of its baseline in degrees,
/// clockwise on the page, in `(-90, 90]`, so the text never reads upside down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    /// The centre of the label.
    pub center: PagePoint,
    /// The angle of the baseline in degrees, clockwise on the page.
    pub angle: f64,
}

/// The label of a line through `points` on the page, `width` pt wide, with `gap` pt of line left
/// out on each side of it: its placement, at the middle of the line's length, and the pieces of
/// the line left to draw. `None` when the line is shorter than [`MIN_LENGTH_FACTOR`] times the
/// label width. A closed line (`closed`, its last point repeating its first) leaves one piece,
/// from the far side of the label round to its near side.
///
/// The baseline follows the chord of the line across the label's width.
pub(crate) fn place(
    points: &[PagePoint],
    closed: bool,
    width: f64,
    gap: f64,
) -> Option<(Placement, Vec<Vec<PagePoint>>)> {
    let lengths = cumulative(points);
    let total = *lengths.last()?;
    if !(width > 0.0 && total >= MIN_LENGTH_FACTOR * width) {
        return None;
    }
    let middle = total / 2.0;
    let center = at(points, &lengths, middle)?;
    let before = at(points, &lengths, (middle - width / 2.0).max(0.0))?;
    let after = at(points, &lengths, (middle + width / 2.0).min(total))?;
    let angle = upright((after.1 - before.1).atan2(after.0 - before.0).to_degrees());
    let half = width / 2.0 + gap;
    let (start, end) = ((middle - half).max(0.0), (middle + half).min(total));
    let head = part(points, &lengths, 0.0, start);
    let tail = part(points, &lengths, end, total);
    let pieces = if closed {
        // The line returns to its first point, so the tail runs on into the head.
        vec![tail.into_iter().chain(head.into_iter().skip(1)).collect()]
    } else {
        vec![head, tail]
    };
    Some((Placement { center, angle }, pieces))
}

/// The closed line through `ring` (its last point repeating its first) started at the point
/// farthest in the direction `turns` quarter turns counter-clockwise from the right (with `y`
/// up), so labels of neighbouring levels on nested rings sit on different sides. The first
/// farthest point wins a tie.
pub(crate) fn rotated_ring(ring: &[PagePoint], turns: usize) -> Vec<PagePoint> {
    let points = ring.get(..ring.len().saturating_sub(1)).unwrap_or(&[]);
    let (dx, dy) = match turns % 4 {
        0 => (1.0, 0.0),
        1 => (0.0, -1.0),
        2 => (-1.0, 0.0),
        _ => (0.0, 1.0),
    };
    let reach = |p: &PagePoint| p.0 * dx + p.1 * dy;
    let start = points
        .iter()
        .enumerate()
        .fold(None, |best: Option<(usize, f64)>, (i, p)| match best {
            Some((_, far)) if far >= reach(p) => best,
            _ => Some((i, reach(p))),
        })
        .map_or(0, |(i, _)| i);
    let mut rotated: Vec<PagePoint> = points[start..]
        .iter()
        .chain(&points[..start])
        .copied()
        .collect();
    if let Some(&first) = rotated.first() {
        rotated.push(first);
    }
    rotated
}

/// `angle` in degrees turned by half a turn when needed to lie in `(-90, 90]`.
fn upright(angle: f64) -> f64 {
    if angle > 90.0 {
        angle - 180.0
    } else if angle <= -90.0 {
        angle + 180.0
    } else {
        angle
    }
}

/// The length of the line through `points` up to each point, from 0.
fn cumulative(points: &[PagePoint]) -> Vec<f64> {
    let mut lengths = Vec::with_capacity(points.len());
    let mut total = 0.0;
    for (i, p) in points.iter().enumerate() {
        if let Some(previous) = i.checked_sub(1).and_then(|i| points.get(i)) {
            total += (p.0 - previous.0).hypot(p.1 - previous.1);
        }
        lengths.push(total);
    }
    lengths
}

/// The point at the length `length` along the line through `points`, whose lengths are
/// `lengths`.
fn at(points: &[PagePoint], lengths: &[f64], length: f64) -> Option<PagePoint> {
    let segment = lengths
        .partition_point(|&l| l <= length)
        .clamp(1, points.len().max(2) - 1);
    let (a, b) = (points.get(segment - 1)?, points.get(segment)?);
    let (la, lb) = (lengths.get(segment - 1)?, lengths.get(segment)?);
    let u = if lb > la {
        ((length - la) / (lb - la)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some((a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u))
}

/// The part of the line through `points` from the length `start` to `end`: the points at both
/// ends and every point strictly between them.
fn part(points: &[PagePoint], lengths: &[f64], start: f64, end: f64) -> Vec<PagePoint> {
    let inner = points
        .iter()
        .zip(lengths)
        .filter(|&(_, &l)| start < l && l < end)
        .map(|(&p, _)| p);
    at(points, lengths, start)
        .into_iter()
        .chain(inner)
        .chain(at(points, lengths, end))
        .collect()
}
