//! Binning: the validated bins of a histogram (a count of equal-width bins or explicit edges),
//! the rule that assigns each value to a bin, the count in each bin, and the bar heights that
//! a histogram's statistic and style make of the counts of one or more groups.

use std::num::NonZeroUsize;
use std::sync::Arc;

use crate::model::{HistStat, HistStyle};
use crate::scale::{MAX_MAGNITUDE, too_narrow};

/// The largest number of bins a histogram may have; more is an `Error::Style` from
/// [`AxesMut::hist`](crate::AxesMut::hist).
///
/// # Examples
///
/// ```
/// use umber_core::figure::HistOptions;
/// use umber_core::{Bins, Figure, MAX_BINS, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let max = MAX_BINS as i64;
/// let bins = |n| HistOptions { bins: Bins::Count(n), ..HistOptions::default() };
/// assert!(axes.hist(&[1.0], bins(max)).is_ok());
/// assert!(axes.hist(&[1.0], bins(max + 1)).is_err());
/// # Ok::<(), umber_core::Error>(())
/// ```
pub const MAX_BINS: usize = 10_000;

/// Constant data `v` is spread over `v ± max(HALF_RANGE, |v| × RELATIVE_HALF_RANGE)`.
const HALF_RANGE: f64 = 0.5;

/// The relative part of the half range of constant data. Unlike `HALF_RANGE` alone, it stays
/// visible next to a value of magnitude 1e300.
const RELATIVE_HALF_RANGE: f64 = 1e-9;

/// A number of bins, within `1..=MAX_BINS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BinCount(NonZeroUsize);

impl BinCount {
    /// The count `n`, or `None` when `n` is below 1 or above [`MAX_BINS`].
    pub(crate) fn new(n: i64) -> Option<BinCount> {
        let n = usize::try_from(n).ok().filter(|&n| n <= MAX_BINS)?;
        NonZeroUsize::new(n).map(BinCount)
    }

    /// The number of bins.
    pub(crate) fn get(self) -> usize {
        self.0.get()
    }
}

/// The validated bins of a histogram: a number of equal-width bins over the range of the
/// values, or explicit edges.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BinSpec {
    /// Equal-width bins from the smallest to the largest finite value.
    Count(BinCount),
    /// At least two finite, strictly increasing edges, within the supported magnitudes and wide
    /// enough for a linear axis; clones share them.
    Edges(Arc<[f64]>),
}

/// How a histogram turns values into bars: its validated bins, its style, whether it
/// accumulates, and its statistic. Accumulating densities is refused before one is made.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistBinning {
    bins: BinSpec,
    style: HistStyle,
    cumulative: bool,
    stat: HistStat,
}

impl HistBinning {
    /// The binning of `bins` drawn in `style`, accumulated when `cumulative`, of `stat`; the
    /// model has validated each part.
    pub(crate) fn new(
        bins: BinSpec,
        style: HistStyle,
        cumulative: bool,
        stat: HistStat,
    ) -> HistBinning {
        HistBinning {
            bins,
            style,
            cumulative,
            stat,
        }
    }

    /// The bins.
    pub(crate) fn bins(&self) -> &BinSpec {
        &self.bins
    }

    /// The style.
    pub(crate) fn style(&self) -> HistStyle {
        self.style
    }

    /// Whether the bars accumulate from left to right.
    pub(crate) fn cumulative(&self) -> bool {
        self.cumulative
    }

    /// The statistic of each bar.
    pub(crate) fn stat(&self) -> HistStat {
        self.stat
    }
}

/// The edges of a histogram's bins: equal-width bins over a range, or the explicit edges.
///
/// Bins are half-open, `[edge(i), edge(i + 1))`, except the last, which also holds its high
/// edge.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Edges {
    /// `bins` bins of equal width from `lo` to `hi`.
    Equal {
        /// The low edge of the first bin.
        lo: f64,
        /// The high edge of the last bin.
        hi: f64,
        /// The number of bins, at least 1.
        bins: usize,
    },
    /// Explicit, strictly increasing edges, at least two.
    Given(Arc<[f64]>),
}

impl Edges {
    /// The edges of `spec` for the finite values among `values`: explicit edges as given, and
    /// equal-width bins from the smallest to the largest finite value. Constant data, and data
    /// narrower than the minimum width of a scale domain, get the range `v ± max(0.5, |v| ×
    /// 1e-9)` around their midpoint `v`, clamped to `±MAX_MAGNITUDE`. `None` for equal-width
    /// bins without a finite value.
    fn of(spec: &BinSpec, values: impl Iterator<Item = f64>) -> Option<Edges> {
        match spec {
            BinSpec::Edges(edges) => Some(Edges::Given(Arc::clone(edges))),
            BinSpec::Count(count) => {
                let (min, max) = values.filter(|v| v.is_finite()).fold(
                    None,
                    |range: Option<(f64, f64)>, v| match range {
                        None => Some((v, v)),
                        Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
                    },
                )?;
                let (lo, hi) = bin_range(min, max);
                Some(Edges::Equal {
                    lo,
                    hi,
                    bins: count.get(),
                })
            }
        }
    }

    /// The number of bins.
    fn bins(&self) -> usize {
        match self {
            Edges::Equal { bins, .. } => *bins,
            Edges::Given(edges) => edges.len() - 1,
        }
    }

    /// The lower edge of bin `i`. `edge(0)` is the low end and `edge(n)` the high end, exactly;
    /// an index beyond `n` saturates to the high end.
    fn edge(&self, i: usize) -> f64 {
        match self {
            Edges::Equal { lo, hi, bins } => {
                if i >= *bins {
                    *hi
                } else {
                    lo + i as f64 * ((hi - lo) / *bins as f64)
                }
            }
            Edges::Given(edges) => edges[i.min(edges.len() - 1)],
        }
    }

    /// The bin that holds `v`, or `None` when `v` lies outside the edges (or is not finite).
    fn index_of(&self, v: f64) -> Option<usize> {
        let last = self.bins() - 1;
        if !(self.edge(0) <= v && v <= self.edge(last + 1)) {
            return None;
        }
        match self {
            Edges::Equal { lo, hi, bins } => {
                // The estimate from the bin width, corrected against the actual edges, which
                // rounding may place off by one. A float-to-integer cast saturates, so a value
                // outside the estimate is still in range.
                let width = (hi - lo) / *bins as f64;
                let mut index = (((v - lo) / width) as usize).min(last);
                while index > 0 && v < self.edge(index) {
                    index -= 1;
                }
                while index < last && v >= self.edge(index + 1) {
                    index += 1;
                }
                Some(index)
            }
            // The number of edges at or below `v`, less one, is the bin of a value inside.
            Edges::Given(edges) => Some((edges.partition_point(|&e| e <= v) - 1).min(last)),
        }
    }

    /// The count of the finite `values` in each bin, lowest first; values outside the edges
    /// are not counted.
    fn count(&self, values: &[f64]) -> Box<[u64]> {
        let mut counts = vec![0; self.bins()].into_boxed_slice();
        for v in values.iter().copied().filter(|v| v.is_finite()) {
            if let Some(count) = self.index_of(v).and_then(|i| counts.get_mut(i)) {
                *count += 1;
            }
        }
        counts
    }
}

/// Equal-width bins over the range of a set of values, and the number of values in each.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Histogram {
    edges: Edges,
    counts: Box<[u64]>,
}

#[cfg(test)]
impl Histogram {
    /// Bins the finite `values` into `bins` equal-width bins from their minimum to their
    /// maximum, as [`Edges::of`] places them. NaN and infinite values are ignored. Returns
    /// `None` when there is no finite value.
    pub(crate) fn from_values(values: &[f64], bins: BinCount) -> Option<Histogram> {
        let edges = Edges::of(&BinSpec::Count(bins), values.iter().copied())?;
        Some(Histogram {
            counts: edges.count(values),
            edges,
        })
    }

    /// The lower edge of bin `i`; see [`Edges::edge`].
    pub(crate) fn edge(&self, i: usize) -> f64 {
        self.edges.edge(i)
    }

    /// The count in each bin, lowest first.
    pub(crate) fn counts(&self) -> &[u64] {
        &self.counts
    }

    /// The largest count in any bin.
    pub(crate) fn max_count(&self) -> u64 {
        self.counts.iter().copied().max().unwrap_or(0)
    }
}

/// The bins of a histogram of one or more groups, which share them, and the height of each
/// group's bar in each bin, as its statistic and style make it.
///
/// A count is the number of values in the bin; a fraction is the count over the number of
/// finite values, and a density that fraction over the bin's width. [`HistStyle::Stacked`]
/// takes the number of finite values of all groups together, so the stack of every bin shows
/// the whole; the other styles take each group's own. Cumulative heights add up the counts
/// from the left before dividing, so a cumulative fraction ends at exactly 1. A group without
/// a finite value has heights of 0.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistBins {
    edges: Edges,
    heights: Box<[Box<[f64]>]>,
}

impl HistBins {
    /// The bins of `binning` for the `groups`, in order, and their heights. `None` for
    /// equal-width bins when no group has a finite value. Explicit edges hold every finite
    /// value, as the model checked.
    pub(crate) fn new(binning: &HistBinning, groups: &[&[f64]]) -> Option<HistBins> {
        let all = groups.iter().flat_map(|values| values.iter().copied());
        let edges = Edges::of(binning.bins(), all)?;
        let counts: Vec<Box<[u64]>> = groups.iter().map(|values| edges.count(values)).collect();
        let stacked_total: u64 = counts.iter().flat_map(|c| c.iter()).sum();
        let heights = counts
            .iter()
            .map(|counts| {
                let total = if binning.style() == HistStyle::Stacked {
                    stacked_total
                } else {
                    counts.iter().sum()
                };
                bar_heights(&edges, counts, total, binning)
            })
            .collect();
        Some(HistBins { edges, heights })
    }

    /// The number of bins.
    pub(crate) fn bins(&self) -> usize {
        self.edges.bins()
    }

    /// The lower edge of bin `i`; `edge(bins())` is the high edge of the last bin.
    pub(crate) fn edge(&self, i: usize) -> f64 {
        self.edges.edge(i)
    }

    /// The binned range: the low edge of the first bin and the high edge of the last.
    pub(crate) fn range(&self) -> (f64, f64) {
        (self.edges.edge(0), self.edges.edge(self.bins()))
    }

    /// The number of groups.
    pub(crate) fn groups(&self) -> usize {
        self.heights.len()
    }

    /// The height of group `g`'s bar in each bin, lowest bin first.
    pub(crate) fn heights(&self, g: usize) -> &[f64] {
        &self.heights[g]
    }

    /// The top of the stack of every bin, lowest bin first: the sum of the groups' heights, in
    /// group order.
    pub(crate) fn tops(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.bins()).map(|i| self.heights.iter().map(|h| h[i]).sum())
    }
}

/// The heights of the bars of one group with `counts` in the bins of `edges`, as `binning`
/// says, `total` finite values making the whole.
fn bar_heights(edges: &Edges, counts: &[u64], total: u64, binning: &HistBinning) -> Box<[f64]> {
    let mut running = 0;
    counts
        .iter()
        .enumerate()
        .map(|(i, &count)| {
            let count = if binning.cumulative() {
                running += count;
                running
            } else {
                count
            };
            match binning.stat() {
                HistStat::Count => count as f64,
                _ if total == 0 => 0.0,
                HistStat::Fraction => count as f64 / total as f64,
                HistStat::Density => {
                    count as f64 / total as f64 / (edges.edge(i + 1) - edges.edge(i))
                }
            }
        })
        .collect()
}

/// The range binned for data spanning `min..=max` (both finite, `min <= max`). Data narrower
/// than the minimum width of a linear scale domain is treated as constant.
fn bin_range(min: f64, max: f64) -> (f64, f64) {
    if !too_narrow(min, max) {
        return (min, max);
    }
    let center = min / 2.0 + max / 2.0;
    let half = HALF_RANGE.max(center.abs() * RELATIVE_HALF_RANGE);
    (
        (center - half).max(-MAX_MAGNITUDE),
        (center + half).min(MAX_MAGNITUDE),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn bins(n: usize) -> BinCount {
        BinCount::new(i64::try_from(n).unwrap()).unwrap()
    }

    #[test]
    fn bin_count_is_bounded() {
        let max = i64::try_from(MAX_BINS).unwrap();
        assert!(BinCount::new(i64::MIN).is_none());
        assert!(BinCount::new(-1).is_none());
        assert!(BinCount::new(0).is_none());
        assert_eq!(BinCount::new(1).map(BinCount::get), Some(1));
        assert_eq!(BinCount::new(max).map(BinCount::get), Some(MAX_BINS));
        assert!(BinCount::new(max + 1).is_none());
        assert!(BinCount::new(i64::MAX).is_none());
    }

    #[test]
    fn no_finite_value_gives_no_histogram() {
        assert!(Histogram::from_values(&[], bins(3)).is_none());
        assert!(Histogram::from_values(&[f64::NAN, f64::INFINITY], bins(3)).is_none());
    }

    #[test]
    fn last_bin_holds_the_maximum() {
        let h = Histogram::from_values(&[0.0, 1.0, 2.0, 3.0, 4.0], bins(4)).unwrap();
        assert_eq!(h.counts(), &[1, 1, 1, 2]);
        assert_eq!(h.edge(0), 0.0);
        assert_eq!(h.edge(4), 4.0);
        assert_eq!(h.max_count(), 2);
    }

    #[test]
    fn ties_go_to_the_upper_bin() {
        let h = Histogram::from_values(&[0.0, 2.0, 4.0], bins(2)).unwrap();
        assert_eq!(h.counts(), &[1, 2]);
    }

    #[test]
    fn nan_is_ignored() {
        let h = Histogram::from_values(&[f64::NAN, 1.0, 2.0], bins(1)).unwrap();
        assert_eq!(h.counts(), &[2]);
    }

    #[test]
    fn constant_data_gets_one_centered_range() {
        let h = Histogram::from_values(&[3.0, 3.0, 3.0], bins(2)).unwrap();
        assert_eq!((h.edge(0), h.edge(2)), (2.5, 3.5));
        assert_eq!(h.counts().iter().sum::<u64>(), 3);

        let h = Histogram::from_values(&[1e200], bins(1)).unwrap();
        assert_eq!((h.edge(0), h.edge(1)), (1e200 - 1e191, 1e200 + 1e191));

        // Narrower than a scale domain may be: treated as constant.
        let h = Histogram::from_values(&[1.0, 1.0 + 1e-14], bins(2)).unwrap();
        assert!(h.edge(2) - h.edge(0) > 0.9);
        assert_eq!(h.counts().iter().sum::<u64>(), 2);
    }

    #[test]
    fn constant_data_at_the_limit_stays_in_range() {
        let h = Histogram::from_values(&[MAX_MAGNITUDE], bins(3)).unwrap();
        assert_eq!(h.edge(3), MAX_MAGNITUDE);
        assert!(h.edge(0) < h.edge(3));
        assert_eq!(h.counts().iter().sum::<u64>(), 1);
    }

    fn binning(style: HistStyle, cumulative: bool, stat: HistStat) -> HistBinning {
        HistBinning::new(BinSpec::Count(bins(4)), style, cumulative, stat)
    }

    fn edges(values: &[f64]) -> BinSpec {
        BinSpec::Edges(Arc::from(values))
    }

    #[test]
    fn explicit_edges_count_as_numpy_does() {
        // Half-open bins, the last closed: ties go to the bin above, the last edge to the last
        // bin, as numpy.histogram counts.
        let spec = HistBinning::new(
            edges(&[0.0, 1.0, 2.5, 4.0, 10.0]),
            HistStyle::Bars,
            false,
            HistStat::Count,
        );
        let values = [0.0, 0.5, 1.0, 1.0, 2.4, 2.5, 3.9, 4.0, 7.5, 10.0, f64::NAN];
        let bins = HistBins::new(&spec, &[&values]).unwrap();
        assert_eq!(bins.heights(0), [2.0, 3.0, 2.0, 3.0]);
        assert_eq!((bins.edge(0), bins.edge(4)), (0.0, 10.0));
        assert_eq!(bins.range(), (0.0, 10.0));
        // Explicit edges are kept even without a finite value to count.
        let empty = HistBins::new(&spec, &[&[f64::NAN]]).unwrap();
        assert_eq!(empty.heights(0), [0.0; 4]);
    }

    #[test]
    fn equal_width_counts_are_the_plain_histogram() {
        let values = [0.0, 1.0, 2.0, 3.0, 4.0];
        let spec = binning(HistStyle::Bars, false, HistStat::Count);
        let binned = HistBins::new(&spec, &[&values]).unwrap();
        let plain = Histogram::from_values(&values, bins(4)).unwrap();
        let counts: Vec<f64> = plain.counts().iter().map(|&c| c as f64).collect();
        assert_eq!(binned.heights(0), counts.as_slice());
        for i in 0..=4 {
            assert_eq!(binned.edge(i).to_bits(), plain.edge(i).to_bits());
        }
        assert!(HistBins::new(&spec, &[&[]]).is_none());
    }

    #[test]
    fn cumulative_fraction_ends_at_one() {
        let values = [0.1, 0.2, 0.7, 1.9, 2.0, 2.0, 3.3];
        for style in [HistStyle::Bars, HistStyle::Step] {
            let spec = binning(style, true, HistStat::Fraction);
            let binned = HistBins::new(&spec, &[&values]).unwrap();
            let heights = binned.heights(0);
            assert_eq!(*heights.last().unwrap(), 1.0, "{heights:?}");
            assert!(heights.windows(2).all(|pair| pair[0] <= pair[1]));
        }
        // Counts accumulate to the total.
        let spec = binning(HistStyle::Bars, true, HistStat::Count);
        let binned = HistBins::new(&spec, &[&values]).unwrap();
        assert_eq!(*binned.heights(0).last().unwrap(), 7.0);
        // Stacked groups share one total, so the cumulative stack ends at one.
        let spec = binning(HistStyle::Stacked, true, HistStat::Fraction);
        let binned = HistBins::new(&spec, &[&values[..3], &values[3..]]).unwrap();
        let top: f64 = (0..2).map(|g| *binned.heights(g).last().unwrap()).sum();
        assert!((top - 1.0).abs() < 1e-15, "{top}");
    }

    /// The area under the bars of group `g`: each height times its bin's width.
    fn area(binned: &HistBins, g: usize) -> f64 {
        binned
            .heights(g)
            .iter()
            .enumerate()
            .map(|(i, h)| h * (binned.edge(i + 1) - binned.edge(i)))
            .sum()
    }

    #[test]
    fn density_area_is_one() {
        let values = [0.0, 0.5, 1.0, 1.0, 2.4, 2.5, 3.9, 4.0, 7.5, 10.0];
        let uneven = HistBinning::new(
            edges(&[0.0, 1.0, 2.5, 4.0, 10.0]),
            HistStyle::Step,
            false,
            HistStat::Density,
        );
        let binned = HistBins::new(&uneven, &[&values]).unwrap();
        assert!((area(&binned, 0) - 1.0).abs() < 1e-12);
        let spec = binning(HistStyle::Bars, false, HistStat::Density);
        let binned = HistBins::new(&spec, &[&values]).unwrap();
        assert!((area(&binned, 0) - 1.0).abs() < 1e-12);
        // Step groups each have area one; stacked groups have area one together.
        let groups: [&[f64]; 2] = [&values[..2], &values[2..]];
        let spec = binning(HistStyle::Step, false, HistStat::Density);
        let step = HistBins::new(&spec, &groups).unwrap();
        assert!((area(&step, 0) - 1.0).abs() < 1e-12);
        assert!((area(&step, 1) - 1.0).abs() < 1e-12);
        let spec = binning(HistStyle::Stacked, false, HistStat::Density);
        let stacked = HistBins::new(&spec, &groups).unwrap();
        assert!((area(&stacked, 0) + area(&stacked, 1) - 1.0).abs() < 1e-12);
        assert!((area(&stacked, 0) - 0.2).abs() < 1e-12);
    }

    #[test]
    fn fractions_normalize_by_the_style() {
        let groups: [&[f64]; 2] = [&[0.0, 1.0, 1.0, 4.0], &[2.0, f64::NAN]];
        let spec = binning(HistStyle::Step, false, HistStat::Fraction);
        let step = HistBins::new(&spec, &groups).unwrap();
        assert_eq!(step.heights(0).iter().sum::<f64>(), 1.0);
        assert_eq!(step.heights(1), [0.0, 0.0, 1.0, 0.0]);
        let spec = binning(HistStyle::Stacked, false, HistStat::Fraction);
        let stacked = HistBins::new(&spec, &groups).unwrap();
        assert_eq!(stacked.heights(1), [0.0, 0.0, 0.2, 0.0]);
        // The stack tops of every bin, for the domain.
        let tops: Vec<f64> = stacked.tops().collect();
        assert_eq!(tops, [0.2, 0.4, 0.2, 0.2]);
        // A group without finite values has nothing to normalize, and its heights are 0.
        let spec = binning(HistStyle::Step, false, HistStat::Fraction);
        let none = HistBins::new(&spec, &[&[1.0, 2.0], &[f64::NAN]]).unwrap();
        assert_eq!(none.heights(1), [0.0; 4]);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        #[test]
        fn values_land_between_their_edges(
            values in prop::collection::vec(
                prop_oneof![
                    -MAX_MAGNITUDE..=MAX_MAGNITUDE,
                    -1e6_f64..1e6,
                    Just(0.0),
                    Just(MAX_MAGNITUDE),
                    Just(-MAX_MAGNITUDE),
                ],
                1..60,
            ),
            n in prop_oneof![1_usize..=20, Just(1), Just(MAX_BINS)],
        ) {
            let h = Histogram::from_values(&values, bins(n)).unwrap();
            prop_assert_eq!(h.counts().len(), n);
            prop_assert_eq!(h.counts().iter().sum::<u64>(), values.len() as u64);
            prop_assert_eq!(h.edge(n), h.edge(n + 1));
            for i in 0..n {
                prop_assert!(h.edge(i) <= h.edge(i + 1));
            }
            // Each value lies in a bin whose edges contain it: count the values that the
            // edges place in each bin and compare with the counts.
            let mut expected = vec![0_u64; n];
            for &v in &values {
                let holders: Vec<usize> = (0..n)
                    .filter(|&i| {
                        h.edge(i) <= v && (v < h.edge(i + 1) || (i == n - 1 && v <= h.edge(n)))
                    })
                    .collect();
                prop_assert!(!holders.is_empty(), "{} lies in no bin", v);
                // Equal edges make empty bins, so a value has exactly one holder.
                prop_assert_eq!(holders.len(), 1);
                expected[holders[0]] += 1;
            }
            prop_assert_eq!(h.counts(), &expected[..]);
        }
    }
}
