//! Binning: the validated bin count, the histogram of a set of values (its range and the
//! count in each bin) and the rule that assigns each value to a bin.

use std::num::NonZeroUsize;

use crate::scale::{MAX_MAGNITUDE, too_narrow};

/// The largest number of bins a histogram may have; more is an `Error::Style` from
/// [`AxesMut::hist`](crate::AxesMut::hist).
///
/// # Examples
///
/// ```
/// use umber_core::figure::HistOptions;
/// use umber_core::{Figure, MAX_BINS, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let max = MAX_BINS as i64;
/// assert!(axes.hist(&[1.0], HistOptions { bins: max, ..HistOptions::default() }).is_ok());
/// assert!(axes.hist(&[1.0], HistOptions { bins: max + 1, ..HistOptions::default() }).is_err());
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

/// Equal-width bins over a range, and the number of values in each.
///
/// Bins are half-open, `[edge(i), edge(i + 1))`, except the last, which also holds `hi`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Histogram {
    lo: f64,
    hi: f64,
    counts: Box<[u64]>,
}

impl Histogram {
    /// Bins the finite `values` into `bins` equal-width bins from their minimum to their
    /// maximum. NaN and infinite values are ignored. Constant data, and data narrower than
    /// the minimum width of a scale domain, get the range `v ± max(0.5, |v| × 1e-9)` around
    /// their midpoint `v`, clamped to `±MAX_MAGNITUDE`.
    ///
    /// Returns `None` when there is no finite value.
    pub(crate) fn from_values(values: &[f64], bins: BinCount) -> Option<Histogram> {
        let (min, max) = values.iter().copied().filter(|v| v.is_finite()).fold(
            None,
            |range: Option<(f64, f64)>, v| match range {
                None => Some((v, v)),
                Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
            },
        )?;
        let (lo, hi) = bin_range(min, max);
        let mut histogram = Histogram {
            lo,
            hi,
            counts: vec![0; bins.get()].into_boxed_slice(),
        };
        for v in values.iter().copied().filter(|v| v.is_finite()) {
            let index = histogram.index_of(v);
            if let Some(count) = histogram.counts.get_mut(index) {
                *count += 1;
            }
        }
        Some(histogram)
    }

    /// The number of bins.
    fn bins(&self) -> usize {
        self.counts.len()
    }

    /// The width of one bin.
    fn width(&self) -> f64 {
        (self.hi - self.lo) / self.bins() as f64
    }

    /// The lower edge of bin `i`. `edge(0)` is `lo` and `edge(n)` is `hi`, exactly; an index
    /// beyond `n` saturates to `hi`.
    pub(crate) fn edge(&self, i: usize) -> f64 {
        if i >= self.bins() {
            self.hi
        } else {
            self.lo + i as f64 * self.width()
        }
    }

    /// The binned range: the low edge of the first bin and the high edge of the last.
    pub(crate) fn range(&self) -> (f64, f64) {
        (self.lo, self.hi)
    }

    /// The count in each bin, lowest first.
    pub(crate) fn counts(&self) -> &[u64] {
        &self.counts
    }

    /// The largest count in any bin.
    pub(crate) fn max_count(&self) -> u64 {
        self.counts.iter().copied().max().unwrap_or(0)
    }

    /// The bin that holds `v`, a value within `lo..=hi`: the estimate from the bin width,
    /// corrected against the actual edges, which rounding may place off by one.
    fn index_of(&self, v: f64) -> usize {
        let last = self.bins() - 1;
        let width = self.width();
        // A float-to-integer cast saturates, so a value outside the estimate is still in range.
        let mut index = (((v - self.lo) / width) as usize).min(last);
        while index > 0 && v < self.edge(index) {
            index -= 1;
        }
        while index < last && v >= self.edge(index + 1) {
            index += 1;
        }
        index
    }
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
