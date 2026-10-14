//! Distribution summaries: type-7 quantiles and Tukey whiskers, computed in data space.

/// The type-7 quantile of non-empty, sorted finite values at `p` in `[0, 1]`.
/// Interpolation starts from the nearer endpoint, as NumPy's linear percentile does.
pub(crate) fn quantile_type7(sorted: &[f64], p: f64) -> f64 {
    let index = (sorted.len().saturating_sub(1)) as f64 * p;
    let lower = index.floor() as usize;
    let Some(&a) = sorted.get(lower) else {
        return f64::NAN;
    };
    let b = sorted.get(lower + 1).copied().unwrap_or(a);
    let fraction = index - lower as f64;
    let difference = b - a;
    if fraction >= 0.5 {
        b - difference * (1.0 - fraction)
    } else {
        a + difference * fraction
    }
}

/// The quartiles, Tukey whiskers and outliers of a non-empty group.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BoxStats {
    /// The lower quartile.
    pub q1: f64,
    /// The median.
    pub median: f64,
    /// The upper quartile.
    pub q3: f64,
    /// The smallest value inside the fences.
    pub low_whisker: f64,
    /// The largest value inside the fences.
    pub high_whisker: f64,
    /// Values outside the fences, in sorted order, including repeats.
    pub outliers: Vec<f64>,
}

/// Drops NaN, sorts a copy, and computes type-7 quartiles and 1.5-IQR Tukey whiskers.
/// Values have already passed model validation; infinities are not accepted there.
/// Empty and all-NaN groups have no summary.
pub(crate) fn box_stats(values: &[f64]) -> Option<BoxStats> {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| !v.is_nan()).collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_unstable_by(f64::total_cmp);
    let q1 = quantile_type7(&sorted, 0.25);
    let median = quantile_type7(&sorted, 0.5);
    let q3 = quantile_type7(&sorted, 0.75);
    let reach = 1.5 * (q3 - q1);
    let first = sorted.partition_point(|&v| v < q1 - reach);
    let after = sorted.partition_point(|&v| v <= q3 + reach);
    Some(BoxStats {
        q1,
        median,
        q3,
        low_whisker: sorted[first],
        high_whisker: sorted[after - 1],
        outliers: sorted[..first]
            .iter()
            .chain(&sorted[after..])
            .copied()
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whiskers_and_outliers_tukey() {
        let stats = box_stats(&[100.0, 0.0, 1.0, f64::NAN, 2.0, 3.0, 4.0, -100.0]).unwrap();
        assert_eq!((stats.q1, stats.median, stats.q3), (0.5, 2.0, 3.5));
        assert_eq!((stats.low_whisker, stats.high_whisker), (0.0, 4.0));
        assert_eq!(stats.outliers, [-100.0, 100.0]);
        // The fences themselves are inside, not outliers.
        let stats = box_stats(&[-6.0, 0.0, 0.0, 2.0, 4.0, 4.0, 10.0]).unwrap();
        assert_eq!((stats.low_whisker, stats.high_whisker), (-6.0, 10.0));
        assert!(stats.outliers.is_empty());
    }

    #[test]
    fn empty_and_constant_groups() {
        assert_eq!(box_stats(&[]), None);
        assert_eq!(box_stats(&[f64::NAN]), None);
        for values in [vec![3.0], vec![3.0, 3.0]] {
            let stats = box_stats(&values).unwrap();
            assert_eq!((stats.q1, stats.median, stats.q3), (3.0, 3.0, 3.0));
            assert_eq!((stats.low_whisker, stats.high_whisker), (3.0, 3.0));
            assert!(stats.outliers.is_empty());
        }
    }

    #[test]
    fn quantiles_use_linear_interpolation() {
        let values = [1.0, 2.0, 4.0, 8.0];
        assert_eq!(quantile_type7(&values, 0.0), 1.0);
        assert_eq!(quantile_type7(&values, 0.25), 1.75);
        assert_eq!(quantile_type7(&values, 0.5), 3.0);
        assert_eq!(quantile_type7(&values, 0.75), 5.0);
        assert_eq!(quantile_type7(&values, 1.0), 8.0);
    }
}
