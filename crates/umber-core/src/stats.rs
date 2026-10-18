//! Distribution summaries, computed in data space: type-7 quantiles and Tukey whiskers, Gaussian
//! kernel densities with Scott's bandwidth, and the steps of an empirical cumulative
//! distribution.

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

/// The finite samples of `values`, with their smallest and largest value; `None` when there is
/// no finite value.
fn finite_samples(values: &[f64]) -> Option<(Vec<f64>, f64, f64)> {
    let samples: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    let low = samples.iter().copied().reduce(f64::min)?;
    let high = samples.iter().copied().reduce(f64::max)?;
    Some((samples, low, high))
}

/// Scott's bandwidth of the `scaled` samples, in their units: the sample standard deviation
/// times `n^(-1/5)`. Needs at least two samples.
fn scott_factor(scaled: &[f64]) -> f64 {
    let n = scaled.len() as f64;
    let mean = scaled.iter().sum::<f64>() / n;
    let variance = scaled.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    variance.sqrt() * n.powf(-0.2)
}

/// The sum of the Gaussian kernels of bandwidth `h` around the `scaled` samples, at `fraction`,
/// all in the same units (data units serve too); not yet divided by the kernels' normalizer.
fn kernel_sum(scaled: &[f64], fraction: f64, h: f64) -> f64 {
    scaled
        .iter()
        .map(|v| {
            let z = (fraction - v) / h;
            (-0.5 * z * z).exp()
        })
        .sum::<f64>()
}

/// `grid` evenly spaced positions from `from` to `to`, both ends exactly; `grid` is at least 2.
/// A span too small for its step to be represented spreads the positions by their share of the
/// span instead.
fn grid_positions(from: f64, to: f64, grid: usize) -> impl Iterator<Item = f64> {
    let span = to - from;
    let step = span / (grid - 1) as f64;
    (0..grid).map(move |i| {
        if i + 1 == grid {
            to
        } else if step == 0.0 {
            from + span * (i as f64 / (grid - 1) as f64)
        } else {
            from + step * i as f64
        }
    })
}

/// Relative Gaussian kernel density with Scott's sample-standard-deviation bandwidth.
/// Densities are measured on the unit data range, without division by the original span.
/// Finite samples alone contribute; fewer than two distinct values or grid points yield none.
/// Scaling the data range to one keeps the variance finite for very large or small samples.
pub(crate) fn kde_scott(values: &[f64], grid: usize) -> Option<Vec<(f64, f64)>> {
    let (samples, low, high) = finite_samples(values)?;
    if low == high || grid < 2 {
        return None;
    }
    let span = high - low;
    let scaled: Vec<f64> = samples.iter().map(|v| (v - low) / span).collect();
    let h = scott_factor(&scaled);
    let normalizer = scaled.len() as f64 * h * (2.0 * std::f64::consts::PI).sqrt();
    Some(
        grid_positions(low, high, grid)
            .map(|at| {
                // Evaluate at the returned grid position, including its rounding at large offsets.
                let fraction = (at - low) / span;
                (at, kernel_sum(&scaled, fraction, h) / normalizer)
            })
            .collect(),
    )
}

/// Scott's bandwidth of the finite `values` in data units: the bandwidth [`kde_scott`] uses,
/// times the span of the values. `None` for fewer than two distinct finite values. The
/// variance is taken over the values scaled to the unit range, so it stays finite for very large
/// or small samples.
pub(crate) fn scott_bandwidth(values: &[f64]) -> Option<f64> {
    let (samples, low, high) = finite_samples(values)?;
    if low == high {
        return None;
    }
    let span = high - low;
    let scaled: Vec<f64> = samples.iter().map(|v| (v - low) / span).collect();
    Some(scott_factor(&scaled) * span)
}

/// The Gaussian kernel density of the finite `values` with `bandwidth` (in data units, finite
/// and positive), in data units, so its area is 1: `grid` evenly spaced positions from `from`
/// to `to`, both ends exactly, each with its density. `None` without a finite value or with
/// fewer than two grid points.
///
/// Each kernel is evaluated at its distance from the position in bandwidths, and the sum of the
/// kernels is divided by the number of values and `sqrt(2π)` before the bandwidth, so no
/// intermediate grows past the density itself, however the span of the values compares with
/// the bandwidth. Distances between supported values (at most 1e300 in magnitude) stay finite.
pub(crate) fn kde_curve(
    values: &[f64],
    bandwidth: f64,
    (from, to): (f64, f64),
    grid: usize,
) -> Option<Vec<(f64, f64)>> {
    let (samples, _, _) = finite_samples(values)?;
    if grid < 2 {
        return None;
    }
    let normalizer = samples.len() as f64 * (2.0 * std::f64::consts::PI).sqrt();
    Some(
        grid_positions(from, to, grid)
            .map(|at| {
                (
                    at,
                    kernel_sum(&samples, at, bandwidth) / normalizer / bandwidth,
                )
            })
            .collect(),
    )
}

/// The empirical cumulative distribution of the `sorted` values (in increasing order, no gaps)
/// as the points of a step line that holds each level up to the next point: x and y of equal
/// length. The first point is the smallest value at 0 (at 1 when `complementary`); then each
/// distinct value, at its last occurrence, with the share of the values at or below it (the
/// share above it when `complementary`). The last level is exactly 1 (exactly 0). Equal values
/// are one step, as `PartialEq` decides. Empty input gives no points.
pub(crate) fn ecdf_steps<T: Copy + PartialEq>(
    sorted: &[T],
    complementary: bool,
) -> (Vec<T>, Vec<f64>) {
    let Some(&first) = sorted.first() else {
        return (Vec::new(), Vec::new());
    };
    let n = sorted.len();
    let mut x = vec![first];
    let mut y = vec![if complementary { 1.0 } else { 0.0 }];
    for (i, &v) in sorted.iter().enumerate() {
        if sorted.get(i + 1) == Some(&v) {
            continue;
        }
        let below = i + 1;
        let share = if complementary { n - below } else { below };
        x.push(v);
        y.push(share as f64 / n as f64);
    }
    (x, y)
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

    /// The ECDF of `values` by its definition: for each distinct value in increasing order, the
    /// share of the values at or below it.
    fn ecdf_by_definition(values: &[f64]) -> Vec<(f64, f64)> {
        let mut distinct = values.to_vec();
        distinct.sort_by(f64::total_cmp);
        distinct.dedup();
        let n = values.len() as f64;
        distinct
            .iter()
            .map(|&x| (x, values.iter().filter(|&&v| v <= x).count() as f64 / n))
            .collect()
    }

    #[test]
    fn ecdf_matches_reference() {
        let values = [3.0, 1.0, 2.0, 2.0, 5.5, 1.0, 4.0, 0.0, 0.0];
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let reference = ecdf_by_definition(&values);
        let (x, y) = ecdf_steps(&sorted, false);
        // A post step line from 0 at the first value up through every distinct value.
        assert_eq!(x[0], reference[0].0);
        assert_eq!(y[0], 0.0);
        assert_eq!(x.len(), reference.len() + 1);
        for (i, &(at, share)) in reference.iter().enumerate() {
            assert_eq!(x[i + 1], at);
            assert!((y[i + 1] - share).abs() < 1e-15, "{} vs {share}", y[i + 1]);
        }
        assert_eq!(*y.last().unwrap(), 1.0);
        // The complementary ECDF falls from 1 to exactly 0.
        let (cx, cy) = ecdf_steps(&sorted, true);
        assert_eq!(cx, x);
        assert_eq!(cy[0], 1.0);
        for (c, f) in cy[1..].iter().zip(&y[1..]) {
            assert!((c - (1.0 - f)).abs() < 1e-15);
        }
        assert_eq!(*cy.last().unwrap(), 0.0);
        // One value: a single jump from 0 to 1.
        assert_eq!(ecdf_steps(&[7], false), (vec![7, 7], vec![0.0, 1.0]));
        assert_eq!(ecdf_steps::<f64>(&[], false), (vec![], vec![]));
    }

    /// The Gaussian kernel density of `samples` at `x`, with bandwidth `h`, by definition.
    fn density_by_definition(samples: &[f64], h: f64, x: f64) -> f64 {
        let n = samples.len() as f64;
        samples
            .iter()
            .map(|v| (-0.5 * ((x - v) / h).powi(2)).exp())
            .sum::<f64>()
            / (n * h * (2.0 * std::f64::consts::PI).sqrt())
    }

    #[test]
    fn kde_matches_violin_bandwidth() {
        let values = [0.3, 1.1, 1.4, 2.0, 2.2, 3.9, f64::NAN, 5.0, 5.1];
        let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
        let n = finite.len() as f64;
        let mean = finite.iter().sum::<f64>() / n;
        let sd = (finite.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
        let h = scott_bandwidth(&values).unwrap();
        assert!((h / (sd * n.powf(-0.2)) - 1.0).abs() < 1e-12, "{h}");
        // The violin's densities are relative to the unit range: the densities in data units of
        // the same bandwidth, times the span, at the same positions.
        let (low, high) = (0.3, 5.1);
        let violin = kde_scott(&values, 128).unwrap();
        let curve = kde_curve(&values, h, (low, high), 128).unwrap();
        for (&(at, relative), &(x, density)) in violin.iter().zip(&curve) {
            assert_eq!(at, x);
            let expected = relative / (high - low);
            assert!((density / expected - 1.0).abs() < 1e-12, "at {x}");
            let direct = density_by_definition(&finite, h, x);
            assert!((density / direct - 1.0).abs() < 1e-12, "at {x}");
        }
    }

    #[test]
    fn kde_curve_area_is_one_and_spans_the_given_range() {
        let values = [1.0, 2.0, 2.5, 7.0];
        let curve = kde_curve(&values, 0.5, (-5.0, 14.0), 4001).unwrap();
        assert_eq!(curve.first().unwrap().0, -5.0);
        assert_eq!(curve.last().unwrap().0, 14.0);
        let area: f64 = curve
            .windows(2)
            .map(|pair| (pair[1].0 - pair[0].0) * (pair[0].1 + pair[1].1) / 2.0)
            .sum();
        assert!((area - 1.0).abs() < 1e-9, "{area}");
        // One value with a given bandwidth is a single Gaussian.
        let one = kde_curve(&[3.0, f64::NAN], 2.0, (3.0, 3.0), 2).unwrap();
        let peak = 1.0 / (2.0 * (2.0 * std::f64::consts::PI).sqrt());
        assert!((one[0].1 / peak - 1.0).abs() < 1e-15);
        assert_eq!(kde_curve(&[f64::NAN], 1.0, (0.0, 1.0), 8), None);
        assert_eq!(kde_curve(&[1.0], 1.0, (0.0, 1.0), 1), None);
    }

    #[test]
    fn scott_needs_two_distinct_values() {
        assert_eq!(scott_bandwidth(&[]), None);
        assert_eq!(scott_bandwidth(&[2.0, 2.0, f64::NAN]), None);
        assert!(scott_bandwidth(&[2.0, 3.0]).is_some_and(|h| h > 0.0));
        // Scaling to the unit range keeps the bandwidth finite for huge samples.
        let huge = scott_bandwidth(&[-1e300, 1e300]).unwrap();
        assert!(huge.is_finite() && huge > 1e299, "{huge}");
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
