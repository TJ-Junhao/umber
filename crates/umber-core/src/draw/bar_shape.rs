//! The bar outline: a rectangle in plot points whose end away from the baseline is rounded.

#![allow(dead_code, reason = "bar and histogram drawing will use it")]

use crate::geometry::{Path, Point};

/// The cubic Bézier handle length for a quarter circle of unit radius.
const KAPPA: f64 = 0.552_284_749_830_793_4;

/// A bar from `base_y` to `end_y` between `x0` and `x1`, in plot points, y down.
///
/// The end at `end_y` is rounded with quarter circles of `radius`, clamped to
/// `min(radius, width / 2, |end_y - base_y|)`; `end_y` may lie above or below `base_y`.
/// `round_end = false` draws square corners. A bar without width or height, or with a
/// coordinate that is not finite, gives an empty path.
///
/// All arithmetic is in `f64`; each coordinate is narrowed to `f32` once, at the end.
pub(super) fn bar_outline(
    x0: f64,
    x1: f64,
    base_y: f64,
    end_y: f64,
    radius: f64,
    round_end: bool,
) -> Path {
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (width, height) = (x1 - x0, (end_y - base_y).abs());
    let mut path = Path::new();
    if !(width > 0.0 && height > 0.0 && width.is_finite() && height.is_finite()) {
        return path;
    }
    let r = if round_end {
        radius.max(0.0).min(width / 2.0).min(height)
    } else {
        0.0
    };
    // `away` points from the base towards the end.
    let away = (end_y - base_y).signum();
    let p = |x: f64, y: f64| Point {
        x: x as f32,
        y: y as f32,
    };
    path.move_to(p(x0, base_y));
    if r > 0.0 {
        let k = KAPPA * r;
        let shoulder = end_y - away * r;
        path.line_to(p(x0, shoulder));
        path.cubic_to(
            p(x0, shoulder + away * k),
            p(x0 + r - k, end_y),
            p(x0 + r, end_y),
        );
        path.line_to(p(x1 - r, end_y));
        path.cubic_to(
            p(x1 - r + k, end_y),
            p(x1, shoulder + away * k),
            p(x1, shoulder),
        );
    } else {
        path.line_to(p(x0, end_y));
        path.line_to(p(x1, end_y));
    }
    path.line_to(p(x1, base_y));
    path.close();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::PathVerb;

    fn bounds(path: &Path) -> (Point, Point) {
        path.bounds().unwrap()
    }

    fn cubics(path: &Path) -> usize {
        path.verbs()
            .iter()
            .filter(|v| matches!(v, PathVerb::CubicTo(..)))
            .count()
    }

    /// The points of the path where it touches `y`, excluding control points.
    fn end_points_at(path: &Path, y: f32) -> Vec<f32> {
        path.verbs()
            .iter()
            .filter_map(|v| match v {
                PathVerb::MoveTo(p) | PathVerb::LineTo(p) | PathVerb::CubicTo(_, _, p)
                    if p.y == y =>
                {
                    Some(p.x)
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn square_corners_make_a_rectangle() {
        let path = bar_outline(10.0, 20.0, 100.0, 60.0, 4.0, false);
        assert_eq!(cubics(&path), 0);
        assert_eq!(
            bounds(&path),
            (Point { x: 10.0, y: 60.0 }, Point { x: 20.0, y: 100.0 })
        );
    }

    #[test]
    fn the_end_away_from_the_base_is_rounded() {
        let path = bar_outline(10.0, 30.0, 100.0, 60.0, 4.0, true);
        assert_eq!(cubics(&path), 2);
        // The base corners stay square; the end corners are pulled in by the radius.
        assert_eq!(end_points_at(&path, 100.0), vec![10.0, 30.0]);
        assert_eq!(end_points_at(&path, 60.0), vec![14.0, 26.0]);
        assert_eq!(
            bounds(&path),
            (Point { x: 10.0, y: 60.0 }, Point { x: 30.0, y: 100.0 })
        );
    }

    #[test]
    fn outline_radius_is_clamped() {
        // Clamped to half the width.
        let path = bar_outline(0.0, 10.0, 100.0, 0.0, 50.0, true);
        assert_eq!(end_points_at(&path, 0.0), vec![5.0, 5.0]);
        // Clamped to the height.
        let path = bar_outline(0.0, 100.0, 10.0, 7.0, 50.0, true);
        assert_eq!(end_points_at(&path, 7.0), vec![3.0, 97.0]);
        // A negative radius is square.
        let path = bar_outline(0.0, 10.0, 10.0, 0.0, -3.0, true);
        assert_eq!(cubics(&path), 0);
        // The curve never leaves the bar.
        let path = bar_outline(0.0, 10.0, 100.0, 0.0, 50.0, true);
        assert_eq!(
            bounds(&path),
            (Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 100.0 })
        );
    }

    #[test]
    fn negative_bars_round_the_bottom() {
        // y is down, so a negative value ends below its base.
        let path = bar_outline(10.0, 30.0, 60.0, 100.0, 4.0, true);
        assert_eq!(cubics(&path), 2);
        assert_eq!(end_points_at(&path, 60.0), vec![10.0, 30.0]);
        assert_eq!(end_points_at(&path, 100.0), vec![14.0, 26.0]);
        assert_eq!(
            bounds(&path),
            (Point { x: 10.0, y: 60.0 }, Point { x: 30.0, y: 100.0 })
        );
    }

    #[test]
    fn degenerate_bars_are_empty() {
        assert!(bar_outline(5.0, 5.0, 10.0, 0.0, 2.0, true).is_empty());
        assert!(bar_outline(0.0, 10.0, 4.0, 4.0, 2.0, true).is_empty());
        assert!(bar_outline(0.0, f64::NAN, 4.0, 0.0, 2.0, true).is_empty());
        assert!(bar_outline(0.0, 10.0, 4.0, f64::INFINITY, 2.0, true).is_empty());
    }

    #[test]
    fn reversed_x_gives_the_same_outline() {
        assert_eq!(
            bar_outline(30.0, 10.0, 100.0, 60.0, 4.0, true),
            bar_outline(10.0, 30.0, 100.0, 60.0, 4.0, true)
        );
    }
}
