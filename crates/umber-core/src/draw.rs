//! Drawing: turns a [`Layout`] into scene items.
//!
//! The axes chrome (gridlines, the baseline, tick labels, the header and the axis labels) is
//! one unclipped group. The marks follow in a second group, clipped to the plot area, drawn in
//! insertion order.
//!
//! Marks are mapped to plot coordinates in `f64` and clipped there, before anything is
//! narrowed to `f32`: lines against the plot area expanded by `max(line_width, 2)` pt, scatter
//! points against the plot area expanded by each marker shape's extent plus its outline and
//! 1 pt. Every mark coordinate in the scene is therefore within a few points of the plot area.
//!
//! Line clipping must stay accurate for any finite `f64` endpoints, including data far outside
//! narrow limits. Each boundary crossing fixes the crossing coordinate to the boundary value
//! exactly. The other coordinate is found from a reference value `r` inside the clip range: the
//! line's implicit equation is evaluated at `r` exactly (error-free expansion arithmetic), and
//! one division gives the offset from `r` to the crossing. The rounding error therefore scales
//! with that offset (at most the clip rectangle's size for an accepted crossing), not with the
//! endpoints' magnitude, even when both endpoints are astronomically far away.

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
/// Crossings whose computed coordinate is at most this far outside the clip rectangle count
/// as on it; their rounding error is far smaller.
const CROSSING_TOLERANCE: f64 = 1.0e-9;
/// Crossing operands are scaled down by a power of two when any exceeds this magnitude, so the
/// exact products in [`crossing`] cannot overflow.
const MAX_UNSCALED: f64 = 3.273_390_607_896_142e150; // 2^500
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
/// Scatter points whose marker (shape extent plus outline) lies further than this many points
/// outside the plot area are dropped.
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

/// A mapped coordinate: NaN stays NaN (a gap), and an overflowed `±inf` becomes `±f64::MAX`.
/// Finite values are kept as they are. Infinity only arises when normalizing overflows `f64`,
/// where no exact position exists; the clip needs finite operands, since `inf - inf` is NaN.
fn plot_coord(v: f64) -> f64 {
    if v.is_nan() {
        v
    } else {
        v.clamp(-f64::MAX, f64::MAX)
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

type Pt = (f64, f64);

/// Cohen–Sutherland outcode of `p`: bit 0 left, 1 right, 2 above, 3 below.
fn outcode(p: Pt, bounds: &Bounds) -> u8 {
    let mut code = 0;
    if p.0 < bounds.left {
        code |= 1;
    } else if p.0 > bounds.right {
        code |= 2;
    }
    if p.1 < bounds.top {
        code |= 4;
    } else if p.1 > bounds.bottom {
        code |= 8;
    }
    code
}

/// The visible part `(start, end)` of the segment `a..b` inside `bounds`, in the direction
/// from `a` to `b`, or `None` when no part is visible. Endpoints inside `bounds` are returned
/// unchanged; every other returned point lies on the boundary of `bounds`. The endpoints must
/// be finite.
///
/// The candidates are the inside endpoints plus the crossings with the four boundary lines
/// that lie on the boundary, ordered along the segment; the first and last are the visible
/// part.
fn clip_segment(a: Pt, b: Pt, bounds: &Bounds) -> Option<(Pt, Pt)> {
    if outcode(a, bounds) & outcode(b, bounds) != 0 {
        return None;
    }
    let (inside_a, inside_b) = (bounds.contains(a), bounds.contains(b));
    if inside_a && inside_b {
        return Some((a, b));
    }
    let mut candidates: Vec<Pt> = Vec::with_capacity(6);
    if inside_a {
        candidates.push(a);
    }
    if inside_b {
        candidates.push(b);
    }
    let (lo_x, hi_x, lo_y, hi_y) = (bounds.left, bounds.right, bounds.top, bounds.bottom);
    let mid_x = lo_x / 2.0 + hi_x / 2.0;
    let mid_y = lo_y / 2.0 + hi_y / 2.0;
    let on =
        |v: f64, lo: f64, hi: f64| (lo - CROSSING_TOLERANCE..=hi + CROSSING_TOLERANCE).contains(&v);
    for edge in [lo_x, hi_x] {
        if let Some(y) = crossing(a, b, edge, mid_y)
            && on(y, lo_y, hi_y)
        {
            candidates.push((edge, y.clamp(lo_y, hi_y)));
        }
    }
    let swap = |p: Pt| (p.1, p.0);
    for edge in [lo_y, hi_y] {
        if let Some(x) = crossing(swap(a), swap(b), edge, mid_x)
            && on(x, lo_x, hi_x)
        {
            candidates.push((x.clamp(lo_x, hi_x), edge));
        }
    }
    // Order along the segment by its dominant axis, which separates distinct points on it.
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let key = |p: &Pt| {
        if dx.abs() >= dy.abs() {
            if dx < 0.0 { -p.0 } else { p.0 }
        } else if dy < 0.0 {
            -p.1
        } else {
            p.1
        }
    };
    let start = candidates
        .iter()
        .copied()
        .min_by(|p, q| key(p).total_cmp(&key(q)))?;
    let end = candidates
        .iter()
        .copied()
        .max_by(|p, q| key(p).total_cmp(&key(q)))?;
    Some((start, end))
}

/// `a + b` as an unevaluated sum `(s, e)` with `s + e == a + b` exactly (Knuth's TwoSum).
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

/// `a * b` as an unevaluated sum `(p, e)` with `p + e == a * b` exactly (fused multiply-add).
fn two_product(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    (p, a.mul_add(b, -p))
}

/// `p * q - r * s` for exact two-term sums `p`, `q`, `r`, `s`, rounded once: the 16 partial
/// products are accumulated without error into a nonoverlapping expansion (Shewchuk's
/// Grow-Expansion), which is then summed from its smallest component up.
fn cross_difference(p: (f64, f64), q: (f64, f64), r: (f64, f64), s: (f64, f64)) -> f64 {
    let mut expansion: Vec<f64> = Vec::with_capacity(32);
    let mut grow = |v: f64| {
        let mut carry = v;
        let mut next = Vec::with_capacity(expansion.len() + 1);
        for &e in &expansion {
            let (sum, err) = two_sum(carry, e);
            if err != 0.0 {
                next.push(err);
            }
            carry = sum;
        }
        if carry != 0.0 {
            next.push(carry);
        }
        expansion = next;
    };
    for (x, y, sign) in [(p, q, 1.0), (r, s, -1.0)] {
        for u in [x.0, x.1] {
            for v in [y.0, y.1] {
                let (hi, lo) = two_product(u, v);
                grow(sign * hi);
                grow(sign * lo);
            }
        }
    }
    expansion.iter().sum()
}

/// Where the segment `a..b` crosses the vertical line `x = edge`: the y there, or `None` when
/// the segment does not reach the line or runs along it.
///
/// With `h(y) = (edge - ax)(by - ay) - (y - ay)(bx - ax)`, the crossing is
/// `y* = r + h(r) / (bx - ax)` for any `r`. `h(r)` is evaluated exactly and rounded once, so
/// the error is a few ulps of `y* - r`, whatever the magnitude of the endpoints. Operands
/// above [`MAX_UNSCALED`] are first scaled down by a power of two, which keeps the products
/// finite.
fn crossing(a: Pt, b: Pt, edge: f64, r: f64) -> Option<f64> {
    if a.0 == b.0 || edge < a.0.min(b.0) || edge > a.0.max(b.0) {
        return None;
    }
    let largest = [a.0, a.1, b.0, b.1, edge, r]
        .iter()
        .fold(0.0_f64, |m, v| m.max(v.abs()));
    // A power of two `2^-k` with `largest * 2^-k <= 2^500`; exact as long as `k <= 1022`.
    let k = if largest > MAX_UNSCALED {
        ((largest.to_bits() >> 52) & 0x7ff) as i32 - 1023 - 500 + 1
    } else {
        0
    };
    let scale = f64::from_bits(((1023 - k) as u64) << 52);
    let [ax, ay, bx, by, edge_s, r_s] = [a.0, a.1, b.0, b.1, edge, r].map(|v| v * scale);
    let h = cross_difference(
        two_sum(edge_s, -ax),
        two_sum(by, -ay),
        two_sum(r_s, -ay),
        two_sum(bx, -ax),
    );
    // `h` is scaled by `scale^2` and `bx - ax` by `scale`: undo one factor.
    let offset = h / (bx - ax) / scale;
    let y = r + offset;
    y.is_finite().then_some(y)
}

/// Splits the polyline through `(xs[i], ys[i])` at NaN and clips it to `bounds`. Each returned
/// subpath has at least one point; a single point is an isolated data point inside `bounds`.
/// A segment that leaves `bounds` ends its subpath, and the part that re-enters starts a new
/// one.
fn clip_polyline(xs: &[f64], ys: &[f64], bounds: &Bounds) -> Vec<Vec<Pt>> {
    let points: Vec<Pt> = xs.iter().copied().zip(ys.iter().copied()).collect();
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
fn clip_run(run: &[Pt], bounds: &Bounds, out: &mut Vec<Vec<Pt>>) {
    let mut current: Vec<Pt> = Vec::new();
    for pair in run.windows(2) {
        let [a, b] = pair else { continue };
        let (a, b) = (*a, *b);
        match clip_segment(a, b, bounds) {
            None => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            Some((start, end)) => {
                // A segment continues the current subpath only if it starts at its own
                // (unchanged, inside) first point, which the previous segment ended at.
                if current.is_empty() || start != a {
                    if !current.is_empty() {
                        out.push(std::mem::take(&mut current));
                    }
                    current.push(start);
                }
                current.push(end);
                if end != b {
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

    if theme.area_fill() == AreaFill::Gradient {
        for subpath in &subpaths {
            items.extend(area_fills(subpath, layout.baseline_y, color));
        }
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

/// Which side of the baseline `y` lies on: -1 above (smaller y), 1 below, 0 on it.
fn side(y: f64, baseline_y: f64) -> i8 {
    if y < baseline_y {
        -1
    } else if y > baseline_y {
        1
    } else {
        0
    }
}

/// The area between one clipped line `subpath` and `baseline_y`, split where the line crosses
/// the baseline. Each part is its own fill, so parts never cancel each other: a closed polygon
/// (the line, then down to the baseline and back) painted with a vertical gradient from the
/// part's extreme y, at 0.32 of the line's opacity, to the baseline, at 0. Parts above the
/// baseline fade from their top, parts below from their bottom.
fn area_fills(subpath: &[Pt], baseline_y: f64, color: Rgba) -> Vec<Item> {
    // Split into parts that each stay on one side of the baseline.
    let mut parts: Vec<(i8, Vec<Pt>)> = Vec::new();
    let mut points = subpath.iter().copied();
    let Some(first) = points.next() else {
        return Vec::new();
    };
    let mut current = (side(first.1, baseline_y), vec![first]);
    let mut prev = first;
    for p in points {
        let s = side(p.1, baseline_y);
        if current.0 == 0 {
            current.0 = s;
        } else if s != 0 && s != current.0 {
            // Where the line crosses the baseline.
            let x = crossing((prev.1, prev.0), (p.1, p.0), baseline_y, prev.0).unwrap_or(prev.0);
            let c = (x, baseline_y);
            current.1.push(c);
            parts.push(std::mem::replace(&mut current, (s, vec![c])));
        }
        current.1.push(p);
        prev = p;
    }
    parts.push(current);

    let top_alpha = (f32::from(color.a) * AREA_TOP_ALPHA)
        .round()
        .clamp(0.0, 255.0) as u8;
    parts
        .into_iter()
        .filter_map(|(side, part)| {
            let (Some(&first), Some(&last)) = (part.first(), part.last()) else {
                return None;
            };
            let ys = part.iter().map(|p| p.1);
            let extreme = match side {
                -1 => ys.fold(f64::INFINITY, f64::min),
                1 => ys.fold(f64::NEG_INFINITY, f64::max),
                _ => return None,
            };
            if part.len() < 2 || (extreme - baseline_y).abs() < MIN_AREA_HEIGHT {
                return None;
            }
            let mut path = Path::new();
            path.move_to(scene_point(first));
            for &p in part.iter().skip(1) {
                path.line_to(scene_point(p));
            }
            path.line_to(point(last.0, baseline_y));
            path.line_to(point(first.0, baseline_y));
            path.close();
            Some(Item::Fill {
                path,
                paint: Paint::Linear(LinearGradient {
                    start: point(0.0, extreme),
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
        })
        .collect()
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

/// The largest `|x|` and `|y|` of any point of `path`, control points included.
fn extent(path: &Path) -> (f64, f64) {
    path.bounds().map_or((0.0, 0.0), |(min, max)| {
        (
            f64::from(min.x.abs().max(max.x.abs())),
            f64::from(min.y.abs().max(max.y.abs())),
        )
    })
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
/// theme's marker alpha times the color's own, outlined 0.8 pt in the background color. Points
/// whose marker cannot reach the plot area are dropped.
fn scatter_item(
    theme: &Theme,
    plot: Area,
    xs: &[f64],
    ys: &[f64],
    color: Rgba,
    shape: Marker,
) -> Option<Item> {
    let d = marker_size(theme);
    let shape_path = marker_shape(shape, d);
    // Cull by the shape's own extent on each axis, plus the outline (whose miter joins reach
    // at most its width beyond the path for these shapes) and the slack.
    let (half_w, half_h) = extent(&shape_path);
    let reach = f64::from(MARKER_OUTLINE) + MARKER_SLACK;
    let bounds = Bounds {
        left: plot.x - half_w - reach,
        top: plot.y - half_h - reach,
        right: plot.right() + half_w + reach,
        bottom: plot.bottom() + half_h + reach,
    };
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
        shape: shape_path,
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
        // In data units: just inside and just outside the radius + 0.8 pt outline + 1 pt slack
        // on the right.
        let inside = 1.0 + (r + 1.7) / plot.width;
        let outside = 1.0 + (r + 1.9) / plot.width;
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

    // ---- Robust clipping, split areas and shape-aware culling (PR #15 review) ----

    #[test]
    fn huge_horizontal_lines_keep_their_visible_width() {
        for x in [1e20, 1e107, 1e300] {
            let mut f = line_figure(
                Theme::graphite(),
                &[-x, x * 0.7],
                &[0.5, 0.5],
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
            let m = f64::from(Theme::graphite().line_width()).max(2.0);
            let parts = subpaths(line_strokes(&scene)[0].0);
            assert_eq!(parts.len(), 1);
            let [a, b] = parts[0].as_slice() else {
                panic!("expected one segment, got {parts:?}")
            };
            assert!((f64::from(a.x) - (plot.x - m)).abs() < 1e-3, "{a:?}");
            assert!((f64::from(b.x) - (plot.right() + m)).abs() < 1e-3, "{b:?}");
            for p in [a, b] {
                assert!((f64::from(p.y) - plot.y_at(0.5)).abs() < 1e-3, "{p:?}");
            }
        }
    }

    #[test]
    fn huge_to_inside_segments_keep_the_inside_end() {
        for v in [1e12, 1e20, 1e60, 1e150, 1e300] {
            let mut f = line_figure(
                Theme::midnight(),
                &[-v, 0.5],
                &[-v, 0.5],
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
            let m = f64::from(Theme::midnight().line_width()).max(2.0);
            let big = plot.width.max(plot.height);
            let at = |v: f64| (plot.x + v * plot.width, plot.bottom() - v * plot.height);
            let parts = subpaths(line_strokes(&scene)[0].0);
            let [a, b] = parts[0].as_slice() else {
                panic!("expected one segment, got {parts:?}")
            };
            for (p, (ex, ey)) in [(a, at(-m / big)), (b, at(0.5))] {
                assert!(
                    (f64::from(p.x) - ex).abs() < 1e-3 && (f64::from(p.y) - ey).abs() < 1e-3,
                    "{v}: {p:?} vs ({ex}, {ey})"
                );
            }
            // The inside end is the mapped data point itself.
            assert_eq!(*b, point(at(0.5).0, at(0.5).1));
        }
    }

    #[test]
    fn narrow_limits_stay_bounded_on_every_axis_combination() {
        let log = ScaleKind::Log {
            base: crate::LogBase::TEN,
        };
        let mut state = 151_010_u64;
        let mut rng = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1_u64 << 53) as f64
        };
        for xkind in [ScaleKind::Linear, log, ScaleKind::Category] {
            for ykind in [ScaleKind::Linear, log] {
                let mut visible = 0;
                for _ in 0..50 {
                    let (a, b) = (0.5 + rng() * 1.5, 0.5 + rng() * 1.5);
                    let x = match xkind {
                        ScaleKind::Linear => nums(&[-1e60 * a, 1e60 * b]),
                        ScaleKind::Category => AxisData::Categories(vec!["a".into(), "b".into()]),
                        _ if ykind == ScaleKind::Linear => nums(&[1.0 + 0.3e-8, 1.0 + 0.7e-8]),
                        _ => nums(&[1e-300, 1e300]),
                    };
                    let y = match (xkind, ykind) {
                        (ScaleKind::Linear, ScaleKind::Linear) => [0.5e-50; 2],
                        (ScaleKind::Linear, _) => [1.0 + 0.5e-8; 2],
                        (_, ScaleKind::Linear) => [-1e60 * b, 1e60 * a],
                        _ => [1e-300, 1e300],
                    };
                    let mut f = Figure::new(6.0, 4.0, Theme::midnight()).unwrap();
                    let axes = f.add_axes(xkind, ykind).unwrap();
                    axes.line(x.clone(), &y, LineOptions::default())
                        .unwrap()
                        .scatter(x, &y, ScatterOptions::default())
                        .unwrap();
                    match xkind {
                        ScaleKind::Linear => drop(axes.xlim(0.0, 1e-50).unwrap()),
                        ScaleKind::Category => {}
                        _ => drop(axes.xlim(1.0, 1.0 + 1e-8).unwrap()),
                    }
                    if ykind == ScaleKind::Linear {
                        axes.ylim(0.0, 1e-50).unwrap();
                    } else {
                        axes.ylim(1.0, 1.0 + 1e-8).unwrap();
                    }
                    let scene = f.to_scene(FontLibrary::global());
                    assert_scene_bounded(&scene);
                    if scene.items.len() == 2 {
                        visible += 1;
                    }
                }
                // Every one of these lines crosses the plot area.
                assert_eq!(visible, 50, "{xkind:?} / {ykind:?}");
            }
        }
    }

    /// The PNG pixel at data position `(x, y)` of a figure with limits 0..1 by -2..2.
    fn data_pixel(f: &Figure, png: &[u8], x: f64, y: f64) -> [u8; 3] {
        let plot = plot_of(f);
        Pixels::decode(png).at_point((plot.x_at(x), plot.y_at((y + 2.0) / 4.0)), 300.0)
    }

    fn area_figure(theme: &Theme, x: &[f64], y: &[f64]) -> Figure {
        let mut f = line_figure(theme.clone(), x, y, LineOptions::default());
        f.axes_mut()
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .ylim(-2.0, 2.0)
            .unwrap();
        f
    }

    /// Whether the area fill shows at each data position: the pixel differs from the same
    /// figure drawn without area fill.
    fn area_shows(x: &[f64], y: &[f64], probes: &[(f64, f64)]) -> Vec<bool> {
        let theme = Theme::midnight().with_grid_lines(GridLines::None).unwrap();
        let with = area_figure(&theme, x, y);
        let without = area_figure(&theme.with_area_fill(AreaFill::None).unwrap(), x, y);
        let (a, b) = (with.to_png(300).unwrap(), without.to_png(300).unwrap());
        probes
            .iter()
            .map(|&(px, py)| data_pixel(&with, &a, px, py) != data_pixel(&without, &b, px, py))
            .collect()
    }

    #[test]
    fn areas_crossing_the_baseline_fill_both_sides() {
        // One line from +1 down to -1: filled above the baseline on the left, below on the
        // right, and nowhere beyond the line.
        let shows = area_shows(
            &[0.0, 0.5, 1.0],
            &[1.0, 0.0, -1.0],
            &[(0.1, 0.4), (0.9, -0.4), (0.1, -0.4), (0.9, 0.4)],
        );
        assert_eq!(shows, [true, true, false, false]);

        // NaN-separated runs on opposite sides, each fading towards the baseline.
        let shows = area_shows(
            &[0.0, 0.4, f64::NAN, 0.6, 1.0],
            &[1.0, 1.0, f64::NAN, -1.0, -1.0],
            &[(0.2, 0.5), (0.8, -0.5), (0.2, -0.5), (0.8, 0.5)],
        );
        assert_eq!(shows, [true, true, false, false]);

        // Each side is its own fill, with its gradient from its extreme to the baseline.
        let theme = Theme::midnight();
        let f = area_figure(&theme, &[0.0, 0.5, 1.0], &[1.0, 0.0, -1.0]);
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let plot = plot_of(&f);
        let baseline = plot.y_at(0.5) as f32;
        let gradients: Vec<&LinearGradient> = mark_group(&scene)
            .1
            .iter()
            .filter_map(|item| match item {
                Item::Fill {
                    paint: Paint::Linear(g),
                    ..
                } => Some(g),
                _ => None,
            })
            .collect();
        let [above, below] = gradients.as_slice() else {
            panic!("expected two area fills, got {gradients:?}")
        };
        assert!((above.start.y - plot.y_at(0.75) as f32).abs() < 1e-3);
        assert!((below.start.y - plot.y_at(0.25) as f32).abs() < 1e-3);
        for g in [above, below] {
            assert_eq!(g.end.y, baseline);
            assert_eq!((g.stops[0].color.a, g.stops[1].color.a), (82, 0));
        }
    }

    #[test]
    fn opposite_runs_do_not_cancel_their_areas() {
        let shows = area_shows(
            &[0.0, 1.0, f64::NAN, 1.0, 0.0],
            &[1.0, 1.0, f64::NAN, 1.0, 1.0],
            &[(0.5, 0.5)],
        );
        assert_eq!(shows, [true]);
        // A reversed run alone is filled too.
        assert_eq!(area_shows(&[1.0, 0.0], &[1.0, 1.0], &[(0.5, 0.5)]), [true]);
    }

    #[test]
    fn markers_are_culled_by_their_true_extent() {
        // The reviewer's repros: the visible top of a triangle and a diamond centered below the
        // plot area.
        let plot = Area {
            x: 10.0,
            y: 20.0,
            width: 400.0,
            height: 240.0,
        };
        for (shape, d, offset) in [(Marker::Triangle, 12.0, 7.5), (Marker::Diamond, 40.0, 23.0)] {
            let theme = Theme::graphite().with_marker_size(d).unwrap();
            let y = plot.bottom() + offset;
            let (min, _) = marker_shape(shape, d as f32).bounds().unwrap();
            assert!(
                y + f64::from(min.y) < plot.bottom(),
                "{shape:?} reaches the plot"
            );
            let item = scatter_item(&theme, plot, &[210.0], &[y], Rgba::opaque(255, 0, 0), shape);
            assert!(item.is_some(), "{shape:?} was dropped");
        }

        // Per shape and axis: kept just inside the reach, dropped just outside it.
        let theme = Theme::graphite().with_marker_size(10.0).unwrap();
        for shape in [
            Marker::Circle,
            Marker::Square,
            Marker::Triangle,
            Marker::Diamond,
            Marker::Cross,
        ] {
            let (w, h) = extent(&marker_shape(shape, 10.0));
            let reach = 0.8 + 1.0;
            let cases = [
                (plot.x - w - reach + 0.01, 100.0, true),
                (plot.x - w - reach - 0.01, 100.0, false),
                (plot.right() + w + reach - 0.01, 100.0, true),
                (plot.right() + w + reach + 0.01, 100.0, false),
                (200.0, plot.y - h - reach + 0.01, true),
                (200.0, plot.y - h - reach - 0.01, false),
                (200.0, plot.bottom() + h + reach - 0.01, true),
                (200.0, plot.bottom() + h + reach + 0.01, false),
            ];
            for (x, y, kept) in cases {
                let item = scatter_item(&theme, plot, &[x], &[y], Rgba::BLACK, shape);
                assert_eq!(item.is_some(), kept, "{shape:?} at ({x}, {y})");
            }
        }
    }

    #[test]
    fn subpath_splitting_edge_cases() {
        let bounds = Bounds {
            left: 10.0,
            top: 20.0,
            right: 410.0,
            bottom: 260.0,
        };
        // A zero-length run on a corner, then a run that leaves through the right edge and
        // comes back.
        let parts = clip_polyline(
            &[10.0, 10.0, f64::NAN, 30.0, 500.0, 30.0],
            &[20.0, 20.0, 0.0, 100.0, 100.0, 200.0],
            &bounds,
        );
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], [(10.0, 20.0), (10.0, 20.0)]);
        assert_eq!(parts[1], [(30.0, 100.0), (410.0, 100.0)]);
        assert_eq!(parts[2].first().map(|p| p.0), Some(410.0));
        assert_eq!(parts[2].last(), Some(&(30.0, 200.0)));
        // NaN in either coordinate breaks the run; a lone inside point survives.
        let parts = clip_polyline(&[10.0, f64::NAN, 10.0], &[f64::NAN, 30.0, 20.0], &bounds);
        assert_eq!(parts, [vec![(10.0, 20.0)]]);
        // A lone point outside is dropped.
        assert!(clip_polyline(&[0.0], &[0.0], &bounds).is_empty());
    }

    #[test]
    fn clipping_matches_an_exact_rational_oracle() {
        let stats = oracle::run(10_000);
        println!("{stats}");
    }

    /// An exact oracle for [`clip_segment`]: Liang–Barsky in exact dyadic and rational
    /// arithmetic on a small big-integer type, so it shares no rounding with the code under
    /// test.
    mod oracle {
        use super::super::{Bounds, Pt, clip_segment};
        use std::cmp::Ordering;
        use std::collections::BTreeMap;

        /// A signed big integer: little-endian 64-bit limbs without trailing zeros.
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct Big {
            neg: bool,
            mag: Vec<u64>,
        }

        fn trim(mut v: Vec<u64>) -> Vec<u64> {
            while v.last() == Some(&0) {
                v.pop();
            }
            v
        }

        fn cmp_mag(a: &[u64], b: &[u64]) -> Ordering {
            a.len()
                .cmp(&b.len())
                .then_with(|| a.iter().rev().cmp(b.iter().rev()))
        }

        fn add_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
            let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
            let mut carry = 0_u128;
            for i in 0..a.len().max(b.len()) {
                let s = u128::from(*a.get(i).unwrap_or(&0))
                    + u128::from(*b.get(i).unwrap_or(&0))
                    + carry;
                out.push(s as u64);
                carry = s >> 64;
            }
            out.push(carry as u64);
            trim(out)
        }

        /// `a - b` for `|a| >= |b|`.
        fn sub_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
            let mut out = Vec::with_capacity(a.len());
            let mut borrow = 0_i128;
            for (i, &x) in a.iter().enumerate() {
                let mut d = i128::from(x) - i128::from(*b.get(i).unwrap_or(&0)) - borrow;
                borrow = 0;
                if d < 0 {
                    d += 1_i128 << 64;
                    borrow = 1;
                }
                out.push(d as u64);
            }
            trim(out)
        }

        fn mul_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
            let mut out = vec![0_u64; a.len() + b.len()];
            for (i, &x) in a.iter().enumerate() {
                let mut carry = 0_u128;
                for (j, &y) in b.iter().enumerate() {
                    let t = u128::from(x) * u128::from(y) + u128::from(out[i + j]) + carry;
                    out[i + j] = t as u64;
                    carry = t >> 64;
                }
                out[i + b.len()] = carry as u64;
            }
            trim(out)
        }

        fn shl_mag(a: &[u64], bits: u32) -> Vec<u64> {
            if a.is_empty() {
                return Vec::new();
            }
            let (limbs, rem) = ((bits / 64) as usize, bits % 64);
            let mut out = vec![0_u64; limbs];
            let mut carry = 0_u64;
            for &x in a {
                if rem == 0 {
                    out.push(x);
                } else {
                    out.push((x << rem) | carry);
                    carry = x >> (64 - rem);
                }
            }
            out.push(carry);
            trim(out)
        }

        impl Big {
            fn new(neg: bool, mag: Vec<u64>) -> Big {
                let mag = trim(mag);
                Big {
                    neg: neg && !mag.is_empty(),
                    mag,
                }
            }
            fn add(&self, o: &Big) -> Big {
                if self.neg == o.neg {
                    return Big::new(self.neg, add_mag(&self.mag, &o.mag));
                }
                match cmp_mag(&self.mag, &o.mag) {
                    Ordering::Less => Big::new(o.neg, sub_mag(&o.mag, &self.mag)),
                    _ => Big::new(self.neg, sub_mag(&self.mag, &o.mag)),
                }
            }
            fn neg(&self) -> Big {
                Big::new(!self.neg, self.mag.clone())
            }
            fn mul(&self, o: &Big) -> Big {
                Big::new(self.neg != o.neg, mul_mag(&self.mag, &o.mag))
            }
            fn shl(&self, bits: u32) -> Big {
                Big::new(self.neg, shl_mag(&self.mag, bits))
            }
            fn signum(&self) -> i32 {
                if self.mag.is_empty() {
                    0
                } else if self.neg {
                    -1
                } else {
                    1
                }
            }
            /// `|self|` as `(m, e)` with `|self| ≈ m * 2^e`.
            fn approx(&self) -> (f64, i64) {
                let Some(&top) = self.mag.last() else {
                    return (0.0, 0);
                };
                let below = if self.mag.len() >= 2 {
                    self.mag[self.mag.len() - 2]
                } else {
                    0
                };
                let m = top as f64 * 2f64.powi(64) + below as f64;
                (m, 64 * (self.mag.len() as i64 - 2))
            }
        }

        /// An exact binary fraction `m * 2^e`.
        #[derive(Clone, Debug)]
        struct Dy {
            m: Big,
            e: i32,
        }

        impl Dy {
            fn from_f64(v: f64) -> Dy {
                assert!(v.is_finite());
                if v == 0.0 {
                    return Dy {
                        m: Big::new(false, Vec::new()),
                        e: 0,
                    };
                }
                let bits = v.to_bits();
                let exp = ((bits >> 52) & 0x7ff) as i32;
                let frac = bits & ((1_u64 << 52) - 1);
                let (m, e) = if exp == 0 {
                    (frac, -1074)
                } else {
                    (frac | (1_u64 << 52), exp - 1075)
                };
                Dy {
                    m: Big::new(v < 0.0, vec![m]),
                    e,
                }
            }
            fn int(v: u64) -> Dy {
                Dy {
                    m: Big::new(false, vec![v]),
                    e: 0,
                }
            }
            fn align(&self, o: &Dy) -> (Big, Big, i32) {
                let e = self.e.min(o.e);
                (
                    self.m.shl((self.e - e) as u32),
                    o.m.shl((o.e - e) as u32),
                    e,
                )
            }
            fn add(&self, o: &Dy) -> Dy {
                let (a, b, e) = self.align(o);
                Dy { m: a.add(&b), e }
            }
            fn sub(&self, o: &Dy) -> Dy {
                self.add(&o.neg())
            }
            fn neg(&self) -> Dy {
                Dy {
                    m: self.m.neg(),
                    e: self.e,
                }
            }
            fn mul(&self, o: &Dy) -> Dy {
                Dy {
                    m: self.m.mul(&o.m),
                    e: self.e + o.e,
                }
            }
            fn signum(&self) -> i32 {
                self.m.signum()
            }
            fn abs(&self) -> Dy {
                if self.signum() < 0 {
                    self.neg()
                } else {
                    self.clone()
                }
            }
            fn cmp(&self, o: &Dy) -> Ordering {
                self.sub(o).signum().cmp(&0)
            }
            /// `self / o` for positive values, approximately.
            fn ratio(&self, o: &Dy) -> f64 {
                let ((m1, e1), (m2, e2)) = (self.m.approx(), o.m.approx());
                if m1 == 0.0 {
                    return 0.0;
                }
                let shift = (e1 + i64::from(self.e) - e2 - i64::from(o.e)).clamp(-2000, 2000);
                m1 / m2 * 2f64.powi(shift as i32)
            }
        }

        /// A rational `n / d` with `d > 0`.
        #[derive(Clone, Debug)]
        struct Q {
            n: Dy,
            d: Dy,
        }

        impl Q {
            fn lt(&self, o: &Q) -> bool {
                self.n.mul(&o.d).cmp(&o.n.mul(&self.d)) == Ordering::Less
            }
            fn eq(&self, o: &Q) -> bool {
                self.n.mul(&o.d).cmp(&o.n.mul(&self.d)) == Ordering::Equal
            }
        }

        /// The exact visible parameter interval of `a..b` in `bounds`.
        fn exact(a: Pt, b: Pt, bounds: &Bounds) -> Option<(Q, Q)> {
            let f = Dy::from_f64;
            let (ax, ay) = (f(a.0), f(a.1));
            let (dx, dy) = (f(b.0).sub(&ax), f(b.1).sub(&ay));
            let edges = [
                (dx.neg(), ax.sub(&f(bounds.left))),
                (dx.clone(), f(bounds.right).sub(&ax)),
                (dy.neg(), ay.sub(&f(bounds.top))),
                (dy.clone(), f(bounds.bottom).sub(&ay)),
            ];
            let mut t0 = Q {
                n: Dy::int(0),
                d: Dy::int(1),
            };
            let mut t1 = Q {
                n: Dy::int(1),
                d: Dy::int(1),
            };
            for (p, q) in edges {
                match p.signum() {
                    0 => {
                        if q.signum() < 0 {
                            return None;
                        }
                    }
                    s => {
                        let t = if s < 0 {
                            Q {
                                n: q.neg(),
                                d: p.neg(),
                            }
                        } else {
                            Q { n: q, d: p }
                        };
                        if s < 0 {
                            if t1.lt(&t) {
                                return None;
                            }
                            if t0.lt(&t) {
                                t0 = t;
                            }
                        } else {
                            if t.lt(&t0) {
                                return None;
                            }
                            if t.lt(&t1) {
                                t1 = t;
                            }
                        }
                    }
                }
            }
            (!t1.lt(&t0)).then_some((t0, t1))
        }

        /// The error of `got` against the exact point at `t` on `a..b`, in points; `None`
        /// when it exceeds 1e-3.
        fn error(a: Pt, b: Pt, t: &Q, got: Pt) -> Option<f64> {
            let f = Dy::from_f64;
            let mut worst = 0.0_f64;
            for (a, b, g) in [(a.0, b.0, got.0), (a.1, b.1, got.1)] {
                // got - (a + t (b - a)) = (got d - a d - n (b - a)) / d.
                let (a, d) = (f(a), f(b).sub(&f(a)));
                let diff = f(g).mul(&t.d).sub(&a.mul(&t.d)).sub(&t.n.mul(&d)).abs();
                if diff.mul(&Dy::int(1000)).cmp(&t.d) == Ordering::Greater {
                    return None;
                }
                worst = worst.max(diff.ratio(&t.d));
            }
            Some(worst)
        }

        /// Runs `n` seeded random segments through [`clip_segment`] and the oracle; panics on
        /// any disagreement and returns a summary.
        pub(super) fn run(n: usize) -> String {
            let mut state = 151_010_u64;
            let mut rng = move || {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                (state >> 11) as f64 / (1_u64 << 53) as f64
            };
            let fixed = Bounds {
                left: 10.0,
                top: 20.0,
                right: 410.0,
                bottom: 260.0,
            };
            let corners = [(10.0, 20.0), (410.0, 20.0), (10.0, 260.0), (410.0, 260.0)];
            let mut counts: BTreeMap<&str, [usize; 3]> = BTreeMap::new();
            let (mut max_error, mut touches) = (0.0_f64, 0);
            for i in 0..n {
                let mut u = |lo: f64, hi: f64| lo + (hi - lo) * rng();
                let (tag, a, b): (&str, Pt, Pt) = match i % 20 {
                    0..=3 => (
                        "ordinary",
                        (u(-1000.0, 1000.0), u(-1000.0, 1000.0)),
                        (u(-1000.0, 1000.0), u(-1000.0, 1000.0)),
                    ),
                    4..=7 => {
                        let c = corners[(u(0.0, 4.0) as usize).min(3)];
                        let (dx, dy) = (u(-1000.0, 1000.0).round(), u(-1000.0, 1000.0).round());
                        ("corner", (c.0 - dx, c.1 - dy), (c.0 + dx, c.1 + dy))
                    }
                    8..=10 => {
                        let (s, t) = (u(-1000.0, 1000.0), u(-1000.0, 1000.0));
                        let edge = [10.0, 410.0, 20.0, 260.0][(u(0.0, 4.0) as usize).min(3)];
                        if edge > 15.0 && edge < 300.0 && edge != 410.0 {
                            ("boundary", (s, edge), (t, edge))
                        } else {
                            ("boundary", (edge, s), (edge, t))
                        }
                    }
                    11 | 12 => {
                        let p = (u(-1000.0, 1000.0), u(-1000.0, 1000.0));
                        ("degenerate", p, p)
                    }
                    13..=15 => {
                        let scale = 10f64.powf(u(9.0, 300.0));
                        let (a, b) = match (u(0.0, 3.0) as usize).min(2) {
                            0 => {
                                let y = u(-100.0, 350.0);
                                ((-scale * u(0.5, 2.0), y), (scale * u(0.5, 2.0), y))
                            }
                            1 => {
                                let x = u(-100.0, 500.0);
                                ((x, -scale * u(0.5, 2.0)), (x, scale * u(0.5, 2.0)))
                            }
                            _ => ((-scale, -scale * u(0.5, 2.0)), (scale, scale * u(0.5, 2.0))),
                        };
                        if u(0.0, 1.0) < 0.5 {
                            ("huge", a, b)
                        } else {
                            ("huge", b, a)
                        }
                    }
                    16 | 17 => {
                        let scale = 10f64.powf(u(12.0, 300.0));
                        let far = (scale * u(-2.0, 2.0), scale * u(-2.0, 2.0));
                        let inside = (u(10.0, 410.0), u(20.0, 260.0));
                        if u(0.0, 1.0) < 0.5 {
                            ("huge_to_inside", far, inside)
                        } else {
                            ("huge_to_inside", inside, far)
                        }
                    }
                    18 => {
                        // Both ends astronomically far, on a line exactly through the origin,
                        // inside a clip rectangle around it.
                        let scale = 10f64.powf(u(7.0, 307.0));
                        let far = (scale * u(-1.0, 1.0), scale * u(-1.0, 1.0));
                        ("huge_symmetric", far, (-far.0, -far.1))
                    }
                    _ => {
                        // Both ends far, on a line through a point near the clip rectangle.
                        let scale = 10f64.powf(u(7.0, 300.0));
                        let c = (u(-50.0, 470.0), u(-50.0, 330.0));
                        let angle = u(0.0, std::f64::consts::TAU);
                        let (dx, dy) = (angle.cos(), angle.sin());
                        let (s, t) = (scale * u(0.5, 2.0), scale * u(0.5, 2.0));
                        (
                            "huge_through",
                            (c.0 - s * dx, c.1 - s * dy),
                            (c.0 + t * dx, c.1 + t * dy),
                        )
                    }
                };
                let bounds = if tag == "huge_symmetric" {
                    Bounds {
                        left: -u(1.0, 400.0),
                        top: -u(1.0, 300.0),
                        right: u(1.0, 400.0),
                        bottom: u(1.0, 300.0),
                    }
                } else {
                    fixed
                };
                let got = clip_segment(a, b, &bounds);
                let want = exact(a, b, &bounds);
                let entry = counts.entry(tag).or_default();
                entry[0] += 1;
                match (want, got) {
                    (None, None) => {}
                    (Some((t0, t1)), Some((start, end))) => {
                        entry[1] += 1;
                        for p in [start, end] {
                            assert!(bounds.contains(p), "{tag} {a:?} {b:?}: {p:?} outside");
                            assert!(p.0.abs() <= 1e6 && p.1.abs() <= 1e6);
                        }
                        let e0 = error(a, b, &t0, start);
                        let e1 = error(a, b, &t1, end);
                        let (Some(e0), Some(e1)) = (e0, e1) else {
                            panic!("{tag} {a:?} {b:?}: got {start:?} {end:?}, error above 1e-3")
                        };
                        max_error = max_error.max(e0).max(e1);
                    }
                    (Some((t0, t1)), None) => {
                        // Only a segment that merely touches the boundary may be missed.
                        assert!(t0.eq(&t1), "{tag} {a:?} {b:?}: visible part lost");
                        entry[2] += 1;
                        touches += 1;
                    }
                    (None, Some((start, end))) => {
                        // Only a rounding-level touch may be reported for a miss.
                        assert_eq!(start, end, "{tag} {a:?} {b:?}: invisible part drawn");
                        entry[2] += 1;
                        touches += 1;
                    }
                }
            }
            let mut summary = format!(
                "{n} segments, max intersection error {max_error:.3e} pt, {touches} \
                 boundary touches resolved either way"
            );
            for (tag, [total, visible, touch]) in counts {
                summary.push_str(&format!(
                    "\n  {tag}: {total} cases, {visible} visible, {touch} touches"
                ));
            }
            summary
        }
    }
}
