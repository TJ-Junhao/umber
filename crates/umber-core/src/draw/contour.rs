//! Contours, on a grid or on a triangle mesh: the filled bands, the lines along the levels and
//! the level labels on them.
//!
//! The geometry comes in the field's own space ([`crate::contour`]), and one drawing serves
//! both fields once each point is mapped onto the axes:
//!
//! - on a grid, a point of index space maps by interpolating, in data coordinates, between the
//!   coordinates of its column and of its row, so a cell is a rectangle on the plot;
//! - on a mesh, a point of the mesh maps as a data point does, and a crossing on an edge lies
//!   at its fraction of the way between the ends of the edge in axis space: on a log axis the
//!   field is linear along the edge as drawn, in the axis's log space, so the crossing lies on
//!   the straight edge between its points on the plot, where neighbouring triangles meet.
//!
//! Bands and lines are clipped in axis space to the plot expanded by a margin, as lines are
//! ([`LineClip`]), before they are mapped to plot points.
//!
//! The bands are filled in order, each over the bands before it, with the even-odd rule, so
//! neighbouring bands meet without a seam and holes stay empty. The lines of
//! [`ContourStyle::Both`] go over the bands.
//!
//! A level label sits on a line long enough to hold it, turned along it and upright, in the
//! tick label font, and the line is broken under it [`LABEL_GAP`] pt either side. It takes the
//! middle of the line, or else the first position along it, nearest the middle first, where
//! its box with [`LABEL_GAP`] pt kept clear all around lies wholly inside the plot area and
//! inside the field, clear of its holes and of its edge ([`labels::place`], [`labels::Room`]);
//! a line with no such position has no label and is drawn whole. On the bands of
//! [`ContourStyle::Both`] a label lies across the band below its level and the band above it,
//! and takes whichever of the theme's text color and its background color reads better on those
//! of them that are painted ([`label_color`]); the labels of [`ContourStyle::Lines`] take the
//! theme's text color.
//!
//! A line colored by its level whose color comes close to the background is drawn darker, or
//! lighter on a dark background ([`super::contrast`]): with `w = clamp(1 − d / 0.25, 0, 1)` from
//! the Oklab distance `d` between the level's color, taken opaque, and the theme background, the
//! line takes that color with its Oklab lightness moved `w × 0.25` away from the background's
//! and clamped to `0..=1`, a and b unchanged, at the line's own opacity and width. A line with
//! `w = 0`, a line in one color and the lines of [`ContourStyle::Both`] keep their color; the
//! colorbar keeps the colormap's colors.

use super::MarkContext;
use super::clip::clip_ring;
use super::contrast::{Background, contrast_ratio};
use super::line::{LineClip, stroke};
use super::mapping::{AxisPoint, PlotPoint, axis_value, point};
use super::triangles::axis_points;
use crate::color_scale::ResolvedColorScale;
use crate::contour::labels::{self, LabelBox, PagePoint, Placement, Room};
use crate::contour::{BOTH_LINE_WIDTH, ContourShapes, GridPoint, LINE_WIDTH, MeshPoint};
use crate::layout::{Label, centered_tick_label};
use crate::model::ContourSpec;
use crate::resolve::{Resolved, ResolvedAxis};
use crate::scene::{FillRule, Item, LineCap, LineJoin, Paint, StrokeStyle, TextItem};
use crate::text::SceneFonts;
use crate::{
    Alpha, ColorSpec, ContourMark, ContourStyle, Path, PathVerb, Point, Result, Rgba, Theme,
    TricontourMark,
};

/// The opacity of the lines of [`ContourStyle::Both`], in the theme's text color.
const BOTH_LINE_OPACITY: f32 = 0.7;
/// The length of line left out on each side of a level label, in points.
const LABEL_GAP: f64 = 3.0;
/// The area in square points above which a band's ring counts as painted: a ring that encloses
/// no more, such as the outline of a ridge that only reaches its level, paints nothing.
const PAINTED_AREA: f64 = 1e-6;

/// A resolved contour, on a grid or a mesh, as [`grid_contour_items`] and
/// [`mesh_contour_items`] draw it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Contour<'a, P> {
    /// The levels and options of the mark.
    pub(super) spec: &'a ContourSpec,
    /// Whether its colors come from the color scale; otherwise its lines take `color`.
    pub(super) mapped: bool,
    /// The color of its lines when they take one color.
    pub(super) color: Rgba,
    /// The labels of its levels, as `annotate` writes them.
    pub(super) labels: &'a [String],
    /// Its lines and bands in the field's space, whose points are `P`.
    pub(super) shapes: &'a ContourShapes<P>,
}

/// The color of the lines of a contour.
#[derive(Debug, Clone, Copy)]
enum LineColor<'a> {
    /// One color for every line.
    One(Rgba),
    /// Each line in the color of its level on the axes' color scale, moved away from the
    /// `background` when it comes close to it ([`Background::contrast`]), at the opacity
    /// `alpha`; no line without a color scale.
    ByLevel {
        /// The color scale, if the axes have one.
        scale: Option<&'a ResolvedColorScale>,
        /// The opacity of the lines.
        alpha: Alpha,
        /// The theme background.
        background: Background,
    },
}

impl LineColor<'_> {
    /// The color of the line along `level`; `None` when no line is drawn.
    fn at(self, level: f64) -> Option<Rgba> {
        match self {
            LineColor::One(color) => Some(color),
            LineColor::ByLevel {
                scale,
                alpha,
                background,
            } => {
                let color = scale?.color(level)?;
                Some(background.apart(color).with_alpha(alpha))
            }
        }
    }
}

/// The map of grid index space onto axis space: a point is interpolated linearly between the
/// coordinates of its column and of its row, as given, then mapped onto the axes, so a point
/// at a fraction of a cell side lies at that fraction of the side in the data on any axis.
struct GridMap<'a> {
    /// The x of each column, as given.
    columns: &'a [f64],
    /// The y of each row, as given.
    rows: &'a [f64],
    /// The x axis.
    x: &'a ResolvedAxis,
    /// The y axis.
    y: &'a ResolvedAxis,
}

impl GridMap<'_> {
    /// The axis-space point of the grid point `p`.
    fn at(&self, p: GridPoint) -> AxisPoint {
        AxisPoint::new(
            axis_value(self.x, interpolated(self.columns, p.x)),
            axis_value(self.y, interpolated(self.rows, p.y)),
        )
    }
}

/// The axis-space point of the point `p` of a mesh whose points lie at `points` in axis space:
/// a crossing on an edge lies its fraction of the way between the ends of the edge, exactly at
/// an end at 0 or 1.
fn mesh_at(points: &[AxisPoint], p: MeshPoint) -> AxisPoint {
    match p {
        MeshPoint::Corner(i) => points[i],
        MeshPoint::Edge { from, to, t } => {
            let (a, b) = (points[from], points[to]);
            AxisPoint::new(between(a.x, b.x, t), between(a.y, b.y, t))
        }
    }
}

/// The value at the fractional index `at` of `values`, at least two of them: interpolated
/// linearly between the two values around it, exactly the value itself at a whole index.
fn interpolated(values: &[f64], at: f64) -> f64 {
    let last = values.len().saturating_sub(2);
    // Grid points lie in `0..=len - 1`; the cast saturates and the index is clamped.
    let i = (at.floor().max(0.0) as usize).min(last);
    let t = at - i as f64;
    match (values.get(i), values.get(i + 1)) {
        (Some(&a), Some(&b)) => between(a, b, t),
        _ => f64::NAN,
    }
}

/// The value the fraction `t` of the way from `a` to `b`: exactly `a` at 0 and `b` at 1. Across
/// zero it weighs both ends, which keeps a small end exact beside a huge one; on one side of
/// zero it steps from `a`, which keeps the result between the ends.
fn between(a: f64, b: f64, t: f64) -> f64 {
    if t <= 0.0 {
        a
    } else if t >= 1.0 {
        b
    } else if (a <= 0.0) != (b <= 0.0) {
        a * (1.0 - t) + b * t
    } else {
        a + (b - a) * t
    }
}

/// The items of the contour `contour` of the grid `mark` appended to `items`, as
/// [`contour_items`] draws them.
///
/// # Errors
///
/// A level label that cannot be laid out fails as `fonts` reports it.
pub(super) fn grid_contour_items(
    context: &MarkContext<'_>,
    fonts: &SceneFonts<'_>,
    resolved: &Resolved,
    (mark, contour): (&ContourMark, Contour<'_, GridPoint>),
    items: &mut Vec<Item>,
) -> Result<()> {
    let map = GridMap {
        columns: mark.x(),
        rows: mark.y(),
        x: &resolved.x,
        y: &resolved.y,
    };
    contour_items(context, fonts, resolved, contour, |p| map.at(p), items)
}

/// The items of the contour `contour` of the mesh `mark` appended to `items`, as
/// [`contour_items`] draws them.
///
/// # Errors
///
/// A level label that cannot be laid out fails as `fonts` reports it.
pub(super) fn mesh_contour_items(
    context: &MarkContext<'_>,
    fonts: &SceneFonts<'_>,
    resolved: &Resolved,
    (mark, contour): (&TricontourMark, Contour<'_, MeshPoint>),
    items: &mut Vec<Item>,
) -> Result<()> {
    let points = axis_points(resolved, mark.mesh());
    contour_items(
        context,
        fonts,
        resolved,
        contour,
        |p| mesh_at(&points, p),
        items,
    )
}

/// The items of `contour` appended to `items`, each point of its shapes mapped to axis space by
/// `at`: its bands, when it is filled and the axes have a color scale; then its lines, colored
/// by level on the color scale (none without one) and moved away from the background where
/// they come close to it, or in their own color, or for [`ContourStyle::Both`] in the theme's
/// text color; then its level labels, shaped with the scene's `fonts`, placed inside the plot
/// area and the field ([`labels::place`]) and colored by [`label_color`].
///
/// # Errors
///
/// A level label that cannot be laid out fails as `fonts` reports it.
fn contour_items<P: Copy>(
    context: &MarkContext<'_>,
    fonts: &SceneFonts<'_>,
    resolved: &Resolved,
    contour: Contour<'_, P>,
    at: impl Fn(P) -> AxisPoint,
    items: &mut Vec<Item>,
) -> Result<()> {
    let spec = contour.spec;
    let levels = spec.levels();
    let painted = match &resolved.color {
        Some(scale) => band_items(context, &at, contour.shapes, (&levels, scale), items),
        None => Vec::new(),
    };
    let (width, line_color) = match spec.options().style {
        ContourStyle::Both => (
            BOTH_LINE_WIDTH,
            LineColor::One(context.theme.text_color().scale_alpha(BOTH_LINE_OPACITY)),
        ),
        ContourStyle::Lines | ContourStyle::Filled => (
            LINE_WIDTH,
            match (contour.mapped, spec.options().color) {
                (true, ColorSpec::Auto(alpha)) => LineColor::ByLevel {
                    scale: resolved.color.as_ref(),
                    alpha,
                    background: Background::of(context.theme),
                },
                _ => LineColor::One(contour.color),
            },
        ),
    };
    // Without a format of its own, a contour on a colorbar writes its levels as the bar does.
    let texts = match (&resolved.color, &spec.options().format) {
        (Some(scale), None) if contour.mapped => scale
            .levels
            .as_ref()
            .map_or(contour.labels, |levels| levels.labels.as_slice()),
        _ => contour.labels,
    };
    // Labels lie across the bands on either side of their level when the bands are drawn.
    let banded = match spec.options().style {
        ContourStyle::Both => resolved.color.as_ref(),
        ContourStyle::Lines | ContourStyle::Filled => None,
    };
    let room = spec
        .options()
        .annotate
        .then(|| label_room(context, &at, &contour.shapes.field));
    let mut labels = Vec::new();
    for (index, lines) in contour.shapes.lines.iter().enumerate() {
        let (Some(&level), Some(text)) = (levels.get(index), texts.get(index)) else {
            continue;
        };
        let Some(color) = line_color.at(level) else {
            continue;
        };
        let label = spec
            .options()
            .annotate
            .then(|| centered_tick_label(fonts, context.theme, text, (0.0, 0.0)))
            .transpose()?;
        // The band below the level is band `index`, the one above it band `index + 1`; only
        // those painted count.
        let bands: Vec<Rgba> = banded.map_or_else(Vec::new, |scale| {
            let below = index.checked_sub(1).and_then(|i| levels.get(i)).copied();
            let above = levels.get(index + 1).copied();
            [
                (index, scale.band_color(below, Some(level))),
                (index + 1, scale.band_color(Some(level), above)),
            ]
            .into_iter()
            .filter(|&(band, _)| painted.get(band) == Some(&true))
            .map(|(_, color)| color)
            .collect()
        });
        let label_color = label_color(context.theme, &bands);
        let clip = LineClip::of(context, width);
        let mut subpaths: Vec<Vec<PlotPoint>> = Vec::new();
        for line in lines {
            let (xs, ys): (Vec<f64>, Vec<f64>) = line
                .points
                .iter()
                .map(|&p| {
                    let point = at(p);
                    (point.x, point.y)
                })
                .unzip();
            let pieces = super::clip::clip_polyline(&xs, &ys, &clip.bounds);
            // A closed line stays closed when the clip leaves it one piece that still returns to
            // its start; a clipped ring is open, even with as many points.
            let closed = line.closed
                && matches!(pieces.as_slice(), [piece] if piece.len() > 2 && piece.first() == piece.last());
            for piece in pieces {
                let page: Vec<PagePoint> = piece
                    .into_iter()
                    .map(|p| {
                        let at = clip.plot_point(p);
                        (at.x, at.y)
                    })
                    .collect();
                let (Some(label), Some(room)) = (&label, &room) else {
                    subpaths.push(plot_points(&page));
                    continue;
                };
                let page = if closed {
                    labels::rotated_ring(&page, index)
                } else {
                    page
                };
                match labels::place(&page, closed, (label_box(label), LABEL_GAP), room) {
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
    Ok(())
}

/// The color of a level label on a contour of `theme` that lies across the painted `bands` on
/// either side of its level: the band below it and the band above, each only when it is
/// painted ([`band_items`]), so one of them, both, or none.
///
/// Of the theme's text color and its background color, taken opaque, the label takes the one
/// whose lowest WCAG 2 contrast ratio ([`contrast_ratio`]) against the band colors is higher,
/// so it reads on every band it lies across; on a tie it takes the text color. A label across
/// no band takes the text color: the labels of [`ContourStyle::Lines`]. On the bands of
/// [`ContourStyle::Both`] the band below a drawn line is always painted, since the field just
/// below the level, beside the line, lies in it; only the band above can be missing, as above
/// a ridge that only reaches the level.
pub(super) fn label_color(theme: &Theme, bands: &[Rgba]) -> Rgba {
    let text = theme.text_color();
    if bands.is_empty() {
        return text;
    }
    let background = Rgba {
        a: 255,
        ..theme.background()
    };
    let least = |color: Rgba| {
        bands
            .iter()
            .map(|&band| contrast_ratio(color, band))
            .fold(f64::INFINITY, f64::min)
    };
    if least(background) > least(text) {
        background
    } else {
        text
    }
}

/// The room for the level labels of a contour whose field is bounded by `rings`, each point
/// mapped to axis space by `at`: the plot area, and the field as its rings bound it, each ring
/// clipped in axis space to the plot and a margin as the bands are and mapped to plot points.
/// The margin lies outside the plot area, so clipping changes no edge a label could reach.
fn label_room<P: Copy>(
    context: &MarkContext<'_>,
    at: &impl Fn(P) -> AxisPoint,
    rings: &[Vec<P>],
) -> Room {
    let clip = LineClip::of(context, 0.0);
    let page: Vec<Vec<PagePoint>> = rings
        .iter()
        .map(|ring| {
            let ring: Vec<AxisPoint> = ring.iter().map(|&p| at(p)).collect();
            clip_ring(&ring, &clip.bounds)
                .into_iter()
                .map(|p| {
                    let at = clip.plot_point(p);
                    (at.x, at.y)
                })
                .collect()
        })
        .collect();
    let plot = context.plot;
    Room::new((plot.x, plot.y, plot.right(), plot.bottom()), &page)
}

/// The box of the text of `label`, measured centred on the origin: along its advance width and
/// from its font's ascender to its descender.
fn label_box(label: &Label) -> LabelBox {
    LabelBox {
        left: label.x,
        top: label.y - f64::from(label.layout.ascent()),
        right: label.x + label.width(),
        bottom: label.y + f64::from(label.layout.descent()),
    }
}

/// The bands of `shapes` appended to `items`, each point mapped to axis space by `at`, each band
/// filled in its color on `scale` between its lower level and the next of `levels`
/// ([`ResolvedColorScale::band_color`]), clipped in axis space to the plot and a margin: and
/// whether each band is painted, that is whether one of its rings, so clipped, has an area of
/// more than [`PAINTED_AREA`] on the page.
fn band_items<P: Copy>(
    context: &MarkContext<'_>,
    at: &impl Fn(P) -> AxisPoint,
    shapes: &ContourShapes<P>,
    (levels, scale): (&[f64], &ResolvedColorScale),
    items: &mut Vec<Item>,
) -> Vec<bool> {
    let clip = LineClip::of(context, 0.0);
    let mut painted = Vec::with_capacity(shapes.bands.len());
    for (index, band) in shapes.bands.iter().enumerate() {
        let below = index.checked_sub(1).and_then(|i| levels.get(i)).copied();
        let above = levels.get(index).copied();
        let mut path = Path::new();
        let mut area = false;
        for ring in &band.rings {
            let ring: Vec<AxisPoint> = ring.iter().map(|&p| at(p)).collect();
            let clipped: Vec<PlotPoint> = clip_ring(&ring, &clip.bounds)
                .into_iter()
                .map(|p| clip.plot_point(p))
                .collect();
            let mut points = clipped.iter().map(|p| p.to_scene());
            let Some(first) = points.next() else {
                continue;
            };
            if clipped.len() < 3 {
                continue;
            }
            area |= ring_area(&clipped) > PAINTED_AREA;
            path.move_to(first);
            points.for_each(|p| path.line_to(p));
            path.close();
        }
        painted.push(area);
        if !path.is_empty() {
            items.push(Item::Fill {
                path,
                paint: Paint::Solid(scale.band_color(below, above)),
                rule: FillRule::EvenOdd,
            });
        }
    }
    painted
}

/// The area of the ring through `points` on the page, in square points, whichever way round.
fn ring_area(points: &[PlotPoint]) -> f64 {
    let twice: f64 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum();
    twice.abs() / 2.0
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
    // A turn clockwise on the page (y down), then the move to the centre.
    let (x, y) = placement.to_page((label.x, label.y));
    let origin = point(x, y);
    let outline = label.layout.outline(point(label.x, label.y));
    let turn = |p: Point| {
        let (x, y) = placement.to_page((f64::from(p.x), f64::from(p.y)));
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
