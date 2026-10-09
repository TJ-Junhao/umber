//! M4 downsampling: reduce a long line to the points that are visible at a given pixel width.

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
    let width = hi - lo;
    let col = |x: f64| -> usize {
        let c = ((x - lo) / width * columns as f64).floor();
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
