//! Bounded least-squares placement of ordered label centres.

/// A preferred label centre and its individual feasible interval.
pub(super) struct Target {
    pub desired: f64,
    pub lower: f64,
    pub upper: f64,
}

struct Block {
    sum: f64,
    count: usize,
    lower: f64,
    upper: f64,
}

impl Block {
    fn level(&self) -> f64 {
        (self.sum / self.count as f64).clamp(self.lower, self.upper)
    }
}

/// Isotonic regression of `desired[i] - i * gap`, using pooled adjacent violators.
/// Each pool minimizes squared motion within the intersection of its transformed bounds.
/// An empty intersection means the ordered labels cannot fit with the required gap.
pub(super) fn centres(targets: &[Target], gap: f64) -> Option<Vec<f64>> {
    let mut blocks: Vec<Block> = Vec::new();
    for (i, target) in targets.iter().enumerate() {
        if target.lower > target.upper {
            return None;
        }
        let offset = i as f64 * gap;
        blocks.push(Block {
            sum: target.desired - offset,
            count: 1,
            lower: target.lower - offset,
            upper: target.upper - offset,
        });
        while blocks.len() >= 2 {
            let n = blocks.len();
            let left = &blocks[n - 2];
            let right = &blocks[n - 1];
            if left.level() <= right.level() {
                break;
            }
            let merged = Block {
                sum: left.sum + right.sum,
                count: left.count + right.count,
                lower: left.lower.max(right.lower),
                upper: left.upper.min(right.upper),
            };
            if merged.lower > merged.upper {
                return None;
            }
            blocks.truncate(n - 2);
            blocks.push(merged);
        }
    }
    let mut result = Vec::with_capacity(targets.len());
    for block in blocks {
        let level = block.level();
        for _ in 0..block.count {
            result.push(level + result.len() as f64 * gap);
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::{Target, centres};

    fn targets(desired: &[f64], lower: f64, upper: f64) -> Vec<Target> {
        desired
            .iter()
            .map(|&desired| Target {
                desired,
                lower,
                upper,
            })
            .collect()
    }

    #[test]
    fn pooled_levels_minimize_motion_and_respect_bounds() {
        assert_eq!(
            centres(&targets(&[0.0, 1.0, 2.0, 40.0], -100.0, 100.0), 10.0).unwrap(),
            [-9.0, 1.0, 11.0, 40.0]
        );
        assert_eq!(
            centres(&targets(&[0.0, 1.0, 2.0, 40.0], 0.0, 35.0), 10.0).unwrap(),
            [0.0, 10.0, 20.0, 35.0]
        );
        assert_eq!(
            centres(&targets(&[0.0, 1.0, 2.0, 40.0], 0.0, 30.0), 10.0).unwrap(),
            [0.0, 10.0, 20.0, 30.0]
        );
        assert!(centres(&[], 10.0).unwrap().is_empty());
    }

    #[test]
    fn individual_bounds_keep_clusters_on_their_side_of_the_circle() {
        let mut top = targets(&[-101.0, -100.0, -99.0], -150.0, 150.0);
        for target in &mut top {
            target.upper = target.desired;
        }
        assert_eq!(centres(&top, 10.0).unwrap(), [-119.0, -109.0, -99.0]);
        let mut bottom = targets(&[99.0, 100.0, 101.0], -150.0, 150.0);
        for target in &mut bottom {
            target.lower = target.desired;
        }
        assert_eq!(centres(&bottom, 10.0).unwrap(), [99.0, 109.0, 119.0]);
        top[0].lower = -110.0;
        assert!(centres(&top, 10.0).is_none());
    }

    #[test]
    fn clamped_targets_keep_the_unclamped_objective_when_pooled() {
        let targets = [
            Target {
                desired: 100.0,
                lower: -100.0,
                upper: 10.0,
            },
            Target {
                desired: 0.0,
                lower: -100.0,
                upper: 100.0,
            },
        ];
        assert_eq!(centres(&targets, 10.0).unwrap(), [10.0, 20.0]);
        assert!(centres(&self::targets(&[0.0, 1.0], 0.0, 5.0), 10.0).is_none());
        assert!(centres(&self::targets(&[0.0], 1.0, 0.0), 10.0).is_none());
    }
}
