//! Automatic limits that fit whole bubble markers, outlines included, and the value labels at
//! bar ends on the final plot.
//!
//! A bubble's extent, like a label's, is in points while an axis domain is in data units, and
//! the plot size is known only once the layout is solved, so the final layout loop asks the fits
//! again after every solve ([`BubbleFits::update`]). A bubble reaches as far on both sides of
//! its centre; a bar's label reaches past the bar's end on one side only ([`EndReach`]), and is
//! fitted as a bubble is, with no reach on the other side. A fit of bar labels alone is made
//! only once a label reaches past the plot edge, so a range that already holds every label is
//! kept as it is; a fit holding a bubble is always made. A fit starts from the axis's raw
//! extent, the extent of the data and of any explicit tick positions as resolve sees it, and
//! widens it to the smallest extent in which every bubble reaches no further than the plot edge
//! at its share of the plot length. Resolve then pads that extent once (rounding it first on log
//! axes), as it does any data extent.
//!
//! Log-axis rounding, padding or a smaller plot after the solve can still leave a bubble past the plot
//! edge. The fit is then made again from the same raw extent, never from the padded domain, so
//! resolve still applies padding and log-axis rounding once: on the shortest plot lengths seen so far when a plot
//! shrank; otherwise padding or log-axis rounding moved the bubble out, and every bubble's reach is
//! measured against at least the span of the final domain, which is enough as long
//! as that span does not grow. The lengths only shrink and the span only grows, so the extents
//! only grow and the fits settle.
//!
//! A time axis is fitted in seconds from its origin, the earliest time of its data and explicit
//! ticks (see [`resolve::time_extents`]), as a linear axis is in its own units.
//!
//! Only the bubbles that are drawn are fitted: a bubble whose value maps to no color has no
//! fill and is not drawn ([`has_fill`]). A marker's reach is first estimated with the widest
//! outline, that of one color; when the estimate leaves no extent, the fit measures each
//! marker with its own outline, the thinner one of a color-mapped mark, so that only the
//! markers as drawn decide that the bubbles do not fit.
//!
//! When no extent holds the bubbles of an axis, the layout fails ([`BubbleFits::failure`]): a
//! bubble reaches at least half the plot length (the furthest reaches towards both ends
//! together take the whole plot), or the fitted extent leaves the supported numeric range.
//! The failure names the bubbles whose bound fails, and estimates the plot lengths that hold
//! them ([`BubbleFit::sufficient_length`]); the pipeline starts from that estimate to find, by
//! laying the figure out at larger sizes, a figure size that holds them, which its error
//! names. A layout that cannot hold the fitted limits fails as layouts do. Explicit limits are
//! never fitted, and the plot clips the bubbles that reach past them.
//!
//! Only a fit that does not settle within [`MAX_REFITS`] attempts, or within the rounds of the
//! layout, still falls back: its axis keeps the limits of the data positions alone, as it would
//! without bubble extents, and the plot clips the bubbles that overflow. Each attempt holds
//! the bubbles on the plots it was made for, and the plots changed again when the axes took
//! the fitted limits, so no plot size can be named that would settle it. A fit of bar labels
//! alone that cannot be made falls back too; the labels that then do not fit fail the render
//! ([`crate::draw::bar_labels`]).

use crate::color_scale::ResolvedColorScale;
use crate::draw::{MARKER_OUTLINE, has_fill, marker_outline, marker_reach};
use crate::error::{number, quoted};
use crate::layout::PlotSize;
use crate::model::ShareGroups;
use crate::model::SharedValues;
use crate::resolve::{
    self, AxisLimits, AxisScale, ContinuousLimits, Limits, Resolved, ResolvedMark, TimeLimits,
    ValueAxis,
};
use crate::scale::{Padding, log_offset, seconds_between};
use crate::{ContinuousScale, Error, Instant, Theme, Transform};

/// The most times a fit is made again after its first, before its axis keeps the limits of
/// the data positions.
const MAX_REFITS: usize = 8;
/// A drawing-space allowance, in points, added to every bubble's reach so that the final `f32`
/// path coordinates stay inside the plot.
const PATH_ALLOWANCE: f64 = 1e-4;
/// The error context of the raw extent's scale; its errors are never reported.
const CONTEXT: &str = "Figure.to_scene()";
/// The share of the plot length that the estimate of a plot holding bubbles too large for
/// their plot leaves to the data positions: the bubbles' reaches take the rest, so the fitted
/// limits are at most twice the data's extent.
const DATA_SHARE: f64 = 0.5;
/// The longest plot, in points, that the estimate of a plot holding bubbles considers: the
/// side of the largest figure inside its padding.
const LONGEST_PLOT: f64 = crate::model::MAX_FIGURE_IN * 72.0 - 2.0 * crate::layout::PADDING;
/// The bisection steps that find the shortest plot length holding the bubbles within the
/// supported range.
const BISECTION_STEPS: usize = 64;
/// The hint of the error for bubbles too large for their plot.
const HINT: &str = "make the figure larger, or set explicit limits, which clip the bubbles.";
/// The hint of the error for bubbles that no figure holds within the supported range.
const RANGE_HINT: &str = "set explicit limits, which clip the bubbles.";

/// The axis a fit is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    /// The x axis.
    X,
    /// The y axis.
    Y,
}

impl Axis {
    /// Both axes, x first, in the order of a fit pair.
    const BOTH: [Axis; 2] = [Axis::X, Axis::Y];

    /// The length of `plot` along this axis, in points.
    fn length(self, plot: PlotSize) -> f64 {
        match self {
            Axis::X => plot.width,
            Axis::Y => plot.height,
        }
    }

    /// The index of this axis in a fit pair.
    fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
        }
    }

    /// The axis's name in messages.
    fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
        }
    }

    /// Panel index to the first member of this axis's group.
    fn groups(self, groups: &ShareGroups) -> &[usize] {
        match self {
            Axis::X => &groups.x,
            Axis::Y => &groups.y,
        }
    }

    /// This axis, as resolve names it.
    fn which(self) -> ValueAxis {
        match self {
            Axis::X => ValueAxis::X,
            Axis::Y => ValueAxis::Y,
        }
    }

    /// The continuous scale of this axis in `resolved`, in seconds from its origin on a time
    /// axis; `None` for a category axis.
    fn scale(self, resolved: &Resolved) -> Option<ContinuousScale> {
        let axis = match self {
            Axis::X => &resolved.x,
            Axis::Y => &resolved.y,
        };
        match axis.scale {
            AxisScale::Continuous(scale) => Some(scale),
            AxisScale::Time(scale) => Some(*scale.seconds_scale()),
            AxisScale::Category(_) => None,
        }
    }
}

/// How far a bar's value label reaches past the bar's value on the bar's value axis: the input
/// of the fits for one label, which [`crate::draw::bar_labels`] measures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EndReach {
    /// The value axis.
    pub axis: ValueAxis,
    /// The bar's value.
    pub value: f64,
    /// How far the label reaches towards the axis's low end, in points; 0 for none.
    pub low: f64,
    /// How far the label reaches towards the axis's high end, in points; 0 for none.
    pub high: f64,
}

/// One drawn bubble or bar label, as one axis sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bubble {
    /// The bubble's coordinate on this axis, or the bar's value.
    value: f64,
    /// How far the marker reaches from its centre towards the low end of this axis, outline
    /// included, as the fit first estimates it: with the outline of one color, the widest
    /// (see [`marker_reach`]); or the label past the bar's value, in points.
    low: f64,
    /// See `low`, towards the high end of this axis.
    high: f64,
    /// How far the marker reaches towards the low and the high end as drawn, with its own
    /// outline (see [`marker_outline`]); `low` and `high` for a label.
    drawn: (f64, f64),
    /// The panel, in the order of [`Figure::panels`](crate::Figure::panels), whose plot length
    /// it is measured against.
    panel: usize,
    /// The index of the bubbles mark among the panel's marks; `None` for a bar label.
    mark: Option<usize>,
}

/// Where a fit stands: what it adds to its axis's data extent.
#[derive(Debug, Clone, PartialEq)]
enum State {
    /// Not fitted yet: the axis has the limits of its data positions.
    Unfitted,
    /// Fitted: the axis encloses `extent`.
    Fitted {
        /// The fitted extent, before padding and log-axis rounding.
        extent: (f64, f64),
        /// The least span the bubbles' reaches were measured against, as a share of the raw
        /// extent's span (in log space on a log axis); 0 at first, when only the fitted extent's
        /// own span counts.
        span: f64,
        /// The plot length of every panel the extent was fitted on, in points.
        lengths: Vec<f64>,
        /// How many times the fit was made again after its first.
        refits: usize,
    },
    /// The fit did not settle, or a fit of bar labels alone cannot be made: the axis has the
    /// limits of its data positions, and the plot clips the bubbles that overflow.
    Clipped,
    /// No extent holds the bubbles as drawn on the plots `plots` (one per panel), so the render
    /// fails.
    Unfit {
        /// Why no extent holds them.
        reason: Unfit,
        /// The solved plot of every panel when the fit failed.
        plots: Vec<PlotSize>,
    },
}

/// Why no extent holds the bubbles and bar labels of a fit, with the bubble or label whose
/// bound fails, as an index into the fit's bubbles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unfit {
    /// The furthest reaches towards both ends together take at least the plot length; the
    /// bubble is the one that takes the largest share of its plot.
    TooLarge(Option<usize>),
    /// The extent leaves the supported numeric range; the bubble is the one that takes its end
    /// past the range.
    OutOfRange(Option<usize>),
}

impl Unfit {
    /// The bubble or label whose bound fails, as an index into the fit's bubbles.
    fn bubble(self) -> Option<usize> {
        match self {
            Unfit::TooLarge(bubble) | Unfit::OutOfRange(bubble) => bubble,
        }
    }
}

/// The fit of the bubbles of one automatic axis, shared by every panel of its group.
#[derive(Debug, Clone, PartialEq)]
struct BubbleFit {
    /// The axis.
    axis: Axis,
    /// The raw extent of the axis as a scale: the data and explicit tick positions, before
    /// rounding and padding, with a single value widened as resolve widens it.
    seed: ContinuousScale,
    /// Every drawn bubble and bar label of the group.
    bubbles: Vec<Bubble>,
    /// Whether the fit holds bar labels alone, no bubble: then it widens the axis only once a
    /// label reaches past the plot edges, and keeps a range that already holds every label.
    /// A fit holding a bubble is made on its first update, whatever the range.
    on_demand: bool,
    /// Whether the fit measures the markers as drawn rather than as first estimated (see
    /// [`Bubble::low`]): set once the estimate leaves no extent, so that only the markers as
    /// drawn decide that the bubbles do not fit. The estimate keeps the limits a mapped
    /// outline would otherwise move by a fraction of a point.
    as_drawn: bool,
    /// Where the fit stands.
    state: State,
}

impl BubbleFit {
    /// The fit of `axis` with `limits`, whose group's members are `members`, each its panel
    /// index, resolved marks and bar label reaches, with the bubbles filled as `fills` says
    /// (see [`Fills`]). `None` when the axis has no drawn bubble nor bar label or its raw
    /// extent has no scale (a non-positive value on a log axis); such an axis has the limits of
    /// its data positions.
    fn new<'a, 'm: 'a>(
        axis: Axis,
        limits: &ContinuousLimits<'_>,
        members: impl Iterator<Item = (usize, &'a [ResolvedMark<'m>], &'a [EndReach])> + Clone,
        fills: Fills<'_>,
    ) -> Option<BubbleFit> {
        let mut bubbles = Self::drawn(
            axis,
            None,
            members.clone().map(|(i, marks, _)| (i, marks)),
            fills,
        );
        let on_demand = bubbles.is_empty();
        bubbles.extend(members.clone().flat_map(|(panel, _, ends)| {
            ends.iter()
                .filter(move |end| end.axis == axis.which())
                .map(move |end| Bubble {
                    value: end.value,
                    low: end.low,
                    high: end.high,
                    drawn: (end.low, end.high),
                    panel,
                    mark: None,
                })
        }));
        if bubbles.is_empty() {
            return None;
        }
        let marks = members.flat_map(|(_, marks, _)| marks.iter());
        let data = resolve::axis_extent(marks, axis.which(), limits.transform);
        let raw = resolve::unrounded_extent(limits, data);
        let seed = ContinuousScale::from_extent(limits.transform, raw, CONTEXT).ok()?;
        Some(BubbleFit {
            axis,
            seed,
            bubbles,
            on_demand,
            as_drawn: false,
            state: State::Unfitted,
        })
    }

    /// The fit of the time `axis` with `limits`, whose group's members are `members`, as
    /// [`new`](Self::new) fits a linear axis, in seconds from the earliest time of the data and
    /// the explicit ticks, the origin of the axis ([`resolve::time_extents`]). A single instant
    /// has the half-hour reach on each side that resolve gives it.
    fn time<'a, 'm: 'a>(
        axis: Axis,
        limits: &TimeLimits<'_>,
        members: impl Iterator<Item = (usize, &'a [ResolvedMark<'m>])> + Clone,
        fills: Fills<'_>,
    ) -> Option<BubbleFit> {
        let marks = members.clone().flat_map(|(_, marks)| marks.iter());
        let (_, extent) = resolve::time_extents(marks.clone(), axis.which(), limits.ticks);
        let (origin, _) = extent?;
        let bubbles = Self::drawn(axis, Some(origin), members, fills);
        if bubbles.is_empty() {
            return None;
        }
        // Mesh cells keep their exact extent whatever the bubbles do, so the fit starts from
        // the extent of the other marks and the ticks, in seconds from the common origin.
        let others = marks.filter(|mark| !matches!(mark, ResolvedMark::Mesh { .. }));
        let (_, other) = resolve::time_extents(others, axis.which(), limits.ticks);
        let (first, last) = other.unwrap_or((origin, origin));
        let (low, high) = if first == last {
            let at = seconds_between(origin, first);
            (
                at - resolve::SINGLE_TIME_REACH,
                at + resolve::SINGLE_TIME_REACH,
            )
        } else {
            (
                seconds_between(origin, first),
                seconds_between(origin, last),
            )
        };
        let seed = ContinuousScale::new(Transform::Linear, low, high, CONTEXT).ok()?;
        Some(BubbleFit {
            axis,
            seed,
            bubbles,
            on_demand: false,
            as_drawn: false,
            state: State::Unfitted,
        })
    }

    /// Every drawn bubble of the group's `members`, as `axis` sees it, with the times of a time
    /// axis in seconds from `origin`, filled as `fills` says.
    fn drawn<'a, 'm: 'a>(
        axis: Axis,
        origin: Option<Instant>,
        members: impl Iterator<Item = (usize, &'a [ResolvedMark<'m>])>,
        fills: Fills<'_>,
    ) -> Vec<Bubble> {
        members
            .flat_map(|(panel, marks)| {
                let fill = (fills.theme, fills.scales[panel]);
                marks.iter().enumerate().flat_map(move |(index, mark)| {
                    bubbles(mark, axis, (panel, index), origin, fill)
                })
            })
            .collect()
    }

    /// How far `bubble` reaches towards the low and the high end, as this fit measures it: as
    /// drawn, or as first estimated (see [`BubbleFit::as_drawn`]).
    fn reach(&self, bubble: &Bubble) -> (f64, f64) {
        if self.as_drawn {
            bubble.drawn
        } else {
            (bubble.low, bubble.high)
        }
    }

    /// The number of panels whose plot lengths the fit's bubbles are measured against: one
    /// past the last of their panels.
    fn panels(&self) -> usize {
        self.bubbles
            .iter()
            .map(|bubble| bubble.panel + 1)
            .max()
            .unwrap_or(1)
    }

    /// The extent this fit adds to its axis's data extent; `None` while it adds nothing.
    fn extent(&self) -> Option<(f64, f64)> {
        match self.state {
            State::Fitted { extent, .. } => Some(extent),
            State::Unfitted | State::Clipped | State::Unfit { .. } => None,
        }
    }

    /// The plot length, in points, on which the furthest reaches towards both ends together
    /// take `1 - DATA_SHARE` of it: long enough for an extent to hold every bubble and label,
    /// with [`DATA_SHARE`] of it left to the data positions.
    fn roomy_length(&self) -> f64 {
        let furthest = |end: fn((f64, f64)) -> f64| {
            self.bubbles
                .iter()
                .map(|bubble| end(self.reach(bubble)))
                .filter(|reach| *reach > 0.0)
                .map(|reach| reach + PATH_ALLOWANCE)
                .fold(0.0, f64::max)
        };
        (furthest(|(low, _)| low) + furthest(|(_, high)| high)) / (1.0 - DATA_SHARE)
    }

    /// Whether every panel's plot at least `length` pt long along the axis holds the bubbles,
    /// as the final layout fits them while the plots keep their lengths: the fit is made, the
    /// axis takes its extent rounded to powers on a log axis and padded on both sides, as
    /// resolve pads the bubbles' axes ([`ContinuousScale::nice`], [`ContinuousScale::padded`]),
    /// and while a bubble then reaches past the plot edge the fit is made again against the
    /// span of that domain, at most [`MAX_REFITS`] times. False as soon as an extent leaves the
    /// supported range.
    fn holds(&self, length: f64) -> bool {
        let lengths = vec![length; self.panels()];
        let mut span = 0.0;
        for _ in 0..=MAX_REFITS {
            let Ok((lo, hi)) = self.solve(span, &lengths) else {
                return false;
            };
            let Ok(extent) = ContinuousScale::new(self.seed.transform(), lo, hi, CONTEXT) else {
                return false;
            };
            let rounded = match self.seed.transform() {
                Transform::Linear => extent,
                Transform::Log(_) => extent.nice(0),
            };
            let domain = rounded.padded(Padding::BothSides);
            if self.contains(domain, &lengths) {
                return true;
            }
            let (lo, hi) = domain.domain();
            span = f64::max(span, self.seed.normalize(hi) - self.seed.normalize(lo));
        }
        false
    }

    /// A plot length, in points, on which the bubbles fit with room to spare: the
    /// [`roomy_length`](Self::roomy_length), or longer when the extent must stay within the
    /// supported range ([`holds`](Self::holds)), the shortest such length to the bisection's
    /// precision. `None` when no plot up to [`LONGEST_PLOT`] holds them: their extent leaves
    /// the supported range on every plot a figure can have.
    fn sufficient_length(&self) -> Option<f64> {
        let roomy = self.roomy_length();
        if self.holds(roomy) {
            return Some(roomy);
        }
        if !self.holds(LONGEST_PLOT) {
            return None;
        }
        // A longer plot gives every reach a smaller share, and so a narrower extent: holding is
        // taken as monotonic in the length.
        let (mut short, mut long) = (roomy, LONGEST_PLOT);
        for _ in 0..BISECTION_STEPS {
            let middle = short + (long - short) / 2.0;
            if middle <= short || middle >= long {
                break;
            }
            if self.holds(middle) {
                long = middle;
            } else {
                short = middle;
            }
        }
        Some(long)
    }

    /// The label of the bubbles mark of the bubble at `bubble` in an error, a space first; empty
    /// for a mark without one. For a bar label, or no bubble, the furthest reaching bubble of
    /// `panel` is named, or of any panel without one there; `marks` are the resolved marks of
    /// every panel.
    fn label(&self, bubble: Option<usize>, panel: usize, marks: &[&[ResolvedMark<'_>]]) -> String {
        let named = bubble
            .map(|i| &self.bubbles[i])
            .filter(|bubble| bubble.mark.is_some())
            .or_else(|| {
                self.bubbles
                    .iter()
                    .filter(|bubble| bubble.mark.is_some())
                    .max_by(|a, b| {
                        let (a_low, a_high) = self.reach(a);
                        let (b_low, b_high) = self.reach(b);
                        (a.panel == panel)
                            .cmp(&(b.panel == panel))
                            .then(a_low.max(a_high).total_cmp(&b_low.max(b_high)))
                    })
            });
        let label = named
            .and_then(|bubble| marks[bubble.panel].get(bubble.mark?))
            .and_then(|mark| match mark {
                ResolvedMark::Bubbles { mark, .. } => Some(mark.options().label.as_str()),
                _ => None,
            })
            .filter(|label| !label.is_empty());
        label.map_or(String::new(), |label| format!(" {}", quoted(label)))
    }

    /// Whether every bubble reaches no further than the plot edges of `scale`, on plots of
    /// `lengths` (one per panel).
    fn contains(&self, scale: ContinuousScale, lengths: &[f64]) -> bool {
        self.bubbles.iter().all(|bubble| {
            let (low, high) = self.reach(bubble);
            let position = scale.normalize(bubble.value);
            let length = lengths[bubble.panel];
            position >= low / length && position <= 1.0 - high / length
        })
    }

    /// Fits again for the axis resolved as `scale` on plots of `lengths`, when needed: the
    /// first time (for a fit of bar labels alone, only once a label reaches past the plot
    /// edges), and whenever a bubble reaches past the plot edges. Returns whether the extent
    /// this fit adds changed.
    fn update(&mut self, scale: ContinuousScale, plots: &[PlotSize]) -> bool {
        let lengths: Vec<f64> = plots.iter().map(|plot| self.axis.length(*plot)).collect();
        let lengths = lengths.as_slice();
        // The span, plot lengths and refit count of the fit to make; `None` to clip the axis.
        let next = match &self.state {
            State::Clipped | State::Unfit { .. } => return false,
            State::Fitted { .. } if self.contains(scale, lengths) => return false,
            State::Unfitted if self.on_demand && self.contains(scale, lengths) => return false,
            State::Unfitted => Some((0.0, lengths.to_vec(), 0)),
            State::Fitted { refits, .. } if *refits == MAX_REFITS => None,
            State::Fitted {
                span,
                lengths: fitted,
                refits,
                ..
            } => {
                let shorter = self
                    .bubbles
                    .iter()
                    .any(|bubble| lengths[bubble.panel] < fitted[bubble.panel]);
                if shorter {
                    let lengths = fitted.iter().zip(lengths).map(|(a, b)| a.min(*b)).collect();
                    Some((*span, lengths, refits + 1))
                } else {
                    // The plots did not shrink, so padding or log-axis rounding moved a bubble out:
                    // measure the reaches against at least the span of the final domain.
                    let (lo, hi) = scale.domain();
                    let rounded = self.seed.normalize(hi) - self.seed.normalize(lo);
                    Some((span.max(rounded), fitted.clone(), refits + 1))
                }
            }
        };
        let next = match next {
            Some((span, lengths, refits)) => self.fitted(span, lengths, refits, plots),
            None => State::Clipped,
        };
        let before = self.extent();
        self.state = next;
        self.extent() != before
    }

    /// Clips this fit when its bubbles or labels reach past the plot edges of `scale` on
    /// `plots`. Returns whether the extent it adds changed, or, for a fit not made yet, whether
    /// its range stopped holding its labels.
    fn clip_if_unsettled(&mut self, scale: ContinuousScale, plots: &[PlotSize]) -> bool {
        let lengths: Vec<f64> = plots.iter().map(|plot| self.axis.length(*plot)).collect();
        if matches!(self.state, State::Clipped | State::Unfit { .. })
            || self.contains(scale, &lengths)
        {
            return false;
        }
        let changed = self.extent().is_some() || self.state == State::Unfitted;
        self.state = State::Clipped;
        changed
    }

    /// The state of a fit measuring the reaches against at least `span`, on plots of `lengths`
    /// along its axis, of the solved `plots`: fitted when [`solve`](Self::solve) finds an
    /// extent; otherwise unfit, or clipped for a fit of bar labels alone, whose labels report
    /// that they do not fit when they are drawn. When the reaches as first estimated leave no
    /// extent, the markers as drawn are measured from then on.
    fn fitted(&mut self, span: f64, lengths: Vec<f64>, refits: usize, plots: &[PlotSize]) -> State {
        let mut solved = self.solve(span, &lengths);
        if solved.is_err() && !self.as_drawn {
            self.as_drawn = true;
            solved = self.solve(span, &lengths);
        }
        match solved {
            Ok(extent) => State::Fitted {
                extent,
                span,
                lengths,
                refits,
            },
            Err(_) if self.on_demand => State::Clipped,
            Err(reason) => State::Unfit {
                reason,
                plots: plots.to_vec(),
            },
        }
    }

    /// The smallest extent holding the raw extent and every bubble, on plots of `lengths`, with
    /// each reach measured against the larger of the extent's own span and `span`. Fails when
    /// the furthest reach towards the low end and the furthest towards the high end together
    /// take at least the plot length (for bubbles: a bubble reaches at least half its plot
    /// length), so no extent holds them, or when the extent leaves the supported numeric range.
    ///
    /// In the raw extent's normalized axis space (log space on a log axis), a bubble at `v`
    /// whose reaches are the shares `a` and `b` of its plot length needs `lo <= v - a d` and
    /// `hi >= v + b d`, where `d` is the larger of `hi - lo` and `span`; a reach of 0 adds no
    /// bound. Against the extent's own span,
    /// `d = max(v + a d) + max(-v + a d)`: both maxima are upper envelopes of lines in `d`, and
    /// sweeping their breakpoints finds the active pair in O(n log n), the raw extent's ends
    /// being the lines of slope 0. When that span is less than `span`, the bounds against `span`
    /// are direct, and their extent is no wider than `span`, since every pair of bounds is
    /// at most as wide against the smaller span.
    fn solve(&self, span: f64, lengths: &[f64]) -> Result<(f64, f64), Unfit> {
        let scale = self.seed;
        let mut upper = vec![Line {
            slope: 0.0,
            intercept: 1.0,
            bubble: None,
        }];
        let mut lower = vec![Line {
            slope: 0.0,
            intercept: 0.0,
            bubble: None,
        }];
        let slope = |reach: f64, panel: usize| (reach + PATH_ALLOWANCE) / lengths[panel];
        for (i, bubble) in self.bubbles.iter().enumerate() {
            let value = scale.normalize(bubble.value);
            let (low, high) = self.reach(bubble);
            if high > 0.0 {
                upper.push(Line {
                    slope: slope(high, bubble.panel),
                    intercept: value,
                    bubble: Some(i),
                });
            }
            if low > 0.0 {
                lower.push(Line {
                    slope: slope(low, bubble.panel),
                    intercept: -value,
                    bubble: Some(i),
                });
            }
        }
        let steepest = |lines: &[Line]| lines.iter().map(|line| line.slope).fold(0.0, f64::max);
        // A plot of no length gives a NaN slope, which `f64::max` would skip.
        let any_nan = upper.iter().chain(&lower).any(|line| line.slope.is_nan());
        if any_nan || steepest(&upper) + steepest(&lower) >= 1.0 {
            // The steepest line, a NaN slope first.
            let binding = upper
                .iter()
                .chain(&lower)
                .max_by(|a, b| {
                    a.slope
                        .is_nan()
                        .cmp(&b.slope.is_nan())
                        .then(a.slope.total_cmp(&b.slope))
                })
                .and_then(|line| line.bubble);
            return Err(Unfit::TooLarge(binding));
        }
        let at = |lines: &[Line], d: f64| {
            lines
                .iter()
                .map(|line| line.intercept + line.slope * d)
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let d = own_span(&upper, &lower).max(span);
        let (lo, hi) = (-at(&lower, d), at(&upper, d));
        let (low, high) = scale.domain();
        let value = |p: f64| match scale.transform() {
            Transform::Linear => low + p * (high - low),
            Transform::Log(_) => {
                let offset = p * log_offset(high, low);
                let scaled = low * offset.exp();
                if scaled.is_normal() {
                    scaled
                } else {
                    (low.ln() + offset).exp()
                }
            }
        };
        let (lo, hi) = (value(lo).min(low), value(hi).max(high));
        ContinuousScale::new(scale.transform(), lo, hi, CONTEXT)
            .map(|scale| scale.domain())
            .map_err(|_| {
                // The end past the range, and the line that takes it there.
                let low_end = ContinuousScale::new(scale.transform(), lo, high, CONTEXT).is_err();
                let lines = if low_end { &lower } else { &upper };
                let reach = |line: &Line| line.intercept + line.slope * d;
                let binding = lines
                    .iter()
                    .max_by(|a, b| reach(a).total_cmp(&reach(b)))
                    .and_then(|line| line.bubble);
                Unfit::OutOfRange(binding)
            })
    }
}

/// How the bubbles of the panels are filled, which decides which of them are drawn: the
/// theme, and each panel's color scale, in the order of the panels.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fills<'a> {
    /// The theme.
    pub theme: &'a Theme,
    /// The color scale of each panel; `None` for a panel without one.
    pub scales: &'a [Option<&'a ResolvedColorScale>],
}

/// The drawn bubbles of `mark`, the mark at `index` of the panel `panel`, as `axis` of that
/// panel sees them: none for any other mark. A bubble is drawn when both its coordinates are
/// finite (or present, for times), its radius is positive and it has a fill under the theme
/// and color scale `fill`, as drawing decides ([`has_fill`]). The times of a time axis are
/// seconds from `origin`.
fn bubbles<'a>(
    mark: &'a ResolvedMark<'_>,
    axis: Axis,
    (panel, index): (usize, usize),
    origin: Option<Instant>,
    (theme, scale): (&'a Theme, Option<&'a ResolvedColorScale>),
) -> impl Iterator<Item = Bubble> + 'a {
    let found = match mark {
        ResolvedMark::Bubbles {
            mark,
            maximum,
            radius,
            color,
            mapped,
            ..
        } => Some((*mark, *maximum, *radius, *color, mapped.as_ref())),
        _ => None,
    };
    found
        .into_iter()
        .flat_map(move |(data, maximum, radius, color, mapped)| {
            let shape = data.options().marker;
            let outline = marker_outline(mapped.is_some());
            data.sizes()
                .iter()
                .enumerate()
                .filter_map(move |(i, &size)| {
                    // A NaN size has no radius, so it is skipped with the zero sizes below.
                    let r = crate::bubble_scale::radius(size, maximum, radius);
                    if !(is_drawn(data.x_values(), i) && is_drawn(data.y_values(), i))
                        || r <= 0.0
                        || !has_fill(theme, scale, color, mapped, i)
                    {
                        return None;
                    }
                    let estimate = marker_reach(shape, 2.0 * r, MARKER_OUTLINE);
                    let drawn = marker_reach(shape, 2.0 * r, outline);
                    let (value, reach, drawn) = match axis {
                        Axis::X => (coordinate(data.x_values(), i, origin), estimate.0, drawn.0),
                        Axis::Y => (coordinate(data.y_values(), i, origin), estimate.1, drawn.1),
                    };
                    Some(Bubble {
                        value,
                        low: reach,
                        high: reach,
                        drawn: (drawn, drawn),
                        panel,
                        mark: Some(index),
                    })
                })
        })
}

/// Whether the coordinate at `i` places a bubble: a finite number, or a present time.
fn is_drawn(values: &SharedValues, i: usize) -> bool {
    match values {
        SharedValues::Numbers(values) => values.get(i).is_some_and(|v| v.is_finite()),
        SharedValues::Times(times) => times.get(i).is_some_and(Option::is_some),
    }
}

/// The coordinate at `i` in the space of its axis: a number as it is, a time in seconds from
/// `origin`; NaN for a missing time, or for a time without an origin, which a numeric axis
/// never has.
fn coordinate(values: &SharedValues, i: usize, origin: Option<Instant>) -> f64 {
    match values {
        SharedValues::Numbers(values) => values.get(i).copied().unwrap_or(f64::NAN),
        SharedValues::Times(times) => match (times.get(i).copied().flatten(), origin) {
            (Some(t), Some(origin)) => seconds_between(origin, t),
            _ => f64::NAN,
        },
    }
}

/// The bubble fits of every automatic axis of a figure: one pair per panel, x then y, filled
/// for the first member of each group whose axis has no explicit limits and holds a drawn
/// bubble.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BubbleFits {
    /// The fits, indexed by panel and axis.
    fits: Vec<[Option<BubbleFit>; 2]>,
    /// The first member of each panel's x and y groups, whose fits are the panel's.
    leaders: Vec<[usize; 2]>,
}

impl BubbleFits {
    /// The fits of the panels whose resolved marks are `marks` and whose bar labels reach as
    /// `ends` says, with the axis limits `limits`, the share groups `groups` and the bubbles
    /// filled as `fills` says. No fit is made yet: every axis has the limits of its data
    /// positions until the first [`update`](Self::update).
    pub fn new(
        marks: &[&[ResolvedMark<'_>]],
        ends: &[Vec<EndReach>],
        limits: &[Limits<'_>],
        groups: &ShareGroups,
        fills: Fills<'_>,
    ) -> Self {
        let fits = (0..marks.len())
            .map(|i| {
                Axis::BOTH.map(|axis| {
                    let keys = axis.groups(groups);
                    if keys[i] != i {
                        return None;
                    }
                    let members = (0..marks.len())
                        .filter(|j| keys[*j] == i)
                        .map(|j| (j, marks[j], ends[j].as_slice()));
                    match limits[i].axis(axis.which()) {
                        AxisLimits::Continuous(limits) if limits.explicit.is_none() => {
                            BubbleFit::new(axis, &limits, members, fills)
                        }
                        AxisLimits::Time(limits) if limits.explicit.is_none() => BubbleFit::time(
                            axis,
                            &limits,
                            members.map(|(j, marks, _)| (j, marks)),
                            fills,
                        ),
                        _ => None,
                    }
                })
            })
            .collect();
        let leaders = (0..marks.len())
            .map(|i| Axis::BOTH.map(|axis| axis.groups(groups)[i]))
            .collect();
        BubbleFits { fits, leaders }
    }

    /// The extent the fit of the x group led by `panel` adds to its data extent, if any.
    pub fn x_extent(&self, panel: usize) -> Option<(f64, f64)> {
        self.fits[panel][0].as_ref().and_then(BubbleFit::extent)
    }

    /// The extent the fit of the y group led by `panel` adds to its data extent, if any.
    pub fn y_extent(&self, panel: usize) -> Option<(f64, f64)> {
        self.fits[panel][1].as_ref().and_then(BubbleFit::extent)
    }

    /// How many times [`update`](Self::update) can change an extent in all: each fit is made
    /// once, again at most [`MAX_REFITS`] times, and clipped or found unfit at most once.
    pub fn budget(&self) -> usize {
        self.fits.iter().flatten().flatten().count() * (MAX_REFITS + 2)
    }

    /// Fits again every axis whose bubbles need it on the axes `resolved` and the solved plots
    /// `plots` (see [`BubbleFit::update`]). Returns whether any extent changed, in which case
    /// the axes must be resolved and the layout solved again, unless a fit failed
    /// ([`failure`](Self::failure)).
    pub fn update(&mut self, resolved: &[Resolved], plots: &[PlotSize]) -> bool {
        self.each(resolved, plots, BubbleFit::update)
    }

    /// The first fit, in panel order and x before y, that no extent can make, as the failure
    /// that names it; `None` while every fit can be made. `marks` are the resolved marks of
    /// every panel, for the label of the bubbles named.
    ///
    /// The failure names the bubbles mark (its label, if any) and the panel of the bubble whose
    /// bound fails, and estimates the plot lengths that hold the bubbles of both axes of that
    /// panel ([`BubbleFit::sufficient_length`]).
    pub fn failure(&self, marks: &[&[ResolvedMark<'_>]]) -> Option<BubbleFailure> {
        let (fit, reason, plots) =
            self.fits
                .iter()
                .flatten()
                .flatten()
                .find_map(|fit| match &fit.state {
                    State::Unfit { reason, plots } => Some((fit, *reason, plots)),
                    _ => None,
                })?;
        let panel = reason.bubble().map_or_else(
            || {
                let length = |bubble: &&Bubble| fit.axis.length(plots[bubble.panel]);
                fit.bubbles
                    .iter()
                    .min_by(|a, b| length(a).total_cmp(&length(b)))
                    .map_or(0, |bubble| bubble.panel)
            },
            |i| fit.bubbles[i].panel,
        );
        let estimate = Axis::BOTH.map(|side| {
            self.fits[self.leaders[panel][side.index()]][side.index()]
                .as_ref()
                .and_then(BubbleFit::sufficient_length)
        });
        Some(BubbleFailure {
            panel,
            label: fit.label(reason.bubble(), panel, marks),
            axis: fit.axis,
            plot: plots[panel],
            estimate,
        })
    }

    /// Whether every fit holding a bubble settled: none fell back to the limits of the data
    /// positions, so the plots hold every drawn bubble.
    pub fn settled(&self) -> bool {
        self.fits
            .iter()
            .flatten()
            .flatten()
            .all(|fit| fit.on_demand || fit.state != State::Clipped)
    }

    /// Clips every fit whose bubbles still reach past the plot edges of the axes `resolved` on
    /// the solved plots `plots`: the fits that did not settle. Returns whether any extent
    /// changed, or a fit of bar labels not made yet, whose range held its labels until now, no
    /// longer holds them.
    pub fn clip_unsettled(&mut self, resolved: &[Resolved], plots: &[PlotSize]) -> bool {
        self.each(resolved, plots, BubbleFit::clip_if_unsettled)
    }

    /// Applies `step` to every fit with its axis's scale in `resolved` and the solved plots
    /// `plots`. Returns whether any step returned true.
    fn each(
        &mut self,
        resolved: &[Resolved],
        plots: &[PlotSize],
        mut step: impl FnMut(&mut BubbleFit, ContinuousScale, &[PlotSize]) -> bool,
    ) -> bool {
        let mut changed = false;
        for (i, pair) in self.fits.iter_mut().enumerate() {
            for fit in pair.iter_mut().flatten() {
                let Some(scale) = fit.axis.scale(&resolved[i]) else {
                    continue;
                };
                changed |= step(fit, scale, plots);
            }
        }
        changed
    }
}

/// Bubbles that no automatic limits hold on the plots of a layout, as the first failing fit
/// names them ([`BubbleFits::failure`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BubbleFailure {
    /// The panel of the bubble whose bound fails, in the order of the panels.
    pub panel: usize,
    /// The label of the bubbles mark in an error, a space first; empty without one.
    label: String,
    /// The axis of the failing fit.
    axis: Axis,
    /// The plot of the panel when the fit failed.
    pub plot: PlotSize,
    /// The plot lengths, x then y, that the fits of the panel's axes estimate to hold their
    /// bubbles ([`BubbleFit::sufficient_length`]); `None` for an axis without a fit, or whose
    /// fit finds no such length.
    pub estimate: [Option<f64>; 2],
}

impl BubbleFailure {
    /// The figure side that grows the failing axis's plots: 0 for the width (x), 1 for the
    /// height (y).
    pub fn side(&self) -> usize {
        self.axis.index()
    }

    /// The error for bubbles that a figure of `figure` (width and height, in points) cannot
    /// hold and one of `needed` can, reported in `context`.
    pub fn needs_figure(&self, context: &'static str, needed: [f64; 2], figure: [f64; 2]) -> Error {
        let [w, h] = needed.map(number);
        let [fw, fh] = figure.map(number);
        Error::style(
            context,
            format!(
                "the bubbles{} need a figure at least {w} × {h} pt; the figure is {fw} × {fh} pt.",
                self.label
            ),
        )
        .with_hint(HINT)
    }

    /// The error for bubbles that no figure holds within the supported numeric range, reported
    /// in `context`.
    pub fn out_of_range(&self, context: &'static str) -> Error {
        Error::style(
            context,
            format!(
                "the bubbles{} need {} limits beyond the supported range.",
                self.label,
                self.axis.name()
            ),
        )
        .with_hint(RANGE_HINT)
    }
}

/// A line of the upper or reflected lower envelope, in normalized axis space.
#[derive(Debug, Clone, Copy)]
struct Line {
    slope: f64,
    intercept: f64,
    /// The bubble or label of the line, as an index into the fit's bubbles; `None` for an end
    /// of the raw extent.
    bubble: Option<usize>,
}

/// The span `d` that solves `d = max(upper(d)) + max(lower(d))`, where every slope is below 1/2:
/// the span of the smallest extent whose bubbles reach no further than its own edges.
fn own_span(upper: &[Line], lower: &[Line]) -> f64 {
    let upper = envelope(upper.to_vec());
    let lower = envelope(lower.to_vec());
    let (mut i, mut j) = (0, 0);
    loop {
        let (a, b) = (upper[i].0, lower[j].0);
        let span = (a.intercept + b.intercept) / (1.0 - a.slope - b.slope);
        let next_upper = upper.get(i + 1).map_or(f64::INFINITY, |line| line.1);
        let next_lower = lower.get(j + 1).map_or(f64::INFINITY, |line| line.1);
        if span <= next_upper.min(next_lower) {
            return span;
        }
        if next_upper <= next_lower {
            i += 1;
        }
        if next_lower <= next_upper {
            j += 1;
        }
    }
}

/// Upper envelope in increasing slope order, paired with each line's first active span.
/// Equal slopes keep only the largest intercept; no division by zero or quadratic search.
fn envelope(mut lines: Vec<Line>) -> Vec<(Line, f64)> {
    lines.sort_by(|a, b| {
        a.slope
            .total_cmp(&b.slope)
            .then(a.intercept.total_cmp(&b.intercept))
    });
    let mut hull: Vec<(Line, f64)> = Vec::new();
    for line in lines {
        let start = loop {
            let Some(&(last, start)) = hull.last() else {
                break f64::NEG_INFINITY;
            };
            if line.slope == last.slope {
                hull.pop();
                continue;
            }
            let crossing = (last.intercept - line.intercept) / (line.slope - last.slope);
            if crossing <= start {
                hull.pop();
            } else {
                break crossing;
            }
        };
        hull.push((line, start));
    }
    hull
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fit(transform: Transform, (lo, hi): (f64, f64), bubbles: Vec<Bubble>) -> BubbleFit {
        BubbleFit {
            axis: Axis::X,
            seed: ContinuousScale::new(transform, lo, hi, "test").unwrap(),
            bubbles,
            on_demand: false,
            as_drawn: false,
            state: State::Unfitted,
        }
    }

    fn bubble(value: f64, reach: f64) -> Bubble {
        Bubble {
            value,
            low: reach,
            high: reach,
            drawn: (reach, reach),
            panel: 0,
            mark: Some(0),
        }
    }

    /// A bar label at `value` reaching `low` and `high` past it.
    fn end_label(value: f64, low: f64, high: f64) -> Bubble {
        Bubble {
            value,
            low,
            high,
            drawn: (low, high),
            panel: 0,
            mark: None,
        }
    }

    /// One plot `width` pt wide, as the x axis fits measure it.
    fn plots(width: f64) -> [PlotSize; 1] {
        [PlotSize {
            width,
            height: 100.0,
        }]
    }

    fn linear(lo: f64, hi: f64) -> ContinuousScale {
        ContinuousScale::new(Transform::Linear, lo, hi, "test").unwrap()
    }

    #[test]
    fn bubbles_reaching_half_the_plot_or_past_the_numeric_range_cannot_fit() {
        // 18.4 pt either side of the centre on a 36.8 pt plot: no extent holds it, whatever
        // span its reach is measured against.
        let wide = fit(Transform::Linear, (0.0, 1.0), vec![bubble(0.5, 18.4)]);
        assert_eq!(wide.solve(0.0, &[36.8]), Err(Unfit::TooLarge(Some(0))));
        assert_eq!(wide.solve(10.0, &[36.8]), Err(Unfit::TooLarge(Some(0))));
        assert!(wide.solve(0.0, &[36.9]).is_ok());
        assert_eq!(wide.solve(0.0, &[0.0]), Err(Unfit::TooLarge(Some(0))));
        for (transform, low, high, value) in [
            (
                Transform::Linear,
                0.0,
                crate::MAX_MAGNITUDE,
                crate::MAX_MAGNITUDE,
            ),
            (
                Transform::Log(crate::LogBase::TEN),
                crate::MIN_LOG_VALUE,
                1.0,
                crate::MIN_LOG_VALUE,
            ),
        ] {
            let edge = fit(transform, (low, high), vec![bubble(value, 18.4)]);
            assert_eq!(edge.solve(0.0, &[200.0]), Err(Unfit::OutOfRange(Some(0))));
        }
    }

    #[test]
    fn a_fit_that_cannot_be_made_is_unfit_unless_it_holds_bar_labels_alone() {
        let mut wide = fit(Transform::Linear, (0.0, 1.0), vec![bubble(0.5, 18.4)]);
        let scale = wide.seed;
        assert!(
            !wide.update(scale, &plots(36.8)),
            "nothing is added to the data extent"
        );
        let unfit = State::Unfit {
            reason: Unfit::TooLarge(Some(0)),
            plots: plots(36.8).to_vec(),
        };
        assert_eq!(wide.state, unfit);
        assert_eq!(wide.extent(), None);
        // An unfit fit stays unfit, whatever the plot becomes.
        assert!(!wide.update(scale, &plots(400.0)));
        assert_eq!(wide.state, unfit);
        // The plot named in its error leaves half of it to the data positions.
        assert!((wide.roomy_length() - 2.0 * (36.8 + 2.0 * PATH_ALLOWANCE)).abs() < 1e-9);
        // Bar labels alone fall back to the data limits, and report when they are drawn.
        let mut labels = BubbleFit {
            on_demand: true,
            ..fit(
                Transform::Linear,
                (0.0, 1.0),
                vec![Bubble {
                    mark: None,
                    ..bubble(1.0, 40.0)
                }],
            )
        };
        assert!(!labels.update(scale, &plots(36.8)));
        assert_eq!(labels.state, State::Clipped);
    }

    #[test]
    fn only_the_markers_as_drawn_decide_that_bubbles_do_not_fit() {
        // A mapped circle's outline is thinner than the estimate's: 36.5 pt as drawn fit a
        // 36.6 pt plot, though the estimate, 36.8 pt, does not.
        let mapped = Bubble {
            drawn: (18.25, 18.25),
            ..bubble(0.0, 18.4)
        };
        let mut f = fit(Transform::Linear, (-1.0, 1.0), vec![mapped]);
        assert!(f.update(f.seed, &plots(36.6)));
        assert!(f.as_drawn);
        let (lo, hi) = f.extent().unwrap();
        assert!(f.contains(linear(lo, hi), &[36.6]));
        // The estimate keeps fitting the limits while it leaves an extent.
        let mut roomy = fit(Transform::Linear, (-1.0, 1.0), vec![mapped]);
        assert!(roomy.update(roomy.seed, &plots(100.0)));
        assert!(!roomy.as_drawn);
    }

    #[test]
    fn the_sufficient_length_keeps_the_extent_within_the_supported_range() {
        let log = Transform::Log(crate::LogBase::TEN);
        // Bubbles 400 decades apart need a plot long enough to keep their limits, padded,
        // above the smallest log value.
        let far = fit(
            log,
            (1e-200, 1e200),
            vec![bubble(1e-200, 18.4), bubble(1e200, 18.4)],
        );
        let length = far.sufficient_length().unwrap();
        assert!(length > far.roomy_length(), "{length}");
        assert!(far.holds(length) && !far.holds(length - 1e-6));
        // Close bubbles need the roomy length alone.
        let near = fit(
            log,
            (1.0, 100.0),
            vec![bubble(1.0, 18.4), bubble(100.0, 18.4)],
        );
        assert_eq!(near.sufficient_length(), Some(near.roomy_length()));
        // A bubble at the end of the range reaches past it on every plot.
        let edge = fit(
            Transform::Linear,
            (0.0, crate::MAX_MAGNITUDE),
            vec![bubble(0.0, 1.0), bubble(crate::MAX_MAGNITUDE, 1.0)],
        );
        assert_eq!(edge.sufficient_length(), None);
    }

    #[test]
    fn direct_fit_matches_all_pair_constraints() {
        let bubbles: Vec<_> = (0..101)
            .map(|i| bubble(f64::from(i) / 100.0, f64::from((i * 37) % 49)))
            .collect();
        let fit = fit(Transform::Linear, (0.0, 1.0), bubbles);
        let (lo, hi) = fit.solve(0.0, &[100.0]).unwrap();
        let mut points = vec![(0.0, 0.0), (1.0, 0.0)];
        points.extend(
            fit.bubbles
                .iter()
                .map(|b| (b.value, (b.high + PATH_ALLOWANCE) / 100.0)),
        );
        let mut expected: f64 = 1.0;
        for &(a, ra) in &points {
            for &(b, rb) in &points {
                expected = expected.max((b - a) / (1.0 - ra - rb));
            }
        }
        assert!((hi - lo - expected).abs() < 1e-12);
        assert!(fit.contains(linear(lo, hi), &[100.0]));
        // A smaller least span changes nothing; a larger one bounds the reaches directly.
        assert_eq!(fit.solve(expected / 2.0, &[100.0]), Ok((lo, hi)));
        let (wide_lo, wide_hi) = fit.solve(2.0 * expected, &[100.0]).unwrap();
        assert!(wide_lo < lo && wide_hi > hi && wide_hi - wide_lo <= 2.0 * expected);
    }

    /// The share of a `length` pt plot that a reach of `reach` pt takes, with the allowance.
    fn share(reach: f64, length: f64) -> f64 {
        (reach + PATH_ALLOWANCE) / length
    }

    /// The tight extent of bubbles at 0 and 10 reaching 10 pt and 20 pt over the raw extent 0
    /// to 10, on a plot `length` pt long: `lo = 0 - a d` and `hi = 10 + b d` for the shares `a`
    /// and `b`, with `d = hi - lo`, so `d = 10 / (1 - a - b)`.
    fn tight(length: f64) -> (f64, f64) {
        let (a, b) = (share(10.0, length), share(20.0, length));
        let d = 10.0 / (1.0 - a - b);
        (-a * d, 10.0 + b * d)
    }

    fn assert_close(a: (f64, f64), b: (f64, f64)) {
        assert!(
            (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9,
            "{a:?} != {b:?}"
        );
    }

    fn fitted(f: &BubbleFit) -> (f64, &[f64], usize) {
        let State::Fitted {
            span,
            ref lengths,
            refits,
            ..
        } = f.state
        else {
            panic!("fitted: {:?}", f.state);
        };
        (span, lengths, refits)
    }

    #[test]
    fn refits_start_from_the_raw_extent_never_from_the_padded_domain() {
        let mut f = fit(
            Transform::Linear,
            (0.0, 10.0),
            vec![bubble(0.0, 10.0), bubble(10.0, 20.0)],
        );
        assert!(f.update(f.seed, &plots(100.0)));
        assert_close(f.extent().unwrap(), tight(100.0));
        // The plot shrinks: the fit is made again from the raw extent on the shorter plot.
        let (lo, hi) = f.extent().unwrap();
        assert!(!f.contains(linear(lo, hi), &[80.0]));
        assert!(f.update(linear(lo, hi), &plots(80.0)));
        assert_eq!(fitted(&f), (0.0, &[80.0][..], 1));
        assert_close(f.extent().unwrap(), tight(80.0));
        // A plot that grows again keeps the fit, which still holds the bubbles.
        let (lo, hi) = f.extent().unwrap();
        assert!(!f.update(linear(lo, hi), &plots(120.0)));
        // Rounding widens the domain on the right only, so the bubble at 0 reaches past the
        // left edge. The fit is made again from the raw extent, with the reaches measured
        // against the rounded span, for resolve to round and pad once: the padded domain is
        // never fitted again.
        let rounded = (lo, hi + 10.0);
        assert!(!f.contains(linear(rounded.0, rounded.1), &[80.0]));
        assert!(f.update(linear(rounded.0, rounded.1), &plots(80.0)));
        let d = rounded.1 - rounded.0;
        let (span, lengths, refits) = fitted(&f);
        assert!((span - d / 10.0).abs() < 1e-12, "{span}");
        assert_eq!((lengths, refits), (&[80.0][..], 2));
        let expected = (-share(10.0, 80.0) * d, 10.0 + share(20.0, 80.0) * d);
        assert_close(f.extent().unwrap(), expected);
        assert!(expected.1 - expected.0 < d);
    }

    #[test]
    fn a_one_sided_reach_widens_one_end_only() {
        // A label 20 pt past the end of a bar at 10, over the raw extent 0 to 10: only the high
        // end moves, to `10 + b d` with `d = hi`, so `hi = 10 / (1 - b)`.
        let label = end_label(10.0, 0.0, 20.0);
        let f = fit(Transform::Linear, (0.0, 10.0), vec![label]);
        let (lo, hi) = f.solve(0.0, &[100.0]).unwrap();
        assert_eq!(lo, 0.0);
        assert!(
            (hi - 10.0 / (1.0 - share(20.0, 100.0))).abs() < 1e-9,
            "{hi}"
        );
        assert!(f.contains(linear(lo, hi), &[100.0]));
        // Two one-sided reaches that together take the whole plot cannot fit.
        let both = fit(
            Transform::Linear,
            (0.0, 10.0),
            vec![label, end_label(10.0, 0.0, 50.0), end_label(0.0, 50.0, 0.0)],
        );
        // The two labels take half the plot each; the last of the steepest is named.
        assert_eq!(both.solve(0.0, &[100.0]), Err(Unfit::TooLarge(Some(2))));
    }

    #[test]
    fn a_fit_of_labels_alone_keeps_a_range_that_holds_them() {
        let label = |high| end_label(10.0, 0.0, high);
        let mut f = BubbleFit {
            on_demand: true,
            ..fit(Transform::Linear, (0.0, 10.0), vec![label(20.0)])
        };
        // The range 0 to 15 leaves a third of a 100 pt plot above the bar: the label fits.
        assert!(!f.update(linear(0.0, 15.0), &plots(100.0)));
        assert_eq!(f.state, State::Unfitted);
        assert!(
            !f.clone()
                .clip_if_unsettled(linear(0.0, 15.0), &plots(100.0))
        );
        // On the range 0 to 10 it does not: the fit is made, and a range not yet fitted that
        // stops holding the label counts as unsettled.
        assert!(
            f.clone()
                .clip_if_unsettled(linear(0.0, 10.0), &plots(100.0))
        );
        assert!(f.update(linear(0.0, 10.0), &plots(100.0)));
        assert!(matches!(f.state, State::Fitted { .. }));
        // A fit holding a bubble is made on its first update, as before.
        let mut bubbles = fit(Transform::Linear, (0.0, 10.0), vec![bubble(5.0, 1.0)]);
        assert!(bubbles.update(linear(0.0, 15.0), &plots(100.0)));
    }

    #[test]
    fn refits_are_bounded_then_clip_the_axis() {
        // A fit that does not settle is the one fallback left: no plot size can be named that
        // settles it, so its axis keeps the data limits and the plot clips its bubbles.
        let mut f = fit(Transform::Linear, (0.0, 10.0), vec![bubble(0.0, 1.0)]);
        assert!(f.update(f.seed, &plots(100.0)));
        let (lo, hi) = f.extent().unwrap();
        // The span moved right, as if the layout kept moving the bubble out: the fit is made
        // again each time, at most `MAX_REFITS` times, then the axis is clipped.
        let away = linear(lo + 1.0, hi + 1.0);
        let mut calls = 0;
        while f.state != State::Clipped {
            f.update(away, &plots(100.0));
            calls += 1;
            assert!(calls <= MAX_REFITS + 1, "{:?}", f.state);
            if let State::Fitted { refits, .. } = f.state {
                assert_eq!(refits, calls);
            }
        }
        assert_eq!(calls, MAX_REFITS + 1);
        assert_eq!(f.extent(), None);
    }
}
