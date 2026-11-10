//! Level labels on drawn contour lines: where a label sits on a line, on the page, and the parts
//! of the line left to draw around it.
//!
//! A label is turned along the line and kept upright, and the line is broken under it with a
//! gap on each side. Only a line at least [`MIN_LENGTH_FACTOR`] times as long as its label
//! carries one.
//!
//! **Position.** A label must lie wholly in its [`Room`]: inside the plot area and inside the
//! field, clear of its holes and of its outer edge. The middle of the line's length is tried
//! first; then the positions [`STEP_FACTOR`] label widths apart on either side of it, nearest
//! the middle first and, at the same distance, the one further along the line first, up to
//! [`MAX_STEPS`] steps on each side, each only while the stretch of line left out under the
//! label lies within the line. The label takes the first position whose box fits
//! ([`Room::holds`]). When none fits, the line has no label and is drawn whole. A closed line
//! is started where [`rotated_ring`] starts it, so its middle is the side opposite that start.

use std::iter;

/// How many times as long as its label a line must be to carry it.
pub(crate) const MIN_LENGTH_FACTOR: f64 = 3.0;
/// The distance between the positions a label is tried at, in label widths.
pub(crate) const STEP_FACTOR: f64 = 0.25;
/// The number of steps a label is tried at on each side of the middle of its line.
pub(crate) const MAX_STEPS: u32 = 32;
/// How far a label's box is widened on every side before it is tested against its room, in
/// points, so that rounding never lets a box that touches an edge pass.
const SLACK: f64 = 1e-6;
/// The largest number of boundary edges in a leaf of the tree of a [`Room`].
const LEAF_EDGES: usize = 8;

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

impl Placement {
    /// The page point of the point `(x, y)` of the label's own frame, whose origin is the
    /// label's centre and whose x axis runs along its baseline: turned clockwise by the angle,
    /// then moved to the centre.
    pub(crate) fn to_page(self, (x, y): (f64, f64)) -> PagePoint {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (cx, cy) = self.center;
        (cx + x * cos - y * sin, cy + x * sin + y * cos)
    }

    /// The point of the label's own frame at the page point `(x, y)`: the inverse of
    /// [`Placement::to_page`].
    fn to_label(self, (x, y): PagePoint) -> (f64, f64) {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (dx, dy) = (x - self.center.0, y - self.center.1);
        (dx * cos + dy * sin, dy * cos - dx * sin)
    }

    /// The corners of `bounds`, in the label's own frame, on the page.
    pub(crate) fn corners(self, bounds: LabelBox) -> [PagePoint; 4] {
        [
            (bounds.left, bounds.top),
            (bounds.right, bounds.top),
            (bounds.right, bounds.bottom),
            (bounds.left, bounds.bottom),
        ]
        .map(|corner| self.to_page(corner))
    }
}

/// The box of a label's text in its own frame, before it is turned along its line: from `left`
/// to `right` along its baseline and from `top` to `bottom` across it, in points from its
/// centre, `y` down. A label is centred along its line, so `left` is `-right`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LabelBox {
    /// The left end of the text.
    pub left: f64,
    /// The top of the text: the ascender.
    pub top: f64,
    /// The right end of the text.
    pub right: f64,
    /// The bottom of the text: the descender.
    pub bottom: f64,
}

impl LabelBox {
    /// The width of the text along its baseline.
    pub(crate) fn width(self) -> f64 {
        self.right - self.left
    }

    /// This box widened by `by` on every side.
    pub(crate) fn widened(self, by: f64) -> LabelBox {
        LabelBox {
            left: self.left - by,
            top: self.top - by,
            right: self.right + by,
            bottom: self.bottom + by,
        }
    }
}

/// The label `label` of a line through `points` on the page, with `gap` pt of line left out on
/// each side of it and kept clear all around it: its placement, at the first position along the
/// line where its box, widened by `gap`, fits in `room` (see the module documentation), and the
/// pieces of the line left to draw. `None` when the line is shorter than [`MIN_LENGTH_FACTOR`]
/// times the label width or no position fits. A closed line (`closed`, its last point
/// repeating its first) leaves one piece, from the far side of the label round to its near
/// side.
///
/// The baseline follows the chord of the line across the label's width.
pub(crate) fn place(
    points: &[PagePoint],
    closed: bool,
    (label, gap): (LabelBox, f64),
    room: &Room,
) -> Option<(Placement, Vec<Vec<PagePoint>>)> {
    let lengths = cumulative(points);
    let total = *lengths.last()?;
    let width = label.width();
    if !(width > 0.0 && total >= MIN_LENGTH_FACTOR * width) {
        return None;
    }
    let half = width / 2.0 + gap;
    let middle = total / 2.0;
    let step = STEP_FACTOR * width;
    let shifted = (1..=MAX_STEPS)
        .flat_map(|k| {
            let by = f64::from(k) * step;
            [middle + by, middle - by]
        })
        .filter(|&at| at - half >= 0.0 && at + half <= total);
    let kept = label.widened(gap);
    let (at, placement) = iter::once(middle).chain(shifted).find_map(|at| {
        let placement = placement_at(points, &lengths, at, width)?;
        room.holds(placement, kept).then_some((at, placement))
    })?;
    let (start, end) = ((at - half).max(0.0), (at + half).min(total));
    let head = part(points, &lengths, 0.0, start);
    let tail = part(points, &lengths, end, total);
    let pieces = if closed {
        // The line returns to its first point, so the tail runs on into the head.
        vec![tail.into_iter().chain(head.into_iter().skip(1)).collect()]
    } else {
        vec![head, tail]
    };
    Some((placement, pieces))
}

/// The placement of a label `width` pt wide at the length `at` along the line through
/// `points`, whose lengths are `lengths`: centred there, its baseline along the chord of the
/// line across its width.
fn placement_at(points: &[PagePoint], lengths: &[f64], at: f64, width: f64) -> Option<Placement> {
    let total = *lengths.last()?;
    let center = self::at(points, lengths, at)?;
    let before = self::at(points, lengths, (at - width / 2.0).max(0.0))?;
    let after = self::at(points, lengths, (at + width / 2.0).min(total))?;
    let angle = upright((after.1 - before.1).atan2(after.0 - before.0).to_degrees());
    Some(Placement { center, angle })
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

/// Where the labels of a contour may lie on the page: inside the plot area and inside its
/// field, the region of its finite cells or triangles, which a grid and a mesh both give as
/// the boundary rings of their first band ([`super::ContourField::whole`]).
///
/// **The test.** A label's box, turned along its line and widened by the gap kept around it,
/// fits when its four corners lie in the plot area and no edge of the field's boundary meets
/// it, on its border or inside. The box is centred on a point of a contour line, and every
/// line lies in the field, so a box that meets no boundary edge lies wholly in the field: the
/// box is connected, and a part of it outside the field would put a boundary edge through it.
/// The test is conservative: the box is first widened by a millionth of a point, so rounding
/// never lets a box that touches the boundary, or reaches past the plot area, pass. A boundary
/// point that is not finite leaves no room at all.
///
/// **The cost.** The boundary edges are kept in a tree of bounding boxes, split at the median
/// of the longer side of each node down to leaves of at most eight edges, built once per
/// contour in `O(e log e)` for `e` edges. A test visits the nodes whose bounds meet the box's,
/// `O(log e + k)` for the `k` edges near the box, so placing every label of a contour costs
/// `O(labels × (2 × 32 + 1) × (log e + k))`.
#[derive(Debug, Clone)]
pub(crate) struct Room {
    /// The plot area.
    plot: Bounds,
    /// The boundary edges of the field, in the order of the tree's leaves.
    edges: Vec<Edge>,
    /// The tree over `edges`, its root first; empty without edges.
    nodes: Vec<Node>,
    /// Whether every point of the boundary is finite; otherwise no label fits.
    finite: bool,
}

/// An edge of a field's boundary on the page.
#[derive(Debug, Clone, Copy)]
struct Edge {
    /// One end.
    from: PagePoint,
    /// The other end.
    to: PagePoint,
}

/// A rectangle on the page, `y` down.
#[derive(Debug, Clone, Copy)]
struct Bounds {
    /// The left edge.
    left: f64,
    /// The top edge.
    top: f64,
    /// The right edge.
    right: f64,
    /// The bottom edge.
    bottom: f64,
}

/// A node of the tree of a [`Room`].
#[derive(Debug, Clone, Copy)]
struct Node {
    /// The bounds of every edge below the node.
    bounds: Bounds,
    /// What lies below it.
    below: Below,
}

/// What lies below a node of the tree of a [`Room`].
#[derive(Debug, Clone, Copy)]
enum Below {
    /// The edges from `start` to `end` of the room's edges: a leaf.
    Edges {
        /// The first edge.
        start: usize,
        /// The edge after the last.
        end: usize,
    },
    /// Two nodes, by their indices.
    Nodes(usize, usize),
}

impl Bounds {
    /// The bounds of nothing, which every union widens.
    const EMPTY: Bounds = Bounds {
        left: f64::INFINITY,
        top: f64::INFINITY,
        right: f64::NEG_INFINITY,
        bottom: f64::NEG_INFINITY,
    };

    /// The bounds of `points`.
    fn of(points: &[PagePoint]) -> Bounds {
        points.iter().fold(Bounds::EMPTY, |bounds, &(x, y)| Bounds {
            left: bounds.left.min(x),
            top: bounds.top.min(y),
            right: bounds.right.max(x),
            bottom: bounds.bottom.max(y),
        })
    }

    /// The bounds of both `self` and `other`.
    fn union(self, other: Bounds) -> Bounds {
        Bounds {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// Whether `self` and `other` share a point, their borders included.
    fn meets(self, other: Bounds) -> bool {
        self.left <= other.right
            && other.left <= self.right
            && self.top <= other.bottom
            && other.top <= self.bottom
    }

    /// Whether `(x, y)` lies in `self`, its border included.
    fn holds(self, (x, y): PagePoint) -> bool {
        self.left <= x && x <= self.right && self.top <= y && y <= self.bottom
    }
}

impl Edge {
    /// The bounds of the edge.
    fn bounds(self) -> Bounds {
        Bounds::of(&[self.from, self.to])
    }

    /// Twice the middle of the edge along x (`horizontal`) or y: the key the tree splits by.
    fn key(self, horizontal: bool) -> f64 {
        if horizontal {
            self.from.0 + self.to.0
        } else {
            self.from.1 + self.to.1
        }
    }

    /// Whether the edge meets `bounds`, in the frame of a label at `placement`, border
    /// included: the edge is taken into that frame and clipped to the box (Liang–Barsky).
    fn meets(self, placement: Placement, bounds: LabelBox) -> bool {
        let (x, y) = placement.to_label(self.from);
        let (to_x, to_y) = placement.to_label(self.to);
        let (dx, dy) = (to_x - x, to_y - y);
        let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
        for (towards, space) in [
            (-dx, x - bounds.left),
            (dx, bounds.right - x),
            (-dy, y - bounds.top),
            (dy, bounds.bottom - y),
        ] {
            if towards == 0.0 {
                if space < 0.0 {
                    return false;
                }
            } else if towards < 0.0 {
                enter = enter.max(space / towards);
            } else {
                leave = leave.min(space / towards);
            }
        }
        enter <= leave
    }
}

impl Room {
    /// The room inside the plot area `plot` (left, top, right and bottom on the page) and inside
    /// the field bounded by `rings`, each a closed ring on the page without its first point
    /// repeated, filled with the even-odd rule.
    pub(crate) fn new(plot: (f64, f64, f64, f64), rings: &[Vec<PagePoint>]) -> Room {
        let (left, top, right, bottom) = plot;
        let mut edges: Vec<Edge> = rings
            .iter()
            .flat_map(|ring| {
                ring.iter()
                    .zip(ring.iter().cycle().skip(1))
                    .map(|(&from, &to)| Edge { from, to })
            })
            .collect();
        let finite = edges.iter().all(|edge| {
            [edge.from, edge.to]
                .iter()
                .all(|p| p.0.is_finite() && p.1.is_finite())
        });
        let mut nodes = Vec::new();
        if finite && !edges.is_empty() {
            build(&mut edges, 0, &mut nodes);
        }
        Room {
            plot: Bounds {
                left,
                top,
                right,
                bottom,
            },
            edges,
            nodes,
            finite,
        }
    }

    /// Whether a label's box `bounds`, in its own frame, at `placement` fits in the room: its
    /// corners lie in the plot area and no edge of the field's boundary meets it, the box first
    /// widened by a millionth of a point (see [`Room`]).
    pub(crate) fn holds(&self, placement: Placement, bounds: LabelBox) -> bool {
        let bounds = bounds.widened(SLACK);
        let corners = placement.corners(bounds);
        if !self.finite || !corners.iter().all(|&corner| self.plot.holds(corner)) {
            return false;
        }
        let around = Bounds::of(&corners);
        let mut pending: Vec<usize> = Vec::new();
        if !self.nodes.is_empty() {
            pending.push(0);
        }
        while let Some(index) = pending.pop() {
            let Some(node) = self.nodes.get(index) else {
                continue;
            };
            if !node.bounds.meets(around) {
                continue;
            }
            match node.below {
                Below::Nodes(first, second) => pending.extend([first, second]),
                Below::Edges { start, end } => {
                    let near = self.edges.get(start..end).unwrap_or(&[]);
                    if near
                        .iter()
                        .any(|edge| edge.bounds().meets(around) && edge.meets(placement, bounds))
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// The tree over `edges`, which start at the index `offset` of the room's edges, appended to
/// `nodes`, its edges reordered to its leaves: the index of its root. A node of more than
/// [`LEAF_EDGES`] edges splits them at the median of their middles along the longer side of
/// its bounds.
fn build(edges: &mut [Edge], offset: usize, nodes: &mut Vec<Node>) -> usize {
    let bounds = edges
        .iter()
        .fold(Bounds::EMPTY, |bounds, edge| bounds.union(edge.bounds()));
    let index = nodes.len();
    nodes.push(Node {
        bounds,
        below: Below::Edges {
            start: offset,
            end: offset + edges.len(),
        },
    });
    if edges.len() > LEAF_EDGES {
        let horizontal = bounds.right - bounds.left >= bounds.bottom - bounds.top;
        let middle = edges.len() / 2;
        edges.select_nth_unstable_by(middle, |a, b| {
            a.key(horizontal).total_cmp(&b.key(horizontal))
        });
        let (low, high) = edges.split_at_mut(middle);
        let first = build(low, offset, nodes);
        let second = build(high, offset + middle, nodes);
        if let Some(node) = nodes.get_mut(index) {
            node.below = Below::Nodes(first, second);
        }
    }
    index
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
