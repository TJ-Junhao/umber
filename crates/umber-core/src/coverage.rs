//! Coverage of the final drawn geometry, used only for legends inside a plot.
//!
//! Points and segments are counted once. Filled paths are measured in square points by
//! horizontal scanlines at most one point apart, with the path's own winding rule. This also
//! handles overlapping contours and areas that cross themselves, rather than cancelling their
//! signed areas. Curves in rounded bar corners are flattened before scoring.

use crate::layout::{Area, Frame};
use crate::{FillRule, Item, Paint, Path, PathVerb, Point};

/// A point in the final scene, widened to f64 for intersections.
type XY = [f64; 2];

/// One filled path, whose contours share a winding rule.
#[derive(Debug, Clone, PartialEq)]
struct Filled {
    contours: Vec<Vec<XY>>,
    rule: FillRule,
}

/// Geometry after axis mapping, clipping and downsampling, in drawing order within each kind.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Geometry {
    points: Vec<XY>,
    segments: Vec<[XY; 2]>,
    fills: Vec<Filled>,
}

fn xy(p: Point) -> XY {
    [f64::from(p.x), f64::from(p.y)]
}

fn visible(paint: &Paint) -> bool {
    match paint {
        Paint::Solid(color) => color.a > 0,
        Paint::Linear(gradient) => gradient.stops.iter().any(|s| s.color.a > 0),
    }
}

impl Geometry {
    /// Records the same scene primitives used for drawing a resolved mark.
    pub fn of(item: &Item) -> Self {
        let mut geometry = Self::default();
        geometry.append(item);
        geometry
    }

    fn append(&mut self, item: &Item) {
        match item {
            Item::Group { items, .. } => {
                for item in items {
                    self.append(item);
                }
            }
            Item::Markers {
                positions,
                fill,
                stroke,
                ..
            } => {
                if fill.is_some_and(|c| c.a > 0)
                    || stroke
                        .as_ref()
                        .is_some_and(|s| s.color.a > 0 && s.width > 0.0)
                {
                    self.points.extend(positions.iter().copied().map(xy));
                }
            }
            Item::Stroke { path, style } => {
                if style.color.a > 0 && style.width > 0.0 {
                    for contour in contours(path) {
                        self.segments
                            .extend(contour.windows(2).map(|p| [p[0], p[1]]));
                    }
                }
            }
            Item::Fill { path, paint, rule } => {
                if visible(paint) {
                    self.fills.push(Filled {
                        contours: contours(path),
                        rule: *rule,
                    });
                }
            }
            Item::Text(_) => {}
        }
    }

    /// Counts only geometry inside both the plot and the candidate box.
    pub fn coverage(&self, area: &Area, box_: &Frame) -> f64 {
        let left = area.x.max(box_.x);
        let top = area.y.max(box_.y);
        let right = area.right().min(box_.right());
        let bottom = area.bottom().min(box_.bottom());
        if right <= left || bottom <= top {
            return 0.0;
        }
        let bounds = Frame {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        };
        let points = self.points.iter().filter(|p| inside(**p, &bounds)).count() as f64;
        let segments = self
            .segments
            .iter()
            .filter(|s| crosses(**s, &bounds))
            .count() as f64;
        self.fills.iter().fold(points + segments, |score, fill| {
            score + fill.overlap(&bounds)
        })
    }
}

fn inside(p: XY, b: &Frame) -> bool {
    p[0] >= b.x && p[0] <= b.right() && p[1] >= b.y && p[1] <= b.bottom()
}

/// Slab intersection includes segments whose two endpoints lie outside the candidate.
fn crosses([a, b]: [XY; 2], box_: &Frame) -> bool {
    let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
    for (axis, lo, hi) in [(0, box_.x, box_.right()), (1, box_.y, box_.bottom())] {
        let delta = b[axis] - a[axis];
        if delta == 0.0 {
            if a[axis] < lo || a[axis] > hi {
                return false;
            }
        } else {
            let t0 = (lo - a[axis]) / delta;
            let t1 = (hi - a[axis]) / delta;
            enter = enter.max(t0.min(t1));
            leave = leave.min(t0.max(t1));
            if enter > leave {
                return false;
            }
        }
    }
    true
}

/// Flatten only the curves used in filled bar corners; straight data segments stay unchanged.
fn contours(path: &Path) -> Vec<Vec<XY>> {
    let mut all = Vec::new();
    let mut run = Vec::new();
    for verb in path.verbs() {
        match *verb {
            PathVerb::MoveTo(p) => {
                if !run.is_empty() {
                    all.push(std::mem::take(&mut run));
                }
                run.push(xy(p));
            }
            PathVerb::LineTo(p) => run.push(xy(p)),
            PathVerb::QuadTo(c, p) => {
                if let Some(&start) = run.last() {
                    let (c, p) = (xy(c), xy(p));
                    for i in 1..=16 {
                        let t = f64::from(i) / 16.0;
                        let u = 1.0 - t;
                        run.push(std::array::from_fn(|j| {
                            u * u * start[j] + 2.0 * u * t * c[j] + t * t * p[j]
                        }));
                    }
                }
            }
            PathVerb::CubicTo(c1, c2, p) => {
                if let Some(&start) = run.last() {
                    let (c1, c2, p) = (xy(c1), xy(c2), xy(p));
                    for i in 1..=16 {
                        let t = f64::from(i) / 16.0;
                        let u = 1.0 - t;
                        run.push(std::array::from_fn(|j| {
                            u * u * u * start[j]
                                + 3.0 * u * u * t * c1[j]
                                + 3.0 * u * t * t * c2[j]
                                + t * t * t * p[j]
                        }));
                    }
                }
            }
            PathVerb::Close => {
                if let Some(&first) = run.first() {
                    run.push(first);
                    all.push(std::mem::take(&mut run));
                }
            }
        }
    }
    if !run.is_empty() {
        all.push(run);
    }
    all
}

impl Filled {
    /// Filled horizontal intervals are integrated at no more than 1 pt spacing. Include every
    /// vertex and every crossing of a vertical box edge as a row boundary: for non-crossing
    /// polygon edges the midpoint integration is exact, even on sub-point slivers.
    fn overlap(&self, box_: &Frame) -> f64 {
        let mut edges = Vec::new();
        let mut rows = vec![box_.y, box_.bottom()];
        for contour in &self.contours {
            for (&a, &b) in contour
                .iter()
                .zip(contour.iter().cycle().skip(1))
                .take(contour.len())
            {
                if a[1] == b[1] || a[1].max(b[1]) <= box_.y || a[1].min(b[1]) >= box_.bottom() {
                    continue;
                }
                edges.push([a, b]);
                for y in [a[1], b[1]] {
                    if y > box_.y && y < box_.bottom() {
                        rows.push(y);
                    }
                }
                if a[0] != b[0] {
                    for x in [box_.x, box_.right()] {
                        let t = (x - a[0]) / (b[0] - a[0]);
                        let y = a[1] + t * (b[1] - a[1]);
                        if t > 0.0 && t < 1.0 && y > box_.y && y < box_.bottom() {
                            rows.push(y);
                        }
                    }
                }
            }
        }
        rows.sort_by(f64::total_cmp);
        rows.dedup();
        let mut area = 0.0;
        let mut hits = Vec::new();
        for pair in rows.windows(2) {
            let height = pair[1] - pair[0];
            let count = height.ceil().max(1.0) as usize;
            let dy = height / count as f64;
            for i in 0..count {
                let y = pair[0] + (i as f64 + 0.5) * dy;
                hits.clear();
                for &[a, b] in &edges {
                    if y >= a[1].min(b[1]) && y < a[1].max(b[1]) {
                        let x = a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                        hits.push((x, if b[1] > a[1] { 1_i64 } else { -1_i64 }));
                    }
                }
                hits.sort_by(|a, b| a.0.total_cmp(&b.0));
                let (mut winding, mut previous, mut width) = (0_i64, box_.x, 0.0);
                for &(x, direction) in &hits {
                    let filled = match self.rule {
                        FillRule::NonZero => winding != 0,
                        FillRule::EvenOdd => winding % 2 != 0,
                    };
                    if filled {
                        width += (x.min(box_.right()) - previous.max(box_.x)).max(0.0);
                    }
                    winding += direction;
                    previous = x;
                }
                area += width * dy;
            }
        }
        area
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_segments_and_points_count_once() {
        let area = Area {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let box_ = Frame {
            x: 40.0,
            y: 40.0,
            width: 20.0,
            height: 20.0,
        };
        let geometry = Geometry {
            points: vec![[50.0, 50.0], [10.0, 10.0]],
            segments: vec![[[0.0, 50.0], [100.0, 50.0]], [[0.0, 0.0], [10.0, 10.0]]],
            fills: Vec::new(),
        };
        assert_eq!(geometry.coverage(&area, &box_), 2.0);
    }

    #[test]
    fn filled_overlap_uses_square_points_and_winding() {
        let fill = Filled {
            contours: vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]],
            rule: FillRule::NonZero,
        };
        let box_ = Frame {
            x: 5.0,
            y: 5.0,
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(fill.overlap(&box_), 25.0);
        let twice = Filled {
            contours: [fill.contours.clone(), fill.contours].concat(),
            rule: FillRule::EvenOdd,
        };
        assert_eq!(twice.overlap(&box_), 0.0);
    }
}
