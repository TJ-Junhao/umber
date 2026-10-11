//! Resolve: data extents to scales to ticks, computed at render time.
//!
//! Auto limits: a continuous axis widens its data extent to nice limits, whose ticks are the
//! tick set that produced them, then pads that nice domain by 5 % of its span on each side (in
//! log space on a log axis), so marks never sit on the plot edge and the outermost ticks sit
//! just inside it. The ticks are not changed by the padding. The exception is the x axis of an
//! axes whose marks are all lines, which keeps the exact data extent. Explicit limits replace
//! the auto range as given and are never padded. A linear y axis that holds bars or a histogram
//! always includes 0, and a side of its nice domain that ends at 0 is not padded, so bars start
//! on the plot edge. A category axis lists the marks' categories in order of first appearance.
//! The baseline sits at 0 when 0 lies inside a linear y domain, and otherwise at the y axis's
//! lower limit before padding.
//! Tick positions are normalized to `[0, 1]` along the axis, so layout and drawing never touch
//! data values again.
//!
//! Marks are resolved here too, once per render: each mark becomes a [`ResolvedMark`], a typed
//! record that carries the mark's data and its [`ColorSpec`] resolved against the theme's
//! palette. Axes are resolved from these records, once per layout pass, and drawing reads them
//! instead of the marks.

use crate::binning::Histogram;
use crate::model::{
    Axes, AxisData, BarGroupMark, BarMark, BarSeriesView, HistMark, LineMark, Mark, ScatterMark,
};
use crate::scale::{Padding, is_single_value, log_offset};
use crate::stacking::Stacks;
use crate::{
    CategoryScale, ColorSpec, ContinuousScale, Dash, Marker, Rgba, ScaleKind, Tick, Transform,
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

/// Both axes of an [`Axes`], resolved.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Resolved {
    /// The x axis.
    pub x: ResolvedAxis,
    /// The y axis.
    pub y: ResolvedAxis,
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

/// How a continuous axis without explicit limits fits its data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoLimits {
    /// The limits are the data extent.
    Exact,
    /// The data extent is widened to nice limits, which carry a tick at both ends, and the nice
    /// domain is then padded on the given sides (see [`ContinuousScale::padded`]); the ticks
    /// stay as they are.
    Nice(Padding),
}

/// What an axis fixes before any data is seen: whether it lists categories, and for a
/// continuous axis its transform and its explicit limits, if set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AxisLimits {
    /// A category axis: its categories come from the marks.
    Category,
    /// A linear or log axis.
    Continuous {
        /// The transform of the axis.
        transform: Transform,
        /// The scale of the explicit limits; `None` to fit the data.
        explicit: Option<ContinuousScale>,
    },
}

impl AxisLimits {
    /// The limits of an axis of scale `kind` with the explicit limits `explicit`.
    fn new(kind: ScaleKind, explicit: Option<ContinuousScale>) -> AxisLimits {
        match kind.transform() {
            Some(transform) => AxisLimits::Continuous {
                transform,
                explicit,
            },
            None => AxisLimits::Category,
        }
    }
}

/// The limits of both axes of an [`Axes`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Limits {
    /// The x axis.
    pub x: AxisLimits,
    /// The y axis.
    pub y: AxisLimits,
}

impl Limits {
    /// The scale kinds and explicit limits of `axes`.
    pub fn of(axes: &Axes) -> Limits {
        Limits {
            x: AxisLimits::new(axes.x_scale(), axes.x_limit_scale()),
            y: AxisLimits::new(axes.y_scale(), axes.y_limit_scale()),
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

/// A continuous axis resolved by [`continuous`], with the lower limit of its domain before
/// padding.
struct ContinuousAxis {
    /// The axis.
    axis: ResolvedAxis,
    /// The lower limit before padding: the nice lower limit of a padded axis, the domain's own
    /// lower limit otherwise.
    unpadded_low: f64,
}

/// A continuous axis: the scale of the explicit limits when set; otherwise the empty-data
/// range (0 to 1, or 1 to 10 on log) when there is no finite data; otherwise the data extent,
/// fitted as `auto` says. Nice axes take the ticks that produced their limits; the others take
/// `ticks()`. Only a nice axis pads its domain, after its ticks are chosen; an extent that
/// counts as a single value is padded by [`ContinuousScale::from_extent`] alone.
fn continuous(
    transform: Transform,
    limits: Option<ContinuousScale>,
    data: Option<(f64, f64)>,
    auto: AutoLimits,
    target: usize,
) -> ContinuousAxis {
    let (scale, unpadded, ticks) = if let Some(scale) = limits {
        (scale, scale, scale.ticks(target))
    } else if data.is_none() {
        // The fallback range is final on every axis: never widened with `nice`.
        let scale = ContinuousScale::unit(transform);
        (scale, scale, scale.ticks(target))
    } else {
        let scale = ContinuousScale::from_extent(transform, data, CONTEXT)
            .unwrap_or_else(|_| ContinuousScale::unit(transform));
        match auto {
            AutoLimits::Exact => (scale, scale, scale.ticks(target)),
            AutoLimits::Nice(padding) => {
                let (nice, ticks) = scale.nice_with_ticks(target);
                let single = data.is_some_and(|(lo, hi)| is_single_value(transform, lo, hi));
                let padded = if single { nice } else { nice.padded(padding) };
                (padded, nice, ticks)
            }
        }
    };
    ContinuousAxis {
        axis: ResolvedAxis {
            ticks: positioned(&scale, ticks),
            scale: AxisScale::Continuous(scale),
        },
        unpadded_low: unpadded.domain().0,
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
fn categories(marks: &[ResolvedMark<'_>]) -> CategoryScale {
    CategoryScale::from_first_appearance(marks.iter().flat_map(|mark| mark.x_categories()))
}

/// Resolves both axes from the resolved `marks` and the axes' `limits`, for about `x_target`
/// and `y_target` ticks.
pub(crate) fn resolve(
    marks: &[ResolvedMark<'_>],
    limits: &Limits,
    x_target: usize,
    y_target: usize,
) -> Resolved {
    let x = match limits.x {
        AxisLimits::Continuous {
            transform,
            explicit,
        } => {
            let data = extent(marks.iter().flat_map(|mark| mark.x_numbers()));
            let all_lines = marks
                .iter()
                .all(|mark| matches!(mark, ResolvedMark::Line { .. }));
            let auto = if all_lines {
                AutoLimits::Exact
            } else {
                AutoLimits::Nice(Padding::BothSides)
            };
            continuous(transform, explicit, data, auto, x_target).axis
        }
        AxisLimits::Category => {
            let scale = categories(marks);
            ResolvedAxis {
                ticks: category_ticks(&scale),
                scale: AxisScale::Category(scale),
            }
        }
    };
    let (y, baseline) = match limits.y {
        AxisLimits::Continuous {
            transform,
            explicit,
        } => {
            let data = extent(marks.iter().flat_map(|mark| mark.y_values()));
            // Bars grow from 0 on a linear axis, so the axis shows 0 and keeps it on the plot
            // edge when it is a limit. On a log axis they grow from the baseline.
            let bars = marks.iter().any(ResolvedMark::grows_from_baseline);
            let (data, padding) = if bars && transform == Transform::Linear {
                let with_zero = data.map(|(lo, hi)| (lo.min(0.0), hi.max(0.0)));
                (with_zero, Padding::KeepZero)
            } else {
                (data, Padding::BothSides)
            };
            let auto = AutoLimits::Nice(padding);
            let ContinuousAxis { axis, unpadded_low } =
                continuous(transform, explicit, data, auto, y_target);
            let baseline = match &axis.scale {
                AxisScale::Continuous(scale) => baseline(scale, transform, unpadded_low),
                AxisScale::Category(_) => NormalizedPos::LOW,
            };
            (axis, baseline)
        }
        AxisLimits::Category => {
            // Marks always hold numeric y, so a category y axis never has categories.
            let scale = CategoryScale::from_first_appearance(std::iter::empty());
            let axis = ResolvedAxis {
                ticks: category_ticks(&scale),
                scale: AxisScale::Category(scale),
            };
            (axis, NormalizedPos::LOW)
        }
    };
    Resolved { x, y, baseline }
}

/// A mark resolved for drawing: one variant per mark kind, each holding the mark and its color
/// resolved against the theme's palette. Axes, the legend and drawing read marks through these
/// records only.
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
    },
    /// A scatter mark.
    Scatter {
        /// The mark.
        mark: &'a ScatterMark,
        /// The marker color.
        color: Rgba,
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
        series: Box<[(BarSeriesView<'a>, Rgba)]>,
    },
    /// A stacked bar mark.
    Stacked {
        /// The mark.
        mark: &'a BarGroupMark,
        /// Each series of the mark, in order, with its resolved color.
        series: Box<[(BarSeriesView<'a>, Rgba)]>,
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
}

impl<'a> ResolvedMark<'a> {
    /// The values the mark contributes to a continuous x domain, in data order; none when its
    /// x values are categories. See [`ResolvedMark`] for the contract.
    ///
    /// A histogram yields the low and the high end of its binned range
    /// ([`Histogram::range`]), and nothing when it has no finite value.
    pub fn x_numbers(&self) -> Box<dyn Iterator<Item = f64> + '_> {
        let values: &[f64] = match self {
            ResolvedMark::Line { mark, .. } => numbers(mark.x()),
            ResolvedMark::Scatter { mark, .. } => numbers(mark.x()),
            ResolvedMark::Bar { .. }
            | ResolvedMark::Grouped { .. }
            | ResolvedMark::Stacked { .. } => &[],
            ResolvedMark::Hist { histogram, .. } => {
                let range = histogram.as_ref().map(Histogram::range);
                return Box::new(range.into_iter().flat_map(|(lo, hi)| [lo, hi]));
            }
        };
        Box::new(values.iter().copied())
    }

    /// The categories the mark contributes to a category x axis, in data order, repeats
    /// included; none when its x values are numbers. See [`ResolvedMark`] for the contract.
    pub fn x_categories(&self) -> impl Iterator<Item = &str> + '_ {
        match self {
            ResolvedMark::Line { mark, .. } => category_names(mark.x()),
            ResolvedMark::Scatter { mark, .. } => category_names(mark.x()),
            ResolvedMark::Bar { mark, .. } => mark.categories(),
            ResolvedMark::Grouped { mark, .. } | ResolvedMark::Stacked { mark, .. } => {
                mark.categories()
            }
            ResolvedMark::Hist { .. } => &[],
        }
        .iter()
        .map(String::as_str)
    }

    /// The values the mark contributes to the y domain. See [`ResolvedMark`] for the contract.
    ///
    /// Lines, scatters and bars yield their y values or heights in data order, and grouped bars
    /// the heights of each series in turn. Stacked bars yield the end of every segment away from
    /// 0 ([`Stacks::ends`]), never 0 itself: on a linear axis the bars put 0 into the domain
    /// anyway (see [`grows_from_baseline`](Self::grows_from_baseline)), and on a log axis, where
    /// every height is positive, these are the positive segment ends, which a log domain can
    /// hold. Histograms yield the count of every non-empty bin, in bin order, and so never 0
    /// either: on a log axis the domain spans the smallest positive count to the largest.
    pub fn y_values(&self) -> Box<dyn Iterator<Item = f64> + '_> {
        let values: &[f64] = match self {
            ResolvedMark::Line { mark, .. } => mark.y(),
            ResolvedMark::Scatter { mark, .. } => mark.y(),
            ResolvedMark::Bar { mark, .. } => mark.heights(),
            ResolvedMark::Grouped { series, .. } => {
                return Box::new(
                    series
                        .iter()
                        .flat_map(|(view, _)| view.heights().iter().copied()),
                );
            }
            ResolvedMark::Stacked { stacks, .. } => return Box::new(stacks.ends()),
            ResolvedMark::Hist { histogram, .. } => {
                let counts = histogram.as_ref().map_or(&[][..], Histogram::counts);
                return Box::new(
                    counts
                        .iter()
                        .filter(|&&count| count > 0)
                        .map(|&count| count as f64),
                );
            }
        };
        Box::new(values.iter().copied())
    }

    /// Whether the mark grows from the baseline, as bars and histogram bins do: such a mark puts
    /// 0 into a linear y domain.
    pub fn grows_from_baseline(&self) -> bool {
        match self {
            ResolvedMark::Line { .. } | ResolvedMark::Scatter { .. } => false,
            ResolvedMark::Bar { .. }
            | ResolvedMark::Grouped { .. }
            | ResolvedMark::Stacked { .. }
            | ResolvedMark::Hist { .. } => true,
        }
    }

    /// The series the mark shows, in order, one key each: what a legend lists for the mark, on
    /// an axes whose axes have the limits `limits`.
    pub fn keys(&self, limits: &Limits) -> Vec<SeriesKey<'a>> {
        match self {
            &ResolvedMark::Line { mark, color } => {
                let options = mark.options();
                vec![SeriesKey {
                    label: &options.label,
                    color,
                    kind: SeriesKind::Line { dash: options.dash },
                    anchor: Anchor::of_line(mark, limits),
                }]
            }
            &ResolvedMark::Scatter { mark, color } => {
                let options = mark.options();
                vec![SeriesKey {
                    label: &options.label,
                    color,
                    kind: SeriesKind::Points {
                        marker: options.marker,
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
    /// The anchor of a line series; `None` for a line without one and for every other series.
    pub anchor: Option<Anchor<'a>>,
}

/// The x value of a line's point, as the line holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AnchorX<'a> {
    /// A number on a linear or log axis; finite.
    Number(f64),
    /// A category name.
    Category(&'a str),
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
    /// [`EndKind::Point`] anchor, unless the next point in data order has finite x and y and an
    /// x past the high end of the domain: the line then leaves the plot between the two, and
    /// the anchor is the [`EndKind::Edge`] where it crosses the high end, the two points
    /// interpolated in axis space (see [`crossing_at`]). A next point with a NaN breaks the
    /// line, so the line ends at the last point. `None` when the line has no point inside.
    ///
    /// Only explicit limits can leave a finite point outside the x domain: auto limits always
    /// contain every finite x value, and a category axis holds every category of its marks. So
    /// the limits decide the anchor before any layout pass, and it is the same in all of them.
    fn of_line(line: &'a LineMark, limits: &Limits) -> Option<Anchor<'a>> {
        let y = line.y();
        match line.x() {
            AxisData::Numbers(xs) => {
                let range = match limits.x {
                    AxisLimits::Continuous {
                        transform,
                        explicit: Some(scale),
                    } => Some((transform, scale.domain())),
                    AxisLimits::Continuous { explicit: None, .. } | AxisLimits::Category => None,
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
                if !(next_x.is_finite() && next_y.is_finite() && next_x > hi) {
                    return Some(point);
                }
                let y_transform = match limits.y {
                    AxisLimits::Continuous { transform, .. } => transform,
                    AxisLimits::Category => Transform::Linear,
                };
                let crossing = crossing_at(
                    (x_transform, y_transform),
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
}

/// The y where the straight segment from `from` to `to` crosses `x = edge`, on axes with the
/// x and y transforms `transforms`. The segment is straight in axis space, where lines are
/// drawn: in the values on a linear axis and in their logarithms on a log axis. So the share of
/// the way to `edge` is measured in axis space along x, and y is interpolated by that share in
/// axis space along y.
///
/// `from.0 <= edge < to.0`, every value finite, and every value on a log axis positive, as the
/// model validates them; the result then lies between `from.1` and `to.1`.
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
        Transform::Log(_) => from.1 * (share * log_offset(to.1, from.1)).exp(),
    }
}

/// How a series is drawn, as far as its legend swatch needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SeriesKind {
    /// A line with its dash style.
    Line {
        /// The dash style.
        dash: Dash,
    },
    /// Individual markers of one shape.
    Points {
        /// The marker shape.
        marker: Marker,
    },
    /// Filled bars.
    Bar,
}

/// Each mark resolved for drawing, in mark order, with its colors resolved against `palette`,
/// the stacks of stacked bars computed and the values of histograms binned. Every series takes
/// its color in series order, the series of a grouped or stacked mark one after another: `Auto`
/// colors take palette slots in that order; `Palette` and `Fixed` colors do not consume a slot.
pub(crate) fn resolve_marks<'a>(marks: &'a [Mark], palette: &[Rgba]) -> Vec<ResolvedMark<'a>> {
    let mut next_auto = 0_usize;
    let mut color_of = |spec: ColorSpec| {
        let color = spec.resolve(palette, next_auto);
        if matches!(spec, ColorSpec::Auto(_)) {
            next_auto += 1;
        }
        color
    };
    marks
        .iter()
        .map(|mark| match mark {
            Mark::Line(mark) => ResolvedMark::Line {
                mark,
                color: color_of(mark.options().color),
            },
            Mark::Scatter(mark) => ResolvedMark::Scatter {
                mark,
                color: color_of(mark.options().color),
            },
            Mark::Bar(mark) => ResolvedMark::Bar {
                mark,
                color: color_of(mark.options().color),
            },
            Mark::GroupedBar(mark) => ResolvedMark::Grouped {
                mark,
                series: mark
                    .series()
                    .map(|view| (view, color_of(view.color())))
                    .collect(),
            },
            Mark::StackedBar(mark) => {
                let series: Box<[_]> = mark
                    .series()
                    .map(|view| (view, color_of(view.color())))
                    .collect();
                let heights: Vec<&[f64]> = series.iter().map(|(view, _)| view.heights()).collect();
                ResolvedMark::Stacked {
                    mark,
                    stacks: Stacks::new(mark.categories().len(), &heights),
                    series,
                }
            }
            Mark::Hist(mark) => ResolvedMark::Hist {
                mark,
                histogram: Histogram::from_values(mark.values(), mark.bin_count()),
                color: color_of(mark.color()),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BarOptions, BarSeries, Figure, HistOptions, LineOptions, ScatterOptions};
    use crate::{LogBase, ScaleKind, Theme};

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

    fn axes(figure: &mut Figure) -> &mut Axes {
        figure.axes_mut().unwrap()
    }

    /// The resolved color of every series of `resolved`, in order. Colors do not depend on the
    /// axis limits.
    fn series_colors(resolved: &[ResolvedMark<'_>]) -> Vec<Rgba> {
        series_keys(
            resolved,
            &Limits {
                x: AxisLimits::Category,
                y: AxisLimits::Category,
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
        let marks = resolve_marks(axes.marks(), &[]);
        resolve(&marks, &Limits::of(axes), x_target, y_target)
    }

    fn domains(figure: &Figure) -> ((f64, f64), (f64, f64)) {
        let r = resolve_axes(figure.axes().unwrap(), 5, 4);
        (
            r.x.continuous().unwrap().domain(),
            r.y.continuous().unwrap().domain(),
        )
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

    fn labels(axis: &ResolvedAxis) -> Vec<&str> {
        axis.ticks.iter().map(|t| t.label.as_str()).collect()
    }

    /// The position of the first and last tick on a padded nice axis: 0.05 / 1.1 of the axis
    /// in from each end.
    fn assert_end_ticks_inset(axis: &ResolvedAxis) {
        let inset = 0.05 / 1.1;
        let (first, last) = (axis.ticks.first().unwrap(), axis.ticks.last().unwrap());
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(labels(&r.x), ["0.0", "2.5", "5.0", "7.5", "10.0"]);
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        for axis in [&r.x, &r.y] {
            assert_end_ticks_inset(axis);
        }
        assert_eq!(labels(&r.x), ["10", "30", "50", "70", "90"]);
        assert_eq!(labels(&r.y), ["0", "10", "20", "30", "40"]);

        // The same on a log axis.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(log, LIN);
        axes(&mut f)
            .scatter(nums(&[12.0, 88.0]), &[1.0, 2.0], ScatterOptions::default())
            .unwrap();
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        let domain = r.x.continuous().unwrap().domain();
        assert!(close(domain, pad_log(10.0, 100.0)), "{domain:?}");
        assert_eq!(labels(&r.x), ["10", "100"]);
        assert_end_ticks_inset(&r.x);
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        // 0 is never inside a log range: the baseline sits at the nice lower limit, the
        // lowest tick, inside the padded domain.
        assert_eq!(r.baseline, r.y.ticks[0].pos);
        assert!(
            (r.baseline.get() - 0.05 / 1.1).abs() < 1e-9,
            "{:?}",
            r.baseline
        );
        assert_eq!(labels(&r.y), ["1", "10", "100", "1000", "10000", "100000"]);
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
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
        let a = axes(&mut f);
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
        let marks = f.axes().unwrap().marks();
        let resolved = resolve_marks(marks, &palette);
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
        assert!(resolve_marks(&[], &palette).is_empty());
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
        let resolved = resolve_marks(f.axes().unwrap().marks(), &palette);
        let keys = series_keys(&resolved, &Limits::of(f.axes().unwrap()));
        assert_eq!(
            keys,
            [
                SeriesKey {
                    label: "trend",
                    color: palette[0],
                    kind: SeriesKind::Line { dash: Dash::Dashed },
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
        assert_eq!(resolve_axes(f.axes().unwrap(), 5, 4).baseline.get(), 0.25);
        // Explicit limits are not padded: their low limit is the plot bottom.
        axes(&mut f).ylim(5.0, 30.0).unwrap();
        assert_eq!(resolve_axes(f.axes().unwrap(), 5, 4).baseline.get(), 0.0);
        axes(&mut f).ylim(-30.0, -5.0).unwrap();
        assert_eq!(resolve_axes(f.axes().unwrap(), 5, 4).baseline.get(), 0.0);
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(LIN, log);
        axes(&mut f)
            .line(nums(&[0.0, 1.0]), &[3.0, 4.2e4], LineOptions::default())
            .unwrap()
            .ylim(2.0, 5e4)
            .unwrap();
        assert_eq!(resolve_axes(f.axes().unwrap(), 5, 4).baseline.get(), 0.0);
    }

    #[test]
    fn baseline_sits_at_zero_or_the_nice_lower_limit() {
        // Auto limits that hold 0: the baseline is at 0, wherever 0 is.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .line(nums(&[0.0, 1.0]), &[-12.0, 36.5], LineOptions::default())
            .unwrap();
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        let scale = r.y.continuous().unwrap();
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
            let r = resolve_axes(f.axes().unwrap(), 5, 4);
            let lowest = &r.y.ticks[0];
            assert_eq!(r.baseline, lowest.pos);
            assert!(
                (r.baseline.get() - 0.05 / 1.1).abs() < 1e-9,
                "{:?}",
                r.baseline
            );
            let expected = if ys[0] > 0.0 { "35" } else { "\u{2212}65" };
            assert_eq!(lowest.label, expected);
        }
        // Log auto limits: the lowest power of the base, 10 here.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = figure(LIN, log);
        axes(&mut f)
            .scatter(nums(&[0.0, 1.0]), &[12.0, 880.0], ScatterOptions::default())
            .unwrap();
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(r.y.ticks[0].label, "10");
        assert_eq!(r.baseline, r.y.ticks[0].pos);
        assert!(r.baseline.get() > 0.0);
        // A single value is not padded, so its nice lower limit is the plot bottom.
        let mut f = figure(LIN, LIN);
        axes(&mut f)
            .scatter(nums(&[1.0]), &[20.0], ScatterOptions::default())
            .unwrap();
        assert_eq!(resolve_axes(f.axes().unwrap(), 5, 4).baseline.get(), 0.0);
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
        // 35..65 is already nice; the domain grows by 1.5 on each side, so the line's minimum
        // and maximum no longer sit on the plot edge, and the ticks stay 35..65.
        assert_ne!((y0, y1), (35.0, 65.0));
        assert!(close((y0, y1), (33.5, 66.5)), "{y0} {y1}");
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(labels(&r.y), ["35", "45", "55", "65"]);
        assert_end_ticks_inset(&r.y);
        // The x axis of an all-line axes keeps the exact extent, unpadded.
        assert_eq!((x0, x1), (0.0, 12.0));
        assert_eq!(r.x.ticks.first().map(|t| t.pos.get()), Some(0.0));
        assert_eq!(r.x.ticks.last().map(|t| t.pos.get()), Some(1.0));
    }

    #[test]
    fn log_auto_limits_pad_in_log_space() {
        // The nice limits 10..1000 span two decades; 5 % of that is a factor of 10^0.1 on each
        // side. The ticks stay the powers inside the nice limits.
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
        for d in [x, y] {
            assert!(close(d, (10.0 / factor, 1000.0 * factor)), "{d:?}");
        }
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(labels(&r.x), ["10", "100", "1000"]);
        assert_end_ticks_inset(&r.y);
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
        resolve_axes(figure.axes().unwrap(), 5, 4)
            .y
            .continuous()
            .unwrap()
            .domain()
    }

    #[test]
    fn bar_y_extent_includes_zero() {
        // Positive bars: the nice limits 0..40, padded above only, so the bars start on the
        // bottom edge, where the baseline and the first tick sit.
        let f = bar_figure(LIN, &[12.0, 36.5]);
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(y_domain(&f), (0.0, 42.0));
        assert_eq!(labels(&r.y), ["0", "10", "20", "30", "40"]);
        assert_eq!(r.y.ticks[0].pos.get(), 0.0);
        assert_eq!(r.baseline.get(), 0.0);

        // Negative bars: the mirror image, padded below only.
        let f = bar_figure(LIN, &[-12.0, -36.5]);
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(y_domain(&f), (-42.0, 0.0));
        assert_eq!(r.baseline.get(), 1.0);

        // Bars on both sides of 0: both sides padded as usual.
        let f = bar_figure(LIN, &[-12.0, 36.5]);
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_end_ticks_inset(&r.y);
        assert!(labels(&r.y).contains(&"0"));

        // A line below 0 on the same axes moves the lower limit off 0, so it is padded.
        let mut f = bar_figure(LIN, &[12.0, 36.5]);
        axes(&mut f)
            .line(
                AxisData::Categories(names(&["c0", "c1"])),
                &[-9.0, 1.0],
                LineOptions::default(),
            )
            .unwrap();
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_end_ticks_inset(&r.y);

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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(labels(&r.x), ["b", "a", "c", "d", "e", "f"]);
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
        let resolved = resolve_marks(f.axes().unwrap().marks(), &palette);
        let colors = series_colors(&resolved);
        assert_eq!(
            colors,
            [palette[0], palette[1], fixed, palette[6], palette[2]]
        );
        let keys = series_keys(&resolved, &Limits::of(f.axes().unwrap()));
        assert_eq!(
            keys[1],
            SeriesKey {
                label: "first",
                color: palette[1],
                kind: SeriesKind::Bar,
                anchor: None,
            }
        );
        assert_eq!(keys[4].label, "last");
    }

    fn bar_series(color: ColorSpec, heights: &[f64]) -> BarSeries {
        BarSeries {
            label: String::new(),
            heights: heights.to_vec(),
            color,
        }
    }

    fn auto(heights: &[f64]) -> BarSeries {
        bar_series(ColorSpec::AUTO, heights)
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
    fn stacked_y_extent_spans_the_stacks_and_zero() {
        // Category a stacks to 12 + 24.5 = 36.5; nothing below 0, so 0 stays on the edge.
        let mut f = figure(ScaleKind::Category, LIN);
        axes(&mut f)
            .stacked_bar(
                names(&["a", "b"]),
                vec![auto(&[12.0, 5.0]), auto(&[24.5, f64::NAN])],
            )
            .unwrap();
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        let (lo, hi) = y_domain(&f);
        assert!(lo < -19.0 && hi > 36.0, "{lo} {hi}");
        let nice = (
            r.y.ticks[0].label.clone(),
            r.y.ticks.last().unwrap().label.clone(),
        );
        assert_ne!(nice.0, "0");
        assert_ne!(nice.1, "0");
        assert_end_ticks_inset(&r.y);
        assert!(labels(&r.y).contains(&"0"));
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
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
                    bar_series(ColorSpec::Fixed(fixed), &[1.0]),
                    bar_series(at(5), &[1.0]),
                    auto(&[1.0]),
                ],
            )
            .unwrap()
            .line(cats(&["a"]), &[1.0], LineOptions::default())
            .unwrap()
            .stacked_bar(
                names(&["a"]),
                vec![auto(&[1.0]), bar_series(at(0), &[1.0]), auto(&[2.0])],
            )
            .unwrap();
        let resolved = resolve_marks(f.axes().unwrap().marks(), &palette);
        assert_eq!(
            series_colors(&resolved),
            [
                palette[0], palette[1], fixed, palette[5], palette[2], palette[3], palette[4],
                palette[0], palette[5],
            ]
        );
        // Every grouped or stacked series is a legend key of its own, drawn as a bar.
        let keys = series_keys(&resolved, &Limits::of(f.axes().unwrap()));
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(labels(&r.x), ["b", "c", "a", "d"]);
    }

    /// Linear axes holding one histogram of `values` in `bins` bins, on a y axis of `y`.
    fn hist_figure(y: ScaleKind, values: &[f64], bins: usize) -> Figure {
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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        let (x, _) = domains(&f);
        assert!(close(x, pad(10.0, 90.0)), "{x:?}");
        assert_eq!(labels(&r.x), ["10", "30", "50", "70", "90"]);
        assert_end_ticks_inset(&r.x);

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
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
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
        // to the unit range 1..10).
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut values = vec![0.0];
        values.extend(std::iter::repeat_n(4.0, 40));
        let f = hist_figure(log, &values, 4);
        let domain = y_domain(&f);
        assert!(close(domain, pad_log(1.0, 100.0)), "{domain:?}");
        // The same domain as a line through exactly the smallest and largest positive count.
        let mut g = figure(LIN, log);
        axes(&mut g)
            .line(nums(&[0.0, 1.0]), &[1.0, 40.0], LineOptions::default())
            .unwrap();
        assert_eq!(domain, y_domain(&g));
        let r = resolve_axes(f.axes().unwrap(), 5, 4);
        assert_eq!(r.baseline, r.y.ticks[0].pos);
        assert_eq!(r.y.ticks[0].label, "1");
    }

    #[test]
    fn hist_without_finite_values_falls_back_to_the_unit_range() {
        for values in [&[][..], &[f64::NAN, f64::NAN][..]] {
            let f = hist_figure(LIN, values, 30);
            assert_eq!(domains(&f), ((0.0, 1.0), (0.0, 1.0)));
            let marks = resolve_marks(f.axes().unwrap().marks(), &[]);
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
        let resolved = resolve_marks(f.axes().unwrap().marks(), &palette);
        assert_eq!(
            series_colors(&resolved),
            [palette[0], palette[1], palette[2]]
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
            resolved[1].keys(&Limits::of(f.axes().unwrap())),
            [SeriesKey {
                label: "sizes",
                color: palette[1],
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
        let axes = f.axes().unwrap();
        let resolved = resolve_marks(axes.marks(), &[]);
        let keys = series_keys(&resolved, &Limits::of(axes));
        keys[0].anchor.map(|anchor| match anchor.x {
            AnchorX::Number(x) => (x, anchor.y, anchor.end),
            AnchorX::Category(name) => panic!("expected a number anchor, got {name}"),
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
        let axes = f.axes().unwrap();
        let resolved = resolve_marks(axes.marks(), &[]);
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
        // A last point on the high end, with the line going on: the edge is that point.
        assert_eq!(number_anchor(lin, &x, &y, Some((0.0, 2.0))), edge(2.0, 7.0));
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
        let axes = f.axes().unwrap();
        let resolved = resolve_marks(axes.marks(), &[]);
        let anchors: Vec<bool> = series_keys(&resolved, &Limits::of(axes))
            .iter()
            .map(|key| key.anchor.is_some())
            .collect();
        assert_eq!(anchors, [true, false, false]);
    }
}
