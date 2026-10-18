//! Final geometric checks for direct labels and their stroked leaders.

use super::{Area, Circle, Point, SliceLabel};

const EPSILON: f64 = 1e-4;
const LEADER_HALF_WIDTH: f64 = 0.375;

fn bounds(label: &SliceLabel) -> Area {
    let text = &label.text;
    Area {
        x: text.x,
        y: text.y - f64::from(text.layout.ascent()),
        width: text.width(),
        height: f64::from(text.layout.ascent() + text.layout.descent()),
    }
}

fn overlap(a: Area, b: Area) -> bool {
    a.x < b.right() - EPSILON
        && b.x < a.right() - EPSILON
        && a.y < b.bottom() - EPSILON
        && b.y < a.bottom() - EPSILON
}

fn distance(point: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (f64::from(b.x - a.x), f64::from(b.y - a.y));
    let length = dx * dx + dy * dy;
    let t = if length == 0.0 {
        0.0
    } else {
        ((f64::from(point.x - a.x) * dx + f64::from(point.y - a.y) * dy) / length).clamp(0.0, 1.0)
    };
    (f64::from(point.x - a.x) - t * dx).hypot(f64::from(point.y - a.y) - t * dy)
}

fn close(a: Point, b: Point, c: Point, d: Point, gap: f64) -> bool {
    let cross = |p: Point, q: Point, r: Point| {
        f64::from(q.x - p.x) * f64::from(r.y - p.y) - f64::from(q.y - p.y) * f64::from(r.x - p.x)
    };
    let ac = cross(a, b, c);
    let ad = cross(a, b, d);
    let ca = cross(c, d, a);
    let cb = cross(c, d, b);
    if ac * ad < 0.0 && ca * cb < 0.0 {
        return true;
    }
    [
        distance(a, c, d),
        distance(b, c, d),
        distance(c, a, b),
        distance(d, a, b),
    ]
    .into_iter()
    .any(|distance| distance < gap - EPSILON)
}

fn hits(a: Point, b: Point, rect: Area) -> bool {
    let inside = |p: Point| {
        f64::from(p.x) >= rect.x
            && f64::from(p.x) <= rect.right()
            && f64::from(p.y) >= rect.y
            && f64::from(p.y) <= rect.bottom()
    };
    let corners = [
        Point {
            x: rect.x as f32,
            y: rect.y as f32,
        },
        Point {
            x: rect.right() as f32,
            y: rect.y as f32,
        },
        Point {
            x: rect.right() as f32,
            y: rect.bottom() as f32,
        },
        Point {
            x: rect.x as f32,
            y: rect.bottom() as f32,
        },
    ];
    inside(a)
        || inside(b)
        || (0..4).any(|i| close(a, b, corners[i], corners[(i + 1) % 4], LEADER_HALF_WIDTH))
}

pub(super) fn fits(labels: &[SliceLabel], circle: Circle, area: Area) -> bool {
    let boxes: Vec<_> = labels.iter().map(bounds).collect();
    for (i, label) in labels.iter().enumerate() {
        let rect = boxes[i];
        if rect.x < area.x - EPSILON
            || rect.y < area.y - EPSILON
            || rect.right() > area.right() + EPSILON
            || rect.bottom() > area.bottom() + EPSILON
        {
            return false;
        }
        if label.fill.is_none() {
            let x = circle.x.clamp(rect.x, rect.right());
            let y = circle.y.clamp(rect.y, rect.bottom());
            if (x - circle.x).hypot(y - circle.y) < circle.radius - EPSILON {
                return false;
            }
        }
        for (j, other) in labels.iter().enumerate() {
            if i == j {
                continue;
            }
            if overlap(rect, boxes[j]) {
                return false;
            }
            for a in label.leader.windows(2) {
                if hits(a[0], a[1], boxes[j]) {
                    return false;
                }
                if j > i
                    && other
                        .leader
                        .windows(2)
                        .any(|b| close(a[0], a[1], b[0], b[1], 2.0 * LEADER_HALF_WIDTH))
                {
                    return false;
                }
            }
        }
    }
    true
}
