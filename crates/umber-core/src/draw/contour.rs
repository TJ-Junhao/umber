//! Contours: the filled bands, the lines along the levels and the level labels on them.
//!
//! The geometry comes in grid index space ([`crate::contour`]); a point maps onto the axes by
//! interpolating, in axis space, between the coordinates of its column and of its row, so a
//! cell is a rectangle on the plot. Bands and lines are clipped in axis space to the plot
//! expanded by a margin, as lines are ([`LineClip`]), before they are mapped to plot points.
//!
//! The bands are filled in order, each over the bands before it, with the even-odd rule, so
//! neighbouring bands meet without a seam and holes stay empty. The lines of
//! [`ContourStyle::Both`] go over the bands. A level label sits at the middle of a line long
//! enough to hold it ([`labels::place`]), turned along it and upright, in the tick label font
//! and the theme's text color, and the line is broken under it [`LABEL_GAP`] pt either side.

use super::MarkContext;
use super::clip::clip_ring;
use super::line::{LineClip, stroke};
use super::mapping::{AxisPoint, PlotPoint, axis_numbers, point};
use crate::color_scale::ResolvedColorScale;
use crate::contour::labels::{self, PagePoint, Placement};
use crate::contour::{ContourShapes, GridPoint};
use crate::layout::{Label, centered_tick_label};
use crate::resolve::Resolved;
use crate::scene::{FillRule, Item, LineCap, LineJoin, Paint, StrokeStyle, TextItem};
use crate::{
    Alpha, ColorSpec, ContourMark, ContourStyle, FontLibrary, Path, PathVerb, Point, Rgba,
};

/// The width of the lines of [`ContourStyle::Lines`], in points.
const LINE_WIDTH: f32 = 1.25;
/// The width of the lines of [`ContourStyle::Both`], in points.
const BOTH_LINE_WIDTH: f32 = 0.6;
/// The opacity of the lines of [`ContourStyle::Both`], in the theme's text color.
const BOTH_LINE_OPACITY: f32 = 0.7;
/// The length of line left out on each side of a level label, in points.
const LABEL_GAP: f64 = 3.0;

/// A resolved contour mark, as [`contour_items`] draws it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Contour<'a> {
    /// The mark.
    pub(super) mark: &'a ContourMark,
    /// Whether its colors come from the color scale; otherwise its lines take `color`.
    pub(super) mapped: bool,
    /// The color of its lines when they take one color.
    pub(super) color: Rgba,
    /// Its lines and bands in grid index space.
    pub(super) shapes: &'a ContourShapes,
}

/// The color of the lines of a contour.
#[derive(Debug, Clone, Copy)]
enum LineColor<'a> {
    /// One color for every line.
    One(Rgba),
    /// Each line in the color of its level on the axes' color scale, at the opacity `alpha`;
    /// no line without a color scale.
    ByLevel {
        /// The color scale, if the axes have one.
        scale: Option<&'a ResolvedColorScale>,
        /// The opacity of the lines.
        alpha: Alpha,
    },
}

impl LineColor<'_> {
    /// The color of the line along `level`; `None` when no line is drawn.
    fn at(self, level: f64) -> Option<Rgba> {
        match self {
            LineColor::One(color) => Some(color),
            LineColor::ByLevel { scale, alpha } => Some(scale?.color(level)?.with_alpha(alpha)),
        }
    }
}

/// The map of grid index space onto axis space: the axis-space coordinates of the columns and
/// of the rows, between which points are interpolated.
struct GridMap {
    /// The axis-space x of each column.
    columns: Vec<f64>,
    /// The axis-space y of each row.
    rows: Vec<f64>,
}

impl GridMap {
    /// The axis-space point of the grid point `p`.
    fn at(&self, p: GridPoint) -> AxisPoint {
        AxisPoint::new(
            interpolated(&self.columns, p.x),
            interpolated(&self.rows, p.y),
        )
    }
}

/// The value at the fractional index `at` of `values`, at least two of them: interpolated
/// linearly between the two values around it.
fn interpolated(values: &[f64], at: f64) -> f64 {
    let last = values.len().saturating_sub(2);
    // Grid points lie in `0..=len - 1`; the cast saturates and the index is clamped.
    let i = (at.floor().max(0.0) as usize).min(last);
    let t = at - i as f64;
    match (values.get(i), values.get(i + 1)) {
        (Some(&a), Some(&b)) => a + (b - a) * t,
        _ => f64::NAN,
    }
}

/// The items of `contour` appended to `items`: its bands, when it is filled and the axes have a
/// color scale; then its lines, colored by level on the color scale (none without one) or in its
/// own color, or for [`ContourStyle::Both`] in the theme's text color; then its level labels,
/// shaped with `lib`.
pub(super) fn contour_items(
    context: &MarkContext<'_>,
    lib: &FontLibrary,
    resolved: &Resolved,
    contour: Contour<'_>,
    items: &mut Vec<Item>,
) {
    let mark = contour.mark;
    let map = GridMap {
        columns: axis_numbers(&resolved.x, mark.x()),
        rows: axis_numbers(&resolved.y, mark.y()),
    };
    let levels = mark.levels();
    if let Some(scale) = &resolved.color {
        band_items(context, &map, contour.shapes, (&levels, scale), items);
    }
    let (width, line_color) = match mark.options().style {
        ContourStyle::Both => (
            BOTH_LINE_WIDTH,
            LineColor::One(context.theme.text_color().scale_alpha(BOTH_LINE_OPACITY)),
        ),
        ContourStyle::Lines | ContourStyle::Filled => (
            LINE_WIDTH,
            match (contour.mapped, mark.options().color) {
                (true, ColorSpec::Auto(alpha)) => LineColor::ByLevel {
                    scale: resolved.color.as_ref(),
                    alpha,
                },
                _ => LineColor::One(contour.color),
            },
        ),
    };
    let label_color = context.theme.text_color();
    let mut labels = Vec::new();
    for (index, lines) in contour.shapes.lines.iter().enumerate() {
        let (Some(&level), Some(text)) = (levels.get(index), mark.labels().get(index)) else {
            continue;
        };
        let Some(color) = line_color.at(level) else {
            continue;
        };
        let label = mark
            .options()
            .annotate
            .then(|| centered_tick_label(lib, context.theme, text, (0.0, 0.0)));
        let clip = LineClip::of(context, width);
        let mut subpaths: Vec<Vec<PlotPoint>> = Vec::new();
        for line in lines {
            let (xs, ys): (Vec<f64>, Vec<f64>) = line
                .points
                .iter()
                .map(|&p| {
                    let at = map.at(p);
                    (at.x, at.y)
                })
                .unzip();
            let pieces = super::clip::clip_polyline(&xs, &ys, &clip.bounds);
            // A closed line that the clip leaves whole is still closed.
            let closed = line.closed && pieces.len() == 1 && pieces[0].len() == xs.len();
            for piece in pieces {
                let page: Vec<PagePoint> = piece
                    .into_iter()
                    .map(|p| {
                        let at = clip.plot_point(p);
                        (at.x, at.y)
                    })
                    .collect();
                let Some(label) = &label else {
                    subpaths.push(plot_points(&page));
                    continue;
                };
                let page = if closed {
                    labels::rotated_ring(&page, index)
                } else {
                    page
                };
                match labels::place(&page, closed, label.width(), LABEL_GAP) {
                    Some((placement, parts)) => {
                        subpaths.extend(parts.iter().map(|part| plot_points(part)));
                        labels.push(label_item(label, placement, label_color));
                    }
                    None => subpaths.push(plot_points(&page)),
                }
            }
        }
        subpaths.retain(|subpath| subpath.len() >= 2);
        if !subpaths.is_empty() {
            items.push(stroke(&subpaths, &line_style(width, color)));
        }
    }
    items.extend(labels);
}

/// The bands of `shapes` appended to `items`, each filled in its color on `scale` between its
/// lower level and the next of `levels` ([`ResolvedColorScale::band_color`]), clipped in axis
/// space to the plot and a margin.
fn band_items(
    context: &MarkContext<'_>,
    map: &GridMap,
    shapes: &ContourShapes,
    (levels, scale): (&[f64], &ResolvedColorScale),
    items: &mut Vec<Item>,
) {
    let clip = LineClip::of(context, 0.0);
    for (index, band) in shapes.bands.iter().enumerate() {
        let below = index.checked_sub(1).and_then(|i| levels.get(i)).copied();
        let above = levels.get(index).copied();
        let mut path = Path::new();
        for ring in &band.rings {
            let ring: Vec<AxisPoint> = ring.iter().map(|&p| map.at(p)).collect();
            let clipped = clip_ring(&ring, &clip.bounds);
            let mut points = clipped.iter().map(|&p| clip.plot_point(p).to_scene());
            let Some(first) = points.next() else {
                continue;
            };
            if clipped.len() < 3 {
                continue;
            }
            path.move_to(first);
            points.for_each(|p| path.line_to(p));
            path.close();
        }
        if !path.is_empty() {
            items.push(Item::Fill {
                path,
                paint: Paint::Solid(scale.band_color(below, above)),
                rule: FillRule::EvenOdd,
            });
        }
    }
}

/// The plot points of the page points `points`.
fn plot_points(points: &[PagePoint]) -> Vec<PlotPoint> {
    points.iter().map(|&(x, y)| PlotPoint { x, y }).collect()
}

/// The stroke of contour lines `width` pt wide in `color`, with round ends and joins.
fn line_style(width: f32, color: Rgba) -> StrokeStyle {
    StrokeStyle {
        color,
        width,
        dash: Vec::new(),
        cap: LineCap::Round,
        join: LineJoin::Round,
    }
}

/// The level label `label`, measured centered on the origin, drawn in `color` at `placement`:
/// turned about its centre, which goes to the placement's centre.
fn label_item(label: &Label, placement: Placement, color: Rgba) -> Item {
    let (sin, cos) = placement.angle.to_radians().sin_cos();
    let (cx, cy) = placement.center;
    // A turn clockwise on the page (y down), then the move to the centre.
    let place = |x: f64, y: f64| (cx + x * cos - y * sin, cy + x * sin + y * cos);
    let (x, y) = place(label.x, label.y);
    let origin = point(x, y);
    let outline = label.layout.outline(point(label.x, label.y));
    let turn = |p: Point| {
        let (x, y) = place(f64::from(p.x), f64::from(p.y));
        point(x, y)
    };
    let mut turned = Path::new();
    for verb in outline.verbs() {
        match *verb {
            PathVerb::MoveTo(p) => turned.move_to(turn(p)),
            PathVerb::LineTo(p) => turned.line_to(turn(p)),
            PathVerb::QuadTo(c, p) => turned.quad_to(turn(c), turn(p)),
            PathVerb::CubicTo(c1, c2, p) => turned.cubic_to(turn(c1), turn(c2), turn(p)),
            PathVerb::Close => turned.close(),
        }
    }
    Item::RotatedText {
        text: TextItem {
            outline: turned,
            ..TextItem::from_layout(
                label.text.clone(),
                label.style,
                origin,
                color,
                &label.layout,
            )
        },
        angle: placement.angle as f32,
    }
}
