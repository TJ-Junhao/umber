//! Exact 8-bit comparisons against the uncached interpolation and transfer function.

use super::*;
use crate::LogBase;
use crate::color_scale::Norm;

fn reference(cmap: &Cmap, t: f64) -> Rgba {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    let last = cmap.stops.len() - 1;
    let position = t * last as f64;
    let segment = (position.floor() as usize).min(last - 1);
    let share = position - segment as f64;
    let (from, to) = (cmap.stops[segment], cmap.stops[segment + 1]);
    let [lightness, a, b] =
        std::array::from_fn(|axis| from[axis] + (to[axis] - from[axis]) * share);
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    Rgba::opaque(
        encode_direct(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
        encode_direct(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
        encode_direct(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s),
    )
}

#[test]
fn every_colormap_matches_uncached_dense_sweep_and_norms() {
    let maps = [
        Cmap::viridis(),
        Cmap::magma(),
        Cmap::cividis(),
        Cmap::blue_red(),
        Cmap::from_colors(&[
            Rgba::opaque(0, 0, 0),
            Rgba::opaque(0, 255, 0),
            Rgba::opaque(255, 0, 255),
            Rgba::opaque(255, 255, 255),
        ])
        .unwrap(),
    ];
    let norms = [
        Norm::Linear {
            low: -3.0,
            high: 9.0,
        },
        Norm::Centered {
            center: 2.0,
            radius: 11.0,
        },
        Norm::Centered {
            center: 0.0,
            radius: 7.0,
        },
        Norm::Log {
            low: 1e-12,
            high: 1e12,
            base: LogBase::TEN,
        },
    ];
    for original in maps {
        for cmap in [original.clone(), original.reversed()] {
            for i in 0..=200_000 {
                let t = f64::from(i) / 200_000.0;
                assert_eq!(cmap.at(t), reference(&cmap, t), "t={t}");
                for (norm, value) in norms.iter().zip([
                    -5.0 + 17.0 * t,
                    -12.0 + 29.0 * t,
                    -10.0 + 20.0 * t,
                    10.0_f64.powf(-14.0 + 28.0 * t),
                ]) {
                    let unit = norm.unit(value).unwrap();
                    assert_eq!(cmap.at(unit), reference(&cmap, unit), "{norm:?}: {value}");
                }
            }
            for t in [
                f64::NAN,
                f64::NEG_INFINITY,
                -1.0,
                -0.0,
                0.0,
                1.0,
                2.0,
                f64::INFINITY,
            ] {
                assert_eq!(cmap.at(t), reference(&cmap, t));
            }
            for stop in 0..cmap.stops.len() {
                let t = stop as f64 / (cmap.stops.len() - 1) as f64;
                for t in [t.next_down(), t, t.next_up()] {
                    assert_eq!(cmap.at(t), reference(&cmap, t));
                }
            }
        }
    }
}

#[test]
fn transfer_matches_at_bins_and_every_rounding_boundary() {
    for i in 0..=4096 {
        let value = f64::from(i) / 4096.0;
        for value in [value.next_down(), value, value.next_up()] {
            assert_eq!(encode(value), encode_direct(value));
        }
    }
    for byte in 0..255 {
        let srgb = (f64::from(byte) + 0.5) / 255.0;
        let boundary = if srgb <= 0.04045 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        };
        for bits in boundary.to_bits() - 2048..=boundary.to_bits() + 2048 {
            let value = f64::from_bits(bits);
            assert_eq!(
                encode(value),
                encode_direct(value),
                "byte={byte}, value={value:?}"
            );
        }
    }
    let mut bits = 1_u64;
    for i in 0..1_000_000 {
        bits = bits.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        for value in [f64::from(i) / 999_999.0, f64::from_bits(bits)] {
            assert_eq!(encode(value), encode_direct(value));
        }
    }
}
