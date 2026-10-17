//! Geometry, dash and gradient normalization shared by the render backends.
//!
//! Scenes may hold values no backend can draw: non-finite or absurdly large coordinates,
//! degenerate dashes, gradients with no usable stops. Both backends decide what such values
//! mean here, once, and then draw only what this module returns. That keeps the SVG and PNG
//! output in agreement on hostile input.

use crate::color::Rgba;
use crate::geometry::{Path, PathVerb, Point};
use crate::scene::{GradientStop, LinearGradient, Rect, StrokeStyle};

/// Largest coordinate magnitude, in points, that a backend will draw.
///
/// A point (control points included) beyond this, or non-finite, makes its segment invalid.
/// Rasterizers can abort on huge but finite control points; mark drawing clips data to
/// the plot area before it builds scenes, so legitimate geometry never gets near this limit.
pub(crate) const MAX_COORD: f32 = 1.0e6;

/// Gradient vectors shorter than this many points are degenerate.
const MIN_EXTENT: f64 = 0.01;

/// Dash periods below this many points are sub-resolution at every allowed DPI (0.1 pt is 3.3
/// px at 2400 dpi), so such a pattern is drawn solid.
const MIN_DASH_PERIOD: f64 = 0.1;

/// A pattern that would produce more dashes than this along the path is drawn solid. The
/// count is estimated the way tiny-skia does, which has a looser budget of 1,000,000.
const MAX_DASH_COUNT: f64 = 100_000.0;

/// Whether a point can be drawn: finite and within [`MAX_COORD`].
pub(crate) fn valid_point(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.x.abs() <= MAX_COORD && p.y.abs() <= MAX_COORD
}

/// Whether a stroke style paints anything: a visible color and a finite, positive width.
///
/// Both backends skip a stroke that fails this, whatever its path or dash.
pub(crate) fn stroke_is_visible(style: &StrokeStyle) -> bool {
    style.color.a != 0 && style.width.is_finite() && style.width > 0.0
}

/// Whether a clip rectangle can clip: finite in every field, with a positive size.
///
/// A group whose clip fails this draws nothing in either backend.
pub(crate) fn clip_is_valid(rect: &Rect) -> bool {
    [rect.x, rect.y, rect.width, rect.height]
        .iter()
        .all(|v| v.is_finite())
        && rect.width > 0.0
        && rect.height > 0.0
}

/// Whether an image can be drawn: a rectangle that is finite, positive in size and within
/// [`MAX_COORD`] at every corner, at least one pixel, and `width * height` pixels.
///
/// Both backends skip an image that fails this.
pub(crate) fn image_is_drawable(rect: &Rect, width: u32, height: u32, pixels: &[Rgba]) -> bool {
    let corners = [
        Point {
            x: rect.x,
            y: rect.y,
        },
        Point {
            x: rect.x + rect.width,
            y: rect.y + rect.height,
        },
    ];
    clip_is_valid(rect)
        && corners.into_iter().all(valid_point)
        && width > 0
        && height > 0
        && u64::from(width) * u64::from(height) == pixels.len() as u64
}

/// Whether a text item can be drawn: a visible color and an origin within [`MAX_COORD`].
///
/// Both backends skip text that fails this, in every SVG text mode.
pub(crate) fn text_is_visible(color: Rgba, origin: Point) -> bool {
    color.a != 0 && valid_point(origin)
}

/// One drawable segment of a subpath.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Segment {
    /// A straight line to the point.
    Line(Point),
    /// A quadratic curve: control point, end point.
    Quad(Point, Point),
    /// A cubic curve: two control points, end point.
    Cubic(Point, Point, Point),
}

impl Segment {
    fn end(self) -> Point {
        match self {
            Segment::Line(p) | Segment::Quad(_, p) | Segment::Cubic(_, _, p) => p,
        }
    }
}

/// A start point followed by at least one segment.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Subpath {
    start: Point,
    segments: Vec<Segment>,
    closed: bool,
}

impl Subpath {
    /// Where the subpath starts.
    pub(crate) fn start(&self) -> Point {
        self.start
    }

    /// The segments, never empty in a [`NormalizedPath`].
    pub(crate) fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Whether the subpath is closed.
    pub(crate) fn is_closed(&self) -> bool {
        self.closed
    }
}

/// A path reduced to what can be drawn: valid subpaths with at least one segment each.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NormalizedPath {
    subpaths: Vec<Subpath>,
}

impl NormalizedPath {
    /// The subpaths, never empty.
    pub(crate) fn subpaths(&self) -> &[Subpath] {
        &self.subpaths
    }

    /// The length of the control polygon, an upper bound on the path length.
    pub(crate) fn control_length(&self) -> f32 {
        let mut total = 0.0_f64;
        let mut dist = |a: Point, b: Point| {
            total += f64::from(b.x - a.x).hypot(f64::from(b.y - a.y));
        };
        for sub in &self.subpaths {
            let mut at = sub.start;
            for seg in &sub.segments {
                match *seg {
                    Segment::Line(p) => dist(at, p),
                    Segment::Quad(c, p) => {
                        dist(at, c);
                        dist(c, p);
                    }
                    Segment::Cubic(c1, c2, p) => {
                        dist(at, c1);
                        dist(c1, c2);
                        dist(c2, p);
                    }
                }
                at = seg.end();
            }
            if sub.closed {
                dist(at, sub.start);
            }
        }
        total as f32
    }
}

/// Reduces `path` to drawable subpaths, or `None` if nothing is drawable.
///
/// - An invalid segment ends the current subpath.
/// - A curve without a valid current point is dropped.
/// - A `LineTo` without a current point starts a new subpath at its end point.
/// - `Close` is kept only on a subpath that has at least one segment; the current point then
///   returns to the subpath start.
/// - A subpath with no segment is dropped.
pub(crate) fn normalize_path(path: &Path) -> Option<NormalizedPath> {
    fn flush(out: &mut Vec<Subpath>, cur: &mut Option<Subpath>) {
        if let Some(sub) = cur.take()
            && !sub.segments.is_empty()
        {
            out.push(sub);
        }
    }
    fn begin(p: Point) -> Subpath {
        Subpath {
            start: p,
            segments: Vec::new(),
            closed: false,
        }
    }

    let mut out = Vec::new();
    let mut cur: Option<Subpath> = None;
    for verb in path.verbs() {
        match *verb {
            PathVerb::MoveTo(p) => {
                flush(&mut out, &mut cur);
                if valid_point(p) {
                    cur = Some(begin(p));
                }
            }
            PathVerb::LineTo(p) => {
                if !valid_point(p) {
                    flush(&mut out, &mut cur);
                } else if let Some(sub) = cur.as_mut() {
                    sub.segments.push(Segment::Line(p));
                } else {
                    cur = Some(begin(p));
                }
            }
            PathVerb::QuadTo(c, p) => match cur.as_mut() {
                Some(sub) if valid_point(c) && valid_point(p) => {
                    sub.segments.push(Segment::Quad(c, p));
                }
                _ => flush(&mut out, &mut cur),
            },
            PathVerb::CubicTo(c1, c2, p) => match cur.as_mut() {
                Some(sub) if valid_point(c1) && valid_point(c2) && valid_point(p) => {
                    sub.segments.push(Segment::Cubic(c1, c2, p));
                }
                _ => flush(&mut out, &mut cur),
            },
            PathVerb::Close => {
                if let Some(mut sub) = cur.take() {
                    if !sub.segments.is_empty() {
                        sub.closed = true;
                        let start = sub.start;
                        out.push(sub);
                        cur = Some(begin(start));
                    } else {
                        cur = Some(sub);
                    }
                }
            }
        }
    }
    flush(&mut out, &mut cur);
    (!out.is_empty()).then_some(NormalizedPath { subpaths: out })
}

/// Validates a dash array. `None` means the stroke is solid.
///
/// The array is solid when it is empty, has a non-finite or negative entry, has a period (the
/// sum, with an odd-length array repeated once as SVG does) that is non-finite or below 0.1
/// pt, or would produce more than 100,000 dashes along `path_len_upper_bound` (counted as
/// `length / period * (entries / 2)`, like tiny-skia), or the path has zero length. Otherwise the
/// array is returned, doubled if its length was odd.
pub(crate) fn normalize_dash(dash: &[f32], path_len_upper_bound: f32) -> Option<Vec<f32>> {
    // A zero-length path has nothing to dash: it is stroked solid, so round and square caps
    // draw a dot or square, as SVG does.
    if path_len_upper_bound <= 0.0 {
        return None;
    }
    if dash.is_empty() || dash.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return None;
    }
    let mut pattern = dash.to_vec();
    if pattern.len() % 2 == 1 {
        pattern.extend_from_slice(dash);
    }
    let period: f32 = pattern.iter().sum();
    if !period.is_finite() || f64::from(period) < MIN_DASH_PERIOD {
        return None;
    }
    let dashes = f64::from(path_len_upper_bound) / f64::from(period) * (pattern.len() / 2) as f64;
    if dashes > MAX_DASH_COUNT {
        return None;
    }
    Some(pattern)
}

/// What a backend should paint for a linear gradient.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GradientKind {
    /// Paint nothing.
    Skip,
    /// Paint one solid color.
    Solid(Rgba),
    /// Paint a gradient with sanitized stops: offsets in `0..=1`, sorted, at least two.
    Linear {
        /// Where the gradient starts.
        start: Point,
        /// Where the gradient ends.
        end: Point,
        /// The sanitized stops.
        stops: Vec<GradientStop>,
    },
}

/// Classifies a gradient.
///
/// Stops with non-finite offsets are dropped, offsets are clamped to `0..=1` (`-0` becomes
/// `+0`) and stably sorted. No stops, or only fully transparent stops, skip the fill; one stop
/// gives that color; invalid endpoints skip; endpoints closer than 0.01 pt give the last stop
/// as a solid color.
pub(crate) fn normalize_gradient(g: &LinearGradient) -> GradientKind {
    let mut stops: Vec<GradientStop> = g
        .stops
        .iter()
        .filter(|s| s.offset.is_finite())
        .map(|s| GradientStop {
            offset: if s.offset <= 0.0 {
                0.0
            } else {
                s.offset.min(1.0)
            },
            color: s.color,
        })
        .collect();
    stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    let Some(last) = stops.last().map(|s| s.color) else {
        return GradientKind::Skip;
    };
    if stops.iter().all(|s| s.color.a == 0) {
        return GradientKind::Skip;
    }
    if stops.len() == 1 {
        return GradientKind::Solid(last);
    }
    if !valid_point(g.start) || !valid_point(g.end) {
        return GradientKind::Skip;
    }
    let length = f64::from(g.end.x - g.start.x).hypot(f64::from(g.end.y - g.start.y));
    if length < MIN_EXTENT {
        return GradientKind::Solid(last);
    }
    GradientKind::Linear {
        start: g.start,
        end: g.end,
        stops,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgba = Rgba {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    const BLUE: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };
    const CLEAR: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    fn p(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    fn stop(offset: f32, color: Rgba) -> GradientStop {
        GradientStop { offset, color }
    }

    fn gradient(stops: Vec<GradientStop>, start: Point, end: Point) -> LinearGradient {
        LinearGradient { start, end, stops }
    }

    fn lines(points: &[Point]) -> Path {
        let mut path = Path::new();
        for (i, pt) in points.iter().enumerate() {
            if i == 0 {
                path.move_to(*pt);
            } else {
                path.line_to(*pt);
            }
        }
        path
    }

    #[test]
    fn valid_points_are_finite_and_bounded() {
        assert!(valid_point(p(0.0, -MAX_COORD)));
        assert!(!valid_point(p(MAX_COORD * 1.01, 0.0)));
        assert!(!valid_point(p(0.0, f32::NAN)));
        assert!(!valid_point(p(f32::INFINITY, 0.0)));
        assert!(!valid_point(p(1.0e10, 1.0)));
    }

    #[test]
    fn huge_finite_control_points_invalidate_the_segment() {
        let mut path = Path::new();
        path.move_to(p(1.0, 1.0));
        path.quad_to(p(1.0e10, -1.0e10), p(30.0, 10.0));
        path.line_to(p(20.0, 15.0));
        // The invalid curve ends the subpath; the line then only starts a new empty one.
        assert!(normalize_path(&path).is_none());
    }

    #[test]
    fn invalid_segment_breaks_the_subpath() {
        let path = lines(&[
            p(2.0, 10.0),
            p(9.0, 10.0),
            p(f32::NAN, 10.0),
            p(30.0, 10.0),
            p(38.0, 10.0),
        ]);
        let n = normalize_path(&path).expect("drawable");
        assert_eq!(n.subpaths().len(), 2);
        assert_eq!(
            n.subpaths()[0].segments(),
            vec![Segment::Line(p(9.0, 10.0))]
        );
        assert_eq!(n.subpaths()[1].start(), p(30.0, 10.0));
        assert_eq!(
            n.subpaths()[1].segments(),
            vec![Segment::Line(p(38.0, 10.0))]
        );
    }

    #[test]
    fn curve_without_a_start_is_dropped() {
        let mut path = Path::new();
        path.quad_to(p(1.0, 1.0), p(2.0, 2.0));
        path.cubic_to(p(1.0, 1.0), p(2.0, 2.0), p(3.0, 3.0));
        assert!(normalize_path(&path).is_none());

        let mut path = Path::new();
        path.move_to(p(f32::NAN, 0.0));
        path.quad_to(p(1.0, 1.0), p(2.0, 2.0));
        path.line_to(p(5.0, 5.0));
        path.line_to(p(6.0, 6.0));
        let n = normalize_path(&path).expect("line survives");
        assert_eq!(n.subpaths().len(), 1);
        assert_eq!(n.subpaths()[0].start(), p(5.0, 5.0));
        assert_eq!(n.subpaths()[0].segments(), vec![Segment::Line(p(6.0, 6.0))]);
    }

    #[test]
    fn line_without_a_start_begins_a_subpath() {
        let mut path = Path::new();
        path.line_to(p(1.0, 2.0));
        path.line_to(p(3.0, 4.0));
        let n = normalize_path(&path).expect("drawable");
        assert_eq!(n.subpaths()[0].start(), p(1.0, 2.0));
        assert_eq!(n.subpaths()[0].segments(), vec![Segment::Line(p(3.0, 4.0))]);
    }

    #[test]
    fn close_needs_a_segment() {
        let mut path = Path::new();
        path.move_to(p(1.0, 1.0));
        path.close();
        assert!(normalize_path(&path).is_none());

        let mut path = Path::new();
        path.move_to(p(1.0, 1.0));
        path.line_to(p(5.0, 1.0));
        path.close();
        let n = normalize_path(&path).expect("drawable");
        assert!(n.subpaths()[0].is_closed());
    }

    #[test]
    fn segments_after_close_start_at_the_subpath_start() {
        let mut path = Path::new();
        path.move_to(p(1.0, 1.0));
        path.line_to(p(5.0, 1.0));
        path.close();
        path.line_to(p(9.0, 9.0));
        let n = normalize_path(&path).expect("drawable");
        assert_eq!(n.subpaths().len(), 2);
        assert!(n.subpaths()[0].is_closed());
        assert_eq!(n.subpaths()[1].start(), p(1.0, 1.0));
        assert!(!n.subpaths()[1].is_closed());
    }

    #[test]
    fn empty_and_undrawable_paths_are_none() {
        assert!(normalize_path(&Path::new()).is_none());
        assert!(normalize_path(&lines(&[p(1.0, 1.0)])).is_none());
        assert!(normalize_path(&lines(&[p(f32::NAN, 1.0), p(f32::NAN, 1.0)])).is_none());
    }

    #[test]
    fn control_length_follows_the_control_polygon() {
        let mut path = lines(&[p(0.0, 0.0), p(3.0, 4.0)]);
        path.close();
        let n = normalize_path(&path).expect("drawable");
        assert!((n.control_length() - 10.0).abs() < 1e-4);
    }

    #[test]
    fn dash_solid_cases() {
        assert_eq!(normalize_dash(&[], 100.0), None);
        assert_eq!(normalize_dash(&[0.0, 0.0], 100.0), None);
        assert_eq!(normalize_dash(&[-1.0, 2.0], 100.0), None);
        assert_eq!(normalize_dash(&[f32::NAN], 100.0), None);
        assert_eq!(normalize_dash(&[f32::INFINITY, 1.0], 100.0), None);
        // Period overflows f32.
        assert_eq!(normalize_dash(&[0.0, 2.0e38, 2.0e38, 0.0], 100.0), None);
        // Period below 0.1 pt, including values a 3-decimal writer would turn into "0".
        assert_eq!(normalize_dash(&[0.00049, 0.00049], 100.0), None);
        assert_eq!(normalize_dash(&[1.0e-20, 1.0e-20], 100.0), None);
    }

    #[test]
    fn zero_length_path_is_solid() {
        assert_eq!(normalize_dash(&[2.0, 2.0], 0.0), None);
        assert_eq!(normalize_dash(&[2.0, 2.0], 0.001), Some(vec![2.0, 2.0]));
    }

    #[test]
    fn dash_too_dense_is_solid() {
        assert_eq!(normalize_dash(&[1.0, 1.0], 200_000.0), Some(vec![1.0, 1.0]));
        assert_eq!(normalize_dash(&[1.0, 1.0], 200_002.0), None);
        assert_eq!(normalize_dash(&[1.0, 1.0], 100_000.0 * 2.0 + 1.0), None);
    }

    #[test]
    fn dash_period_boundary() {
        assert_eq!(normalize_dash(&[0.05, 0.05], 10.0), Some(vec![0.05, 0.05]));
        assert_eq!(normalize_dash(&[0.0499, 0.0500], 10.0), None);
        // Many tiny entries whose total period is still below the minimum.
        assert_eq!(normalize_dash(&vec![0.00004; 1000], 36.0), None);
    }

    #[test]
    fn dash_budget_counts_dashes_like_tiny_skia() {
        // Period 2.4, 24 entries: 230,000 / 2.4 * 12 is about 1.15 million dashes.
        assert_eq!(normalize_dash(&[0.1; 24], 230_000.0), None);
        // 12 entries of 1: period 12, 6 dashes per period; 100,000 dashes is the cap.
        let pattern = vec![1.0; 12];
        assert!(normalize_dash(&pattern, 200_000.0).is_some());
        assert_eq!(normalize_dash(&pattern, 200_002.0), None);
    }

    #[test]
    fn odd_dash_arrays_are_repeated() {
        assert_eq!(normalize_dash(&[3.0], 100.0), Some(vec![3.0, 3.0]));
        assert_eq!(
            normalize_dash(&[1.0, 2.0, 3.0], 100.0),
            Some(vec![1.0, 2.0, 3.0, 1.0, 2.0, 3.0])
        );
        assert_eq!(normalize_dash(&[4.0, 2.0], 100.0), Some(vec![4.0, 2.0]));
    }

    #[test]
    fn gradient_drops_non_finite_offsets() {
        let g = gradient(
            vec![stop(f32::NAN, RED), stop(1.0, BLUE)],
            p(0.0, 0.0),
            p(10.0, 0.0),
        );
        assert_eq!(normalize_gradient(&g), GradientKind::Solid(BLUE));
        let g = gradient(
            vec![stop(f32::NEG_INFINITY, RED), stop(f32::INFINITY, BLUE)],
            p(0.0, 0.0),
            p(10.0, 0.0),
        );
        assert_eq!(normalize_gradient(&g), GradientKind::Skip);
    }

    #[test]
    fn gradient_clamps_and_sorts_stably() {
        let g = gradient(
            vec![
                stop(2.0, BLUE),
                stop(-3.0, RED),
                stop(0.5, CLEAR),
                stop(0.5, RED),
            ],
            p(0.0, 0.0),
            p(10.0, 0.0),
        );
        match normalize_gradient(&g) {
            GradientKind::Linear { stops, .. } => {
                let offsets: Vec<f32> = stops.iter().map(|s| s.offset).collect();
                assert_eq!(offsets, vec![0.0, 0.5, 0.5, 1.0]);
                assert_eq!(stops[1].color, CLEAR);
                assert_eq!(stops[2].color, RED);
            }
            other => panic!("expected a gradient, got {other:?}"),
        }
    }

    #[test]
    fn gradient_negative_zero_keeps_input_order() {
        let g = gradient(
            vec![
                stop(0.0, RED),
                stop(-0.0, BLUE),
                stop(1.0, CLEAR),
                stop(1.0, RED),
            ],
            p(0.0, 0.0),
            p(10.0, 0.0),
        );
        match normalize_gradient(&g) {
            GradientKind::Linear { stops, .. } => {
                assert_eq!(stops[0].color, RED);
                assert_eq!(stops[1].color, BLUE);
                assert!(stops.iter().all(|s| s.offset.is_sign_positive()));
            }
            other => panic!("expected a gradient, got {other:?}"),
        }
    }

    #[test]
    fn gradient_stop_counts() {
        let ends = (p(0.0, 0.0), p(10.0, 0.0));
        assert_eq!(
            normalize_gradient(&gradient(vec![], ends.0, ends.1)),
            GradientKind::Skip
        );
        assert_eq!(
            normalize_gradient(&gradient(vec![stop(0.3, RED)], ends.0, ends.1)),
            GradientKind::Solid(RED)
        );
        assert_eq!(
            normalize_gradient(&gradient(
                vec![stop(0.0, CLEAR), stop(1.0, CLEAR)],
                ends.0,
                ends.1
            )),
            GradientKind::Skip
        );
    }

    #[test]
    fn gradient_endpoints() {
        let stops = vec![stop(0.0, RED), stop(1.0, BLUE)];
        let skip = gradient(stops.clone(), p(f32::NAN, 0.0), p(1.0, 0.0));
        assert_eq!(normalize_gradient(&skip), GradientKind::Skip);
        let skip = gradient(stops.clone(), p(-3.0e38, 0.0), p(3.0e38, 0.0));
        assert_eq!(normalize_gradient(&skip), GradientKind::Skip);
        let same = gradient(stops.clone(), p(3.0, 3.0), p(3.0, 3.0));
        assert_eq!(normalize_gradient(&same), GradientKind::Solid(BLUE));
        let near = gradient(stops.clone(), p(16.000498, 0.0), p(16.00051, 0.0));
        assert_eq!(normalize_gradient(&near), GradientKind::Solid(BLUE));
        let apart = gradient(stops, p(16.0, 0.0), p(16.02, 0.0));
        assert!(matches!(
            normalize_gradient(&apart),
            GradientKind::Linear { .. }
        ));
    }

    /// Hostile input rendered through both backends: the shared rules must classify it the same
    /// way in SVG and PNG.
    mod backends_agree {
        use super::*;
        use crate::render::{SvgText, render_png, render_svg};
        use crate::scene::{FillRule, Item, LineCap, LineJoin, Paint, Scene};
        use crate::test_support::image::{DecodedPng, decode_rgba8};

        const WHITE: Rgba = Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        const BLACK: Rgba = Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };

        fn render(scene: &Scene) -> (String, DecodedPng) {
            let svg = render_svg(scene, SvgText::Outlines);
            let png = decode_rgba8(&render_png(scene, 72).expect("renders")).expect("decodes");
            (svg, png)
        }

        fn has_ink(image: &DecodedPng) -> bool {
            (0..image.height).any(|y| (0..image.width).any(|x| image.at(x, y)[0] < 128))
        }

        fn style(color: Rgba, width: f32, dash: Vec<f32>) -> StrokeStyle {
            StrokeStyle {
                color,
                width,
                dash,
                cap: LineCap::Butt,
                join: LineJoin::Miter,
            }
        }

        fn horizontal() -> Path {
            lines(&[p(0.0, 10.0), p(40.0, 10.0)])
        }

        fn stroke_scene(style: StrokeStyle) -> Scene {
            let mut scene = Scene::new(40.0, 20.0, WHITE);
            scene.push(Item::Stroke {
                path: horizontal(),
                style,
            });
            scene
        }

        #[test]
        fn stroke_visibility_is_classified_alike() {
            let transparent = Rgba { a: 0, ..BLACK };
            let cases = [
                (BLACK, 2.0, true),
                (BLACK, 0.0, false),
                (BLACK, -1.0, false),
                (BLACK, f32::NAN, false),
                (BLACK, f32::INFINITY, false),
                (BLACK, f32::NEG_INFINITY, false),
                (transparent, 2.0, false),
            ];
            for (color, width, visible) in cases {
                let stroke = style(color, width, Vec::new());
                assert_eq!(stroke_is_visible(&stroke), visible, "width {width}");
                let (svg, png) = render(&stroke_scene(stroke));
                assert_eq!(svg.contains("stroke-width"), visible, "svg, width {width}");
                assert_eq!(has_ink(&png), visible, "png, width {width}");
            }
        }

        #[test]
        fn clip_validity_is_classified_alike() {
            let clip = |x: f32, y: f32, width: f32, height: f32| Rect {
                x,
                y,
                width,
                height,
            };
            let cases = [
                (clip(0.0, 0.0, 40.0, 20.0), true),
                (clip(-5.0, -5.0, 100.0, 100.0), true),
                (clip(0.0, 0.0, 0.0, 20.0), false),
                (clip(0.0, 0.0, 40.0, 0.0), false),
                (clip(0.0, 0.0, -40.0, 20.0), false),
                (clip(0.0, 0.0, 40.0, -20.0), false),
                (clip(f32::NAN, 0.0, 40.0, 20.0), false),
                (clip(0.0, f32::INFINITY, 40.0, 20.0), false),
                (clip(0.0, 0.0, f32::NAN, 20.0), false),
                (clip(0.0, 0.0, 40.0, f32::INFINITY), false),
            ];
            for (rect, valid) in cases {
                assert_eq!(clip_is_valid(&rect), valid, "{rect:?}");
                let mut scene = Scene::new(40.0, 20.0, WHITE);
                scene.push(Item::Group {
                    clip: Some(rect),
                    items: vec![Item::Fill {
                        path: lines(&[p(0.0, 0.0), p(40.0, 0.0), p(40.0, 20.0), p(0.0, 20.0)]),
                        paint: Paint::Solid(BLACK),
                        rule: FillRule::NonZero,
                    }],
                });
                let (svg, png) = render(&scene);
                assert_eq!(svg.contains("<path"), valid, "svg, {rect:?}");
                assert_eq!(has_ink(&png), valid, "png, {rect:?}");
            }
        }

        #[test]
        fn dash_degeneration_is_classified_alike() {
            let patterns: [(&[f32], bool); 9] = [
                (&[4.0, 4.0], true),
                (&[4.0], true),
                (&[], false),
                (&[0.0, 0.0], false),
                (&[-1.0, 1.0], false),
                (&[f32::NAN, 1.0], false),
                (&[f32::INFINITY, 1.0], false),
                (&[0.02, 0.02], false),
                (&[1.0e-20, 1.0e-20], false),
            ];
            for (dash, dashed) in patterns {
                let stroke = style(BLACK, 4.0, dash.to_vec());
                assert_eq!(
                    normalize_dash(dash, 40.0).is_some(),
                    dashed,
                    "dash {dash:?}"
                );
                let (svg, png) = render(&stroke_scene(stroke));
                assert_eq!(svg.contains("stroke-dasharray"), dashed, "svg, {dash:?}");
                // Row 9 lies inside the 4 pt wide stroke: a gap shows up only when dashed.
                let solid = (1..39).all(|x| png.at(x, 9)[0] < 128);
                assert_eq!(!solid, dashed, "png, {dash:?}");
            }
        }

        #[test]
        fn text_visibility_is_classified_alike() {
            use crate::fonts::{Font, Weight};
            use crate::text::{FontLibrary, TextStyle};

            let style = TextStyle {
                font: Font::Inter,
                weight: Weight::Regular,
                size: 16.0,
                tabular_figures: false,
            };
            let transparent = Rgba { a: 0, ..BLACK };
            let cases = [
                (BLACK, p(4.0, 16.0), true),
                (transparent, p(4.0, 16.0), false),
                (BLACK, p(f32::NAN, 16.0), false),
                (BLACK, p(4.0, f32::NAN), false),
                (BLACK, p(f32::INFINITY, 16.0), false),
                (BLACK, p(4.0, f32::NEG_INFINITY), false),
                (BLACK, p(2.0 * MAX_COORD, 16.0), false),
            ];
            for (color, origin, visible) in cases {
                assert_eq!(text_is_visible(color, origin), visible, "{origin:?}");
                let mut scene = Scene::new(40.0, 20.0, WHITE);
                scene.push_text(FontLibrary::global(), "Hi", style, origin, color);
                for mode in [SvgText::Editable, SvgText::Outlines] {
                    let svg = render_svg(&scene, mode);
                    let drawn = svg.contains("<text") || svg.contains("<path");
                    assert_eq!(drawn, visible, "svg {mode:?}, {origin:?}");
                }
                let png = decode_rgba8(&render_png(&scene, 72).expect("renders")).expect("decodes");
                assert_eq!(has_ink(&png), visible, "png, {origin:?}");
            }
        }

        #[test]
        fn alpha_zero_paints_draw_nothing_in_both() {
            let clear = Rgba { a: 0, ..BLACK };
            let mut scene = Scene::new(40.0, 20.0, WHITE);
            scene.push(Item::Fill {
                path: lines(&[p(0.0, 0.0), p(40.0, 0.0), p(40.0, 20.0), p(0.0, 20.0)]),
                paint: Paint::Solid(clear),
                rule: FillRule::NonZero,
            });
            scene.push(Item::Fill {
                path: lines(&[p(0.0, 0.0), p(40.0, 0.0), p(40.0, 20.0), p(0.0, 20.0)]),
                paint: Paint::Linear(gradient(
                    vec![stop(0.0, clear), stop(1.0, clear)],
                    p(0.0, 0.0),
                    p(40.0, 0.0),
                )),
                rule: FillRule::NonZero,
            });
            let (svg, png) = render(&scene);
            assert!(!svg.contains("<path"), "{svg}");
            assert!(!has_ink(&png));
        }
    }
}
