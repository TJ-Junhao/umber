//! Tick positions (Extended Wilkinson) and tick label formatting.
//!
//! Ticks are produced by [`ContinuousScale::ticks`](crate::ContinuousScale::ticks). Labels are
//! clean (`0.3`, never `0.30000000000000004`), use U+2212 for negatives, and never read `-0`.
//!
//! # Examples
//!
//! ```
//! use umber_core::{ContinuousScale, Transform};
//!
//! let scale = ContinuousScale::new(Transform::Linear, -40.0, 120.0, "doc").unwrap();
//! let labels: Vec<String> = scale.ticks(5).into_iter().map(|t| t.label).collect();
//! assert_eq!(labels, ["\u{2212}40", "0", "40", "80", "120"]);
//! ```

use std::collections::HashMap;

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

/// Where tick values come from, which decides whether a value near zero means zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TickValues {
    /// Values computed on an evenly spaced linear grid with this step, as `start + i * step`: a
    /// value within `step * 1e-9` of zero is a zero that missed by a rounding error.
    Grid {
        /// The grid step.
        step: f64,
    },
    /// Exact values: positions a caller chose, and log ticks.
    Exact,
}

/// Automatic ticks, with where their values come from: the step their labels were written for
/// on a linear grid, so that another tick format decides zero exactly as the automatic labels
/// did.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AutoTicks {
    /// The ticks, with their automatic labels.
    pub ticks: Vec<Tick>,
    /// Where their values come from.
    pub values: TickValues,
}

/// Largest number of ticks any function here aims for; larger targets are clamped to it.
pub(crate) const MAX_TARGET: usize = 64;

/// Clamps a caller-supplied tick target to `2..=MAX_TARGET`.
pub(crate) fn clamp_target(target: usize) -> usize {
    target.clamp(2, MAX_TARGET)
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

/// `1 - 0.5 * sum_sq / (0.1 * range)^2`, where `sum_sq` is the sum of the squared `diffs`.
///
/// The direct form reproduces the reference implementation bit for bit, which matters because
/// candidates can tie exactly. When a square would overflow or underflow (spans below about
/// 1e-150 or above about 1e150), each difference is divided by `0.1 * range` before squaring.
fn coverage_penalty(range: f64, diffs: [f64; 2]) -> f64 {
    let den = (0.1 * range).powi(2);
    let num = diffs[0].powi(2) + diffs[1].powi(2);
    if den.is_normal() && num.is_finite() {
        1.0 - 0.5 * num / den
    } else {
        let scale = 0.1 * range;
        let (a, b) = (diffs[0] / scale, diffs[1] / scale);
        1.0 - 0.5 * (a * a + b * b)
    }
}

fn coverage(dmin: f64, dmax: f64, lmin: f64, lmax: f64) -> f64 {
    coverage_penalty(dmax - dmin, [dmax - lmax, dmin - lmin])
}

fn coverage_max(dmin: f64, dmax: f64, span: f64) -> f64 {
    let r = dmax - dmin;
    if span > r {
        let half = (span - r) / 2.0;
        coverage_penalty(r, [half, half])
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
    let n = clamp_target(n);
    let last = (n - 1) as f64;
    (0..n)
        .map(|i| {
            let t = i as f64 / last;
            lo * (1.0 - t) + hi * t
        })
        .collect()
}

/// Which tick sets [`extended`] may choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TickCoverage {
    /// Loose labeling: the ticks cover the range, so the first is `<= dmin` and the last is
    /// `>= dmax`.
    Loose,
    /// Any tick set, which may stop short of either end of the range.
    Free,
}

/// Extended Wilkinson tick positions for `[dmin, dmax]` (Talbot, Lin & Hanrahan, 2010).
///
/// `tick_coverage` says whether the ticks must cover the range. Values are computed as
/// `start * step`, never by accumulation. Invalid input (non-finite or `dmin >= dmax`) and the
/// case where no candidate is found yield `max(target, 2)` evenly spaced values instead.
pub(crate) fn extended(
    dmin: f64,
    dmax: f64,
    target: usize,
    tick_coverage: TickCoverage,
) -> Vec<f64> {
    let target = clamp_target(target);
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
                        let step = jf * q * 10f64.powf(f64::from(z));
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
                                        && (tick_coverage == TickCoverage::Free
                                            || (lmin <= dmin && lmax >= dmax))
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

/// Most decimals tried in fixed form before falling back to shortest round-trip.
const MAX_DECIMALS: usize = 15;
/// Significant digits tried in scientific form: from the default up to the `f64` limit.
const SCIENTIFIC_DIGITS: std::ops::RangeInclusive<usize> = 12..=17;

/// Scientific form `<mantissa>e<exp>` of a non-zero `v`. `digits` is the number of significant
/// digits (trailing zeros are trimmed), or `None` for the shortest round-trip mantissa.
fn scientific(v: f64, digits: Option<usize>) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    let text = match digits {
        None => format!("{v:e}"),
        Some(n) => {
            let rounded = format!("{v:.prec$e}", prec = n.saturating_sub(1));
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
        }
    };
    text.replace('-', "\u{2212}")
}

/// Fixed-point form of `v`: `decimals` places, or shortest round-trip for `None`. A value that
/// rounds to zero prints without a sign, and negatives use U+2212.
fn fixed(v: f64, decimals: Option<usize>) -> String {
    let text = match decimals {
        Some(d) => format!("{v:.d$}"),
        None => format!("{v}"),
    };
    let unsigned = text.trim_start_matches('-');
    if unsigned.chars().all(|c| c == '0' || c == '.') {
        unsigned.to_string()
    } else {
        text.replace('-', "\u{2212}")
    }
}

/// True when no two distinct values share a label.
fn labels_are_unique(values: &[f64], labels: &[String]) -> bool {
    let mut seen: HashMap<&str, u64> = HashMap::with_capacity(labels.len());
    values.iter().zip(labels).all(|(v, label)| {
        let bits = v.to_bits();
        *seen.entry(label.as_str()).or_insert(bits) == bits
    })
}

/// Fixed-point labels with `start` decimals, raising the decimals one at a time (up to
/// [`MAX_DECIMALS`], then shortest round-trip) until distinct values get distinct labels.
fn unique_fixed(values: &[f64], start: usize) -> Vec<String> {
    for d in start..=MAX_DECIMALS {
        let labels: Vec<String> = values.iter().map(|&v| fixed(v, Some(d))).collect();
        if labels_are_unique(values, &labels) {
            return labels;
        }
    }
    values.iter().map(|&v| fixed(v, None)).collect()
}

/// Scientific labels (zero stays `0`), raising the significant digits one at a time until
/// distinct values get distinct labels, then shortest round-trip.
fn unique_scientific(values: &[f64]) -> Vec<String> {
    let label = |v: f64, digits: Option<usize>| {
        if v == 0.0 {
            "0".to_string()
        } else {
            scientific(v, digits)
        }
    };
    for digits in SCIENTIFIC_DIGITS {
        let labels: Vec<String> = values.iter().map(|&v| label(v, Some(digits))).collect();
        if labels_are_unique(values, &labels) {
            return labels;
        }
    }
    values.iter().map(|&v| label(v, None)).collect()
}

/// Labels for tick `values` spaced `step` apart (the grid policy).
///
/// The number of decimals follows from `step`, values within `step * 1e-9` of zero print as an
/// unsigned zero, negatives use U+2212, and a set containing a value of at least `1e6` or a
/// non-zero value below `1e-4` is written in scientific form. If distinct values would share a
/// label, the precision is raised until they do not.
pub(crate) fn format_ticks(values: &[f64], step: f64) -> Vec<String> {
    let step = step.abs();
    grid_labels(&snapped_to_zero(values, step), decimals(step))
}

/// `values` with every value within `step * 1e-9` of zero set to an unsigned zero: ticks
/// computed as `start + i * step` on a grid of that `step` can miss zero by a rounding error,
/// and such a tick means zero. Only computed grid ticks are snapped; values a caller chose, and
/// log ticks, are exact.
pub(crate) fn snapped_to_zero(values: &[f64], step: f64) -> Vec<f64> {
    let tiny = step.abs() * 1e-9;
    values
        .iter()
        .map(|&v| if v == 0.0 || v.abs() < tiny { 0.0 } else { v })
        .collect()
}

/// Grid-policy labels for `values` with `places` decimals: scientific form when a non-zero value
/// is at least `1e6` or below `1e-4`, fixed-point otherwise, with the precision raised until
/// distinct values get distinct labels.
fn grid_labels(values: &[f64], places: usize) -> Vec<String> {
    let use_scientific = values
        .iter()
        .any(|&v| v != 0.0 && (v.abs() >= 1e6 || v.abs() < 1e-4));
    if use_scientific {
        unique_scientific(values)
    } else {
        unique_fixed(values, places)
    }
}

/// Labels for tick values that a caller chose rather than a tick search, by the grid policy of
/// [`format_ticks`], for the values exactly as given. Such values need not sit on a regular
/// grid, so the decimals are the finest place any of them needs (`0.25` and `1.5` give two):
/// `[0, 1.5, 3.25]` reads `0.00`, `1.50`, `3.25`. No value is snapped to zero, so tiny values
/// such as `1e-24` choose the scientific form and keep labels of their own.
pub(crate) fn format_given(values: &[f64]) -> Vec<String> {
    let places = values
        .iter()
        .filter(|v| **v != 0.0)
        .map(|v| decimals(v.abs()))
        .max()
        .unwrap_or(0);
    // Only the sign of a zero changes.
    let values: Vec<f64> = values
        .iter()
        .map(|&v| if v == 0.0 { 0.0 } else { v })
        .collect();
    grid_labels(&values, places)
}

/// Fixed-point labels with exactly `decimals` places for every value as given, even when two
/// values then share a label. A value that rounds to zero prints without a sign, and negatives
/// use U+2212.
pub(crate) fn format_fixed(values: &[f64], decimals: usize) -> Vec<String> {
    values.iter().map(|&v| fixed(v, Some(decimals))).collect()
}

/// Scientific labels (`2.5e6`, `1e−5`) with the default 12 significant digits, trailing zeros
/// trimmed, for every value as given, even when two values then share a label. Zero is `0`.
pub(crate) fn format_scientific(values: &[f64]) -> Vec<String> {
    values
        .iter()
        .map(|&v| {
            if v == 0.0 {
                "0".to_string()
            } else {
                scientific(v, Some(*SCIENTIFIC_DIGITS.start()))
            }
        })
        .collect()
}

/// `v` rounded to `digits` significant digits.
fn round_significant(v: f64, digits: usize) -> f64 {
    format!("{v:.prec$e}", prec = digits.saturating_sub(1))
        .parse()
        .unwrap_or(v)
}

/// Labels for values that do not sit on a regular grid (the value policy).
///
/// The whole set uses one form: scientific (`<mantissa>e<exp>`) when any non-zero value lies
/// outside `1e-4 <= |v| < 1e6`, otherwise fixed. Zero is `0`. Values are written with 12
/// significant digits (trailing zeros trimmed, so `1.1^2` reads `1.21`); if distinct values
/// would collide, with 17, and finally with the shortest round-trip digits, which always
/// separates distinct values.
pub(crate) fn format_values(values: &[f64]) -> Vec<String> {
    let use_scientific = values
        .iter()
        .any(|&v| v != 0.0 && !(1e-4..1e6).contains(&v.abs()));
    let label = |v: f64, digits: Option<usize>| -> String {
        if v == 0.0 {
            "0".to_string()
        } else if use_scientific {
            scientific(v, digits)
        } else {
            fixed(digits.map_or(v, |n| round_significant(v, n)), None)
        }
    };
    for digits in [12, 17] {
        let labels: Vec<String> = values.iter().map(|&v| label(v, Some(digits))).collect();
        if labels_are_unique(values, &labels) {
            return labels;
        }
    }
    values.iter().map(|&v| label(v, None)).collect()
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
        use TickCoverage::{Free, Loose};
        // (dmin, dmax, target, coverage) -> expected, produced by the verified Python port.
        let cases: &[(f64, f64, usize, TickCoverage, &[f64])] = &[
            (0.0, 100.0, 5, Free, &[0.0, 25.0, 50.0, 75.0, 100.0]),
            (0.0, 100.0, 4, Free, &[0.0, 50.0, 100.0]),
            (-1.5, 1.5, 4, Free, &[-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5]),
            (0.0, 30.0, 5, Free, &[0.0, 10.0, 20.0, 30.0]),
            (0.0, 25.0, 4, Free, &[0.0, 5.0, 10.0, 15.0, 20.0, 25.0]),
            (0.5, 9.5, 5, Free, &[0.0, 2.5, 5.0, 7.5, 10.0]),
            (3.2, 7.9, 5, Free, &[3.0, 4.0, 5.0, 6.0, 7.0, 8.0]),
            (0.0, 1.0, 5, Free, &[0.0, 0.25, 0.5, 0.75, 1.0]),
            (-37.0, 112.0, 5, Free, &[-40.0, 0.0, 40.0, 80.0, 120.0]),
            (0.001, 0.0027, 5, Free, &[0.001, 0.0015, 0.002, 0.0025]),
            (
                0.001,
                0.0027,
                5,
                Loose,
                &[0.001, 0.0015, 0.002, 0.0025, 0.003],
            ),
            (1200.0, 98000.0, 4, Free, &[0.0, 50000.0, 100000.0]),
            (0.0, 8.0, 4, Free, &[0.0, 2.5, 5.0, 7.5]),
            (0.0, 8.0, 4, Loose, &[0.0, 2.0, 4.0, 6.0, 8.0]),
            // Random ranges where `powi(10, z)` diverged from the Python reference.
            (
                -2.6178863563145905e68,
                2.301814660011723e68,
                2,
                Loose,
                &[-3e68, 3e68],
            ),
            (
                -5.219454767136544e37,
                4.908864837878777e37,
                6,
                Loose,
                &[-6e37, -3e37, 0.0, 3e37, 6e37],
            ),
            (
                -1.161350009461411e101,
                2.0915500407958136e100,
                8,
                Free,
                &[-1.25e101, -1e101, -7.5e100, -5e100, -2.5e100, 0.0, 2.5e100],
            ),
        ];
        for &(lo, hi, m, coverage, want) in cases {
            let got = extended(lo, hi, m, coverage);
            assert!(
                close(&got, want),
                "extended({lo}, {hi}, {m}, {coverage:?}) = {got:?}, want {want:?}"
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
    fn scientific_labels_keep_enough_digits() {
        assert_eq!(
            format_ticks(&[0.0, 2.5e6, 5e6], 2.5e6),
            ["0", "2.5e6", "5e6"]
        );
        assert_eq!(
            format_ticks(&[0.0, 1.5e-5, 3e-5], 1.5e-5),
            ["0", "1.5e\u{2212}5", "3e\u{2212}5"]
        );
    }

    #[test]
    fn colliding_labels_raise_precision() {
        let labels = format_ticks(&[1.0, 1.0 + 5e-13, 1.0 + 1e-12], 1e-12);
        assert_eq!(labels.len(), 3);
        assert!(labels[0] != labels[1] && labels[1] != labels[2]);
        let labels = format_ticks(&[1e9, 1e9 + 2.5e-4], 2.5e-4);
        assert!(labels[0] != labels[1], "{labels:?}");
    }

    #[test]
    fn value_policy_uses_one_form_per_axis() {
        // Any value outside [1e-4, 1e6) puts the whole set in scientific form.
        assert_eq!(
            format_values(&[0.0, 3.375, -0.001, 1e6, 2.5e-5, -3e-5]),
            [
                "0",
                "3.375e0",
                "\u{2212}1e\u{2212}3",
                "1e6",
                "2.5e\u{2212}5",
                "\u{2212}3e\u{2212}5"
            ]
        );
        assert_eq!(
            format_values(&[0.0, 3.375, -0.001, 999999.0]),
            ["0", "3.375", "\u{2212}0.001", "999999"]
        );
    }

    #[test]
    fn value_policy_trims_float_noise_unless_labels_collide() {
        assert_eq!(format_values(&[1.1, 1.1 * 1.1]), ["1.1", "1.21"]);
        // Distinct at 17 digits but not at 12.
        let labels = format_values(&[1.0, 1.0 + f64::EPSILON]);
        assert_eq!(labels, ["1", "1.0000000000000002"]);
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
