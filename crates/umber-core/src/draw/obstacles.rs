//! What a value label placed automatically must stay clear of: the drawn geometry of marks and
//! the boxes of labels already placed, indexed by a uniform grid over the plot.
//!
//! The geometry is read from the scene items the marks were drawn as, so a label avoids
//! exactly what is drawn, after clipping and downsampling:
//!
//! - a batch of markers is one box per position: the shape's extent plus its whole outline
//!   width, which is at least as far as a miter join of these shapes reaches;
//! - a stroked path is its segments, each widened by half the stroke width on every side
//!   (dashes count as solid);
//! - a filled path is its edges and its interior, with the path's own fill rule;
//! - a placed label is its box.
//!
//! Each shape is listed in every grid cell its bounding box touches, clamped to the grid, so
//! geometry a few points outside the plot falls in the edge cells. A query tests only the
//! shapes listed in the cells its box touches. The interior of a fill is tested with a ray from
//! the box's centre to the left along the centre's row of cells, counting each edge's crossing
//! in the cell that holds it: a box that no edge crosses lies wholly inside or wholly outside.

use crate::coverage::{XY, contours, crosses};
use crate::layout::{Area, Frame};
use crate::scene::{FillRule, Item, Paint};

use super::marker::extent;

/// The side of a grid cell, in points, when the plot is small enough.
const CELL: f64 = 8.0;
/// The most cells along each side of the grid.
const MAX_CELLS: usize = 256;

/// A box: left, top, right and bottom edges, in scene points.
pub(super) type Bounds = [f64; 4];

/// One indexed shape.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    /// A filled box: a marker or a label.
    Box(Bounds),
    /// A stroked segment, `half` its stroke width on each side.
    Segment {
        /// One end.
        a: XY,
        /// The other end.
        b: XY,
        /// Half the stroke width.
        half: f64,
    },
    /// An edge of the filled path `fill` (an index into [`Obstacles::rules`]), from `a` to `b`.
    Edge {
        /// The filled path.
        fill: usize,
        /// The start, in the path's direction.
        a: XY,
        /// The end.
        b: XY,
    },
}

impl Shape {
    /// The bounding box of the shape, its stroke included.
    fn bounds(self) -> Bounds {
        match self {
            Shape::Box(bounds) => bounds,
            Shape::Segment { a, b, half } => [
                a[0].min(b[0]) - half,
                a[1].min(b[1]) - half,
                a[0].max(b[0]) + half,
                a[1].max(b[1]) + half,
            ],
            Shape::Edge { a, b, .. } => [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[0].max(b[0]),
                a[1].max(b[1]),
            ],
        }
    }

    /// Whether the shape meets the box `bounds`: overlaps it, or for an edge, crosses or
    /// touches it. The interior of a fill is not tested here.
    fn meets(self, bounds: Bounds) -> bool {
        match self {
            Shape::Box(other) => overlaps(other, bounds),
            Shape::Segment { a, b, half } => {
                let [left, top, right, bottom] = bounds;
                crosses(
                    [a, b],
                    &frame([left - half, top - half, right + half, bottom + half]),
                )
            }
            Shape::Edge { a, b, .. } => crosses([a, b], &frame(bounds)),
        }
    }
}

/// The obstacles of one panel, indexed by a uniform grid over its plot.
#[derive(Debug, Clone)]
pub(super) struct Obstacles {
    /// The left and top edges of the grid: the plot's.
    origin: XY,
    /// The side of a cell, in points.
    cell: f64,
    /// The number of columns and rows.
    size: (usize, usize),
    /// Every shape, in the order it was added.
    shapes: Vec<Shape>,
    /// For each cell, row by row, the indices into `shapes` of the shapes it lists.
    cells: Vec<Vec<usize>>,
    /// The fill rule of each filled path.
    rules: Vec<FillRule>,
}

impl Obstacles {
    /// An empty index over `plot`.
    pub(super) fn new(plot: Area) -> Obstacles {
        let longest = plot.width.max(plot.height).max(1.0);
        let cell = CELL.max(longest / MAX_CELLS as f64);
        let count = |length: f64| ((length / cell).ceil() as usize).clamp(1, MAX_CELLS);
        let size = (count(plot.width), count(plot.height));
        Obstacles {
            origin: [plot.x, plot.y],
            cell,
            size,
            shapes: Vec::new(),
            cells: vec![Vec::new(); size.0 * size.1],
            rules: Vec::new(),
        }
    }

    /// Adds the drawn geometry of `items`, groups included; text and images are left out.
    pub(super) fn add_items(&mut self, items: &[Item]) {
        for item in items {
            self.add_item(item);
        }
    }

    /// Adds the box `bounds` of a placed label.
    pub(super) fn add_box(&mut self, bounds: Bounds) {
        self.add(Shape::Box(bounds));
    }

    /// Whether the box `bounds` meets any obstacle: a shape listed in a cell it touches, or
    /// the interior of a filled path holding its centre.
    pub(super) fn hit(&self, bounds: Bounds) -> bool {
        let (columns, rows) = self.span(bounds);
        for row in rows.clone() {
            for column in columns.clone() {
                let listed = &self.cells[row * self.size.0 + column];
                if listed.iter().any(|&i| self.shapes[i].meets(bounds)) {
                    return true;
                }
            }
        }
        !self.rules.is_empty()
            && self.inside_fill([(bounds[0] + bounds[2]) / 2.0, (bounds[1] + bounds[3]) / 2.0])
    }

    /// Whether `point` lies inside a filled path, by its fill rule: the winding number of each
    /// path around `point`, from the edges crossing the ray to its left along its row.
    fn inside_fill(&self, point: XY) -> bool {
        let row = self.index(point[1], 1, self.size.1);
        let last = self.index(point[0], 0, self.size.0);
        let mut windings = vec![0_i64; self.rules.len()];
        for column in 0..=last {
            for &i in &self.cells[row * self.size.0 + column] {
                let Shape::Edge { fill, a, b } = self.shapes[i] else {
                    continue;
                };
                // Half-open in y, so an edge through a vertex at the ray counts once.
                let (low, high) = (a[1].min(b[1]), a[1].max(b[1]));
                if !(low <= point[1] && point[1] < high) {
                    continue;
                }
                let x = a[0] + (point[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
                // Count the crossing once, in the cell that holds it.
                if x < point[0] && self.index(x, 0, self.size.0) == column {
                    windings[fill] += if b[1] > a[1] { 1 } else { -1 };
                }
            }
        }
        windings
            .iter()
            .zip(&self.rules)
            .any(|(&winding, rule)| match rule {
                FillRule::NonZero => winding != 0,
                FillRule::EvenOdd => winding % 2 != 0,
            })
    }

    /// Adds the geometry of `item`, as the module documentation describes.
    fn add_item(&mut self, item: &Item) {
        match item {
            Item::Group { items, .. } => self.add_items(items),
            Item::Markers {
                shape,
                fill,
                stroke,
                positions,
            } => {
                let stroke = stroke
                    .as_ref()
                    .filter(|s| s.color.a > 0 && s.width > 0.0)
                    .map_or(0.0, |s| f64::from(s.width));
                if !fill.is_some_and(|c| c.a > 0) && stroke == 0.0 {
                    return;
                }
                let (half_w, half_h) = extent(shape);
                let (half_w, half_h) = (half_w + stroke, half_h + stroke);
                for p in positions {
                    let (x, y) = (f64::from(p.x), f64::from(p.y));
                    self.add(Shape::Box([x - half_w, y - half_h, x + half_w, y + half_h]));
                }
            }
            Item::Stroke { path, style } => {
                if style.color.a == 0 || style.width <= 0.0 {
                    return;
                }
                let half = f64::from(style.width) / 2.0;
                for contour in contours(path) {
                    if let [only] = contour.as_slice() {
                        self.add(Shape::Segment {
                            a: *only,
                            b: *only,
                            half,
                        });
                    }
                    for pair in contour.windows(2) {
                        self.add(Shape::Segment {
                            a: pair[0],
                            b: pair[1],
                            half,
                        });
                    }
                }
            }
            Item::Fill { path, paint, rule } => {
                if !visible(paint) {
                    return;
                }
                let fill = self.rules.len();
                self.rules.push(*rule);
                for contour in contours(path) {
                    // A fill closes every contour.
                    let closing = match (contour.first(), contour.last()) {
                        (Some(&first), Some(&last)) if first != last => Some([last, first]),
                        _ => None,
                    };
                    let edges = contour.windows(2).map(|pair| [pair[0], pair[1]]);
                    for [a, b] in edges.chain(closing) {
                        self.add(Shape::Edge { fill, a, b });
                    }
                }
            }
            Item::Text(_) | Item::RotatedText { .. } | Item::Image { .. } => {}
        }
    }

    /// Adds `shape` and lists it in every cell its bounding box touches.
    fn add(&mut self, shape: Shape) {
        let index = self.shapes.len();
        self.shapes.push(shape);
        let (columns, rows) = self.span(shape.bounds());
        for row in rows {
            for column in columns.clone() {
                self.cells[row * self.size.0 + column].push(index);
            }
        }
    }

    /// The columns and rows of the cells the box `bounds` touches, clamped to the grid.
    fn span(
        &self,
        [left, top, right, bottom]: Bounds,
    ) -> (
        std::ops::RangeInclusive<usize>,
        std::ops::RangeInclusive<usize>,
    ) {
        let (columns, rows) = self.size;
        (
            self.index(left, 0, columns)..=self.index(right, 0, columns),
            self.index(top, 1, rows)..=self.index(bottom, 1, rows),
        )
    }

    /// The cell index of the coordinate `at` along the grid's axis `axis` (0 for x, 1 for y)
    /// of `count` cells, clamped to the grid; NaN falls in the first cell.
    fn index(&self, at: f64, axis: usize, count: usize) -> usize {
        let cell = ((at - self.origin[axis]) / self.cell).floor();
        if cell >= 0.0 {
            (cell as usize).min(count - 1)
        } else {
            0
        }
    }
}

/// Whether the boxes `a` and `b` overlap or touch.
fn overlaps(a: Bounds, b: Bounds) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// The box `bounds` as a frame.
fn frame([left, top, right, bottom]: Bounds) -> Frame {
    Frame {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

/// Whether `paint` puts any color down.
fn visible(paint: &Paint) -> bool {
    match paint {
        Paint::Solid(color) => color.a > 0,
        Paint::Linear(gradient) => gradient.stops.iter().any(|s| s.color.a > 0),
        Paint::Sampled { .. } => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgba;
    use crate::geometry::{Path, Point};
    use crate::scene::{LineCap, LineJoin, StrokeStyle};

    const INK: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    fn plot() -> Area {
        Area {
            x: 10.0,
            y: 20.0,
            width: 200.0,
            height: 100.0,
        }
    }

    fn path(points: &[(f32, f32)], close: bool) -> Path {
        let mut path = Path::new();
        for (i, &(x, y)) in points.iter().enumerate() {
            if i == 0 {
                path.move_to(Point { x, y });
            } else {
                path.line_to(Point { x, y });
            }
        }
        if close {
            path.close();
        }
        path
    }

    fn stroke(width: f32) -> StrokeStyle {
        StrokeStyle {
            color: INK,
            width,
            dash: Vec::new(),
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }
    }

    #[test]
    fn a_stroke_blocks_boxes_within_half_its_width() {
        let mut obstacles = Obstacles::new(plot());
        obstacles.add_items(&[Item::Stroke {
            path: path(&[(20.0, 50.0), (200.0, 50.0)], false),
            style: stroke(2.0),
        }]);
        assert!(obstacles.hit([60.0, 40.0, 70.0, 49.5]));
        assert!(!obstacles.hit([60.0, 40.0, 70.0, 48.9]));
        // A diagonal stroke blocks only along its path, not its whole bounding box.
        let mut diagonal = Obstacles::new(plot());
        diagonal.add_items(&[Item::Stroke {
            path: path(&[(10.0, 20.0), (210.0, 120.0)], false),
            style: stroke(1.0),
        }]);
        assert!(diagonal.hit([105.0, 65.0, 115.0, 75.0]));
        assert!(!diagonal.hit([150.0, 30.0, 170.0, 40.0]));
    }

    #[test]
    fn a_fill_blocks_boxes_on_its_edges_and_inside_it() {
        let mut obstacles = Obstacles::new(plot());
        obstacles.add_items(&[Item::Fill {
            path: path(
                &[(20.0, 30.0), (180.0, 30.0), (180.0, 110.0), (20.0, 110.0)],
                true,
            ),
            paint: Paint::Solid(INK),
            rule: FillRule::NonZero,
        }]);
        assert!(obstacles.hit([100.0, 60.0, 110.0, 70.0]), "inside");
        assert!(obstacles.hit([170.0, 60.0, 190.0, 70.0]), "across an edge");
        assert!(!obstacles.hit([185.0, 60.0, 205.0, 70.0]), "outside");
        // An even-odd ring leaves its hole clear.
        let mut ring = Path::new();
        for points in [
            [(20.0, 30.0), (180.0, 30.0), (180.0, 110.0), (20.0, 110.0)],
            [(60.0, 50.0), (140.0, 50.0), (140.0, 90.0), (60.0, 90.0)],
        ] {
            for (i, &(x, y)) in points.iter().enumerate() {
                if i == 0 {
                    ring.move_to(Point { x, y });
                } else {
                    ring.line_to(Point { x, y });
                }
            }
            ring.close();
        }
        let mut holed = Obstacles::new(plot());
        holed.add_items(&[Item::Fill {
            path: ring,
            paint: Paint::Solid(INK),
            rule: FillRule::EvenOdd,
        }]);
        assert!(!holed.hit([95.0, 65.0, 105.0, 75.0]), "in the hole");
        assert!(holed.hit([30.0, 65.0, 40.0, 75.0]), "in the ring");
    }

    #[test]
    fn markers_block_their_extent_and_outline_and_labels_their_box() {
        let mut obstacles = Obstacles::new(plot());
        let square = path(&[(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (-2.0, 2.0)], true);
        obstacles.add_items(&[Item::Markers {
            shape: square,
            fill: Some(INK),
            stroke: Some(stroke(1.0)),
            positions: vec![Point { x: 100.0, y: 60.0 }],
        }]);
        assert!(obstacles.hit([102.5, 55.0, 110.0, 65.0]));
        assert!(!obstacles.hit([103.5, 55.0, 110.0, 65.0]));
        obstacles.add_box([120.0, 40.0, 140.0, 50.0]);
        assert!(obstacles.hit([135.0, 45.0, 150.0, 55.0]));
        assert!(!obstacles.hit([141.0, 45.0, 150.0, 55.0]));
    }

    #[test]
    fn geometry_outside_the_plot_falls_in_the_edge_cells() {
        let mut obstacles = Obstacles::new(plot());
        obstacles.add_box([0.0, 0.0, 12.0, 22.0]);
        assert!(obstacles.hit([11.0, 21.0, 15.0, 25.0]));
        assert!(!obstacles.hit([13.0, 23.0, 15.0, 25.0]));
    }
}
