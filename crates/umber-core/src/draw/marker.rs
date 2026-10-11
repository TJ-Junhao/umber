//! Marker shapes, centered at (0, 0), and the marker diameter drawn.

use crate::geometry::{Path, Point};
use crate::{Marker, Theme};

/// The largest marker diameter drawn, whatever the theme says.
const MAX_MARKER_SIZE: f32 = 1.0e4;
/// The side of a square with the area of a unit-diameter circle: `sqrt(pi) / 2`.
const SQUARE_SIDE: f32 = 0.886;
/// The cubic Bézier handle length for a quarter circle of unit radius.
const KAPPA: f32 = 0.552_284_8;

/// A closed polygon through `points`.
pub(super) fn polygon(points: &[(f32, f32)]) -> Path {
    let mut path = Path::new();
    let mut iter = points.iter().map(|&(x, y)| Point { x, y });
    if let Some(first) = iter.next() {
        path.move_to(first);
        iter.for_each(|p| path.line_to(p));
        path.close();
    }
    path
}

/// The marker `shape` for a nominal `diameter`, centered at (0, 0).
///
/// - Circle: diameter `d`, from 4 cubics.
/// - Square: side `0.886 d`, the area of the circle.
/// - Triangle: equilateral with its centroid at 0, pointing up, with the area of the circle
///   (side `d * sqrt(pi / sqrt(3))`).
/// - Diamond: the square rotated by 45°.
/// - Cross: a filled plus `d` across, with arms `d / 3` wide.
pub(super) fn marker_shape(shape: Marker, diameter: f32) -> Path {
    let r = diameter / 2.0;
    match shape {
        Marker::Circle => {
            let k = KAPPA * r;
            let p = |x: f32, y: f32| Point { x, y };
            let mut path = Path::new();
            path.move_to(p(r, 0.0));
            path.cubic_to(p(r, k), p(k, r), p(0.0, r));
            path.cubic_to(p(-k, r), p(-r, k), p(-r, 0.0));
            path.cubic_to(p(-r, -k), p(-k, -r), p(0.0, -r));
            path.cubic_to(p(k, -r), p(r, -k), p(r, 0.0));
            path.close();
            path
        }
        Marker::Square => {
            let h = SQUARE_SIDE * diameter / 2.0;
            polygon(&[(-h, -h), (h, -h), (h, h), (-h, h)])
        }
        Marker::Triangle => {
            let side = diameter * (std::f32::consts::PI / 3.0_f32.sqrt()).sqrt();
            let height = side * 3.0_f32.sqrt() / 2.0;
            // The centroid sits a third of the height above the base; y points down.
            let (apex, base) = (-2.0 * height / 3.0, height / 3.0);
            polygon(&[(0.0, apex), (side / 2.0, base), (-side / 2.0, base)])
        }
        Marker::Diamond => {
            let h = SQUARE_SIDE * diameter * std::f32::consts::FRAC_1_SQRT_2;
            polygon(&[(0.0, -h), (h, 0.0), (0.0, h), (-h, 0.0)])
        }
        Marker::Cross => {
            let a = diameter / 6.0;
            polygon(&[
                (-a, -r),
                (a, -r),
                (a, -a),
                (r, -a),
                (r, a),
                (a, a),
                (a, r),
                (-a, r),
                (-a, a),
                (-r, a),
                (-r, -a),
                (-a, -a),
            ])
        }
    }
}

/// The largest `|x|` and `|y|` of any point of `path`, control points included.
pub(super) fn extent(path: &Path) -> (f64, f64) {
    path.bounds().map_or((0.0, 0.0), |(min, max)| {
        (
            f64::from(min.x.abs().max(max.x.abs())),
            f64::from(min.y.abs().max(max.y.abs())),
        )
    })
}

/// The marker diameter: the theme's, finite and within `0..=MAX_MARKER_SIZE`.
pub(super) fn marker_size(theme: &Theme) -> f32 {
    let d = theme.marker_size();
    if d.is_finite() && d > 0.0 {
        d.min(MAX_MARKER_SIZE)
    } else {
        0.0
    }
}
