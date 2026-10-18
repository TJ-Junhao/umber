//! Deterministic category-point offsets in physical points.

/// How category samples are spread across their band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Placement {
    /// Uniform jitter up to this fraction of the band.
    Jitter(f64),
    /// Nearest-free-offset circle packing.
    Swarm,
}

/// SplitMix64 with a category-local seed; no mutable state is shared across marks or renders.
pub(crate) struct Jitter(u64);

impl Jitter {
    /// Starts a reproducible stream for one category index.
    pub(crate) fn new(category: usize) -> Self {
        Self(category as u64)
    }

    /// The next uniform displacement in `[-1, 1)` using the high 53 random bits.
    pub(crate) fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        ((z >> 11) as f64 / ((1_u64 << 53) as f64)) * 2.0 - 1.0
    }
}

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
/// gives the exact nearest free position; ties go to the negative side. The circle radius
/// must stay inside `half_band`. Only nearby previous points enter the interval calculation.
pub(crate) fn pack(values: &[f64], diameter: f64, half_band: f64) -> Packing {
    let limit = half_band - diameter / 2.0;
    // Scene coordinates are f32. A tiny separation preserves tangency after narrowing.
    let distance = diameter + 1e-5;
    let mut offsets = Vec::with_capacity(values.len());
    let mut active: Vec<(f64, f64)> = Vec::new();
    let mut intervals = Vec::new();
    for &value in values {
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
            while i < intervals.len() && intervals[i].0 < high {
                high = high.max(intervals[i].1);
                i += 1;
            }
            if low < 0.0 && high > 0.0 {
                chosen = if low.abs() <= high.abs() { low } else { high };
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

    #[test]
    fn empty_and_oversize_circles() {
        assert!(pack(&[], 100.0, 1.0).complete);
        assert!(!pack(&[1.0], 100.0, 1.0).complete);
        assert!(pack(&[1.0], 2.0, 1.0).complete);
        assert!(!pack(&[1.0; 100], 6.0, 20.0).complete);
    }

    #[test]
    fn splitmix_stream_is_fixed_and_category_local() {
        let mut a = Jitter::new(0);
        let mut b = Jitter::new(0);
        let mut c = Jitter::new(1);
        assert_eq!(a.next(), 0.7666216164272852);
        let _ = b.next();
        for _ in 0..1000 {
            let x = a.next();
            assert_eq!(x, b.next());
            assert!((-1.0..1.0).contains(&x));
            assert_ne!(x, c.next());
        }
    }
}
