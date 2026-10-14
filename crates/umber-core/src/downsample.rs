//! M4 downsampling: reduce a long line to the points that are visible at a given pixel width.
//!
//! # Examples
//!
//! ```
//! use umber_core::downsample::m4_indices;
//!
//! // 1000 points drawn across 10 pixel columns shrink to at most 4 points per column.
//! let xs: Vec<f64> = (0..1000).map(f64::from).collect();
//! let ys: Vec<f64> = xs.iter().map(|x| x.sin()).collect();
//! let kept = m4_indices(&xs, &ys, (0.0, 1000.0), 10);
//! assert!(kept.len() <= 40);
//!
//! // A short line is returned unchanged.
//! assert_eq!(m4_indices(&xs[..8], &ys[..8], (0.0, 8.0), 2), (0..8).collect::<Vec<_>>());
//! ```

/// Indices of the points to draw when a line spanning `x_range` is rendered across `columns`
/// device-pixel columns.
///
/// For every column, within every NaN-free run of points, the first, last, minimum-y and
/// maximum-y points are kept. This draws identically to the full line at that resolution.
/// NaN points (gaps) are always kept. The input is returned unchanged when it is already
/// small (`len <= 4 * columns`), when `x_range` is invalid, or when x is not sorted.
///
/// # Examples
///
/// ```
/// let xs: Vec<f64> = (0..1000).map(f64::from).collect();
/// let ys: Vec<f64> = xs.iter().map(|x| x.sin()).collect();
/// let kept = umber_core::m4_indices(&xs, &ys, (0.0, 1000.0), 10);
/// assert!(kept.len() <= 40);
/// assert_eq!(kept.first(), Some(&0));
/// ```
pub fn m4_indices(xs: &[f64], ys: &[f64], x_range: (f64, f64), columns: usize) -> Vec<usize> {
    let n = xs.len().min(ys.len());
    let (lo, hi) = x_range;
    let identity = || (0..n).collect::<Vec<usize>>();
    let valid_range = lo.is_finite() && hi.is_finite() && lo < hi;
    if columns == 0
        || n <= columns.saturating_mul(4)
        || !valid_range
        || !is_sorted_ignoring_nan(&xs[..n])
    {
        return identity();
    }
    // Normalize with the plain span. Only when the span itself overflows to infinity (for
    // example (-1e308, 1e308)) halve every operand; halving otherwise loses bits near the
    // smallest floats and would shift column boundaries.
    let (scale, width) = if (hi - lo).is_finite() {
        (1.0, hi - lo)
    } else {
        (0.5, hi * 0.5 - lo * 0.5)
    };
    if !(width.is_finite() && width > 0.0) {
        return identity();
    }
    let col = |x: f64| -> usize {
        let c = ((x * scale - lo * scale) / width * columns as f64).floor();
        if c <= 0.0 {
            0
        } else {
            (c as usize).min(columns - 1)
        }
    };

    let mut out = Vec::with_capacity(columns * 4 + 16);
    // Current bucket state: (column, first, last, argmin, argmax).
    let mut bucket: Option<(usize, usize, usize, usize, usize)> = None;
    let flush = |b: Option<(usize, usize, usize, usize, usize)>, out: &mut Vec<usize>| {
        if let Some((_, first, last, imin, imax)) = b {
            let mut four = [first, imin, imax, last];
            four.sort_unstable();
            for i in four {
                if out.last() != Some(&i) {
                    out.push(i);
                }
            }
        }
    };
    for i in 0..n {
        let (x, y) = (xs[i], ys[i]);
        if x.is_nan() || y.is_nan() {
            flush(bucket.take(), &mut out);
            out.push(i);
            continue;
        }
        let c = col(x);
        match bucket {
            Some((bc, first, _, imin, imax)) if bc == c => {
                let imin = if y < ys[imin] { i } else { imin };
                let imax = if y > ys[imax] { i } else { imax };
                bucket = Some((bc, first, i, imin, imax));
            }
            _ => {
                flush(bucket.take(), &mut out);
                bucket = Some((c, i, i, i, i));
            }
        }
    }
    flush(bucket, &mut out);
    out
}

/// M4 downsampling inside `x_range`, with the adjacent point outside each limit retained.
/// This keeps segments crossing the limits exact instead of letting off-screen extrema share
/// the first and last visible columns. Short or unsorted inputs are left unchanged.
pub(crate) fn visible_m4_indices(
    xs: &[f64],
    ys: &[f64],
    x_range: (f64, f64),
    columns: usize,
) -> Vec<usize> {
    let n = xs.len().min(ys.len());
    let xs = &xs[..n];
    let (lo, hi) = x_range;
    if columns == 0
        || n <= columns.saturating_mul(4)
        || !(lo.is_finite() && hi.is_finite() && lo < hi)
        || !is_sorted_ignoring_nan(xs)
    {
        return (0..n).collect();
    }
    let first = xs.iter().position(|&x| x >= lo).unwrap_or(n);
    let end = xs.iter().position(|&x| x > hi).unwrap_or(n);
    let (first, end) = (first.min(end), end);
    let mut kept: Vec<usize> = first.checked_sub(1).into_iter().collect();
    kept.extend(
        m4_indices(&xs[first..end], &ys[first..end], x_range, columns)
            .into_iter()
            .map(|i| first + i),
    );
    kept.extend((end < n).then_some(end));
    kept
}

fn is_sorted_ignoring_nan(xs: &[f64]) -> bool {
    let mut prev = f64::NEG_INFINITY;
    for &x in xs {
        if x.is_nan() {
            continue;
        }
        if x < prev {
            return false;
        }
        prev = x;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn visible_columns_keep_only_the_adjacent_outside_points() {
        let xs: Vec<f64> = (0..30).map(f64::from).collect();
        let mut ys = vec![1.0; 30];
        ys[2] = 100.0;
        ys[27] = -100.0;
        assert_eq!(
            visible_m4_indices(&xs, &ys, (10.0, 20.0), 2),
            [9, 10, 14, 15, 20, 21]
        );
        // No point inside: the one segment across the view is still exact.
        assert_eq!(visible_m4_indices(&xs, &ys, (10.25, 10.75), 2), [10, 11]);
    }

    #[test]
    fn visible_columns_keep_boundary_gaps_and_leave_unsorted_x_alone() {
        let mut xs: Vec<f64> = (0..30).map(f64::from).collect();
        let ys = vec![1.0; 30];
        xs[9] = f64::NAN;
        xs[20] = f64::NAN;
        assert_eq!(
            visible_m4_indices(&xs, &ys, (10.0, 20.0), 2),
            [9, 10, 14, 15, 19, 20, 21]
        );
        xs.swap(3, 4);
        assert_eq!(
            visible_m4_indices(&xs, &ys, (10.0, 20.0), 2),
            (0..30).collect::<Vec<_>>()
        );
    }

    #[test]
    fn small_inputs_are_untouched() {
        let xs: Vec<f64> = (0..8).map(f64::from).collect();
        assert_eq!(
            m4_indices(&xs, &xs, (0.0, 7.0), 2),
            (0..8).collect::<Vec<_>>()
        );
    }

    #[test]
    fn keeps_first_last_min_max_per_column() {
        // 2 columns, 10 points per column.
        let xs: Vec<f64> = (0..20).map(f64::from).collect();
        let mut ys = vec![0.0; 20];
        ys[3] = 5.0; // max of column 0
        ys[6] = -5.0; // min of column 0
        ys[12] = 9.0; // max of column 1
        ys[17] = -9.0; // min of column 1
        let got = m4_indices(&xs, &ys, (0.0, 20.0), 2);
        // Column 0: first 0, max 3, min 6, last 9. Column 1: first 10, max 12, min 17, last 19.
        assert_eq!(got, [0, 3, 6, 9, 10, 12, 17, 19]);
    }

    #[test]
    fn non_monotonic_x_is_left_untouched() {
        let xs: Vec<f64> = (0..100).map(|i| ((i * 37) % 100) as f64).collect();
        let ys = xs.clone();
        assert_eq!(m4_indices(&xs, &ys, (0.0, 100.0), 4).len(), 100);
    }

    #[test]
    fn invalid_range_or_zero_columns_is_identity() {
        let xs: Vec<f64> = (0..100).map(f64::from).collect();
        assert_eq!(m4_indices(&xs, &xs, (5.0, 5.0), 4).len(), 100);
        assert_eq!(m4_indices(&xs, &xs, (0.0, f64::NAN), 4).len(), 100);
        assert_eq!(m4_indices(&xs, &xs, (0.0, 100.0), 0).len(), 100);
    }

    #[test]
    fn span_overflow_does_not_collapse_columns() {
        let xs: Vec<f64> = (0..20).map(|i| (f64::from(i) - 10.0) * 1e307).collect();
        let mut ys = vec![0.0; 20];
        ys[3] = 5.0;
        ys[6] = -5.0;
        ys[12] = 9.0;
        ys[17] = -9.0;
        let got = m4_indices(&xs, &ys, (-1e308, 1e308), 2);
        assert_eq!(got, [0, 3, 6, 9, 10, 12, 17, 19]);
    }

    #[test]
    fn unsorted_x_within_one_column_is_full_identity() {
        // 12 points, all in column 0 of 2; x goes up and down.
        let xs = [1.0, 5.0, 2.0, 6.0, 3.0, 7.0, 4.0, 8.0, 2.5, 6.5, 1.5, 9.0];
        let ys: Vec<f64> = (0..12).map(f64::from).collect();
        assert_eq!(
            m4_indices(&xs, &ys, (0.0, 100.0), 2),
            (0..12).collect::<Vec<_>>()
        );
    }

    #[test]
    fn nan_in_x_is_a_gap_and_keeps_segment_endpoints() {
        // 2 columns, 20 points; x[10] is NaN while y[10] is finite.
        let mut xs: Vec<f64> = (0..20).map(f64::from).collect();
        xs[10] = f64::NAN;
        let ys = vec![0.0; 20];
        let got = m4_indices(&xs, &ys, (0.0, 20.0), 2);
        // Segment 0 is 0..=9 (column 0), gap at 10, segment 1 is 11..=19 (column 1).
        assert_eq!(got, [0, 9, 10, 11, 19]);
    }

    #[test]
    fn repeated_extrema_keep_the_first_occurrence() {
        // 2 columns, 10 points each. Column 0: max 5.0 at 2 and 7, min -5.0 at 4 and 8.
        // Column 1: max 9.0 at 11 and 15, min -9.0 at 13 and 18.
        let xs: Vec<f64> = (0..20).map(f64::from).collect();
        let mut ys = vec![0.0; 20];
        for i in [2, 7] {
            ys[i] = 5.0;
        }
        for i in [4, 8] {
            ys[i] = -5.0;
        }
        for i in [11, 15] {
            ys[i] = 9.0;
        }
        for i in [13, 18] {
            ys[i] = -9.0;
        }
        let got = m4_indices(&xs, &ys, (0.0, 20.0), 2);
        assert_eq!(got, [0, 2, 4, 9, 10, 11, 13, 19]);
    }

    #[test]
    fn x_outside_range_is_clamped_into_edge_columns() {
        // Range 10..20 over 2 columns; x runs from 0 to 29, so 10 points lie below the
        // range, 10 inside and 10 above. Below-range points join column 0, above-range
        // points join column 1.
        let xs: Vec<f64> = (0..30).map(f64::from).collect();
        let mut ys = vec![0.0; 30];
        ys[2] = 5.0; // max of column 0 (below range)
        ys[7] = -5.0; // min of column 0 (below range)
        ys[22] = 9.0; // max of column 1 (above range)
        ys[27] = -9.0; // min of column 1 (above range)
        let got = m4_indices(&xs, &ys, (10.0, 20.0), 2);
        // Column 0 spans x 0..=14, column 1 spans x 15..=29.
        assert_eq!(got, [0, 2, 7, 14, 15, 22, 27, 29]);
    }

    #[test]
    fn length_mismatch_uses_the_common_prefix() {
        let xs: Vec<f64> = (0..30).map(f64::from).collect();
        let ys = vec![0.0; 20];
        // xs longer: only 0..20 is considered, 20 > 4 * 2 so it is downsampled.
        assert_eq!(m4_indices(&xs, &ys, (0.0, 20.0), 2), [0, 9, 10, 19]);
        // ys longer.
        let xs2: Vec<f64> = (0..20).map(f64::from).collect();
        let ys2 = vec![0.0; 30];
        assert_eq!(m4_indices(&xs2, &ys2, (0.0, 20.0), 2), [0, 9, 10, 19]);
        // Short common prefix is returned as identity over the prefix.
        assert_eq!(m4_indices(&xs, &ys[..5], (0.0, 20.0), 2), [0, 1, 2, 3, 4]);
    }

    /// 20 points in two groups of 10 with extrema at fixed positions.
    fn extrema_ys() -> Vec<f64> {
        let mut ys = vec![0.0; 20];
        ys[3] = 5.0;
        ys[6] = -5.0;
        ys[10] = 9.0;
        ys[17] = -9.0;
        ys
    }

    #[test]
    fn subnormal_range_keeps_column_boundaries() {
        let u = f64::from_bits(1);
        let xs: Vec<f64> = (0..20).map(|i| f64::from(i / 2) * u).collect();
        let got = m4_indices(&xs, &extrema_ys(), (0.0, 10.0 * u), 2);
        assert_eq!(got, [0, 3, 6, 9, 10, 17, 19]);
    }

    #[test]
    fn range_near_min_positive_keeps_column_boundaries() {
        let b = f64::MIN_POSITIVE.to_bits();
        let xs: Vec<f64> = (0..20).map(|i| f64::from_bits(b + i / 2)).collect();
        let range = (f64::from_bits(b), f64::from_bits(b + 10));
        let got = m4_indices(&xs, &extrema_ys(), range, 2);
        assert_eq!(got, [0, 3, 6, 9, 10, 17, 19]);
    }

    #[test]
    fn identical_subnormal_x_is_downsampled_not_identity() {
        let u = f64::from_bits(1);
        let xs = vec![3.0 * u; 20];
        // One column holds everything: first 0, max 9.0 at 10, min -9.0 at 17, last 19.
        let got = m4_indices(&xs, &extrema_ys(), (3.0 * u, 4.0 * u), 2);
        assert_eq!(got, [0, 10, 17, 19]);
    }

    fn sorted_xs() -> impl Strategy<Value = Vec<f64>> {
        prop::collection::vec(0.0f64..1.0, 50..2000).prop_map(|steps| {
            steps
                .iter()
                .scan(0.0, |acc, d| {
                    *acc += d;
                    Some(*acc)
                })
                .collect()
        })
    }

    proptest! {
        #[test]
        fn preserves_column_extremes_and_gaps(
            xs in sorted_xs(),
            seed_ys in prop::collection::vec(prop_oneof![9 => -1e3f64..1e3, 1 => Just(f64::NAN)], 2000),
            columns in 1usize..40,
        ) {
            let n = xs.len();
            let ys = &seed_ys[..n];
            let lo = xs[0];
            let hi = xs[n - 1] + 1.0;
            let kept = m4_indices(&xs, ys, (lo, hi), columns);

            prop_assert!(kept.windows(2).all(|w| w[0] < w[1]));
            prop_assert!(kept.iter().all(|&i| i < n));
            for (i, y) in ys.iter().enumerate() {
                if y.is_nan() { prop_assert!(kept.contains(&i), "gap {} dropped", i); }
            }
            if n > 4 * columns {
                // For every (segment, column) bucket, min and max y over kept == over all points.
                let col = |x: f64| (((x - lo) / (hi - lo) * columns as f64).floor() as usize).min(columns - 1);
                let mut seg = 0usize;
                let mut all = std::collections::HashMap::<(usize, usize), (f64, f64)>::new();
                let mut seg_of = vec![usize::MAX; n];
                for i in 0..n {
                    if ys[i].is_nan() { seg += 1; continue; }
                    seg_of[i] = seg;
                    let e = all.entry((seg, col(xs[i]))).or_insert((f64::INFINITY, f64::NEG_INFINITY));
                    e.0 = e.0.min(ys[i]); e.1 = e.1.max(ys[i]);
                }
                let mut got = std::collections::HashMap::<(usize, usize), (f64, f64)>::new();
                for &i in &kept {
                    if ys[i].is_nan() { continue; }
                    let e = got.entry((seg_of[i], col(xs[i]))).or_insert((f64::INFINITY, f64::NEG_INFINITY));
                    e.0 = e.0.min(ys[i]); e.1 = e.1.max(ys[i]);
                }
                prop_assert_eq!(all, got);
            } else {
                prop_assert_eq!(kept.len(), n);
            }
        }
    }
}
