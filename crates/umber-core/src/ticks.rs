//! Tick positions (Extended Wilkinson) and tick label formatting.
//!
//! Ticks are produced by [`ContinuousScale::ticks`](crate::ContinuousScale::ticks). Labels are
//! clean (`0.3`, never `0.30000000000000004`), use U+2212 for negatives, and never read `-0`.
//!
//! # Examples
//!
//! ```
//! use umber_core::{ContinuousScale, Transform};
//!
//! let scale = ContinuousScale::new(Transform::Linear, -40.0, 120.0, "doc").unwrap();
//! let labels: Vec<String> = scale.ticks(5).into_iter().map(|t| t.label).collect();
//! assert_eq!(labels, ["\u{2212}40", "0", "40", "80", "120"]);
//! ```

use std::collections::HashMap;

/// One axis tick: a position in data space and the text printed next to it.
///
/// # Examples
///
/// ```
/// use umber_core::{ContinuousScale, Transform};
///
/// let scale = ContinuousScale::new(Transform::Linear, 0.0, 100.0, "doc").unwrap();
/// let ticks = scale.ticks(5);
/// assert_eq!(ticks[0].value, 0.0);
/// assert_eq!(ticks[0].label, "0");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    /// Position of the tick in data space.
    pub value: f64,
    /// Text printed next to the tick, e.g. `"2.5"` or `"\u{2212}40"`.
    pub label: String,
}

/// Where tick values come from, which decides whether a value near zero means zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TickValues {
    /// Values computed on an evenly spaced linear grid with this step, as `start + i * step`: a
    /// value within `step * 1e-9` of zero is a zero that missed by a rounding error.
    Grid {
        /// The grid step.
        step: f64,
    },
    /// Exact values: positions a caller chose, and log ticks.
    Exact,
}

/// Automatic ticks, with where their values come from: the step their labels were written for
/// on a linear grid, so that another tick format decides zero exactly as the automatic labels
/// did.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AutoTicks {
    /// The ticks, with their automatic labels.
    pub ticks: Vec<Tick>,
    /// Where their values come from.
    pub values: TickValues,
}

/// Largest number of ticks any function here aims for; larger targets are clamped to it.
pub(crate) const MAX_TARGET: usize = 64;

/// Clamps a caller-supplied tick target to `2..=MAX_TARGET`.
pub(crate) fn clamp_target(target: usize) -> usize {
    target.clamp(2, MAX_TARGET)
}

const Q: [f64; 6] = [1.0, 5.0, 2.0, 2.5, 4.0, 3.0];
const W: [f64; 4] = [0.25, 0.2, 0.5, 0.05];
/// `Q.len() - 1`, as a float.
const Q_LAST: f64 = 5.0;

fn simplicity(qi: usize, j: f64, lmin: f64, lmax: f64, lstep: f64) -> f64 {
    // Measure the error in grid units: an absolute tolerance in data units loses zero on
    // large grids and invents it between ticks on tiny ones.
    let index = lmin / lstep;
    let tolerance = 4.0 * f64::EPSILON * index.abs().max(1.0);
    let zero_included = (index - index.round()).abs() <= tolerance && lmin <= 0.0 && 0.0 <= lmax;
    let v = if zero_included { 1.0 } else { 0.0 };
    1.0 - qi as f64 / Q_LAST - j + v
}

fn simplicity_max(qi: usize, j: f64) -> f64 {
    1.0 - qi as f64 / Q_LAST - j + 1.0
}

/// `1 - 0.5 * sum_sq / (0.1 * range)^2`, where `sum_sq` is the sum of the squared `diffs`.
///
/// The direct form reproduces the reference implementation bit for bit, which matters because
/// candidates can tie exactly. When a square would overflow or underflow (spans below about
/// 1e-150 or above about 1e150), each difference is divided by `0.1 * range` before squaring.
fn coverage_penalty(range: f64, diffs: [f64; 2]) -> f64 {
    let den = (0.1 * range).powi(2);
    let num = diffs[0].powi(2) + diffs[1].powi(2);
    if den.is_normal() && num.is_finite() {
        1.0 - 0.5 * num / den
    } else {
        let scale = 0.1 * range;
        let (a, b) = (diffs[0] / scale, diffs[1] / scale);
        1.0 - 0.5 * (a * a + b * b)
    }
}

fn coverage(dmin: f64, dmax: f64, lmin: f64, lmax: f64) -> f64 {
    coverage_penalty(dmax - dmin, [dmax - lmax, dmin - lmin])
}

fn coverage_max(dmin: f64, dmax: f64, span: f64) -> f64 {
    let r = dmax - dmin;
    if span > r {
        let half = (span - r) / 2.0;
        coverage_penalty(r, [half, half])
    } else {
        1.0
    }
}

fn density(k: f64, m: f64, dmin: f64, dmax: f64, lmin: f64, lmax: f64) -> f64 {
    let r = (k - 1.0) / (lmax - lmin);
    let rt = (m - 1.0) / (lmax.max(dmax) - dmin.min(lmin));
    2.0 - (r / rt).max(rt / r)
}

fn density_max(k: f64, m: f64) -> f64 {
    if k >= m {
        2.0 - (k - 1.0) / (m - 1.0)
    } else {
        1.0
    }
}

/// `n` values from `lo` to `hi` inclusive, each computed from its index (no accumulation).
pub(crate) fn evenly_spaced(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let n = clamp_target(n);
    let last = (n - 1) as f64;
    (0..n)
        .map(|i| {
            let t = i as f64 / last;
            lo * (1.0 - t) + hi * t
        })
        .collect()
}

/// Which tick sets [`extended`] may choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TickCoverage {
    /// Loose labeling: the ticks cover the range, so the first is `<= dmin` and the last is
    /// `>= dmax`.
    Loose,
    /// Any tick set, which may stop short of either end of the range.
    Free,
}

/// What a tick search must keep apart: a candidate set of tick values fits when its labels
/// leave room between them on screen.
#[derive(Clone, Copy)]
pub(crate) struct Fit<'a> {
    /// The least step, as a share of the searched range `dmax - dmin`, that can fit whatever
    /// the labels: on a linear axis that shows at least the range, ticks are never further
    /// apart on screen than their step's share of it. A candidate with a smaller step does not
    /// fit and is not judged; 0 judges every candidate.
    pub least_share: f64,
    /// The room a candidate's values leave between their labels.
    pub slack: &'a dyn Fn(&[f64]) -> Room,
}

/// The room a candidate's labels leave on screen, as [`Fit::slack`] judges it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Room {
    /// Fewer than two of the candidate's ticks are drawn, so it has no labels to keep apart.
    TooFewTicks,
    /// The room, in points, between the most crowded pair of adjacent labels: the labels fit
    /// when it is not negative.
    Slack(f64),
}

impl Fit<'_> {
    /// Every candidate fits: the search chooses for its target alone.
    pub(crate) const ANY: Fit<'static> = Fit {
        least_share: 0.0,
        slack: &unjudged,
    };

    /// Whether the best scored candidate, `values` `step` apart in a searched range `range`
    /// wide, stays the choice: its labels fit, or fewer than two of its ticks are drawn, and the
    /// caller chooses again for another target as it always has.
    fn keeps(&self, values: &[f64], step: f64, range: f64) -> bool {
        self.steps_apart(step, range)
            && match (self.slack)(values) {
                Room::TooFewTicks => true,
                Room::Slack(slack) => slack >= 0.0,
            }
    }

    /// Whether a candidate chosen in place of the best scored one, `values` `step` apart in a
    /// searched range `range` wide, fits: at least two of its ticks are drawn and their labels
    /// fit.
    fn admits(&self, values: &[f64], step: f64, range: f64) -> bool {
        self.steps_apart(step, range)
            && matches!((self.slack)(values), Room::Slack(slack) if slack >= 0.0)
    }

    /// Whether `step` reaches the least share of a range `range` wide.
    fn steps_apart(&self, step: f64, range: f64) -> bool {
        // A step a rounding error short of the least share is left to the judge.
        step >= self.least_share * range * (1.0 - 1e-9)
    }
}

/// The room left by values that are not judged: unlimited.
fn unjudged(_: &[f64]) -> Room {
    Room::Slack(f64::INFINITY)
}

/// `k` values from `lmin`, `step` apart, each computed from its index.
fn candidate(lmin: f64, step: f64, k: usize) -> Vec<f64> {
    (0..k).map(|i| lmin + i as f64 * step).collect()
}

/// One Extended Wilkinson candidate: `k` ticks from `lmin`, `step` apart, and its score.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    /// The first value.
    lmin: f64,
    /// The step.
    step: f64,
    /// The number of ticks.
    k: usize,
    /// The score.
    score: f64,
}

impl Candidate {
    /// The tick values, each computed from its index.
    fn values(&self) -> Vec<f64> {
        candidate(self.lmin, self.step, self.k)
    }
}

/// The mantissas of round steps, which [`round_candidates`] tries, each with its index in
/// [`Q`], which scores its simplicity.
const ROUND_MANTISSAS: [(usize, f64); 4] = [(0, 1.0), (2, 2.0), (3, 2.5), (1, 5.0)];

/// What one pass of the Extended Wilkinson search found.
#[derive(Debug, Default)]
struct Search {
    /// The best scored candidate that was admitted.
    best: Option<Candidate>,
    /// The fallback among the two-tick candidates that were not admitted.
    widest: Option<Widest>,
}

/// A two-tick candidate that [`extended`] may fall back to when no candidate fits, ranked by
/// how far apart its ticks lie on the axis, then by score, then by the order found.
#[derive(Debug, Clone, Copy)]
struct Widest {
    /// The step as a share of the span the axis shows: the candidate's own span or the range,
    /// whichever is wider.
    share: f64,
    /// The candidate.
    candidate: Candidate,
}

impl Widest {
    /// `candidate` as a fallback for the range `[dmin, dmax]`.
    fn of(candidate: Candidate, dmin: f64, dmax: f64) -> Widest {
        let lmax = candidate.lmin + candidate.step * (candidate.k as f64 - 1.0);
        Widest {
            share: candidate.step / (lmax.max(dmax) - candidate.lmin.min(dmin)),
            candidate,
        }
    }

    /// Whether `self` is a better fallback than `other`.
    fn beats(&self, other: Option<&Widest>) -> bool {
        other.is_none_or(|other| {
            self.share > other.share
                || (self.share == other.share && self.candidate.score > other.candidate.score)
        })
    }
}

/// Extended Wilkinson tick positions for `[dmin, dmax]` (Talbot, Lin & Hanrahan, 2010).
///
/// `tick_coverage` says whether the ticks must cover the range. Linear candidates have at most
/// `ceil(1.5 * target)` ticks, each a whole multiple of the step, so zero belongs to the grid.
/// The best scored candidate stays when its labels fit, otherwise the best fitting round set
/// from [`round_candidates`]. If neither fits, the remaining whole-step sets, including pairs
/// strictly inside a free range, are ranked by fewest ticks, then score. If none fits, the
/// largest label slack wins,
/// with fewer ticks, wider spacing and the score breaking ties. Covering both signs needs at
/// least three whole-step ticks.
/// Values are computed from their indices, never by accumulation. Invalid input (non-finite or
/// `dmin >= dmax`) and the case where no candidate is found yield `max(target, 2)` evenly
/// spaced values instead, or the two range ends when those do not fit.
pub(crate) fn extended(
    dmin: f64,
    dmax: f64,
    target: usize,
    tick_coverage: TickCoverage,
    fit: &Fit<'_>,
) -> Vec<f64> {
    extended_in(dmin, dmax, target, tick_coverage, fit, SearchMode::Linear)
}

/// Linear fallback ticks on a log axis, retaining the full Extended Wilkinson search.
pub(crate) fn log_fallback(dmin: f64, dmax: f64, target: usize, fit: &Fit<'_>) -> Vec<f64> {
    extended_in(
        dmin,
        dmax,
        target,
        TickCoverage::Free,
        fit,
        SearchMode::LogFallback,
    )
}

/// The grid and density rules of the axis that requested the search.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    /// Whole-step ticks, with a cap on their number.
    Linear,
    /// The original Extended Wilkinson search for a log axis without two powers.
    LogFallback,
}

impl SearchMode {
    /// The largest skip allowed in the main search.
    fn max_skip(self) -> u32 {
        match self {
            Self::Linear => 1,
            Self::LogFallback => 50,
        }
    }

    /// The largest candidate set for this target.
    fn max_ticks(self, target: usize) -> usize {
        match self {
            Self::Linear => (3 * target).div_ceil(2).min(50),
            Self::LogFallback => 50,
        }
    }
}

/// The shared search, with skips above one allowed only for a log-axis fallback.
fn extended_in(
    dmin: f64,
    dmax: f64,
    target: usize,
    tick_coverage: TickCoverage,
    fit: &Fit<'_>,
    mode: SearchMode,
) -> Vec<f64> {
    let target = clamp_target(target);
    let evenly = || {
        let values = evenly_spaced(dmin, dmax, target);
        let fits = match (fit.slack)(&values) {
            Room::TooFewTicks => true,
            Room::Slack(slack) => slack >= 0.0,
        };
        if fits {
            values
        } else {
            evenly_spaced(dmin, dmax, 2)
        }
    };
    if !(dmin.is_finite() && dmax.is_finite() && dmin < dmax) {
        return evenly();
    }
    let range = dmax - dmin;
    let best = search(dmin, dmax, target, tick_coverage, mode, &|_| true).best;
    if let Some(best) = best {
        if fit.keeps(&best.values(), best.step, range) {
            return best.values();
        }
    } else if mode == SearchMode::LogFallback {
        return evenly();
    }
    // A candidate in place of the best needs two ticks drawn: one that does not would only
    // raise the bar for the others.
    let admits = |c: &Candidate| fit.admits(&c.values(), c.step, range);
    let (round, whole) = round_candidates(dmin, dmax, target, tick_coverage, mode);
    let better = |chosen: Option<Candidate>, c: Candidate| match chosen {
        Some(chosen) if chosen.score >= c.score => Some(chosen),
        _ => Some(c),
    };
    if let Some(chosen) = round.iter().copied().filter(admits).fold(None, better) {
        return chosen.values();
    }
    if mode == SearchMode::Linear {
        if let Some(chosen) = whole
            .iter()
            .filter(|c| admits(c))
            .min_by(|a, b| a.k.cmp(&b.k).then_with(|| b.score.total_cmp(&a.score)))
        {
            return chosen.values();
        }
        // Even equally spaced pairs can have different label widths: compare actual slack.
        return whole
            .iter()
            .map(|c| {
                let slack = match (fit.slack)(&c.values()) {
                    Room::Slack(slack) => slack,
                    Room::TooFewTicks => f64::NEG_INFINITY,
                };
                (c, slack)
            })
            .max_by(|(a, sa), (b, sb)| {
                sa.total_cmp(sb)
                    .then_with(|| b.k.cmp(&a.k))
                    .then_with(|| {
                        Widest::of(**a, dmin, dmax)
                            .share
                            .total_cmp(&Widest::of(**b, dmin, dmax).share)
                    })
                    .then_with(|| a.score.total_cmp(&b.score))
            })
            .map_or_else(evenly, |(c, _)| c.values());
    }
    // Log-axis fallback: retain the full search and its two-tick fallbacks.
    let pair = |c: &Candidate| c.k == 2 && admits(c);
    if let Some(chosen) = search(dmin, dmax, target, tick_coverage, mode, &pair).best {
        return chosen.values();
    }
    let widest_round = round
        .iter()
        .filter(|c| c.k == 2)
        .map(|&c| Widest::of(c, dmin, dmax))
        .fold(None, |widest: Option<Widest>, c| {
            if c.beats(widest.as_ref()) {
                Some(c)
            } else {
                widest
            }
        });
    let widest =
        widest_round.or_else(|| search(dmin, dmax, target, tick_coverage, mode, &|_| false).widest);
    match widest {
        Some(widest) => widest.candidate.values(),
        None => evenly(),
    }
}

/// Whole-step fallback sets, scored as [`extended`] scores every candidate. Each step is a
/// mantissa times a power of ten, from a fiftieth of the range up to ten times its next power
/// of ten. Loose sets cover the range tightly; free sets lie inside it. Linear searches also
/// try adjacent pairs inside each free set, keeping zero whenever the range contains it.
/// Returns the round sets first, then all whole-step sets. Log fallbacks retain the four
/// round mantissas and full sets of 2 to 50 ticks.
fn round_candidates(
    dmin: f64,
    dmax: f64,
    target: usize,
    tick_coverage: TickCoverage,
    mode: SearchMode,
) -> (Vec<Candidate>, Vec<Candidate>) {
    let range = dmax - dmin;
    let tol = 1e-9 * range;
    let (first, last) = ((range / 50.0).log10().floor(), range.log10().ceil() + 1.0);
    if !(first.is_finite() && last.is_finite()) {
        return (Vec::new(), Vec::new());
    }
    let mantissas: &[(usize, f64)] = match mode {
        SearchMode::Linear => &[(0, 1.0), (2, 2.0), (3, 2.5), (5, 3.0), (4, 4.0), (1, 5.0)],
        SearchMode::LogFallback => &ROUND_MANTISSAS,
    };
    let mut round = Vec::new();
    let mut whole = Vec::new();
    let mut z = first;
    while z <= last {
        for &(qi, mantissa) in mantissas {
            let step = mantissa * 10f64.powf(z);
            let (lo, hi) = match tick_coverage {
                TickCoverage::Loose => ((dmin / step).floor(), (dmax / step).ceil()),
                TickCoverage::Free => (((dmin - tol) / step).ceil(), ((dmax + tol) / step).floor()),
            };
            let count = hi - lo + 1.0;
            if !(count.is_finite() && (2.0..=50.0).contains(&count)) {
                continue;
            }
            let candidate = |start: f64, k: usize| {
                let lmin = start * step;
                let lmax = lmin + step * (k - 1) as f64;
                Candidate {
                    lmin,
                    step,
                    k,
                    score: W[0] * simplicity(qi, 1.0, lmin, lmax, step)
                        + W[1] * coverage(dmin, dmax, lmin, lmax)
                        + W[2] * density(k as f64, target as f64, dmin, dmax, lmin, lmax)
                        + W[3],
                }
            };
            if count <= mode.max_ticks(target) as f64 {
                let c = candidate(lo, count as usize);
                if qi < 4 {
                    round.push(c);
                }
                whole.push(c);
            }
            if mode == SearchMode::Linear && tick_coverage == TickCoverage::Free && count > 2.0 {
                for i in 0..(count as usize - 1) {
                    let start = lo + i as f64;
                    if dmin <= 0.0 && dmax >= 0.0 && !(start <= 0.0 && start + 1.0 >= 0.0) {
                        continue;
                    }
                    whole.push(candidate(start, 2));
                }
            }
        }
        z += 1.0;
    }
    (round, whole)
}

/// One pass of the Extended Wilkinson search over `[dmin, dmax]` for about `target` ticks,
/// among the candidates `admit` lets through, keeping the two-tick fallbacks among those it
/// does not.
fn search(
    dmin: f64,
    dmax: f64,
    target: usize,
    tick_coverage: TickCoverage,
    mode: SearchMode,
    admit: &dyn Fn(&Candidate) -> bool,
) -> Search {
    let tol = 1e-9 * (dmax - dmin);
    let m = target as f64;
    let mut found = Search::default();
    let mut best_score = -2.0_f64;

    let mut j: u32 = 1;
    // A skip above one permits starts between whole multiples of the step.
    'jobs: while j <= mode.max_skip() {
        let jf = f64::from(j);
        for (qi, &q) in Q.iter().enumerate() {
            let sm = simplicity_max(qi, jf);
            if W[0] * sm + W[1] + W[2] + W[3] < best_score {
                break 'jobs;
            }
            let mut k: usize = 2;
            while k <= mode.max_ticks(target) {
                let kf = k as f64;
                let dm = density_max(kf, m);
                if W[0] * sm + W[1] + W[2] * dm + W[3] < best_score {
                    break;
                }
                let delta = (dmax - dmin) / (kf + 1.0) / jf / q;
                let zf = delta.log10().ceil();
                if zf.is_finite() && zf.abs() < 1e6 {
                    let mut z = zf as i32;
                    while z < 400 {
                        let step = jf * q * 10f64.powf(f64::from(z));
                        if !step.is_finite() || step <= 0.0 {
                            break;
                        }
                        let cm = coverage_max(dmin, dmax, step * (kf - 1.0));
                        if W[0] * sm + W[1] * cm + W[2] * dm + W[3] < best_score {
                            break;
                        }
                        let min_start = (dmax / step).floor() * jf - (kf - 1.0) * jf;
                        let max_start = (dmin / step).ceil() * jf;
                        if min_start.is_finite() && max_start.is_finite() {
                            let (lo_i, hi_i) = (min_start as i64, max_start as i64);
                            // A sane search window is a handful of starts; skip absurd ones.
                            if hi_i.saturating_sub(lo_i) <= 1_000_000 {
                                for start in lo_i..=hi_i {
                                    let lmin = start as f64 * (step / jf);
                                    let lmax = lmin + step * (kf - 1.0);
                                    let s = simplicity(qi, jf, lmin, lmax, step);
                                    let c = coverage(dmin, dmax, lmin, lmax);
                                    let g = density(kf, m, dmin, dmax, lmin, lmax);
                                    let score = W[0] * s + W[1] * c + W[2] * g + W[3];
                                    let covers = lmin <= dmin && lmax >= dmax;
                                    if !(score > best_score
                                        && (tick_coverage == TickCoverage::Free || covers))
                                    {
                                        continue;
                                    }
                                    let candidate = Candidate {
                                        lmin,
                                        step,
                                        k,
                                        score,
                                    };
                                    if admit(&candidate) {
                                        best_score = score;
                                        found.best = Some(candidate);
                                        continue;
                                    }
                                    let inside = lmin >= dmin - tol && lmax <= dmax + tol;
                                    if k != 2 || (tick_coverage == TickCoverage::Free && !inside) {
                                        continue;
                                    }
                                    let fallback = Widest::of(candidate, dmin, dmax);
                                    if fallback.beats(found.widest.as_ref()) {
                                        found.widest = Some(fallback);
                                    }
                                }
                            }
                        }
                        z += 1;
                    }
                }
                k += 1;
            }
        }
        j += 1;
    }
    found
}

/// Smallest number of decimals (0..=12) at which `value` is a whole number of units,
/// within `relative_tolerance` of the scaled value.
fn decimals(value: f64, relative_tolerance: f64) -> usize {
    (0..=12_i32)
        .find(|&d| {
            let scaled = value * 10f64.powi(d);
            (scaled - scaled.round()).abs() < relative_tolerance * scaled
        })
        .map_or(12, |d| d as usize)
}

/// Most decimals tried in fixed form before falling back to shortest round-trip.
const MAX_DECIMALS: usize = 15;
/// Significant digits tried in scientific form: from the default up to the `f64` limit.
const SCIENTIFIC_DIGITS: std::ops::RangeInclusive<usize> = 12..=17;

/// Scientific form `<mantissa>e<exp>` of a non-zero `v`. `digits` is the number of significant
/// digits (trailing zeros are trimmed), or `None` for the shortest round-trip mantissa.
fn scientific(v: f64, digits: Option<usize>) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    let text = match digits {
        None => format!("{v:e}"),
        Some(n) => {
            let rounded = format!("{v:.prec$e}", prec = n.saturating_sub(1));
            match rounded.split_once('e') {
                Some((mantissa, exp)) => {
                    let mantissa = if mantissa.contains('.') {
                        mantissa.trim_end_matches('0').trim_end_matches('.')
                    } else {
                        mantissa
                    };
                    format!("{mantissa}e{exp}")
                }
                None => rounded,
            }
        }
    };
    text.replace('-', "\u{2212}")
}

/// Fixed-point form of `v`: `decimals` places, or shortest round-trip for `None`. A value that
/// rounds to zero prints without a sign, and negatives use U+2212.
fn fixed(v: f64, decimals: Option<usize>) -> String {
    let text = match decimals {
        Some(d) => format!("{v:.d$}"),
        None => format!("{v}"),
    };
    let unsigned = text.trim_start_matches('-');
    if unsigned.chars().all(|c| c == '0' || c == '.') {
        unsigned.to_string()
    } else {
        text.replace('-', "\u{2212}")
    }
}

/// True when no two distinct values share a label.
fn labels_are_unique(values: &[f64], labels: &[String]) -> bool {
    let mut seen: HashMap<&str, u64> = HashMap::with_capacity(labels.len());
    values.iter().zip(labels).all(|(v, label)| {
        let bits = v.to_bits();
        *seen.entry(label.as_str()).or_insert(bits) == bits
    })
}

/// Fixed-point labels with `start` decimals, raising the decimals one at a time (up to
/// [`MAX_DECIMALS`], then shortest round-trip) until distinct values get distinct labels.
fn unique_fixed(values: &[f64], start: usize) -> Vec<String> {
    for d in start..=MAX_DECIMALS {
        let labels: Vec<String> = values.iter().map(|&v| fixed(v, Some(d))).collect();
        if labels_are_unique(values, &labels) {
            return labels;
        }
    }
    values.iter().map(|&v| fixed(v, None)).collect()
}

/// Scientific labels (zero stays `0`), raising the significant digits one at a time until
/// distinct values get distinct labels, then shortest round-trip.
fn unique_scientific(values: &[f64]) -> Vec<String> {
    let label = |v: f64, digits: Option<usize>| {
        if v == 0.0 {
            "0".to_string()
        } else {
            scientific(v, digits)
        }
    };
    for digits in SCIENTIFIC_DIGITS {
        let labels: Vec<String> = values.iter().map(|&v| label(v, Some(digits))).collect();
        if labels_are_unique(values, &labels) {
            return labels;
        }
    }
    values.iter().map(|&v| label(v, None)).collect()
}

/// Labels for tick `values` spaced `step` apart (the grid policy).
///
/// The number of decimals follows from `step`, values within `step * 1e-9` of zero print as an
/// unsigned zero, negatives use U+2212, and a set containing a value of at least `1e6` or a
/// non-zero value below `1e-4` is written in scientific form. If distinct values would share a
/// label, the precision is raised until they do not.
pub(crate) fn format_ticks(values: &[f64], step: f64) -> Vec<String> {
    let step = step.abs();
    grid_labels(&snapped_to_zero(values, step), decimals(step, 1e-6))
}

/// `values` with every value within `step * 1e-9` of zero set to an unsigned zero: ticks
/// computed as `start + i * step` on a grid of that `step` can miss zero by a rounding error,
/// and such a tick means zero. Only computed grid ticks are snapped; values a caller chose, and
/// log ticks, are exact.
pub(crate) fn snapped_to_zero(values: &[f64], step: f64) -> Vec<f64> {
    let tiny = step.abs() * 1e-9;
    values
        .iter()
        .map(|&v| if v == 0.0 || v.abs() < tiny { 0.0 } else { v })
        .collect()
}

/// Grid-policy labels for `values` with `places` decimals: scientific form when a non-zero value
/// is at least `1e6` or below `1e-4`, fixed-point otherwise, with the precision raised until
/// distinct values get distinct labels.
fn grid_labels(values: &[f64], places: usize) -> Vec<String> {
    let use_scientific = values
        .iter()
        .any(|&v| v != 0.0 && (v.abs() >= 1e6 || v.abs() < 1e-4));
    if use_scientific {
        unique_scientific(values)
    } else {
        unique_fixed(values, places)
    }
}

/// Labels for tick values that a caller chose rather than a tick search, by the grid policy of
/// [`format_ticks`], for the values exactly as given. Such values need not sit on a regular
/// grid, so the decimals are the finest place any of them needs (`0.25` and `1.5` give two):
/// `[0, 1.5, 3.25]` reads `0.00`, `1.50`, `3.25`. No value is snapped to zero, so tiny values
/// such as `1e-24` choose the scientific form and keep labels of their own.
pub(crate) fn format_given(values: &[f64]) -> Vec<String> {
    let places = values
        .iter()
        .filter(|v| **v != 0.0)
        // Unlike computed steps, explicit values may have meaningful fractional parts at
        // any magnitude. Allow only the rounding error of scaling a value by a power of ten.
        .map(|v| decimals(v.abs(), 4.0 * f64::EPSILON))
        .max()
        .unwrap_or(0);
    // Only the sign of a zero changes.
    let values: Vec<f64> = values
        .iter()
        .map(|&v| if v == 0.0 { 0.0 } else { v })
        .collect();
    grid_labels(&values, places)
}

/// Fixed-point labels with exactly `decimals` places for every value as given, even when two
/// values then share a label. A value that rounds to zero prints without a sign, and negatives
/// use U+2212.
pub(crate) fn format_fixed(values: &[f64], decimals: usize) -> Vec<String> {
    values.iter().map(|&v| fixed(v, Some(decimals))).collect()
}

/// Scientific labels (`2.5e6`, `1e−5`) with the default 12 significant digits, trailing zeros
/// trimmed, for every value as given, even when two values then share a label. Zero is `0`.
pub(crate) fn format_scientific(values: &[f64]) -> Vec<String> {
    values
        .iter()
        .map(|&v| {
            if v == 0.0 {
                "0".to_string()
            } else {
                scientific(v, Some(*SCIENTIFIC_DIGITS.start()))
            }
        })
        .collect()
}

/// `v` rounded to `digits` significant digits.
fn round_significant(v: f64, digits: usize) -> f64 {
    format!("{v:.prec$e}", prec = digits.saturating_sub(1))
        .parse()
        .unwrap_or(v)
}

/// Labels for values that do not sit on a regular grid (the value policy).
///
/// The whole set uses one form: scientific (`<mantissa>e<exp>`) when any non-zero value lies
/// outside `1e-4 <= |v| < 1e6`, otherwise fixed. Zero is `0`. Values are written with 12
/// significant digits (trailing zeros trimmed, so `1.1^2` reads `1.21`); if distinct values
/// would collide, with 17, and finally with the shortest round-trip digits, which always
/// separates distinct values.
pub(crate) fn format_values(values: &[f64]) -> Vec<String> {
    let use_scientific = values
        .iter()
        .any(|&v| v != 0.0 && !(1e-4..1e6).contains(&v.abs()));
    let label = |v: f64, digits: Option<usize>| -> String {
        if v == 0.0 {
            "0".to_string()
        } else if use_scientific {
            scientific(v, digits)
        } else {
            fixed(digits.map_or(v, |n| round_significant(v, n)), None)
        }
    };
    for digits in [12, 17] {
        let labels: Vec<String> = values.iter().map(|&v| label(v, Some(digits))).collect();
        if labels_are_unique(values, &labels) {
            return labels;
        }
    }
    values.iter().map(|&v| label(v, None)).collect()
}

/// Labels for finite `values` that are not ticks, such as the values written on the cells of a
/// heatmap, one label each. With a `step`, each value has the decimals that labels of ticks
/// `step` apart have (the grid policy); without one, up to 12 significant digits (the value
/// policy). The whole set is written in scientific form when a non-zero value lies outside
/// `1e-4 <= |v| < 1e6`. Unlike tick labels, two values may share a label: the precision is
/// never raised to tell them apart.
pub(crate) fn format_annotations(values: &[f64], step: Option<f64>) -> Vec<String> {
    let use_scientific = values
        .iter()
        .any(|&v| v != 0.0 && !(1e-4..1e6).contains(&v.abs()));
    let places = step.map(|step| decimals(step.abs(), 1e-6));
    values
        .iter()
        .map(|&v| match (use_scientific, places) {
            (true, _) if v == 0.0 => "0".to_owned(),
            (true, _) => scientific(v, Some(*SCIENTIFIC_DIGITS.start())),
            (false, Some(places)) => fixed(v, Some(places)),
            (false, None) => fixed(round_significant(v, *SCIENTIFIC_DIGITS.start()), None),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(x, y)| (x - y).abs() <= 1e-9 * y.abs().max(1e-12))
    }

    #[test]
    fn matches_reference_port() {
        use TickCoverage::{Free, Loose};
        // Reference results, with whole-step starts and corrected zero membership.
        let cases: &[(f64, f64, usize, TickCoverage, &[f64])] = &[
            (0.0, 100.0, 5, Free, &[0.0, 25.0, 50.0, 75.0, 100.0]),
            (0.0, 100.0, 4, Free, &[0.0, 50.0, 100.0]),
            (-1.5, 1.5, 4, Free, &[-1.0, 0.0, 1.0]),
            (0.0, 30.0, 5, Free, &[0.0, 10.0, 20.0, 30.0]),
            (0.0, 25.0, 4, Free, &[0.0, 5.0, 10.0, 15.0, 20.0, 25.0]),
            (0.5, 9.5, 5, Free, &[0.0, 2.5, 5.0, 7.5, 10.0]),
            (3.2, 7.9, 5, Free, &[3.0, 4.0, 5.0, 6.0, 7.0, 8.0]),
            (0.0, 1.0, 5, Free, &[0.0, 0.25, 0.5, 0.75, 1.0]),
            (-37.0, 112.0, 5, Free, &[-40.0, 0.0, 40.0, 80.0, 120.0]),
            (0.001, 0.0027, 5, Free, &[0.001, 0.0015, 0.002, 0.0025]),
            (
                0.001,
                0.0027,
                5,
                Loose,
                &[0.001, 0.0015, 0.002, 0.0025, 0.003],
            ),
            (1200.0, 98000.0, 4, Free, &[0.0, 50000.0, 100000.0]),
            (0.0, 8.0, 4, Free, &[0.0, 2.5, 5.0, 7.5]),
            (0.0, 8.0, 4, Loose, &[0.0, 2.0, 4.0, 6.0, 8.0]),
            // Random ranges where `powi(10, z)` diverged from the Python reference.
            (
                -2.6178863563145905e68,
                2.301814660011723e68,
                2,
                Loose,
                &[-3e68, 0.0, 3e68],
            ),
            // Zero belongs to the grid with step 2e37 despite the non-zero f64 remainder.
            (
                -5.219454767136544e37,
                4.908864837878777e37,
                6,
                Loose,
                &[-6e37, -4e37, -2e37, 0.0, 2e37, 4e37, 6e37],
            ),
            // The same correction counts zero on the grid with step 2e100.
            (
                -1.161350009461411e101,
                2.0915500407958136e100,
                8,
                Free,
                &[-1.2e101, -1e101, -8e100, -6e100, -4e100, -2e100, 0.0, 2e100],
            ),
        ];
        for &(lo, hi, m, coverage, want) in cases {
            let got = extended(lo, hi, m, coverage, &Fit::ANY);
            assert!(
                close(&got, want),
                "extended({lo}, {hi}, {m}, {coverage:?}) = {got:?}, want {want:?}"
            );
        }
    }

    /// A fit that admits the values `slack` finds room for, judging every candidate.
    fn judged(slack: &dyn Fn(&[f64]) -> Room) -> Fit<'_> {
        Fit {
            least_share: 0.0,
            slack,
        }
    }

    /// The smallest gap between adjacent values, less `least`.
    fn gap_beyond(values: &[f64], least: f64) -> f64 {
        values
            .windows(2)
            .map(|w| w[1] - w[0] - least)
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn whole_step_search_caps_tick_count() {
        for (low, high) in [(11.0, 65.0), (59.9, 94.3), (-0.01, 5.69)] {
            for target in 2usize..=8 {
                for coverage in [TickCoverage::Loose, TickCoverage::Free] {
                    let values = extended(low, high, target, coverage, &Fit::ANY);
                    assert!(
                        values.len() <= (3 * target).div_ceil(2),
                        "target {target}: {values:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn crowded_whole_steps_choose_the_largest_slack() {
        for (coverage, want) in [
            (TickCoverage::Loose, [0.0, 20.0]),
            (TickCoverage::Free, [2.0, 4.0]),
        ] {
            let slack =
                |values: &[f64]| Room::Slack(if close(values, &want) { -1.0 } else { -10.0 });
            let values = extended(0.5, 9.5, 5, coverage, &judged(&slack));
            assert!(close(&values, &want), "{values:?}");
        }
    }

    #[test]
    fn a_fitting_whole_step_set_can_use_three_or_four_times_a_power_of_ten() {
        let slack = |values: &[f64]| {
            Room::Slack(if close(values, &[-3.0, 0.0, 3.0]) {
                1.0
            } else {
                -1.0
            })
        };
        let values = extended(-2.9, 2.9, 5, TickCoverage::Loose, &judged(&slack));
        assert!(close(&values, &[-3.0, 0.0, 3.0]), "{values:?}");
    }

    #[test]
    fn only_admitted_candidates_are_chosen() {
        use TickCoverage::{Free, Loose};
        // The cap allows at most five ticks for a target of three.
        let any = extended(-0.01, 5.69, 3, Loose, &Fit::ANY);
        assert!(close(&any, &[-2.0, 0.0, 2.0, 4.0, 6.0]), "{any:?}");
        // The same whole-step set also fits when ticks need to be at least 1.5 apart.
        let slack = |values: &[f64]| Room::Slack(gap_beyond(values, 1.5));
        let fit = judged(&slack);
        let loose = extended(-0.01, 5.69, 3, Loose, &fit);
        assert!(close(&loose, &[-2.0, 0.0, 2.0, 4.0, 6.0]), "{loose:?}");
        // The best scored free candidate, 0, 3, 6, is 3 apart and stays.
        let free = extended(-0.01, 5.69, 3, Free, &fit);
        assert!(close(&free, &[0.0, 3.0, 6.0]), "{free:?}");
        // Asked for 4 apart, it does not: a round step of 5.
        let slack = |values: &[f64]| Room::Slack(gap_beyond(values, 4.0));
        let free = extended(-0.01, 5.69, 3, Free, &judged(&slack));
        assert!(close(&free, &[0.0, 5.0]), "{free:?}");
        // A candidate the unjudged search prefers and that fits is still the choice.
        let slack = |values: &[f64]| Room::Slack(gap_beyond(values, 0.5));
        assert_eq!(extended(-0.01, 5.69, 3, Loose, &judged(&slack)), any);
    }

    #[test]
    fn the_least_share_rejects_without_judging() {
        // A step below 2 / 5.7 of the range, below 2, is rejected before the judge sees it.
        let judged = std::cell::Cell::new(f64::INFINITY);
        let slack = |values: &[f64]| {
            judged.set(judged.get().min(values[1] - values[0]));
            Room::Slack(f64::INFINITY)
        };
        let fit = Fit {
            least_share: 2.0 / 5.7,
            slack: &slack,
        };
        let values = extended(-0.01, 5.69, 3, TickCoverage::Loose, &fit);
        assert!(judged.get() >= 2.0 - 1e-9, "{}", judged.get());
        assert!(gap_beyond(&values, 2.0) >= -1e-9, "{values:?}");
    }

    /// Whether `values` are round: a step of 1, 2, 2.5 or 5 times a power of ten, and every
    /// value a whole multiple of it.
    fn is_round(values: &[f64]) -> bool {
        let step = values[1] - values[0];
        let mantissa = step / 10f64.powf(step.log10().floor());
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-6 * b.abs().max(1.0);
        [1.0, 2.0, 2.5, 5.0, 10.0]
            .iter()
            .any(|&m| near(mantissa, m))
            && values.iter().all(|v| near(v / step, (v / step).round()))
    }

    proptest::proptest! {
        #[test]
        fn rejected_choices_are_replaced_by_round_ones(
            low in -1e4f64..1e4,
            e in -3.0f64..5.0,
            target in 2usize..9,
            loose in proptest::prelude::any::<bool>(),
            least in 0.0f64..0.6,
        ) {
            let high = low + 10f64.powf(e);
            let coverage = if loose { TickCoverage::Loose } else { TickCoverage::Free };
            let least = least * (high - low);
            let slack = |values: &[f64]| gap_beyond(values, least);
            let room = |values: &[f64]| Room::Slack(slack(values));
            let best = extended(low, high, target, coverage, &Fit::ANY);
            let chosen = extended(low, high, target, coverage, &judged(&room));
            if slack(&best) >= 0.0 {
                proptest::prop_assert_eq!(chosen, best);
            } else if chosen.len() > 2 {
                // A round candidate that fits; only a two-tick fallback may be neither.
                proptest::prop_assert!(slack(&chosen) >= 0.0, "{:?}", chosen);
                proptest::prop_assert!(is_round(&chosen), "{:?} for {:?}", chosen, best);
            }
        }
    }

    #[test]
    fn no_fit_falls_back_to_the_widest_two_ticks() {
        use TickCoverage::{Free, Loose};
        let slack = |_: &[f64]| Room::Slack(-1.0);
        let none = judged(&slack);
        // Two loose ticks always span the whole axis: the best scored pair.
        assert!(close(&extended(0.5, 9.5, 5, Loose, &none), &[0.0, 10.0]));
        // Covering both signs on a whole-step grid needs zero and both outer ticks.
        assert!(close(
            &extended(-0.01, 5.69, 3, Loose, &none),
            &[-10.0, 0.0, 10.0]
        ));
        // Two free ticks inside the range, as far apart as any candidate the search scores.
        let free = extended(0.5, 9.5, 5, Free, &none);
        assert!(close(&free, &[4.0, 8.0]), "{free:?}");
        // Without a candidate: the two range ends.
        assert_eq!(extended(1.0, 1.0, 5, Free, &none), [1.0, 1.0]);
    }

    #[test]
    fn crowded_free_ticks_keep_a_whole_step_inside_the_range() {
        let none = judged(&|_| Room::Slack(-1.0));
        for (low, high) in [
            (1000.0, 3000.0),
            (9000.0, 84000.0),
            (-3000.0, -1000.0),
            (-1000.0, 3000.0),
        ] {
            let values = extended(low, high, 3, TickCoverage::Free, &none);
            assert_eq!(values.len(), 2, "{values:?}");
            let step = values[1] - values[0];
            for value in &values {
                assert!((low..=high).contains(value), "{values:?}");
                assert!(
                    (value / step - (value / step).round()).abs() < 1e-9,
                    "{values:?}"
                );
            }
            if low <= 0.0 && high >= 0.0 {
                assert!(values.contains(&0.0), "{values:?}");
            }
        }
    }

    #[test]
    fn given_large_fractional_ticks_keep_their_decimal_places() {
        let labels: Vec<Vec<String>> = [500000.4, 100000.05, 50000.04]
            .into_iter()
            .map(|value| format_given(&[0.0, value]))
            .collect();
        assert_eq!(
            labels,
            [
                vec!["0.0", "500000.4"],
                vec!["0.00", "100000.05"],
                vec!["0.00", "50000.04"],
            ]
        );
    }

    #[test]
    fn simplicity_does_not_count_zero_between_tiny_ticks() {
        // With j = 3 these bounds can straddle zero without a tick there.
        assert_eq!(simplicity(0, 3.0, -3e-12, 7e-12, 6e-12), -2.0);
    }

    #[test]
    fn simplicity_counts_zero_on_a_large_grid() {
        // The floating-point remainder of -1e20 by 1e20 / 3 is not zero.
        assert_eq!(simplicity(0, 3.0, -1e20, 2e20, 1e20 / 3.0), -1.0);
    }

    #[test]
    fn corrected_large_grid_choices_match_the_unit_ranges() {
        for (lo, hi, target, coverage, factor) in [
            (
                -5.219454767136544,
                4.908864837878777,
                6,
                TickCoverage::Loose,
                1e37,
            ),
            (
                -11.61350009461411,
                2.0915500407958136,
                8,
                TickCoverage::Free,
                1e100,
            ),
        ] {
            let unit = extended(lo, hi, target, coverage, &Fit::ANY);
            let scaled: Vec<f64> = extended(lo * factor, hi * factor, target, coverage, &Fit::ANY)
                .iter()
                .map(|v| v / factor)
                .collect();
            assert!(close(&scaled, &unit), "{scaled:?} vs {unit:?}");
        }
    }

    #[test]
    fn simplicity_is_scale_invariant_for_integer_grid_indices() {
        for factor in [1e-290, 1e-12, 1.0, 1e20, 1e299] {
            for j in [1, 3, 7] {
                for (qi, q) in Q.into_iter().enumerate() {
                    let jf = f64::from(j);
                    let step = jf * q * factor;
                    for start in -30..=0 {
                        let lo = f64::from(start) * (step / jf);
                        let hi = lo + 11.0 * step;
                        let has_zero = start % j == 0 && start >= -11 * j;
                        let want = 1.0 - qi as f64 / Q_LAST - jf + if has_zero { 1.0 } else { 0.0 };
                        assert_eq!(
                            simplicity(qi, jf, lo, hi, step),
                            want,
                            "factor {factor:e}, j {j}, q {q}, start {start}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn given_labels_keep_common_precision_and_trim_float_noise() {
        assert_eq!(format_given(&[0.0, 1.5, 3.25]), ["0.00", "1.50", "3.25"]);
        assert_eq!(format_given(&[0.0, 0.30000000000000004]), ["0.0", "0.3"]);
        assert_eq!(
            format_given(&[-500000.4, -0.0]),
            ["\u{2212}500000.4", "0.0"]
        );
        // Computed steps keep their existing tolerance, independent of explicit values.
        assert_eq!(format_ticks(&[0.0, 500000.4], 500000.4), ["0", "500000"]);
    }

    #[test]
    fn labels_are_clean_for_fractional_steps() {
        assert_eq!(
            format_ticks(&[0.0, 0.1, 0.2, 0.30000000000000004], 0.1),
            ["0.0", "0.1", "0.2", "0.3"]
        );
        assert_eq!(
            format_ticks(&[0.0, 0.25, 0.5], 0.25),
            ["0.00", "0.25", "0.50"]
        );
        assert_eq!(format_ticks(&[0.0, 2.5, 5.0], 2.5), ["0.0", "2.5", "5.0"]);
        assert_eq!(
            format_ticks(&[-40.0, 0.0, 40.0], 40.0),
            ["\u{2212}40", "0", "40"]
        );
    }

    #[test]
    fn zero_never_signed() {
        assert_eq!(format_ticks(&[-1e-17, 0.5], 0.5), ["0.0", "0.5"]);
        assert_eq!(format_ticks(&[-0.0], 1.0), ["0"]);
    }

    #[test]
    fn scientific_labels_keep_enough_digits() {
        assert_eq!(
            format_ticks(&[0.0, 2.5e6, 5e6], 2.5e6),
            ["0", "2.5e6", "5e6"]
        );
        assert_eq!(
            format_ticks(&[0.0, 1.5e-5, 3e-5], 1.5e-5),
            ["0", "1.5e\u{2212}5", "3e\u{2212}5"]
        );
    }

    #[test]
    fn colliding_labels_raise_precision() {
        let labels = format_ticks(&[1.0, 1.0 + 5e-13, 1.0 + 1e-12], 1e-12);
        assert_eq!(labels.len(), 3);
        assert!(labels[0] != labels[1] && labels[1] != labels[2]);
        let labels = format_ticks(&[1e9, 1e9 + 2.5e-4], 2.5e-4);
        assert!(labels[0] != labels[1], "{labels:?}");
    }

    #[test]
    fn value_policy_uses_one_form_per_axis() {
        // Any value outside [1e-4, 1e6) puts the whole set in scientific form.
        assert_eq!(
            format_values(&[0.0, 3.375, -0.001, 1e6, 2.5e-5, -3e-5]),
            [
                "0",
                "3.375e0",
                "\u{2212}1e\u{2212}3",
                "1e6",
                "2.5e\u{2212}5",
                "\u{2212}3e\u{2212}5"
            ]
        );
        assert_eq!(
            format_values(&[0.0, 3.375, -0.001, 999999.0]),
            ["0", "3.375", "\u{2212}0.001", "999999"]
        );
    }

    #[test]
    fn value_policy_trims_float_noise_unless_labels_collide() {
        assert_eq!(format_values(&[1.1, 1.1 * 1.1]), ["1.1", "1.21"]);
        // Distinct at 17 digits but not at 12.
        let labels = format_values(&[1.0, 1.0 + f64::EPSILON]);
        assert_eq!(labels, ["1", "1.0000000000000002"]);
    }

    #[test]
    fn huge_and_tiny_values_use_scientific_form() {
        assert_eq!(format_ticks(&[0.0, 5e6, 1e7], 5e6), ["0", "5e6", "1e7"]);
        assert_eq!(
            format_ticks(&[-3e-5, 0.0, 3e-5], 3e-5),
            ["\u{2212}3e\u{2212}5", "0", "3e\u{2212}5"]
        );
    }
}
