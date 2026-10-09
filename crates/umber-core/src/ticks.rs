//! Tick positions (Extended Wilkinson) and tick label formatting.

/// One axis tick: a position in data space and the text printed next to it.
///
/// # Examples
///
/// ```
/// use umber_core::{ContinuousScale, Transform};
///
/// let scale = ContinuousScale::new(Transform::Linear, 0.0, 100.0, "doc").unwrap();
/// let ticks = scale.ticks(5);
/// assert_eq!(ticks[0].value, 0.0);
/// assert_eq!(ticks[0].label, "0");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    /// Position of the tick in data space.
    pub value: f64,
    /// Text printed next to the tick, e.g. `"2.5"` or `"\u{2212}40"`.
    pub label: String,
}

const Q: [f64; 6] = [1.0, 5.0, 2.0, 2.5, 4.0, 3.0];
const W: [f64; 4] = [0.25, 0.2, 0.5, 0.05];
const EPS: f64 = 1e-10;
/// `Q.len() - 1`, as a float.
const Q_LAST: f64 = 5.0;

fn simplicity(qi: usize, j: f64, lmin: f64, lmax: f64, lstep: f64) -> f64 {
    let r = lmin % lstep;
    let zero_included = (r.abs() < EPS || lstep - r.abs() < EPS) && lmin <= 0.0 && 0.0 <= lmax;
    let v = if zero_included { 1.0 } else { 0.0 };
    1.0 - qi as f64 / Q_LAST - j + v
}

fn simplicity_max(qi: usize, j: f64) -> f64 {
    1.0 - qi as f64 / Q_LAST - j + 1.0
}

fn coverage(dmin: f64, dmax: f64, lmin: f64, lmax: f64) -> f64 {
    let r = dmax - dmin;
    1.0 - 0.5 * ((dmax - lmax).powi(2) + (dmin - lmin).powi(2)) / (0.1 * r).powi(2)
}

fn coverage_max(dmin: f64, dmax: f64, span: f64) -> f64 {
    let r = dmax - dmin;
    if span > r {
        let half = (span - r) / 2.0;
        1.0 - 0.5 * (2.0 * half * half) / (0.1 * r).powi(2)
    } else {
        1.0
    }
}

fn density(k: f64, m: f64, dmin: f64, dmax: f64, lmin: f64, lmax: f64) -> f64 {
    let r = (k - 1.0) / (lmax - lmin);
    let rt = (m - 1.0) / (lmax.max(dmax) - dmin.min(lmin));
    2.0 - (r / rt).max(rt / r)
}

fn density_max(k: f64, m: f64) -> f64 {
    if k >= m {
        2.0 - (k - 1.0) / (m - 1.0)
    } else {
        1.0
    }
}

/// `n` values from `lo` to `hi` inclusive, each computed from its index (no accumulation).
pub(crate) fn evenly_spaced(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let n = n.max(2);
    let last = (n - 1) as f64;
    (0..n)
        .map(|i| {
            let t = i as f64 / last;
            lo * (1.0 - t) + hi * t
        })
        .collect()
}

/// Extended Wilkinson tick positions for `[dmin, dmax]` (Talbot, Lin & Hanrahan, 2010).
///
/// With `loose` the first tick is `<= dmin` and the last is `>= dmax`. Values are computed as
/// `start * step`, never by accumulation. Invalid input (non-finite or `dmin >= dmax`) and the
/// case where no candidate is found yield `max(target, 2)` evenly spaced values instead.
pub(crate) fn extended(dmin: f64, dmax: f64, target: usize, loose: bool) -> Vec<f64> {
    let target = target.max(2);
    if !(dmin.is_finite() && dmax.is_finite() && dmin < dmax) {
        return evenly_spaced(dmin, dmax, target);
    }
    let m = target as f64;
    let mut best: Option<(f64, f64, f64, usize)> = None; // (lmin, lmax, step, k)
    let mut best_score = -2.0_f64;

    let mut j: u32 = 1;
    'jobs: while j <= 50 {
        let jf = f64::from(j);
        for (qi, &q) in Q.iter().enumerate() {
            let sm = simplicity_max(qi, jf);
            if W[0] * sm + W[1] + W[2] + W[3] < best_score {
                break 'jobs;
            }
            let mut k: usize = 2;
            while k <= 50 {
                let kf = k as f64;
                let dm = density_max(kf, m);
                if W[0] * sm + W[1] + W[2] * dm + W[3] < best_score {
                    break;
                }
                let delta = (dmax - dmin) / (kf + 1.0) / jf / q;
                let zf = delta.log10().ceil();
                if zf.is_finite() && zf.abs() < 1e6 {
                    let mut z = zf as i32;
                    while z < 400 {
                        let step = jf * q * 10f64.powi(z);
                        if !step.is_finite() || step <= 0.0 {
                            break;
                        }
                        let cm = coverage_max(dmin, dmax, step * (kf - 1.0));
                        if W[0] * sm + W[1] * cm + W[2] * dm + W[3] < best_score {
                            break;
                        }
                        let min_start = (dmax / step).floor() * jf - (kf - 1.0) * jf;
                        let max_start = (dmin / step).ceil() * jf;
                        if min_start.is_finite() && max_start.is_finite() {
                            let (lo_i, hi_i) = (min_start as i64, max_start as i64);
                            // A sane search window is a handful of starts; skip absurd ones.
                            if hi_i.saturating_sub(lo_i) <= 1_000_000 {
                                for start in lo_i..=hi_i {
                                    let lmin = start as f64 * (step / jf);
                                    let lmax = lmin + step * (kf - 1.0);
                                    let s = simplicity(qi, jf, lmin, lmax, step);
                                    let c = coverage(dmin, dmax, lmin, lmax);
                                    let g = density(kf, m, dmin, dmax, lmin, lmax);
                                    let score = W[0] * s + W[1] * c + W[2] * g + W[3];
                                    if score > best_score
                                        && (!loose || (lmin <= dmin && lmax >= dmax))
                                    {
                                        best_score = score;
                                        best = Some((lmin, lmax, step, k));
                                    }
                                }
                            }
                        }
                        z += 1;
                    }
                }
                k += 1;
            }
        }
        j += 1;
    }

    match best {
        Some((lmin, _, step, k)) => (0..k).map(|i| lmin + i as f64 * step).collect(),
        None => evenly_spaced(dmin, dmax, target),
    }
}

/// Smallest number of decimals (0..=12) at which `step` is a whole number of units.
fn decimals(step: f64) -> usize {
    (0..=12_i32)
        .find(|&d| {
            let scaled = step * 10f64.powi(d);
            (scaled - scaled.round()).abs() < 1e-6 * scaled
        })
        .map_or(12, |d| d as usize)
}

/// Scientific form with U+2212 minus signs: `5e6`, `2.5e6`, `-3e-5`. `exact` keeps the
/// shortest round-trip mantissa; otherwise the mantissa is rounded to 12 significant digits
/// so that products like `3 * 1e-5` read `3e-5`.
fn scientific(v: f64, exact: bool) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    let text = if exact {
        format!("{v:e}")
    } else {
        let rounded = format!("{v:.11e}");
        match rounded.split_once('e') {
            Some((mantissa, exp)) => {
                let mantissa = if mantissa.contains('.') {
                    mantissa.trim_end_matches('0').trim_end_matches('.')
                } else {
                    mantissa
                };
                format!("{mantissa}e{exp}")
            }
            None => rounded,
        }
    };
    text.replace('-', "\u{2212}")
}

/// Labels for tick `values` spaced `step` apart.
///
/// The number of decimals follows from `step`, values within `step * 1e-9` of zero print as an
/// unsigned zero, negatives use U+2212, and a set containing a value of at least `1e6` or a
/// non-zero value below `1e-4` is written in scientific form.
pub(crate) fn format_ticks(values: &[f64], step: f64) -> Vec<String> {
    let step = step.abs();
    let tiny = step * 1e-9;
    let snapped: Vec<f64> = values
        .iter()
        .map(|&v| if v == 0.0 || v.abs() < tiny { 0.0 } else { v })
        .collect();
    let use_scientific = snapped
        .iter()
        .any(|&v| v != 0.0 && (v.abs() >= 1e6 || v.abs() < 1e-4));
    if use_scientific {
        return snapped
            .iter()
            .map(|&v| {
                if v == 0.0 {
                    "0".to_string()
                } else {
                    scientific(v, false)
                }
            })
            .collect();
    }
    let d = decimals(step);
    snapped
        .iter()
        .map(|&v| {
            let text = format!("{v:.d$}");
            // Rounding can leave a negative that reads as zero; it must not show a sign.
            let unsigned = text.trim_start_matches('-');
            if unsigned.chars().all(|c| c == '0' || c == '.') {
                unsigned.to_string()
            } else {
                text.replace('-', "\u{2212}")
            }
        })
        .collect()
}

/// Labels in scientific form for every non-zero value, used for degenerate ranges where the
/// values are too close together (or too extreme) for fixed-point labels.
pub(crate) fn format_ticks_scientific(values: &[f64]) -> Vec<String> {
    values
        .iter()
        .map(|&v| {
            if v == 0.0 {
                "0".to_string()
            } else {
                scientific(v, true)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(x, y)| (x - y).abs() <= 1e-9 * y.abs().max(1e-12))
    }

    #[test]
    fn matches_reference_port() {
        // (dmin, dmax, target, loose) -> expected, produced by the verified Python port.
        let cases: &[(f64, f64, usize, bool, &[f64])] = &[
            (0.0, 100.0, 5, false, &[0.0, 25.0, 50.0, 75.0, 100.0]),
            (0.0, 100.0, 4, false, &[0.0, 50.0, 100.0]),
            (-1.5, 1.5, 4, false, &[-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5]),
            (0.0, 30.0, 5, false, &[0.0, 10.0, 20.0, 30.0]),
            (0.0, 25.0, 4, false, &[0.0, 5.0, 10.0, 15.0, 20.0, 25.0]),
            (0.5, 9.5, 5, false, &[0.0, 2.5, 5.0, 7.5, 10.0]),
            (3.2, 7.9, 5, false, &[3.0, 4.0, 5.0, 6.0, 7.0, 8.0]),
            (0.0, 1.0, 5, false, &[0.0, 0.25, 0.5, 0.75, 1.0]),
            (-37.0, 112.0, 5, false, &[-40.0, 0.0, 40.0, 80.0, 120.0]),
            (0.001, 0.0027, 5, false, &[0.001, 0.0015, 0.002, 0.0025]),
            (
                0.001,
                0.0027,
                5,
                true,
                &[0.001, 0.0015, 0.002, 0.0025, 0.003],
            ),
            (1200.0, 98000.0, 4, false, &[0.0, 50000.0, 100000.0]),
            (0.0, 8.0, 4, false, &[0.0, 2.5, 5.0, 7.5]),
            (0.0, 8.0, 4, true, &[0.0, 2.0, 4.0, 6.0, 8.0]),
        ];
        for &(lo, hi, m, loose, want) in cases {
            let got = extended(lo, hi, m, loose);
            assert!(
                close(&got, want),
                "extended({lo}, {hi}, {m}, {loose}) = {got:?}, want {want:?}"
            );
        }
    }

    #[test]
    fn labels_are_clean_for_fractional_steps() {
        assert_eq!(
            format_ticks(&[0.0, 0.1, 0.2, 0.30000000000000004], 0.1),
            ["0.0", "0.1", "0.2", "0.3"]
        );
        assert_eq!(
            format_ticks(&[0.0, 0.25, 0.5], 0.25),
            ["0.00", "0.25", "0.50"]
        );
        assert_eq!(format_ticks(&[0.0, 2.5, 5.0], 2.5), ["0.0", "2.5", "5.0"]);
        assert_eq!(
            format_ticks(&[-40.0, 0.0, 40.0], 40.0),
            ["\u{2212}40", "0", "40"]
        );
    }

    #[test]
    fn zero_never_signed() {
        assert_eq!(format_ticks(&[-1e-17, 0.5], 0.5), ["0.0", "0.5"]);
        assert_eq!(format_ticks(&[-0.0], 1.0), ["0"]);
    }

    #[test]
    fn huge_and_tiny_values_use_scientific_form() {
        assert_eq!(format_ticks(&[0.0, 5e6, 1e7], 5e6), ["0", "5e6", "1e7"]);
        assert_eq!(
            format_ticks(&[-3e-5, 0.0, 3e-5], 3e-5),
            ["\u{2212}3e\u{2212}5", "0", "3e\u{2212}5"]
        );
    }
}
