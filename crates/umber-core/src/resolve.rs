//! Resolve: data extents to scales to ticks, computed at render time.
//!
//! Auto limits: a continuous axis widens its data extent to nice limits, whose ticks are the
//! tick set that produced them, then pads that nice domain by 5 % of its span on each side (in
//! log space on a log axis), so marks never sit on the plot edge and the outermost ticks sit
//! just inside it. The ticks are not changed by the padding. The exception is the x axis of an
//! axes whose marks are all lines, which keeps the exact data extent. Explicit limits replace
//! the auto range as given and are never padded. Areas count as lines here, since each is drawn
//! along its line. A linear y axis that holds bars, a histogram or an area always includes 0, and a
//! side of its nice domain that ends at 0 is not padded, so bars start on the plot edge. A category
//! axis lists the marks' categories in order of first appearance.
//! The baseline sits at 0 when 0 lies inside a linear y domain, and otherwise at the y axis's
//! lower limit before padding. Auto y limits keep that lower limit strictly below the data: when
//! the smallest y value lands on (or within 5 % of the span above) the lowest nice tick, the
//! limit moves down one tick step.
//! Explicit ticks replace the automatic ones. Without explicit limits the domain then encloses
//! the data and the ticks as they are, with no rounding to nice limits and no lowering of the
//! lowest tick, and is padded as auto limits are; with explicit limits the ticks outside them
//! are dropped. The axis's tick format labels every tick except those given with their own
//! labels.
//! Tick positions are normalized to `[0, 1]` along the axis, so layout and drawing never touch
//! data values again.
//!
//! Marks are resolved here too, once per render: each mark becomes a [`ResolvedMark`], a typed
//! record that carries the mark's data, its [`ColorSpec`] resolved against the theme's palette
//! and, for lines, areas and scatters, its [`SizeSpec`](crate::SizeSpec) resolved against the
//! theme's line width or marker size. Axes are resolved from these records, once per layout
//! pass, and drawing and the legend read them instead of the marks.

use std::collections::BTreeMap;

use crate::binning::Histogram;
use crate::error::quoted;
use crate::layout::TickRoom;
use crate::legend::{LegendEntry, Swatch};
use crate::model::{
    AreaMark, Axes, AxisData, AxisTicks, BarGroupMark, BarMark, ExplicitTicks, HistMark, LineMark,
    Mark, ScatterMark, SeriesView, StepMark,
};
use crate::model::{AxisValue, BandMark, Figure, HLineMark, RuleOptions, SpanMark, VLineMark};
use crate::model::{Edge, holds};
use crate::model::{ErrorBarMark, ErrorBarOrientation};
use crate::scale::{Finish, Padding, TickJudge, is_single_value, log_offset};
use crate::stacking::Stacks;
use crate::tick_format::TickFormat;
use crate::ticks::{AutoTicks, TickValues};
use crate::{
    CategoryScale, ColorSpec, ContinuousScale, Dash, Error, Marker, Result, Rgba, ScaleKind, Theme,
    Tick, Transform, ticks,
};

/// The context of errors that cannot happen here: the model validated every input already.
const CONTEXT: &str = "Figure.to_scene()";

/// The resolved scale of one axis.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AxisScale {
    /// A linear or log axis.
    Continuous(ContinuousScale),
    /// A category axis.
    Category(CategoryScale),
}

/// A position along an axis as a share of its length: 0 is the low end (left or bottom), 1 the
/// high end. Ticks and the baseline lie in `[0, 1]`; the type does not enforce that, so a
/// position can also describe a point outside the axis.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) struct NormalizedPos(f64);

impl NormalizedPos {
    /// The low end of the axis.
    pub const LOW: NormalizedPos = NormalizedPos(0.0);

    /// The position `share` of the axis length from the low end. `share` is stored as given:
    /// neither its range nor its finiteness is checked, so positions extrapolated beyond the
    /// axis ends and NaN pass through unchanged.
    pub fn new(share: f64) -> NormalizedPos {
        NormalizedPos(share)
    }

    /// The share of the axis length from the low end.
    pub fn get(self) -> f64 {
        self.0
    }
}

/// One tick: where it sits along the axis, in `[0, 1]`, and its label.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AxisTick {
    /// Position along the axis.
    pub pos: NormalizedPos,
    /// The label text.
    pub label: String,
}

/// A resolved axis: its scale and its ticks.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedAxis {
    /// The scale.
    pub scale: AxisScale,
    /// The ticks, in increasing position.
    pub ticks: Vec<AxisTick>,
}

impl ResolvedAxis {
    /// Whether this is a category axis.
    pub fn is_category(&self) -> bool {
        matches!(self.scale, AxisScale::Category(_))
    }

    /// The continuous scale, if this is not a category axis.
    #[cfg(test)]
    pub fn continuous(&self) -> Option<&ContinuousScale> {
        match &self.scale {
            AxisScale::Continuous(scale) => Some(scale),
            AxisScale::Category(_) => None,
        }
    }
}

/// A resolved continuous axis: its scale and its ticks. The y axis is always one.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedContinuousAxis {
    /// The scale.
    pub scale: ContinuousScale,
    /// The ticks, in increasing position.
    pub ticks: Vec<AxisTick>,
}

impl ResolvedContinuousAxis {
    /// This axis as a [`ResolvedAxis`], for the x axis, which may also be a category axis.
    fn into_axis(self) -> ResolvedAxis {
        ResolvedAxis {
            scale: AxisScale::Continuous(self.scale),
            ticks: self.ticks,
        }
    }
}

/// Both axes of an [`Axes`], resolved.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Resolved {
    /// The x axis.
    pub x: ResolvedAxis,
    /// The y axis.
    pub y: ResolvedContinuousAxis,
    /// Where the baseline sits along the y axis, in `[0, 1]`: at y = 0 when 0 is inside the y
    /// domain of a linear axis, otherwise at the y axis's lower limit before padding (the
    /// lowest nice tick for auto limits, the explicit low limit, which is the plot bottom, for
    /// explicit ones). Resolve decides it once; layout draws the baseline there, line area
    /// fills fade to it, and bars on a log y axis grow from it.
    pub baseline: NormalizedPos,
}

/// Whether two normalized positions are the same, for matching ticks to the baseline.
pub(crate) fn coincides(a: NormalizedPos, b: NormalizedPos) -> bool {
    (a.0 - b.0).abs() <= 1e-9
}

/// How a continuous axis without explicit limits fits its data. With explicit ticks there is
/// no rounding to nice limits and no moving of the lowest tick: the nice variants only say how
/// the domain is padded (see [`enclosing`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoLimits {
    /// The limits are the data extent.
    Exact,
    /// The data extent is widened to nice limits, which carry a tick at both ends, and the nice
    /// domain is then padded on the given sides (see [`ContinuousScale::padded`]); the ticks
    /// stay as they are.
    Nice(Padding),
    /// As [`Nice`](Self::Nice), with the lowest tick moved strictly below the data before the
    /// padding when the data starts on it (see [`ContinuousScale::below_data`]): the y axis,
    /// whose baseline sits at the lower limit when 0 is not in the domain.
    NiceBelowData(Padding),
}

/// What a linear or log axis fixes before any data is seen: its transform, its explicit limits,
/// if set, and its tick settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ContinuousLimits<'a> {
    /// The transform of the axis.
    pub transform: Transform,
    /// The scale of the explicit limits; `None` to fit the data.
    pub explicit: Option<ContinuousScale>,
    /// The explicit ticks, if set, and the tick label format.
    pub ticks: &'a AxisTicks,
}

/// What an x axis fixes before any data is seen: whether it lists categories, and for a
/// continuous axis its transform, its explicit limits and its tick settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AxisLimits<'a> {
    /// A category axis: its categories come from the marks.
    Category,
    /// A linear or log axis.
    Continuous(ContinuousLimits<'a>),
}

impl<'a> AxisLimits<'a> {
    /// The limits of an axis of scale `kind` with the explicit limits `explicit` and the tick
    /// settings `ticks`, which a category axis does not use.
    fn new(
        kind: ScaleKind,
        explicit: Option<ContinuousScale>,
        ticks: &'a AxisTicks,
    ) -> AxisLimits<'a> {
        match kind.transform() {
            Some(transform) => AxisLimits::Continuous(ContinuousLimits {
                transform,
                explicit,
                ticks,
            }),
            None => AxisLimits::Category,
        }
    }
}

/// The limits and tick settings of both axes of an [`Axes`]. The y axis is always linear or
/// log.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Limits<'a> {
    /// The x axis.
    pub x: AxisLimits<'a>,
    /// The y axis.
    pub y: ContinuousLimits<'a>,
}

impl<'a> Limits<'a> {
    /// The scales, explicit limits and tick settings of `axes`.
    pub fn of(axes: &'a Axes) -> Limits<'a> {
        Limits {
            x: AxisLimits::new(axes.x_scale(), axes.x_limit_scale(), axes.x_tick_settings()),
            y: ContinuousLimits {
                transform: axes.y_transform(),
                explicit: axes.y_limit_scale(),
                ticks: axes.y_tick_settings(),
            },
        }
    }
}

/// The smallest and largest finite value, ignoring NaN; `None` when there is none.
fn extent(values: impl IntoIterator<Item = f64>) -> Option<(f64, f64)> {
    values
        .into_iter()
        .filter(|v| v.is_finite())
        .fold(None, |acc, v| match acc {
            None => Some((v, v)),
            Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
        })
}

/// A continuous axis resolved by [`continuous`], with its scale and the lower limit of its
/// domain before padding.
struct ContinuousAxis {
    /// The axis.
    axis: ResolvedContinuousAxis,
    /// The lower limit before padding: the nice lower limit of a padded axis, the domain's own
    /// lower limit otherwise.
    unpadded_low: f64,
}

/// A continuous axis, as its `limits` and tick settings fix it and the `data` extent fits it.
///
/// The domain is the scale of the explicit limits when set. Otherwise, with automatic ticks, it
/// is the empty-data range (0 to 1, or 1 to 10 on log) when there is no finite data, else the
/// data extent fitted as `auto` says; with explicit ticks it is [`enclosing`] the data and the
/// ticks. Nice axes take the ticks that produced their limits, other automatic axes take
/// `ticks()`, and explicit ticks are the ones inside the domain ([`explicit_ticks`]). The tick
/// format then labels every tick except those given with labels of their own. Automatic ticks
/// keep their labels apart along the axis as `room` asks ([`DrawnTicks`]).
fn continuous(
    limits: ContinuousLimits<'_>,
    data: Option<(f64, f64)>,
    auto: AutoLimits,
    room: &TickRoom<'_>,
) -> ContinuousAxis {
    let ContinuousLimits {
        transform,
        explicit,
        ticks: settings,
    } = limits;
    let (scale, unpadded, ticks) = match settings.explicit() {
        None => {
            let judge = DrawnTicks {
                format: settings.format(),
                room,
            };
            let (scale, unpadded, ticks) =
                automatic(transform, explicit, data, auto, room.target, &judge);
            (
                scale,
                unpadded,
                settings.format().relabel(ticks.ticks, ticks.values),
            )
        }
        Some(given) => {
            let (scale, unpadded) = match explicit {
                Some(scale) => (scale, scale),
                None => enclosing(transform, data, given, auto),
            };
            let ticks = match explicit_ticks(given, transform, &scale) {
                ExplicitLabels::Given(ticks) => ticks,
                ExplicitLabels::Automatic(ticks) => {
                    settings.format().relabel(ticks, TickValues::Exact)
                }
            };
            (scale, unpadded, ticks)
        }
    };
    ContinuousAxis {
        axis: ResolvedContinuousAxis {
            ticks: positioned(&scale, ticks),
            scale,
        },
        unpadded_low: unpadded.domain().0,
    }
}

/// The domain, the domain before padding and the automatic ticks of a continuous axis: the
/// scale of the explicit limits when set; otherwise the empty-data range (0 to 1, or 1 to 10 on
/// log) when there is no finite data; otherwise the data extent, fitted as `auto` says. Nice
/// axes take the ticks that produced their limits ([`ContinuousScale::fitted`]); the others
/// take `ticks()`. Only a nice axis pads its domain, after its ticks are chosen; an extent that
/// counts as a single value is padded by [`ContinuousScale::from_extent`] alone. The ticks are
/// those `judge` lets through on the final domain, with the automatic labels, and carry the
/// step they were written for.
fn automatic(
    transform: Transform,
    limits: Option<ContinuousScale>,
    data: Option<(f64, f64)>,
    auto: AutoLimits,
    target: usize,
    judge: &dyn TickJudge,
) -> (ContinuousScale, ContinuousScale, AutoTicks) {
    if let Some(scale) = limits {
        (scale, scale, scale.auto_ticks(target, judge))
    } else if data.is_none() {
        // The fallback range is final on every axis: never widened with `nice`.
        let scale = ContinuousScale::unit(transform);
        (scale, scale, scale.auto_ticks(target, judge))
    } else {
        let scale = ContinuousScale::from_extent(transform, data, CONTEXT)
            .unwrap_or_else(|_| ContinuousScale::unit(transform));
        match auto {
            AutoLimits::Exact => (scale, scale, scale.auto_ticks(target, judge)),
            AutoLimits::Nice(padding) | AutoLimits::NiceBelowData(padding) => {
                let below_data = match (auto, data) {
                    (AutoLimits::NiceBelowData(_), Some((data_low, _))) => Some(data_low),
                    _ => None,
                };
                let single = data.is_some_and(|(lo, hi)| is_single_value(transform, lo, hi));
                let finish = Finish {
                    below_data,
                    padding: (!single).then_some(padding),
                };
                scale.fitted(target, finish, judge)
            }
        }
    }
}

/// Judges automatic ticks as the axis draws them: labelled by its tick format, at their
/// positions along the axis, with labels kept apart as its tick room asks.
struct DrawnTicks<'a> {
    /// The axis's tick format.
    format: &'a TickFormat,
    /// The axis's tick room.
    room: &'a TickRoom<'a>,
}

impl TickJudge for DrawnTicks<'_> {
    fn deferred(&self) -> bool {
        self.room.deferred()
    }

    fn least_share(&self) -> f64 {
        self.room.least_share()
    }

    fn slack(&self, scale: &ContinuousScale, ticks: &AutoTicks) -> f64 {
        let labelled = self.format.relabel(ticks.ticks.clone(), ticks.values);
        self.room.slack(&positioned(scale, labelled))
    }
}

/// The domain and the domain before padding of a continuous axis with explicit ticks and no
/// explicit limits: the extent of the `data` and the tick positions together, as it is (no
/// rounding to nice limits, and the lowest tick is never moved below the data), then padded
/// as `auto` pads a nice domain. A linear axis padded with [`Padding::KeepZero`] holds bars,
/// a histogram or an area, so its extent includes 0 even when the marks have no finite value.
/// An exact axis is not padded, and an extent that counts as a single value is padded by
/// [`ContinuousScale::from_extent`] alone. Without data and without ticks it is the empty-data
/// range, which is not padded either.
fn enclosing(
    transform: Transform,
    data: Option<(f64, f64)>,
    given: &ExplicitTicks,
    auto: AutoLimits,
) -> (ContinuousScale, ContinuousScale) {
    let positions: Vec<f64> = match given {
        ExplicitTicks::Positions(values) => values.clone(),
        ExplicitTicks::Labeled(ticks) => ticks.iter().map(|tick| tick.value).collect(),
    };
    let ends = data.into_iter().flat_map(|(lo, hi)| [lo, hi]);
    let Some((lo, hi)) = extent(ends.chain(positions)) else {
        let unit = ContinuousScale::unit(transform);
        return (unit, unit);
    };
    let keeps_zero = matches!(
        auto,
        AutoLimits::Nice(Padding::KeepZero) | AutoLimits::NiceBelowData(Padding::KeepZero)
    );
    let (lo, hi) = if keeps_zero && transform == Transform::Linear {
        (lo.min(0.0), hi.max(0.0))
    } else {
        (lo, hi)
    };
    let scale = ContinuousScale::from_extent(transform, Some((lo, hi)), CONTEXT)
        .unwrap_or_else(|_| ContinuousScale::unit(transform));
    let padded = match auto {
        AutoLimits::Exact => scale,
        AutoLimits::Nice(_) | AutoLimits::NiceBelowData(_)
            if is_single_value(transform, lo, hi) =>
        {
            scale
        }
        AutoLimits::Nice(padding) | AutoLimits::NiceBelowData(padding) => scale.padded(padding),
    };
    (padded, scale)
}

/// The explicit ticks of an axis that lie inside its domain, and where their labels come from.
enum ExplicitLabels {
    /// Ticks with the labels given with them, drawn as given.
    Given(Vec<Tick>),
    /// Ticks with automatic labels, for the axis's tick format to relabel.
    Automatic(Vec<Tick>),
}

/// The explicit ticks `given` that lie inside the domain of `scale` (both ends included), so
/// that ticks outside explicit limits are not drawn. Positions without labels get the automatic
/// labels for their transform: the grid policy over the decimals they need on a linear axis,
/// the value policy on a log axis.
fn explicit_ticks(
    given: &ExplicitTicks,
    transform: Transform,
    scale: &ContinuousScale,
) -> ExplicitLabels {
    let (lo, hi) = scale.domain();
    let inside = |v: f64| lo <= v && v <= hi;
    match given {
        ExplicitTicks::Labeled(ticks) => ExplicitLabels::Given(
            ticks
                .iter()
                .filter(|tick| inside(tick.value))
                .cloned()
                .collect(),
        ),
        ExplicitTicks::Positions(values) => {
            let values: Vec<f64> = values.iter().copied().filter(|&v| inside(v)).collect();
            let labels = match transform {
                Transform::Linear => ticks::format_given(&values),
                Transform::Log(_) => ticks::format_values(&values),
            };
            ExplicitLabels::Automatic(
                values
                    .into_iter()
                    .zip(labels)
                    .map(|(value, label)| Tick { value, label })
                    .collect(),
            )
        }
    }
}

/// Where the baseline sits on the continuous y axis `scale` of `transform`, whose lower limit
/// before padding is `unpadded_low`: at 0 when 0 lies inside the domain of a linear axis,
/// otherwise at `unpadded_low`. A position that does not map (impossible for a validated
/// scale) falls back to the bottom.
fn baseline(scale: &ContinuousScale, transform: Transform, unpadded_low: f64) -> NormalizedPos {
    let (lo, hi) = scale.domain();
    let value = if transform == Transform::Linear && lo <= 0.0 && 0.0 <= hi {
        0.0
    } else {
        unpadded_low
    };
    let pos = scale.normalize(value);
    if pos.is_finite() {
        NormalizedPos::new(pos)
    } else {
        NormalizedPos::LOW
    }
}

/// Ticks with their positions along `scale`; ticks that do not map are dropped.
fn positioned(scale: &ContinuousScale, ticks: Vec<Tick>) -> Vec<AxisTick> {
    ticks
        .into_iter()
        .filter_map(|tick| {
            let pos = scale.normalize(tick.value);
            pos.is_finite().then_some(AxisTick {
                pos: NormalizedPos::new(pos),
                label: tick.label,
            })
        })
        .collect()
}

fn category_ticks(scale: &CategoryScale) -> Vec<AxisTick> {
    scale
        .categories()
        .iter()
        .enumerate()
        .map(|(i, name)| AxisTick {
            pos: NormalizedPos::new(scale.center(i)),
            label: name.clone(),
        })
        .collect()
}

/// The x categories of every mark, in mark order, then in data order.
fn categories<'a, 'm: 'a>(marks: impl Iterator<Item = &'a ResolvedMark<'m>>) -> CategoryScale {
    CategoryScale::from_first_appearance(marks.flat_map(|mark| mark.x_categories()))
}

/// Checks, before `figure` is rendered in `context`, that every vertical reference line on a
/// category x axis sits at a category of its axes: one that a mark of the same axes names. A
/// line cannot add a category, and the model cannot know the categories of marks added later,
/// so this is the one place where an unknown name is found. The first line, in the row-major
/// order of the axes' top-left cells and the order of the marks, whose name is not a category is
/// the error: `Error::Data` in `context`, without a parameter, since the call that renders has
/// no `x`. On a grid of more than one cell the message starts with the axes' top-left cell, as
/// the layout errors of [`pipeline::scene`](crate::pipeline) do.
pub(crate) fn check_references(figure: &Figure, context: &'static str) -> Result<()> {
    for axes in figure.panels() {
        if axes.x_scale() != ScaleKind::Category {
            continue;
        }
        let mut lines = axes.marks().iter().filter_map(|mark| match mark {
            Mark::VLine(line) => match line.x() {
                AxisValue::Category(name) => Some(name),
                AxisValue::Number(_) => None,
            },
            _ => None,
        });
        // Axes without a vertical line at a category need no lookup, and no marks resolved.
        let Some(first) = lines.next() else {
            continue;
        };
        let cells = axes.cells();
        let key = figure.x_group_of(cells.row(), cells.col());
        let scale = CategoryScale::from_first_appearance(
            figure
                .panels()
                .filter(|member| {
                    let c = member.cells();
                    figure.x_group_of(c.row(), c.col()) == key
                })
                .flat_map(|member| member.marks())
                .flat_map(mark_categories)
                .map(String::as_str),
        );
        if let Some(name) = std::iter::once(first)
            .chain(lines)
            .find(|name| scale.index_of(name).is_none())
        {
            let error = Error::data(
                context,
                format!(
                    "x is {}; no mark on this axes has that category.",
                    quoted(name)
                ),
            )
            .with_hint("draw a mark with that category first.");
            let grid = figure.grid();
            if grid.rows() * grid.cols() == 1 {
                return Err(error);
            }
            let cells = axes.cells();
            return Err(error.with_message_prefix(&format!(
                "row {}, col {}: ",
                cells.row(),
                cells.col()
            )));
        }
    }
    Ok(())
}

/// The categories `mark` names on a category x axis, in data order, repeats included: what
/// [`ResolvedMark::x_categories`] yields for the same mark, found without resolving it.
fn mark_categories(mark: &Mark) -> &[String] {
    match mark {
        Mark::Line(mark) => category_names(mark.x()),
        Mark::Scatter(mark) => category_names(mark.x()),
        Mark::Area(mark) => category_names(mark.x()),
        Mark::Step(mark) => category_names(mark.x()),
        Mark::Bar(mark) => mark.categories(),
        Mark::GroupedBar(mark) | Mark::StackedBar(mark) => mark.categories(),
        Mark::Hist(_) | Mark::HLine(_) | Mark::VLine(_) | Mark::HSpan(_) | Mark::VSpan(_) => &[],
        Mark::Band(_) => &[],
        Mark::ErrorBars(mark) => category_names(mark.positions()),
    }
}

/// Resolves both axes from the resolved `marks` and the axes' `limits`, with automatic ticks as
/// the tick rooms `x_room` and `y_room` ask: about their target number of ticks, with labels
/// kept apart.
#[cfg(test)]
pub(crate) fn resolve(
    marks: &[ResolvedMark<'_>],
    limits: &Limits,
    x_room: &TickRoom<'_>,
    y_room: &TickRoom<'_>,
) -> Resolved {
    let x = resolve_x(marks.iter(), limits.x, x_room);
    let (y, baseline) = resolve_y(marks.iter(), limits.y, y_room);
    Resolved { x, y, baseline }
}

/// Resolves one x group, with members' marks in row-major panel order, then mark order.
/// The iterator is cloned to inspect the extent and padding without copying any mark data.
pub(crate) fn resolve_x<'a, 'm: 'a>(
    marks: impl Iterator<Item = &'a ResolvedMark<'m>> + Clone,
    limits: AxisLimits<'_>,
    room: &TickRoom<'_>,
) -> ResolvedAxis {
    match limits {
        AxisLimits::Continuous(x_limits) => {
            let data = extent(marks.clone().flat_map(|mark| mark.x_numbers()));
            // Reference lines and spans do not decide the padding: an axis of lines and
            // references keeps the exact extent of the lines, while an axis of references
            // alone is padded.
            // Only references that give this axis values pad it; a horizontal reference alone
            // leaves the x axis as it would be without it.
            let contributing = || {
                marks
                    .clone()
                    .filter(|mark| mark.x_numbers().any(f64::is_finite))
            };
            let only_references = data.is_some() && contributing().all(ResolvedMark::is_reference);
            let all_lines = !only_references
                && contributing()
                    .filter(|mark| !mark.is_reference())
                    .all(ResolvedMark::has_exact_x_extent);
            let auto = if all_lines {
                AutoLimits::Exact
            } else {
                AutoLimits::Nice(Padding::BothSides)
            };
            continuous(x_limits, data, auto, room).axis.into_axis()
        }
        AxisLimits::Category => {
            let scale = categories(marks);
            ResolvedAxis {
                ticks: category_ticks(&scale),
                scale: AxisScale::Category(scale),
            }
        }
    }
}

/// Resolves one y group and the baseline shared by its members.
pub(crate) fn resolve_y<'a, 'm: 'a>(
    marks: impl Iterator<Item = &'a ResolvedMark<'m>> + Clone,
    limits: ContinuousLimits<'_>,
    room: &TickRoom<'_>,
) -> (ResolvedContinuousAxis, NormalizedPos) {
    let transform = limits.transform;
    let data = extent(marks.clone().flat_map(|mark| mark.y_values()));
    // Bars and areas grow from 0 on a linear axis, so the axis shows 0 and keeps it on the plot
    // edge when it is a limit. On a log axis they grow from the baseline.
    // With no data at all, explicit ticks still use the baseline convention of the mark.
    let bars = marks.clone().any(|mark| {
        mark.grows_from_baseline() && (data.is_none() || mark.y_values().any(f64::is_finite))
    });
    let (data, padding) = if bars && transform == Transform::Linear {
        let with_zero = data.map(|(lo, hi)| (lo.min(0.0), hi.max(0.0)));
        (with_zero, Padding::KeepZero)
    } else {
        (data, Padding::BothSides)
    };
    let auto = AutoLimits::NiceBelowData(padding);
    let ContinuousAxis {
        axis: y,
        unpadded_low,
    } = continuous(limits, data, auto, room);
    let baseline = baseline(&y.scale, transform, unpadded_low);
    (y, baseline)
}

/// The values one mark contributes to an axis domain, in order (see [`ResolvedMark`]).
///
/// Raw data is a slice, and folding over it, as the extent does, runs as a plain loop over the
/// slice; a boxed iterator would make one dynamic call per value. Values computed from what a
/// record owns are few and stay boxed.
pub(crate) enum DomainValues<'a> {
    /// The mark's own data, in data order.
    Data(std::iter::Copied<std::slice::Iter<'a, f64>>),
    /// Values computed from the record.
    Computed(Box<dyn Iterator<Item = f64> + 'a>),
}

impl Iterator for DomainValues<'_> {
    type Item = f64;

    fn next(&mut self) -> Option<f64> {
        match self {
            DomainValues::Data(values) => values.next(),
            DomainValues::Computed(values) => values.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            DomainValues::Data(values) => values.size_hint(),
            DomainValues::Computed(values) => values.size_hint(),
        }
    }

    fn fold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, f64) -> B,
    {
        match self {
            DomainValues::Data(values) => values.fold(init, f),
            DomainValues::Computed(values) => values.fold(init, f),
        }
    }
}

/// A mark resolved for drawing: one variant per mark kind, each holding the mark and its color
/// resolved against the theme's palette, and for lines, areas and scatters the stroke width or
/// marker diameter they are drawn at, resolved against the theme. Axes, the legend and drawing
/// read marks through these records only, so every reader sees the same color and size.
///
/// A record may own data computed from its mark, such as bins or stacks, so the type is not
/// `Copy`, and its accessors borrow from the record rather than from the mark.
///
/// The axis-domain accessors ([`x_numbers`](Self::x_numbers),
/// [`x_categories`](Self::x_categories) and [`y_values`](Self::y_values)) each yield what the
/// mark contributes to an axis, in a fixed order, borrowed from the record. A mark with raw data
/// yields its data in data order. A mark with computed data yields values computed from what
/// the record owns, such as the ends of its stack segments or the range of its bin counts, never
/// a copy of raw data shaped to fit. Non-finite numbers are allowed; the extent ignores them.
/// Axis resolution scans the marks in order, and each mark's values in the order given, so that
/// order is part of the output.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResolvedMark<'a> {
    /// A line mark.
    Line {
        /// The mark.
        mark: &'a LineMark,
        /// The line color.
        color: Rgba,
        /// The stroke width as drawn, in points.
        width: f32,
    },
    /// A scatter mark.
    Scatter {
        /// The mark.
        mark: &'a ScatterMark,
        /// The marker color.
        color: Rgba,
        /// The marker diameter as drawn, in points.
        size: f32,
    },
    /// An area mark.
    Area {
        /// The mark.
        mark: &'a AreaMark,
        /// The color of the line and of its fill.
        color: Rgba,
        /// The stroke width of the line as drawn, in points.
        width: f32,
    },
    /// A bar mark.
    Bar {
        /// The mark.
        mark: &'a BarMark,
        /// The bar color.
        color: Rgba,
    },
    /// A grouped bar mark.
    Grouped {
        /// The mark.
        mark: &'a BarGroupMark,
        /// Each series of the mark, in order, with its resolved color.
        series: Box<[(SeriesView<'a>, Rgba)]>,
    },
    /// A stacked bar mark.
    Stacked {
        /// The mark.
        mark: &'a BarGroupMark,
        /// Each series of the mark, in order, with its resolved color.
        series: Box<[(SeriesView<'a>, Rgba)]>,
        /// The segments of every series in every category of the mark, indexed as the mark's
        /// categories and series.
        stacks: Stacks,
    },
    /// A histogram mark.
    Hist {
        /// The mark.
        mark: &'a HistMark,
        /// The mark's values binned; `None` when it has no finite value.
        histogram: Option<Histogram>,
        /// The bar color.
        color: Rgba,
    },
    /// A step mark.
    Step {
        /// The mark.
        mark: &'a StepMark,
        /// The line color.
        color: Rgba,
        /// The stroke width as drawn, in points.
        width: f32,
    },
    /// A horizontal reference line.
    HLine {
        /// The mark.
        mark: &'a HLineMark,
        /// The line color.
        color: Rgba,
        /// The stroke width as drawn, in points.
        width: f32,
    },
    /// A vertical reference line.
    VLine {
        /// The mark.
        mark: &'a VLineMark,
        /// The line color.
        color: Rgba,
        /// The stroke width as drawn, in points.
        width: f32,
    },
    /// A horizontal reference span.
    HSpan {
        /// The mark.
        mark: &'a SpanMark,
        /// The fill color, before the area opacity.
        color: Rgba,
    },
    /// A vertical reference span.
    VSpan {
        /// The mark.
        mark: &'a SpanMark,
        /// The fill color, before the area opacity.
        color: Rgba,
    },
    /// A band between two curves.
    Band {
        /// The mark.
        mark: &'a BandMark,
        /// The fill color, before the area opacity.
        color: Rgba,
    },
    /// Error bars.
    ErrorBars {
        /// The mark.
        mark: &'a ErrorBarMark,
        /// The color of the bars and their caps.
        color: Rgba,
        /// The stroke width of the bars and their caps as drawn, in points.
        width: f32,
    },
}

impl<'a> ResolvedMark<'a> {
    /// The values the mark contributes to a continuous x domain, in data order; none when its
    /// x values are categories. See [`ResolvedMark`] for the contract.
    ///
    /// A histogram yields the low and the high end of its binned range
    /// ([`Histogram::range`]), and nothing when it has no finite value.
    pub fn x_numbers(&self) -> DomainValues<'_> {
        let values: &[f64] = match self {
            ResolvedMark::Line { mark, .. } => numbers(mark.x()),
            ResolvedMark::Scatter { mark, .. } => numbers(mark.x()),
            ResolvedMark::Area { mark, .. } => numbers(mark.x()),
            ResolvedMark::Step { mark, .. } => numbers(mark.x()),
            ResolvedMark::VLine { mark, .. } => mark.x_numbers(),
            ResolvedMark::VSpan { mark, .. } => mark.bounds(),
            ResolvedMark::Band { mark, .. } => mark.x(),
            ResolvedMark::ErrorBars { mark, .. } => match mark.orientation() {
                ErrorBarOrientation::Vertical => numbers(mark.positions()),
                // The limits of horizontal bars are x values.
                ErrorBarOrientation::Horizontal => {
                    return DomainValues::Computed(Box::new(
                        mark.low().iter().chain(mark.high()).copied(),
                    ));
                }
            },
            ResolvedMark::Bar { .. }
            | ResolvedMark::Grouped { .. }
            | ResolvedMark::Stacked { .. }
            | ResolvedMark::HLine { .. }
            | ResolvedMark::HSpan { .. } => &[],
            ResolvedMark::Hist { histogram, .. } => {
                let range = histogram.as_ref().map(Histogram::range);
                return DomainValues::Computed(Box::new(
                    range.into_iter().flat_map(|(lo, hi)| [lo, hi]),
                ));
            }
        };
        DomainValues::Data(values.iter().copied())
    }

    /// The categories the mark contributes to a category x axis, in data order, repeats
    /// included; none when its x values are numbers. See [`ResolvedMark`] for the contract.
    pub fn x_categories(&self) -> impl Iterator<Item = &str> + '_ {
        match self {
            ResolvedMark::Line { mark, .. } => category_names(mark.x()),
            ResolvedMark::Scatter { mark, .. } => category_names(mark.x()),
            ResolvedMark::Area { mark, .. } => category_names(mark.x()),
            ResolvedMark::Step { mark, .. } => category_names(mark.x()),
            ResolvedMark::ErrorBars { mark, .. } => category_names(mark.positions()),
            ResolvedMark::Bar { mark, .. } => mark.categories(),
            ResolvedMark::Grouped { mark, .. } | ResolvedMark::Stacked { mark, .. } => {
                mark.categories()
            }
            // A reference line at a category does not extend the category axis; it is looked up
            // in the categories of the other marks (see `check_references`).
            ResolvedMark::Hist { .. }
            | ResolvedMark::HLine { .. }
            | ResolvedMark::VLine { .. }
            | ResolvedMark::HSpan { .. }
            | ResolvedMark::VSpan { .. }
            | ResolvedMark::Band { .. } => &[],
        }
        .iter()
        .map(String::as_str)
    }

    /// The values the mark contributes to the y domain. See [`ResolvedMark`] for the contract.
    ///
    /// Lines, scatters, areas and bars yield their y values or heights in data order, and grouped
    /// bars the values of each series in turn. Stacked bars yield the end of every segment away
    /// from 0 ([`Stacks::ends`]) and any zero heights, so an all-zero stack still has a domain.
    /// On a linear axis the bars put 0 into the domain anyway (see
    /// [`grows_from_baseline`](Self::grows_from_baseline)), and on a log axis, where every height
    /// is positive, these are the positive segment ends, which a log domain can hold.
    /// Histograms yield the count of every non-empty bin, in bin order, and so never 0 either: on a
    /// log axis the domain spans the smallest positive count to the largest.
    pub fn y_values(&self) -> DomainValues<'_> {
        let values: &[f64] = match self {
            ResolvedMark::Line { mark, .. } => mark.y(),
            ResolvedMark::Scatter { mark, .. } => mark.y(),
            ResolvedMark::Area { mark, .. } => mark.y(),
            ResolvedMark::Step { mark, .. } => mark.y(),
            ResolvedMark::HLine { mark, .. } => mark.y_values(),
            ResolvedMark::HSpan { mark, .. } => mark.bounds(),
            ResolvedMark::VLine { .. } | ResolvedMark::VSpan { .. } => &[],
            ResolvedMark::Band { mark, .. } => {
                return DomainValues::Computed(Box::new(
                    mark.low().iter().chain(mark.high()).copied(),
                ));
            }
            ResolvedMark::ErrorBars { mark, .. } => match mark.orientation() {
                ErrorBarOrientation::Vertical => {
                    return DomainValues::Computed(Box::new(
                        mark.low().iter().chain(mark.high()).copied(),
                    ));
                }
                ErrorBarOrientation::Horizontal => numbers(mark.positions()),
            },
            ResolvedMark::Bar { mark, .. } => mark.heights(),
            ResolvedMark::Grouped { series, .. } => {
                return DomainValues::Computed(Box::new(
                    series
                        .iter()
                        .flat_map(|(view, _)| view.values().iter().copied()),
                ));
            }
            ResolvedMark::Stacked { stacks, series, .. } => {
                let zeros = series
                    .iter()
                    .flat_map(|(view, _)| view.values().iter().copied())
                    .filter(|&height| height == 0.0);
                return DomainValues::Computed(Box::new(stacks.ends().chain(zeros)));
            }
            ResolvedMark::Hist { histogram, .. } => {
                let counts = histogram.as_ref().map_or(&[][..], Histogram::counts);
                return DomainValues::Computed(Box::new(
                    counts
                        .iter()
                        .filter(|&&count| count > 0)
                        .map(|&count| count as f64),
                ));
            }
        };
        DomainValues::Data(values.iter().copied())
    }

    /// Whether the mark grows from the baseline, as bars, histogram bins and the fill of an
    /// area do: such a mark puts 0 into a linear y domain.
    pub fn grows_from_baseline(&self) -> bool {
        match self {
            ResolvedMark::Line { .. }
            | ResolvedMark::Scatter { .. }
            | ResolvedMark::Step { .. }
            | ResolvedMark::HLine { .. }
            | ResolvedMark::VLine { .. }
            | ResolvedMark::HSpan { .. }
            | ResolvedMark::VSpan { .. }
            | ResolvedMark::Band { .. }
            | ResolvedMark::ErrorBars { .. } => false,
            ResolvedMark::Area { .. }
            | ResolvedMark::Bar { .. }
            | ResolvedMark::Grouped { .. }
            | ResolvedMark::Stacked { .. }
            | ResolvedMark::Hist { .. } => true,
        }
    }

    /// The stroke width, as drawn, of the line the mark is drawn along, as line marks and the
    /// line that bounds an area are; `None` for marks not drawn along a line. An x axis whose
    /// marks all draw a line keeps the exact data extent, and such marks widen the marks clip by
    /// half their stroke.
    pub fn line_width(&self) -> Option<f32> {
        match *self {
            ResolvedMark::Line { width, .. }
            | ResolvedMark::Area { width, .. }
            | ResolvedMark::Step { width, .. } => Some(width),
            ResolvedMark::Scatter { .. }
            | ResolvedMark::Bar { .. }
            | ResolvedMark::Grouped { .. }
            | ResolvedMark::Stacked { .. }
            | ResolvedMark::Hist { .. }
            | ResolvedMark::HLine { .. }
            | ResolvedMark::VLine { .. }
            | ResolvedMark::HSpan { .. }
            | ResolvedMark::VSpan { .. }
            | ResolvedMark::Band { .. }
            | ResolvedMark::ErrorBars { .. } => None,
        }
    }

    /// Whether the mark is a reference line or span: it runs across the whole plot, extends a
    /// domain only by its own values, and does not count among the marks that decide how an axis
    /// is padded (see [`resolve_x`]).
    pub fn is_reference(&self) -> bool {
        match self {
            ResolvedMark::HLine { .. }
            | ResolvedMark::VLine { .. }
            | ResolvedMark::HSpan { .. }
            | ResolvedMark::VSpan { .. } => true,
            ResolvedMark::Line { .. }
            | ResolvedMark::Scatter { .. }
            | ResolvedMark::Area { .. }
            | ResolvedMark::Bar { .. }
            | ResolvedMark::Grouped { .. }
            | ResolvedMark::Stacked { .. }
            | ResolvedMark::Hist { .. }
            | ResolvedMark::Step { .. }
            | ResolvedMark::Band { .. }
            | ResolvedMark::ErrorBars { .. } => false,
        }
    }

    /// Whether an x axis padded by the marks reads the exact extent of this mark: the marks
    /// drawn along a line ([`line_width`](Self::line_width)) and bands, which are bounded by
    /// curves over their x values. An x axis whose marks, references apart, all have it keeps
    /// the exact data extent (see [`resolve_x`]).
    pub fn has_exact_x_extent(&self) -> bool {
        self.line_width().is_some() || matches!(self, ResolvedMark::Band { .. })
    }

    /// The labelled series' swatches in series order, without resolving direct-label anchors.
    /// Empty labels do not produce entries.
    pub fn legend_swatch(&self) -> Vec<LegendEntry> {
        let one = |label: &str, color, swatch| {
            LegendEntry::new(label, color, swatch).into_iter().collect()
        };
        match *self {
            ResolvedMark::Line { mark, color, width } => one(
                &mark.options().label,
                color,
                Swatch::Line {
                    dash: mark.options().dash,
                    width,
                },
            ),
            ResolvedMark::Area { mark, color, width } => one(
                &mark.options().label,
                color,
                Swatch::Line {
                    dash: mark.options().dash,
                    width,
                },
            ),
            ResolvedMark::Step { mark, color, width } => one(
                &mark.options().label,
                color,
                Swatch::Line {
                    dash: mark.options().dash,
                    width,
                },
            ),
            ResolvedMark::HLine { mark, color, width } => one(
                &mark.options().label,
                color,
                Swatch::Line {
                    dash: mark.options().dash,
                    width,
                },
            ),
            ResolvedMark::VLine { mark, color, width } => one(
                &mark.options().label,
                color,
                Swatch::Line {
                    dash: mark.options().dash,
                    width,
                },
            ),
            ResolvedMark::Scatter { mark, color, size } => {
                one(&mark.options().label, color, Swatch::Dot { diameter: size })
            }
            ResolvedMark::Bar { mark, color } => one(&mark.options().label, color, Swatch::Square),
            ResolvedMark::Grouped { ref series, .. } | ResolvedMark::Stacked { ref series, .. } => {
                series
                    .iter()
                    .filter_map(|(view, color)| {
                        LegendEntry::new(view.label(), *color, Swatch::Square)
                    })
                    .collect()
            }
            ResolvedMark::Hist { mark, color, .. } => one(mark.label(), color, Swatch::Square),
            ResolvedMark::HSpan { mark, color } | ResolvedMark::VSpan { mark, color } => {
                one(&mark.options().label, color, Swatch::Square)
            }
            ResolvedMark::Band { mark, color } => one(&mark.options().label, color, Swatch::Square),
            ResolvedMark::ErrorBars { mark, color, width } => one(
                &mark.options().label,
                color,
                Swatch::ErrorBar {
                    vertical: mark.orientation() == ErrorBarOrientation::Vertical,
                    width,
                },
            ),
        }
    }

    /// The series the mark shows, in order, one key each: what a legend lists for the mark, on
    /// an axes whose axes have the limits `limits`.
    pub fn keys(&self, limits: &Limits) -> Vec<SeriesKey<'a>> {
        match self {
            &ResolvedMark::Line { mark, color, width } => {
                vec![SeriesKey::of_line(mark, color, width, limits)]
            }
            // An area is labelled as the line that bounds it: the line's swatch, and a direct
            // label at the line's anchor.
            &ResolvedMark::Area { mark, color, width } => {
                vec![SeriesKey::of_line(mark.line(), color, width, limits)]
            }
            &ResolvedMark::Scatter { mark, color, size } => {
                let options = mark.options();
                vec![SeriesKey {
                    label: &options.label,
                    color,
                    kind: SeriesKind::Points {
                        marker: options.marker,
                        size,
                    },
                    anchor: None,
                }]
            }
            &ResolvedMark::Bar { mark, color } => vec![SeriesKey {
                label: &mark.options().label,
                color,
                kind: SeriesKind::Bar,
                anchor: None,
            }],
            ResolvedMark::Grouped { series, .. } | ResolvedMark::Stacked { series, .. } => series
                .iter()
                .map(|&(view, color)| SeriesKey {
                    label: view.label(),
                    color,
                    kind: SeriesKind::Bar,
                    anchor: None,
                })
                .collect(),
            &ResolvedMark::Hist { mark, color, .. } => vec![SeriesKey {
                label: mark.label(),
                color,
                kind: SeriesKind::Bar,
                anchor: None,
            }],
            // A step line is labelled as a line is: the line's swatch, and a direct label at the
            // end of the line.
            &ResolvedMark::Step { mark, color, width } => {
                vec![SeriesKey::of_step(mark, color, width, limits)]
            }
            // Reference lines and spans have no anchor, and their kinds say that no direct
            // label ever goes to them (see `SeriesKind::takes_direct_label`).
            &ResolvedMark::HLine { mark, color, width } => {
                vec![SeriesKey::of_rule(mark.options(), color, width)]
            }
            &ResolvedMark::VLine { mark, color, width } => {
                vec![SeriesKey::of_rule(mark.options(), color, width)]
            }
            &ResolvedMark::HSpan { mark, color } | &ResolvedMark::VSpan { mark, color } => {
                vec![SeriesKey {
                    label: &mark.options().label,
                    color,
                    kind: SeriesKind::Span,
                    anchor: None,
                }]
            }
            // A band is a filled region and is listed as a span is: a square swatch, and an
            // entry in the legend row under every style.
            &ResolvedMark::Band { mark, color } => vec![SeriesKey {
                label: &mark.options().label,
                color,
                kind: SeriesKind::Span,
                anchor: None,
            }],
            // Error bars are listed with a short bar of their own, and never take a direct
            // label.
            &ResolvedMark::ErrorBars { mark, color, width } => vec![SeriesKey {
                label: &mark.options().label,
                color,
                kind: SeriesKind::ErrorBar {
                    vertical: mark.orientation() == ErrorBarOrientation::Vertical,
                    width,
                },
                anchor: None,
            }],
        }
    }
}

/// The numbers of `data`; empty for categories.
fn numbers(data: &AxisData) -> &[f64] {
    match data {
        AxisData::Numbers(values) => values,
        AxisData::Categories(_) => &[],
    }
}

/// The categories of `data`; empty for numbers.
fn category_names(data: &AxisData) -> &[String] {
    match data {
        AxisData::Categories(names) => names,
        AxisData::Numbers(_) => &[],
    }
}

/// One series as a legend shows it: its label, its resolved color, how it is drawn, and the
/// point a direct label would be attached to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SeriesKey<'a> {
    /// The label; empty for an unlabelled series.
    pub label: &'a str,
    /// The resolved color.
    pub color: Rgba,
    /// How the series is drawn.
    pub kind: SeriesKind,
    /// The anchor of a line series, an area's included; `None` for a line without one and for
    /// every other series.
    pub anchor: Option<Anchor<'a>>,
}

impl<'a> SeriesKey<'a> {
    /// The key of `line` in `color`, drawn `width` pt wide, on axes with the limits `limits`: a
    /// line swatch in the line's dash and width, and the line's [`Anchor`].
    fn of_line(line: &'a LineMark, color: Rgba, width: f32, limits: &Limits) -> SeriesKey<'a> {
        let options = line.options();
        SeriesKey {
            label: &options.label,
            color,
            kind: SeriesKind::Line {
                dash: options.dash,
                width,
            },
            anchor: Anchor::of_line(line, limits),
        }
    }
}

impl<'a> SeriesKey<'a> {
    /// The key of `step` in `color`, drawn `width` pt wide, on axes with the limits `limits`: a
    /// line swatch in the step line's dash and width, and the step line's [`Anchor`].
    fn of_step(step: &'a StepMark, color: Rgba, width: f32, limits: &Limits) -> SeriesKey<'a> {
        let options = step.options();
        SeriesKey {
            label: &options.label,
            color,
            kind: SeriesKind::Line {
                dash: options.dash,
                width,
            },
            anchor: Anchor::of_step(step, limits),
        }
    }
}

impl<'a> SeriesKey<'a> {
    /// The key of a reference line of `options` in `color`, drawn `width` pt wide: a line swatch
    /// in the rule's dash and width, and no anchor.
    fn of_rule(options: &'a RuleOptions, color: Rgba, width: f32) -> SeriesKey<'a> {
        SeriesKey {
            label: &options.label,
            color,
            kind: SeriesKind::Rule {
                dash: options.dash,
                width,
            },
            anchor: None,
        }
    }
}

/// The x value of a line's point, as the line holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AnchorX<'a> {
    /// A number on a linear or log axis; finite.
    Number(f64),
    /// A category name.
    Category(&'a str),
    /// Midway between the band centres of two category names, where a step line that changes
    /// level between two categories ends.
    Between(&'a str, &'a str),
}

/// How a direct label's anchor ends its line: whether the line ends there, so that an end dot
/// marks the point, or leaves the plot there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndKind {
    /// The anchor is a point of the line, the last one the plot shows: an end dot marks it.
    Point,
    /// The anchor is where the line leaves the plot across the high end of the x domain: the
    /// line continues past it, so no end dot is drawn.
    Edge,
}

/// The point of a line that its direct label is attached to: the last point whose x and y are
/// finite and whose x lies inside the x domain, or, when the line continues from that point
/// past the high end of the x domain, the point where it crosses that end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Anchor<'a> {
    /// The x value.
    pub x: AnchorX<'a>,
    /// The y value; finite.
    pub y: f64,
    /// Whether the line ends at the anchor or leaves the plot there.
    pub end: EndKind,
}

impl<'a> Anchor<'a> {
    /// The anchor of `line` on axes with the limits `limits`. Its last point, in data order,
    /// with finite x and y whose x lies inside the x domain (both ends included), is the
    /// [`EndKind::Point`] anchor, unless it lies below the high end of the domain and the next
    /// point in data order has finite x and y and an x past that end: the line then leaves the
    /// plot between the two, and the anchor is the [`EndKind::Edge`] where it crosses the high
    /// end, the two points interpolated in axis space (see [`crossing_at`]). A last point on
    /// the high end is a data point the plot shows, so it stays the `Point` anchor. A next
    /// point with a NaN breaks the line, so the line ends at the last point. `None` when the
    /// line has no point inside.
    ///
    /// Only explicit limits can leave a finite point outside the x domain: auto limits always
    /// contain every finite x value, and a category axis holds every category of its marks. So
    /// the limits decide the anchor before any layout pass, and it is the same in all of them.
    fn of_line(line: &'a LineMark, limits: &Limits) -> Option<Anchor<'a>> {
        let y = line.y();
        match line.x() {
            AxisData::Numbers(xs) => {
                let range = match limits.x {
                    AxisLimits::Continuous(ContinuousLimits {
                        transform,
                        explicit: Some(scale),
                        ..
                    }) => Some((transform, scale.domain())),
                    AxisLimits::Continuous(ContinuousLimits { explicit: None, .. })
                    | AxisLimits::Category => None,
                };
                let inside = |x: f64| range.is_none_or(|(_, (lo, hi))| lo <= x && x <= hi);
                let last = xs
                    .iter()
                    .zip(y)
                    .rposition(|(&x, &y)| x.is_finite() && y.is_finite() && inside(x))?;
                let point = Anchor {
                    x: AnchorX::Number(xs[last]),
                    y: y[last],
                    end: EndKind::Point,
                };
                let (Some((x_transform, (_, hi))), Some(&next_x), Some(&next_y)) =
                    (range, xs.get(last + 1), y.get(last + 1))
                else {
                    return Some(point);
                };
                let leaves = xs[last] < hi && next_x > hi;
                if !(leaves && next_x.is_finite() && next_y.is_finite()) {
                    return Some(point);
                }
                let crossing = crossing_at(
                    (x_transform, limits.y.transform),
                    (xs[last], y[last]),
                    (next_x, next_y),
                    hi,
                );
                Some(Anchor {
                    x: AnchorX::Number(hi),
                    y: crossing,
                    end: EndKind::Edge,
                })
            }
            AxisData::Categories(names) => names
                .iter()
                .zip(y)
                .rev()
                .find(|&(_, &y)| y.is_finite())
                .map(|(name, &y)| Anchor {
                    x: AnchorX::Category(name),
                    y,
                    end: EndKind::Point,
                }),
        }
    }

    /// The anchor of `step`: where the step line as drawn ends. The line is the holds of
    /// [`holds`], each a level held from one x to another, and its last vertex in data order is
    /// the end of the last hold, so a level held out to a neighbour whose y is NaN reaches that
    /// neighbour's x. The end is the [`EndKind::Point`] anchor, unless limits cut the line: the
    /// last hold that overlaps the x domain (both ends included) is then the one that ends the
    /// visible line, and when it goes on past the high end of the domain, even from the end itself,
    /// the anchor is the [`EndKind::Edge`] where its level crosses the end. A midpoint is the
    /// arithmetic mean of the two x values, and on a log x axis their geometric mean, as the line
    /// is drawn. A category line ends at the band of a point, or midway between two bands
    /// ([`AnchorX::Between`]). `None` when the line has no vertex inside.
    fn of_step(step: &'a StepMark, limits: &Limits) -> Option<Anchor<'a>> {
        let position = step.options().position;
        match step.x() {
            AxisData::Numbers(xs) => {
                let (range, log) = match limits.x {
                    AxisLimits::Continuous(ContinuousLimits {
                        transform,
                        explicit,
                        ..
                    }) => (
                        explicit.map(|scale| scale.domain()),
                        matches!(transform, Transform::Log(_)),
                    ),
                    AxisLimits::Category => (None, false),
                };
                let mid = |a: f64, b: f64| {
                    if log {
                        ((a.ln() + b.ln()) / 2.0).exp()
                    } else {
                        (a + b) / 2.0
                    }
                };
                let line = holds(xs, step.y(), position);
                // The last hold that is visible: it overlaps the x domain, ends included.
                let (lo, hi) = range.unwrap_or((f64::NEG_INFINITY, f64::INFINITY));
                let (hold, from, to) = line
                    .iter()
                    .map(|hold| {
                        let at = |edge: Edge| edge.position(xs, mid);
                        (hold, at(hold.from), at(hold.to))
                    })
                    .rfind(|&(_, from, to)| to >= lo && from <= hi)?;
                let y = step.y()[hold.point];
                // A hold that goes on past the high end is cut by it; one that ends inside the
                // domain ends the line there.
                let (x, end) = if to > hi && from <= hi {
                    (hi, EndKind::Edge)
                } else {
                    (to, EndKind::Point)
                };
                Some(Anchor {
                    x: AnchorX::Number(x),
                    y,
                    end,
                })
            }
            AxisData::Categories(names) => {
                // Band positions in data order; every one is finite.
                let order: Vec<f64> = (0..names.len()).map(|i| i as f64).collect();
                let last = holds(&order, step.y(), position).pop()?;
                let x = match last.to {
                    Edge::At(k) => AnchorX::Category(&names[k]),
                    Edge::Mid(i, k) => AnchorX::Between(&names[i], &names[k]),
                };
                Some(Anchor {
                    x,
                    y: step.y()[last.point],
                    end: EndKind::Point,
                })
            }
        }
    }
}

/// The y where the straight segment from `from` to `to` crosses `x = edge`, on axes with the
/// x and y transforms `transforms`. The segment is straight in axis space, where lines are
/// drawn: in the values on a linear axis and in their logarithms on a log axis. So the share of
/// the way to `edge` is measured in axis space along x, and y is interpolated by that share in
/// axis space along y.
///
/// `from.0 < edge < to.0`, every value finite, and every value on a log axis positive, as the
/// model validates them; the result then lies between `from.1` and `to.1`. On a log y axis no
/// intermediate value leaves the range of `f64`, even between the smallest and largest values
/// a log axis holds, and the result is kept between the two ends, so it is finite and
/// positive.
fn crossing_at(
    (x_transform, y_transform): (Transform, Transform),
    from: (f64, f64),
    to: (f64, f64),
    edge: f64,
) -> f64 {
    let share = match x_transform {
        Transform::Linear => (edge - from.0) / (to.0 - from.0),
        Transform::Log(_) => log_offset(edge, from.0) / log_offset(to.0, from.0),
    };
    match y_transform {
        Transform::Linear => from.1 + share * (to.1 - from.1),
        Transform::Log(_) => {
            // The way along in natural-log units, measured from `from.1` with full precision
            // for narrow spans (see `log_offset`).
            let rise = share * log_offset(to.1, from.1);
            // A factor within e^±700 neither overflows nor underflows, and the product lies
            // between the ends; beyond that, add the logarithms and exponentiate once.
            let y = if rise.abs() <= LOG_FACTOR_LIMIT {
                from.1 * rise.exp()
            } else {
                (from.1.ln() + rise).exp()
            };
            y.clamp(from.1.min(to.1), from.1.max(to.1))
        }
    }
}

/// The largest natural-log distance [`crossing_at`] turns into a factor with `exp`: `e^700` is
/// about `1e304`, inside the range of `f64`, and `e^-700` is a normal number.
const LOG_FACTOR_LIMIT: f64 = 700.0;

/// How a series is drawn, as far as its legend swatch needs to know.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SeriesKind {
    /// A line with its dash style and drawn width.
    Line {
        /// The dash style.
        dash: Dash,
        /// The stroke width as drawn, in points.
        width: f32,
    },
    /// Individual markers of one shape and drawn size.
    Points {
        /// The marker shape.
        marker: Marker,
        /// The marker diameter as drawn, in points.
        size: f32,
    },
    /// Filled bars.
    Bar,
    /// A reference line, drawn across the plot with its dash style and drawn width.
    Rule {
        /// The dash style.
        dash: Dash,
        /// The stroke width as drawn, in points.
        width: f32,
    },
    /// A filled region without an outline: a reference span across the plot, or a band between
    /// two curves.
    Span,
    /// Error bars, drawn as a short bar with a cap at each end, with the drawn width of their
    /// lines.
    ErrorBar {
        /// Whether the bar runs along y; otherwise along x.
        vertical: bool,
        /// The stroke width as drawn, in points.
        width: f32,
    },
}

impl SeriesKind {
    /// Whether a series of this kind may take a direct label at an [`Anchor`] of its own, where
    /// the legend style is [`LegendStyle::Direct`](crate::LegendStyle::Direct): lines and, through their lines, steps and
    /// areas. Reference lines and spans never do, whatever their key holds: they take an entry
    /// in the legend row under every style, as do points and bars.
    pub fn takes_direct_label(self) -> bool {
        match self {
            SeriesKind::Line { .. } => true,
            SeriesKind::Points { .. }
            | SeriesKind::Bar
            | SeriesKind::Rule { .. }
            | SeriesKind::Span
            | SeriesKind::ErrorBar { .. } => false,
        }
    }
}

/// Each mark resolved for drawing in `theme`, in mark order: its colors resolved against the
/// theme's palette, the width of lines and areas and the marker size of scatters resolved
/// against the theme's line width and marker size (see [`Theme::drawn_line_width`] and
/// [`Theme::drawn_marker_size`]), the stacks of stacked bars computed and the values of
/// histograms binned. This is the one place where a mark's [`SizeSpec`](crate::SizeSpec) is resolved.
///
/// Label slots precede unlabelled automatic colors; explicit colors do not consume a slot.
#[cfg(test)]
pub(crate) fn resolve_marks<'a>(marks: &'a [Mark], theme: &Theme) -> Vec<ResolvedMark<'a>> {
    let slots = label_slots(marks.iter());
    resolve_panel_marks(marks, theme, &slots)
}

/// Resolves every panel in row-major order. Labels with automatic colors take slots in first
/// appearance order across the figure. Each panel's unlabelled automatic series then count
/// from the number of label slots. Explicit palette and fixed colors never register a label
/// or consume a slot.
pub(crate) fn resolve_figure_marks(figure: &Figure) -> Vec<Vec<ResolvedMark<'_>>> {
    let slots = label_slots(figure.panels().flat_map(|axes| axes.marks()));
    figure
        .panels()
        .map(|axes| resolve_panel_marks(axes.marks(), figure.theme(), &slots))
        .collect()
}

/// The label and requested color of every series, in mark order.
fn color_sources(mark: &Mark) -> Vec<(&str, ColorSpec)> {
    match mark {
        Mark::Line(m) => vec![(&m.options().label, m.options().color)],
        Mark::Scatter(m) => vec![(&m.options().label, m.options().color)],
        Mark::Area(m) => vec![(&m.options().label, m.options().color)],
        Mark::Bar(m) => vec![(&m.options().label, m.options().color)],
        Mark::GroupedBar(m) | Mark::StackedBar(m) => {
            m.series().map(|s| (s.label(), s.color())).collect()
        }
        Mark::Hist(m) => vec![(m.label(), m.color())],
        Mark::Step(m) => vec![(&m.options().label, m.options().color)],
        Mark::HLine(m) => vec![(&m.options().label, m.options().color)],
        Mark::VLine(m) => vec![(&m.options().label, m.options().color)],
        Mark::HSpan(m) | Mark::VSpan(m) => vec![(&m.options().label, m.options().color)],
        Mark::Band(m) => vec![(&m.options().label, m.options().color)],
        Mark::ErrorBars(m) => vec![(&m.options().label, m.options().color)],
    }
}

/// The palette slots registered by labelled automatic series, in first appearance order.
fn label_slots<'a>(marks: impl Iterator<Item = &'a Mark>) -> BTreeMap<&'a str, usize> {
    let mut slots = BTreeMap::new();
    for (label, color) in marks.flat_map(color_sources) {
        if !label.is_empty() && matches!(color, ColorSpec::Auto(_)) {
            let next = slots.len();
            slots.entry(label).or_insert(next);
        }
    }
    slots
}

/// Resolves a panel against the label slots of its figure.
fn resolve_panel_marks<'a>(
    marks: &'a [Mark],
    theme: &Theme,
    slots: &BTreeMap<&str, usize>,
) -> Vec<ResolvedMark<'a>> {
    let palette = theme.palette();
    let mut next_auto = slots.len();
    let mut color_of = |spec: ColorSpec, label: &str| {
        let slot = if matches!(spec, ColorSpec::Auto(_)) {
            if let Some(&slot) = slots.get(label) {
                slot
            } else {
                let slot = next_auto;
                next_auto += 1;
                slot
            }
        } else {
            0
        };
        spec.resolve(palette, slot)
    };
    marks
        .iter()
        .map(|mark| match mark {
            Mark::Line(mark) => ResolvedMark::Line {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
                width: theme.drawn_line_width(mark.options().width),
            },
            Mark::Scatter(mark) => ResolvedMark::Scatter {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
                size: theme.drawn_marker_size(mark.options().size),
            },
            Mark::Area(mark) => ResolvedMark::Area {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
                width: theme.drawn_line_width(mark.options().width),
            },
            Mark::Bar(mark) => ResolvedMark::Bar {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
            },
            Mark::GroupedBar(mark) => ResolvedMark::Grouped {
                mark,
                series: mark
                    .series()
                    .map(|view| (view, color_of(view.color(), view.label())))
                    .collect(),
            },
            Mark::StackedBar(mark) => {
                let series: Box<[_]> = mark
                    .series()
                    .map(|view| (view, color_of(view.color(), view.label())))
                    .collect();
                let heights: Vec<&[f64]> = series.iter().map(|(view, _)| view.values()).collect();
                ResolvedMark::Stacked {
                    mark,
                    stacks: Stacks::new(mark.categories().len(), &heights),
                    series,
                }
            }
            Mark::Hist(mark) => ResolvedMark::Hist {
                mark,
                histogram: Histogram::from_values(mark.values(), mark.bin_count()),
                color: color_of(mark.color(), mark.label()),
            },
            Mark::Step(mark) => ResolvedMark::Step {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
                width: theme.drawn_line_width(mark.options().width),
            },
            Mark::HLine(mark) => ResolvedMark::HLine {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
                width: theme.drawn_line_width(mark.options().width),
            },
            Mark::VLine(mark) => ResolvedMark::VLine {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
                width: theme.drawn_line_width(mark.options().width),
            },
            Mark::HSpan(mark) => ResolvedMark::HSpan {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
            },
            Mark::VSpan(mark) => ResolvedMark::VSpan {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
            },
            Mark::Band(mark) => ResolvedMark::Band {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
            },
            Mark::ErrorBars(mark) => ResolvedMark::ErrorBars {
                mark,
                color: color_of(mark.options().color, &mark.options().label),
                width: theme.drawn_line_width(mark.options().width),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AreaOptions, BandOptions, BarOptions, Figure, HistOptions, LineOptions, ScatterOptions,
        Series, Step, StepOptions,
    };
    use crate::{AxesMut, FontLibrary, LogBase, MIN_LOG_VALUE, ScaleKind, Theme, TickFormat};

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    fn cats(v: &[&str]) -> AxisData {
        AxisData::Categories(v.iter().map(|s| (*s).to_owned()).collect())
    }

    fn figure(x: ScaleKind, y: ScaleKind) -> Figure {
        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        figure.add_axes(x, y).unwrap();
        figure
    }

    fn axes(figure: &mut Figure) -> AxesMut<'_> {
        figure.single_axes_mut()
    }

    /// The resolved color of every series of `resolved`, in order. Colors do not depend on the
    /// axis limits.
    fn series_colors(resolved: &[ResolvedMark<'_>]) -> Vec<Rgba> {
        series_keys(
            resolved,
            &Limits {
                x: AxisLimits::Category,
                y: ContinuousLimits {
                    transform: Transform::Linear,
                    explicit: None,
                    ticks: &AxisTicks::default(),
                },
            },
        )
        .iter()
        .map(|key| key.color)
        .collect()
    }

    /// The key of every series of `resolved`, in order, on axes with the limits `limits`.
    fn series_keys<'a>(resolved: &[ResolvedMark<'a>], limits: &Limits) -> Vec<SeriesKey<'a>> {
        resolved.iter().flat_map(|mark| mark.keys(limits)).collect()
    }

    /// Both axes of `axes`, resolved from its marks.
    fn resolve_axes(axes: &Axes, x_target: usize, y_target: usize) -> Resolved {
        let marks = resolve_marks(axes.marks(), &Theme::pigment());
        let (x, y) = TickRoom::unlimited(x_target, y_target, FontLibrary::global());
        resolve(&marks, &Limits::of(axes), &x, &y)
    }

    fn domains(figure: &Figure) -> ((f64, f64), (f64, f64)) {
        let r = resolve_axes(figure.single_axes(), 5, 4);
        (r.x.continuous().unwrap().domain(), r.y.scale.domain())
    }

    const LIN: ScaleKind = ScaleKind::Linear;

    /// The nice domain `lo..hi` padded by 5 % of its span on each side.
    fn pad(lo: f64, hi: f64) -> (f64, f64) {
        let p = 0.05 * (hi - lo);
        (lo - p, hi + p)
    }

    /// The log nice domain `lo..hi` padded by 5 % of its span in log space on each side.
    fn pad_log(lo: f64, hi: f64) -> (f64, f64) {
        let factor = (hi / lo).powf(0.05);
        (lo / factor, hi * factor)
    }

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        let near = |x: f64, y: f64| (x - y).abs() <= 1e-12 * x.abs().max(y.abs()).max(1.0);
        near(a.0, b.0) && near(a.1, b.1)
    }

    fn labels(ticks: &[AxisTick]) -> Vec<&str> {
        ticks.iter().map(|t| t.label.as_str()).collect()
    }

    /// The position of the first and last tick on a padded nice axis: 0.05 / 1.1 of the axis
    /// in from each end.
    fn assert_end_ticks_inset(ticks: &[AxisTick]) {
        let inset = 0.05 / 1.1;
        let (first, last) = (ticks.first().unwrap(), ticks.last().unwrap());
        assert!((first.pos.get() - inset).abs() < 1e-9, "{first:?}");
        assert!((last.pos.get() - (1.0 - inset)).abs() < 1e-9, "{last:?}");
    }

    #[test]
    fn all_line_marks_use_exact_x_extent() {
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .line(
                nums(&[0.3, 1.0, 9.7]),
                &[1.0, 2.0, 3.0],
                LineOptions::default(),
            )
            .unwrap()
            .line(
                nums(&[f64::NAN, 2.0, 7.1]),
                &[0.5, 2.5, 3.3],
                LineOptions::default(),
            )
            .unwrap();
        let ((x0, x1), (y0, y1)) = domains(&f);
        assert_eq!((x0, x1), (0.3, 9.7));
        // y is still widened to nice limits.
        assert!(y0 <= 0.5 && y1 >= 3.3);
        assert_ne!((y0, y1), (0.5, 3.3));

        // Adding a scatter turns the exception off: x gets the nice limits 0..10, padded.
        axes(&mut f)
            .scatter(nums(&[0.3]), &[1.0], ScatterOptions::default())
            .unwrap();
        let ((x0, x1), _) = domains(&f);
        assert!(close((x0, x1), pad(0.0, 10.0)), "{x0} {x1}");
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.x.ticks), ["0.0", "2.5", "5.0", "7.5", "10.0"]);
    }

    #[test]
    fn scatter_uses_nice_limits() {
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(
                nums(&[12.0, 47.0, 88.0]),
                &[3.2, 18.0, 36.5],
                ScatterOptions::default(),
            )
            .unwrap();
        // The nice limits 10..90 and 0..40, padded by 5 % of their span.
        let ((x0, x1), (y0, y1)) = domains(&f);
        assert!(close((x0, x1), pad(10.0, 90.0)), "{x0} {x1}");
        assert!(close((y0, y1), pad(0.0, 40.0)), "{y0} {y1}");
        // The ticks are the set that produced the nice limits, so the outermost ones sit at the
        // nice limits, just inside the padded domain: x is 10, 30, ..., 90, not 25, 50, 75.
        let r = resolve_axes(f.single_axes(), 5, 4);
        for ticks in [&r.x.ticks, &r.y.ticks] {
            assert_end_ticks_inset(ticks);
        }
        assert_eq!(labels(&r.x.ticks), ["10", "30", "50", "70", "90"]);
        assert_eq!(labels(&r.y.ticks), ["0", "10", "20", "30", "40"]);

        // The same on a log axis.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(log, LIN);
        axes(&mut f)
            .scatter(nums(&[12.0, 88.0]), &[1.0, 2.0], ScatterOptions::default())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        let domain = r.x.continuous().unwrap().domain();
        assert!(close(domain, pad_log(10.0, 100.0)), "{domain:?}");
        assert_eq!(labels(&r.x.ticks), ["10", "100"]);
        assert_end_ticks_inset(&r.x.ticks);
    }

    #[test]
    fn explicit_limits_win() {
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&[12.0, 88.0]), &[3.2, 36.5], ScatterOptions::default())
            .unwrap()
            .xlim(0.5, 9.5)
            .unwrap()
            .ylim(-1.3, 2.7)
            .unwrap();
        let ((x0, x1), (y0, y1)) = domains(&f);
        assert_eq!((x0, x1), (0.5, 9.5));
        assert_eq!((y0, y1), (-1.3, 2.7));
        // Every tick lies inside the explicit limits.
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert!(r.x.ticks.iter().all(|t| (0.0..=1.0).contains(&t.pos.get())));
        assert!(r.y.ticks.iter().all(|t| (0.0..=1.0).contains(&t.pos.get())));
        // 0 is inside -1.3..2.7, so the baseline sits at its normalized position.
        assert!((r.baseline.get() - 1.3 / 4.0).abs() < 1e-12);
    }

    #[test]
    fn empty_axes_fall_back_to_unit_range() {
        let f = figure(LIN, LIN);
        assert_eq!(domains(&f), ((0.0, 1.0), (0.0, 1.0)));

        // All-NaN data counts as empty.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&[f64::NAN]), &[f64::NAN], ScatterOptions::default())
            .unwrap();
        assert_eq!(domains(&f), ((0.0, 1.0), (0.0, 1.0)));

        let log = ScaleKind::Log { base: LogBase::TEN };
        let f = figure(log, log);
        assert_eq!(domains(&f), ((1.0, 10.0), (1.0, 10.0)));
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(r.baseline.get(), 0.0);

        // Empty scatter data is empty data too.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&[]), &[], ScatterOptions::default())
            .unwrap();
        assert_eq!(domains(&f), ((0.0, 1.0), (0.0, 1.0)));
    }

    #[test]
    fn empty_log_axes_keep_the_fallback_range() {
        // The fallback is not widened with `nice`, so both axes agree whatever the base.
        for base in [2.0, 100.0] {
            let log = ScaleKind::Log {
                base: LogBase::new(base).unwrap(),
            };
            let f = figure(log, log);
            assert_eq!(domains(&f), ((1.0, 10.0), (1.0, 10.0)), "base {base}");

            // All-NaN data, on a line and on a scatter.
            let mut f = figure(log, log);
            axes(&mut f)
                .line(nums(&[f64::NAN]), &[f64::NAN], LineOptions::default())
                .unwrap();
            assert_eq!(domains(&f), ((1.0, 10.0), (1.0, 10.0)), "base {base}");
            axes(&mut f)
                .scatter(
                    nums(&[f64::NAN, 2.0]),
                    &[f64::NAN, f64::NAN],
                    ScatterOptions::default(),
                )
                .unwrap();
            let ((_, _), (y0, y1)) = domains(&f);
            assert_eq!((y0, y1), (1.0, 10.0), "base {base}");

            // Empty scatter.
            let mut f = figure(log, log);
            axes(&mut f)
                .scatter(nums(&[]), &[], ScatterOptions::default())
                .unwrap();
            assert_eq!(domains(&f), ((1.0, 10.0), (1.0, 10.0)), "base {base}");
        }
    }

    #[test]
    fn log_axis_limits() {
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(LIN, log);
        axes(&mut f)
            .scatter(nums(&[1.0, 2.0]), &[3.0, 4.2e4], ScatterOptions::default())
            .unwrap();
        let (_, (y0, y1)) = domains(&f);
        assert!(close((y0, y1), pad_log(1.0, 1e5)), "{y0} {y1}");
        let r = resolve_axes(f.single_axes(), 5, 4);
        // 0 is never inside a log range: the baseline sits at the nice lower limit, the
        // lowest tick, inside the padded domain.
        assert_eq!(r.baseline, r.y.ticks[0].pos);
        assert!(
            (r.baseline.get() - 0.05 / 1.1).abs() < 1e-9,
            "{:?}",
            r.baseline
        );
        assert_eq!(
            labels(&r.y.ticks),
            ["1", "10", "100", "1000", "10000", "100000"]
        );
        assert!(r.y.ticks.windows(2).all(|w| w[0].pos < w[1].pos));

        // Explicit log limits are used as given.
        axes(&mut f).ylim(2.0, 500.0).unwrap();
        let (_, (y0, y1)) = domains(&f);
        assert_eq!((y0, y1), (2.0, 500.0));

        // A log x axis of lines keeps the exact extent.
        let mut f = figure(log, LIN);
        axes(&mut f)
            .line(nums(&[3.0, 700.0]), &[1.0, 2.0], LineOptions::default())
            .unwrap();
        let ((x0, x1), _) = domains(&f);
        assert_eq!((x0, x1), (3.0, 700.0));
    }

    #[test]
    fn category_order_is_first_appearance_across_marks() {
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .line(
                cats(&["b", "a", "b"]),
                &[1.0, 2.0, 3.0],
                LineOptions::default(),
            )
            .unwrap()
            .scatter(
                cats(&["c", "a", "d"]),
                &[1.0, 2.0, 3.0],
                ScatterOptions::default(),
            )
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert!(r.x.is_category());
        let labels: Vec<&str> = r.x.ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["b", "a", "c", "d"]);
        let pos: Vec<f64> = r.x.ticks.iter().map(|t| t.pos.get()).collect();
        assert_eq!(pos, [0.125, 0.375, 0.625, 0.875]);
    }

    #[test]
    fn resolved_marks_pair_each_mark_with_its_color_in_order() {
        use crate::{Alpha, Rgba};
        let mut f = figure(LIN, LIN);
        let palette = f.theme().palette().to_vec();
        let fixed = Rgba::opaque(9, 8, 7);
        let half = Alpha::new(0.5).unwrap();
        let specs = [
            ColorSpec::Palette {
                index: 3,
                alpha: Alpha::OPAQUE,
            },
            ColorSpec::AUTO,
            ColorSpec::Fixed(fixed),
            ColorSpec::AUTO.with_alpha(half),
            ColorSpec::Palette {
                index: 0,
                alpha: half,
            },
            ColorSpec::AUTO,
        ];
        let mut a = axes(&mut f);
        for (i, color) in specs.into_iter().enumerate() {
            // Lines and scatters alternate; both take Auto slots from the same sequence.
            if i % 2 == 0 {
                let options = ScatterOptions {
                    color,
                    ..ScatterOptions::default()
                };
                a.scatter(nums(&[i as f64]), &[1.0], options).unwrap();
            } else {
                let options = LineOptions {
                    color,
                    ..LineOptions::default()
                };
                a.line(nums(&[i as f64]), &[1.0], options).unwrap();
            }
        }
        let marks = f.single_axes().marks();
        let resolved = resolve_marks(marks, f.theme());
        assert_eq!(resolved.len(), marks.len());
        for (record, mark) in resolved.iter().zip(marks) {
            let same = match (record, mark) {
                (ResolvedMark::Line { mark: record, .. }, Mark::Line(mark)) => {
                    std::ptr::eq(*record, mark)
                }
                (ResolvedMark::Scatter { mark: record, .. }, Mark::Scatter(mark)) => {
                    std::ptr::eq(*record, mark)
                }
                _ => false,
            };
            assert!(same, "{record:?} does not hold {mark:?}");
        }
        let colors = series_colors(&resolved);
        assert_eq!(
            colors,
            [
                palette[3],
                palette[0],
                fixed,
                palette[1].with_alpha(half),
                palette[0].with_alpha(half),
                palette[2],
            ]
        );
        assert!(resolve_marks(&[], f.theme()).is_empty());
    }

    #[test]
    fn widths_and_sizes_resolve_against_the_theme_once() {
        use crate::SizeSpec;
        let line = |width: SizeSpec| LineOptions {
            width,
            ..LineOptions::default()
        };
        let scatter = |size: SizeSpec| ScatterOptions {
            size,
            ..ScatterOptions::default()
        };
        let specs = [
            SizeSpec::Auto,
            SizeSpec::times(2.5).unwrap(),
            SizeSpec::times(0.5).unwrap(),
            SizeSpec::pt(3.0).unwrap(),
        ];
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let (w, d) = (theme.line_width(), theme.marker_size());
            let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
            let mut a = f.add_axes(LIN, LIN).unwrap();
            for spec in specs {
                a.line(nums(&[0.0, 1.0]), &[1.0, 2.0], line(spec))
                    .unwrap()
                    .area(nums(&[0.0, 1.0]), &[1.0, 2.0], line(spec))
                    .unwrap()
                    .scatter(nums(&[0.5]), &[1.5], scatter(spec))
                    .unwrap();
            }
            let resolved = resolve_marks(f.single_axes().marks(), &theme);
            let expected = [(w, d), (2.5 * w, 2.5 * d), (0.5 * w, 0.5 * d), (3.0, 3.0)];
            for (records, (width, size)) in resolved.chunks(3).zip(expected) {
                match records {
                    [
                        ResolvedMark::Line { width: lw, .. },
                        ResolvedMark::Area { width: aw, .. },
                        ResolvedMark::Scatter { size: sd, .. },
                    ] => assert_eq!((*lw, *aw, *sd), (width, width, size)),
                    other => panic!("unexpected records {other:?}"),
                }
                assert_eq!(records[0].line_width(), Some(width));
                assert_eq!(records[1].line_width(), Some(width));
                assert_eq!(records[2].line_width(), None);
            }
        }

        // The drawn size is capped at 10 000 pt, as the theme's own values are.
        let theme = Theme::graphite()
            .with_line_width(5000.0)
            .unwrap()
            .with_marker_size(3e38)
            .unwrap();
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(LIN, LIN)
            .unwrap()
            .line(nums(&[0.0]), &[1.0], line(SizeSpec::times(10.0).unwrap()))
            .unwrap()
            .scatter(
                nums(&[0.0]),
                &[1.0],
                scatter(SizeSpec::times(10.0).unwrap()),
            )
            .unwrap();
        match resolve_marks(f.single_axes().marks(), &theme).as_slice() {
            [
                ResolvedMark::Line { width, .. },
                ResolvedMark::Scatter { size, .. },
            ] => assert_eq!((*width, *size), (1.0e4, 1.0e4)),
            other => panic!("unexpected records {other:?}"),
        }
    }

    #[test]
    fn keys_carry_label_color_and_drawing() {
        use crate::{Dash, Marker};
        let mut f = figure(LIN, LIN);
        let palette = f.theme().palette().to_vec();
        axes(&mut f)
            .line(
                nums(&[0.0]),
                &[1.0],
                LineOptions {
                    label: "trend".to_owned(),
                    dash: Dash::Dashed,
                    ..LineOptions::default()
                },
            )
            .unwrap()
            .scatter(
                nums(&[0.0]),
                &[1.0],
                ScatterOptions {
                    marker: Marker::Square,
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        let resolved = resolve_marks(f.single_axes().marks(), f.theme());
        let keys = series_keys(&resolved, &Limits::of(f.single_axes()));
        assert_eq!(
            keys,
            [
                SeriesKey {
                    label: "trend",
                    color: palette[0],
                    kind: SeriesKind::Line {
                        dash: Dash::Dashed,
                        width: 2.0,
                    },
                    anchor: Some(Anchor {
                        x: AnchorX::Number(0.0),
                        y: 1.0,
                        end: EndKind::Point,
                    }),
                },
                SeriesKey {
                    label: "",
                    color: palette[1],
                    kind: SeriesKind::Points {
                        marker: Marker::Square,
                        size: 6.0,
                    },
                    anchor: None,
                },
            ]
        );
    }

    #[test]
    fn baseline_sits_at_zero_or_the_explicit_low_limit() {
        let mut f = figure(LIN, LIN);
        axes(&mut f).ylim(-10.0, 30.0).unwrap();
        assert_eq!(resolve_axes(f.single_axes(), 5, 4).baseline.get(), 0.25);
        // Explicit limits are not padded: their low limit is the plot bottom.
        axes(&mut f).ylim(5.0, 30.0).unwrap();
        assert_eq!(resolve_axes(f.single_axes(), 5, 4).baseline.get(), 0.0);
        axes(&mut f).ylim(-30.0, -5.0).unwrap();
        assert_eq!(resolve_axes(f.single_axes(), 5, 4).baseline.get(), 0.0);
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(LIN, log);
        axes(&mut f)
            .line(nums(&[0.0, 1.0]), &[3.0, 4.2e4], LineOptions::default())
            .unwrap()
            .ylim(2.0, 5e4)
            .unwrap();
        assert_eq!(resolve_axes(f.single_axes(), 5, 4).baseline.get(), 0.0);
    }

    #[test]
    fn lowest_tick_lies_strictly_below_the_data() {
        let log = ScaleKind::Log { base: LogBase::TEN };
        let line_axes = |y_scale, ys: &[f64]| {
            let mut f = figure(LIN, y_scale);
            let xs: Vec<f64> = (0..ys.len()).map(|i| i as f64).collect();
            axes(&mut f)
                .line(nums(&xs), ys, LineOptions::default())
                .unwrap();
            f
        };
        let lowest = |f: &Figure| {
            let r = resolve_axes(f.single_axes(), 5, 4);
            assert_eq!(r.baseline, r.y.ticks[0].pos);
            r.y.ticks[0].label.clone()
        };
        // A minimum on a nice tick: the limit moves one tick step down, linear and log.
        assert_eq!(lowest(&line_axes(LIN, &[20.0, 35.0, 40.0])), "15");
        assert_eq!(lowest(&line_axes(log, &[1.0, 40.0])), "0.1");
        assert_eq!(lowest(&line_axes(log, &[10.0, 40.0])), "1");
        // A minimum within 5 % of the span above the lowest tick counts as on it: 20.9 on
        // 20..40, and on log 1.2 on 1..100 (a factor of 100^0.05, about 1.26).
        assert_eq!(lowest(&line_axes(LIN, &[20.9, 35.0, 40.0])), "15");
        assert_eq!(lowest(&line_axes(log, &[1.2, 40.0])), "0.1");
        // A minimum further above the lowest tick: unchanged.
        assert_eq!(lowest(&line_axes(LIN, &[21.0, 35.0, 40.0])), "20");
        assert_eq!(lowest(&line_axes(LIN, &[22.0, 35.0, 40.0])), "20");
        assert_eq!(lowest(&line_axes(log, &[2.0, 40.0])), "1");
        // A linear domain that holds 0 keeps 0 as its limit: the baseline is at 0 already.
        let f = line_axes(LIN, &[0.0, 30.0]);
        assert!(close(y_domain(&f), pad(0.0, 30.0)), "{:?}", y_domain(&f));
        let f = bar_figure(LIN, &[0.0, 40.0]);
        assert_eq!(y_domain(&f), (0.0, 42.0));
        // Explicit limits are used as given.
        let mut f = line_axes(LIN, &[20.0, 40.0]);
        axes(&mut f).ylim(20.0, 40.0).unwrap();
        assert_eq!(y_domain(&f), (20.0, 40.0));
        // A move that would leave the supported magnitudes keeps the old limit.
        let f = line_axes(log, &[MIN_LOG_VALUE, 1.0]);
        assert_eq!(y_domain(&f).0, MIN_LOG_VALUE);
        let f = line_axes(LIN, &[-1e300, 1e300]);
        assert_eq!(y_domain(&f).0, -1e300);
        // Log bars: the smallest height counts.
        let f = bar_figure(log, &[10.0, 1000.0]);
        assert_eq!(lowest(&f), "1");
        // The x axis is not affected: x from 35 to 65 keeps its lowest tick at 35.
        let (_, ys) = sinusoid();
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&ys), &ys, ScatterOptions::default())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 4, 4);
        assert_eq!(labels(&r.x.ticks)[0], "35");
        assert_eq!(labels(&r.y.ticks)[0], "25");
    }

    #[test]
    fn lowering_a_log_limit_keeps_the_ticks_evenly_spaced_and_distinct() {
        // 1..1e100 on base 10 has its powers thinned; the lower limit moves to 0.1 and the
        // powers are chosen again for 0.1..1e100, so they stay evenly spaced in log space and
        // none crowds the new end. Bases near 1 keep distinct positions too.
        for base in [10.0, 1.000_001, 1.0 + f64::EPSILON] {
            let log = ScaleKind::Log {
                base: LogBase::new(base).unwrap(),
            };
            let mut f = figure(LIN, log);
            axes(&mut f)
                .line(nums(&[0.0, 1.0]), &[1.0, 1e100], LineOptions::default())
                .unwrap();
            let r = resolve_axes(f.single_axes(), 5, 4);
            let pos: Vec<f64> = r.y.ticks.iter().map(|t| t.pos.get()).collect();
            assert!(pos.len() >= 2, "base {base}: {:?}", r.y.ticks);
            assert!(
                pos.windows(2).all(|w| w[0] < w[1]),
                "base {base}: {:?}",
                r.y.ticks
            );
            assert_eq!(r.baseline, r.y.ticks[0].pos, "base {base}");
            // At base 1 + EPSILON one step down is a factor of 1 + EPSILON, too small to move
            // any position; every other base moves the limit visibly below the data.
            let data_low = r.y.scale.normalize(1.0);
            if base > 1.0 + f64::EPSILON {
                assert!(r.baseline.get() < data_low, "base {base}: {:?}", r.y.ticks);
            } else {
                assert!(r.baseline.get() <= data_low, "base {base}: {:?}", r.y.ticks);
            }
            if base == 10.0 {
                assert_eq!(r.y.ticks[0].label, "1e\u{2212}1");
                let gaps: Vec<f64> = pos.windows(2).map(|w| w[1] - w[0]).collect();
                let (min, max) = gaps
                    .iter()
                    .fold((f64::MAX, 0.0_f64), |(a, b), &g| (a.min(g), b.max(g)));
                assert!(max - min < 0.02, "uneven spacing: {gaps:?}");
            }
        }
    }

    #[test]
    fn lowering_works_at_extreme_linear_magnitudes() {
        // Spans far above or below the tick search's usual range still move their limit below
        // the data, positive and negative.
        for (low, high) in [
            (1e250, 2e250),
            (1e-250, 2e-250),
            (-2e250, -1e250),
            (-2e-250, -1e-250),
        ] {
            let mut f = figure(LIN, LIN);
            axes(&mut f)
                .line(nums(&[0.0, 1.0]), &[low, high], LineOptions::default())
                .unwrap();
            let r = resolve_axes(f.single_axes(), 5, 4);
            let data_low = r.y.scale.normalize(low);
            assert!(
                r.baseline.get() < data_low,
                "{low:e}..{high:e}: {:?}",
                r.y.ticks
            );
            assert_eq!(r.baseline, r.y.ticks[0].pos);
            let values: Vec<&str> = labels(&r.y.ticks);
            let unique: std::collections::HashSet<&&str> = values.iter().collect();
            assert_eq!(unique.len(), values.len(), "{values:?}");
        }
    }

    #[test]
    fn baseline_sits_at_zero_or_the_nice_lower_limit() {
        // Auto limits that hold 0: the baseline is at 0, wherever 0 is.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .line(nums(&[0.0, 1.0]), &[-12.0, 36.5], LineOptions::default())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        let scale = r.y.scale;
        assert_eq!(r.baseline.get(), scale.normalize(0.0));
        assert!(r.baseline.get() > r.y.ticks[0].pos.get());
        // Auto limits away from 0, above or below it: the baseline is at the lowest tick, the
        // nice lower limit, not at the padded plot bottom.
        let (xs, ys) = sinusoid();
        for ys in [ys.clone(), ys.iter().map(|y| -y).collect()] {
            let mut f = figure(LIN, LIN);
            axes(&mut f)
                .line(nums(&xs), &ys, LineOptions::default())
                .unwrap();
            let r = resolve_axes(f.single_axes(), 5, 4);
            let lowest = &r.y.ticks[0];
            assert_eq!(r.baseline, lowest.pos);
            assert!(
                (r.baseline.get() - 0.05 / 1.1).abs() < 1e-9,
                "{:?}",
                r.baseline
            );
            // The extreme lands on a nice tick, so the limit moves one step below it.
            let expected = if ys[0] > 0.0 { "25" } else { "\u{2212}75" };
            assert_eq!(lowest.label, expected);
        }
        // Log auto limits: the lowest power of the base, 10 here.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(LIN, log);
        axes(&mut f)
            .scatter(nums(&[0.0, 1.0]), &[20.0, 880.0], ScatterOptions::default())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(r.y.ticks[0].label, "10");
        assert_eq!(r.baseline, r.y.ticks[0].pos);
        assert!(r.baseline.get() > 0.0);
        // A single value is not padded, so its nice lower limit is the plot bottom.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&[1.0]), &[20.0], ScatterOptions::default())
            .unwrap();
        assert_eq!(resolve_axes(f.single_axes(), 5, 4).baseline.get(), 0.0);
    }

    /// `50 + 15 sin(t)` over two periods: its extent is 35 to 65, which are nice values.
    fn sinusoid() -> (Vec<f64>, Vec<f64>) {
        let xs: Vec<f64> = (0..=96).map(|i| f64::from(i) / 8.0).collect();
        let ys: Vec<f64> = (0..=96)
            .map(|i| 50.0 + 15.0 * (f64::from(i) * std::f64::consts::PI / 24.0).sin())
            .collect();
        (xs, ys)
    }

    #[test]
    fn auto_limits_pad_the_nice_domain_and_keep_its_ticks() {
        let (xs, ys) = sinusoid();
        let (lo, hi) = extent(ys.iter().copied()).unwrap();
        assert_eq!((lo, hi), (35.0, 65.0));
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .line(nums(&xs), &ys, LineOptions::default())
            .unwrap();
        let ((x0, x1), (y0, y1)) = domains(&f);
        // 35..65 is already nice, and the minimum lands on its lowest tick, so the lower limit
        // moves down one step to 25. The domain 25..65 then grows by 2 on each side, so the
        // line's minimum and maximum no longer sit on the plot edge, and the ticks stay 25..65.
        assert_ne!((y0, y1), (25.0, 65.0));
        assert!(close((y0, y1), (23.0, 67.0)), "{y0} {y1}");
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.y.ticks), ["25", "35", "45", "55", "65"]);
        assert_end_ticks_inset(&r.y.ticks);
        // The x axis of an all-line axes keeps the exact extent, unpadded.
        assert_eq!((x0, x1), (0.0, 12.0));
        assert_eq!(r.x.ticks.first().map(|t| t.pos.get()), Some(0.0));
        assert_eq!(r.x.ticks.last().map(|t| t.pos.get()), Some(1.0));
    }

    #[test]
    fn log_auto_limits_pad_in_log_space() {
        // The nice x limits 10..1000 span two decades; 5 % of that is a factor of 10^0.1 on
        // each side. The ticks stay the powers inside the nice limits. On y the minimum lands
        // on the lowest tick, so the lower limit moves down a decade: 1..1000, three decades,
        // padded by a factor of 10^0.15.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(log, log);
        axes(&mut f)
            .scatter(
                nums(&[10.0, 1000.0]),
                &[10.0, 1000.0],
                ScatterOptions::default(),
            )
            .unwrap();
        let (x, y) = domains(&f);
        let factor = 10f64.powf(0.1);
        assert!(close(x, (10.0 / factor, 1000.0 * factor)), "{x:?}");
        let factor = 10f64.powf(0.15);
        assert!(close(y, (1.0 / factor, 1000.0 * factor)), "{y:?}");
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.x.ticks), ["10", "100", "1000"]);
        assert_eq!(labels(&r.y.ticks), ["1", "10", "100", "1000"]);
        assert_end_ticks_inset(&r.y.ticks);
    }

    #[test]
    fn explicit_limits_are_never_padded() {
        let (xs, ys) = sinusoid();
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&xs), &ys, ScatterOptions::default())
            .unwrap()
            .xlim(0.0, 12.0)
            .unwrap()
            .ylim(35.0, 65.0)
            .unwrap();
        assert_eq!(domains(&f), ((0.0, 12.0), (35.0, 65.0)));

        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(log, log);
        axes(&mut f)
            .scatter(
                nums(&[10.0, 1000.0]),
                &[10.0, 1000.0],
                ScatterOptions::default(),
            )
            .unwrap()
            .xlim(10.0, 1000.0)
            .unwrap()
            .ylim(10.0, 1000.0)
            .unwrap();
        assert_eq!(domains(&f), ((10.0, 1000.0), (10.0, 1000.0)));
    }

    #[test]
    fn exact_x_limits_are_not_padded() {
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .line(nums(&[35.0, 65.0]), &[1.0, 2.0], LineOptions::default())
            .unwrap();
        let ((x0, x1), _) = domains(&f);
        assert_eq!((x0, x1), (35.0, 65.0));

        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(log, LIN);
        axes(&mut f)
            .line(nums(&[10.0, 1000.0]), &[1.0, 2.0], LineOptions::default())
            .unwrap();
        let ((x0, x1), _) = domains(&f);
        assert_eq!((x0, x1), (10.0, 1000.0));
    }

    #[test]
    fn degenerate_and_empty_extents_keep_their_handling() {
        // A single value is padded by `from_extent` alone, then rounded: 20 becomes 19..21.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&[20.0]), &[20.0], ScatterOptions::default())
            .unwrap();
        let single = ContinuousScale::from_extent(Transform::Linear, Some((20.0, 20.0)), "t")
            .unwrap()
            .nice(5);
        let ((x0, x1), _) = domains(&f);
        assert_eq!((x0, x1), single.domain());
        // An extent narrower than the minimum width counts as a single value, unpadded.
        let narrow = (1e9, 1e9 + 1e-3);
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(
                nums(&[narrow.0, narrow.1]),
                &[1.0, 2.0],
                ScatterOptions::default(),
            )
            .unwrap();
        let expected = ContinuousScale::from_extent(Transform::Linear, Some(narrow), "t")
            .unwrap()
            .nice(5);
        let ((x0, x1), _) = domains(&f);
        assert_eq!((x0, x1), expected.domain());
        // Empty data still falls back to the unit range.
        let f = figure(LIN, LIN);
        assert_eq!(domains(&f), ((0.0, 1.0), (0.0, 1.0)));
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    /// Category x axes holding one bar mark of `heights`, on a y axis of `y`.
    fn bar_figure(y: ScaleKind, heights: &[f64]) -> Figure {
        let categories: Vec<String> = (0..heights.len()).map(|i| format!("c{i}")).collect();
        let mut f = figure(ScaleKind::Category, y);
        axes(&mut f)
            .bar(categories, heights, BarOptions::default())
            .unwrap();
        f
    }

    fn y_domain(figure: &Figure) -> (f64, f64) {
        resolve_axes(figure.single_axes(), 5, 4).y.scale.domain()
    }

    #[test]
    fn bar_y_extent_includes_zero() {
        // Positive bars: the nice limits 0..40, padded above only, so the bars start on the
        // bottom edge, where the baseline and the first tick sit.
        let f = bar_figure(LIN, &[12.0, 36.5]);
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(y_domain(&f), (0.0, 42.0));
        assert_eq!(labels(&r.y.ticks), ["0", "10", "20", "30", "40"]);
        assert_eq!(r.y.ticks[0].pos.get(), 0.0);
        assert_eq!(r.baseline.get(), 0.0);

        // Negative bars: the mirror image, padded below only.
        let f = bar_figure(LIN, &[-12.0, -36.5]);
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(y_domain(&f), (-42.0, 0.0));
        assert_eq!(r.baseline.get(), 1.0);

        // Bars on both sides of 0: both sides padded as usual.
        let f = bar_figure(LIN, &[-12.0, 36.5]);
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_end_ticks_inset(&r.y.ticks);
        assert!(labels(&r.y.ticks).contains(&"0"));

        // A line below 0 on the same axes moves the lower limit off 0, so it is padded.
        let mut f = bar_figure(LIN, &[12.0, 36.5]);
        axes(&mut f)
            .line(
                AxisData::Categories(names(&["c0", "c1"])),
                &[-9.0, 1.0],
                LineOptions::default(),
            )
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_end_ticks_inset(&r.y.ticks);

        // Without bars, data away from 0 keeps its own nice limits.
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .line(
                AxisData::Categories(names(&["a", "b"])),
                &[12.0, 36.5],
                LineOptions::default(),
            )
            .unwrap();
        assert!(y_domain(&f).0 > 0.0, "{:?}", y_domain(&f));

        // Log y: 0 cannot be shown, so the axis fits the heights and pads both sides; the
        // baseline is the nice lower limit, the lowest tick.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let f = bar_figure(log, &[3.0, 4.2e4]);
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert!(close(y_domain(&f), pad_log(1.0, 1e5)), "{:?}", y_domain(&f));
        assert_eq!(r.baseline, r.y.ticks[0].pos);
        assert_eq!(r.y.ticks[0].label, "1");

        // All-NaN bars have no extent: the axis falls back to 0..1, as for other marks.
        let f = bar_figure(LIN, &[f64::NAN, f64::NAN]);
        assert_eq!(y_domain(&f), (0.0, 1.0));
        // Explicit limits are used as given, even when they exclude 0.
        let mut f = bar_figure(LIN, &[12.0, 36.5]);
        axes(&mut f).ylim(10.0, 20.0).unwrap();
        assert_eq!(y_domain(&f), (10.0, 20.0));
    }

    #[test]
    fn bar_categories_join_first_appearance() {
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .line(cats(&["b", "a"]), &[1.0, 2.0], LineOptions::default())
            .unwrap()
            .bar(
                names(&["c", "a", "d"]),
                &[1.0, 2.0, 3.0],
                BarOptions::default(),
            )
            .unwrap()
            .scatter(cats(&["e", "d"]), &[1.0, 2.0], ScatterOptions::default())
            .unwrap()
            .bar(names(&["f"]), &[1.0], BarOptions::default())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.x.ticks), ["b", "a", "c", "d", "e", "f"]);
    }

    #[test]
    fn each_bar_mark_takes_one_auto_color() {
        use crate::Alpha;
        let mut f = figure(ScaleKind::Category, LIN);
        let palette = f.theme().palette().to_vec();
        let fixed = Rgba::opaque(9, 8, 7);
        let color = |color: ColorSpec, label: &str| BarOptions {
            label: label.to_owned(),
            color,
        };
        axes(&mut f)
            .line(cats(&["a"]), &[1.0], LineOptions::default())
            .unwrap()
            .bar(
                names(&["a", "b"]),
                &[1.0, 2.0],
                color(ColorSpec::AUTO, "first"),
            )
            .unwrap()
            .bar(names(&["a"]), &[1.0], color(ColorSpec::Fixed(fixed), ""))
            .unwrap()
            .bar(
                names(&["c"]),
                &[1.0],
                color(
                    ColorSpec::Palette {
                        index: 6,
                        alpha: Alpha::OPAQUE,
                    },
                    "",
                ),
            )
            .unwrap()
            .bar(names(&["d"]), &[1.0], color(ColorSpec::AUTO, "last"))
            .unwrap();
        let resolved = resolve_marks(f.single_axes().marks(), f.theme());
        let colors = series_colors(&resolved);
        assert_eq!(
            colors,
            [palette[2], palette[0], fixed, palette[6], palette[1]]
        );
        let keys = series_keys(&resolved, &Limits::of(f.single_axes()));
        assert_eq!(
            keys[1],
            SeriesKey {
                label: "first",
                color: palette[0],
                kind: SeriesKind::Bar,
                anchor: None,
            }
        );
        assert_eq!(keys[4].label, "last");
    }

    fn series(color: ColorSpec, values: &[f64]) -> Series {
        Series {
            label: String::new(),
            values: values.to_vec(),
            color,
        }
    }

    fn auto(values: &[f64]) -> Series {
        series(ColorSpec::AUTO, values)
    }

    #[test]
    fn grouped_y_extent_includes_zero() {
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .grouped_bar(
                names(&["a", "b"]),
                vec![auto(&[12.0, f64::NAN]), auto(&[36.5, 3.0])],
            )
            .unwrap();
        assert_eq!(y_domain(&f), (0.0, 42.0));
        // The heights of every series count, not only the first's.
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .grouped_bar(names(&["a"]), vec![auto(&[-12.0]), auto(&[-36.5])])
            .unwrap();
        assert_eq!(y_domain(&f), (-42.0, 0.0));
    }

    #[test]
    fn all_zero_bars_have_the_same_y_domain() {
        let mut plain = figure(ScaleKind::Category, LIN);
        axes(&mut plain)
            .bar(names(&["a", "b"]), &[0.0, 0.0], BarOptions::default())
            .unwrap();
        let mut grouped = figure(ScaleKind::Category, LIN);
        axes(&mut grouped)
            .grouped_bar(
                names(&["a", "b"]),
                vec![auto(&[0.0, 0.0]), auto(&[0.0, 0.0])],
            )
            .unwrap();
        let mut stacked = figure(ScaleKind::Category, LIN);
        axes(&mut stacked)
            .stacked_bar(
                names(&["a", "b"]),
                vec![auto(&[0.0, 0.0]), auto(&[0.0, 0.0])],
            )
            .unwrap();
        assert_eq!(y_domain(&plain), y_domain(&grouped));
        assert_eq!(y_domain(&stacked), y_domain(&plain));
    }

    #[test]
    fn stacked_y_extent_spans_the_stacks_and_zero() {
        // Category a stacks to 12 + 24.5 = 36.5; nothing below 0, so 0 stays on the edge.
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .stacked_bar(
                names(&["a", "b"]),
                vec![auto(&[12.0, 5.0]), auto(&[24.5, f64::NAN])],
            )
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(y_domain(&f), (0.0, 42.0));
        assert_eq!(r.baseline.get(), 0.0);
        // All negative: mirrored.
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .stacked_bar(names(&["a"]), vec![auto(&[-12.0]), auto(&[-24.5])])
            .unwrap();
        assert_eq!(y_domain(&f), (-42.0, 0.0));
    }

    #[test]
    fn mixed_sign_stacks_are_padded_on_both_sides() {
        // Stacks reach 30 + 6 = 36 above 0 and -(12 + 7) = -19 below it: the nice limits
        // -20..40 end at neither 0, so `KeepZero` keeps neither side and both are padded.
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .stacked_bar(
                names(&["a", "b"]),
                vec![auto(&[30.0, -12.0]), auto(&[6.0, -7.0]), auto(&[-3.0, 2.0])],
            )
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        let (lo, hi) = y_domain(&f);
        assert!(lo < -19.0 && hi > 36.0, "{lo} {hi}");
        let nice = (
            r.y.ticks[0].label.clone(),
            r.y.ticks.last().unwrap().label.clone(),
        );
        assert_ne!(nice.0, "0");
        assert_ne!(nice.1, "0");
        assert_end_ticks_inset(&r.y.ticks);
        assert!(labels(&r.y.ticks).contains(&"0"));
        // The baseline sits at 0, inside the plot.
        assert!(r.baseline.get() > 0.0 && r.baseline.get() < 1.0);
    }

    #[test]
    fn stacked_log_y_extent_comes_from_positive_segment_ends() {
        // On a log axis the stacks start at the baseline. The domain is fitted to the
        // segment ends, 2..1500 here, and never to an extent that includes 0, which a log axis
        // cannot hold (it would fall back to the unit range 1..10).
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(ScaleKind::Category, log);
        axes(&mut f)
            .stacked_bar(
                names(&["a", "b"]),
                vec![auto(&[2.0, 500.0]), auto(&[f64::NAN, 1000.0])],
            )
            .unwrap();
        let domain = y_domain(&f);
        assert!(close(domain, pad_log(1.0, 1e4)), "{domain:?}");
        // The same domain as a line through exactly the segment ends.
        let mut g = figure(ScaleKind::Category, log);
        axes(&mut g)
            .line(
                cats(&["a", "b", "b"]),
                &[2.0, 500.0, 1500.0],
                LineOptions::default(),
            )
            .unwrap();
        assert_eq!(domain, y_domain(&g));
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(r.baseline, r.y.ticks[0].pos);
        assert_eq!(r.y.ticks[0].label, "1");
    }

    #[test]
    fn auto_colors_follow_series_order_across_marks() {
        use crate::Alpha;
        let mut f = figure(ScaleKind::Category, LIN);
        let palette = f.theme().palette().to_vec();
        let fixed = Rgba::opaque(9, 8, 7);
        let at = |index: usize| ColorSpec::Palette {
            index,
            alpha: Alpha::OPAQUE,
        };
        axes(&mut f)
            .bar(names(&["a"]), &[1.0], BarOptions::default())
            .unwrap()
            .grouped_bar(
                names(&["a"]),
                vec![
                    auto(&[1.0]),
                    series(ColorSpec::Fixed(fixed), &[1.0]),
                    series(at(5), &[1.0]),
                    auto(&[1.0]),
                ],
            )
            .unwrap()
            .line(cats(&["a"]), &[1.0], LineOptions::default())
            .unwrap()
            .stacked_bar(
                names(&["a"]),
                vec![auto(&[1.0]), series(at(0), &[1.0]), auto(&[2.0])],
            )
            .unwrap();
        let resolved = resolve_marks(f.single_axes().marks(), f.theme());
        assert_eq!(
            series_colors(&resolved),
            [
                palette[0], palette[1], fixed, palette[5], palette[2], palette[3], palette[4],
                palette[0], palette[5],
            ]
        );
        // Every grouped or stacked series is a legend key of its own, drawn as a bar.
        let keys = series_keys(&resolved, &Limits::of(f.single_axes()));
        assert_eq!(keys.len(), 9);
        assert!(keys[1..5].iter().all(|key| key.kind == SeriesKind::Bar));
    }

    #[test]
    fn bar_group_categories_join_first_appearance() {
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .bar(names(&["b"]), &[1.0], BarOptions::default())
            .unwrap()
            .stacked_bar(names(&["c", "b", "a"]), vec![auto(&[1.0, 2.0, 3.0])])
            .unwrap()
            .grouped_bar(names(&["d", "a"]), vec![auto(&[1.0, 2.0])])
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.x.ticks), ["b", "c", "a", "d"]);
    }

    /// Linear axes holding one histogram of `values` in `bins` bins, on a y axis of `y`.
    fn hist_figure(y: ScaleKind, values: &[f64], bins: i64) -> Figure {
        let mut f = figure(LIN, y);
        let options = HistOptions {
            bins,
            ..HistOptions::default()
        };
        axes(&mut f).hist(values, options).unwrap();
        f
    }

    #[test]
    fn hist_x_extent_is_nice_over_the_data() {
        // The bins span the data, 12 to 88; x takes the nice limits 10..90, padded on both
        // sides like any axes that is not all lines.
        let f = hist_figure(LIN, &[12.0, 47.0, 88.0, 30.0, f64::NAN], 4);
        let r = resolve_axes(f.single_axes(), 5, 4);
        let (x, _) = domains(&f);
        assert!(close(x, pad(10.0, 90.0)), "{x:?}");
        assert_eq!(labels(&r.x.ticks), ["10", "30", "50", "70", "90"]);
        assert_end_ticks_inset(&r.x.ticks);

        // Constant data is binned over v ± 0.5, which the x axis then encloses.
        let f = hist_figure(LIN, &[3.0, 3.0], 2);
        let (x, _) = domains(&f);
        assert!(x.0 < 2.5 && x.1 > 3.5, "{x:?}");
    }

    #[test]
    fn hist_y_extent_spans_zero_to_the_largest_count() {
        // Counts 1, 0, 2, 5: on a linear axis the y domain starts at 0, on the plot edge, with
        // the baseline, and reaches past the largest count.
        let values = [0.0, 2.5, 2.6, 3.1, 3.2, 3.3, 3.4, 4.0];
        let f = hist_figure(LIN, &values, 4);
        let r = resolve_axes(f.single_axes(), 5, 4);
        let (_, (lo, hi)) = domains(&f);
        assert_eq!(lo, 0.0);
        assert!(hi > 5.0, "{hi}");
        assert_eq!(r.y.ticks[0].pos.get(), 0.0);
        assert_eq!(r.baseline.get(), 0.0);
        // The same domain as bars as high as the counts.
        let g = bar_figure(LIN, &[1.0, 0.0, 2.0, 5.0]);
        assert_eq!(y_domain(&f), y_domain(&g));
    }

    #[test]
    fn hist_log_y_extent_comes_from_positive_counts() {
        // Counts 1, 0, 0, 40: on a log axis the domain is fitted to the positive counts, 1..40,
        // never to an extent that includes 0, which a log axis cannot hold (it would fall back
        // to the unit range 1..10). The count 1 lands on the lowest nice tick, so the lower
        // limit moves down a decade, to 0.1, and the bin of count 1 rises above the baseline.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut values = vec![0.0];
        values.extend(std::iter::repeat_n(4.0, 40));
        let f = hist_figure(log, &values, 4);
        let domain = y_domain(&f);
        assert!(close(domain, pad_log(0.1, 100.0)), "{domain:?}");
        // The same domain as a line through exactly the smallest and largest positive count.
        let mut g = figure(LIN, log);
        axes(&mut g)
            .line(nums(&[0.0, 1.0]), &[1.0, 40.0], LineOptions::default())
            .unwrap();
        assert_eq!(domain, y_domain(&g));
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(r.baseline, r.y.ticks[0].pos);
        assert_eq!(r.y.ticks[0].label, "0.1");
    }

    #[test]
    fn hist_without_finite_values_falls_back_to_the_unit_range() {
        for values in [&[][..], &[f64::NAN, f64::NAN][..]] {
            let f = hist_figure(LIN, values, 30);
            assert_eq!(domains(&f), ((0.0, 1.0), (0.0, 1.0)));
            let marks = resolve_marks(f.single_axes().marks(), &Theme::pigment());
            assert!(matches!(
                marks.as_slice(),
                [ResolvedMark::Hist {
                    histogram: None,
                    ..
                }]
            ));
        }
    }

    #[test]
    fn hist_is_binned_once_and_takes_one_auto_color() {
        let mut f = figure(LIN, LIN);
        let palette = f.theme().palette().to_vec();
        axes(&mut f)
            .scatter(nums(&[1.0]), &[1.0], ScatterOptions::default())
            .unwrap()
            .hist(
                &[1.0, 2.0, 2.0, 3.0],
                HistOptions {
                    bins: 2,
                    label: "sizes".into(),
                    ..HistOptions::default()
                },
            )
            .unwrap()
            .line(nums(&[1.0]), &[1.0], LineOptions::default())
            .unwrap();
        let resolved = resolve_marks(f.single_axes().marks(), f.theme());
        assert_eq!(
            series_colors(&resolved),
            [palette[1], palette[0], palette[2]]
        );
        let ResolvedMark::Hist {
            histogram: Some(histogram),
            ..
        } = &resolved[1]
        else {
            panic!("expected a binned histogram, got {:?}", resolved[1]);
        };
        assert_eq!(histogram.counts(), &[1, 3]);
        assert_eq!(
            resolved[1].keys(&Limits::of(f.single_axes())),
            [SeriesKey {
                label: "sizes",
                color: palette[0],
                kind: SeriesKind::Bar,
                anchor: None,
            }]
        );
    }

    /// The anchor of a labelled line through the numbers `x` and `y` on axes of the scales
    /// `kinds`, with the explicit x limits `xlim`, if any, as its x, y and end.
    fn number_anchor(
        kinds: (ScaleKind, ScaleKind),
        x: &[f64],
        y: &[f64],
        xlim: Option<(f64, f64)>,
    ) -> Option<(f64, f64, EndKind)> {
        let mut f = figure(kinds.0, kinds.1);
        let options = LineOptions {
            label: "Line".to_owned(),
            ..LineOptions::default()
        };
        axes(&mut f).line(nums(x), y, options).unwrap();
        if let Some((lo, hi)) = xlim {
            axes(&mut f).xlim(lo, hi).unwrap();
        }
        let axes = f.single_axes();
        let resolved = resolve_marks(axes.marks(), &Theme::pigment());
        let keys = series_keys(&resolved, &Limits::of(axes));
        keys[0].anchor.map(|anchor| match anchor.x {
            AnchorX::Number(x) => (x, anchor.y, anchor.end),
            AnchorX::Category(name) => panic!("expected a number anchor, got {name}"),
            AnchorX::Between(a, b) => panic!("expected a number anchor, got {a} to {b}"),
        })
    }

    #[test]
    fn anchor_is_last_visible_finite_point() {
        let nan = f64::NAN;
        let check = |x: &[f64], y: &[f64], xlim, expected| {
            assert_eq!(number_anchor((LIN, LIN), x, y, xlim), expected);
        };
        let point = |x, y| Some((x, y, EndKind::Point));
        // The last point.
        check(&[0.0, 1.0, 2.0], &[5.0, 6.0, 7.0], None, point(2.0, 7.0));
        // A NaN tail in y, and a NaN x, are skipped.
        check(
            &[0.0, 1.0, 2.0, nan],
            &[5.0, 6.0, nan, 8.0],
            None,
            point(1.0, 6.0),
        );
        // Unsorted x: the last point in data order, not the largest x.
        check(&[3.0, 1.0], &[5.0, 6.0], None, point(1.0, 6.0));
        // Explicit limits that hold the whole line: it ends at its last point.
        let (x, y) = ([0.0, 1.0, 2.0, 3.0], [5.0, 6.0, 7.0, 8.0]);
        check(&x, &y, Some((0.0, 3.0)), point(3.0, 8.0));
        // No point to label: all NaN, no finite x, or wholly outside the limits.
        check(&x, &y, Some((5.0, 6.0)), None);
        check(&[0.0, 1.0], &[nan, nan], None, None);
        check(&[nan, nan], &[1.0, 2.0], None, None);
        // A category line: the last category with a finite y, ending there.
        let mut f = figure(ScaleKind::Category, LIN);
        let options = LineOptions {
            label: "Line".to_owned(),
            ..LineOptions::default()
        };
        axes(&mut f)
            .line(cats(&["Jan", "Feb", "Mar"]), &[1.0, 2.0, nan], options)
            .unwrap();
        let axes = f.single_axes();
        let resolved = resolve_marks(axes.marks(), &Theme::pigment());
        let keys = series_keys(&resolved, &Limits::of(axes));
        let feb = Anchor {
            x: AnchorX::Category("Feb"),
            y: 2.0,
            end: EndKind::Point,
        };
        assert_eq!(keys[0].anchor, Some(feb));
    }

    #[test]
    fn anchor_of_a_line_cut_by_the_x_limits_is_where_it_leaves_the_plot() {
        let nan = f64::NAN;
        let log = ScaleKind::Log { base: LogBase::TEN };
        let lin = (LIN, LIN);
        let edge = |x, y| Some((x, y, EndKind::Edge));
        let point = |x, y| Some((x, y, EndKind::Point));
        let (x, y) = ([0.0, 1.0, 2.0, 3.0], [5.0, 6.0, 7.0, 8.0]);
        // Linear x: the segment from (1, 6) to (2, 7) crosses x = 1.5 at y = 6.5.
        assert_eq!(number_anchor(lin, &x, &y, Some((0.5, 1.5))), edge(1.5, 6.5));
        // A last point on the high end is a data point the plot shows: the line ends there,
        // with its dot, although it goes on past the limit; on a log x axis too.
        assert_eq!(
            number_anchor(lin, &x, &y, Some((0.0, 2.0))),
            point(2.0, 7.0)
        );
        let on_end = number_anchor((log, LIN), &[1.0, 10.0, 100.0], &y[..3], Some((1.0, 10.0)));
        assert_eq!(on_end, point(10.0, 6.0));
        // A falling segment: from (1, 6) to (2, -2) crosses x = 1.25 at y = 4.
        let falling = [5.0, 6.0, -2.0, 0.0];
        let cut = Some((0.0, 1.25));
        assert_eq!(number_anchor(lin, &x, &falling, cut), edge(1.25, 4.0));

        // Log x: the share is measured in log space. From (10, 2) to (1000, 4), x = 100 lies
        // halfway, so y = 3.
        let near = |anchor: Option<(f64, f64, EndKind)>, x: f64, y: f64| {
            let (ax, ay, end) = anchor.expect("an anchor");
            assert_eq!((ax, end), (x, EndKind::Edge));
            assert!((ay - y).abs() <= 1e-12 * y.abs(), "{ay} vs {y}");
        };
        let (lx, cut) = ([1.0, 10.0, 1000.0], Some((1.0, 100.0)));
        near(
            number_anchor((log, LIN), &lx, &[1.0, 2.0, 4.0], cut),
            100.0,
            3.0,
        );
        // Log y as well: y is interpolated in log space, halfway from 2 to 32 is 8.
        near(
            number_anchor((log, log), &lx, &[1.0, 2.0, 32.0], cut),
            100.0,
            8.0,
        );
        // Linear x, log y: a quarter of the way from (0, 1) to (4, 10000) is y = 10.
        let anchor = number_anchor((LIN, log), &[0.0, 4.0], &[1.0, 1e4], Some((0.0, 1.0)));
        near(anchor, 1.0, 10.0);

        // Log y between the smallest and largest values a log axis holds: nine tenths of the
        // way from 1e-300 to 1e300 is 1e240, rising, and 1e-240, falling, both finite and
        // positive.
        let cut = Some((0.0, 9.0));
        for (ys, expected) in [([1e-300, 1e300], 1e240), ([1e300, 1e-300], 1e-240)] {
            let (ax, ay, end) = number_anchor((LIN, log), &[0.0, 10.0], &ys, cut).unwrap();
            assert_eq!((ax, end), (9.0, EndKind::Edge));
            assert!(ay.is_finite() && ay > 0.0, "{ay:e}");
            assert!((ay / expected - 1.0).abs() < 1e-9, "{ay:e} vs {expected:e}");
        }
        // Narrow log spans keep full precision: from 1e200 to 1e200 (1 + 1e-8), halfway.
        let (lo, hi) = (1e200, 1e200 * (1.0 + 1e-8));
        let (_, ay, _) =
            number_anchor((LIN, log), &[0.0, 10.0], &[lo, hi], Some((0.0, 5.0))).unwrap();
        let expected = lo * (0.5 * (1e-8_f64).ln_1p()).exp();
        assert!(
            (ay / expected - 1.0).abs() < 1e-15,
            "{ay:e} vs {expected:e}"
        );

        // A NaN right after the last point inside breaks the line there: it ends inside.
        let cut = Some((0.0, 1.5));
        let gap_y = [5.0, 6.0, nan, 8.0];
        assert_eq!(number_anchor(lin, &x, &gap_y, cut), point(1.0, 6.0));
        let gap_x = [0.0, 1.0, nan, 3.0];
        assert_eq!(number_anchor(lin, &gap_x, &y, cut), point(1.0, 6.0));
        // A line that goes on past the low end, not the high end, ends at its last point.
        let back = number_anchor(lin, &[1.0, 2.0, 0.0], &[5.0, 6.0, 7.0], Some((0.5, 3.0)));
        assert_eq!(back, point(2.0, 6.0));
        // A line wholly past the high end has no anchor, so it goes to the header row.
        assert_eq!(number_anchor(lin, &x, &y, Some((-2.0, -1.0))), None);
    }

    /// A series key as owned values: label, color, kind and the anchor's x, y and end.
    type OwnedKey = (String, Rgba, SeriesKind, Option<(f64, f64, EndKind)>);

    #[test]
    fn area_keys_are_their_line_keys() {
        use crate::Dash;
        // An area is keyed exactly as the line that bounds it: the line swatch in its dash and
        // width, its color, and the line's anchor, at the edge where an x limit cuts it.
        let x = || nums(&[0.0, 1.0, 2.0]);
        let options = LineOptions {
            label: "Load".to_owned(),
            dash: Dash::Dotted,
            width: crate::SizeSpec::times(1.5).unwrap(),
            ..LineOptions::default()
        };
        for xlim in [None, Some((0.0, 1.5))] {
            let keys_of = |area: bool| {
                let mut f = figure(LIN, LIN);
                let mut a = axes(&mut f);
                if area {
                    a.area(x(), &[5.0, 6.0, 7.0], options.clone()).unwrap();
                } else {
                    a.line(x(), &[5.0, 6.0, 7.0], options.clone()).unwrap();
                }
                if let Some((lo, hi)) = xlim {
                    a.xlim(lo, hi).unwrap();
                }
                let axes = f.single_axes();
                let palette = f.theme().palette();
                let resolved = resolve_marks(axes.marks(), f.theme());
                let keys: Vec<OwnedKey> = series_keys(&resolved, &Limits::of(axes))
                    .iter()
                    .map(|key| {
                        let anchor = key.anchor.map(|a| match a.x {
                            AnchorX::Number(ax) => (ax, a.y, a.end),
                            AnchorX::Category(_) | AnchorX::Between(..) => {
                                panic!("numbers expected")
                            }
                        });
                        (key.label.to_owned(), key.color, key.kind, anchor)
                    })
                    .collect();
                (keys, palette[0])
            };
            let (area, color) = keys_of(true);
            assert_eq!(area, keys_of(false).0);
            let end = if xlim.is_some() {
                (1.5, 6.5, EndKind::Edge)
            } else {
                (2.0, 7.0, EndKind::Point)
            };
            assert_eq!(
                area,
                [(
                    "Load".to_owned(),
                    color,
                    SeriesKind::Line {
                        dash: Dash::Dotted,
                        width: 3.0,
                    },
                    Some(end)
                )]
            );
        }
    }

    #[test]
    fn each_area_takes_one_auto_color() {
        // Areas take Auto slots in series order with the other marks, one each.
        let mut f = figure(LIN, LIN);
        let palette = f.theme().palette().to_vec();
        let x = || nums(&[0.0, 1.0]);
        axes(&mut f)
            .area(x(), &[1.0, 2.0], AreaOptions::default())
            .unwrap()
            .line(x(), &[1.0, 2.0], LineOptions::default())
            .unwrap()
            .area(
                x(),
                &[1.0, 2.0],
                AreaOptions {
                    color: ColorSpec::Palette {
                        index: 5,
                        alpha: crate::Alpha::OPAQUE,
                    },
                    ..AreaOptions::default()
                },
            )
            .unwrap()
            .area(x(), &[1.0, 2.0], AreaOptions::default())
            .unwrap();
        let resolved = resolve_marks(f.single_axes().marks(), f.theme());
        assert!(matches!(
            resolved.as_slice(),
            [
                ResolvedMark::Area { .. },
                ResolvedMark::Line { .. },
                ResolvedMark::Area { .. },
                ResolvedMark::Area { .. },
            ]
        ));
        assert_eq!(
            series_colors(&resolved),
            [palette[0], palette[1], palette[5], palette[2]]
        );
    }

    #[test]
    fn areas_include_zero_and_keep_the_exact_x_extent() {
        // A linear y axis holding an area includes 0 and keeps it on the plot edge; areas and
        // lines alone keep the exact x extent, as lines alone do.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .area(nums(&[35.0, 65.0]), &[10.0, 40.0], AreaOptions::default())
            .unwrap()
            .line(nums(&[40.0, 60.0]), &[20.0, 30.0], LineOptions::default())
            .unwrap();
        let ((x_lo, x_hi), (y_lo, y_hi)) = domains(&f);
        assert_eq!((x_lo, x_hi), (35.0, 65.0));
        assert_eq!(y_lo, 0.0);
        assert!(y_hi > 40.0);
        // Next to a scatter, x gets nice padded limits.
        axes(&mut f)
            .scatter(nums(&[50.0]), &[20.0], ScatterOptions::default())
            .unwrap();
        let ((x_lo, x_hi), _) = domains(&f);
        assert!(x_lo < 35.0 && x_hi > 65.0, "{x_lo} {x_hi}");

        // On a log y axis, the area adds no 0: the domain spans the data, and the baseline
        // sits at the lowest tick, below the data.
        let mut f = figure(LIN, ScaleKind::Log { base: LogBase::TEN });
        axes(&mut f)
            .area(nums(&[0.0, 1.0]), &[10.0, 40.0], AreaOptions::default())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(r.y.ticks[0].label, "1");
        assert_eq!(r.baseline, r.y.ticks[0].pos);
        assert!(r.y.scale.domain().0 > 0.0);
    }

    #[test]
    fn only_line_keys_carry_an_anchor() {
        let mut f = figure(LIN, LIN);
        let x = || nums(&[0.0, 1.0]);
        axes(&mut f)
            .line(x(), &[1.0, 2.0], LineOptions::default())
            .unwrap()
            .scatter(x(), &[1.0, 2.0], ScatterOptions::default())
            .unwrap()
            .hist(&[1.0, 2.0], HistOptions::default())
            .unwrap();
        let axes = f.single_axes();
        let resolved = resolve_marks(axes.marks(), &Theme::pigment());
        let anchors: Vec<bool> = series_keys(&resolved, &Limits::of(axes))
            .iter()
            .map(|key| key.anchor.is_some())
            .collect();
        assert_eq!(anchors, [true, false, false]);
    }

    // ---- Explicit ticks and tick formats ----

    fn log10() -> ScaleKind {
        ScaleKind::Log { base: LogBase::TEN }
    }

    fn values(ticks: &[AxisTick], scale: &ContinuousScale) -> Vec<f64> {
        // The positions back in data space, for readable assertions on linear axes.
        let (lo, hi) = scale.domain();
        ticks.iter().map(|t| lo + t.pos.get() * (hi - lo)).collect()
    }

    /// A figure with one scatter over `x` and `y`, so that both axes get nice auto limits.
    fn scatter_figure(x: ScaleKind, y: ScaleKind, xs: &[f64], ys: &[f64]) -> Figure {
        let mut f = figure(x, y);
        axes(&mut f)
            .scatter(nums(xs), ys, ScatterOptions::default())
            .unwrap();
        f
    }

    #[test]
    fn explicit_ticks_set_the_domain_without_nice_rounding() {
        // Inside, outside and straddling the data, on a scatter's padded x axis.
        for (ticks, want) in [
            (vec![2.5, 7.5], (0.3, 9.7)),
            (vec![-3.0, 13.0], (-3.0, 13.0)),
            (vec![5.0, 12.5], (0.3, 12.5)),
        ] {
            let mut f = scatter_figure(LIN, LIN, &[0.3, 9.7], &[1.0, 2.0]);
            axes(&mut f).xticks(&ticks).unwrap();
            let r = resolve_axes(f.single_axes(), 5, 4);
            let x = r.x.continuous().unwrap();
            assert!(close(x.domain(), pad(want.0, want.1)), "{ticks:?}: {x:?}");
            assert_eq!(r.x.ticks.len(), ticks.len());
            for (got, want) in values(&r.x.ticks, x).iter().zip(&ticks) {
                assert!((got - want).abs() < 1e-9, "{ticks:?}");
            }
        }
        // Without explicit ticks the same data gets the nice limits 0 and 10.
        let f = scatter_figure(LIN, LIN, &[0.3, 9.7], &[1.0, 2.0]);
        assert!(close(domains(&f).0, pad(0.0, 10.0)));

        // On a log axis the domain encloses data and ticks and is padded in log space.
        for (ticks, want) in [
            (vec![10.0, 100.0], (3.0, 400.0)),
            (vec![1.0, 10.0, 100.0, 1000.0], (1.0, 1000.0)),
            (vec![30.0, 3000.0], (3.0, 3000.0)),
        ] {
            let mut f = scatter_figure(LIN, log10(), &[0.0, 1.0], &[3.0, 400.0]);
            axes(&mut f).yticks(&ticks).unwrap();
            let r = resolve_axes(f.single_axes(), 5, 4);
            assert!(
                close(r.y.scale.domain(), pad_log(want.0, want.1)),
                "{ticks:?}"
            );
            assert_eq!(r.y.ticks.len(), ticks.len());
        }

        // An axes of lines keeps its exact x extent: data and ticks, unpadded.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .line(nums(&[0.0, 10.0]), &[1.0, 2.0], LineOptions::default())
            .unwrap()
            .xticks(&[-2.0, 4.0])
            .unwrap();
        assert_eq!(domains(&f).0, (-2.0, 10.0));
    }

    #[test]
    fn explicit_limits_hide_the_ticks_outside_them() {
        let mut f = scatter_figure(LIN, LIN, &[0.0, 10.0], &[1.0, 2.0]);
        axes(&mut f)
            .xlim(0.0, 10.0)
            .unwrap()
            .xticks(&[-5.0, 0.0, 5.0, 10.0, 15.0])
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(r.x.continuous().unwrap().domain(), (0.0, 10.0));
        assert_eq!(labels(&r.x.ticks), ["0", "5", "10"]);
        let pos: Vec<f64> = r.x.ticks.iter().map(|t| t.pos.get()).collect();
        assert_eq!(pos, [0.0, 0.5, 1.0]);

        // Labelled ticks too, and on a log axis.
        let mut f = scatter_figure(LIN, log10(), &[0.0, 1.0], &[3.0, 400.0]);
        let names: Vec<String> = ["one", "ten", "hundred", "thousand"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        axes(&mut f)
            .ylim(5.0, 500.0)
            .unwrap()
            .yticks_labeled(&[1.0, 10.0, 100.0, 1000.0], names)
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(r.y.scale.domain(), (5.0, 500.0));
        assert_eq!(labels(&r.y.ticks), ["ten", "hundred"]);
        // The baseline sits at the explicit low limit.
        assert_eq!(r.baseline, NormalizedPos::LOW);
    }

    #[test]
    fn explicit_y_ticks_keep_the_zero_anchor_and_skip_the_lowering() {
        // Auto ticks move the lowest tick below data that starts on it; explicit ones do not.
        let mut f = scatter_figure(LIN, LIN, &[0.0, 1.0], &[10.0, 20.0]);
        let auto = resolve_axes(f.single_axes(), 5, 4);
        assert!(auto.y.scale.domain().0 < 10.0 - 1.0);
        axes(&mut f).yticks(&[10.0, 15.0, 20.0]).unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert!(close(r.y.scale.domain(), pad(10.0, 20.0)));
        assert_eq!(labels(&r.y.ticks), ["10", "15", "20"]);
        // The baseline uses the unpadded lower limit of the resulting domain: the lowest tick.
        assert!(coincides(r.baseline, r.y.ticks[0].pos));
        assert!((r.baseline.get() - 0.05 / 1.1).abs() < 1e-12);

        // Bars keep 0 in a linear y domain, and the zero side stays unpadded.
        let mut f = bar_figure(LIN, &[3.0, 7.0]);
        axes(&mut f).yticks(&[2.0, 4.0, 6.0, 8.0]).unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert!(close(r.y.scale.domain(), (0.0, 8.4)), "{:?}", r.y.scale);
        assert_eq!(r.baseline, NormalizedPos::LOW);
        assert_eq!(labels(&r.y.ticks), ["2", "4", "6", "8"]);

        // A histogram on a log y axis grows from the unpadded low end of data and ticks.
        let mut f = hist_figure(log10(), &[1.0, 2.0, 2.5, 3.0, 3.5, 4.0], 3);
        axes(&mut f).yticks(&[0.5, 5.0]).unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert!(
            close(r.y.scale.domain(), pad_log(0.5, 5.0)),
            "{:?}",
            r.y.scale
        );
        assert!(coincides(r.baseline, r.y.ticks[0].pos));
    }

    #[test]
    fn empty_tick_lists_leave_no_ticks_or_gridlines() {
        let mut f = scatter_figure(LIN, LIN, &[1.0, 9.0], &[2.0, 4.0]);
        axes(&mut f).xticks(&[]).unwrap().yticks(&[]).unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert!(r.x.ticks.is_empty() && r.y.ticks.is_empty());
        // The domain is the padded data extent, not rounded to nice limits.
        assert!(close(r.x.continuous().unwrap().domain(), pad(1.0, 9.0)));
        assert!(close(r.y.scale.domain(), pad(2.0, 4.0)));
        let (_, layout) = crate::pipeline::plan_figure(&f, (0, 0), crate::FontLibrary::global());
        assert!(layout.x_grid.is_empty() && layout.y_grid.is_empty());
        assert!(layout.x_tick_labels.is_empty() && layout.y_tick_labels.is_empty());

        // Without data, ticks alone set the domain; with neither, the fallback range stays.
        // An axes without marks has no mark that is not a line, so its x axis is exact.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .xticks(&[2.0, 4.0])
            .unwrap()
            .yticks(&[])
            .unwrap();
        assert_eq!(domains(&f), ((2.0, 4.0), (0.0, 1.0)));
        axes(&mut f)
            .xticks(&[])
            .unwrap()
            .yticks(&[2.0, 4.0])
            .unwrap();
        let (x, y) = domains(&f);
        assert_eq!(x, (0.0, 1.0));
        assert!(close(y, pad(2.0, 4.0)));
    }

    #[test]
    fn explicit_labels_are_drawn_as_given_and_positions_take_the_format() {
        let mut f = scatter_figure(LIN, LIN, &[0.0, 3.0], &[0.0, 0.5]);
        axes(&mut f)
            .xticks_labeled(
                &[0.0, 1.5, 3.0],
                vec!["start".into(), "".into(), "\u{2212}end".into()],
            )
            .unwrap()
            .yticks(&[0.0, 0.25, 0.5])
            .unwrap()
            .ytick_format(TickFormat::percent(0).unwrap().affix("", " share").unwrap())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.x.ticks), ["start", "", "\u{2212}end"]);
        assert_eq!(labels(&r.y.ticks), ["0% share", "25% share", "50% share"]);

        // AUTO writes explicit linear positions to the finest decimal place they need.
        axes(&mut f)
            .xticks(&[0.0, 1.5, 3.25])
            .unwrap()
            .yticks(&[-0.5, 0.0, 0.5])
            .unwrap()
            .ytick_format(TickFormat::AUTO)
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.x.ticks), ["0.00", "1.50", "3.25"]);
        assert_eq!(labels(&r.y.ticks), ["\u{2212}0.5", "0.0", "0.5"]);

        // On a log axis AUTO labels each value, and other styles write the values.
        let mut f = scatter_figure(LIN, log10(), &[0.0, 1.0], &[3.0, 400.0]);
        axes(&mut f).yticks(&[2.0, 20.0, 200.0]).unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.y.ticks), ["2", "20", "200"]);
        axes(&mut f)
            .ytick_format(TickFormat::fixed(1).unwrap())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.y.ticks), ["2.0", "20.0", "200.0"]);
    }

    #[test]
    fn formats_relabel_the_automatic_ticks() {
        let f = scatter_figure(LIN, LIN, &[0.0, 10.0], &[0.0, 40.0]);
        let auto = resolve_axes(f.single_axes(), 5, 5);
        assert_eq!(labels(&auto.y.ticks), ["0", "10", "20", "30", "40"]);
        let mut f = f;
        axes(&mut f)
            .ytick_format(TickFormat::fixed(1).unwrap().affix("$", "").unwrap())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 5);
        assert_eq!(
            labels(&r.y.ticks),
            ["$0.0", "$10.0", "$20.0", "$30.0", "$40.0"]
        );
        // The format changes labels only: the ticks and the domain stay as they were.
        assert_eq!(r.y.scale, auto.y.scale);
        let pos = |ticks: &[AxisTick]| ticks.iter().map(|t| t.pos).collect::<Vec<_>>();
        assert_eq!(pos(&r.y.ticks), pos(&auto.y.ticks));
        axes(&mut f).ytick_format(TickFormat::scientific()).unwrap();
        let r = resolve_axes(f.single_axes(), 5, 5);
        assert_eq!(labels(&r.y.ticks), ["0", "1e1", "2e1", "3e1", "4e1"]);

        // On a log axis, AUTO keeps the value labels and the other styles write the values.
        let mut f = scatter_figure(LIN, log10(), &[0.0, 1.0], &[2.0, 3000.0]);
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.y.ticks), ["1", "10", "100", "1000", "10000"]);
        axes(&mut f)
            .ytick_format(TickFormat::percent(0).unwrap())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(
            labels(&r.y.ticks),
            ["100%", "1000%", "10000%", "100000%", "1000000%"]
        );
        axes(&mut f).ytick_format(TickFormat::scientific()).unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.y.ticks), ["1e0", "1e1", "1e2", "1e3", "1e4"]);
    }

    #[test]
    fn only_auto_raises_the_precision() {
        // Two positions that agree to 12 decimals: AUTO separates them, fixed does not.
        let close_pair = [1.0, 1.0 + 3e-13];
        let mut f = scatter_figure(LIN, LIN, &[0.0, 1.0], &[0.0, 1.0]);
        axes(&mut f).yticks(&close_pair).unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        let auto = labels(&r.y.ticks);
        assert_ne!(auto[0], auto[1], "{auto:?}");
        axes(&mut f)
            .ytick_format(TickFormat::fixed(2).unwrap())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert_eq!(labels(&r.y.ticks), ["1.00", "1.00"]);
        // A coarse fixed format may merge automatic ticks too.
        let mut f = scatter_figure(LIN, LIN, &[0.0, 1.0], &[0.0, 1.0]);
        axes(&mut f)
            .ytick_format(TickFormat::fixed(0).unwrap())
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 5);
        assert_eq!(labels(&r.y.ticks), ["0", "0", "0", "1", "1"]);
    }

    /// The y tick labels of an axes without marks whose y axis of `y` has ticks at `positions`
    /// and the tick format `format`.
    fn y_labels_of(y: ScaleKind, positions: &[f64], format: TickFormat) -> Vec<String> {
        let mut f = figure(LIN, y);
        axes(&mut f)
            .yticks(positions)
            .unwrap()
            .ytick_format(format)
            .unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        r.y.ticks.into_iter().map(|t| t.label).collect()
    }

    #[test]
    fn tiny_explicit_positions_keep_labels_of_their_own() {
        assert_eq!(
            y_labels_of(LIN, &[0.0, 1e-24], TickFormat::AUTO),
            ["0", "1e\u{2212}24"]
        );
        assert_eq!(
            y_labels_of(LIN, &[-1e-24, 0.0, 1e-24], TickFormat::AUTO),
            ["\u{2212}1e\u{2212}24", "0", "1e\u{2212}24"]
        );
        // Positions far from their neighbours are exact values, never snapped to zero.
        for y in [LIN, log10()] {
            assert_eq!(
                y_labels_of(y, &[1e-12, 1.0], TickFormat::scientific()),
                ["1e\u{2212}12", "1e0"],
                "{y:?}"
            );
            assert_eq!(
                y_labels_of(y, &[1e-10, 1.0], TickFormat::fixed(12).unwrap()),
                ["0.000000000100", "1.000000000000"],
                "{y:?}"
            );
            assert_eq!(
                y_labels_of(y, &[1e-12, 1.0], TickFormat::AUTO),
                ["1e\u{2212}12", "1e0"],
                "{y:?}"
            );
        }
    }

    #[test]
    fn explicit_ticks_keep_zero_for_marks_without_finite_values() {
        // Bars and a histogram with no finite value still keep 0 in a linear y domain.
        let mut bars = bar_figure(LIN, &[f64::NAN]);
        let mut hist = hist_figure(LIN, &[f64::NAN, f64::NAN], 3);
        for f in [&mut bars, &mut hist] {
            axes(f).yticks(&[2.0, 4.0]).unwrap();
            let r = resolve_axes(f.single_axes(), 5, 4);
            assert!(close(r.y.scale.domain(), (0.0, 4.2)), "{:?}", r.y.scale);
            assert_eq!(r.baseline, NormalizedPos::LOW);
            assert_eq!(labels(&r.y.ticks), ["2", "4"]);
            // Without ticks either, the empty-data range stays.
            axes(f).yticks(&[]).unwrap();
            let r = resolve_axes(f.single_axes(), 5, 4);
            assert_eq!(r.y.scale.domain(), (0.0, 1.0));
        }
        // On a log axis there is no 0 to keep.
        let mut f = bar_figure(log10(), &[f64::NAN]);
        axes(&mut f).yticks(&[2.0, 4.0]).unwrap();
        let r = resolve_axes(f.single_axes(), 5, 4);
        assert!(
            close(r.y.scale.domain(), pad_log(2.0, 4.0)),
            "{:?}",
            r.y.scale
        );
    }

    #[test]
    fn every_format_decides_zero_as_the_automatic_labels_do() {
        // With the lower limit within 1e-9 of the domain of zero, the zero candidate of the
        // grid is moved onto the limit. Whether it then reads as zero is decided by the grid
        // step, never by the gap the move left, and in every format alike.
        let y_labels = |low: f64, format: TickFormat| {
            let mut f = figure(LIN, LIN);
            axes(&mut f)
                .xticks(&[])
                .unwrap()
                .ylim(low, 1.0)
                .unwrap()
                .ytick_format(format)
                .unwrap();
            let r = resolve_axes(f.single_axes(), 5, 6);
            r.y.ticks.into_iter().map(|t| t.label).collect::<Vec<_>>()
        };
        let low = -0.2_f64 * 1e-9;
        assert_eq!(y_labels(low, TickFormat::AUTO)[0], "\u{2212}2e\u{2212}10");
        assert_eq!(
            y_labels(low, TickFormat::scientific())[0],
            "\u{2212}2e\u{2212}10"
        );
        assert_eq!(
            y_labels(low, TickFormat::fixed(12).unwrap())[0],
            "\u{2212}0.000000000200"
        );
        assert_eq!(
            y_labels(low, TickFormat::percent(12).unwrap())[0],
            "\u{2212}0.000000020000%"
        );

        let reads_zero = |label: &str| {
            let number = label.trim_end_matches('%');
            number == "0" || number.chars().all(|c| c == '0' || c == '.')
        };
        for low in [-2.1e-10, low, -1.9e-10, -1e-12, -1e-17, 0.0] {
            let auto: Vec<bool> = y_labels(low, TickFormat::AUTO)
                .iter()
                .map(|l| reads_zero(l))
                .collect();
            for format in [
                TickFormat::scientific(),
                TickFormat::fixed(12).unwrap(),
                TickFormat::percent(12).unwrap(),
            ] {
                let zeros: Vec<bool> = y_labels(low, format.clone())
                    .iter()
                    .map(|l| reads_zero(l))
                    .collect();
                assert_eq!(zeros, auto, "low {low:e}, {format:?}");
            }
        }
    }

    /// A figure of linear axes holding one labelled step line of `position` through `x` and `y`,
    /// with the explicit x limits `xlim`, if any.
    fn step_figure(position: Step, x: &[f64], y: &[f64], xlim: Option<(f64, f64)>) -> Figure {
        let mut f = figure(LIN, LIN);
        let options = StepOptions {
            position,
            label: "Step".to_owned(),
            ..StepOptions::default()
        };
        axes(&mut f).step(nums(x), y, options).unwrap();
        if let Some((lo, hi)) = xlim {
            axes(&mut f).xlim(lo, hi).unwrap();
        }
        f
    }

    #[test]
    fn a_step_is_a_line_to_domains_width_swatch_and_color() {
        let mut f = step_figure(Step::Post, &[0.0, 10.0], &[2.0, 6.0], None);
        axes(&mut f)
            .line(nums(&[1.0]), &[1.0], LineOptions::default())
            .unwrap();
        let theme = Theme::pigment();
        let resolved = resolve_marks(f.single_axes().marks(), &theme);
        let [step, line] = resolved.as_slice() else {
            panic!("{resolved:?}")
        };
        assert_eq!(
            step.line_width(),
            Some(theme.drawn_line_width(crate::SizeSpec::Auto))
        );
        assert!(!step.grows_from_baseline());
        assert_eq!(step.x_numbers().collect::<Vec<_>>(), [0.0, 10.0]);
        assert_eq!(step.y_values().collect::<Vec<_>>(), [2.0, 6.0]);
        // Colors come from the one palette sequence, in mark order.
        assert_eq!(
            series_colors(&resolved),
            [theme.palette()[0], theme.palette()[1]]
        );
        let limits = Limits::of(f.single_axes());
        let [key] = step.keys(&limits).try_into().unwrap();
        assert_eq!(key.label, "Step");
        assert_eq!(
            key.kind,
            SeriesKind::Line {
                dash: Dash::Solid,
                width: theme.drawn_line_width(crate::SizeSpec::Auto)
            }
        );
        assert_eq!(line.keys(&limits)[0].kind, key.kind);
        // Lines and steps alone keep the exact data extent of the x axis.
        assert_eq!(
            domains(&step_figure(Step::Mid, &[0.0, 10.0], &[2.0, 6.0], None)).0,
            (0.0, 10.0)
        );

        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .step(cats(&["a", "b"]), &[1.0, 2.0], StepOptions::default())
            .unwrap();
        let resolved = resolve_marks(f.single_axes().marks(), &theme);
        assert_eq!(resolved[0].x_categories().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(resolved[0].x_numbers().count(), 0);
    }

    #[test]
    fn the_anchor_of_a_step_cut_by_the_x_limits_has_the_level_the_step_has_there() {
        let anchor = |position, x_hi| {
            let f = step_figure(
                position,
                &[0.0, 2.0, 6.0],
                &[1.0, 5.0, 9.0],
                Some((0.0, x_hi)),
            );
            let axes = f.single_axes();
            let resolved = resolve_marks(axes.marks(), &Theme::pigment());
            let keys = series_keys(&resolved, &Limits::of(axes));
            keys[0].anchor.map(|a| match a.x {
                AnchorX::Number(x) => (x, a.y, a.end),
                AnchorX::Category(name) => panic!("category {name}"),
                AnchorX::Between(a, b) => panic!("between {a} and {b}"),
            })
        };
        let edge = |x, y| Some((x, y, EndKind::Edge));
        // The line leaves across x = 3, between the points (2, 5) and (6, 9). The step line
        // holds 5 up to 6 (Post), already holds 9 from 2 on (Pre), and changes at x = 4 (Mid).
        assert_eq!(anchor(Step::Post, 3.0), edge(3.0, 5.0));
        assert_eq!(anchor(Step::Pre, 3.0), edge(3.0, 9.0));
        assert_eq!(anchor(Step::Mid, 3.0), edge(3.0, 5.0));
        assert_eq!(anchor(Step::Mid, 5.0), edge(5.0, 9.0));
        // A step line that ends inside the limits ends at its last point.
        assert_eq!(anchor(Step::Post, 6.0), Some((6.0, 9.0, EndKind::Point)));
    }

    /// The anchor of a labelled step line of `position` through `x` and `y` on axes of the x scale
    /// `kind`, with the explicit x limits `xlim`, if any.
    fn step_anchor(
        kind: ScaleKind,
        x: AxisData,
        y: &[f64],
        position: Step,
        xlim: Option<(f64, f64)>,
    ) -> Option<Anchor<'static>> {
        let mut f = figure(kind, LIN);
        let options = StepOptions {
            position,
            label: "Step".to_owned(),
            ..StepOptions::default()
        };
        axes(&mut f).step(x, y, options).unwrap();
        if let Some((lo, hi)) = xlim {
            axes(&mut f).xlim(lo, hi).unwrap();
        }
        let axes = f.single_axes();
        let resolved = resolve_marks(axes.marks(), &Theme::pigment());
        let anchor = series_keys(&resolved, &Limits::of(axes))[0].anchor?;
        // Category names borrow from the figure; the tests compare their text.
        let x = match anchor.x {
            AnchorX::Number(x) => AnchorX::Number(x),
            AnchorX::Category(name) => AnchorX::Category(leak(name)),
            AnchorX::Between(a, b) => AnchorX::Between(leak(a), leak(b)),
        };
        Some(Anchor { x, ..anchor })
    }

    fn leak(name: &str) -> &'static str {
        Box::leak(name.to_owned().into_boxed_str())
    }

    #[test]
    fn log_mid_step_anchor_at_repeated_low_limit() {
        let anchor = step_anchor(
            ScaleKind::Log { base: LogBase::TEN },
            nums(&[6.0, 6.0]),
            &[1.0, f64::NAN],
            Step::Mid,
            Some((6.0, 10.0)),
        );
        assert_eq!(
            anchor,
            Some(Anchor {
                x: AnchorX::Number(6.0),
                y: 1.0,
                end: EndKind::Point
            })
        );
    }

    #[test]
    fn log_mid_step_anchor_at_repeated_high_limit() {
        let anchor = step_anchor(
            ScaleKind::Log { base: LogBase::TEN },
            nums(&[3.0, 3.0]),
            &[1.0, 2.0],
            Step::Mid,
            Some((1.0, 3.0)),
        );
        assert_eq!(
            anchor,
            Some(Anchor {
                x: AnchorX::Number(3.0),
                y: 2.0,
                end: EndKind::Point
            })
        );
    }

    #[test]
    fn a_step_label_is_anchored_where_the_drawn_step_ends() {
        let nan = f64::NAN;
        let (x, y) = ([0.0, 1.0, 4.0], [1.0, 2.0, nan]);
        let number = |x, y, end| {
            Some(Anchor {
                x: AnchorX::Number(x),
                y,
                end,
            })
        };
        let point = |x, y| number(x, y, EndKind::Point);
        let edge = |x, y| number(x, y, EndKind::Edge);
        let at = |position, xlim| step_anchor(LIN, nums(&x), &y, position, xlim);
        // The level of the last point with a y is held to the next x, however its y reads.
        assert_eq!(at(Step::Post, None), point(4.0, 2.0));
        assert_eq!(at(Step::Mid, None), point(2.5, 2.0));
        assert_eq!(at(Step::Pre, None), point(1.0, 2.0));
        // Explicit limits that hold the end, cut the held level, or cut it before its end.
        assert_eq!(at(Step::Post, Some((0.0, 4.0))), point(4.0, 2.0));
        assert_eq!(at(Step::Post, Some((0.0, 3.0))), edge(3.0, 2.0));
        assert_eq!(at(Step::Mid, Some((0.0, 3.0))), point(2.5, 2.0));
        assert_eq!(at(Step::Mid, Some((0.0, 2.0))), edge(2.0, 2.0));
        assert_eq!(at(Step::Pre, Some((0.0, 3.0))), point(1.0, 2.0));
        // A gap in the middle ends nothing: the last hold is the end.
        let gapped = |position| {
            step_anchor(
                LIN,
                nums(&[0.0, 1.0, 2.0]),
                &[1.0, nan, 3.0],
                position,
                None,
            )
        };
        assert_eq!(gapped(Step::Post), point(2.0, 3.0));
        // On a log x axis the middle of 100 and 10000 is 1000.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let anchor = step_anchor(log, nums(&[1.0, 100.0, 10000.0]), &y, Step::Mid, None);
        let Some(Anchor {
            x: AnchorX::Number(x),
            ..
        }) = anchor
        else {
            panic!("expected a number anchor, got {anchor:?}")
        };
        assert!((x / 1000.0 - 1.0).abs() < 1e-12, "{x}");
    }

    #[test]
    fn a_category_step_label_is_anchored_where_the_drawn_step_ends() {
        let nan = f64::NAN;
        let names = || cats(&["a", "b", "c"]);
        let y = [1.0, 2.0, nan];
        let at = |name, y| {
            Some(Anchor {
                x: name,
                y,
                end: EndKind::Point,
            })
        };
        let anchor = |position| step_anchor(ScaleKind::Category, names(), &y, position, None);
        assert_eq!(anchor(Step::Post), at(AnchorX::Category("c"), 2.0));
        assert_eq!(anchor(Step::Mid), at(AnchorX::Between("b", "c"), 2.0));
        assert_eq!(anchor(Step::Pre), at(AnchorX::Category("b"), 2.0));
        // Without a trailing gap the step ends at its last band.
        let full = |position| {
            step_anchor(
                ScaleKind::Category,
                names(),
                &[1.0, 2.0, 3.0],
                position,
                None,
            )
        };
        assert_eq!(full(Step::Post), at(AnchorX::Category("c"), 3.0));
        assert_eq!(full(Step::Mid), at(AnchorX::Category("c"), 3.0));
        assert_eq!(full(Step::Pre), at(AnchorX::Category("c"), 3.0));
    }

    #[test]
    fn a_step_hold_that_covers_the_view_or_starts_at_the_high_limit_is_cut_by_it() {
        let number = |x, y, end| {
            Some(Anchor {
                x: AnchorX::Number(x),
                y,
                end,
            })
        };
        let edge = |x, y| number(x, y, EndKind::Edge);
        // The hold of y = 2 runs from 5 to 15 and covers the view 9 to 11, with no vertex in it.
        let anchor = |x: &[f64], xlim, kind| {
            step_anchor(kind, nums(x), &[1.0, 2.0, 3.0], Step::Mid, Some(xlim))
        };
        assert_eq!(
            anchor(&[0.0, 10.0, 20.0], (9.0, 11.0), LIN),
            edge(11.0, 2.0)
        );
        // On a log x axis the holds change level at 10 and 1000, and 90 to 110 lies in the second.
        let log = ScaleKind::Log { base: LogBase::TEN };
        assert_eq!(
            anchor(&[1.0, 100.0, 10000.0], (90.0, 110.0), log),
            edge(110.0, 2.0)
        );
        // A view wholly inside one hold, or before the first, or after the last.
        assert_eq!(anchor(&[0.0, 10.0, 20.0], (0.0, 1.0), LIN), edge(1.0, 1.0));
        assert_eq!(anchor(&[0.0, 10.0, 20.0], (30.0, 40.0), LIN), None);

        // The last hold runs from 4 to 6; a limit at its start, just before it and just past it.
        let (x, y) = ([0.0, 2.0, 6.0], [1.0, 5.0, 9.0]);
        let at = |hi| step_anchor(LIN, nums(&x), &y, Step::Mid, Some((0.0, hi)));
        assert_eq!(at(4.0), edge(4.0, 9.0));
        assert_eq!(at(3.99), edge(3.99, 5.0));
        assert_eq!(at(4.01), edge(4.01, 9.0));
        // A hold that ends exactly at the high limit ends the line there.
        assert_eq!(at(6.0), number(6.0, 9.0, EndKind::Point));
    }

    /// The last point of the step path through `xs` and `ys`, drawn from its holds, that lies in
    /// the x domain `lo..=hi`, and whether the path goes on past `hi` from there. Segments are
    /// clipped one by one, in drawing order.
    fn last_visible(
        xs: &[f64],
        ys: &[f64],
        position: Step,
        lo: f64,
        hi: f64,
    ) -> Option<(f64, f64, bool)> {
        let mid = |a: f64, b: f64| (a + b) / 2.0;
        let mut found = None;
        let mut visit = |px: f64, (qx, qy): (f64, f64)| {
            // x never decreases along the path.
            if qx < lo || px > hi {
                return;
            }
            found = Some(if qx > hi {
                (hi, qy, true)
            } else {
                (qx, qy, false)
            });
        };
        let mut previous: Option<(usize, f64)> = None;
        for hold in holds(xs, ys, position) {
            let y = ys[hold.point];
            let from = (hold.from.position(xs, mid), y);
            let to = (hold.to.position(xs, mid), y);
            if let Some((_, end)) = previous.filter(|&(p, _)| p + 1 == hold.point) {
                visit(end, from);
            }
            visit(from.0, to);
            previous = Some((hold.point, to.0));
        }
        found
    }

    #[test]
    fn a_step_anchor_is_the_last_visible_point_of_the_drawn_path() {
        let mut state = 7_u32;
        let mut next = |n: u32| {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (state >> 8) % n
        };
        for case in 0..400 {
            let len = 1 + next(6) as usize;
            let mut x = 0.0;
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for _ in 0..len {
                x += f64::from(next(4)) * 0.5;
                xs.push(x);
                ys.push(if next(5) == 0 {
                    f64::NAN
                } else {
                    f64::from(next(9))
                });
            }
            let lo = f64::from(next(8)) * 0.5 - 1.0;
            let hi = lo + 0.5 + f64::from(next(10)) * 0.5;
            let position = [Step::Post, Step::Pre, Step::Mid][next(3) as usize];
            let expected = last_visible(&xs, &ys, position, lo, hi);
            let got = step_anchor(LIN, nums(&xs), &ys, position, Some((lo, hi))).map(|a| {
                let AnchorX::Number(x) = a.x else {
                    panic!("number expected")
                };
                (x, a.y, a.end == EndKind::Edge)
            });
            assert_eq!(
                got, expected,
                "case {case}: {position:?} {xs:?} {ys:?} {lo}..{hi}"
            );
        }
    }

    use crate::model::{RuleOptions, SpanOptions};

    fn rule() -> RuleOptions {
        RuleOptions::default()
    }

    fn span() -> SpanOptions {
        SpanOptions::default()
    }

    fn at(x: f64) -> AxisValue {
        AxisValue::Number(x)
    }

    /// A figure of linear axes holding a line over x in 0..=10 and y in 2..=6.
    fn lined() -> Figure {
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .line(nums(&[0.0, 10.0]), &[2.0, 6.0], LineOptions::default())
            .unwrap();
        f
    }

    #[test]
    fn rules_extend_the_domain() {
        // The line alone: the exact x extent, the nice y extent.
        let alone = domains(&lined());
        assert_eq!(alone.0, (0.0, 10.0));
        assert!(alone.1.1 < 9.0, "{:?}", alone.1);
        // A rule or a span widens its own axis only, and a line does not stop the x axis from
        // keeping the exact extent of its data.
        let mut f = lined();
        axes(&mut f).hline(9.0, rule()).unwrap();
        let (x, (_, y_hi)) = domains(&f);
        assert_eq!(x, alone.0, "an hline does not touch x");
        assert!(y_hi >= 9.0, "{y_hi}");
        let mut f = lined();
        axes(&mut f).vline(at(15.0), rule()).unwrap();
        let (x, y) = domains(&f);
        assert_eq!(x, (0.0, 15.0));
        assert_eq!(y, alone.1, "a vline does not touch y");
        let mut f = lined();
        axes(&mut f)
            .vspan(-3.0, 4.0, span())
            .unwrap()
            .hspan(-2.0, 1.0, span())
            .unwrap();
        let ((x_lo, x_hi), (y_lo, _)) = domains(&f);
        assert_eq!((x_lo, x_hi), (-3.0, 10.0));
        assert!(y_lo <= -2.0, "{y_lo}");
    }

    #[test]
    fn explicit_limits_win_over_rules() {
        let mut f = lined();
        axes(&mut f)
            .hline(90.0, rule())
            .unwrap()
            .vspan(-30.0, 40.0, span())
            .unwrap()
            .xlim(0.0, 12.0)
            .unwrap()
            .ylim(0.0, 8.0)
            .unwrap();
        assert_eq!(domains(&f), ((0.0, 12.0), (0.0, 8.0)));
    }

    #[test]
    fn empty_lines_do_not_remove_reference_padding() {
        for values in [&[][..], &[f64::NAN][..]] {
            let mut f = figure(LIN, LIN);
            axes(&mut f).vspan(2.0, 6.0, span()).unwrap();
            let alone = domains(&f);
            axes(&mut f)
                .line(nums(values), values, LineOptions::default())
                .unwrap();
            assert_eq!(domains(&f), alone);
        }
    }

    #[test]
    fn empty_scatters_do_not_pad_line_domains() {
        for values in [&[][..], &[f64::NAN][..]] {
            let mut f = lined();
            let alone = domains(&f);
            axes(&mut f)
                .scatter(nums(values), values, ScatterOptions::default())
                .unwrap();
            assert_eq!(domains(&f), alone);
        }
    }

    #[test]
    fn empty_areas_do_not_change_reference_y_padding() {
        let mut f = figure(LIN, LIN);
        axes(&mut f).hspan(2.0, 6.0, span()).unwrap();
        let alone = domains(&f);
        axes(&mut f)
            .area(nums(&[]), &[], AreaOptions::default())
            .unwrap();
        assert_eq!(domains(&f), alone);
    }

    #[test]
    fn references_alone_pad_the_axis_and_beside_lines_do_not() {
        // A span alone: padded, so the span does not fill the view edge to edge.
        let mut f = figure(LIN, LIN);
        axes(&mut f).vspan(2.0, 6.0, span()).unwrap();
        let (x_lo, x_hi) = domains(&f).0;
        assert!(x_lo < 2.0 && x_hi > 6.0, "{x_lo}..{x_hi}");
        // Beside a scatter, the nice padding of the scatter decides.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&[0.0, 10.0]), &[1.0, 2.0], ScatterOptions::default())
            .unwrap();
        let alone = domains(&f).0;
        axes(&mut f).vspan(2.0, 6.0, span()).unwrap();
        assert_eq!(domains(&f).0, alone);
        // Beside lines, the exact extent of the lines stays.
        let mut f = lined();
        axes(&mut f).vspan(2.0, 6.0, span()).unwrap();
        assert_eq!(domains(&f).0, (0.0, 10.0));
    }

    #[test]
    fn rules_have_no_effect_on_the_baseline_or_the_categories() {
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .bar(
                vec!["a".into(), "b".into()],
                &[1.0, 2.0],
                BarOptions::default(),
            )
            .unwrap()
            .vline(AxisValue::Category("zzz".into()), rule())
            .unwrap();
        let marks = resolve_marks(f.single_axes().marks(), &Theme::pigment());
        assert_eq!(marks[1].x_categories().count(), 0);
        assert_eq!(marks[1].x_numbers().count(), 0);
        assert_eq!(marks[1].y_values().count(), 0);
        assert!(!marks[1].grows_from_baseline());
        assert_eq!(marks[1].line_width(), None);
        assert!(marks[1].is_reference() && !marks[0].is_reference());
        // The unknown name added no band.
        let r = resolve_axes(f.single_axes(), 5, 4);
        let AxisScale::Category(scale) = &r.x.scale else {
            panic!()
        };
        assert_eq!(scale.categories(), ["a", "b"]);
    }

    #[test]
    fn reference_keys_have_rule_and_span_kinds_and_no_anchor() {
        let theme = Theme::pigment();
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .hline(
                1.0,
                RuleOptions {
                    label: "h".into(),
                    dash: Dash::Dotted,
                    width: crate::SizeSpec::times(2.0).unwrap(),
                    ..rule()
                },
            )
            .unwrap()
            .vline(at(1.0), rule())
            .unwrap()
            .hspan(
                1.0,
                2.0,
                SpanOptions {
                    label: "s".into(),
                    ..span()
                },
            )
            .unwrap()
            .vspan(1.0, 2.0, span())
            .unwrap();
        let axes = f.single_axes();
        let resolved = resolve_marks(axes.marks(), &theme);
        let keys = series_keys(&resolved, &Limits::of(axes));
        assert_eq!(keys.len(), 4);
        let width = theme.drawn_line_width(crate::SizeSpec::times(2.0).unwrap());
        assert_eq!(
            keys[0].kind,
            SeriesKind::Rule {
                dash: Dash::Dotted,
                width
            }
        );
        assert_eq!(
            keys[1].kind,
            SeriesKind::Rule {
                dash: Dash::Solid,
                width: theme.drawn_line_width(crate::SizeSpec::Auto)
            }
        );
        assert_eq!(
            (keys[2].kind, keys[3].kind),
            (SeriesKind::Span, SeriesKind::Span)
        );
        assert!(keys.iter().all(|key| key.anchor.is_none()));
        assert_eq!(
            (keys[0].label, keys[2].label, keys[1].label),
            ("h", "s", "")
        );
        // Only lines take direct labels.
        assert!(
            SeriesKind::Line {
                dash: Dash::Solid,
                width: 1.0
            }
            .takes_direct_label()
        );
        assert!(keys.iter().all(|key| !key.kind.takes_direct_label()));
        // The two labels take the first slots; unlabelled rules and spans follow them.
        let palette = theme.palette();
        assert_eq!(
            series_colors(&resolved),
            [palette[0], palette[2], palette[1], palette[3]]
        );
    }

    #[test]
    fn an_unknown_category_is_refused_before_anything_is_drawn() {
        let build = |name: &str| {
            let mut f = figure(ScaleKind::Category, LIN);
            axes(&mut f)
                .vline(AxisValue::Category(name.into()), rule())
                .unwrap()
                .scatter(cats(&["Q1", "Q2"]), &[1.0, 2.0], ScatterOptions::default())
                .unwrap();
            f
        };
        assert!(check_references(&build("Q2"), "Figure.to_svg()").is_ok());
        let err = check_references(&build("Q5"), "Figure.to_svg()").unwrap_err();
        assert!(matches!(err, crate::Error::Data { .. }));
        assert_eq!(
            err.to_string(),
            "Figure.to_svg(): x is \"Q5\"; no mark on this axes has that category."
        );
        assert_eq!(
            (err.parameter(), err.hint()),
            (None, Some("draw a mark with that category first."))
        );
        // Linear axes have no categories to check.
        let mut f = figure(LIN, LIN);
        axes(&mut f).vline(at(3.0), rule()).unwrap();
        assert!(check_references(&f, "Figure.to_svg()").is_ok());
    }

    #[test]
    fn a_reference_on_the_other_axis_leaves_an_axis_with_explicit_ticks_alone() {
        let log = ScaleKind::Log { base: LogBase::TEN };
        let build = |x: ScaleKind, reference: Option<&dyn Fn(&mut Figure)>| {
            let mut f = figure(x, LIN);
            if x == LIN {
                axes(&mut f).xticks(&[0.0, 10.0]).unwrap();
            } else {
                axes(&mut f).xticks(&[1.0, 100.0]).unwrap();
            }
            axes(&mut f).ylim(0.0, 10.0).unwrap();
            if let Some(add) = reference {
                add(&mut f);
            }
            domains(&f)
        };
        let hline = |f: &mut Figure| {
            axes(f).hline(5.0, rule()).unwrap();
        };
        let hspan = |f: &mut Figure| {
            axes(f).hspan(2.0, 4.0, span()).unwrap();
        };
        for x in [LIN, log] {
            assert_eq!(build(x, None), build(x, Some(&hline)), "hline on {x:?}");
            assert_eq!(build(x, None), build(x, Some(&hspan)), "hspan on {x:?}");
        }
        // The same for y: a vertical reference leaves a y axis with explicit ticks alone.
        let build_y = |reference: bool| {
            let mut f = figure(LIN, LIN);
            axes(&mut f)
                .yticks(&[0.0, 10.0])
                .unwrap()
                .xlim(0.0, 10.0)
                .unwrap();
            if reference {
                axes(&mut f)
                    .vline(at(5.0), rule())
                    .unwrap()
                    .vspan(2.0, 4.0, span())
                    .unwrap();
            }
            domains(&f)
        };
        assert_eq!(build_y(false), build_y(true));
        // A reference that does give the axis values still pads it when alone.
        let mut f = figure(LIN, LIN);
        axes(&mut f).vspan(2.0, 6.0, span()).unwrap();
        let (lo, hi) = domains(&f).0;
        assert!(lo < 2.0 && hi > 6.0);
    }

    #[test]
    fn categories_of_marks_are_found_without_resolving_them() {
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .bar(
                vec!["a".into(), "b".into()],
                &[1.0, 2.0],
                BarOptions::default(),
            )
            .unwrap()
            .scatter(cats(&["c", "a"]), &[1.0, 2.0], ScatterOptions::default())
            .unwrap()
            .hline(1.0, rule())
            .unwrap();
        let marks = f.single_axes().marks();
        let found: Vec<&str> = marks
            .iter()
            .flat_map(mark_categories)
            .map(String::as_str)
            .collect();
        let resolved = resolve_marks(marks, &Theme::pigment());
        let expected: Vec<&str> = resolved.iter().flat_map(|m| m.x_categories()).collect();
        assert_eq!(found, expected);
    }

    /// A figure of linear axes holding one labelled band over `x` between `low` and `high`.
    fn band_figure(x: &[f64], low: &[f64], high: &[f64]) -> Figure {
        let mut f = figure(LIN, LIN);
        let options = BandOptions {
            label: "Band".to_owned(),
            ..BandOptions::default()
        };
        axes(&mut f).band(x, low, high, options).unwrap();
        f
    }

    #[test]
    fn a_band_gives_x_and_both_curves_to_the_domains() {
        let f = band_figure(&[0.0, 5.0, 10.0], &[1.0, 2.0, 0.5], &[5.0, 9.0, 4.0]);
        let marks = resolve_marks(f.single_axes().marks(), &Theme::pigment());
        let [band] = marks.as_slice() else {
            panic!("{marks:?}")
        };
        assert_eq!(band.x_numbers().collect::<Vec<_>>(), [0.0, 5.0, 10.0]);
        // Every low, then every high.
        assert_eq!(
            band.y_values().collect::<Vec<_>>(),
            [1.0, 2.0, 0.5, 5.0, 9.0, 4.0]
        );
        assert_eq!(band.x_categories().count(), 0);
        assert!(!band.grows_from_baseline() && !band.is_reference());
        assert_eq!(band.line_width(), None);
        // The y axis holds the lowest low and the highest high, padded as a scatter's is.
        let mut scatter = figure(LIN, LIN);
        axes(&mut scatter)
            .scatter(nums(&[0.0, 10.0]), &[0.5, 9.0], ScatterOptions::default())
            .unwrap();
        assert_eq!(domains(&f).1, domains(&scatter).1);
    }

    #[test]
    fn a_band_reads_the_exact_x_extent_like_a_line_and_pads_beside_other_marks() {
        // Alone, and beside a line: the exact extent.
        let f = band_figure(&[0.0, 10.0], &[1.0, 2.0], &[3.0, 4.0]);
        assert_eq!(domains(&f).0, (0.0, 10.0));
        let mut f = band_figure(&[0.0, 10.0], &[1.0, 2.0], &[3.0, 4.0]);
        axes(&mut f)
            .line(nums(&[2.0, 6.0]), &[2.0, 3.0], LineOptions::default())
            .unwrap();
        assert_eq!(domains(&f).0, (0.0, 10.0));
        // Beside a scatter, the nice padding of the scatter decides, and a band inside the
        // scatter's extent changes nothing.
        let mut scatter = figure(LIN, LIN);
        axes(&mut scatter)
            .scatter(nums(&[0.0, 10.0]), &[1.0, 2.0], ScatterOptions::default())
            .unwrap();
        let alone = domains(&scatter).0;
        assert_ne!(alone, (0.0, 10.0));
        axes(&mut scatter)
            .band(
                &[2.0, 6.0],
                &[1.0, 1.0],
                &[2.0, 2.0],
                BandOptions::default(),
            )
            .unwrap();
        assert_eq!(domains(&scatter).0, alone);
        // A band beside references: the references do not pad it.
        let mut f = band_figure(&[0.0, 10.0], &[1.0, 2.0], &[3.0, 4.0]);
        axes(&mut f).vspan(2.0, 6.0, span()).unwrap();
        assert_eq!(domains(&f).0, (0.0, 10.0));
    }

    #[test]
    fn a_band_key_is_a_square_swatch_without_an_anchor_and_takes_a_palette_slot() {
        let theme = Theme::pigment();
        let mut f = band_figure(&[0.0, 1.0], &[0.0, 0.0], &[1.0, 2.0]);
        axes(&mut f)
            .line(nums(&[0.0, 1.0]), &[0.5, 1.0], LineOptions::default())
            .unwrap();
        let axes = f.single_axes();
        let resolved = resolve_marks(axes.marks(), &theme);
        let keys = series_keys(&resolved, &Limits::of(axes));
        let [band, line] = keys.as_slice() else {
            panic!("{keys:?}")
        };
        assert_eq!(
            (band.label, band.kind, band.anchor, band.color),
            ("Band", SeriesKind::Span, None, theme.palette()[0])
        );
        assert!(!band.kind.takes_direct_label());
        assert_eq!(line.color, theme.palette()[1]);
    }

    /// A figure of linear axes holding labelled vertical error bars over `x` between `low` and
    /// `high`.
    fn bars_figure(x: &[f64], low: &[f64], high: &[f64]) -> Figure {
        let mut f = figure(LIN, LIN);
        let options = crate::model::ErrorBarOptions {
            label: "Interval".to_owned(),
            ..crate::model::ErrorBarOptions::default()
        };
        axes(&mut f)
            .y_error_bars(nums(x), low, high, options)
            .unwrap();
        f
    }

    #[test]
    fn vertical_error_bars_give_positions_to_x_and_limits_to_y() {
        let f = bars_figure(&[1.0, 5.0], &[0.0, 2.0], &[3.0, 9.0]);
        let marks = resolve_marks(f.single_axes().marks(), &Theme::pigment());
        let [bars] = marks.as_slice() else {
            panic!("{marks:?}")
        };
        assert_eq!(bars.x_numbers().collect::<Vec<_>>(), [1.0, 5.0]);
        // Every low, then every high.
        assert_eq!(bars.y_values().collect::<Vec<_>>(), [0.0, 2.0, 3.0, 9.0]);
        assert_eq!(bars.x_categories().count(), 0);
        assert!(!bars.grows_from_baseline() && !bars.is_reference());
        assert_eq!(bars.line_width(), None);
        // The domains are those of a scatter at the positions and at both limits.
        let mut scatter = figure(LIN, LIN);
        axes(&mut scatter)
            .scatter(nums(&[1.0, 5.0]), &[0.0, 9.0], ScatterOptions::default())
            .unwrap();
        assert_eq!(domains(&f), domains(&scatter));
    }

    #[test]
    fn horizontal_error_bars_give_limits_to_x_and_positions_to_y() {
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .x_error_bars(
                &[1.0, 5.0],
                &[0.0, 2.0],
                &[3.0, 9.0],
                crate::model::ErrorBarOptions::default(),
            )
            .unwrap();
        let marks = resolve_marks(f.single_axes().marks(), &Theme::pigment());
        let [bars] = marks.as_slice() else {
            panic!("{marks:?}")
        };
        assert_eq!(bars.x_numbers().collect::<Vec<_>>(), [0.0, 2.0, 3.0, 9.0]);
        assert_eq!(bars.y_values().collect::<Vec<_>>(), [1.0, 5.0]);
        assert_eq!(bars.x_categories().count(), 0);
        let mut scatter = figure(LIN, LIN);
        axes(&mut scatter)
            .scatter(nums(&[0.0, 9.0]), &[1.0, 5.0], ScatterOptions::default())
            .unwrap();
        assert_eq!(domains(&f), domains(&scatter));
    }

    #[test]
    fn error_bars_on_a_category_axis_give_their_categories_to_x() {
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .y_error_bars(
                cats(&["b", "a", "b"]),
                &[1.0, 2.0, 3.0],
                &[2.0, 3.0, 4.0],
                crate::model::ErrorBarOptions::default(),
            )
            .unwrap();
        let marks = resolve_marks(f.single_axes().marks(), &Theme::pigment());
        let [bars] = marks.as_slice() else {
            panic!("{marks:?}")
        };
        assert_eq!(bars.x_categories().collect::<Vec<_>>(), ["b", "a", "b"]);
        assert_eq!(bars.x_numbers().count(), 0);
        // The mark names the same categories without being resolved.
        let found: Vec<&str> = f
            .single_axes()
            .marks()
            .iter()
            .flat_map(mark_categories)
            .map(String::as_str)
            .collect();
        assert_eq!(found, ["b", "a", "b"]);
    }

    #[test]
    fn error_bars_that_give_x_nothing_leave_its_padding_as_it_was() {
        let line = |f: &mut Figure| {
            axes(f)
                .line(nums(&[0.0, 10.0]), &[1.0, 2.0], LineOptions::default())
                .unwrap();
        };
        let mut alone = figure(LIN, LIN);
        line(&mut alone);
        assert_eq!(domains(&alone).0, (0.0, 10.0));
        // Bars with no position, or only gaps, add nothing to either axis: the line's exact x
        // extent holds, for bars along y and along x.
        for gap in [vec![], vec![f64::NAN]] {
            let mut f = figure(LIN, LIN);
            line(&mut f);
            axes(&mut f)
                .y_error_bars(
                    nums(&gap),
                    &gap,
                    &gap,
                    crate::model::ErrorBarOptions::default(),
                )
                .unwrap()
                .x_error_bars(&gap, &gap, &gap, crate::model::ErrorBarOptions::default())
                .unwrap();
            assert_eq!(domains(&f).0, (0.0, 10.0));
            assert_eq!(domains(&f).1, domains(&alone).1);
        }
        // Bars that give x a value pad it as a scatter does.
        let mut f = figure(LIN, LIN);
        line(&mut f);
        axes(&mut f)
            .y_error_bars(
                nums(&[5.0]),
                &[1.0],
                &[2.0],
                crate::model::ErrorBarOptions::default(),
            )
            .unwrap();
        assert_ne!(domains(&f).0, (0.0, 10.0));
    }

    #[test]
    fn an_error_bar_key_has_its_orientation_and_width_takes_a_slot_and_no_direct_label() {
        let theme = Theme::pigment();
        let mut f = bars_figure(&[0.0, 1.0], &[0.0, 0.0], &[1.0, 2.0]);
        axes(&mut f)
            .x_error_bars(
                &[0.5],
                &[0.0],
                &[1.0],
                crate::model::ErrorBarOptions {
                    label: "Across".to_owned(),
                    width: crate::SizeSpec::times(2.0).unwrap(),
                    ..crate::model::ErrorBarOptions::default()
                },
            )
            .unwrap()
            .line(nums(&[0.0, 1.0]), &[0.5, 1.0], LineOptions::default())
            .unwrap();
        let axes = f.single_axes();
        let resolved = resolve_marks(axes.marks(), &theme);
        let keys = series_keys(&resolved, &Limits::of(axes));
        let [vertical, horizontal, line] = keys.as_slice() else {
            panic!("{keys:?}")
        };
        let drawn = theme.line_width();
        assert_eq!(
            (
                vertical.label,
                vertical.kind,
                vertical.anchor,
                vertical.color
            ),
            (
                "Interval",
                SeriesKind::ErrorBar {
                    vertical: true,
                    width: drawn
                },
                None,
                theme.palette()[0]
            )
        );
        assert_eq!(
            (horizontal.kind, horizontal.color),
            (
                SeriesKind::ErrorBar {
                    vertical: false,
                    width: 2.0 * drawn
                },
                theme.palette()[1]
            )
        );
        assert!(!vertical.kind.takes_direct_label() && !horizontal.kind.takes_direct_label());
        assert_eq!(line.color, theme.palette()[2]);
    }
}
