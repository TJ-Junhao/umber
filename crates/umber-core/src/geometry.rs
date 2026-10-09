//! Backend-independent 2D geometry: points and paths.
//!
//! Coordinates are in points (1/72 inch) with y pointing down, the convention used throughout
//! the crate.
//!
//! # Examples
//!
//! ```
//! use umber_core::geometry::{Path, Point};
//!
//! let mut path = Path::new();
//! path.move_to(Point { x: 0.0, y: 0.0 });
//! path.line_to(Point { x: 10.0, y: 5.0 });
//! assert_eq!(path.bounds(), Some((Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 5.0 })));
//! ```

/// A point in points (1/72 inch), y-down.
///
/// # Examples
///
/// ```
/// use umber_core::Point;
///
/// let p = Point { x: 1.5, y: -2.0 };
/// assert_eq!(p.x + p.y, -0.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// Horizontal coordinate, growing to the right.
    pub x: f32,
    /// Vertical coordinate, growing downwards.
    pub y: f32,
}

impl Point {
    /// Returns this point moved by `dx` and `dy`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Point;
    ///
    /// assert_eq!(Point { x: 1.0, y: 2.0 }.translated(3.0, -1.0), Point { x: 4.0, y: 1.0 });
    /// ```
    #[must_use]
    pub fn translated(self, dx: f32, dy: f32) -> Point {
        Point {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

/// One drawing command of a [`Path`].
///
/// # Examples
///
/// ```
/// use umber_core::{Path, PathVerb, Point};
///
/// let mut path = Path::new();
/// path.move_to(Point { x: 1.0, y: 1.0 });
/// path.close();
/// assert_eq!(path.verbs(), &[PathVerb::MoveTo(Point { x: 1.0, y: 1.0 }), PathVerb::Close]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathVerb {
    /// Starts a new contour at the point.
    MoveTo(Point),
    /// A straight line to the point.
    LineTo(Point),
    /// A quadratic Bézier curve: control point, then end point.
    QuadTo(Point, Point),
    /// A cubic Bézier curve: two control points, then end point.
    CubicTo(Point, Point, Point),
    /// Closes the current contour.
    Close,
}

impl PathVerb {
    fn translated(&self, dx: f32, dy: f32) -> PathVerb {
        match *self {
            PathVerb::MoveTo(p) => PathVerb::MoveTo(p.translated(dx, dy)),
            PathVerb::LineTo(p) => PathVerb::LineTo(p.translated(dx, dy)),
            PathVerb::QuadTo(c, p) => PathVerb::QuadTo(c.translated(dx, dy), p.translated(dx, dy)),
            PathVerb::CubicTo(c1, c2, p) => PathVerb::CubicTo(
                c1.translated(dx, dy),
                c2.translated(dx, dy),
                p.translated(dx, dy),
            ),
            PathVerb::Close => PathVerb::Close,
        }
    }

    /// Calls `f` on every point of this verb, control points included.
    fn for_each_point(&self, mut f: impl FnMut(Point)) {
        match *self {
            PathVerb::MoveTo(p) | PathVerb::LineTo(p) => f(p),
            PathVerb::QuadTo(c, p) => {
                f(c);
                f(p);
            }
            PathVerb::CubicTo(c1, c2, p) => {
                f(c1);
                f(c2);
                f(p);
            }
            PathVerb::Close => {}
        }
    }
}

/// A sequence of contours made of lines and Bézier curves, in points, y-down.
///
/// # Examples
///
/// ```
/// use umber_core::{Path, Point};
///
/// let mut square = Path::new();
/// square.move_to(Point { x: 0.0, y: 0.0 });
/// square.line_to(Point { x: 4.0, y: 0.0 });
/// square.line_to(Point { x: 4.0, y: 4.0 });
/// square.line_to(Point { x: 0.0, y: 4.0 });
/// square.close();
/// assert_eq!(square.verbs().len(), 5);
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Path {
    verbs: Vec<PathVerb>,
}

impl Path {
    /// Creates an empty path.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Path;
    ///
    /// assert!(Path::new().is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Path {
        Path { verbs: Vec::new() }
    }

    /// Starts a new contour at `p`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Path, PathVerb, Point};
    ///
    /// let mut path = Path::new();
    /// path.move_to(Point { x: 2.0, y: 3.0 });
    /// assert_eq!(path.verbs(), &[PathVerb::MoveTo(Point { x: 2.0, y: 3.0 })]);
    /// ```
    pub fn move_to(&mut self, p: Point) {
        self.verbs.push(PathVerb::MoveTo(p));
    }

    /// Adds a straight line to `p`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Path, PathVerb, Point};
    ///
    /// let mut path = Path::new();
    /// path.move_to(Point { x: 0.0, y: 0.0 });
    /// path.line_to(Point { x: 1.0, y: 0.0 });
    /// assert_eq!(path.verbs()[1], PathVerb::LineTo(Point { x: 1.0, y: 0.0 }));
    /// ```
    pub fn line_to(&mut self, p: Point) {
        self.verbs.push(PathVerb::LineTo(p));
    }

    /// Adds a quadratic Bézier curve with control point `c` that ends at `p`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Path, PathVerb, Point};
    ///
    /// let mut path = Path::new();
    /// path.move_to(Point { x: 0.0, y: 0.0 });
    /// path.quad_to(Point { x: 1.0, y: 1.0 }, Point { x: 2.0, y: 0.0 });
    /// assert!(matches!(path.verbs()[1], PathVerb::QuadTo(..)));
    /// ```
    pub fn quad_to(&mut self, c: Point, p: Point) {
        self.verbs.push(PathVerb::QuadTo(c, p));
    }

    /// Adds a cubic Bézier curve with control points `c1` and `c2` that ends at `p`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Path, PathVerb, Point};
    ///
    /// let mut path = Path::new();
    /// path.move_to(Point { x: 0.0, y: 0.0 });
    /// path.cubic_to(Point { x: 0.0, y: 1.0 }, Point { x: 1.0, y: 1.0 }, Point { x: 1.0, y: 0.0 });
    /// assert!(matches!(path.verbs()[1], PathVerb::CubicTo(..)));
    /// ```
    pub fn cubic_to(&mut self, c1: Point, c2: Point, p: Point) {
        self.verbs.push(PathVerb::CubicTo(c1, c2, p));
    }

    /// Closes the current contour.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Path, PathVerb, Point};
    ///
    /// let mut path = Path::new();
    /// path.move_to(Point { x: 0.0, y: 0.0 });
    /// path.close();
    /// assert_eq!(path.verbs().last(), Some(&PathVerb::Close));
    /// ```
    pub fn close(&mut self) {
        self.verbs.push(PathVerb::Close);
    }

    /// The drawing commands, in order.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Path;
    ///
    /// assert!(Path::new().verbs().is_empty());
    /// ```
    #[must_use]
    pub fn verbs(&self) -> &[PathVerb] {
        &self.verbs
    }

    /// Whether the path has no drawing commands.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Path, Point};
    ///
    /// let mut path = Path::new();
    /// assert!(path.is_empty());
    /// path.move_to(Point { x: 0.0, y: 0.0 });
    /// assert!(!path.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    /// The axis-aligned bounding box `(min, max)` of every point, control points included.
    ///
    /// Returns `None` when the path has no points.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Path, Point};
    ///
    /// let mut path = Path::new();
    /// assert_eq!(path.bounds(), None);
    /// path.move_to(Point { x: 0.0, y: 0.0 });
    /// path.quad_to(Point { x: 5.0, y: -4.0 }, Point { x: 10.0, y: 0.0 });
    /// assert_eq!(path.bounds(), Some((Point { x: 0.0, y: -4.0 }, Point { x: 10.0, y: 0.0 })));
    /// ```
    #[must_use]
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let mut bounds: Option<(Point, Point)> = None;
        for verb in &self.verbs {
            verb.for_each_point(|p| {
                bounds = Some(match bounds {
                    None => (p, p),
                    Some((min, max)) => (
                        Point {
                            x: min.x.min(p.x),
                            y: min.y.min(p.y),
                        },
                        Point {
                            x: max.x.max(p.x),
                            y: max.y.max(p.y),
                        },
                    ),
                });
            });
        }
        bounds
    }

    /// Returns a copy of this path moved by `dx` and `dy`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Path, PathVerb, Point};
    ///
    /// let mut path = Path::new();
    /// path.move_to(Point { x: 1.0, y: 1.0 });
    /// let moved = path.translated(2.0, 3.0);
    /// assert_eq!(moved.verbs(), &[PathVerb::MoveTo(Point { x: 3.0, y: 4.0 })]);
    /// ```
    #[must_use]
    pub fn translated(&self, dx: f32, dy: f32) -> Path {
        Path {
            verbs: self.verbs.iter().map(|v| v.translated(dx, dy)).collect(),
        }
    }

    /// Appends every drawing command of `other` to this path.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Path, Point};
    ///
    /// let mut a = Path::new();
    /// a.move_to(Point { x: 0.0, y: 0.0 });
    /// let mut b = Path::new();
    /// b.move_to(Point { x: 1.0, y: 1.0 });
    /// b.close();
    /// a.extend(&b);
    /// assert_eq!(a.verbs().len(), 3);
    /// ```
    pub fn extend(&mut self, other: &Path) {
        self.verbs.extend_from_slice(&other.verbs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    #[test]
    fn new_and_default_are_empty() {
        assert!(Path::new().is_empty());
        assert_eq!(Path::default(), Path::new());
        assert_eq!(Path::new().bounds(), None);
    }

    #[test]
    fn builders_record_verbs_in_order() {
        let mut path = Path::new();
        path.move_to(pt(0.0, 0.0));
        path.line_to(pt(1.0, 0.0));
        path.quad_to(pt(2.0, 1.0), pt(3.0, 0.0));
        path.cubic_to(pt(4.0, 1.0), pt(5.0, -1.0), pt(6.0, 0.0));
        path.close();
        assert_eq!(
            path.verbs(),
            &[
                PathVerb::MoveTo(pt(0.0, 0.0)),
                PathVerb::LineTo(pt(1.0, 0.0)),
                PathVerb::QuadTo(pt(2.0, 1.0), pt(3.0, 0.0)),
                PathVerb::CubicTo(pt(4.0, 1.0), pt(5.0, -1.0), pt(6.0, 0.0)),
                PathVerb::Close,
            ]
        );
        assert!(!path.is_empty());
    }

    #[test]
    fn bounds_cover_control_points() {
        let mut path = Path::new();
        path.move_to(pt(0.0, 0.0));
        path.cubic_to(pt(1.0, -5.0), pt(2.0, 7.0), pt(3.0, 0.0));
        assert_eq!(path.bounds(), Some((pt(0.0, -5.0), pt(3.0, 7.0))));

        let mut quad = Path::new();
        quad.move_to(pt(0.0, 0.0));
        quad.quad_to(pt(-2.0, 9.0), pt(1.0, 1.0));
        assert_eq!(quad.bounds(), Some((pt(-2.0, 0.0), pt(1.0, 9.0))));
    }

    #[test]
    fn close_only_path_has_no_bounds() {
        let mut path = Path::new();
        path.close();
        assert!(!path.is_empty());
        assert_eq!(path.bounds(), None);
    }

    #[test]
    fn translated_moves_every_point() {
        let mut path = Path::new();
        path.move_to(pt(0.0, 0.0));
        path.line_to(pt(1.0, 1.0));
        path.quad_to(pt(2.0, 2.0), pt(3.0, 3.0));
        path.cubic_to(pt(4.0, 4.0), pt(5.0, 5.0), pt(6.0, 6.0));
        path.close();
        let moved = path.translated(10.0, -1.0);
        assert_eq!(
            moved.verbs(),
            &[
                PathVerb::MoveTo(pt(10.0, -1.0)),
                PathVerb::LineTo(pt(11.0, 0.0)),
                PathVerb::QuadTo(pt(12.0, 1.0), pt(13.0, 2.0)),
                PathVerb::CubicTo(pt(14.0, 3.0), pt(15.0, 4.0), pt(16.0, 5.0)),
                PathVerb::Close,
            ]
        );
        assert_eq!(
            path.verbs()[0],
            PathVerb::MoveTo(pt(0.0, 0.0)),
            "original untouched"
        );
    }

    #[test]
    fn extend_appends() {
        let mut a = Path::new();
        a.move_to(pt(0.0, 0.0));
        let mut b = Path::new();
        b.move_to(pt(5.0, 5.0));
        b.line_to(pt(6.0, 6.0));
        a.extend(&b);
        assert_eq!(a.verbs().len(), 3);
        assert_eq!(a.bounds(), Some((pt(0.0, 0.0), pt(6.0, 6.0))));
        a.extend(&Path::new());
        assert_eq!(a.verbs().len(), 3);
    }

    #[test]
    fn point_translated() {
        assert_eq!(pt(1.0, 2.0).translated(0.5, 0.5), pt(1.5, 2.5));
    }
}
