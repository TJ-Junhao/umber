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
    /// The smaller of q1 and the smallest sample at or above the lower fence.
    pub low_whisker: f64,
    /// The larger of q3 and the largest sample at or below the upper fence.
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
        low_whisker: sorted[first].min(q1),
        high_whisker: sorted[after - 1].max(q3),
        outliers: sorted[..first]
            .iter()
            .chain(&sorted[after..])
            .copied()
            .collect(),
    })
}

/// Relative Gaussian kernel density with Scott's sample-standard-deviation bandwidth.
/// Densities are measured on the unit data range, without division by the original span.
/// Finite samples alone contribute; fewer than two distinct values or grid points yield none.
/// Scaling the data range to one keeps the variance finite for very large or small samples.
pub(crate) fn kde_scott(values: &[f64], grid: usize) -> Option<Vec<(f64, f64)>> {
    let samples: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    let low = samples.iter().copied().reduce(f64::min)?;
    let high = samples.iter().copied().reduce(f64::max)?;
    if low == high || grid < 2 {
        return None;
    }
    let span = high - low;
    let scaled: Vec<f64> = samples.iter().map(|v| (v - low) / span).collect();
    let n = scaled.len() as f64;
    let mean = scaled.iter().sum::<f64>() / n;
    let variance = scaled.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let h = variance.sqrt() * n.powf(-0.2);
    let normalizer = n * h * (2.0 * std::f64::consts::PI).sqrt();
    let step = span / (grid - 1) as f64;
    Some(
        (0..grid)
            .map(|i| {
                let at = if i + 1 == grid {
                    high
                } else if step == 0.0 {
                    low + span * (i as f64 / (grid - 1) as f64)
                } else {
                    low + step * i as f64
                };
                // Evaluate at the returned grid position, including its rounding at large offsets.
                let fraction = (at - low) / span;
                let density = scaled
                    .iter()
                    .map(|v| {
                        let z = (fraction - v) / h;
                        (-0.5 * z * z).exp()
                    })
                    .sum::<f64>()
                    / normalizer;
                (at, density)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kde_subnormal_span_has_finite_densities_at_returned_positions() {
        let density = kde_scott(&[0.0, 0.0, f64::from_bits(1)], 128).unwrap();
        assert!(density.iter().all(|p| p.1.is_finite()), "{density:?}");
        for pair in density.windows(2) {
            if pair[0].0 == pair[1].0 {
                assert_eq!(pair[0].1, pair[1].1);
            }
        }
    }

    #[test]
    fn kde_large_span_preserves_relative_tail_densities() {
        let mut values = vec![0.0; 256];
        values.extend([-1.0, 1.0]);
        let base = kde_scott(&values, 128).unwrap();
        let large = kde_scott(&values.iter().map(|v| v * 1e300).collect::<Vec<_>>(), 128).unwrap();
        let base_peak = base.iter().map(|p| p.1).fold(0.0, f64::max);
        let large_peak = large.iter().map(|p| p.1).fold(0.0, f64::max);
        for (a, b) in base.iter().zip(&large) {
            let expected = a.1 / base_peak;
            let actual = b.1 / large_peak;
            assert!(expected > 0.0);
            let error = (actual / expected - 1.0).abs();
            assert!(
                error < 1e-9,
                "at {}: {actual:e} vs {expected:e}, relative error {error}",
                b.0
            );
        }
    }

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
    fn whiskers_are_clamped_to_quartiles() {
        for (values, q1, median, q3, outlier) in [
            ([0.0, 100.0, 100.0, 100.0], 75.0, 100.0, 100.0, 0.0),
            ([0.0, 0.0, 0.0, 100.0], 0.0, 0.0, 25.0, 100.0),
            ([0.0, 90.0, 100.0, 100.0], 67.5, 95.0, 100.0, 0.0),
            ([0.0, 0.0, 10.0, 100.0], 0.0, 5.0, 32.5, 100.0),
        ] {
            let stats = box_stats(&values).unwrap();
            assert_eq!((stats.q1, stats.median, stats.q3), (q1, median, q3));
            assert_eq!((stats.low_whisker, stats.high_whisker), (q1, q3));
            assert_eq!(stats.outliers, [outlier]);
        }
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
