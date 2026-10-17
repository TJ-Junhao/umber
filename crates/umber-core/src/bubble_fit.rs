//! Automatic limits that fit whole bubble markers, outlines included, on the final plot.
//!
//! A bubble's extent is in points while an axis domain is in data units, and the plot size is
//! known only once the layout is solved, so the final layout loop asks the fits again after
//! every solve ([`BubbleFits::update`]). A fit starts from the axis's raw extent, the extent of
//! the data and of any explicit tick positions as resolve sees it, and widens it to the smallest
//! extent in which every bubble reaches no further than the plot edge at its share of the plot
//! length. Resolve then pads that extent once (rounding it first on log axes), as it does any data extent.
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
//! When the bubbles cannot all fit on an axis (a bubble reaches at least half the plot length,
//! the fit leaves the supported numeric range, the layout cannot hold the fitted limits, or no
//! fit settles within [`MAX_REFITS`] attempts), the axis keeps the limits of the data positions
//! alone, as it would without bubble extents, and the plot clips the bubbles that overflow.

use crate::draw::marker_reach;
use crate::layout::PlotSize;
use crate::model::ShareGroups;
use crate::model::SharedValues;
use crate::resolve::{
    self, AxisLimits, AxisScale, ContinuousLimits, Limits, Resolved, ResolvedMark, TimeLimits,
    ValueAxis,
};
use crate::scale::{log_offset, seconds_between};
use crate::{ContinuousScale, Instant, Transform};

/// The most times a fit is made again after its first, before its axis keeps the limits of
/// the data positions.
const MAX_REFITS: usize = 8;
/// A drawing-space allowance, in points, added to every bubble's reach so that the final `f32`
/// path coordinates stay inside the plot.
const PATH_ALLOWANCE: f64 = 1e-4;
/// The error context of the raw extent's scale; its errors are never reported.
const CONTEXT: &str = "Figure.to_scene()";

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

/// One drawn bubble, as one axis sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bubble {
    /// The bubble's coordinate on this axis.
    value: f64,
    /// How far the marker reaches from its centre along this axis, outline included, in points
    /// (see [`marker_reach`]).
    reach: f64,
    /// The bubble's panel, in the order of [`Figure::panels`](crate::Figure::panels), whose plot
    /// length it is measured against.
    panel: usize,
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
    /// The bubbles cannot all fit: the axis has the limits of its data positions, and the plot
    /// clips the bubbles that overflow.
    Clipped,
}

/// The fit of the bubbles of one automatic axis, shared by every panel of its group.
#[derive(Debug, Clone, PartialEq)]
struct BubbleFit {
    /// The axis.
    axis: Axis,
    /// The raw extent of the axis as a scale: the data and explicit tick positions, before
    /// rounding and padding, with a single value widened as resolve widens it.
    seed: ContinuousScale,
    /// Every drawn bubble of the group.
    bubbles: Vec<Bubble>,
    /// Where the fit stands.
    state: State,
}

impl BubbleFit {
    /// The fit of `axis` with `limits`, whose group's members are `members`, each its panel
    /// index and resolved marks. `None` when the axis has no drawn bubble or its raw extent has
    /// no scale (a non-positive value on a log axis); such an axis has the limits of its data
    /// positions.
    fn new<'a, 'm: 'a>(
        axis: Axis,
        limits: &ContinuousLimits<'_>,
        members: impl Iterator<Item = (usize, &'a [ResolvedMark<'m>])> + Clone,
    ) -> Option<BubbleFit> {
        let bubbles = Self::drawn(axis, None, members.clone());
        if bubbles.is_empty() {
            return None;
        }
        let marks = members.flat_map(|(_, marks)| marks.iter());
        let data = resolve::axis_extent(marks, axis.which(), limits.transform);
        let raw = resolve::unrounded_extent(limits, data);
        let seed = ContinuousScale::from_extent(limits.transform, raw, CONTEXT).ok()?;
        Some(BubbleFit {
            axis,
            seed,
            bubbles,
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
    ) -> Option<BubbleFit> {
        let marks = members.clone().flat_map(|(_, marks)| marks.iter());
        let (_, extent) = resolve::time_extents(marks.clone(), axis.which(), limits.ticks);
        let (origin, _) = extent?;
        let bubbles = Self::drawn(axis, Some(origin), members);
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
            state: State::Unfitted,
        })
    }

    /// Every drawn bubble of the group's `members`, as `axis` sees it, with the times of a time
    /// axis in seconds from `origin`.
    fn drawn<'a, 'm: 'a>(
        axis: Axis,
        origin: Option<Instant>,
        members: impl Iterator<Item = (usize, &'a [ResolvedMark<'m>])>,
    ) -> Vec<Bubble> {
        members
            .flat_map(|(panel, marks)| {
                marks
                    .iter()
                    .flat_map(move |mark| bubbles(mark, axis, panel, origin))
            })
            .collect()
    }

    /// The extent this fit adds to its axis's data extent; `None` while it adds nothing.
    fn extent(&self) -> Option<(f64, f64)> {
        match self.state {
            State::Fitted { extent, .. } => Some(extent),
            State::Unfitted | State::Clipped => None,
        }
    }

    /// Whether every bubble reaches no further than the plot edges of `scale`, on plots of
    /// `lengths` (one per panel).
    fn contains(&self, scale: ContinuousScale, lengths: &[f64]) -> bool {
        self.bubbles.iter().all(|bubble| {
            let position = scale.normalize(bubble.value);
            let share = bubble.reach / lengths[bubble.panel];
            position >= share && position <= 1.0 - share
        })
    }

    /// Fits again for the axis resolved as `scale` on plots of `lengths`, when needed: the
    /// first time, and whenever a bubble reaches past the plot edges. Returns whether the
    /// extent this fit adds changed.
    fn update(&mut self, scale: ContinuousScale, lengths: &[f64]) -> bool {
        let next = match &self.state {
            State::Clipped => return false,
            State::Fitted { .. } if self.contains(scale, lengths) => return false,
            State::Unfitted => self.fitted(0.0, lengths.to_vec(), 0),
            State::Fitted { refits, .. } if *refits == MAX_REFITS => State::Clipped,
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
                    self.fitted(*span, lengths, refits + 1)
                } else {
                    // The plots did not shrink, so padding or log-axis rounding moved a bubble out:
                    // measure the reaches against at least the span of the final domain.
                    let (lo, hi) = scale.domain();
                    let rounded = self.seed.normalize(hi) - self.seed.normalize(lo);
                    self.fitted(span.max(rounded), fitted.clone(), refits + 1)
                }
            }
        };
        let before = self.extent();
        self.state = next;
        self.extent() != before
    }

    /// The state of a fit measuring the reaches against at least `span`, on plots of `lengths`:
    /// fitted when [`solve`](Self::solve) finds an extent, clipped otherwise.
    fn fitted(&self, span: f64, lengths: Vec<f64>, refits: usize) -> State {
        match self.solve(span, &lengths) {
            Some(extent) => State::Fitted {
                extent,
                span,
                lengths,
                refits,
            },
            None => State::Clipped,
        }
    }

    /// The smallest extent holding the raw extent and every bubble, on plots of `lengths`, with
    /// each reach measured against the larger of the extent's own span and `span`. `None` when a
    /// bubble reaches at least half its plot length, so no extent holds it, or the extent leaves
    /// the supported numeric range.
    ///
    /// In the raw extent's normalized axis space (log space on a log axis), a bubble at `v`
    /// whose reach is the share `a` of its plot length needs `lo <= v - a d` and `hi >= v + a d`,
    /// where `d` is the larger of `hi - lo` and `span`. Against the extent's own span,
    /// `d = max(v + a d) + max(-v + a d)`: both maxima are upper envelopes of lines in `d`, and
    /// sweeping their breakpoints finds the active pair in O(n log n), the raw extent's ends
    /// being the lines of slope 0. When that span is less than `span`, the bounds against `span`
    /// are direct, and their extent is no wider than `span`, since every pair of bounds is
    /// at most as wide against the smaller span.
    fn solve(&self, span: f64, lengths: &[f64]) -> Option<(f64, f64)> {
        let scale = self.seed;
        let mut upper = vec![Line {
            slope: 0.0,
            intercept: 1.0,
        }];
        let mut lower = vec![Line {
            slope: 0.0,
            intercept: 0.0,
        }];
        for bubble in &self.bubbles {
            let slope = (bubble.reach + PATH_ALLOWANCE) / lengths[bubble.panel];
            // A plot of no length gives a NaN slope.
            if slope.is_nan() || 2.0 * slope >= 1.0 {
                return None;
            }
            let value = scale.normalize(bubble.value);
            upper.push(Line {
                slope,
                intercept: value,
            });
            lower.push(Line {
                slope,
                intercept: -value,
            });
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
            .ok()
            .map(|scale| scale.domain())
    }
}

/// The drawn bubbles of `mark`, as `axis` of the panel `panel` sees them: none for any other
/// mark. A bubble is drawn when both its coordinates are finite (or present, for times) and its
/// radius is positive. The times of a time axis are seconds from `origin`.
fn bubbles<'a>(
    mark: &'a ResolvedMark<'_>,
    axis: Axis,
    panel: usize,
    origin: Option<Instant>,
) -> impl Iterator<Item = Bubble> + 'a {
    let (data, maximum, radius) = match *mark {
        ResolvedMark::Bubbles {
            mark,
            maximum,
            radius,
            ..
        } => (Some(mark), maximum, radius),
        _ => (None, 0.0, 0.0),
    };
    data.into_iter().flat_map(move |data| {
        let shape = data.options().marker;
        data.sizes()
            .iter()
            .enumerate()
            .filter_map(move |(i, &size)| {
                let r = crate::bubble_scale::radius(size, maximum, radius);
                if !(is_drawn(data.x_values(), i) && is_drawn(data.y_values(), i)) || r <= 0.0 {
                    return None;
                }
                let (reach_x, reach_y) = marker_reach(shape, 2.0 * r);
                let (value, reach) = match axis {
                    Axis::X => (coordinate(data.x_values(), i, origin), reach_x),
                    Axis::Y => (coordinate(data.y_values(), i, origin), reach_y),
                };
                Some(Bubble {
                    value,
                    reach,
                    panel,
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
}

impl BubbleFits {
    /// The fits of the panels whose resolved marks are `marks`, with the axis limits `limits`
    /// and the share groups `groups`. No fit is made yet: every axis has the limits of its data
    /// positions until the first [`update`](Self::update).
    pub fn new(marks: &[&[ResolvedMark<'_>]], limits: &[Limits<'_>], groups: &ShareGroups) -> Self {
        let fits = (0..marks.len())
            .map(|i| {
                Axis::BOTH.map(|axis| {
                    let keys = axis.groups(groups);
                    if keys[i] != i {
                        return None;
                    }
                    let members = (0..marks.len())
                        .filter(|j| keys[*j] == i)
                        .map(|j| (j, marks[j]));
                    match limits[i].axis(axis.which()) {
                        AxisLimits::Continuous(limits) if limits.explicit.is_none() => {
                            BubbleFit::new(axis, &limits, members)
                        }
                        AxisLimits::Time(limits) if limits.explicit.is_none() => {
                            BubbleFit::time(axis, &limits, members)
                        }
                        _ => None,
                    }
                })
            })
            .collect();
        BubbleFits { fits }
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
    /// once, again at most [`MAX_REFITS`] times, and clipped at most once.
    pub fn budget(&self) -> usize {
        self.fits.iter().flatten().flatten().count() * (MAX_REFITS + 2)
    }

    /// Fits again every axis whose bubbles need it on the axes `resolved` and the solved plots
    /// `plots` (see [`BubbleFit::update`]). Returns whether any extent changed, in which case
    /// the axes must be resolved and the layout solved again.
    pub fn update(&mut self, resolved: &[Resolved], plots: &[PlotSize]) -> bool {
        self.each(resolved, plots, BubbleFit::update)
    }

    /// Clips every fit whose bubbles still reach past the plot edges of the axes `resolved` on
    /// the solved plots `plots`: the fits that did not settle. Returns whether any extent
    /// changed.
    pub fn clip_unsettled(&mut self, resolved: &[Resolved], plots: &[PlotSize]) -> bool {
        self.each(resolved, plots, |fit, scale, lengths| {
            if fit.state == State::Clipped || fit.contains(scale, lengths) {
                return false;
            }
            let changed = fit.extent().is_some();
            fit.state = State::Clipped;
            changed
        })
    }

    /// Clips every fit, for a layout that cannot hold the fitted limits. Returns whether any
    /// extent changed.
    pub fn clip_all(&mut self) -> bool {
        let mut changed = false;
        for fit in self.fits.iter_mut().flatten().flatten() {
            changed |= fit.extent().is_some();
            fit.state = State::Clipped;
        }
        changed
    }

    /// Applies `step` to every fit with its axis's scale in `resolved` and its axis's plot
    /// lengths in `plots`. Returns whether any step returned true.
    fn each(
        &mut self,
        resolved: &[Resolved],
        plots: &[PlotSize],
        mut step: impl FnMut(&mut BubbleFit, ContinuousScale, &[f64]) -> bool,
    ) -> bool {
        let mut changed = false;
        for (i, pair) in self.fits.iter_mut().enumerate() {
            for fit in pair.iter_mut().flatten() {
                let Some(scale) = fit.axis.scale(&resolved[i]) else {
                    continue;
                };
                let lengths: Vec<f64> = plots.iter().map(|plot| fit.axis.length(*plot)).collect();
                changed |= step(fit, scale, &lengths);
            }
        }
        changed
    }
}

/// A line of the upper or reflected lower envelope, in normalized axis space.
#[derive(Debug, Clone, Copy)]
struct Line {
    slope: f64,
    intercept: f64,
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
            state: State::Unfitted,
        }
    }

    fn bubble(value: f64, reach: f64) -> Bubble {
        Bubble {
            value,
            reach,
            panel: 0,
        }
    }

    fn linear(lo: f64, hi: f64) -> ContinuousScale {
        ContinuousScale::new(Transform::Linear, lo, hi, "test").unwrap()
    }

    #[test]
    fn bubbles_reaching_half_the_plot_or_past_the_numeric_range_cannot_fit() {
        // 18.4 pt either side of the centre on a 36.8 pt plot: no extent holds it, whatever
        // span its reach is measured against.
        let wide = fit(Transform::Linear, (0.0, 1.0), vec![bubble(0.5, 18.4)]);
        assert_eq!(wide.solve(0.0, &[36.8]), None);
        assert_eq!(wide.solve(10.0, &[36.8]), None);
        assert!(wide.solve(0.0, &[36.9]).is_some());
        assert_eq!(wide.solve(0.0, &[0.0]), None);
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
            assert_eq!(edge.solve(0.0, &[200.0]), None);
        }
    }

    #[test]
    fn a_fit_that_cannot_be_made_clips_its_axis_without_failing() {
        let mut wide = fit(Transform::Linear, (0.0, 1.0), vec![bubble(0.5, 18.4)]);
        let scale = wide.seed;
        assert!(
            !wide.update(scale, &[36.8]),
            "nothing is added to the data extent"
        );
        assert_eq!(wide.state, State::Clipped);
        assert_eq!(wide.extent(), None);
        // A clipped fit stays clipped, whatever the plot becomes.
        assert!(!wide.update(scale, &[400.0]));
        assert_eq!(wide.state, State::Clipped);
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
                .map(|b| (b.value, (b.reach + PATH_ALLOWANCE) / 100.0)),
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
        assert_eq!(fit.solve(expected / 2.0, &[100.0]), Some((lo, hi)));
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
        assert!(f.update(f.seed, &[100.0]));
        assert_close(f.extent().unwrap(), tight(100.0));
        // The plot shrinks: the fit is made again from the raw extent on the shorter plot.
        let (lo, hi) = f.extent().unwrap();
        assert!(!f.contains(linear(lo, hi), &[80.0]));
        assert!(f.update(linear(lo, hi), &[80.0]));
        assert_eq!(fitted(&f), (0.0, &[80.0][..], 1));
        assert_close(f.extent().unwrap(), tight(80.0));
        // A plot that grows again keeps the fit, which still holds the bubbles.
        let (lo, hi) = f.extent().unwrap();
        assert!(!f.update(linear(lo, hi), &[120.0]));
        // Rounding widens the domain on the right only, so the bubble at 0 reaches past the
        // left edge. The fit is made again from the raw extent, with the reaches measured
        // against the rounded span, for resolve to round and pad once: the padded domain is
        // never fitted again.
        let rounded = (lo, hi + 10.0);
        assert!(!f.contains(linear(rounded.0, rounded.1), &[80.0]));
        assert!(f.update(linear(rounded.0, rounded.1), &[80.0]));
        let d = rounded.1 - rounded.0;
        let (span, lengths, refits) = fitted(&f);
        assert!((span - d / 10.0).abs() < 1e-12, "{span}");
        assert_eq!((lengths, refits), (&[80.0][..], 2));
        let expected = (-share(10.0, 80.0) * d, 10.0 + share(20.0, 80.0) * d);
        assert_close(f.extent().unwrap(), expected);
        assert!(expected.1 - expected.0 < d);
    }

    #[test]
    fn refits_are_bounded_then_clip_the_axis() {
        let mut f = fit(Transform::Linear, (0.0, 10.0), vec![bubble(0.0, 1.0)]);
        assert!(f.update(f.seed, &[100.0]));
        let (lo, hi) = f.extent().unwrap();
        // The span moved right, as if the layout kept moving the bubble out: the fit is made
        // again each time, at most `MAX_REFITS` times, then the axis is clipped.
        let away = linear(lo + 1.0, hi + 1.0);
        let mut calls = 0;
        while f.state != State::Clipped {
            f.update(away, &[100.0]);
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
