//! Drawing: turns a [`Layout`] into scene items.
//!
//! The axes chrome (gridlines, the baseline, tick labels, the header and the axis labels) is
//! one unclipped group. The marks follow in a second group, clipped to the plot area, drawn in
//! insertion order.
//!
//! Marks are mapped to plot coordinates in `f64` and clipped there, before anything is
//! narrowed to `f32`: lines against the plot area expanded by `max(line_width, 2)` pt
//! (Liang–Barsky), scatter points against the plot area expanded by the marker radius plus
//! 1 pt. Every mark coordinate in the scene is therefore within a few points of the plot area.

use crate::figure::{Axes, AxisData, Mark};
use crate::geometry::{Path, Point};
use crate::layout::{Area, Label, Layout};
use crate::m4_indices;
use crate::resolve::{AxisScale, Resolved, ResolvedAxis};
use crate::scene::{
    FillRule, GradientStop, Item, LineCap, LineJoin, LinearGradient, Paint, Rect, StrokeStyle,
    TextItem,
};
use crate::{AreaFill, Dash, GridLines, Marker, Rgba, Theme};

/// The widest stroke drawn, whatever the theme says.
const MAX_STROKE_WIDTH: f32 = 1.0e4;
/// The largest marker diameter drawn, whatever the theme says.
const MAX_MARKER_SIZE: f32 = 1.0e4;
/// Mapped data coordinates are clamped to this magnitude before clipping. Only data far
/// outside very narrow limits gets near it; clamping keeps every intermediate of the clip
/// finite.
const MAX_PLOT_COORD: f64 = 1.0e150;
/// The smallest line clip margin, in points.
const MIN_LINE_MARGIN: f64 = 2.0;
/// The nominal device resolution that sets the downsampling column count (spec section 7).
const DOWNSAMPLE_DPI: f64 = 300.0;
/// The area gradient's opacity at the top of the line, as a share of the line's own opacity.
const AREA_TOP_ALPHA: f32 = 0.32;
/// Gradient vectors shorter than this give no area fill (the area is empty).
const MIN_AREA_HEIGHT: f64 = 0.01;
/// The scatter outline width, in points.
const MARKER_OUTLINE: f32 = 0.8;
/// Scatter points further than the marker radius plus this many points outside the plot area
/// are dropped.
const MARKER_SLACK: f64 = 1.0;
/// The side of a square with the area of a unit-diameter circle: `sqrt(pi) / 2`.
const SQUARE_SIDE: f32 = 0.886;
/// The cubic Bézier handle length for a quarter circle of unit radius.
const KAPPA: f32 = 0.552_284_8;

/// A scene point from layout coordinates. Layout keeps every value finite and far below the
/// renderers' `MAX_COORD`, so narrowing to `f32` is safe.
fn point(x: f64, y: f64) -> Point {
    Point {
        x: x as f32,
        y: y as f32,
    }
}

/// A stroke width that is finite and within `0..=MAX_STROKE_WIDTH`.
fn stroke_width(width: f32) -> f32 {
    if width.is_finite() && width > 0.0 {
        width.min(MAX_STROKE_WIDTH)
    } else {
        0.0
    }
}

fn segment(from: Point, to: Point) -> Path {
    let mut path = Path::new();
    path.move_to(from);
    path.line_to(to);
    path
}

/// The gridline stroke: `Dotted` is round dots every 3.2 pt, `Dashed` 2 on / 4 off.
fn grid_style(theme: &Theme) -> StrokeStyle {
    let (dash, cap) = match theme.grid_dash() {
        Dash::Solid => (Vec::new(), LineCap::Butt),
        Dash::Dashed => (vec![2.0, 4.0], LineCap::Butt),
        Dash::Dotted => (vec![0.0, 3.2], LineCap::Round),
    };
    StrokeStyle {
        color: theme.grid_color(),
        width: stroke_width(theme.grid_width()),
        dash,
        cap,
        join: LineJoin::Miter,
    }
}

fn text(label: &Label, color: Rgba) -> Item {
    let origin = point(label.x, label.y);
    Item::Text(TextItem {
        text: label.text.clone(),
        style: label.style,
        origin,
        color,
        outline: label.layout.outline(origin),
        width: label.layout.width(),
    })
}

/// The axes chrome as one unclipped group: gridlines, baseline, tick labels, header and axis
/// labels, in that order.
pub(crate) fn chrome(theme: &Theme, layout: &Layout) -> Item {
    let plot = layout.plot;
    let mut items = Vec::new();

    let grid = grid_style(theme);
    let lines = theme.grid_lines();
    if matches!(lines, GridLines::Horizontal | GridLines::Both) {
        for &y in &layout.y_grid {
            items.push(Item::Stroke {
                path: segment(point(plot.x, y), point(plot.right(), y)),
                style: grid.clone(),
            });
        }
    }
    if matches!(lines, GridLines::Vertical | GridLines::Both) {
        for &x in &layout.x_grid {
            items.push(Item::Stroke {
                path: segment(point(x, plot.y), point(x, plot.bottom())),
                style: grid.clone(),
            });
        }
    }

    items.push(Item::Stroke {
        path: segment(
            point(plot.x, layout.baseline_y),
            point(plot.right(), layout.baseline_y),
        ),
        style: StrokeStyle {
            color: theme.baseline_color(),
            width: stroke_width(theme.baseline_width()),
            dash: Vec::new(),
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        },
    });

    let muted = theme.muted_color();
    for label in layout.y_tick_labels.iter().chain(&layout.x_tick_labels) {
        items.push(text(label, muted));
    }
    if let Some(label) = &layout.title {
        items.push(text(label, theme.text_color()));
    }
    if let Some(label) = &layout.subtitle {
        items.push(text(label, muted));
    }
    for label in layout.ylabel.iter().chain(&layout.xlabel) {
        items.push(text(label, theme.text_color()));
    }

    Item::Group { clip: None, items }
}

/// An axis-aligned rectangle in `f64` points, for clipping.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl Bounds {
    /// `area` expanded by `margin` on every side.
    fn around(area: Area, margin: f64) -> Bounds {
        Bounds {
            left: area.x - margin,
            top: area.y - margin,
            right: area.right() + margin,
            bottom: area.bottom() + margin,
        }
    }

    fn contains(&self, (x, y): (f64, f64)) -> bool {
        (self.left..=self.right).contains(&x) && (self.top..=self.bottom).contains(&y)
    }
}

/// A mapped coordinate: NaN stays NaN (a gap), everything else is clamped to
/// [`MAX_PLOT_COORD`], infinities included.
fn plot_coord(v: f64) -> f64 {
    if v.is_nan() {
        v
    } else {
        v.clamp(-MAX_PLOT_COORD, MAX_PLOT_COORD)
    }
}

/// The plot x coordinates of `x`. Category names map to their band center; names the scale
/// does not know (impossible after resolve) and NaN map to NaN.
fn map_x(axis: &ResolvedAxis, plot: Area, x: &AxisData) -> Vec<f64> {
    match (&axis.scale, x) {
        (AxisScale::Continuous(scale), AxisData::Numbers(values)) => values
            .iter()
            .map(|&v| plot_coord(plot.x_at(scale.normalize(v))))
            .collect(),
        (AxisScale::Category(scale), AxisData::Categories(names)) => names
            .iter()
            .map(|name| {
                scale
                    .index_of(name)
                    .map_or(f64::NAN, |i| plot_coord(plot.x_at(scale.center(i))))
            })
            .collect(),
        // The model rejects data of the wrong kind for the axis.
        (_, data) => vec![f64::NAN; data_len(data)],
    }
}

fn data_len(data: &AxisData) -> usize {
    match data {
        AxisData::Numbers(v) => v.len(),
        AxisData::Categories(v) => v.len(),
    }
}

/// The plot y coordinates of `y`; NaN maps to NaN.
fn map_y(axis: &ResolvedAxis, plot: Area, y: &[f64]) -> Vec<f64> {
    match &axis.scale {
        AxisScale::Continuous(scale) => y
            .iter()
            .map(|&v| plot_coord(plot.y_at(scale.normalize(v))))
            .collect(),
        // Marks hold numeric y, which a category y axis cannot place.
        AxisScale::Category(_) => vec![f64::NAN; y.len()],
    }
}

/// Liang–Barsky: the parameter range `(t0, t1)` of the segment `a + t (b - a)`, `t` in
/// `0..=1`, that lies inside `bounds`, or `None` when no part does.
fn clip_segment(a: (f64, f64), b: (f64, f64), bounds: &Bounds) -> Option<(f64, f64)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, a.0 - bounds.left),
        (dx, bounds.right - a.0),
        (-dy, a.1 - bounds.top),
        (dy, bounds.bottom - a.1),
    ] {
        if p == 0.0 {
            // Parallel to this edge: inside or out for the whole segment.
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            if t > t1 {
                return None;
            }
            t0 = t0.max(t);
        } else {
            if t < t0 {
                return None;
            }
            t1 = t1.min(t);
        }
    }
    (t0 <= t1).then_some((t0, t1))
}

/// The point at `t` along `a..b`, exact at both ends.
fn lerp(a: (f64, f64), b: (f64, f64), t: f64) -> (f64, f64) {
    if t <= 0.0 {
        a
    } else if t >= 1.0 {
        b
    } else {
        (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
    }
}

/// Splits the polyline through `(xs[i], ys[i])` at NaN and clips it to `bounds`. Each returned
/// subpath has at least one point; a single point is an isolated data point inside `bounds`.
/// A segment that leaves `bounds` ends its subpath, and the part that re-enters starts a new
/// one.
fn clip_polyline(xs: &[f64], ys: &[f64], bounds: &Bounds) -> Vec<Vec<(f64, f64)>> {
    let points: Vec<(f64, f64)> = xs.iter().copied().zip(ys.iter().copied()).collect();
    let mut out = Vec::new();
    for run in points.split(|p| p.0.is_nan() || p.1.is_nan()) {
        match run {
            [] => {}
            [p] => {
                if bounds.contains(*p) {
                    out.push(vec![*p]);
                }
            }
            _ => clip_run(run, bounds, &mut out),
        }
    }
    out
}

/// Clips one NaN-free run of at least two points, appending its visible subpaths to `out`.
fn clip_run(run: &[(f64, f64)], bounds: &Bounds, out: &mut Vec<Vec<(f64, f64)>>) {
    let mut current: Vec<(f64, f64)> = Vec::new();
    for pair in run.windows(2) {
        let [a, b] = pair else { continue };
        let (a, b) = (*a, *b);
        match clip_segment(a, b, bounds) {
            None => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            Some((t0, t1)) => {
                if current.is_empty() || t0 > 0.0 {
                    if !current.is_empty() {
                        out.push(std::mem::take(&mut current));
                    }
                    current.push(lerp(a, b, t0));
                }
                current.push(lerp(a, b, t1));
                if t1 < 1.0 {
                    out.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
}

/// The number of device columns the downsampler works with for a plot `width` pt wide.
fn downsample_columns(width: f64) -> usize {
    let columns = (width / 72.0 * DOWNSAMPLE_DPI).ceil();
    if columns.is_finite() && columns >= 1.0 {
        // Figures are at most 200 in wide, so this is far below `usize::MAX`.
        columns as usize
    } else {
        1
    }
}

/// The stroke of a line mark of `color` and `dash` at the theme's line width.
fn line_style(theme: &Theme, color: Rgba, dash: Dash) -> StrokeStyle {
    let w = stroke_width(theme.line_width());
    let (dash, cap) = match dash {
        Dash::Solid => (Vec::new(), LineCap::Round),
        Dash::Dashed => (vec![4.0 * w, 3.0 * w], LineCap::Butt),
        Dash::Dotted => (vec![0.0, 2.5 * w], LineCap::Round),
    };
    StrokeStyle {
        color,
        width: w,
        dash,
        cap,
        join: LineJoin::Round,
    }
}

fn scene_point((x, y): (f64, f64)) -> Point {
    point(x, y)
}

/// The scene items of one line: the area fill (with `AreaFill::Gradient`), then the stroke.
fn line_items(
    theme: &Theme,
    layout: &Layout,
    xs: &[f64],
    ys: &[f64],
    color: Rgba,
    dash: Dash,
    items: &mut Vec<Item>,
) {
    let plot = layout.plot;
    let margin = f64::from(stroke_width(theme.line_width())).max(MIN_LINE_MARGIN);
    let subpaths = clip_polyline(xs, ys, &Bounds::around(plot, margin));
    if subpaths.is_empty() {
        return;
    }

    if theme.area_fill() == AreaFill::Gradient
        && let Some(fill) = area_fill(&subpaths, layout.baseline_y, color)
    {
        items.push(fill);
    }

    let mut path = Path::new();
    for subpath in &subpaths {
        let mut points = subpath.iter().copied().map(scene_point);
        let Some(first) = points.next() else {
            continue;
        };
        path.move_to(first);
        match points.next() {
            // An isolated point: a zero-length subpath, which the round cap draws as a dot.
            None => path.line_to(first),
            Some(second) => {
                path.line_to(second);
                points.for_each(|p| path.line_to(p));
            }
        }
    }
    items.push(Item::Stroke {
        path,
        style: line_style(theme, color, dash),
    });
}

/// The area under the clipped line `subpaths`, closed down to `baseline_y`, painted with a
/// vertical gradient from the line's extreme y (the top of its bounding box, or the bottom for
/// a line under the baseline) at 0.32 of the line's opacity to the baseline at 0.
fn area_fill(subpaths: &[Vec<(f64, f64)>], baseline_y: f64, color: Rgba) -> Option<Item> {
    let mut path = Path::new();
    let (mut top, mut bottom) = (f64::INFINITY, f64::NEG_INFINITY);
    for subpath in subpaths {
        let (Some(&first), Some(&last)) = (subpath.first(), subpath.last()) else {
            continue;
        };
        if subpath.len() < 2 {
            continue;
        }
        path.move_to(scene_point(first));
        for &p in subpath.iter().skip(1) {
            path.line_to(scene_point(p));
            top = top.min(p.1);
            bottom = bottom.max(p.1);
        }
        top = top.min(first.1);
        bottom = bottom.max(first.1);
        path.line_to(point(last.0, baseline_y));
        path.line_to(point(first.0, baseline_y));
        path.close();
    }
    if path.is_empty() {
        return None;
    }
    let start = if top < baseline_y { top } else { bottom };
    if (start - baseline_y).abs() < MIN_AREA_HEIGHT {
        return None;
    }
    let top_alpha = (f32::from(color.a) * AREA_TOP_ALPHA)
        .round()
        .clamp(0.0, 255.0) as u8;
    Some(Item::Fill {
        path,
        paint: Paint::Linear(LinearGradient {
            start: point(0.0, start),
            end: point(0.0, baseline_y),
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: Rgba {
                        a: top_alpha,
                        ..color
                    },
                },
                GradientStop {
                    offset: 1.0,
                    color: Rgba { a: 0, ..color },
                },
            ],
        }),
        rule: FillRule::NonZero,
    })
}

/// A closed polygon through `points`.
fn polygon(points: &[(f32, f32)]) -> Path {
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
fn marker_shape(shape: Marker, diameter: f32) -> Path {
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

/// The marker diameter: the theme's, finite and within `0..=MAX_MARKER_SIZE`.
fn marker_size(theme: &Theme) -> f32 {
    let d = theme.marker_size();
    if d.is_finite() && d > 0.0 {
        d.min(MAX_MARKER_SIZE)
    } else {
        0.0
    }
}

/// The scene item of one scatter: one batch of `shape` markers, filled with `color` at the
/// theme's marker alpha times the color's own, outlined 0.8 pt in the background color.
fn scatter_item(
    theme: &Theme,
    plot: Area,
    xs: &[f64],
    ys: &[f64],
    color: Rgba,
    shape: Marker,
) -> Option<Item> {
    let d = marker_size(theme);
    let bounds = Bounds::around(plot, f64::from(d) / 2.0 + MARKER_SLACK);
    let positions: Vec<Point> = xs
        .iter()
        .zip(ys)
        .map(|(&x, &y)| (x, y))
        .filter(|&p| bounds.contains(p))
        .map(scene_point)
        .collect();
    if positions.is_empty() {
        return None;
    }
    let alpha = theme.marker_alpha();
    let alpha = if alpha.is_finite() {
        alpha.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let fill_alpha = (f32::from(color.a) * alpha).round().clamp(0.0, 255.0) as u8;
    Some(Item::Markers {
        shape: marker_shape(shape, d),
        fill: Some(Rgba {
            a: fill_alpha,
            ..color
        }),
        stroke: Some(StrokeStyle {
            color: theme.background(),
            width: MARKER_OUTLINE,
            dash: Vec::new(),
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }),
        positions,
    })
}

/// The marks of `axes` as one group clipped to the plot area, in insertion order, each in its
/// color from `colors` (as from `Figure::resolved_colors`). `None` when nothing is visible.
pub(crate) fn marks(
    theme: &Theme,
    axes: &Axes,
    resolved: &Resolved,
    layout: &Layout,
    colors: &[Rgba],
) -> Option<Item> {
    let plot = layout.plot;
    let mut items = Vec::new();
    for (mark, &color) in axes.marks().iter().zip(colors) {
        match mark {
            Mark::Line { x, y, options } => {
                let mut xs = map_x(&resolved.x, plot, x);
                let mut ys = map_y(&resolved.y, plot, y);
                if !resolved.x.is_category() {
                    let columns = downsample_columns(plot.width);
                    let kept = m4_indices(&xs, &ys, (plot.x, plot.right()), columns);
                    if kept.len() < xs.len() {
                        let pick = |v: &[f64]| -> Vec<f64> {
                            kept.iter().filter_map(|&i| v.get(i).copied()).collect()
                        };
                        (xs, ys) = (pick(&xs), pick(&ys));
                    }
                }
                line_items(theme, layout, &xs, &ys, color, options.dash, &mut items);
            }
            Mark::Scatter { x, y, options } => {
                let xs = map_x(&resolved.x, plot, x);
                let ys = map_y(&resolved.y, plot, y);
                items.extend(scatter_item(theme, plot, &xs, &ys, color, options.marker));
            }
        }
    }
    if items.is_empty() {
        return None;
    }
    Some(Item::Group {
        clip: Some(Rect {
            x: plot.x as f32,
            y: plot.y as f32,
            width: plot.width as f32,
            height: plot.height as f32,
        }),
        items,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::geometry::PathVerb;
    use crate::render::{SvgText, render_svg};
    use crate::scene::Scene;
    use crate::{Figure, FontLibrary, ScaleKind};

    /// The renderers' coordinate limit, restated so the tests do not depend on `normalize`.
    const MAX_COORD: f32 = 1.0e6;

    fn check(p: Point) {
        assert!(
            p.x.is_finite() && p.y.is_finite() && p.x.abs() <= MAX_COORD && p.y.abs() <= MAX_COORD,
            "{p:?}"
        );
    }

    fn check_path(path: &Path) {
        for verb in path.verbs() {
            match *verb {
                PathVerb::MoveTo(p) | PathVerb::LineTo(p) => check(p),
                PathVerb::QuadTo(c, p) => {
                    check(c);
                    check(p);
                }
                PathVerb::CubicTo(c1, c2, p) => {
                    check(c1);
                    check(c2);
                    check(p);
                }
                PathVerb::Close => {}
            }
        }
    }

    fn check_items(items: &[Item]) {
        for item in items {
            match item {
                Item::Fill { path, .. } => check_path(path),
                Item::Stroke { path, style } => {
                    check_path(path);
                    assert!(style.width.is_finite());
                }
                Item::Markers {
                    shape, positions, ..
                } => {
                    check_path(shape);
                    positions.iter().copied().for_each(check);
                }
                Item::Text(t) => {
                    check(t.origin);
                    check_path(&t.outline);
                    assert!(t.width.is_finite());
                }
                Item::Group { clip, items } => {
                    if let Some(r) = clip {
                        check(Point { x: r.x, y: r.y });
                        check(Point {
                            x: r.x + r.width,
                            y: r.y + r.height,
                        });
                    }
                    check_items(items);
                }
            }
        }
    }

    /// Asserts every coordinate of `scene` is finite and within `MAX_COORD`.
    pub(crate) fn assert_scene_bounded(scene: &Scene) {
        assert!(scene.width.is_finite() && scene.height.is_finite());
        check_items(&scene.items);
    }

    fn chrome_items(scene: &Scene) -> &[Item] {
        match scene.items.as_slice() {
            [Item::Group { clip: None, items }] | [Item::Group { clip: None, items }, _] => items,
            other => panic!("expected one unclipped group, got {other:?}"),
        }
    }

    fn strokes(scene: &Scene) -> Vec<(&Path, &StrokeStyle)> {
        chrome_items(scene)
            .iter()
            .filter_map(|item| match item {
                Item::Stroke { path, style } => Some((path, style)),
                _ => None,
            })
            .collect()
    }

    fn texts(scene: &Scene) -> Vec<&TextItem> {
        chrome_items(scene)
            .iter()
            .filter_map(|item| match item {
                Item::Text(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    fn chrome_figure(theme: Theme) -> Figure {
        let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
        figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .title("Station output")
            .subtitle("Daily mean, MW")
            .xlabel("Day")
            .ylabel("MW")
            .xlim(0.0, 100.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap();
        figure
    }

    #[test]
    fn chrome_snapshot() {
        let scene = chrome_figure(Theme::pigment()).to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        insta::assert_snapshot!(render_svg(&scene, SvgText::Editable));
    }

    #[test]
    fn figure_without_axes_is_background_only() {
        let figure = Figure::new(2.0, 1.0, Theme::midnight()).unwrap();
        let scene = figure.to_scene(FontLibrary::global());
        assert!(scene.items.is_empty());
        assert_eq!((scene.width, scene.height), (144.0, 72.0));
        assert_eq!(scene.background, Theme::midnight().background());
    }

    #[test]
    fn grid_dashes_follow_the_theme() {
        let cases = [
            (Theme::pigment(), vec![0.0, 3.2], LineCap::Round),
            (Theme::graphite(), vec![], LineCap::Butt),
            (Theme::midnight(), vec![2.0, 4.0], LineCap::Butt),
        ];
        for (theme, dash, cap) in cases {
            let scene = chrome_figure(theme.clone()).to_scene(FontLibrary::global());
            let all = strokes(&scene);
            let (grid, baseline) = all.split_at(all.len() - 1);
            assert!(!grid.is_empty());
            for (path, style) in grid {
                assert_eq!((&style.dash, style.cap), (&dash, cap));
                assert_eq!((style.color, style.width), (theme.grid_color(), 0.75));
                // Horizontal lines only.
                let ys: Vec<f32> = path
                    .verbs()
                    .iter()
                    .filter_map(|v| match v {
                        PathVerb::MoveTo(p) | PathVerb::LineTo(p) => Some(p.y),
                        _ => None,
                    })
                    .collect();
                assert!(ys.windows(2).all(|w| w[0] == w[1]));
            }
            let (_, style) = baseline[0];
            assert_eq!(style.color, theme.baseline_color());
            assert_eq!(style.width, theme.baseline_width());
            assert!(style.dash.is_empty());
        }
    }

    #[test]
    fn baseline_is_at_zero_or_bottom() {
        let baseline_y = |lo: f64, hi: f64| {
            let mut f = chrome_figure(Theme::graphite());
            f.axes_mut().unwrap().ylim(lo, hi).unwrap();
            let scene = f.to_scene(FontLibrary::global());
            let all = strokes(&scene);
            let (path, _) = all.last().copied().unwrap();
            let Some(PathVerb::MoveTo(p)) = path.verbs().first() else {
                panic!("baseline must start with a move")
            };
            let gridlines: Vec<f32> = all[..all.len() - 1]
                .iter()
                .filter_map(|(path, _)| match path.verbs().first() {
                    Some(PathVerb::MoveTo(p)) => Some(p.y),
                    _ => None,
                })
                .collect();
            (p.y, gridlines)
        };
        let (zero, grid) = baseline_y(-50.0, 50.0);
        assert!(grid.iter().any(|&y| y > zero) && grid.iter().any(|&y| y < zero));
        assert!(grid.iter().all(|&y| (y - zero).abs() > 0.01));
        let (bottom, grid) = baseline_y(10.0, 50.0);
        assert!(grid.iter().all(|&y| y < bottom));
    }

    #[test]
    fn vertical_grid_and_category_axes() {
        let theme = Theme::graphite().with_grid_lines(GridLines::Both).unwrap();
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xlim(0.0, 10.0)
            .unwrap();
        let scene = f.to_scene(FontLibrary::global());
        let vertical = strokes(&scene)
            .iter()
            .filter(|(path, _)| match path.verbs() {
                [PathVerb::MoveTo(a), PathVerb::LineTo(b)] => a.x == b.x,
                _ => false,
            })
            .count();
        assert!(vertical >= 3);

        // A category x axis: one label per category, centered in its band, no vertical grid.
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .line(
                crate::AxisData::Categories(vec!["Q1".into(), "Q2".into(), "Q3".into()]),
                &[1.0, 2.0, 3.0],
                crate::LineOptions::default(),
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global());
        let vertical = strokes(&scene)
            .iter()
            .filter(|(path, _)| match path.verbs() {
                [PathVerb::MoveTo(a), PathVerb::LineTo(b)] => a.x == b.x,
                _ => false,
            })
            .count();
        assert_eq!(vertical, 0);
        let all = texts(&scene);
        let q: Vec<&&TextItem> = all.iter().filter(|t| t.text.starts_with('Q')).collect();
        assert_eq!(q.len(), 3);
        let centers: Vec<f32> = q.iter().map(|t| t.origin.x + t.width / 2.0).collect();
        let (gap1, gap2) = (centers[1] - centers[0], centers[2] - centers[1]);
        assert!((gap1 - gap2).abs() < 1e-3, "{centers:?}");
    }

    #[test]
    fn text_colors_follow_roles() {
        let theme = Theme::midnight();
        let scene = chrome_figure(theme.clone()).to_scene(FontLibrary::global());
        for t in texts(&scene) {
            let expected = match t.text.as_str() {
                "Station output" | "Day" | "MW" => theme.text_color(),
                _ => theme.muted_color(),
            };
            assert_eq!(t.color, expected, "{}", t.text);
            assert_eq!(
                t.style.tabular_figures,
                t.text.chars().all(|c| !c.is_alphabetic())
            );
        }
    }

    /// Asserts every glyph-outline point of every text item lies on the canvas.
    #[track_caller]
    fn assert_text_on_canvas(scene: &Scene) {
        let texts = texts(scene);
        assert!(!texts.is_empty());
        for t in texts {
            for verb in t.outline.verbs() {
                let points: Vec<Point> = match *verb {
                    PathVerb::MoveTo(p) | PathVerb::LineTo(p) => vec![p],
                    PathVerb::QuadTo(c, p) => vec![c, p],
                    PathVerb::CubicTo(c1, c2, p) => vec![c1, c2, p],
                    PathVerb::Close => vec![],
                };
                for p in points {
                    assert!(
                        (0.0..=scene.width).contains(&p.x) && (0.0..=scene.height).contains(&p.y),
                        "{:?} at {p:?} leaves the {}x{} canvas",
                        t.text,
                        scene.width,
                        scene.height
                    );
                }
            }
        }
    }

    #[test]
    fn x_end_labels_stay_on_the_canvas() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            for (lo, hi) in [(0.0, 100_000.0), (-100_000.0, 100_000.0), (-900_000.0, 0.0)] {
                let mut f = chrome_figure(theme.clone());
                f.axes_mut()
                    .unwrap()
                    .xlim(lo, hi)
                    .unwrap()
                    .ylim(-100_000.0, 100_000.0)
                    .unwrap();
                let scene = f.to_scene(FontLibrary::global());
                assert_text_on_canvas(&scene);
            }

            // Category labels wider than the padding at both ends.
            let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
            f.add_axes(ScaleKind::Category, ScaleKind::Linear)
                .unwrap()
                .title("Revenue by quarter")
                .line(
                    crate::AxisData::Categories(vec![
                        "First quarter of the year".into(),
                        "Q2".into(),
                        "Last quarter of the year".into(),
                    ]),
                    &[1.0, 2.0, 3.0],
                    crate::LineOptions::default(),
                )
                .unwrap();
            assert_text_on_canvas(&f.to_scene(FontLibrary::global()));
        }
    }

    #[test]
    fn above_gridline_inset_grows_only_for_the_first_x_label() {
        let plot_left = |lo: f64| {
            let mut f = chrome_figure(Theme::pigment());
            f.axes_mut().unwrap().xlim(lo, 100_000.0).unwrap();
            let scene = f.to_scene(FontLibrary::global());
            let all = strokes(&scene);
            let (path, _) = all.last().copied().unwrap();
            match path.verbs().first() {
                Some(PathVerb::MoveTo(p)) => p.x,
                _ => panic!("baseline must start with a move"),
            }
        };
        // "0" fits in the padding; "−100000" does not.
        assert_eq!(plot_left(0.0), 12.0);
        assert!(plot_left(-100_000.0) > 12.0);
    }

    #[test]
    fn scenes_are_deterministic() {
        let a = chrome_figure(Theme::pigment()).to_scene(FontLibrary::global());
        let b = chrome_figure(Theme::pigment()).to_scene(FontLibrary::global());
        assert_eq!(a, b);
    }

    // ---- Marks (Task 4) ----

    use crate::{AxisData, LineOptions, ScatterOptions};

    /// The marks group of `scene`: its clip and items.
    fn mark_group(scene: &Scene) -> (Rect, &[Item]) {
        match scene.items.as_slice() {
            [
                Item::Group { clip: None, .. },
                Item::Group {
                    clip: Some(clip),
                    items,
                },
            ] => (*clip, items),
            other => panic!("expected chrome and a clipped marks group, got {other:?}"),
        }
    }

    fn line_strokes(scene: &Scene) -> Vec<(&Path, &StrokeStyle)> {
        mark_group(scene)
            .1
            .iter()
            .filter_map(|item| match item {
                Item::Stroke { path, style } => Some((path, style)),
                _ => None,
            })
            .collect()
    }

    /// The subpaths of `path`, as lists of points.
    fn subpaths(path: &Path) -> Vec<Vec<Point>> {
        let mut out: Vec<Vec<Point>> = Vec::new();
        for verb in path.verbs() {
            match *verb {
                PathVerb::MoveTo(p) => out.push(vec![p]),
                PathVerb::LineTo(p) => out.last_mut().unwrap().push(p),
                PathVerb::Close => {}
                other => panic!("unexpected verb {other:?}"),
            }
        }
        out
    }

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    fn plot_of(figure: &Figure) -> Area {
        crate::layout::plan(
            figure.width_in() * 72.0,
            figure.height_in() * 72.0,
            figure.theme(),
            figure.axes().unwrap(),
            FontLibrary::global(),
        )
        .1
        .plot
    }

    fn line_figure(theme: Theme, x: &[f64], y: &[f64], options: LineOptions) -> Figure {
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(nums(x), y, options)
            .unwrap();
        f
    }

    #[test]
    fn marks_group_is_clipped_to_the_plot_area() {
        let f = line_figure(
            Theme::graphite(),
            &[0.0, 1.0],
            &[0.0, 1.0],
            LineOptions::default(),
        );
        let scene = f.to_scene(FontLibrary::global());
        let plot = plot_of(&f);
        let (clip, items) = mark_group(&scene);
        assert_eq!(
            clip,
            Rect {
                x: plot.x as f32,
                y: plot.y as f32,
                width: plot.width as f32,
                height: plot.height as f32,
            }
        );
        assert_eq!(items.len(), 1);
        assert_scene_bounded(&scene);
        // No marks, or none visible: no marks group.
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(nums(&[f64::NAN]), &[f64::NAN], LineOptions::default())
            .unwrap();
        assert_eq!(f.to_scene(FontLibrary::global()).items.len(), 1);
    }

    #[test]
    fn marks_follow_insertion_order_and_resolved_colors() {
        let theme = Theme::pigment();
        let fixed = Rgba::opaque(1, 2, 3);
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(nums(&[0.0, 1.0]), &[0.0, 1.0], LineOptions::default())
            .unwrap()
            .scatter(nums(&[0.5]), &[0.5], ScatterOptions::default())
            .unwrap()
            .line(
                nums(&[0.0, 1.0]),
                &[1.0, 0.0],
                LineOptions {
                    color: crate::ColorSpec::Fixed(fixed),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global());
        let (_, items) = mark_group(&scene);
        let palette = theme.palette();
        match items {
            [
                Item::Stroke { style: a, .. },
                Item::Markers { fill: Some(b), .. },
                Item::Stroke { style: c, .. },
            ] => {
                assert_eq!(a.color, palette[0]);
                assert_eq!((b.r, b.g, b.b), (palette[1].r, palette[1].g, palette[1].b));
                assert_eq!(c.color, fixed);
            }
            other => panic!("unexpected marks {other:?}"),
        }
    }

    #[test]
    fn lines_far_outside_limits_are_clipped() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let mut f = line_figure(
                theme.clone(),
                &[-1e9, 0.5, 1e9],
                &[-1e9, 0.5, 1e9],
                LineOptions::default(),
            );
            f.axes_mut()
                .unwrap()
                .xlim(0.0, 1.0)
                .unwrap()
                .ylim(0.0, 1.0)
                .unwrap();
            let scene = f.to_scene(FontLibrary::global());
            assert_scene_bounded(&scene);
            let plot = plot_of(&f);
            // The line is y = x. In plot coordinates it is (x + v w, bottom - v h); inside the
            // plot area expanded by m it runs for v in -m / max(w, h) ..= 1 + m / max(w, h).
            let m = f64::from(theme.line_width()).max(2.0);
            let big = plot.width.max(plot.height);
            let at = |v: f64| (plot.x + v * plot.width, plot.bottom() - v * plot.height);
            let expected = [at(-m / big), at(0.5), at(1.0 + m / big)];
            let strokes = line_strokes(&scene);
            let [(path, _)] = strokes.as_slice() else {
                panic!("expected one stroke, got {strokes:?}")
            };
            let subpaths = subpaths(path);
            assert_eq!(subpaths.len(), 1);
            let got = &subpaths[0];
            assert_eq!(got.len(), 3);
            for (p, (ex, ey)) in got.iter().zip(expected) {
                assert!(
                    (f64::from(p.x) - ex).abs() < 1e-3 && (f64::from(p.y) - ey).abs() < 1e-3,
                    "{p:?} vs ({ex}, {ey})"
                );
            }
        }
    }

    #[test]
    fn lines_leaving_and_reentering_start_new_subpaths() {
        // Up through the top edge and back down: two visible pieces.
        let mut f = line_figure(
            Theme::graphite(),
            &[0.0, 0.5, 1.0],
            &[0.5, 1e6, 0.5],
            LineOptions::default(),
        );
        f.axes_mut()
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap();
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let plot = plot_of(&f);
        let top = plot.y - f64::from(Theme::graphite().line_width()).max(2.0);
        let strokes = line_strokes(&scene);
        let parts = subpaths(strokes[0].0);
        assert_eq!(parts.len(), 2);
        for part in &parts {
            assert_eq!(part.len(), 2);
            assert!(part.iter().any(|p| (f64::from(p.y) - top).abs() < 1e-3));
        }

        // Extreme data and limits: mapped coordinates near f64's range stay finite.
        let mut f = line_figure(
            Theme::midnight(),
            &[-1e300, 0.0, 1e300],
            &[1e300, 0.0, -1e300],
            LineOptions::default(),
        );
        f.axes_mut()
            .unwrap()
            .xlim(0.0, 1e-290)
            .unwrap()
            .ylim(-1e-290, 1e-290)
            .unwrap();
        assert_scene_bounded(&f.to_scene(FontLibrary::global()));
    }

    #[test]
    fn nan_breaks_lines() {
        let f = line_figure(
            Theme::graphite(),
            &[0.0, 1.0, 2.0, 3.0, f64::NAN, 5.0, 6.0, 7.0],
            &[1.0, 2.0, f64::NAN, 3.0, 4.0, 2.0, 1.0, 3.0],
            LineOptions::default(),
        );
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let strokes = line_strokes(&scene);
        let parts = subpaths(strokes[0].0);
        // Runs: [0, 1], [3] (an isolated point), [5, 6, 7].
        let lens: Vec<usize> = parts.iter().map(Vec::len).collect();
        assert_eq!(lens, [2, 2, 3]);
        // The isolated point is a zero-length subpath, drawn as a dot by the round cap.
        assert_eq!(parts[1][0], parts[1][1]);
        assert_eq!(strokes[0].1.cap, LineCap::Round);
    }

    #[test]
    fn downsampling_kicks_in_above_threshold() {
        let n = 1_000_000;
        let gaps = [123_457_usize, 500_001, 876_543];
        let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
        // Deterministic noise, so every column keeps four distinct points.
        let mut ys: Vec<f64> = (0..n)
            .map(|i| {
                let h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40;
                (i as f64 / 5e4).sin() + h as f64 / f64::from(1_u32 << 24)
            })
            .collect();
        for &g in &gaps {
            ys[g] = f64::NAN;
        }
        let f = line_figure(Theme::graphite(), &xs, &ys, LineOptions::default());
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let columns = downsample_columns(plot_of(&f).width);
        let strokes = line_strokes(&scene);
        let parts = subpaths(strokes[0].0);
        assert_eq!(parts.len(), gaps.len() + 1);
        let points: usize = parts.iter().map(Vec::len).sum();
        // A gap splits at most one column into two buckets of up to 4 points each.
        assert!(
            points <= 4 * columns + 4 * gaps.len(),
            "{points} points for {columns} columns"
        );
        assert!(
            points > 2 * columns,
            "{points} points for {columns} columns"
        );

        // At or below the threshold, every point is kept.
        let m = 4 * columns;
        let f = line_figure(
            Theme::graphite(),
            &xs[..m],
            &ys[..m].iter().map(|y| y.max(0.0)).collect::<Vec<_>>(),
            LineOptions::default(),
        );
        let scene = f.to_scene(FontLibrary::global());
        let points: usize = subpaths(line_strokes(&scene)[0].0)
            .iter()
            .map(Vec::len)
            .sum();
        assert_eq!(points, m);
    }

    #[test]
    fn category_lines_map_to_band_centers() {
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Categories(vec!["a".into(), "b".into(), "c".into(), "d".into()]),
                &[1.0, 3.0, 2.0, 4.0],
                LineOptions::default(),
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global());
        let plot = plot_of(&f);
        let parts = subpaths(line_strokes(&scene)[0].0);
        let xs: Vec<f64> = parts[0].iter().map(|p| f64::from(p.x)).collect();
        for (x, center) in xs.iter().zip([0.125, 0.375, 0.625, 0.875]) {
            assert!((x - plot.x_at(center)).abs() < 1e-3, "{xs:?}");
        }
    }

    #[test]
    fn area_fill_sits_under_line() {
        let theme = Theme::midnight();
        let f = line_figure(
            theme.clone(),
            &[0.0, 1.0, 2.0, 3.0],
            &[10.0, 40.0, 25.0, 30.0],
            LineOptions::default(),
        );
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let (_, items) = mark_group(&scene);
        let [
            Item::Fill {
                path: area,
                paint: Paint::Linear(gradient),
                rule: FillRule::NonZero,
            },
            Item::Stroke { path: line, style },
        ] = items
        else {
            panic!("expected an area fill under the line, got {items:?}")
        };
        let color = theme.palette()[0];
        assert_eq!(style.color, color);
        let (line_min, _) = line.bounds().unwrap();
        let (area_min, area_max) = area.bounds().unwrap();
        let (_, layout) = crate::layout::plan(
            432.0,
            288.0,
            &theme,
            f.axes().unwrap(),
            FontLibrary::global(),
        );
        let baseline = layout.baseline_y as f32;
        assert_eq!(area_min.y, line_min.y);
        assert_eq!(area_max.y, baseline);
        // The area's outline is the line, then down to the baseline and back.
        let area_points = &subpaths(area)[0];
        let line_points = &subpaths(line)[0];
        assert_eq!(&area_points[..line_points.len()], line_points.as_slice());
        assert_eq!(area_points.len(), line_points.len() + 2);
        assert!(
            area_points[line_points.len()..]
                .iter()
                .all(|p| p.y == baseline)
        );
        // The gradient runs from the line's top at 0.32 alpha to the baseline at 0.
        assert_eq!(gradient.start.y, line_min.y);
        assert_eq!(gradient.end.y, baseline);
        assert_eq!(gradient.start.x, gradient.end.x);
        let alphas: Vec<(f32, u8)> = gradient
            .stops
            .iter()
            .map(|s| (s.offset, s.color.a))
            .collect();
        assert_eq!(alphas, [(0.0, 82), (1.0, 0)]);
        assert!(
            gradient
                .stops
                .iter()
                .all(|s| (s.color.r, s.color.g, s.color.b) == (color.r, color.g, color.b))
        );

        // Without `AreaFill::Gradient`, there is no fill.
        let f = line_figure(
            Theme::graphite(),
            &[0.0, 1.0],
            &[1.0, 2.0],
            LineOptions::default(),
        );
        let scene = f.to_scene(FontLibrary::global());
        assert!(matches!(mark_group(&scene).1, [Item::Stroke { .. }]));
    }

    fn scatter_figure(theme: Theme, x: &[f64], y: &[f64], marker: Marker) -> Figure {
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(
                nums(x),
                y,
                ScatterOptions {
                    marker,
                    ..ScatterOptions::default()
                },
            )
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap();
        f
    }

    #[test]
    fn scatter_drops_points_far_outside() {
        let theme = Theme::graphite();
        let probe = scatter_figure(theme.clone(), &[0.5], &[0.5], Marker::Circle);
        let plot = plot_of(&probe);
        let r = f64::from(theme.marker_size()) / 2.0;
        // In data units: just inside and just outside the radius + 1 pt slack on the right.
        let inside = 1.0 + (r + 0.9) / plot.width;
        let outside = 1.0 + (r + 1.1) / plot.width;
        let f = scatter_figure(
            theme.clone(),
            &[0.5, 1e9, -1e9, f64::NAN, 0.25, inside, outside, 0.5],
            &[0.5, 0.5, 0.5, 0.5, f64::NAN, 0.5, 0.5, -1e12],
            Marker::Circle,
        );
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let (_, items) = mark_group(&scene);
        let [
            Item::Markers {
                positions,
                fill: Some(fill),
                stroke: Some(stroke),
                ..
            },
        ] = items
        else {
            panic!("expected one marker batch, got {items:?}")
        };
        assert_eq!(positions.len(), 2, "{positions:?}");
        assert!((f64::from(positions[0].x) - plot.x_at(0.5)).abs() < 1e-3);
        assert!((f64::from(positions[1].x) - plot.x_at(inside)).abs() < 1e-3);
        // Fill at the theme's marker alpha, outline 0.8 pt in the background color.
        let color = theme.palette()[0];
        assert_eq!(fill.a, (0.9_f32 * 255.0).round() as u8);
        assert_eq!((fill.r, fill.g, fill.b), (color.r, color.g, color.b));
        assert_eq!((stroke.color, stroke.width), (theme.background(), 0.8));
        assert!(stroke.dash.is_empty());

        // Every point outside: no marks group at all.
        let f = scatter_figure(theme, &[5.0, -5.0], &[0.5, 0.5], Marker::Circle);
        assert_eq!(f.to_scene(FontLibrary::global()).items.len(), 1);
    }

    #[test]
    fn scatter_alpha_multiplies_the_color_alpha() {
        let theme = Theme::pigment();
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .scatter(
                nums(&[0.5]),
                &[0.5],
                ScatterOptions {
                    color: crate::ColorSpec::AUTO.with_alpha(crate::Alpha::new(0.5).unwrap()),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        let scene = f.to_scene(FontLibrary::global());
        let [
            Item::Markers {
                fill: Some(fill), ..
            },
        ] = mark_group(&scene).1
        else {
            panic!("expected one marker batch")
        };
        // 128 (alpha 0.5) * 0.85.
        assert_eq!(fill.a, (128.0_f32 * 0.85).round() as u8);
    }

    #[test]
    fn marker_shapes_have_expected_bounds() {
        let d = 6.0_f32;
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        let bounds = |m: Marker| marker_shape(m, d).bounds().unwrap();

        let (min, max) = bounds(Marker::Circle);
        assert!(close(min.x, -3.0) && close(min.y, -3.0) && close(max.x, 3.0) && close(max.y, 3.0));
        assert_eq!(marker_shape(Marker::Circle, d).verbs().len(), 6);

        let h = 0.886 * d / 2.0;
        let (min, max) = bounds(Marker::Square);
        assert!(close(min.x, -h) && close(min.y, -h) && close(max.x, h) && close(max.y, h));

        let (min, max) = bounds(Marker::Diamond);
        let k = h * std::f32::consts::SQRT_2;
        assert!(close(min.x, -k) && close(min.y, -k) && close(max.x, k) && close(max.y, k));

        let (min, max) = bounds(Marker::Cross);
        assert!(close(min.x, -3.0) && close(min.y, -3.0) && close(max.x, 3.0) && close(max.y, 3.0));
        // Arms are d / 3 wide.
        let xs: Vec<f32> = subpaths(&marker_shape(Marker::Cross, d))[0]
            .iter()
            .map(|p| p.x)
            .collect();
        assert!(
            xs.iter()
                .all(|&x| close(x.abs(), 1.0) || close(x.abs(), 3.0))
        );

        // The triangle: equilateral, centroid at 0, with the circle's area.
        let tri = &subpaths(&marker_shape(Marker::Triangle, d))[0];
        assert_eq!(tri.len(), 3);
        let (cx, cy) = tri
            .iter()
            .fold((0.0, 0.0), |(x, y), p| (x + p.x / 3.0, y + p.y / 3.0));
        assert!(close(cx, 0.0) && close(cy, 0.0));
        let side = |a: Point, b: Point| (a.x - b.x).hypot(a.y - b.y);
        let s = side(tri[0], tri[1]);
        assert!(close(s, side(tri[1], tri[2])) && close(s, side(tri[2], tri[0])));
        let area = 3.0_f32.sqrt() / 4.0 * s * s;
        assert!((area - std::f32::consts::PI * 9.0).abs() < 1e-3);
        // Pointing up: the apex has the smallest y.
        assert!(tri[0].y < tri[1].y && close(tri[1].y, tri[2].y));

        // Every shape is used as the scene's marker shape.
        for m in [
            Marker::Circle,
            Marker::Square,
            Marker::Triangle,
            Marker::Diamond,
            Marker::Cross,
        ] {
            let theme = Theme::graphite();
            let scene =
                scatter_figure(theme.clone(), &[0.5], &[0.5], m).to_scene(FontLibrary::global());
            let [Item::Markers { shape, .. }] = mark_group(&scene).1 else {
                panic!("expected one marker batch")
            };
            assert_eq!(shape, &marker_shape(m, theme.marker_size()));
        }
    }

    #[test]
    fn dash_patterns_scale_with_width() {
        for w in [1.0, 3.0] {
            let theme = Theme::graphite().with_line_width(w).unwrap();
            let w = w as f32;
            for (dash, pattern, cap) in [
                (Dash::Solid, vec![], LineCap::Round),
                (Dash::Dashed, vec![4.0 * w, 3.0 * w], LineCap::Butt),
                (Dash::Dotted, vec![0.0, 2.5 * w], LineCap::Round),
            ] {
                let f = line_figure(
                    theme.clone(),
                    &[0.0, 1.0],
                    &[0.0, 1.0],
                    LineOptions {
                        dash,
                        ..LineOptions::default()
                    },
                );
                let scene = f.to_scene(FontLibrary::global());
                let strokes = line_strokes(&scene);
                let style = strokes[0].1;
                assert_eq!(style.dash, pattern, "{dash:?}");
                assert_eq!(
                    (style.cap, style.join, style.width),
                    (cap, LineJoin::Round, w)
                );
            }
        }
    }

    #[test]
    fn mark_scenes_are_deterministic() {
        let build = || {
            let mut f = line_figure(
                Theme::midnight(),
                &[0.0, 1.0, 2.0],
                &[3.0, f64::NAN, 4.0],
                LineOptions::default(),
            );
            f.axes_mut()
                .unwrap()
                .scatter(nums(&[0.5, 1.5]), &[3.5, 3.7], ScatterOptions::default())
                .unwrap();
            f
        };
        assert_eq!(
            build().to_scene(FontLibrary::global()),
            build().to_scene(FontLibrary::global())
        );
        assert_eq!(build().to_png(72).unwrap(), build().to_png(72).unwrap());
    }

    // ---- Pixel sanity ----

    struct Pixels {
        width: usize,
        rgba: Vec<u8>,
    }

    impl Pixels {
        fn decode(png_bytes: &[u8]) -> Pixels {
            let mut reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
                .read_info()
                .unwrap();
            let mut buf = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut buf).unwrap();
            assert_eq!(info.color_type, png::ColorType::Rgba);
            buf.truncate(info.buffer_size());
            Pixels {
                width: info.width as usize,
                rgba: buf,
            }
        }

        fn at(&self, x: usize, y: usize) -> [u8; 3] {
            let i = (y * self.width + x) * 4;
            [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
        }

        /// The pixel under the point `p`, in points, at `dpi`.
        fn at_point(&self, p: (f64, f64), dpi: f64) -> [u8; 3] {
            let s = dpi / 72.0;
            self.at((p.0 * s) as usize, (p.1 * s) as usize)
        }
    }

    fn near(a: [u8; 3], b: Rgba, tolerance: u8) -> bool {
        a.iter()
            .zip([b.r, b.g, b.b])
            .all(|(&x, y)| x.abs_diff(y) <= tolerance)
    }

    #[test]
    fn line_color_appears_in_plot() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let theme = theme.with_line_width(4.0).unwrap();
            let mut f = line_figure(
                theme.clone(),
                &[0.0, 1.0],
                &[0.7, 0.7],
                LineOptions::default(),
            );
            f.axes_mut().unwrap().ylim(0.0, 1.0).unwrap();
            let plot = plot_of(&f);
            let dpi = 144.0;
            let pixels = Pixels::decode(&f.to_png(dpi as u32).unwrap());
            let color = theme.palette()[0];
            let on_line = (plot.x_at(0.5), plot.y_at(0.7));
            assert!(
                near(pixels.at_point(on_line, dpi), color, 2),
                "{:?} vs {color:?}",
                pixels.at_point(on_line, dpi)
            );
            // Above the line there is no line color.
            let above = (plot.x_at(0.5), plot.y_at(0.7) - 6.0);
            assert!(!near(pixels.at_point(above, dpi), color, 40));
        }
    }

    #[test]
    fn scatter_outline_uses_background() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let theme = theme
                .with_marker_size(40.0)
                .unwrap()
                .with_marker_alpha(1.0)
                .unwrap();
            // Two overlapping markers: the second's outline crosses the first's fill.
            let probe = scatter_figure(theme.clone(), &[0.5], &[0.5], Marker::Circle);
            let plot = plot_of(&probe);
            let dx = 16.0 / plot.width;
            let f = scatter_figure(theme.clone(), &[0.5, 0.5 + dx], &[0.5, 0.5], Marker::Circle);
            let dpi = 300.0;
            let pixels = Pixels::decode(&f.to_png(dpi as u32).unwrap());
            let (cx, cy) = (plot.x_at(0.5 + dx), plot.y_at(0.5));
            let color = theme.palette()[0];
            // The second marker's left edge, 20 pt left of its center, inside the first marker.
            let edge = pixels.at_point((cx - 20.0, cy), dpi);
            assert!(near(edge, theme.background(), 24), "{edge:?}");
            // Fill on either side of the outline.
            assert!(near(pixels.at_point((cx - 23.0, cy), dpi), color, 2));
            assert!(near(pixels.at_point((cx - 17.0, cy), dpi), color, 2));
        }
    }
}
