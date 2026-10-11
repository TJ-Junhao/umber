//! Scales: how values along one axis map to positions in `[0, 1]`.
//!
//! A [`ContinuousScale`] maps numbers (linear or logarithmic) and produces ticks; a
//! [`CategoryScale`] maps named categories to equal bands.
//!
//! # Examples
//!
//! ```
//! use umber_core::{CategoryScale, ContinuousScale, LogBase, Transform};
//!
//! let log = ContinuousScale::new(Transform::Log(LogBase::TEN), 1.0, 100.0, "doc").unwrap();
//! assert!((log.normalize(10.0) - 0.5).abs() < 1e-12);
//!
//! let categories = CategoryScale::from_first_appearance(["a", "b"]);
//! assert_eq!(categories.band(1), (0.5, 1.0));
//! ```

use std::collections::HashMap;

use crate::ticks::{self, Tick, TickCoverage};
use crate::{Error, Result};

/// How values along one axis map to positions.
///
/// In v0.1 the Rust API checks scale and data compatibility at runtime; compile-time typing of
/// axes is provided by the Python layer.
///
/// # Examples
///
/// ```
/// use umber_core::{LogBase, ScaleKind};
///
/// let kind = ScaleKind::Log { base: LogBase::TEN };
/// assert_ne!(kind, ScaleKind::Linear);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive] // room for `Time` later without a breaking change
pub enum ScaleKind {
    /// Values map linearly.
    Linear,
    /// Values map by their logarithm in the given base.
    Log {
        /// The logarithm base.
        base: LogBase,
    },
    /// Discrete categories; order = first appearance across marks, resolved at render time.
    Category,
}

impl ScaleKind {
    /// The value transform of a linear or log axis; `None` for a category axis.
    pub(crate) fn transform(self) -> Option<Transform> {
        match self {
            ScaleKind::Linear => Some(Transform::Linear),
            ScaleKind::Log { base } => Some(Transform::Log(base)),
            ScaleKind::Category => None,
        }
    }
}

/// A logarithm base, guaranteed finite and > 1.
///
/// # Examples
///
/// ```
/// use umber_core::LogBase;
///
/// assert_eq!(LogBase::TEN.get(), 10.0);
/// assert_eq!(LogBase::new(2.0).unwrap().get(), 2.0);
/// assert!(LogBase::new(1.0).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogBase(f64); // private field: only constructible through `new`

impl LogBase {
    /// Base ten.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::LogBase;
    ///
    /// assert_eq!(LogBase::TEN.get(), 10.0);
    /// ```
    pub const TEN: LogBase = LogBase(10.0);

    /// Creates a base, or `Error::Style` unless `base` is finite and greater than 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::LogBase;
    ///
    /// assert!(LogBase::new(2.0).is_ok());
    /// let err = LogBase::new(0.5).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "Scale.log(): base is 0.5; a log base must be finite and greater than 1."
    /// );
    /// ```
    pub fn new(base: f64) -> Result<LogBase> {
        if base.is_finite() && base > 1.0 {
            Ok(LogBase(base))
        } else {
            Err(Error::style(
                "Scale.log()",
                format!("base is {base}; a log base must be finite and greater than 1."),
            )
            .with_parameter("base"))
        }
    }

    /// The base as a number.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::LogBase;
    ///
    /// assert_eq!(LogBase::TEN.get(), 10.0);
    /// ```
    pub fn get(self) -> f64 {
        self.0
    }
}

/// The value transform of a [`ContinuousScale`].
///
/// # Examples
///
/// ```
/// use umber_core::{LogBase, Transform};
///
/// assert_ne!(Transform::Linear, Transform::Log(LogBase::TEN));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transform {
    /// Values map linearly.
    Linear,
    /// Values map by their logarithm in the given base.
    Log(LogBase),
}

/// The largest magnitude a [`ContinuousScale`] bound can have.
///
/// # Examples
///
/// ```
/// use umber_core::{ContinuousScale, Transform, MAX_MAGNITUDE};
///
/// assert!(ContinuousScale::new(Transform::Linear, 0.0, MAX_MAGNITUDE, "doc").is_ok());
/// assert!(ContinuousScale::new(Transform::Linear, 0.0, 1e308, "doc").is_err());
/// ```
pub const MAX_MAGNITUDE: f64 = 1e300;

/// The smallest positive value a log [`ContinuousScale`] can show.
///
/// # Examples
///
/// ```
/// use umber_core::{ContinuousScale, LogBase, Transform, MIN_LOG_VALUE};
///
/// let log = Transform::Log(LogBase::TEN);
/// assert!(ContinuousScale::new(log, MIN_LOG_VALUE, 1.0, "doc").is_ok());
/// assert!(ContinuousScale::new(log, 1e-310, 1.0, "doc").is_err());
/// ```
pub const MIN_LOG_VALUE: f64 = 1e-300;

/// A log domain must span at least this ratio, or `ln(hi) - ln(lo)` loses all precision.
const MIN_LOG_RATIO: f64 = 1.0 + 1e-9;

/// A linear domain must be at least this share of its larger limit's magnitude wide. One `f64`
/// ulp is then at most about `2.2e-16 / 1e-10`, or 2.2e-6, of the domain, so positions are
/// resolvable to about 1.1e-6 of the plot extent (4.4e-4 pt on a 400 pt plot).
const MIN_LINEAR_RELATIVE_WIDTH: f64 = 1e-10;

/// A linear domain must also be at least this wide, which keeps the same resolution near 0
/// (where the relative bound alone would allow a range of a few subnormals).
const MIN_LINEAR_WIDTH: f64 = 1e-290;

/// Whether `lo..=hi` (with `lo <= hi`, both finite) is too narrow for a linear domain: narrower
/// than `max(1e-10 × max(|lo|, |hi|), 1e-290)`. A single value (`lo == hi`) is too narrow.
pub(crate) fn too_narrow(lo: f64, hi: f64) -> bool {
    let magnitude = lo.abs().max(hi.abs());
    hi - lo < (MIN_LINEAR_RELATIVE_WIDTH * magnitude).max(MIN_LINEAR_WIDTH)
}

/// Whether the data extent `lo..=hi` (finite, `lo <= hi`) counts as a single value: a linear
/// extent narrower than [`ContinuousScale::new`] allows, or a log extent narrower than a factor
/// of `1 + 1e-9`. [`ContinuousScale::from_extent`] pads such an extent on its own terms.
pub(crate) fn is_single_value(transform: Transform, lo: f64, hi: f64) -> bool {
    match transform {
        Transform::Linear => lo == hi || too_narrow(lo, hi),
        Transform::Log(_) => hi / lo < MIN_LOG_RATIO,
    }
}

/// The share of a nice domain's span that auto limits add on each side, so that marks never
/// sit on the plot edge.
const AUTO_LIMIT_PADDING: f64 = 0.05;

/// Which sides of a nice domain [`ContinuousScale::padded`] widens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Padding {
    /// Both sides.
    BothSides,
    /// Both sides, except a side whose limit is exactly 0 on a linear domain: bars grow from 0,
    /// so their base stays on the plot edge. A log domain never has 0 as a limit, so it is
    /// padded on both sides.
    KeepZero,
}

/// `ln(v / lo)` for positive `v` and `lo`: the log-axis position of `v` measured from the
/// lower limit, in natural-log units. Measuring from `lo` keeps narrow log domains at large
/// magnitudes resolvable: `ln(1e200 * (1 + 1e-8))` and `ln(1e200)` are only about 175 000 ulps
/// apart. Within a factor of 2 of `lo`, `v - lo` is exact, so `ln_1p((v - lo) / lo)` has full
/// relative precision even when `v / lo` is within a few ulps of 1. Further out `ln(v / lo)`
/// is used, and when `v / lo` is not a normal `f64` (beyond about `1e±308`) the difference of
/// the logarithms; such values are far from `lo`, where that rounding is negligible.
pub(crate) fn log_offset(v: f64, lo: f64) -> f64 {
    let ratio = v / lo;
    if (0.5..=2.0).contains(&ratio) {
        ((v - lo) / lo).ln_1p()
    } else if ratio.is_normal() {
        ratio.ln()
    } else {
        v.ln() - lo.ln()
    }
}

/// A numeric axis: a transform plus a domain `[lo, hi]` that maps onto `[0, 1]`.
///
/// # Examples
///
/// ```
/// use umber_core::{ContinuousScale, Transform};
///
/// let scale = ContinuousScale::new(Transform::Linear, -10.0, 30.0, "doc").unwrap();
/// assert_eq!(scale.normalize(10.0), 0.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContinuousScale {
    transform: Transform,
    lo: f64,
    hi: f64,
}

/// Domains narrower than this relative or absolute width, or wider than the upper bound, get
/// evenly spaced scientific-form ticks instead of the Extended Wilkinson search.
fn is_degenerate(lo: f64, hi: f64) -> bool {
    let span = hi - lo;
    let magnitude = lo.abs().max(hi.abs()).max(f64::MIN_POSITIVE);
    span < 1e-12 * magnitude || !(1e-200..=1e200).contains(&span)
}

fn power(base: f64, e: i64) -> f64 {
    base.powf(e as f64)
}

/// The smallest `e` with `base^e >= v`.
fn ceil_exp(v: f64, base: f64) -> Option<i64> {
    let e = (v.ln() / base.ln()).ceil();
    if !e.is_finite() {
        return None;
    }
    let mut e = e as i64;
    if power(base, e.saturating_sub(1)) >= v {
        e = e.saturating_sub(1);
    } else if power(base, e) < v {
        e = e.saturating_add(1);
    }
    Some(e)
}

/// The largest `e` with `base^e <= v`.
fn floor_exp(v: f64, base: f64) -> Option<i64> {
    let e = (v.ln() / base.ln()).floor();
    if !e.is_finite() {
        return None;
    }
    let mut e = e as i64;
    if power(base, e) > v {
        e = e.saturating_sub(1);
    } else if power(base, e.saturating_add(1)) <= v {
        e = e.saturating_add(1);
    }
    Some(e)
}

/// The next representable `f64` above a finite `x` (`f64::next_up` needs Rust 1.86).
fn next_up(x: f64) -> f64 {
    if x == 0.0 {
        return f64::from_bits(1);
    }
    let bits = x.to_bits();
    f64::from_bits(if x > 0.0 { bits + 1 } else { bits - 1 })
}

/// The next representable `f64` below a finite `x`.
fn next_down(x: f64) -> f64 {
    -next_up(-x)
}

/// Drops values that do not rise strictly above their predecessor.
fn strictly_increasing(values: Vec<f64>) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::with_capacity(values.len());
    for v in values {
        if out.last().is_none_or(|last| v > *last) {
            out.push(v);
        }
    }
    out
}

impl ContinuousScale {
    /// Creates a scale over `[lo, hi]`.
    ///
    /// Returns `Error::Style` (with the caller's `context`) unless both limits are finite and
    /// `lo < hi`. Linear limits must not exceed [`MAX_MAGNITUDE`] in magnitude, and the range
    /// must be at least `max(1e-10 * max(|lo|, |hi|), 1e-290)` wide, which keeps positions
    /// resolvable to about 1.1e-6 of the plot extent (4.4e-4 pt on a 400 pt plot). Log limits
    /// must lie in `[MIN_LOG_VALUE, MAX_MAGNITUDE]` with `hi / lo >= 1 + 1e-9`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContinuousScale, LogBase, Transform};
    ///
    /// let scale = ContinuousScale::new(Transform::Linear, 0.0, 1.0, "Axes.xlim()").unwrap();
    /// assert_eq!(scale.domain(), (0.0, 1.0));
    /// assert!(ContinuousScale::new(Transform::Linear, 2.0, 1.0, "Axes.xlim()").is_err());
    /// assert!(ContinuousScale::new(Transform::Linear, 1.0, 1.0 + 1e-12, "Axes.xlim()").is_err());
    /// assert!(ContinuousScale::new(Transform::Log(LogBase::TEN), 0.0, 10.0, "Axes.ylim()").is_err());
    /// ```
    pub fn new(transform: Transform, lo: f64, hi: f64, context: &'static str) -> Result<Self> {
        if !(lo.is_finite() && hi.is_finite() && lo < hi) {
            return Err(Error::style(
                context,
                format!(
                    "the limits are {lo} and {hi}; both must be finite and the lower limit must be below the upper limit."
                ),
            ));
        }
        match transform {
            Transform::Linear => {
                if lo.abs() > MAX_MAGNITUDE || hi.abs() > MAX_MAGNITUDE {
                    return Err(Error::style(
                        context,
                        format!(
                            "the limits are {lo} and {hi}; their magnitude must not exceed {MAX_MAGNITUDE:e}."
                        ),
                    ));
                }
                if too_narrow(lo, hi) {
                    return Err(Error::style(
                        context,
                        format!(
                            "the limits {lo} and {hi} are too close; the range must be at least 1e-10 of their magnitude (and at least 1e-290)."
                        ),
                    ));
                }
            }
            Transform::Log(_) => {
                if lo <= 0.0 {
                    return Err(Error::style(
                        context,
                        format!(
                            "the lower limit is {lo}; a log scale needs a positive lower limit."
                        ),
                    )
                    .with_parameter("low")
                    .with_hint("use a linear axis if the range includes 0."));
                }
                if lo < MIN_LOG_VALUE || hi > MAX_MAGNITUDE {
                    return Err(Error::style(
                        context,
                        format!(
                            "the limits are {lo} and {hi}; a log scale supports values from {MIN_LOG_VALUE:e} to {MAX_MAGNITUDE:e}."
                        ),
                    ));
                }
                if hi / lo < MIN_LOG_RATIO {
                    return Err(Error::style(
                        context,
                        format!(
                            "the limits are {lo} and {hi}; the upper limit must be at least a factor of 1.000000001 above the lower limit."
                        ),
                    ));
                }
            }
        }
        Ok(ContinuousScale { transform, lo, hi })
    }

    /// Creates a scale that fits a data `extent` (`None` when there is no data).
    ///
    /// `None` gives `(0, 1)` for linear and `(1, 10)` for log. A single-valued extent is padded:
    /// `v +- 0.5` when `v == 0`, otherwise `v +- max(5%, 1e-290)` for linear, and
    /// `(v / f, v * f)` with `f = max(base, 1.1)` for log, both kept within the supported magnitudes. A
    /// linear extent narrower than [`ContinuousScale::new`] allows counts as a single value at
    /// its midpoint, and a log extent narrower than a factor of `1 + 1e-9` as a single value. Returns `Error::Data` for
    /// non-finite bounds, for bounds beyond [`MAX_MAGNITUDE`], and for a log transform when the
    /// extent starts below [`MIN_LOG_VALUE`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContinuousScale, Transform};
    ///
    /// let empty = ContinuousScale::from_extent(Transform::Linear, None, "doc").unwrap();
    /// assert_eq!(empty.domain(), (0.0, 1.0));
    /// let flat = ContinuousScale::from_extent(Transform::Linear, Some((20.0, 20.0)), "doc").unwrap();
    /// assert_eq!(flat.domain(), (19.0, 21.0));
    /// ```
    pub fn from_extent(
        transform: Transform,
        extent: Option<(f64, f64)>,
        context: &'static str,
    ) -> Result<Self> {
        let Some((lo, hi)) = extent else {
            return Ok(Self::unit(transform));
        };
        if !(lo.is_finite() && hi.is_finite()) {
            return Err(Error::data(
                context,
                format!("the data range is {lo} to {hi}; it must be finite."),
            ));
        }
        if lo > hi {
            return Err(Error::data(
                context,
                format!(
                    "the data range is {lo} to {hi}; the lower bound must not exceed the upper bound."
                ),
            ));
        }
        match transform {
            Transform::Linear => {
                if lo.abs() > MAX_MAGNITUDE || hi.abs() > MAX_MAGNITUDE {
                    return Err(Error::data(
                        context,
                        format!(
                            "the data range is {lo} to {hi}; values must not exceed {MAX_MAGNITUDE:e} in magnitude."
                        ),
                    ));
                }
            }
            Transform::Log(_) => {
                if lo <= 0.0 {
                    return Err(Error::data(
                        context,
                        format!(
                            "the data range starts at {lo}; a log scale can only show positive values."
                        ),
                    )
                    .with_hint("use a linear axis, or remove values that are not positive."));
                }
                if lo < MIN_LOG_VALUE || hi > MAX_MAGNITUDE {
                    return Err(Error::data(
                        context,
                        format!(
                            "the data range is {lo} to {hi}; a log scale supports values from {MIN_LOG_VALUE:e} to {MAX_MAGNITUDE:e}."
                        ),
                    ));
                }
            }
        }
        if !is_single_value(transform, lo, hi) {
            return Self::new(transform, lo, hi, context);
        }
        let (mut lo, mut hi) = match transform {
            Transform::Linear => {
                // Centered on the extent's midpoint. The pad is at least half of any extent that
                // counts as a single value, so the data stays inside.
                let mid = lo / 2.0 + hi / 2.0;
                let pad = if mid == 0.0 {
                    0.5
                } else {
                    (mid.abs() * 0.05).max(MIN_LINEAR_WIDTH)
                };
                (
                    (mid - pad).max(-MAX_MAGNITUDE),
                    (mid + pad).min(MAX_MAGNITUDE),
                )
            }
            Transform::Log(base) => {
                // At least 1.1, so the padded domain always satisfies the minimum ratio.
                let factor = base.get().max(1.1);
                (
                    (lo / factor).max(MIN_LOG_VALUE),
                    (lo * factor).min(MAX_MAGNITUDE),
                )
            }
        };
        // Rounding must never collapse the padded domain.
        if lo >= hi {
            if hi >= MAX_MAGNITUDE {
                lo = next_down(hi);
            } else {
                hi = next_up(lo);
            }
        }
        Self::new(transform, lo, hi, context)
    }

    /// The empty-data scale: `(0, 1)` for linear, `(1, 10)` for log. Both domains satisfy
    /// [`ContinuousScale::new`], so this cannot fail; `from_extent(transform, None, _)` returns
    /// it.
    pub(crate) fn unit(transform: Transform) -> ContinuousScale {
        let (lo, hi) = match transform {
            Transform::Linear => (0.0, 1.0),
            Transform::Log(_) => (1.0, 10.0),
        };
        ContinuousScale { transform, lo, hi }
    }

    /// The value transform.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContinuousScale, Transform};
    ///
    /// let scale = ContinuousScale::new(Transform::Linear, 0.0, 1.0, "doc").unwrap();
    /// assert_eq!(scale.transform(), Transform::Linear);
    /// ```
    pub fn transform(&self) -> Transform {
        self.transform
    }

    /// The domain `(lo, hi)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContinuousScale, Transform};
    ///
    /// let scale = ContinuousScale::new(Transform::Linear, 2.0, 4.0, "doc").unwrap();
    /// assert_eq!(scale.domain(), (2.0, 4.0));
    /// ```
    pub fn domain(&self) -> (f64, f64) {
        (self.lo, self.hi)
    }

    /// Maps `v` to a position where the domain is `[0, 1]`. Returns NaN for `v <= 0` on a log
    /// scale.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContinuousScale, LogBase, Transform};
    ///
    /// let log = ContinuousScale::new(Transform::Log(LogBase::TEN), 1.0, 100.0, "doc").unwrap();
    /// assert!((log.normalize(10.0) - 0.5).abs() < 1e-12);
    /// assert!(log.normalize(0.0).is_nan());
    /// ```
    pub fn normalize(&self, v: f64) -> f64 {
        match self.transform {
            Transform::Linear => (v - self.lo) / (self.hi - self.lo),
            Transform::Log(_) => {
                if v <= 0.0 {
                    f64::NAN
                } else {
                    // Measured from `lo` (see `log_offset`), which marks share.
                    log_offset(v, self.lo) / log_offset(self.hi, self.lo)
                }
            }
        }
    }

    /// A scale whose domain is widened to enclosing round values: the outer ticks for linear,
    /// the enclosing powers of the base for log. Degenerate domains are returned unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContinuousScale, Transform};
    ///
    /// let scale = ContinuousScale::new(Transform::Linear, 0.5, 9.5, "doc").unwrap();
    /// assert_eq!(scale.nice(5).domain(), (0.0, 10.0));
    /// ```
    pub fn nice(&self, target: usize) -> ContinuousScale {
        let target = ticks::clamp_target(target);
        let widened = match self.transform {
            Transform::Linear => {
                let values = self.loose_values(target);
                values.first().copied().zip(values.last().copied())
            }
            Transform::Log(base) => {
                let b = base.get();
                let lo = floor_exp(self.lo, b).map(|e| power(b, e));
                let hi = ceil_exp(self.hi, b).map(|e| power(b, e));
                lo.zip(hi)
            }
        };
        self.widened(widened)
    }

    /// The loose Extended Wilkinson tick values that cover a linear domain; empty when the
    /// domain is degenerate. Their ends are the nice limits.
    fn loose_values(&self, target: usize) -> Vec<f64> {
        if is_degenerate(self.lo, self.hi) {
            Vec::new()
        } else {
            ticks::extended(self.lo, self.hi, target, TickCoverage::Loose)
        }
    }

    /// This scale widened to `(lo, hi)`, never narrowed and kept within the supported
    /// magnitudes; unchanged for `None` or an unusable range.
    fn widened(&self, widened: Option<(f64, f64)>) -> ContinuousScale {
        match widened {
            Some((lo, hi)) => {
                // Never narrow the domain, whatever rounding did, and stay within the limits.
                let floor = match self.transform {
                    Transform::Linear => -MAX_MAGNITUDE,
                    Transform::Log(_) => MIN_LOG_VALUE,
                };
                let (lo, hi) = (
                    lo.min(self.lo).max(floor),
                    hi.max(self.hi).min(MAX_MAGNITUDE),
                );
                if lo.is_finite() && hi.is_finite() && lo < hi {
                    ContinuousScale {
                        transform: self.transform,
                        lo,
                        hi,
                    }
                } else {
                    *self
                }
            }
            None => *self,
        }
    }

    /// [`nice`](Self::nice) together with ticks that agree with it, so both ends of the nice
    /// domain carry a tick.
    ///
    /// Linear: the loose tick set that produced the nice limits, labelled like
    /// [`ticks`](Self::ticks). Log: the powers of the base in the nice domain, always including
    /// both domain ends; more than `2 * target` powers are thinned to `target` evenly spaced
    /// exponents that keep the first and the last. Log labels use the value policy. A degenerate
    /// domain, or a loose set with fewer than two values inside the domain, falls back to
    /// `ticks` of the nice scale.
    pub(crate) fn nice_with_ticks(&self, target: usize) -> (ContinuousScale, Vec<Tick>) {
        let target = ticks::clamp_target(target);
        let nice = self.nice(target);
        let (lo, hi) = nice.domain();
        let values = match self.transform {
            Transform::Linear => strictly_increasing(
                self.loose_values(target)
                    .into_iter()
                    .filter(|v| (lo..=hi).contains(v))
                    .collect(),
            ),
            Transform::Log(base) => nice.log_values_with_ends(base.get(), target),
        };
        if let (Some(a), Some(b)) = (values.first(), values.get(1)) {
            let labels = match self.transform {
                Transform::Linear => ticks::format_ticks(&values, b - a),
                Transform::Log(_) => ticks::format_values(&values),
            };
            return (nice, Self::pair(values, labels));
        }
        let ticks = nice.ticks(target);
        (nice, ticks)
    }

    /// This scale with its domain widened on each side by [`AUTO_LIMIT_PADDING`] of its span,
    /// measured in log space on a log axis, and kept within the supported magnitudes
    /// (`|v| <= MAX_MAGNITUDE` linear, `[MIN_LOG_VALUE, MAX_MAGNITUDE]` log). Auto limits apply
    /// it to a nice domain, after its ticks are chosen, so the ticks stay on round values and
    /// the outermost ones sit just inside the plot edge. With [`Padding::KeepZero`], a linear
    /// limit that is exactly 0 is not moved. Never narrower than `self`.
    pub(crate) fn padded(&self, padding: Padding) -> ContinuousScale {
        let (lo, hi) = (self.lo, self.hi);
        let padded = match self.transform {
            Transform::Linear => {
                // Both limits are at most MAX_MAGNITUDE in magnitude, so the span is finite.
                let pad = AUTO_LIMIT_PADDING * (hi - lo);
                let kept = |limit: f64| padding == Padding::KeepZero && limit == 0.0;
                (
                    if kept(lo) { lo } else { lo - pad },
                    if kept(hi) { hi } else { hi + pad },
                )
            }
            Transform::Log(_) => {
                // A share of the span in log space is the same factor in every base, so natural
                // logarithms serve all of them.
                let factor = (AUTO_LIMIT_PADDING * log_offset(hi, lo)).exp();
                (lo / factor, hi * factor)
            }
        };
        // `widened` keeps the result within the supported magnitudes.
        self.widened(Some(padded))
    }

    /// Powers of `base` inside a log domain, with the domain ends always first and last.
    ///
    /// The ends are normally powers themselves (a nice domain); an end that is not (clamped to
    /// the supported magnitudes) is added as a value of its own. When there are more than
    /// `2 * target` powers, `target` exponents spaced evenly from the first to the last power
    /// are kept. Interior powers within `0.5 / (target - 1)` of an added end, in normalized
    /// position, are dropped so that their labels cannot crowd it.
    fn log_values_with_ends(&self, base: f64, target: usize) -> Vec<f64> {
        let (lo, hi) = (self.lo, self.hi);
        let mut values = Vec::new();
        if let (Some(first), Some(last)) = (ceil_exp(lo, base), floor_exp(hi, base))
            && first <= last
        {
            let span = last.saturating_sub(first);
            let count = span.saturating_add(1);
            let target_i = target.max(2) as i64;
            if count <= target_i.saturating_mul(2) {
                let mut e = first;
                while e <= last {
                    values.push(power(base, e));
                    match e.checked_add(1) {
                        Some(next) => e = next,
                        None => break,
                    }
                }
            } else {
                let steps = (target_i - 1) as f64;
                for i in 0..target_i {
                    let offset = (i as f64 * span as f64 / steps).round() as i64;
                    values.push(power(base, first.saturating_add(offset)));
                }
            }
        }
        let tol = 1e-9;
        values.retain(|&v| {
            v.is_finite() && v > 0.0 && v >= lo * (1.0 - tol) && v <= hi * (1.0 + tol)
        });
        // Snap the outer powers onto the ends, or add the ends when they are not powers.
        let inserted_lo = match values.first_mut() {
            Some(v) if (*v - lo).abs() <= tol * lo => {
                *v = lo;
                false
            }
            _ => {
                values.insert(0, lo);
                true
            }
        };
        let inserted_hi = match values.last_mut() {
            Some(v) if (*v - hi).abs() <= tol * hi => {
                *v = hi;
                false
            }
            _ => {
                values.push(hi);
                true
            }
        };
        // An inserted end is not a power, so a power may sit right next to it: drop interior
        // powers within half a nominal tick gap, `0.5 / (n - 1)`, of an inserted end.
        if inserted_lo || inserted_hi {
            let gap = 0.5 / (target.max(2) - 1) as f64;
            let last = values.len().saturating_sub(1);
            let mut i = 0;
            values.retain(|&v| {
                let interior = i != 0 && i != last;
                i += 1;
                let pos = self.normalize(v);
                !(interior && ((inserted_lo && pos < gap) || (inserted_hi && 1.0 - pos < gap)))
            });
        }
        strictly_increasing(values)
    }

    /// Tick positions and labels for about `target` ticks (at least 2).
    ///
    /// Linear ticks lie inside the domain. Log ticks are the powers of the base inside the
    /// domain, thinned to every n-th power when there are more than `2 * target`; when fewer
    /// than two powers fit, they are ordinary linear ticks over the domain. Distinct tick values
    /// always get distinct labels.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContinuousScale, Transform};
    ///
    /// let scale = ContinuousScale::new(Transform::Linear, 0.5, 9.5, "doc").unwrap();
    /// let labels: Vec<String> = scale.ticks(5).into_iter().map(|t| t.label).collect();
    /// assert_eq!(labels, ["2.5", "5.0", "7.5"]);
    /// ```
    pub fn ticks(&self, target: usize) -> Vec<Tick> {
        let target = ticks::clamp_target(target);
        match self.transform {
            Transform::Linear => self.linear_ticks(target),
            Transform::Log(base) => self.log_ticks(base.get(), target),
        }
    }

    fn pair(values: Vec<f64>, labels: Vec<String>) -> Vec<Tick> {
        values
            .into_iter()
            .zip(labels)
            .map(|(value, label)| Tick { value, label })
            .collect()
    }

    fn end_ticks(&self) -> Vec<Tick> {
        let values = vec![self.lo, self.hi];
        let labels = ticks::format_values(&values);
        Self::pair(values, labels)
    }

    fn linear_ticks(&self, target: usize) -> Vec<Tick> {
        let (lo, hi) = (self.lo, self.hi);
        if is_degenerate(lo, hi) {
            let values = strictly_increasing(
                ticks::evenly_spaced(lo, hi, target)
                    .into_iter()
                    .map(|v| v.clamp(lo, hi))
                    .collect(),
            );
            let labels = ticks::format_values(&values);
            return Self::pair(values, labels);
        }
        let tol = 1e-9 * (hi - lo);
        let is_log = matches!(self.transform, Transform::Log(_));
        for t in target..=target + 6 {
            // On a log scale a tick must be a real positive value inside the domain: snapping
            // one to an endpoint would put a label on the wrong value (a zero candidate next to
            // a tiny `lo` would read "0").
            let inside: Vec<f64> = ticks::extended(lo, hi, t, TickCoverage::Free)
                .into_iter()
                .filter(|&v| {
                    if is_log {
                        v > 0.0 && v >= lo && v <= hi
                    } else {
                        v >= lo - tol && v <= hi + tol
                    }
                })
                .collect();
            if let (Some(a), Some(b)) = (inside.first(), inside.get(1)) {
                let step = b - a;
                let values = strictly_increasing(
                    inside
                        .iter()
                        .map(|&v| {
                            if is_log {
                                v
                            } else if (v - lo).abs() <= tol {
                                lo
                            } else if (v - hi).abs() <= tol {
                                hi
                            } else {
                                v
                            }
                        })
                        .collect(),
                );
                let mut labels = ticks::format_ticks(&values, step);
                if is_log {
                    // The grid formatter shows values within `step * 1e-9` of zero as `0`.
                    for (value, label) in values.iter().zip(labels.iter_mut()) {
                        if *value > 0.0 && label == "0" {
                            *label = ticks::format_values(&[*value]).concat();
                        }
                    }
                }
                return Self::pair(values, labels);
            }
        }
        self.end_ticks()
    }

    fn log_ticks(&self, base: f64, target: usize) -> Vec<Tick> {
        // Fewer than two powers of the base inside the domain cannot form an axis; fall back to
        // ordinary linear ticks over the same domain.
        let (Some(first), Some(last)) = (ceil_exp(self.lo, base), floor_exp(self.hi, base)) else {
            return self.linear_ticks(target);
        };
        if first >= last {
            return self.linear_ticks(target);
        }
        let count = last.saturating_sub(first).saturating_add(1);
        let target_i = target as i64;
        let every = if count > target_i.saturating_mul(2) {
            (count / target_i + i64::from(count % target_i != 0)).max(1)
        } else {
            1
        };
        let (floor, ceiling) = (self.lo * (1.0 - 1e-9), self.hi * (1.0 + 1e-9));
        let mut values = Vec::new();
        let mut e = first;
        while e <= last {
            let v = power(base, e);
            if v.is_finite() && v > 0.0 && v >= floor && v <= ceiling {
                values.push(v);
            }
            match e.checked_add(every) {
                Some(next) => e = next,
                None => break,
            }
        }
        if values.len() < 2 {
            return self.linear_ticks(target);
        }
        let labels = ticks::format_values(&values);
        Self::pair(values, labels)
    }
}

/// A discrete axis: categories in order of first appearance, each owning an equal band of
/// `[0, 1]`.
///
/// # Examples
///
/// ```
/// use umber_core::CategoryScale;
///
/// let scale = CategoryScale::from_first_appearance(["b", "a", "b", "c"]);
/// assert_eq!(scale.categories(), ["b", "a", "c"]);
/// assert_eq!(scale.index_of("a"), Some(1));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryScale {
    categories: Vec<String>,
    index: HashMap<String, usize>,
}

impl CategoryScale {
    /// Collects categories in order of first appearance, dropping repeats.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::CategoryScale;
    ///
    /// let scale = CategoryScale::from_first_appearance(["x", "y", "x"]);
    /// assert_eq!(scale.categories().len(), 2);
    /// ```
    pub fn from_first_appearance<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        let mut categories = Vec::new();
        let mut index = HashMap::new();
        for name in names {
            if !index.contains_key(name) {
                index.insert(name.to_string(), categories.len());
                categories.push(name.to_string());
            }
        }
        CategoryScale { categories, index }
    }

    /// The categories in order of first appearance.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::CategoryScale;
    ///
    /// let scale = CategoryScale::from_first_appearance(["b", "a"]);
    /// assert_eq!(scale.categories(), ["b", "a"]);
    /// ```
    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    /// The position of `name` among the categories, if present.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::CategoryScale;
    ///
    /// let scale = CategoryScale::from_first_appearance(["b", "a"]);
    /// assert_eq!(scale.index_of("a"), Some(1));
    /// assert_eq!(scale.index_of("z"), None);
    /// ```
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }

    /// The band `(start, end)` of category `index` within `[0, 1]`. With no categories, or an
    /// `index` out of range, the whole `(0.0, 1.0)` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::CategoryScale;
    ///
    /// let scale = CategoryScale::from_first_appearance(["a", "b"]);
    /// assert_eq!(scale.band(1), (0.5, 1.0));
    /// assert_eq!(scale.band(9), (0.0, 1.0));
    /// ```
    pub fn band(&self, index: usize) -> (f64, f64) {
        let n = self.categories.len();
        if index >= n {
            return (0.0, 1.0);
        }
        (index as f64 / n as f64, (index + 1) as f64 / n as f64)
    }

    /// The midpoint of the band of category `index`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::CategoryScale;
    ///
    /// let scale = CategoryScale::from_first_appearance(["a", "b"]);
    /// assert_eq!(scale.center(0), 0.25);
    /// ```
    pub fn center(&self, index: usize) -> f64 {
        let (start, end) = self.band(index);
        (start + end) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn lin(lo: f64, hi: f64) -> ContinuousScale {
        ContinuousScale::new(Transform::Linear, lo, hi, "test").unwrap()
    }

    /// A linear scale without validation: domains narrower than `new` allows, which the tick
    /// and nice code must still handle.
    fn raw(lo: f64, hi: f64) -> ContinuousScale {
        ContinuousScale {
            transform: Transform::Linear,
            lo,
            hi,
        }
    }

    /// Asserts that `nice_with_ticks` agrees with `nice` and puts a tick on both domain ends.
    #[track_caller]
    fn assert_nice_ticks_agree(scale: ContinuousScale, target: usize) -> Vec<Tick> {
        let (nice, ticks) = scale.nice_with_ticks(target);
        assert_eq!(nice, scale.nice(target));
        let (lo, hi) = nice.domain();
        assert_eq!(ticks.first().map(|t| t.value), Some(lo), "{ticks:?}");
        assert_eq!(ticks.last().map(|t| t.value), Some(hi), "{ticks:?}");
        assert!(ticks.windows(2).all(|w| w[0].value < w[1].value));
        ticks
    }

    #[test]
    fn nice_with_ticks_puts_ticks_on_both_ends() {
        // `ticks(5)` of the nice domain (10, 90) would give 25, 50, 75 only.
        let scale = lin(12.0, 88.0);
        assert_eq!(scale.nice(5).domain(), (10.0, 90.0));
        let ticks = assert_nice_ticks_agree(scale, 5);
        let labels: Vec<&str> = ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["10", "30", "50", "70", "90"]);

        for (lo, hi) in [
            (3.0, 97.0),
            (13.0, 77.0),
            (-3.0, 17.0),
            (0.5, 3.3),
            (1e-7, 3e-7),
        ] {
            for target in 3..=8 {
                assert_nice_ticks_agree(lin(lo, hi), target);
            }
        }

        let log = ContinuousScale::new(Transform::Log(LogBase::TEN), 3.0, 4.2e4, "test").unwrap();
        let ticks = assert_nice_ticks_agree(log, 5);
        assert_eq!(ticks.first().map(|t| t.label.as_str()), Some("1"));
        assert_eq!(ticks.len(), 6);
    }

    #[test]
    fn nice_log_ticks_keep_both_ends_when_thinning() {
        let e = std::f64::consts::E;
        // (base, lo, hi) spanning few to very many powers.
        let cases = [
            (10.0, 3.0, 4.2e12),
            (10.0, 3.0, 4.2e4),
            (10.0, 1e-200, 1e250),
            (10.0, 0.5, 2.0),
            (2.0, 3.0, 5000.0),
            (2.0, 1e-30, 1e40),
            (2.0, 1e-300, 1e300),
            (e, 0.2, 9000.0),
            (e, 1e-100, 1e120),
        ];
        for (base, lo, hi) in cases {
            let scale =
                ContinuousScale::new(Transform::Log(LogBase::new(base).unwrap()), lo, hi, "test")
                    .unwrap();
            for target in [2, 3, 5, 8] {
                let ticks = assert_nice_ticks_agree(scale, target);
                let labels: Vec<&String> = ticks.iter().map(|t| &t.label).collect();
                let unique: std::collections::HashSet<&String> = labels.iter().copied().collect();
                assert_eq!(unique.len(), labels.len(), "{labels:?}");
                assert!(
                    ticks.len() <= (2 * target).max(2) + 2,
                    "{base} {lo} {hi} {target}: {labels:?}"
                );
            }
        }
        // Regression case: 1 .. 1e13 keeps 1e13.
        let scale =
            ContinuousScale::new(Transform::Log(LogBase::TEN), 3.0, 4.2e12, "test").unwrap();
        let (nice, ticks) = scale.nice_with_ticks(5);
        assert_eq!(nice.domain(), (1.0, 1e13));
        let labels: Vec<&str> = ticks.iter().map(|t| t.label.as_str()).collect();
        // Scientific form applies to the whole axis.
        // Exponents 13 * i / 4, rounded: 0, 3, 7, 10, 13.
        assert_eq!(labels, ["1e0", "1e3", "1e7", "1e10", "1e13"]);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        #[test]
        fn nice_log_ticks_start_and_end_on_the_domain(
            base in prop_oneof![Just(2.0), Just(std::f64::consts::E), Just(10.0), 1.05_f64..1000.0],
            lo_exp in -300.0_f64..300.0,
            span in 1e-6_f64..600.0,
            target in 0_usize..20,
        ) {
            let lo = 10_f64.powf(lo_exp);
            let hi = (lo * 10_f64.powf(span)).min(MAX_MAGNITUDE);
            let Ok(scale) = ContinuousScale::new(
                Transform::Log(LogBase::new(base).unwrap()), lo, hi, "test",
            ) else {
                return Ok(());
            };
            let (nice, ticks) = scale.nice_with_ticks(target);
            prop_assert_eq!(nice, scale.nice(target));
            let (dlo, dhi) = nice.domain();
            prop_assert_eq!(ticks.first().map(|t| t.value), Some(dlo));
            prop_assert_eq!(ticks.last().map(|t| t.value), Some(dhi));
            prop_assert!(ticks.windows(2).all(|w| w[0].value < w[1].value));
        }
    }

    #[test]
    fn added_log_ends_drop_crowding_powers() {
        for base in [2.0, std::f64::consts::E, 999.0] {
            let log = Transform::Log(LogBase::new(base).unwrap());
            let scale = ContinuousScale::new(log, 1e-300, 1e300, "test").unwrap();
            for target in [3, 5, 8] {
                let (nice, ticks) = scale.nice_with_ticks(target);
                // Both ends are clamped, not powers, and are kept exactly.
                assert_eq!(nice.domain(), (1e-300, 1e300));
                assert_eq!(ticks.first().map(|t| t.value), Some(1e-300));
                assert_eq!(ticks.last().map(|t| t.value), Some(1e300));
                let gap = 0.5 / (target - 1) as f64;
                let pos: Vec<f64> = ticks.iter().map(|t| nice.normalize(t.value)).collect();
                for p in &pos[1..pos.len() - 1] {
                    assert!(*p >= gap && 1.0 - *p >= gap, "base {base}: {pos:?}");
                }
            }
        }
    }

    #[test]
    fn linear_limits_must_not_be_too_close() {
        let next_up = |v: f64| f64::from_bits(v.to_bits() + 1);
        for (lo, hi) in [
            (1.0, next_up(1.0)),
            (0.0, 5e-324),
            (1e200, next_up(1e200)),
            (1.0, 1.0 + 1e-11),
            (-3e9, -3e9 + 0.1),
            (0.0, 1e-291),
        ] {
            match ContinuousScale::new(Transform::Linear, lo, hi, "Axes.xlim()") {
                Err(Error::Style {
                    context, message, ..
                }) => {
                    assert_eq!(context, "Axes.xlim()");
                    assert!(
                        message.contains("are too close") && message.contains("1e-10"),
                        "{message}"
                    );
                }
                other => panic!("{lo}..{hi}: {other:?}"),
            }
            // As a data extent it is a single value at its midpoint, padded.
            let s = ContinuousScale::from_extent(Transform::Linear, Some((lo, hi)), "t").unwrap();
            let (dlo, dhi) = s.domain();
            assert!(dlo < lo && dhi > hi, "{lo}..{hi}: {dlo}..{dhi}");
            assert!(!too_narrow(dlo, dhi));
        }
        // The bounds themselves are allowed.
        for (lo, hi) in [
            (1.0, 1.0 + 1e-10),
            (0.0, 1e-290),
            (-1e-290, 1e-290),
            (1e200, 1e200 * (1.0 + 2e-10)),
            (-3e9, -3e9 + 0.3),
        ] {
            assert!(
                ContinuousScale::new(Transform::Linear, lo, hi, "t").is_ok(),
                "{lo}..{hi}"
            );
        }
        // Log limits keep their own ratio rule.
        let log = Transform::Log(LogBase::TEN);
        assert!(ContinuousScale::new(log, 1e200, 1e200 * (1.0 + 2e-9), "t").is_ok());
        assert!(ContinuousScale::new(log, 1e200, next_up(1e200), "t").is_err());
    }

    #[test]
    fn log_normalization_is_measured_from_the_lower_limit() {
        let log = Transform::Log(LogBase::TEN);
        // The narrowest allowed ratio at 1e299; the reference is a 100-digit computation.
        let s = ContinuousScale::new(log, 1.385e299, 1.385_000_001_385_000_2e299, "t").unwrap();
        let pos = s.normalize(1.385_000_000_989_286e299);
        assert!((pos - 0.714_285_829_422_576_1).abs() < 1e-9, "{pos}");
        // Both ends exactly, and a ratio beyond the f64 range.
        assert_eq!(s.normalize(1.385e299), 0.0);
        assert_eq!(s.normalize(1.385_000_001_385_000_2e299), 1.0);
        let wide = ContinuousScale::new(log, 1e-300, 1e300, "t").unwrap();
        assert!((wide.normalize(1.0) - 0.5).abs() < 1e-15);
        assert!((wide.normalize(1e300) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn nice_with_ticks_falls_back_on_degenerate_domains() {
        let scale = raw(1.0, 1.0 + 1e-14);
        let (nice, ticks) = scale.nice_with_ticks(5);
        assert_eq!(nice, scale);
        assert_eq!(ticks, scale.ticks(5));
    }

    #[test]
    fn log_base_validation() {
        assert_eq!(LogBase::new(2.0).unwrap().get(), 2.0);
        for bad in [1.0, 0.5, 0.0, -10.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                LogBase::new(bad),
                Err(Error::Style {
                    context: "Scale.log()",
                    ..
                })
            ));
        }
    }

    #[test]
    fn new_rejects_empty_inverted_or_non_finite_domains() {
        for (lo, hi) in [
            (1.0, 1.0),
            (2.0, 1.0),
            (f64::NAN, 1.0),
            (0.0, f64::INFINITY),
        ] {
            assert!(ContinuousScale::new(Transform::Linear, lo, hi, "Axes.xlim()").is_err());
        }
    }

    #[test]
    fn log_scale_rejects_non_positive_domain() {
        let t = Transform::Log(LogBase::TEN);
        assert!(matches!(
            ContinuousScale::new(t, 0.0, 10.0, "Axes.ylim()"),
            Err(Error::Style { .. })
        ));
        assert!(matches!(
            ContinuousScale::from_extent(t, Some((-1.0, 10.0)), "Axes.line()"),
            Err(Error::Data { .. })
        ));
        let s = ContinuousScale::new(t, 1.0, 100.0, "t").unwrap();
        assert!(s.normalize(0.0).is_nan());
        assert!((s.normalize(10.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn linear_normalize() {
        let s = lin(-10.0, 30.0);
        assert_eq!(s.normalize(-10.0), 0.0);
        assert_eq!(s.normalize(30.0), 1.0);
        assert_eq!(s.normalize(10.0), 0.5);
    }

    #[test]
    fn nice_expands_to_enclosing_ticks() {
        assert_eq!(lin(0.5, 9.5).nice(5).domain(), (0.0, 10.0));
        assert_eq!(lin(3.2, 7.9).nice(5).domain(), (3.0, 8.0));
        let log = ContinuousScale::new(Transform::Log(LogBase::TEN), 3.0, 420.0, "t").unwrap();
        assert_eq!(log.nice(5).domain(), (1.0, 1000.0));
    }

    #[test]
    fn ticks_stay_within_domain_for_exact_extent() {
        let ticks = lin(0.5, 9.5).ticks(5);
        let values: Vec<f64> = ticks.iter().map(|t| t.value).collect();
        assert_eq!(values, [2.5, 5.0, 7.5]);
        assert_eq!(ticks[0].label, "2.5");
    }

    #[test]
    fn log_ticks_are_powers() {
        let s = ContinuousScale::new(Transform::Log(LogBase::TEN), 1.0, 1e4, "t").unwrap();
        let labels: Vec<String> = s.ticks(5).into_iter().map(|t| t.label).collect();
        assert_eq!(labels, ["1", "10", "100", "1000", "10000"]);
    }

    #[test]
    fn empty_data_scale_is_a_valid_domain_and_the_from_extent_default() {
        for transform in [
            Transform::Linear,
            Transform::Log(LogBase::TEN),
            Transform::Log(LogBase::new(2.0).unwrap()),
        ] {
            let unit = ContinuousScale::unit(transform);
            let (lo, hi) = unit.domain();
            assert_eq!(ContinuousScale::new(transform, lo, hi, "t").unwrap(), unit);
            assert_eq!(
                ContinuousScale::from_extent(transform, None, "t").unwrap(),
                unit
            );
        }
        assert_eq!(
            ContinuousScale::unit(Transform::Linear).domain(),
            (0.0, 1.0)
        );
        assert_eq!(
            ContinuousScale::unit(Transform::Log(LogBase::TEN)).domain(),
            (1.0, 10.0)
        );
    }

    #[test]
    fn scale_kind_transform_maps_linear_and_log_and_rejects_category() {
        let base = LogBase::new(2.0).unwrap();
        assert_eq!(ScaleKind::Linear.transform(), Some(Transform::Linear));
        assert_eq!(
            ScaleKind::Log { base }.transform(),
            Some(Transform::Log(base))
        );
        assert_eq!(ScaleKind::Category.transform(), None);
    }

    #[test]
    fn from_extent_handles_empty_and_degenerate() {
        let t = Transform::Linear;
        assert_eq!(
            ContinuousScale::from_extent(t, None, "t").unwrap().domain(),
            (0.0, 1.0)
        );
        assert_eq!(
            ContinuousScale::from_extent(t, Some((0.0, 0.0)), "t")
                .unwrap()
                .domain(),
            (-0.5, 0.5)
        );
        assert_eq!(
            ContinuousScale::from_extent(t, Some((20.0, 20.0)), "t")
                .unwrap()
                .domain(),
            (19.0, 21.0)
        );
        let log = Transform::Log(LogBase::TEN);
        assert_eq!(
            ContinuousScale::from_extent(log, Some((5.0, 5.0)), "t")
                .unwrap()
                .domain(),
            (0.5, 50.0)
        );
    }

    #[test]
    fn extreme_ranges_terminate_with_sane_ticks() {
        for (lo, hi) in [
            (-1e-300, 1e-300),
            (0.0, 1e300),
            (1e9, 1e9 + 1e-3),
            (5.0, 5.000000001),
            (0.0, 1e-320),
        ] {
            let s = raw(lo, hi);
            let ticks = s.ticks(5);
            assert!(!ticks.is_empty(), "{lo}..{hi}");
            assert!(
                ticks
                    .iter()
                    .all(|t| t.value.is_finite() && !t.label.is_empty())
            );
            let nice = s.nice(5).domain();
            assert!(nice.0.is_finite() && nice.1.is_finite() && nice.0 <= lo && nice.1 >= hi);
        }
    }

    #[test]
    fn category_scale_orders_by_first_appearance() {
        let s = CategoryScale::from_first_appearance(["b", "a", "b", "c"]);
        assert_eq!(s.categories(), ["b", "a", "c"]);
        assert_eq!(s.index_of("a"), Some(1));
        assert_eq!(s.band(1), (1.0 / 3.0, 2.0 / 3.0));
        assert_eq!(s.center(0), 1.0 / 6.0);
        assert_eq!(
            CategoryScale::from_first_appearance(std::iter::empty::<&str>()).band(0),
            (0.0, 1.0)
        );
    }

    fn rel_close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * b.abs()
    }

    #[test]
    fn huge_targets_are_clamped_not_overflowed() {
        let scales = [
            lin(0.0, 1.0),
            lin(0.0, 1e300),
            ContinuousScale::new(Transform::Log(LogBase::TEN), 1.0, 100.0, "t").unwrap(),
        ];
        for s in scales {
            for target in [usize::MAX, i64::MAX as usize, usize::MAX - 3, 0, 1] {
                let ticks = s.ticks(target);
                assert!(!ticks.is_empty());
                assert!(ticks.len() <= 2 * 64 + 1, "{} ticks", ticks.len());
                let (lo, hi) = s.nice(target).domain();
                let (dlo, dhi) = s.domain();
                assert!(lo <= dlo && hi >= dhi);
            }
        }
    }

    #[test]
    fn linear_domain_beyond_max_magnitude_is_rejected() {
        assert!(matches!(
            ContinuousScale::new(Transform::Linear, -1e308, 1e308, "t"),
            Err(Error::Style { .. })
        ));
        assert!(
            ContinuousScale::new(Transform::Linear, -MAX_MAGNITUDE, MAX_MAGNITUDE, "t").is_ok()
        );
        assert!(matches!(
            ContinuousScale::from_extent(Transform::Linear, Some((-1e308, 1e308)), "t"),
            Err(Error::Data { .. })
        ));
        let s = lin(-MAX_MAGNITUDE, MAX_MAGNITUDE);
        assert_eq!(s.normalize(0.0), 0.5);
        assert_eq!(s.normalize(MAX_MAGNITUDE), 1.0);
    }

    #[test]
    fn log_domain_limits_and_adjacent_bounds() {
        let t = Transform::Log(LogBase::TEN);
        assert!(matches!(
            ContinuousScale::new(t, 1e9, next_up(1e9), "t"),
            Err(Error::Style { .. })
        ));
        assert!(matches!(
            ContinuousScale::new(t, f64::from_bits(1), 1.0, "t"),
            Err(Error::Style { .. })
        ));
        assert!(matches!(
            ContinuousScale::new(t, 1.0, f64::MAX, "t"),
            Err(Error::Style { .. })
        ));
        assert!(matches!(
            ContinuousScale::from_extent(t, Some((f64::from_bits(1), 1.0)), "t"),
            Err(Error::Data { .. })
        ));
        assert!(matches!(
            ContinuousScale::from_extent(t, Some((1.0, f64::MAX)), "t"),
            Err(Error::Data { .. })
        ));
        let ok = ContinuousScale::new(t, MIN_LOG_VALUE, MAX_MAGNITUDE, "t").unwrap();
        assert!(ok.normalize(1.0) > 0.0 && ok.normalize(1.0) < 1.0);
        assert!(
            ok.ticks(5)
                .iter()
                .all(|t| t.value.is_finite() && t.value > 0.0)
        );
    }

    #[test]
    fn padded_adds_five_percent_of_the_span() {
        for ((lo, hi), (want_lo, want_hi)) in [
            ((35.0, 65.0), (33.5, 66.5)),
            ((-10.0, 10.0), (-11.0, 11.0)),
            ((1e-5, 3e-5), (0.9e-5, 3.1e-5)),
        ] {
            let (plo, phi) = raw(lo, hi).padded(Padding::BothSides).domain();
            assert!(
                rel_close(plo, want_lo) && rel_close(phi, want_hi),
                "{plo} {phi}"
            );
        }
        // Log: 5 % of the span in log space, the same factor at both ends and in every base.
        for base in [2.0, 10.0, 999.0] {
            let log = Transform::Log(LogBase::new(base).unwrap());
            let (lo, hi) = ContinuousScale::new(log, 10.0, 1000.0, "t")
                .unwrap()
                .padded(Padding::BothSides)
                .domain();
            let factor = 10f64.powf(0.1);
            assert!(rel_close(lo, 10.0 / factor), "base {base}: {lo}");
            assert!(rel_close(hi, 1000.0 * factor), "base {base}: {hi}");
        }
    }

    #[test]
    fn padded_is_clamped_to_the_supported_magnitudes() {
        let lin = Transform::Linear;
        let log = Transform::Log(LogBase::TEN);
        let padded = |t: Transform, lo: f64, hi: f64| {
            ContinuousScale::new(t, lo, hi, "t")
                .unwrap()
                .padded(Padding::BothSides)
                .domain()
        };
        assert_eq!(
            padded(lin, -MAX_MAGNITUDE, MAX_MAGNITUDE),
            (-MAX_MAGNITUDE, MAX_MAGNITUDE)
        );
        let (lo, hi) = padded(lin, 0.0, MAX_MAGNITUDE);
        assert_eq!(hi, MAX_MAGNITUDE);
        assert!(rel_close(lo, -0.05 * MAX_MAGNITUDE), "{lo}");
        assert_eq!(
            padded(log, MIN_LOG_VALUE, MAX_MAGNITUDE),
            (MIN_LOG_VALUE, MAX_MAGNITUDE)
        );
        // About 100 decades pad by about 5 decades, past 1e-300 and 1e300.
        let (lo, hi) = padded(log, 2e-300, 1e-200);
        assert_eq!(lo, MIN_LOG_VALUE);
        assert!(hi > 1e-196 && hi < 1e-195, "{hi}");
        let (lo, hi) = padded(log, 1e200, 5e299);
        assert_eq!(hi, MAX_MAGNITUDE);
        assert!(lo < 1e196, "{lo}");
        // The padded domains are valid domains.
        for (t, lo, hi) in [
            (lin, -MAX_MAGNITUDE, MAX_MAGNITUDE),
            (lin, 0.0, MAX_MAGNITUDE),
            (log, MIN_LOG_VALUE, MAX_MAGNITUDE),
            (log, 2e-300, 1e-200),
            (log, 1e200, 5e299),
        ] {
            let (lo, hi) = padded(t, lo, hi);
            assert!(ContinuousScale::new(t, lo, hi, "t").is_ok(), "{lo} {hi}");
        }
    }

    #[test]
    fn keep_zero_padding_leaves_a_zero_limit_in_place() {
        let keep = |lo: f64, hi: f64| raw(lo, hi).padded(Padding::KeepZero).domain();
        // A zero limit stays; the far side is padded as usual.
        assert_eq!(keep(0.0, 40.0), (0.0, 42.0));
        assert_eq!(keep(-40.0, 0.0), (-42.0, 0.0));
        // A domain that does not end at 0 is padded on both sides.
        assert_eq!(keep(-10.0, 30.0), (-12.0, 32.0));
        assert_eq!(keep(10.0, 30.0), (9.0, 31.0));
        // A log domain never ends at 0, so it is padded on both sides.
        let log = Transform::Log(LogBase::TEN);
        let scale = ContinuousScale::new(log, 10.0, 1000.0, "t").unwrap();
        assert_eq!(
            scale.padded(Padding::KeepZero),
            scale.padded(Padding::BothSides)
        );
    }

    #[test]
    fn single_values_are_recognized_like_from_extent() {
        let lin = Transform::Linear;
        let log = Transform::Log(LogBase::TEN);
        for (t, lo, hi) in [
            (lin, 20.0, 20.0),
            (lin, 0.0, 0.0),
            (lin, 1e9, 1e9 + 1e-3),
            (log, 5.0, 5.0),
            (log, 1e9, next_up(1e9)),
        ] {
            assert!(is_single_value(t, lo, hi), "{lo} {hi}");
        }
        for (t, lo, hi) in [(lin, 0.0, 1.0), (lin, 35.0, 65.0), (log, 1.0, 1.1)] {
            assert!(!is_single_value(t, lo, hi), "{lo} {hi}");
        }
    }

    #[test]
    fn singleton_padding_stays_representable() {
        let log = Transform::Log(LogBase::TEN);
        let d = ContinuousScale::from_extent(log, Some((1e9, 1e9)), "t")
            .unwrap()
            .domain();
        assert_eq!(d, (1e8, 1e10));
        // A log extent narrower than 1 + 1e-9 is a singleton at its lower bound.
        let near = ContinuousScale::from_extent(log, Some((1e9, next_up(1e9))), "t").unwrap();
        assert_eq!(near.domain(), (1e8, 1e10));
        let tiny = ContinuousScale::from_extent(
            Transform::Linear,
            Some((f64::from_bits(1), f64::from_bits(1))),
            "t",
        )
        .unwrap();
        let (lo, hi) = tiny.domain();
        assert!(lo.is_finite() && hi.is_finite() && lo < hi);
        let big = ContinuousScale::from_extent(
            Transform::Linear,
            Some((MAX_MAGNITUDE, MAX_MAGNITUDE)),
            "t",
        )
        .unwrap();
        let (lo, hi) = big.domain();
        assert!(lo < hi && hi <= MAX_MAGNITUDE);
        let top =
            ContinuousScale::from_extent(log, Some((MAX_MAGNITUDE, MAX_MAGNITUDE)), "t").unwrap();
        let (lo, hi) = top.domain();
        assert!(lo < hi && hi <= MAX_MAGNITUDE);
        let bottom =
            ContinuousScale::from_extent(log, Some((MIN_LOG_VALUE, MIN_LOG_VALUE)), "t").unwrap();
        let (lo, hi) = bottom.domain();
        assert!(lo >= MIN_LOG_VALUE && lo < hi);
    }

    #[test]
    fn nice_is_scale_invariant_for_extreme_spans() {
        for scale in [1e-180, 1e180] {
            let (lo, hi) = lin(1.3 * scale, 8.7 * scale).nice(5).domain();
            assert!(rel_close(lo, 1.0 * scale), "{lo} vs {}", 1.0 * scale);
            assert!(rel_close(hi, 9.0 * scale), "{hi} vs {}", 9.0 * scale);
        }
    }

    #[test]
    fn ticks_have_strictly_increasing_positions() {
        let s = raw(1.0, 1.0 + f64::EPSILON);
        let ticks = s.ticks(5);
        assert_eq!(ticks.len(), 2);
        assert_eq!(ticks[0].value, 1.0);
        assert_eq!(ticks[1].value, 1.0 + f64::EPSILON);
        for (lo, hi) in [
            (0.0, f64::from_bits(1)),
            (1e9, 1e9 + 1e-3),
            (0.0, 1e-320),
            (0.0, 1.0),
            (-7.0, 1e300),
        ] {
            let v: Vec<f64> = raw(lo, hi).ticks(8).iter().map(|t| t.value).collect();
            assert!(v.len() >= 2, "{lo}..{hi}");
            assert!(v.windows(2).all(|w| w[1] > w[0]), "{lo}..{hi}: {v:?}");
        }
    }

    #[test]
    fn log_ticks_with_base_near_one_are_thinned_before_powers_are_computed() {
        let base = LogBase::new(1.000000001).unwrap();
        let s = ContinuousScale::new(Transform::Log(base), 2.0, 4.0, "t").unwrap();
        let ticks = s.ticks(5);
        assert!(!ticks.is_empty() && ticks.len() <= 5);
        assert!(
            ticks
                .iter()
                .all(|t| t.value >= 2.0 * (1.0 - 1e-9) && t.value <= 4.0 * (1.0 + 1e-9))
        );
        let (lo, hi) = s.nice(5).domain();
        assert!(lo <= 2.0 && hi >= 4.0);
    }

    #[test]
    fn log_ticks_cover_subnormal_free_extremes() {
        let t = Transform::Log(LogBase::TEN);
        let s = ContinuousScale::new(t, 1e-300, 1e-290, "t").unwrap();
        let v: Vec<f64> = s.ticks(5).iter().map(|t| t.value).collect();
        assert!(v.len() >= 2 && v.iter().all(|x| *x > 0.0));
        assert!(v.windows(2).all(|w| w[1] > w[0]));
    }

    fn assert_distinct_labels(lo: f64, hi: f64) {
        let ticks = raw(lo, hi).ticks(5);
        assert!(ticks.len() >= 2, "{lo}..{hi}: {ticks:?}");
        assert!(
            ticks.windows(2).all(|w| w[1].value > w[0].value),
            "{lo}..{hi}"
        );
        let labels: std::collections::HashSet<&str> =
            ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels.len(), ticks.len(), "{lo}..{hi}: {ticks:?}");
        assert!(ticks.iter().all(|t| !t.label.is_empty()));
    }

    #[test]
    fn distinct_values_get_distinct_labels() {
        assert_distinct_labels(1e9, 1e9 + 1e-3);
        assert_distinct_labels(1e6, 1e6 + 2e-6);
        assert_distinct_labels(1e-5, 1e-5 + 2e-17);
        assert_distinct_labels(-1e9, -1e9 + 2e-3);
        assert_distinct_labels(1.0, 1.0 + 2e-12);
        assert_distinct_labels(0.1, 0.1 + 2e-13);
        assert_distinct_labels(1e-4, 1e-4 + 2e-16);
        assert_distinct_labels(1.0, 1.0 + f64::EPSILON);
        assert_distinct_labels(0.0, f64::from_bits(1));
    }

    #[test]
    fn log_powers_are_labelled_by_value() {
        let base = LogBase::new(1.5).unwrap();
        let s = ContinuousScale::new(Transform::Log(base), 1.0, 5.0, "t").unwrap();
        let labels: Vec<String> = s.ticks(5).into_iter().map(|t| t.label).collect();
        assert_eq!(labels, ["1", "1.5", "2.25", "3.375"]);

        let wide = ContinuousScale::new(Transform::Log(LogBase::TEN), 1e-5, 1e9, "t").unwrap();
        let labels: Vec<String> = wide.ticks(5).into_iter().map(|t| t.label).collect();
        assert_eq!(labels, ["1e\u{2212}5", "1e\u{2212}2", "1e1", "1e4", "1e7"]);

        let near = LogBase::new(1.0001).unwrap();
        let s = ContinuousScale::new(Transform::Log(near), 1.0, 1.01, "t").unwrap();
        let ticks = s.ticks(5);
        let distinct: std::collections::HashSet<&str> =
            ticks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(distinct.len(), ticks.len());
    }

    #[test]
    fn log_without_two_powers_falls_back_to_linear_ticks() {
        let s = ContinuousScale::new(Transform::Log(LogBase::TEN), 2.2, 3.2, "t").unwrap();
        let ticks = s.ticks(5);
        assert!(ticks.len() >= 2, "{ticks:?}");
        for t in &ticks {
            assert!(t.value >= 2.2 && t.value <= 3.2, "{t:?}");
            let parsed: f64 = t.label.replace('\u{2212}', "-").parse().unwrap();
            assert!((parsed - t.value).abs() <= 1e-9 * t.value.abs(), "{t:?}");
        }
        // Exactly one power inside is still too few.
        let one = ContinuousScale::new(Transform::Log(LogBase::TEN), 2.0, 50.0, "t").unwrap();
        assert!(one.ticks(5).len() >= 2);
    }

    #[test]
    fn log_ticks_are_thinned_evenly_in_exponent() {
        let s = ContinuousScale::new(Transform::Log(LogBase::TEN), 1e-30, 1e30, "t").unwrap();
        let ticks = s.ticks(5);
        assert!(
            ticks.len() >= 2 && ticks.len() <= 11,
            "{} ticks",
            ticks.len()
        );
        let exps: Vec<f64> = ticks.iter().map(|t| t.value.log10().round()).collect();
        let gap = exps[1] - exps[0];
        assert!(gap > 1.0);
        assert!(exps.windows(2).all(|w| w[1] - w[0] == gap), "{exps:?}");
    }

    #[test]
    fn log_singleton_with_near_one_base_still_pads() {
        for base in [1.0000000001, next_up(1.0), 1.05] {
            let t = Transform::Log(LogBase::new(base).unwrap());
            let (lo, hi) = ContinuousScale::from_extent(t, Some((1.0, 1.0)), "t")
                .unwrap()
                .domain();
            assert!(
                (lo - 1.0 / 1.1).abs() < 1e-12 && (hi - 1.1).abs() < 1e-12,
                "{base}: {lo}..{hi}"
            );
        }
        // Bases above 1.1 keep their own padding.
        let t = Transform::Log(LogBase::TEN);
        let near =
            ContinuousScale::from_extent(t, Some((MAX_MAGNITUDE, MAX_MAGNITUDE)), "t").unwrap();
        assert!(near.domain().1 <= MAX_MAGNITUDE);
    }

    #[test]
    fn log_fallback_never_labels_a_positive_tick_zero() {
        let base = LogBase::new(1e20).unwrap();
        let s = ContinuousScale::new(Transform::Log(base), 1e-10, 1e4, "t").unwrap();
        let ticks = s.ticks(5);
        assert!(ticks.len() >= 2, "{ticks:?}");
        let (lo, hi) = s.domain();
        for t in &ticks {
            assert!(t.value > 0.0 && t.value >= lo && t.value <= hi, "{t:?}");
            assert_ne!(t.label, "0", "{t:?}");
            let parsed: f64 = t.label.replace('\u{2212}', "-").parse().unwrap();
            assert!((parsed - t.value).abs() <= 1e-9 * t.value.abs(), "{t:?}");
        }
    }

    #[test]
    fn log_labels_have_at_most_twelve_significant_digits() {
        let s = ContinuousScale::new(Transform::Log(LogBase::new(1.1).unwrap()), 1.0, 2.0, "t")
            .unwrap();
        let labels: Vec<String> = s.ticks(5).into_iter().map(|t| t.label).collect();
        assert!(labels.contains(&"1.21".to_string()), "{labels:?}");
        for label in &labels {
            let digits = label
                .split('e')
                .next()
                .unwrap_or("")
                .chars()
                .filter(char::is_ascii_digit)
                .collect::<String>();
            assert!(digits.trim_start_matches('0').len() <= 12, "{label}");
        }
    }

    proptest! {
        #[test]
        fn loose_ticks_cover_the_range(lo in -1e6f64..1e6, span in 1e-3f64..1e6, m in 2usize..10) {
            let s = lin(lo, lo + span);
            let (nlo, nhi) = s.nice(m).domain();
            prop_assert!(nlo <= lo && nhi >= lo + span);
        }

        #[test]
        fn ticks_are_evenly_spaced_and_inside(lo in -1e6f64..1e6, span in 1e-3f64..1e6, m in 2usize..10) {
            let s = lin(lo, lo + span);
            let v: Vec<f64> = s.ticks(m).iter().map(|t| t.value).collect();
            prop_assert!(!v.is_empty());
            let tol = 1e-9 * span;
            prop_assert!(v.iter().all(|x| *x >= lo - tol && *x <= lo + span + tol));
            if v.len() >= 3 {
                let step = v[1] - v[0];
                prop_assert!(v.windows(2).all(|w| ((w[1] - w[0]) - step).abs() <= 1e-6 * step.abs()));
            }
        }
    }
}
