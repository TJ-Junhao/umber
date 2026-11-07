//! The values written next to scatter points ([`PointLabels`](crate::PointLabels)): measured
//! once per render, before layout ([`ScatterLabels::measure`]); on a fixed side, and on the
//! sides that automatic labels took in the previous pass, they widen automatic limits so that
//! they fit ([`ScatterLabels::end_reaches`], see [`crate::bubble_fit`]); they are placed over
//! the marks once the marks are drawn ([`items`]), and the limits are fitted again until the
//! placement settles ([`fit_then_place`]).
//!
//! **Which points.** A point gets a label when it is drawn: both coordinates present (a finite
//! number, a category, a time) and a fill (a color-mapped value that maps to a color), within
//! the explicit limits of either axis, with its centre inside the plot. A point outside explicit
//! limits gets no label and adds no reach to the limit fits. The label writes the point's coordinate on the value axis, x on a
//! category y axis and y otherwise, in the mark's format, else the value axis's tick format:
//! numbers as bar labels write them ([`bar_labels::value_texts`]), times as [`time_texts`]
//! writes them.
//!
//! **Style.** Every label is set in the tick label style at the size of its mark's labels, in
//! the mark's resolved color at full opacity: the series color, each group's own with `by=`,
//! and the theme's text color for a color-mapped mark, whose per-point colors can lie close to
//! the background. A series color moves in lightness only as far as it needs to reach a
//! contrast of 4.5:1 with the background ([`readable_text`]); the markers keep it.
//! Its box is its advance width by its ascent and descent.
//!
//! **Fixed sides.** [`Place::Above`] and [`Place::Below`] put the box [`GAP`] beyond the
//! marker's drawn edge (its shape's extent plus its outline, [`marker_reach`]), centred
//! horizontally on the point; [`Place::Left`] and [`Place::Right`] put it there centred
//! vertically. A fixed-side label never moves: one that leaves the plot fails the render.
//!
//! **Automatic place.** [`Place::Auto`] tries above, right, below and left, in that order, and
//! takes the first box that lies inside the plot and is clear: the box widened by [`PADDING`]
//! meets none of the [`Obstacles`] of the panel, which are the marks drawn in it
//! ([`obstacle_marks`]) and every label placed before. When no box inside the plot is clear,
//! the label may leave the plot across automatic limits: it takes the first side, in the same
//! order, whose box is clear and leaves the plot only along axes whose limits are automatic
//! (not explicit, not categories). A label with no such side fails the render. Labels are
//! placed in input order, mark by mark in draw order and point by point within a mark, after
//! the bar labels, so the same figure always places them the same way.
//!
//! **Limits.** The first pass fits the limits as the plot has them without automatic labels,
//! and places every label against them. When every automatic label lies inside the plot, that
//! placement is final, and the limits are those of the plot without labels. Otherwise each
//! automatic label reaches past its point on the side it took, as a fixed-side label does, and
//! the next pass fits the limits again from the plot without automatic labels plus these
//! reaches, never from the limits of an earlier pass, so that they grow just enough to hold
//! them. Growing the limits moves every point, so the labels are placed again; each label then
//! tries the side it took in the pass before first, and the other sides in the usual order, so
//! that a label the limits now hold keeps its side rather than move to a side earlier in the
//! order that the moved points have just cleared. The placement is final once every label lies
//! inside the plot on the side it took in the pass before.
//! A label still outside on that side means the limits cannot hold it, which fails the render;
//! so does a placement that has not settled after [`MAX_PASSES`] passes.
//!
//! **Column.** [`Place::Column`] writes the values outside the plot instead, in the value
//! column of the axes, which the layout reserves before the marks are drawn
//! ([`crate::layout::ColumnInput`]); [`mod@column`] describes how they are written there. They
//! reach nothing past their points, so they never change limits, and they are no obstacles
//! here.
//!
//! Text written by `text()` and `annotate()` is drawn after the labels and is not an obstacle.
//! Heatmap cell labels need two category axes, on which no scatter is drawn, so they never
//! meet point labels. The layers of a twin-axes panel share one plot, so they share its
//! obstacles: the marks and bar labels of every layer, and the point labels placed so far.
//! The layers place their labels in layer order, the left axes first, each with its own axes'
//! mapping and the limit fits of its own axes.

use super::bar_labels::{self, contains};
use super::chrome::text;
use super::contrast::readable_text;
use super::mapping::{AxisPoint, axis_values};
use super::obstacles::{Bounds, Obstacles};
use super::{DrawnMarks, has_fill, mark_context, marker_outline, marker_reach, solid};
use crate::bubble_fit::{EndReach, ReachFrom};
use crate::color_scale::ResolvedColorScale;
use crate::error::quoted;
use crate::layout::{Area, ColumnInput, Label, Layout, value_text};
use crate::model::{Axes, AxisData, values_on_x};
use crate::resolve::{Resolved, ResolvedMark, ValueAxis};
use crate::scene::Item;
use crate::text::SceneFonts;
use crate::ticks::{Tick, TickValues};
use crate::time::{explicit_labels, iso_label, label_step};
use crate::{Error, Instant, NumberStyle, Place, Result, Rgba, Theme, TickFormat};

mod column;
pub(crate) use column::column_items;

/// The gap between a marker's drawn edge and the box of its label, in points.
pub(crate) const GAP: f64 = bar_labels::END_GAP;
/// How far, in points, the box of an automatic label must stay from every obstacle.
pub(crate) const PADDING: f64 = 1.0;
/// The most passes of [`fit_then_place`]: each fits the limits and places the labels once.
pub(crate) const MAX_PASSES: usize = 4;

/// The hint of an automatic label with no clear position.
const CLEAR_HINT: &str = "use place=Place.ABOVE (or another side) to fix the side, a larger figure, or a smaller label size.";
/// The hint of a label that leaves the plot under automatic limits.
const ROOM_HINT: &str =
    "enlarge the figure, use place=Place.COLUMN or another place, or a smaller label size.";
/// The hint of a fixed-side label that leaves the plot under explicit limits.
const LIMITS_HINT: &str = "widen xlim or ylim, or let the limits be automatic.";

/// A side of its point that a label takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// Above the point, centred horizontally.
    Above,
    /// Right of the point, centred vertically.
    Right,
    /// Below the point, centred horizontally.
    Below,
    /// Left of the point, centred vertically.
    Left,
}

impl Side {
    /// The sides an automatic label tries, in order.
    const AUTO_ORDER: [Side; 4] = [Side::Above, Side::Right, Side::Below, Side::Left];
}

/// Where the labels of one mark go: on a side of their points in the plot, fixed or chosen
/// automatically, or in the value column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placing {
    /// In the plot, on the fixed side; `None` for a side chosen automatically.
    InPlot(Option<Side>),
    /// In the value column.
    Column,
}

impl Placing {
    /// Where `place` puts the labels.
    fn of(place: Place) -> Placing {
        match place {
            Place::Auto => Placing::InPlot(None),
            Place::Above => Placing::InPlot(Some(Side::Above)),
            Place::Right => Placing::InPlot(Some(Side::Right)),
            Place::Below => Placing::InPlot(Some(Side::Below)),
            Place::Left => Placing::InPlot(Some(Side::Left)),
            Place::Column => Placing::Column,
        }
    }
}

/// The side each automatic label of one axes took in one pass: for each annotated mark, in
/// mark order, the side of each of its labels, in data order; `None` for a label on a fixed
/// side and for one that was not placed.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Sides(Vec<Vec<Option<Side>>>);

impl Sides {
    /// The side of the label `point` of the annotated mark `mark`.
    fn side(&self, mark: usize, point: usize) -> Option<Side> {
        self.0
            .get(mark)
            .and_then(|sides| sides.get(point))
            .copied()
            .flatten()
    }

    /// The first label, as the annotated mark's index and the point's, whose side differs
    /// between `self` and `other`.
    fn first_difference(&self, other: &Sides) -> Option<(usize, usize)> {
        let marks = self.0.len().max(other.0.len());
        (0..marks).find_map(|mark| {
            let points = [self, other]
                .map(|sides| sides.0.get(mark).map_or(0, Vec::len))
                .into_iter()
                .max()
                .unwrap_or(0);
            (0..points)
                .find(|&point| self.side(mark, point) != other.side(mark, point))
                .map(|point| (mark, point))
        })
    }
}

/// The explicit limits of one axis, as the points to label are chosen within them.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Limits {
    /// Automatic limits, which grow to hold every point.
    Automatic,
    /// Explicit numeric limits, the lower first.
    Numbers(f64, f64),
    /// Explicit time limits, the earlier first.
    Times(Instant, Instant),
}

impl Limits {
    /// The limits of an axis with the explicit numeric limits `numbers` or time limits `times`.
    fn new(numbers: Option<(f64, f64)>, times: Option<(Instant, Instant)>) -> Limits {
        match (numbers, times) {
            (Some((a, b)), _) => Limits::Numbers(a.min(b), a.max(b)),
            (None, Some((a, b))) => Limits::Times(a.min(b), a.max(b)),
            (None, None) => Limits::Automatic,
        }
    }

    /// Whether the coordinate `at`, as the limit fits read it, lies within the limits; a
    /// category (`None`) always does.
    fn holds(self, at: Option<ReachFrom>) -> bool {
        match (self, at) {
            (Limits::Numbers(low, high), Some(ReachFrom::Number(v))) => (low..=high).contains(&v),
            (Limits::Times(low, high), Some(ReachFrom::Time(t))) => (low..=high).contains(&t),
            _ => true,
        }
    }
}

/// One point's label, measured.
#[derive(Debug, Clone)]
struct PointLabel {
    /// The point's index in its mark's data.
    index: usize,
    /// The point's x and y coordinates as the limit fits read them; `None` on a category axis,
    /// whose limits never grow.
    at: [Option<ReachFrom>; 2],
    /// The label, unplaced.
    label: Label,
}

impl PointLabel {
    /// The width and height of the label's box: its advance width by its ascent and descent.
    fn size(&self) -> (f64, f64) {
        let layout = &self.label.layout;
        (
            self.label.width(),
            f64::from(layout.ascent() + layout.descent()),
        )
    }
}

/// The measured labels of one annotated scatter mark.
#[derive(Debug, Clone)]
struct MarkLabels {
    /// The mark's index among the axes' marks.
    mark: usize,
    /// How messages name the mark: `the series "Long training"`, or `scatter 2` (the second
    /// scatter of the axes) for a mark without a label.
    name: String,
    /// The fixed side of its labels; `None` for a side chosen automatically.
    side: Option<Side>,
    /// The color of its labels.
    color: Rgba,
    /// How far its markers reach from their centres along x and along y, outline included,
    /// in points.
    reach: (f64, f64),
    /// The labels of its drawn points, in data order.
    points: Vec<PointLabel>,
}

/// The measured value labels of the annotated scatter marks of one axes.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScatterLabels {
    /// One record per annotated mark whose labels go in the plot, in mark order.
    marks: Vec<MarkLabels>,
    /// One record per annotated mark whose labels go in the value column and that labels a
    /// point, in mark order: one sub-column each.
    columns: Vec<column::ColumnValues>,
    /// The value column as the layout reserves it; `None` when no value goes there and the
    /// axes set no column title.
    column: Option<ColumnInput>,
    /// Whether the x and the y axis have explicit limits: an automatic label never leaves the
    /// plot across them, and the hint of a fixed-side label that leaves the plot names them.
    explicit: [bool; 2],
    /// The sides the automatic labels took in the previous pass, which the limit fits make
    /// room for; `None` in the first pass.
    held: Option<Sides>,
}

impl ScatterLabels {
    /// Measures the labels of every annotated scatter mark among `marks`, the resolved marks
    /// of `axes`, in `theme`, with the axes' color scale `scale` deciding which color-mapped
    /// points are drawn.
    ///
    /// # Errors
    ///
    /// As [`value_text`], for the first label that cannot be laid out.
    pub(crate) fn measure(
        theme: &Theme,
        lib: &SceneFonts<'_>,
        axes: &Axes,
        marks: &[ResolvedMark<'_>],
        scale: Option<&ResolvedColorScale>,
    ) -> Result<ScatterLabels> {
        let on_x = values_on_x(axes.y_scale());
        let axis_format = if on_x {
            axes.x_tick_format()
        } else {
            axes.y_tick_format()
        };
        let within = [
            Limits::new(axes.x_limits(), axes.x_time_limits()),
            Limits::new(axes.y_limits(), axes.y_time_limits()),
        ];
        let mut measured = Vec::new();
        let mut columns = Vec::new();
        let mut scatters = 0;
        for (index, mark) in marks.iter().enumerate() {
            let ResolvedMark::Scatter {
                mark: data,
                color,
                size,
                mapped,
                ..
            } = mark
            else {
                continue;
            };
            scatters += 1;
            let options = data.options();
            let labels = &options.labels;
            if !labels.annotate {
                continue;
            }
            let drawn: Vec<usize> = (0..data.x().len())
                .filter(|&i| {
                    present(data.x(), i)
                        && present(data.y(), i)
                        && has_fill(theme, scale, *color, mapped.as_ref(), i)
                        && within[0].holds(coordinate(data.x(), i))
                        && within[1].holds(coordinate(data.y(), i))
                })
                .collect();
            let format = labels.format.as_ref().unwrap_or(axis_format);
            let values = if on_x { data.x() } else { data.y() };
            let texts = match values {
                AxisData::Numbers(numbers) => {
                    let picked: Vec<f64> = drawn.iter().map(|&i| numbers[i]).collect();
                    bar_labels::value_texts(&picked, format)
                }
                AxisData::Times(times) => {
                    let picked: Vec<Instant> = drawn.iter().filter_map(|&i| times[i]).collect();
                    time_texts(&picked, format)
                }
                // The model refuses a scatter on two category axes.
                AxisData::Categories(_) => Vec::new(),
            };
            let points = drawn
                .iter()
                .zip(texts)
                .map(|(&i, content)| {
                    Ok(PointLabel {
                        index: i,
                        at: [coordinate(data.x(), i), coordinate(data.y(), i)],
                        label: value_text(lib, theme, &content, labels.size)?,
                    })
                })
                .collect::<Result<_>>()?;
            let name = if options.label.is_empty() {
                format!("scatter {scatters}")
            } else {
                format!("the series {}", quoted(&options.label))
            };
            match Placing::of(labels.place) {
                Placing::InPlot(side) => measured.push(MarkLabels {
                    mark: index,
                    name,
                    side,
                    color: readable_text(solid(*color), theme),
                    reach: marker_reach(options.marker, *size, marker_outline(mapped.is_some())),
                    points,
                }),
                Placing::Column if points.is_empty() => {}
                Placing::Column => columns.push(column::ColumnValues::measure(
                    (lib, theme),
                    (index, name, readable_text(solid(*color), theme)),
                    points,
                    (on_x, format, labels.size),
                )?),
            }
        }
        let explicit = within.map(|limits| limits != Limits::Automatic);
        let column = column::input(axes, on_x, &columns);
        Ok(ScatterLabels {
            marks: measured,
            columns,
            column,
            explicit,
            held: None,
        })
    }

    /// Whether no mark writes labels in the plot.
    pub(crate) fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    /// The value column as the layout reserves it; `None` without one.
    pub(crate) fn column(&self) -> Option<&ColumnInput> {
        self.column.as_ref()
    }

    /// Holds `sides`, the sides the automatic labels took in the previous pass: the limit fits
    /// make room for them ([`end_reaches`](Self::end_reaches)), and [`items`] compares the
    /// next placement with them.
    pub(crate) fn hold(&mut self, sides: Sides) {
        self.held = Some(sides);
    }

    /// How far each label reaches past its point, for the fits of automatic limits: a label on
    /// a fixed side, and an automatic label on the side it took in the previous pass ([`hold`](
    /// Self::hold)). Along the axis of its side, it reaches the marker's reach, [`GAP`] and
    /// the label's extent, towards that side; along the other axis, half the label's extent on
    /// each side, since the label is centred there. Coordinates on a category axis add no
    /// reach.
    ///
    /// In the first pass automatic labels add none, so the limits are those of the plot without
    /// them.
    pub(crate) fn end_reaches(&self) -> Vec<EndReach> {
        let mut reaches = Vec::new();
        for (m, mark) in self.marks.iter().enumerate() {
            for (p, point) in mark.points.iter().enumerate() {
                let side = mark
                    .side
                    .or_else(|| self.held.as_ref().and_then(|held| held.side(m, p)));
                let Some(side) = side else {
                    continue;
                };
                let (x, y) = reach(side, mark.reach, point.size());
                for (axis, (low, high), at) in [
                    (ValueAxis::X, x, point.at[0]),
                    (ValueAxis::Y, y, point.at[1]),
                ] {
                    if let Some(value) = at {
                        reaches.push(EndReach {
                            axis,
                            value,
                            low,
                            high,
                        });
                    }
                }
            }
        }
        reaches
    }

    /// The sides the automatic label `point` of the annotated mark `mark` tries, in order:
    /// [`Side::AUTO_ORDER`], with the side it took in the previous pass first, since the limits
    /// now make room for that side.
    fn order(&self, mark: usize, point: usize) -> [Side; 4] {
        let mut order = Side::AUTO_ORDER;
        let held = self.held.as_ref().and_then(|held| held.side(mark, point));
        if let Some(at) = order.iter().position(|&side| Some(side) == held) {
            order[..=at].rotate_right(1);
        }
        order
    }

    /// The error for the label `point` of the annotated mark `mark` that has not settled on a
    /// side after the last pass.
    fn unsettled(&self, (mark, point): (usize, usize), context: &'static str) -> Error {
        let (name, text) = self.named(mark, point);
        Error::style(
            context,
            format!(
                "the value label {text} of {name} does not settle on a side: each time the \
                 limits grow to make room for the labels, it takes another."
            ),
        )
        .with_hint(CLEAR_HINT)
    }

    /// The error for the automatic label `point` of the annotated mark `mark` that stays
    /// outside the plot on the side whose reach the limits were fitted to hold.
    fn unfit(&self, (mark, point): (usize, usize), context: &'static str) -> Error {
        let (name, text) = self.named(mark, point);
        Error::style(
            context,
            format!("the value label {text} of {name} does not fit inside the plot."),
        )
        .with_hint(ROOM_HINT)
    }

    /// How messages name the annotated mark `mark` and the label of its `point`, quoted.
    fn named(&self, mark: usize, point: usize) -> (&str, String) {
        let mark = &self.marks[mark];
        (&mark.name, quoted(&mark.points[point].label.text))
    }
}

/// The value labels of one axes, placed by [`items`].
#[derive(Debug, Default)]
pub(crate) struct Placed {
    /// The labels, as one unclipped group; `None` without labels.
    pub item: Option<Item>,
    /// Where the automatic labels went, for [`fit_then_place`].
    pub placement: Placement,
}

/// Where the automatic labels of one axes went in one pass, and whether that is final.
#[derive(Debug, Default)]
pub(crate) struct Placement {
    /// The side each automatic label took.
    sides: Sides,
    /// Why the placement is not final, naming its first label that left the plot or took
    /// another side than the one held for it; `None` when it is final.
    unsettled: Option<Error>,
}

impl Placement {
    /// The placement with `f` applied to the error it would raise.
    pub(crate) fn map_error(self, f: impl FnOnce(Error) -> Error) -> Placement {
        Placement {
            unsettled: self.unsettled.map(f),
            ..self
        }
    }
}

/// The output of the first pass of `pass` whose placements are all final, as the module
/// documentation describes: `pass` fits the limits with the sides it is given held for the
/// automatic labels of each axes ([`ScatterLabels::hold`]), none in the first pass, places
/// every label, and returns its output and one [`Placement`] per axes. Each later pass is given
/// the sides of the pass before, and nothing else carries over.
///
/// # Errors
///
/// As `pass`; then the error of the first placement that is not final after [`MAX_PASSES`]
/// passes.
pub(crate) fn fit_then_place<T>(
    mut pass: impl FnMut(Option<&[Sides]>) -> Result<(T, Vec<Placement>)>,
) -> Result<T> {
    let mut held = None;
    let mut passes = 0;
    loop {
        let (output, placements) = pass(held.as_deref())?;
        passes += 1;
        let mut sides = Vec::with_capacity(placements.len());
        let mut unsettled = None;
        for placement in placements {
            unsettled = unsettled.or(placement.unsettled);
            sides.push(placement.sides);
        }
        let Some(error) = unsettled else {
            return Ok(output);
        };
        if passes == MAX_PASSES {
            return Err(error);
        }
        held = Some(sides);
    }
}

/// The value labels `labels` of the marks `marks`, resolved as `resolved`, laid out as
/// `layout` and drawn as `drawn`, placed after the bar labels whose boxes are `placed`, with
/// where the automatic labels went and whether that is final, as the module documentation
/// describes.
///
/// # Errors
///
/// `Error::Style` under `context` for the first label, in placing order, that cannot be placed:
/// a fixed-side label that leaves the plot, an automatic label with no clear position inside
/// the plot nor across automatic limits, or an automatic label that leaves the plot on the
/// side held for it, whose reach the limits could not hold. The message names the mark and
/// the label.
pub(crate) fn items(
    theme: &Theme,
    resolved: &Resolved,
    layout: &Layout,
    marks: &[ResolvedMark<'_>],
    labels: &ScatterLabels,
    obstacles: &mut Obstacles,
    context: &'static str,
) -> Result<Placed> {
    if labels.is_empty() {
        return Ok(Placed {
            item: None,
            placement: Placement::default(),
        });
    }
    let plot = layout.plot;
    let maps = mark_context(theme, resolved, layout, marks).maps;
    let mut items = Vec::new();
    let mut sides = Vec::with_capacity(labels.marks.len());
    // The first automatic label, in placing order, that leaves the plot.
    let mut outside = None;
    for (m, mark_labels) in labels.marks.iter().enumerate() {
        let mut mark_sides = vec![None; mark_labels.points.len()];
        let Some(ResolvedMark::Scatter { mark, .. }) = marks.get(mark_labels.mark) else {
            sides.push(mark_sides);
            continue;
        };
        let xs = axis_values(&resolved.x, mark.x());
        let ys = axis_values(&resolved.y, mark.y());
        for (p, point) in mark_labels.points.iter().enumerate() {
            let (Some(&x), Some(&y)) = (xs.get(point.index), ys.get(point.index)) else {
                continue;
            };
            let centre = maps.to_plot(AxisPoint::new(x, y));
            let centre = (centre.x, centre.y);
            if !(x.is_finite() && y.is_finite())
                || !contains(plot, [centre.0, centre.1, centre.0, centre.1])
            {
                continue;
            }
            let candidate = |side| candidate(side, centre, mark_labels.reach, point.size());
            let bounds = if let Some(side) = mark_labels.side {
                let bounds = candidate(side);
                if !contains(plot, bounds) {
                    return Err(Error::style(
                        context,
                        format!(
                            "the value label {} of {} does not fit inside the plot.",
                            quoted(&point.label.text),
                            mark_labels.name,
                        ),
                    )
                    .with_hint(if labels.explicit.contains(&true) {
                        LIMITS_HINT
                    } else {
                        ROOM_HINT
                    }));
                }
                bounds
            } else {
                // An axis whose limits grow: automatic, and not a category axis.
                let grows = [0, 1].map(|axis| !labels.explicit[axis] && point.at[axis].is_some());
                let clear = |bounds| !obstacles.hit(padded(bounds));
                let boxes = labels.order(m, p).map(|side| (side, candidate(side)));
                let (side, bounds) = boxes
                    .into_iter()
                    .find(|&(_, bounds)| contains(plot, bounds) && clear(bounds))
                    .or_else(|| {
                        boxes.into_iter().find(|&(_, bounds)| {
                            contains(plot, along(plot, bounds, grows)) && clear(bounds)
                        })
                    })
                    .ok_or_else(|| {
                        Error::style(
                            context,
                            format!(
                                "the value label {} of {} has no clear position: above, right, \
                                 below and left of its point each leave the plot or overlap a \
                                 mark or a label.",
                                quoted(&point.label.text),
                                mark_labels.name,
                            ),
                        )
                        .with_hint(CLEAR_HINT)
                    })?;
                if outside.is_none() && !contains(plot, bounds) {
                    outside = Some((m, p));
                }
                mark_sides[p] = Some(side);
                bounds
            };
            obstacles.add_box(bounds);
            let mut label = point.label.clone();
            label.x = bounds[0];
            label.y = bounds[1] + f64::from(label.layout.ascent());
            items.push(text(&label, mark_labels.color));
        }
        sides.push(mark_sides);
    }
    let sides = Sides(sides);
    let unsettled = match &labels.held {
        None => outside,
        Some(held) if *held == sides => {
            if let Some(label) = outside {
                return Err(labels.unfit(label, context));
            }
            None
        }
        Some(held) => held.first_difference(&sides).or(outside),
    };
    Ok(Placed {
        item: Some(Item::Group { clip: None, items }),
        placement: Placement {
            sides,
            unsettled: unsettled.map(|label| labels.unsettled(label, context)),
        },
    })
}

/// The obstacles of the automatic value labels of the plot `plot`, empty; every layer of the
/// panel adds its own ([`add_obstacles`]) before any label is placed ([`items`]).
pub(crate) fn obstacles(plot: Area) -> Obstacles {
    Obstacles::new(plot)
}

/// Adds to `obstacles` what the automatic value labels of a plot must stay clear of in one of
/// its layers: the marks `marks` drawn as `drawn` ([`obstacle_marks`]) and the boxes `placed`
/// of the bar labels the layer placed.
pub(crate) fn add_obstacles(
    obstacles: &mut Obstacles,
    marks: &[ResolvedMark<'_>],
    (drawn, placed): (&DrawnMarks, &[[f64; 4]]),
) {
    for (index, mark) in marks.iter().enumerate() {
        if obstacle_marks(mark) {
            obstacles.add_items(drawn.of_mark(index));
        }
    }
    for &bounds in placed {
        obstacles.add_box(bounds);
    }
}

/// Whether the drawn items of `mark` are obstacles of automatic labels: every data mark, but
/// not the cells and contours of fields nor reference lines and spans, which cover or cross
/// the whole plot.
fn obstacle_marks(mark: &ResolvedMark<'_>) -> bool {
    match mark {
        ResolvedMark::Line { .. }
        | ResolvedMark::Area { .. }
        | ResolvedMark::Scatter { .. }
        | ResolvedMark::Bubbles { .. }
        | ResolvedMark::Stem { .. }
        | ResolvedMark::Bar { .. }
        | ResolvedMark::Grouped { .. }
        | ResolvedMark::Stacked { .. }
        | ResolvedMark::StackedArea { .. }
        | ResolvedMark::Hist { .. }
        | ResolvedMark::Step { .. }
        | ResolvedMark::Band { .. }
        | ResolvedMark::ErrorBars { .. }
        | ResolvedMark::Box { .. }
        | ResolvedMark::Violin { .. }
        | ResolvedMark::Events { .. }
        | ResolvedMark::Points { .. }
        | ResolvedMark::Ecdf { .. }
        | ResolvedMark::Kde { .. } => true,
        ResolvedMark::Mesh { .. }
        | ResolvedMark::Hist2d { .. }
        | ResolvedMark::Hexbin { .. }
        | ResolvedMark::Heatmap { .. }
        | ResolvedMark::Contour { .. }
        | ResolvedMark::HLine { .. }
        | ResolvedMark::VLine { .. }
        | ResolvedMark::HSpan { .. }
        | ResolvedMark::VSpan { .. } => false,
    }
}

/// The box (left, top, right, bottom) of a label of `size` on the side `side` of a point at
/// `centre` whose marker reaches `reach` along x and y, as the module documentation describes.
fn candidate(
    side: Side,
    (x, y): (f64, f64),
    (reach_x, reach_y): (f64, f64),
    (width, height): (f64, f64),
) -> Bounds {
    match side {
        Side::Above => {
            let bottom = y - reach_y - GAP;
            [x - width / 2.0, bottom - height, x + width / 2.0, bottom]
        }
        Side::Below => {
            let top = y + reach_y + GAP;
            [x - width / 2.0, top, x + width / 2.0, top + height]
        }
        Side::Right => {
            let left = x + reach_x + GAP;
            [left, y - height / 2.0, left + width, y + height / 2.0]
        }
        Side::Left => {
            let right = x - reach_x - GAP;
            [right - width, y - height / 2.0, right, y + height / 2.0]
        }
    }
}

/// How far the box [`candidate`] gives for the same label reaches past its point, for the
/// limit fits: towards the low and the high end of x, then of y, in points. The high end of
/// y is up, above the point.
fn reach(
    side: Side,
    (reach_x, reach_y): (f64, f64),
    (width, height): (f64, f64),
) -> ((f64, f64), (f64, f64)) {
    match side {
        Side::Above => ((width / 2.0, width / 2.0), (0.0, reach_y + GAP + height)),
        Side::Below => ((width / 2.0, width / 2.0), (reach_y + GAP + height, 0.0)),
        Side::Left => ((reach_x + GAP + width, 0.0), (height / 2.0, height / 2.0)),
        Side::Right => ((0.0, reach_x + GAP + width), (height / 2.0, height / 2.0)),
    }
}

/// The box `bounds` with its extent along each axis whose limits grow (`grows`, x then y)
/// replaced by the plot's: inside the plot exactly when `bounds` leaves the plot only along
/// those axes.
fn along(plot: Area, [left, top, right, bottom]: Bounds, [x, y]: [bool; 2]) -> Bounds {
    let (left, right) = if x {
        (plot.x, plot.right())
    } else {
        (left, right)
    };
    let (top, bottom) = if y {
        (plot.y, plot.bottom())
    } else {
        (top, bottom)
    };
    [left, top, right, bottom]
}

/// The box `bounds` widened by [`PADDING`] on every side.
fn padded([left, top, right, bottom]: Bounds) -> Bounds {
    [
        left - PADDING,
        top - PADDING,
        right + PADDING,
        bottom + PADDING,
    ]
}

/// Whether the coordinate at `i` of `data` is present: a finite number, a category, a time.
fn present(data: &AxisData, i: usize) -> bool {
    match data {
        AxisData::Numbers(values) => values.get(i).is_some_and(|v| v.is_finite()),
        AxisData::Categories(names) => i < names.len(),
        AxisData::Times(times) => times.get(i).is_some_and(Option::is_some),
    }
}

/// The present coordinate at `i` of `data` as the limit fits read it; `None` for a category.
fn coordinate(data: &AxisData, i: usize) -> Option<ReachFrom> {
    match data {
        AxisData::Numbers(values) => values.get(i).copied().map(ReachFrom::Number),
        AxisData::Times(times) => times.get(i).copied().flatten().map(ReachFrom::Time),
        AxisData::Categories(_) => None,
    }
}

/// The labels of the times `times` of one mark in `format`, with its prefix and suffix: each
/// written down to the finest unit any of them needs ([`label_step`]), as the axis writes a
/// lone explicit tick for the automatic format (`2026`, `Mar 2026`, `Mar 3`, `Mar 3 06:30`),
/// or in ISO 8601 for [`TickFormat::iso`] (`2026-03-03T06:30`).
fn time_texts(times: &[Instant], format: &TickFormat) -> Vec<String> {
    let step = label_step(times);
    let ticks = times
        .iter()
        .map(|&t| {
            let label = match format.style() {
                NumberStyle::Iso => iso_label(t, step),
                // The model refuses number formats for times.
                NumberStyle::Auto
                | NumberStyle::Fixed(_)
                | NumberStyle::Percent(_)
                | NumberStyle::Scientific => explicit_labels(&[t], step).concat(),
            };
            Tick { value: 0.0, label }
        })
        .collect();
    format
        .relabel(ticks, TickValues::Exact)
        .into_iter()
        .map(|tick| tick.label)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(civil: (i64, u8, u8, u8, u8)) -> Instant {
        let (year, month, day, hour, minute) = civil;
        let days = crate::time::days_from_civil(year, i64::from(month), i64::from(day));
        Instant::from_micros(((days * 24 + i64::from(hour)) * 60 + i64::from(minute)) * 60_000_000)
            .unwrap()
    }

    #[test]
    fn fixed_sides_sit_beyond_the_marker_edge() {
        let (centre, reach, size) = ((100.0, 50.0), (4.0, 5.0), (20.0, 10.0));
        assert_eq!(
            candidate(Side::Above, centre, reach, size),
            [90.0, 32.0, 110.0, 42.0]
        );
        assert_eq!(
            candidate(Side::Below, centre, reach, size),
            [90.0, 58.0, 110.0, 68.0]
        );
        assert_eq!(
            candidate(Side::Right, centre, reach, size),
            [107.0, 45.0, 127.0, 55.0]
        );
        assert_eq!(
            candidate(Side::Left, centre, reach, size),
            [73.0, 45.0, 93.0, 55.0]
        );
    }

    #[test]
    fn a_label_leaves_the_plot_only_along_axes_that_grow() {
        let plot = Area {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        };
        // Above the top edge: only a growing y axis holds it.
        let above = [40.0, 12.0, 60.0, 18.0];
        assert!(!contains(plot, along(plot, above, [true, false])));
        assert!(contains(plot, along(plot, above, [false, true])));
        // Past the top right corner: both axes must grow.
        let corner = [105.0, 12.0, 125.0, 18.0];
        assert!(!contains(plot, along(plot, corner, [false, true])));
        assert!(contains(plot, along(plot, corner, [true, true])));
    }

    #[test]
    fn reaches_match_the_boxes() {
        let (centre, marker, size) = ((100.0, 50.0), (4.0, 5.0), (20.0, 10.0));
        for side in Side::AUTO_ORDER {
            let [left, top, right, bottom] = candidate(side, centre, marker, size);
            // Screen y grows downwards: the high end of y is the top of the box. A box on one
            // side of the point reaches nothing towards the other.
            let clip = |v: f64| v.max(0.0);
            let ((x_low, x_high), (y_low, y_high)) = reach(side, marker, size);
            assert_eq!(
                [x_low, x_high, y_low, y_high],
                [
                    clip(centre.0 - left),
                    clip(right - centre.0),
                    clip(bottom - centre.1),
                    clip(centre.1 - top)
                ],
                "{side:?}"
            );
        }
    }

    fn placement(sides: &[Option<Side>], unsettled: bool) -> Placement {
        Placement {
            sides: Sides(vec![sides.to_vec()]),
            unsettled: unsettled.then(|| Error::style("Figure.to_scene()", "unsettled".to_owned())),
        }
    }

    #[test]
    fn the_passes_stop_once_placements_settle() {
        // Final in the first pass: one pass, with nothing held.
        let mut held = Vec::new();
        let output = fit_then_place(|sides| {
            held.push(sides.map(<[Sides]>::to_vec));
            Ok((held.len(), vec![placement(&[Some(Side::Above)], false)]))
        })
        .unwrap();
        assert_eq!((output, held), (1, vec![None]));
        // Not final in the first pass: the second is given the first's sides, and only those.
        let mut held = Vec::new();
        let output = fit_then_place(|sides| {
            held.push(sides.map(<[Sides]>::to_vec));
            let unsettled = held.len() == 1;
            Ok((held.len(), vec![placement(&[Some(Side::Right)], unsettled)]))
        })
        .unwrap();
        assert_eq!(output, 2);
        assert_eq!(
            held,
            vec![None, Some(vec![Sides(vec![vec![Some(Side::Right)]])])]
        );
    }

    #[test]
    fn placements_that_never_settle_fail_after_the_last_pass() {
        let mut passes = 0;
        let err = fit_then_place(|_| {
            passes += 1;
            let side = if passes % 2 == 0 {
                Side::Above
            } else {
                Side::Below
            };
            Ok(((), vec![placement(&[Some(side)], true)]))
        })
        .unwrap_err();
        assert_eq!(passes, MAX_PASSES);
        assert!(matches!(err, Error::Style { .. }), "{err}");
        // An error of a pass ends the passes at once.
        let mut passes = 0;
        let err = fit_then_place::<()>(|_| {
            passes += 1;
            Err(Error::style("Figure.to_scene()", "failed".to_owned()))
        })
        .unwrap_err();
        assert_eq!(
            (passes, err.to_string()),
            (1, "Figure.to_scene(): failed".to_owned())
        );
    }

    #[test]
    fn a_label_tries_its_held_side_first() {
        let mut labels = ScatterLabels::default();
        assert_eq!(labels.order(0, 0), Side::AUTO_ORDER);
        labels.hold(Sides(vec![vec![Some(Side::Below), None]]));
        assert_eq!(
            labels.order(0, 0),
            [Side::Below, Side::Above, Side::Right, Side::Left]
        );
        assert_eq!(labels.order(0, 1), Side::AUTO_ORDER);
    }

    #[test]
    fn the_first_difference_names_the_first_label_that_moved() {
        let a = Sides(vec![vec![Some(Side::Above), None], vec![Some(Side::Right)]]);
        let b = Sides(vec![vec![Some(Side::Above), None], vec![Some(Side::Left)]]);
        assert_eq!(a.first_difference(&a), None);
        assert_eq!(a.first_difference(&b), Some((1, 0)));
        assert_eq!(Sides::default().first_difference(&a), Some((0, 0)));
    }

    #[test]
    fn times_are_written_to_the_finest_unit_they_need() {
        let days = [at((2026, 3, 1, 0, 0)), at((2026, 3, 15, 0, 0))];
        assert_eq!(time_texts(&days, &TickFormat::AUTO), ["Mar 1", "Mar 15"]);
        assert_eq!(
            time_texts(&days, &TickFormat::iso()),
            ["2026-03-01", "2026-03-15"]
        );
        let months = [at((2026, 3, 1, 0, 0)), at((2026, 4, 1, 0, 0))];
        assert_eq!(
            time_texts(&months, &TickFormat::AUTO),
            ["Mar 2026", "Apr 2026"]
        );
        let clock = [at((2026, 3, 3, 6, 30))];
        let local = TickFormat::AUTO.affix("", " local").unwrap();
        assert_eq!(time_texts(&clock, &local), ["Mar 3 06:30 local"]);
    }
}
