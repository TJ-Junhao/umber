//! Deterministic category-point offsets in physical points.

/// How category samples are spread across their band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Placement {
    /// Uniform jitter up to a fraction of the band, seeded by the strip's ordinal.
    Jitter {
        /// The maximum displacement as a fraction of the band.
        fraction: f64,
        /// This strip's ordinal among strips on the same axes.
        strip: usize,
    },
    /// Nearest-free-offset circle packing.
    Swarm,
}

/// Tolerance in physical points for tangent pockets and equal-distance choices.
const OFFSET_TOLERANCE: f64 = 1e-9;

/// A packed prefix and whether all requested circles fit. An incomplete packing is used only
/// during tentative legend layout; final rendering checks it and returns a category error.
pub(crate) struct Packing {
    /// Offsets perpendicular to the values, one per successfully placed point.
    pub(crate) offsets: Vec<f64>,
    /// Whether every requested point fits.
    pub(crate) complete: bool,
}

/// Packs sorted physical value coordinates at the nearest available centre-line offset.
/// Neighbours less than one diameter away exclude open intervals of offsets. Their union
/// gives the nearest free position; equal-distance choices alternate sides by point order.
/// Touching ends leave a tangent pocket even when their last bits differ. The circle radius
/// must stay inside `half_band`. Only nearby previous points enter the interval calculation.
pub(crate) fn pack(values: &[f64], diameter: f64, half_band: f64) -> Packing {
    let limit = half_band - diameter / 2.0;
    // Scene coordinates are f32. A tiny separation preserves tangency after narrowing.
    let distance = diameter + 1e-5;
    let mut offsets = Vec::with_capacity(values.len());
    let mut active: Vec<(f64, f64)> = Vec::new();
    let mut intervals = Vec::new();
    for (order, &value) in values.iter().enumerate() {
        active.retain(|&(previous, _)| (value - previous).abs() < distance);
        intervals.clear();
        for &(previous, offset) in &active {
            let dy = (value - previous).abs();
            let reach = ((distance - dy) * (distance + dy)).sqrt();
            intervals.push((offset - reach, offset + reach));
        }
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut chosen = 0.0;
        let mut i = 0;
        while i < intervals.len() {
            let (low, mut high) = intervals[i];
            i += 1;
            while i < intervals.len() && intervals[i].0 < high - OFFSET_TOLERANCE {
                high = high.max(intervals[i].1);
                i += 1;
            }
            if low < -OFFSET_TOLERANCE && high > OFFSET_TOLERANCE {
                chosen = if (low.abs() - high.abs()).abs() <= OFFSET_TOLERANCE {
                    if order % 2 == 0 { high } else { low }
                } else if low.abs() < high.abs() {
                    low
                } else {
                    high
                };
                break;
            }
        }
        if chosen.abs() > limit {
            return Packing {
                offsets,
                complete: false,
            };
        }
        offsets.push(chosen);
        active.push((value, chosen));
    }
    Packing {
        offsets,
        complete: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_offset_is_zero_or_tangent_and_inside_band() {
        for step in [0.0, 0.01, 0.5, 2.0, 6.0] {
            let values: Vec<_> = (0..30).map(|i| f64::from(i) * step).collect();
            let packing = pack(&values, 6.0, 120.0);
            assert!(packing.complete);
            for (i, &offset) in packing.offsets.iter().enumerate() {
                assert!(offset.abs() + 3.0 <= 120.0);
                if offset != 0.0 {
                    assert!((0..i).any(|j| {
                        ((offset - packing.offsets[j]).hypot(values[i] - values[j]) - 6.00001).abs()
                            < 1e-7
                    }));
                }
                for j in 0..i {
                    assert!((offset - packing.offsets[j]).hypot(values[i] - values[j]) >= 6.0);
                }
            }
        }
    }

    /// Searches every tangent candidate independently of the interval-union algorithm.
    fn nearest_free(values: &[f64], offsets: &[f64], value: f64, diameter: f64) -> f64 {
        let distance = diameter + 1e-5;
        let mut candidates = vec![0.0];
        for (&previous, &offset) in values.iter().zip(offsets) {
            let dy = (value - previous).abs();
            if dy < distance {
                let reach = (distance * distance - dy * dy).sqrt();
                candidates.extend([offset - reach, offset + reach]);
            }
        }
        candidates
            .into_iter()
            .filter(|&candidate| {
                values.iter().zip(offsets).all(|(&previous, &offset)| {
                    (candidate - offset).hypot(value - previous) >= distance - 1e-9
                })
            })
            .map(f64::abs)
            .min_by(f64::total_cmp)
            .unwrap()
    }

    #[test]
    fn touching_intervals_leave_a_tangent_pocket() {
        let values = [0.0, 3.0, 6.0, 7.0, 11.0, 12.0];
        let packing = pack(&values, 6.0, 1e9);
        assert!(packing.complete);
        let nearest = nearest_free(&values, &packing.offsets[..5], values[5], 6.0);
        assert!(
            (packing.offsets[5].abs() - nearest).abs() <= 1e-9,
            "last offset = {}, nearest free distance = {nearest}",
            packing.offsets[5]
        );
        assert!((2.60..2.62).contains(&nearest));
    }

    #[test]
    fn swarm_points_never_overlap() {
        let mut seed = 0x6a09_e667_f3bc_c909_u64;
        let mut random = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 11) as f64 / (1_u64 << 53) as f64
        };
        for step in [0.0, 0.5, 2.0, 3.0] {
            for trial in 0..48 {
                let count = 10 + (random() * 151.0) as usize;
                let mut values: Vec<_> = (0..count)
                    .map(|_| {
                        let value = random() * 60.0;
                        if step == 0.0 {
                            value
                        } else {
                            (value / step).round() * step
                        }
                    })
                    .collect();
                values.sort_by(f64::total_cmp);
                let half_band = 20.0 + random() * 100.0;
                let packing = pack(&values, 6.0, half_band);
                for (i, &offset) in packing.offsets.iter().enumerate() {
                    assert!(offset.abs() + 3.0 <= half_band);
                    for j in 0..i {
                        assert!((offset - packing.offsets[j]).hypot(values[i] - values[j]) >= 6.0);
                    }
                    let nearest = nearest_free(&values, &packing.offsets[..i], values[i], 6.0);
                    assert!(
                        (offset.abs() - nearest).abs() <= 1e-9,
                        "step={step}, trial={trial}, point={i}: offset={offset}, nearest={nearest}"
                    );
                }
                if !packing.complete {
                    let i = packing.offsets.len();
                    assert!(
                        nearest_free(&values, &packing.offsets, values[i], 6.0) > half_band - 3.0
                    );
                }
            }
        }
    }

    #[test]
    fn equidistant_offsets_alternate_by_point_order() {
        // Each pair starts on the centre line; the second point has two equal choices.
        let packing = pack(&[0.0, 0.0, 20.0, 40.0, 40.0], 6.0, 100.0);
        assert!(packing.complete);
        assert!(packing.offsets[1] < 0.0);
        assert!(packing.offsets[4] > 0.0);
    }

    #[test]
    fn empty_and_oversize_circles() {
        assert!(pack(&[], 100.0, 1.0).complete);
        assert!(!pack(&[1.0], 100.0, 1.0).complete);
        assert!(pack(&[1.0], 2.0, 1.0).complete);
        assert!(!pack(&[1.0; 100], 6.0, 20.0).complete);
    }
}
