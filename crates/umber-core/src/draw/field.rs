//! Vector fields: arrows and wind barbs, and the scale of automatic arrows.
//!
//! A mark is drawn as one item per color rather than one per arrow, so a page of arrows stays a
//! handful of paths in every format: the arrows of a color are subpaths of one filled path, the
//! staffs and lines of the barbs of a color one stroked path, and their flags one filled path.
//! Items follow the order in which colors first appear, so overlapping arrows of a color mapping
//! are not drawn in data order. All arrows wind the same way, so their nonzero
//! fill stays solid where they overlap.
//!
//! A sample that its shape cannot carry to the plot is dropped in axis space, before anything is
//! mapped to points. Its shape reaches at most the length of the longest one, so everything left
//! stays within that length of the plot, and [`MAX_ARROW_LENGTH`] keeps that within the
//! renderers' coordinates. The marks clip cuts what reaches past the plot edge.
//!
//! A color-mapped arrow or barb whose color comes close to the background moves away from it, as
//! a contour line does ([`Background::apart`]); one color and the colorbar stay as they are.

use std::collections::HashMap;

use super::MarkContext;
use super::clip::Bounds;
use super::contrast::Background;
use super::mapping::{AxisMaps, AxisPoint, PlotPoint, axis_shared, point};
use crate::color_scale::{MappedColors, ResolvedColorScale};
use crate::field::arrow::{ArrowFrame, HEAD_WIDTH, Outline, SHAFT_WIDTH};
use crate::field::barb::{self, Barb};
use crate::field::{Reach, SCREEN_SCALES, spacing};
use crate::geometry::Path;
use crate::layout::Area;
use crate::model::{BarbsMark, MAX_ARROW_LENGTH, QuiverMark, VectorField};
use crate::resolve::{Resolved, ResolvedMark};
use crate::scene::{FillRule, Item, LineCap, LineJoin, Paint, StrokeStyle};
use crate::{ArrowLength, Direction, FieldForm, Pivot, Rgba, Theme};

/// The share of the shorter side of the plot that the longest automatic arrow takes at most.
const MOST_OF_THE_PLOT: f64 = 0.75;
/// The share of a radius by which the control points of a quarter circle leave its ends.
const CIRCLE_CONTROL: f64 = 0.552_284_749_830_793_6;

/// The colors of the arrows or barbs of a mark.
#[derive(Debug, Clone, Copy)]
pub(super) enum FieldFill<'a> {
    /// Every arrow in this color.
    Uniform(Rgba),
    /// Each arrow in the color its value maps to.
    Mapped {
        /// The axes' color scale.
        scale: &'a ResolvedColorScale,
        /// One value per value of `u`.
        values: &'a [f64],
    },
}

impl FieldFill<'_> {
    /// The color at `index` before it moves away from the background; `None` for a value that
    /// maps to no color (NaN), which is not drawn.
    fn at(self, index: usize) -> Option<Rgba> {
        match self {
            FieldFill::Uniform(color) => Some(color),
            FieldFill::Mapped { scale, values } => {
                values.get(index).and_then(|&value| scale.color(value))
            }
        }
    }
}

/// How a field mark in `color` is filled: in that color, or, when it maps its colors from values
/// (`mapped`), in the colors of the axes' color scale `scale`. `None` for a color-mapped mark
/// without a color scale (no finite mapped value), which draws nothing.
pub(super) fn fill<'a, 'm: 'a>(
    scale: Option<&'a ResolvedColorScale>,
    color: Rgba,
    mapped: Option<&'a MappedColors<'m>>,
) -> Option<FieldFill<'a>> {
    match (mapped, scale) {
        (None, _) => Some(FieldFill::Uniform(color)),
        (Some(mapped), Some(scale)) => Some(FieldFill::Mapped {
            scale,
            values: mapped.values,
        }),
        (Some(_), None) => None,
    }
}

/// The paths of one mark, one set per color, in the order the colors first appear.
struct Batches {
    /// The colors of the mapped colors, moved away from the background.
    drawn: HashMap<Rgba, Rgba>,
    /// The index in `paths` of each color, as drawn.
    index: HashMap<Rgba, usize>,
    /// Each color, as drawn, with its stroked and its filled path.
    paths: Vec<(Rgba, Path, Path)>,
    /// The theme background, for mapped colors; `None` for one color.
    background: Option<Background>,
}

impl Batches {
    /// The batches of a mark filled as `fill` under `theme`.
    fn new(theme: &Theme, fill: FieldFill<'_>) -> Batches {
        Batches {
            drawn: HashMap::new(),
            index: HashMap::new(),
            paths: Vec::new(),
            background: match fill {
                FieldFill::Uniform(_) => None,
                FieldFill::Mapped { .. } => Some(Background::of(theme)),
            },
        }
    }

    /// The stroked and the filled path of `color`.
    fn of(&mut self, color: Rgba) -> (&mut Path, &mut Path) {
        let drawn = match self.background {
            Some(background) => *self
                .drawn
                .entry(color)
                .or_insert_with(|| background.apart(color)),
            None => color,
        };
        let paths = &mut self.paths;
        let at = *self.index.entry(drawn).or_insert_with(|| {
            paths.push((drawn, Path::new(), Path::new()));
            paths.len() - 1
        });
        let (_, stroked, filled) = &mut paths[at];
        (stroked, filled)
    }

    /// Appends the items of the batches: each color's stroke, then its fill, with the stroke
    /// made by `style` from the color.
    fn append(self, style: impl Fn(Rgba) -> StrokeStyle, items: &mut Vec<Item>) {
        for (color, stroked, filled) in self.paths {
            if !stroked.is_empty() {
                items.push(Item::Stroke {
                    path: stroked,
                    style: style(color),
                });
            }
            if !filled.is_empty() {
                items.push(Item::Fill {
                    path: filled,
                    paint: Paint::Solid(color),
                    rule: FillRule::NonZero,
                });
            }
        }
    }
}

/// The axis-space box around the plot in which a shape reaching `reach` pt can still touch it.
fn culling_bounds(context: &MarkContext<'_>, reach: f64) -> Bounds {
    let (left, right) = context.maps.x.expanded(reach + 1.0);
    let (top, bottom) = context.maps.y.expanded(reach + 1.0);
    Bounds {
        left,
        top,
        right,
        bottom,
    }
}

/// The scene point `offset` points from `at`, the offset with y up.
fn offset(at: PlotPoint, [x, y]: [f64; 2]) -> crate::geometry::Point {
    point(at.x + x, at.y - y)
}

/// Appends the closed polygon of `corners` around `at` to `path`.
fn push_polygon(path: &mut Path, at: PlotPoint, corners: &[[f64; 2]]) {
    let mut points = corners.iter().map(|&corner| offset(at, corner));
    if let Some(first) = points.next() {
        path.move_to(first);
        for point in points {
            path.line_to(point);
        }
        path.close();
    }
}

/// Appends a closed circle of `radius` around `at` to `path`, clockwise on the page, as arrows
/// wind.
fn push_circle(path: &mut Path, at: PlotPoint, radius: f64) {
    let c = CIRCLE_CONTROL * radius;
    let p = |corner| offset(at, corner);
    path.move_to(p([0.0, radius]));
    path.cubic_to(p([c, radius]), p([radius, c]), p([radius, 0.0]));
    path.cubic_to(p([radius, -c]), p([c, -radius]), p([0.0, -radius]));
    path.cubic_to(p([-c, -radius]), p([-radius, -c]), p([-radius, 0.0]));
    path.cubic_to(p([-radius, c]), p([-c, radius]), p([0.0, radius]));
    path.close();
}

/// Appends the arrows of `mark`, in the `fill` and the `frame` of the layout (see the module
/// documentation). A zero vector draws a dot as wide as the shaft.
pub(super) fn quiver_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &QuiverMark,
    (fill, frame): (FieldFill<'_>, ArrowFrame),
    items: &mut Vec<Item>,
) {
    let field = mark.field();
    let pivot = mark.options().pivot;
    let xs = axis_shared(&resolved.x, field.x_values());
    let ys = axis_shared(&resolved.y, field.y_values());
    let bounds = culling_bounds(context, frame.length_of(field.longest()) + HEAD_WIDTH);
    let mut batches = Batches::new(context.theme, fill);
    for sample in field.samples() {
        let Some(color) = fill.at(sample.index) else {
            continue;
        };
        let position = AxisPoint::new(xs[sample.column], ys[sample.row]);
        if !bounds.contains(position) {
            continue;
        }
        let at = context.maps.to_plot(position);
        let (_, path) = batches.of(color);
        match frame.outline(sample.u, sample.v, pivot) {
            Some(outline) => push_polygon(path, at, outline.corners()),
            None => push_circle(path, at, SHAFT_WIDTH / 2.0),
        }
    }
    batches.append(|color| barb_stroke(color, 0.0), items);
}

/// The stroke of the barbs in `color`, `width` pt wide.
fn barb_stroke(color: Rgba, width: f64) -> StrokeStyle {
    StrokeStyle {
        color,
        width: width as f32,
        dash: Vec::new(),
        cap: LineCap::Round,
        join: LineJoin::Round,
    }
}

/// Appends the barbs of `mark`, on a staff `staff` pt long, in the `fill` (see the module
/// documentation).
pub(super) fn barb_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &BarbsMark,
    (fill, staff): (FieldFill<'_>, f64),
    items: &mut Vec<Item>,
) {
    let field = mark.field();
    let flip = mark.options().flip;
    let width = staff * barb::STROKE;
    let xs = axis_shared(&resolved.x, field.x_values());
    let ys = axis_shared(&resolved.y, field.y_values());
    let bounds = culling_bounds(context, barb::furthest_reach(staff) + width);
    let mut batches = Batches::new(context.theme, fill);
    for sample in field.samples() {
        let Some(color) = fill.at(sample.index) else {
            continue;
        };
        let position = AxisPoint::new(xs[sample.column], ys[sample.row]);
        if !bounds.contains(position) {
            continue;
        }
        let at = context.maps.to_plot(position);
        let (stroked, filled) = batches.of(color);
        match Barb::new(sample.u, sample.v, staff, flip) {
            Barb::Calm { radius } => push_circle(stroked, at, radius),
            Barb::Wind { lines, flags } => {
                for [start, end] in lines {
                    stroked.move_to(offset(at, start));
                    stroked.line_to(offset(at, end));
                }
                for flag in flags {
                    push_polygon(filled, at, &flag);
                }
            }
        }
    }
    batches.append(|color| barb_stroke(color, width), items);
}

/// The arrow of a key, `length` pt long, its tail at `(x, y)` of the layout, pointing right.
pub(super) fn key_arrow(length: f64, x: f64, y: f64) -> Path {
    let mut path = Path::new();
    let outline = Outline::new(length, [1.0, 0.0], Pivot::Tail);
    push_polygon(&mut path, PlotPoint { x, y }, outline.corners());
    path
}

/// The typical spacing of the present samples of `field` on `maps`, in points: on a grid the
/// smaller of the median column and row spacing, for points the median distance to the nearest
/// neighbour. `None` for a field with no spacing.
fn typical_spacing(maps: &AxisMaps, resolved: &Resolved, field: &VectorField) -> Option<f64> {
    match field.form() {
        FieldForm::Grid => {
            let gap = |axis, values, map: &super::mapping::AxisMap| {
                let values = axis_shared(axis, values);
                spacing::median_gap(&values).map(|gap| map.points(gap))
            };
            let columns = gap(&resolved.x, field.kept_x(), &maps.x);
            let rows = gap(&resolved.y, field.kept_y(), &maps.y);
            match (columns, rows) {
                (Some(columns), Some(rows)) => Some(columns.min(rows)),
                (one, other) => one.or(other),
            }
        }
        FieldForm::Points => {
            let xs = axis_shared(&resolved.x, field.x_values());
            let ys = axis_shared(&resolved.y, field.y_values());
            let points = field
                .samples()
                .map(|sample| {
                    let position = AxisPoint::new(xs[sample.column], ys[sample.row]);
                    let at = maps.to_plot(position);
                    [at.x, at.y]
                })
                .filter(|[x, y]| x.is_finite() && y.is_finite())
                .collect();
            spacing::median_nearest(points)
        }
    }
}

/// The length, in points, of the key arrow of `mark` in `frame`; 0 without a key. Any key past
/// [`MAX_ARROW_LENGTH`] is refused: the bound keeps its layout finite until then.
pub(crate) fn key_length(mark: &QuiverMark, frame: &ArrowFrame) -> f64 {
    let key = mark.options().key.as_ref();
    let length = key.map_or(0.0, |key| frame.length_of(key.value));
    length.min(2.0 * MAX_ARROW_LENGTH)
}

/// The points of one data unit along x and along y on the axes `resolved` and the `plot`, what
/// the direction of an arrow along the data reads. They are kept apart: their ratio can leave
/// the range of `f64`.
pub(crate) fn data_scales(resolved: &Resolved, plot: Area) -> [f64; 2] {
    let maps = AxisMaps::new(&resolved.x, &resolved.y, plot);
    [maps.x.points(1.0), maps.y.points(1.0)]
}

/// The marks whose reach a plot does not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Spill {
    /// Arrows.
    Arrows,
    /// Wind barbs.
    Barbs,
}

/// How far a shape may lie past the plot and still count as held, in points: well below anything
/// visible, and above the rounding of the frames.
const HELD_SLACK: f64 = 1e-3;

/// What reaches past the edges of `plot` along the axes that `automatic` names, x then y, on the
/// axes `resolved`: an arrow or barb of `marks` as it is drawn, the arrows in the `frames` of the
/// quivers, in order. Automatic limits hold whole shapes, so a layout where this finds one is not
/// a layout to draw; explicit limits clip them, so those edges do not count.
pub(crate) fn spill(
    marks: &[ResolvedMark<'_>],
    frames: &[ArrowFrame],
    resolved: &Resolved,
    plot: Area,
    automatic: [bool; 2],
) -> Option<Spill> {
    let maps = AxisMaps::new(&resolved.x, &resolved.y, plot);
    let spills = |field: &VectorField, fill: FieldFill<'_>, reach: &dyn Fn(f64, f64) -> Reach| {
        let xs = axis_shared(&resolved.x, field.x_values());
        let ys = axis_shared(&resolved.y, field.y_values());
        field.samples().any(|sample| {
            if fill.at(sample.index).is_none() {
                return false;
            }
            let at = maps.to_plot(AxisPoint::new(xs[sample.column], ys[sample.row]));
            let reach = reach(sample.u, sample.v);
            let along_x = at.x - reach.left < plot.x - HELD_SLACK
                || at.x + reach.right > plot.right() + HELD_SLACK;
            let along_y = at.y - reach.up < plot.y - HELD_SLACK
                || at.y + reach.down > plot.bottom() + HELD_SLACK;
            (automatic[0] && along_x) || (automatic[1] && along_y)
        })
    };
    let scale = resolved.color.as_ref();
    let mut frames = frames.iter();
    marks.iter().find_map(|mark| match mark {
        ResolvedMark::Quiver {
            mark,
            color,
            mapped,
            ..
        } => {
            let frame = frames.next()?;
            let pivot = mark.options().pivot;
            let filled = self::fill(scale, *color, mapped.as_ref())?;
            spills(mark.field(), filled, &|u, v| frame.reach(u, v, pivot)).then_some(Spill::Arrows)
        }
        ResolvedMark::Barbs {
            mark,
            color,
            mapped,
            staff,
            ..
        } => {
            let flip = mark.options().flip;
            let width = staff * barb::STROKE;
            let filled = self::fill(scale, *color, mapped.as_ref())?;
            spills(mark.field(), filled, &|u, v| {
                Barb::new(u, v, *staff, flip).reach(width)
            })
            .then_some(Spill::Barbs)
        }
        _ => None,
    })
}

/// The frame of each quiver among `marks`, in order, measured on the axes `resolved` and the
/// `plot` of one layout: what the next layout should hold and draw. All automatic quivers
/// share one scale, which makes the longest of their arrows as long as the smallest of their
/// spacings, or as long as an automatic barb staff when they have no spacing, and without any
/// length to scale that scale has no length. The longest arrow is at most [`MOST_OF_THE_PLOT`] of
/// the shorter side of the plot: a row of a few samples can have a spacing longer than the plot
/// is high, which no limits hold.
///
/// On axes with `equal_aspect` a data unit is equally long along both axes, so
/// [`Direction::Data`] takes the screen scales exactly and both directions draw the same bits.
pub(crate) fn measured_frames(
    theme: &Theme,
    marks: &[ResolvedMark<'_>],
    resolved: &Resolved,
    plot: Area,
    equal_aspect: bool,
) -> Vec<ArrowFrame> {
    let maps = AxisMaps::new(&resolved.x, &resolved.y, plot);
    let quivers = marks.iter().filter_map(|mark| match mark {
        ResolvedMark::Quiver { mark, .. } => Some(*mark),
        _ => None,
    });
    let automatic = quivers
        .clone()
        .filter(|mark| mark.options().length == ArrowLength::Auto);
    let longest = automatic
        .clone()
        .map(|mark| mark.field().longest())
        .fold(0.0, f64::max);
    let length = if longest > 0.0 {
        let length = automatic
            .filter_map(|mark| typical_spacing(&maps, resolved, mark.field()))
            .reduce(f64::min)
            .unwrap_or_else(|| barb::STAFF * f64::from(theme.font_scale()));
        length.min(MOST_OF_THE_PLOT * plot.width.min(plot.height))
    } else {
        0.0
    };
    quivers
        .map(|mark| {
            let options = mark.options();
            let (length, magnitude) = match options.length {
                ArrowLength::Auto if longest > 0.0 => (length, longest),
                ArrowLength::Auto => (0.0, 1.0),
                ArrowLength::PtPerUnit(scale) => (scale, 1.0),
            };
            let mut frame = ArrowFrame {
                length,
                magnitude,
                scales: match options.direction {
                    Direction::Data if !equal_aspect => data_scales(resolved, plot),
                    Direction::Data | Direction::Screen => SCREEN_SCALES,
                },
                key_length: 0.0,
            };
            frame.key_length = key_length(mark, &frame);
            frame
        })
        .collect()
}
