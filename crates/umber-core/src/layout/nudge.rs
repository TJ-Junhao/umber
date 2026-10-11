//! Direct-label separation: moves labels apart along one dimension so that neighbours keep a
//! minimum distance.

/// The positions of labels that want to sit at `targets`, moved apart so that every two
/// neighbours are at least `min_gap` apart, in the order of `targets`.
///
/// The labels keep their order: they are sorted by target, ties broken by their index in
/// `targets` (a stable sort), and the k-th label of that order gets the k-th smallest position.
///
/// When the labels fit, that is when `(n - 1) × min_gap <= hi - lo`, every position lies in
/// `[lo, hi]`: a forward pass puts each label at its target, but no lower than `lo` and at least
/// `min_gap` past the previous label, and a backward pass then pulls each label back to at most
/// `hi` and at least `min_gap` before the next one. Labels already in range and far enough
/// apart stay at their targets. When the labels do not fit, they keep the gap as a stack
/// centered on the mean target, which leaves the range on both sides.
///
/// O(n log n) for n targets. Targets are expected to be finite.
pub(super) fn separate(targets: &[f64], min_gap: f64, lo: f64, hi: f64) -> Vec<f64> {
    let mut sorted: Vec<(usize, f64)> = targets.iter().copied().enumerate().collect();
    // A stable sort: equal targets keep their index order.
    sorted.sort_by(|a, b| a.1.total_cmp(&b.1));
    let n = sorted.len();
    let span = n.saturating_sub(1) as f64 * min_gap;
    if span > hi - lo {
        let mean = targets.iter().sum::<f64>() / n as f64;
        let start = mean - span / 2.0;
        for (k, (_, position)) in sorted.iter_mut().enumerate() {
            *position = start + k as f64 * min_gap;
        }
    } else {
        let mut floor = lo;
        for (_, position) in &mut sorted {
            *position = position.max(floor);
            floor = *position + min_gap;
        }
        let mut ceiling = hi;
        for (_, position) in sorted.iter_mut().rev() {
            *position = position.min(ceiling);
            ceiling = *position - min_gap;
        }
    }
    let mut out = vec![0.0; n];
    for (index, position) in sorted {
        if let Some(slot) = out.get_mut(index) {
            *slot = position;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The indices of `targets` in label order: by target, ties by index.
    fn label_order(targets: &[f64]) -> Vec<usize> {
        let mut order: Vec<usize> = (0..targets.len()).collect();
        order.sort_by(|&a, &b| targets[a].total_cmp(&targets[b]));
        order
    }

    /// A tolerance for comparing positions near `scale`.
    fn eps(scale: f64) -> f64 {
        1e-9 * scale.abs().max(1.0)
    }

    #[test]
    fn separated_labels_stay_at_their_targets() {
        let targets = [40.0, 10.0, 25.0];
        assert_eq!(separate(&targets, 12.0, 0.0, 100.0), targets);
        assert!(separate(&[], 12.0, 0.0, 100.0).is_empty());
        assert_eq!(separate(&[150.0], 12.0, 0.0, 100.0), [100.0]);
    }

    #[test]
    fn close_labels_move_apart_and_stay_in_range() {
        // Two labels near the high end: the lower one is pushed back by the upper one.
        assert_eq!(separate(&[98.0, 99.0], 10.0, 0.0, 100.0), [90.0, 100.0]);
        // Two labels near the low end: the upper one is pushed on.
        assert_eq!(separate(&[1.0, 0.0], 10.0, 0.0, 100.0), [10.0, 0.0]);
        // Equal targets: the first index comes first.
        assert_eq!(separate(&[50.0, 50.0], 10.0, 0.0, 100.0), [50.0, 60.0]);
    }

    #[test]
    fn labels_that_do_not_fit_center_on_the_mean_target() {
        // Five labels 10 apart need 40, and the range is 30.
        let out = separate(&[0.0, 5.0, 10.0, 15.0, 20.0], 10.0, 0.0, 30.0);
        assert_eq!(out, [-10.0, 0.0, 10.0, 20.0, 30.0]);
        // Labels that fit exactly fill the range.
        let out = separate(&[15.0, 15.0, 15.0, 15.0], 10.0, 0.0, 30.0);
        assert_eq!(out, [0.0, 10.0, 20.0, 30.0]);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(2000))]

        #[test]
        fn separate_keeps_order_and_min_gap(
            targets in prop::collection::vec(
                prop_oneof![-100.0_f64..400.0, Just(0.0), Just(300.0), Just(150.0)],
                0..40,
            ),
            min_gap in prop_oneof![0.0_f64..30.0, Just(10.2)],
            lo in -50.0_f64..100.0,
            span in 0.0_f64..400.0,
        ) {
            let hi = lo + span;
            let out = separate(&targets, min_gap, lo, hi);
            prop_assert_eq!(out.len(), targets.len());
            prop_assert!(out.iter().all(|p| p.is_finite()));
            // Neighbours in label order keep their order and at least the gap.
            let order = label_order(&targets);
            for pair in order.windows(2) {
                let (a, b) = (out[pair[0]], out[pair[1]]);
                prop_assert!(b - a >= min_gap - eps(b), "{} then {} with gap {}", a, b, min_gap);
            }
            let n = targets.len();
            if n > 0 && (n - 1) as f64 * min_gap <= hi - lo {
                // Whenever the labels fit, they stay inside the range.
                for &p in &out {
                    prop_assert!(
                        lo - eps(lo) <= p && p <= hi + eps(hi),
                        "{} outside {}..{}", p, lo, hi
                    );
                }
            } else if n > 0 {
                // Otherwise the stack is centered on the mean target.
                let mean = targets.iter().sum::<f64>() / n as f64;
                let center = (out[order[0]] + out[order[n - 1]]) / 2.0;
                prop_assert!((center - mean).abs() <= 100.0 * eps(mean));
            }
        }
    }
}
