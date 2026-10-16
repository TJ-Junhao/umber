//! Bar outlines in plot points, shared by bars and histogram bins: where bars start, the
//! clipping of their ends in axis space, the mapping to plot points, and the outline itself, a
//! rectangle whose end away from the baseline may be rounded; and the rounded rectangle that
//! outline is built on, which also draws the pills of the chips legend.
//!
//! A frame's axes are those of its axis maps (see [`super::mapping`]): "x" is the axis the bars
//! stand on and "y" the axis they grow along, the plot's own for vertical bars, and swapped
//! for horizontal ones.

use super::mapping::{AxisMap, AxisMaps};
use crate::Transform;
use crate::geometry::{Path, Point};
use crate::resolve::{AxisScale, NormalizedPos, ResolvedAxis};

/// The cubic Bézier handle length for a quarter circle of unit radius.
const KAPPA: f64 = 0.552_284_749_830_793_4;
/// The smallest bar clip margin, in points.
const MIN_BAR_MARGIN: f64 = 2.0;

/// Where bars start on the y axis `y`, in axis space: the value 0 on a linear axis, and on a
/// log axis, which cannot show 0, the resolved `baseline` (the lowest nice tick for auto
/// limits, the bottom of the plot for explicit ones). A category axis has no 0 either, so bars
/// along it would start at the baseline too; the model draws none there.
fn bar_base(y: &ResolvedAxis, map: AxisMap, baseline: NormalizedPos) -> f64 {
    match &y.scale {
        AxisScale::Continuous(scale) => match scale.transform() {
            Transform::Linear => 0.0,
            Transform::Log(_) => map.at(baseline),
        },
        AxisScale::Category(_) | AxisScale::Time(_) => map.at(baseline),
    }
}

/// What the bars of one mark share: the axis maps, where bars start, the corner radius and the
/// axis-space range bar ends are clipped to.
///
/// Bars are clipped in axis space against the y domain expanded by `max(r, 2)` pt, where `r`
/// is the corner radius as [`bar_outline`] clamps it to half the bar width; only the clipped
/// ends are mapped to plot points. An end is rounded only when it may be rounded and its true
/// value lies inside that expanded domain; a cut end is square and lies at least `r` beyond the
/// plot area, outside the marks clip.
#[derive(Debug, Clone, Copy)]
pub(super) struct BarFrame {
    /// The axis maps.
    maps: AxisMaps,
    /// Where bars start, in y axis space; see [`bar_base`].
    base: f64,
    /// The corner radius, in points.
    radius: f64,
    /// The clip margin, in points.
    margin: f64,
    /// The y range, in axis space, that bar ends are clipped to.
    low: f64,
    /// See `low`.
    high: f64,
}

impl BarFrame {
    /// The frame of bars `width` points wide whose ends may be rounded by `radius` points (at
    /// least 0 and finite), on the y axis `y` whose baseline sits at `baseline`.
    pub(super) fn new(
        maps: AxisMaps,
        y: &ResolvedAxis,
        baseline: NormalizedPos,
        radius: f64,
        width: f64,
    ) -> BarFrame {
        let base = bar_base(y, maps.y, baseline);
        let margin = radius.min(width / 2.0).max(MIN_BAR_MARGIN);
        let (low, high) = maps.y.expanded(margin);
        BarFrame {
            maps,
            base,
            radius,
            margin,
            low,
            high,
        }
    }

    /// The frame of bars with square corners on the y axis `y`, clipped with the smallest
    /// margin; see [`BarFrame::new`].
    pub(super) fn square(maps: AxisMaps, y: &ResolvedAxis, baseline: NormalizedPos) -> BarFrame {
        BarFrame::new(maps, y, baseline, 0.0, 0.0)
    }

    /// Where bars start, in y axis space: 0 on a linear axis, the baseline on a log axis.
    pub(super) fn base(&self) -> f64 {
        self.base
    }

    /// `v` (y axis space) clipped to the range bar ends are clipped to. NaN stays NaN.
    pub(super) fn clip_y(&self, v: f64) -> f64 {
        // `max` then `min` never panics, unlike `clamp`, and keeps finite values within the
        // range.
        v.max(self.low).min(self.high)
    }

    /// Whether the y axis-space value `v` lies inside the range bar ends are clipped to, so
    /// that it is drawn where it truly is.
    pub(super) fn contains_y(&self, v: f64) -> bool {
        (self.low..=self.high).contains(&v)
    }

    /// The x range `(left, right)` (axis space, `left <= right`) clipped to the x domain
    /// expanded by the clip margin; `None` when nothing of it is left. Category bands always
    /// lie inside the x domain, so only bars on a continuous x axis need this.
    pub(super) fn clip_x(&self, (left, right): (f64, f64)) -> Option<(f64, f64)> {
        let (low, high) = self.maps.x.expanded(self.margin);
        let clip = |v: f64| v.max(low).min(high);
        let (left, right) = (clip(left), clip(right));
        (left < right).then_some((left, right))
    }

    /// The plot y of the y axis-space value `v`, clipped first as [`clip_y`](Self::clip_y)
    /// does, so it is never far outside the plot area.
    pub(super) fn plot_y(&self, v: f64) -> f64 {
        self.maps.y.plot_at(self.clip_y(v))
    }

    /// The outline of one bar across `(left, right)` in x axis space, from `from` to `to` in y
    /// axis space, in plot points: built in the frame's plot points and then placed in the
    /// scene, its axes swapped in the frame of horizontal bars. The end at `to` is rounded when
    /// `round` holds and `to` lies inside the clip range. `None` when either end is NaN, the
    /// ends are equal, the whole bar lies beyond one end of the clip range, or the outline is
    /// empty.
    pub(super) fn outline(
        &self,
        (left, right): (f64, f64),
        from: f64,
        to: f64,
        round: bool,
    ) -> Option<Path> {
        if from.is_nan() || to.is_nan() || from == to {
            return None;
        }
        let (visible_from, visible_to) = (self.clip_y(from), self.clip_y(to));
        if visible_from == visible_to {
            return None;
        }
        let round_end = round && self.contains_y(to);
        let (x, y) = (self.maps.x, self.maps.y);
        let path = bar_outline(
            x.plot_at(left),
            x.plot_at(right),
            y.plot_at(visible_from),
            y.plot_at(visible_to),
            self.radius,
            round_end,
        );
        (!path.is_empty()).then(|| self.maps.path_to_scene(path))
    }
}

/// A bar from `base_y` to `end_y` between `x0` and `x1`, in plot points, y down.
///
/// The end at `end_y` is rounded with quarter circles of `radius`, clamped to
/// `min(radius, width / 2, |end_y - base_y|)`; `end_y` may lie above or below `base_y`.
/// `round_end = false` draws square corners. A bar without width or height, or with a
/// coordinate that is not finite, gives an empty path.
///
/// All arithmetic is in `f64`; each coordinate is narrowed to `f32` once, at the end. The
/// caller passes coordinates already clipped to `MAX_COORD`: narrowing an arbitrary finite
/// `f64` beyond the `f32` range would overflow.
pub(super) fn bar_outline(
    x0: f64,
    x1: f64,
    base_y: f64,
    end_y: f64,
    radius: f64,
    round_end: bool,
) -> Path {
    let end_radius = if round_end { radius } else { 0.0 };
    rounded_rect(x0, x1, base_y, end_y, 0.0, end_radius)
}

/// A rectangle between `x0` and `x1` and from `base_y` to `end_y`, in points, y down, whose two
/// corners at `end_y` are rounded with quarter circles of `end_radius` and whose two corners at
/// `base_y` with quarter circles of `base_radius`.
///
/// The end radius is clamped to `min(end_radius, width / 2, height)` and the base radius to
/// `min(base_radius, width / 2, height - end radius)`, so the corners never overlap; a negative
/// or NaN radius is 0, a square corner. A pill is `base_radius = end_radius = height / 2`.
/// `end_y` may lie above or below `base_y`. A rectangle without width or height, or with a
/// coordinate that is not finite, gives an empty path.
///
/// The path starts at the base on the `x0` side and runs up that side, along the end and down
/// the `x1` side. All arithmetic is in `f64`; each coordinate is narrowed to `f32` once, at the
/// end, so the caller passes coordinates within the `f32` range.
pub(super) fn rounded_rect(
    x0: f64,
    x1: f64,
    base_y: f64,
    end_y: f64,
    base_radius: f64,
    end_radius: f64,
) -> Path {
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (width, height) = (x1 - x0, (end_y - base_y).abs());
    let mut path = Path::new();
    if !(width > 0.0 && height > 0.0 && width.is_finite() && height.is_finite()) {
        return path;
    }
    let re = end_radius.max(0.0).min(width / 2.0).min(height);
    let rb = base_radius.max(0.0).min(width / 2.0).min(height - re);
    // `away` points from the base towards the end.
    let away = (end_y - base_y).signum();
    let p = |x: f64, y: f64| Point {
        x: x as f32,
        y: y as f32,
    };
    // Square base corners use `base_y` itself, so a bar's base keeps its exact bits.
    let base_shoulder = if rb > 0.0 { base_y + away * rb } else { base_y };
    path.move_to(p(x0, base_shoulder));
    if re > 0.0 {
        let k = KAPPA * re;
        let shoulder = end_y - away * re;
        path.line_to(p(x0, shoulder));
        path.cubic_to(
            p(x0, shoulder + away * k),
            p(x0 + re - k, end_y),
            p(x0 + re, end_y),
        );
        path.line_to(p(x1 - re, end_y));
        path.cubic_to(
            p(x1 - re + k, end_y),
            p(x1, shoulder + away * k),
            p(x1, shoulder),
        );
    } else {
        path.line_to(p(x0, end_y));
        path.line_to(p(x1, end_y));
    }
    path.line_to(p(x1, base_shoulder));
    if rb > 0.0 {
        let k = KAPPA * rb;
        path.cubic_to(
            p(x1, base_shoulder - away * k),
            p(x1 - rb + k, base_y),
            p(x1 - rb, base_y),
        );
        path.line_to(p(x0 + rb, base_y));
        path.cubic_to(
            p(x0 + rb - k, base_y),
            p(x0, base_shoulder - away * k),
            p(x0, base_shoulder),
        );
    }
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
    fn the_visible_end_of_a_stack_keeps_its_rounding() {
        use crate::stacking::Stacks;

        // 1e100 + 1.0 == 1e100: the second series spans nothing and must not take the end.
        for sign in [1.0_f64, -1.0] {
            let (big, tiny) = ([sign * 1e100], [sign]);
            let stacks = Stacks::new(1, &[&big, &tiny]);
            let segment = stacks.segment(0, 0).unwrap();
            // Maps 0..1e100 onto 100..0 points, y down.
            let end_y = 100.0 - segment.top / 1e100 * 100.0;
            let path = bar_outline(0.0, 20.0, 100.0, end_y, 4.0, segment.outermost);
            assert_eq!(cubics(&path), 2);
        }
    }

    #[test]
    fn a_pill_rounds_all_four_corners_with_half_its_height() {
        // A 40 by 10 box from y 20 (base) to y 10 (end).
        let path = rounded_rect(0.0, 40.0, 20.0, 10.0, 5.0, 5.0);
        assert_eq!(cubics(&path), 4);
        assert_eq!(
            bounds(&path),
            (Point { x: 0.0, y: 10.0 }, Point { x: 40.0, y: 20.0 })
        );
        // The flat top and bottom run between the end points of the half circles.
        assert_eq!(end_points_at(&path, 10.0), vec![5.0, 35.0]);
        assert_eq!(end_points_at(&path, 20.0), vec![35.0, 5.0]);
        // Both sides are a single point, at mid height.
        assert_eq!(end_points_at(&path, 15.0), vec![0.0, 0.0, 40.0, 40.0, 0.0]);
    }

    #[test]
    fn rounded_rect_corners_never_overlap() {
        // The end radius takes the height first; the base gets what is left.
        let path = rounded_rect(0.0, 100.0, 10.0, 0.0, 8.0, 6.0);
        assert_eq!(end_points_at(&path, 0.0), vec![6.0, 94.0]);
        assert_eq!(end_points_at(&path, 10.0), vec![96.0, 4.0]);
        // Both radii are limited to half the width.
        let path = rounded_rect(0.0, 4.0, 100.0, 0.0, 50.0, 50.0);
        assert_eq!(end_points_at(&path, 0.0), vec![2.0, 2.0]);
        assert_eq!(end_points_at(&path, 100.0), vec![2.0, 2.0]);
        // Negative and NaN radii are square corners.
        assert_eq!(
            cubics(&rounded_rect(0.0, 4.0, 10.0, 0.0, -1.0, f64::NAN)),
            0
        );
        assert!(rounded_rect(0.0, 4.0, 10.0, 10.0, 1.0, 1.0).is_empty());
    }

    #[test]
    fn a_bar_base_keeps_its_exact_bits() {
        // The square base starts at `base_y` itself, the sign of zero included.
        let path = bar_outline(10.0, 30.0, -0.0, 30.0, 4.0, true);
        let Some(PathVerb::MoveTo(start)) = path.verbs().first() else {
            panic!("the outline starts with a move");
        };
        assert!(start.y == 0.0 && start.y.is_sign_negative());
    }

    #[test]
    fn reversed_x_gives_the_same_outline() {
        assert_eq!(
            bar_outline(30.0, 10.0, 100.0, 60.0, 4.0, true),
            bar_outline(10.0, 30.0, 100.0, 60.0, 4.0, true)
        );
    }
}
