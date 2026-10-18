//! Bounded least-squares placement of ordered label centres.

/// Isotonic regression of `desired[i] - i * gap`, using pooled adjacent violators.
/// Clamping pooled levels to the transformed bounds preserves the least-squares solution.
pub(super) fn centres(desired: &[f64], gap: f64, lower: f64, upper: f64) -> Vec<f64> {
    let mut blocks: Vec<(f64, usize)> = Vec::new();
    for (i, &y) in desired.iter().enumerate() {
        blocks.push((y - i as f64 * gap, 1));
        while blocks.len() >= 2 {
            let n = blocks.len();
            let (left, a) = blocks[n - 2];
            let (right, b) = blocks[n - 1];
            if left / a as f64 <= right / b as f64 {
                break;
            }
            blocks.truncate(n - 1);
            blocks[n - 2] = (left + right, a + b);
        }
    }
    let upper = upper - desired.len().saturating_sub(1) as f64 * gap;
    let mut result = Vec::with_capacity(desired.len());
    for (sum, count) in blocks {
        let level = (sum / count as f64).max(lower).min(upper);
        for _ in 0..count {
            result.push(level + result.len() as f64 * gap);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::centres;

    #[test]
    fn pooled_levels_minimize_motion_and_respect_bounds() {
        assert_eq!(
            centres(&[0.0, 1.0, 2.0, 40.0], 10.0, -100.0, 100.0),
            [-9.0, 1.0, 11.0, 40.0]
        );
        assert_eq!(
            centres(&[0.0, 1.0, 2.0, 40.0], 10.0, 0.0, 35.0),
            [0.0, 10.0, 20.0, 35.0]
        );
        assert_eq!(
            centres(&[0.0, 1.0, 2.0, 40.0], 10.0, 0.0, 30.0),
            [0.0, 10.0, 20.0, 30.0]
        );
        assert!(centres(&[], 10.0, 0.0, 30.0).is_empty());
    }
}
