//! Scales: how values along one axis map to positions in `[0, 1]`.

use std::collections::HashMap;

use crate::ticks::{self, Tick};
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
            Err(Error::Style {
                context: "Scale.log()",
                message: format!("base is {base}; a log base must be finite and greater than 1."),
            })
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

/// The smallest `e` with `base^e >= v`, or `None` if it does not fit an `i32`.
fn ceil_exp(v: f64, base: f64) -> Option<i32> {
    let e = (v.ln() / base.ln()).ceil();
    if !e.is_finite() || e.abs() > 1e9 {
        return None;
    }
    let mut e = e as i32;
    if base.powi(e.checked_sub(1)?) >= v {
        e = e.checked_sub(1)?;
    } else if base.powi(e) < v {
        e = e.checked_add(1)?;
    }
    Some(e)
}

/// The largest `e` with `base^e <= v`, or `None` if it does not fit an `i32`.
fn floor_exp(v: f64, base: f64) -> Option<i32> {
    let e = (v.ln() / base.ln()).floor();
    if !e.is_finite() || e.abs() > 1e9 {
        return None;
    }
    let mut e = e as i32;
    if base.powi(e) > v {
        e = e.checked_sub(1)?;
    } else if base.powi(e.checked_add(1)?) <= v {
        e = e.checked_add(1)?;
    }
    Some(e)
}

impl ContinuousScale {
    /// Creates a scale over `[lo, hi]`.
    ///
    /// Returns `Error::Style` (with the caller's `context`) unless both limits are finite and
    /// `lo < hi`, and, for a log transform, `lo > 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContinuousScale, LogBase, Transform};
    ///
    /// let scale = ContinuousScale::new(Transform::Linear, 0.0, 1.0, "Axes.xlim()").unwrap();
    /// assert_eq!(scale.domain(), (0.0, 1.0));
    /// assert!(ContinuousScale::new(Transform::Linear, 2.0, 1.0, "Axes.xlim()").is_err());
    /// assert!(ContinuousScale::new(Transform::Log(LogBase::TEN), 0.0, 10.0, "Axes.ylim()").is_err());
    /// ```
    pub fn new(transform: Transform, lo: f64, hi: f64, context: &'static str) -> Result<Self> {
        if !(lo.is_finite() && hi.is_finite() && lo < hi) {
            return Err(Error::Style {
                context,
                message: format!(
                    "the limits are {lo} and {hi}; both must be finite and the lower limit must be below the upper limit."
                ),
            });
        }
        if matches!(transform, Transform::Log(_)) && lo <= 0.0 {
            return Err(Error::Style {
                context,
                message: format!(
                    "the lower limit is {lo}; a log scale needs a positive lower limit."
                ),
            });
        }
        Ok(ContinuousScale { transform, lo, hi })
    }

    /// Creates a scale that fits a data `extent` (`None` when there is no data).
    ///
    /// `None` gives `(0, 1)` for linear and `(1, 10)` for log. A single-valued extent is padded:
    /// `v +- 0.5` when `v == 0`, otherwise `v +- 5%` for linear, and `(v / base, v * base)` for
    /// log. Returns `Error::Data` for non-finite bounds, or for a log transform when the extent
    /// starts at or below zero.
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
            let (lo, hi) = match transform {
                Transform::Linear => (0.0, 1.0),
                Transform::Log(_) => (1.0, 10.0),
            };
            return Self::new(transform, lo, hi, context);
        };
        if !(lo.is_finite() && hi.is_finite()) {
            return Err(Error::Data {
                context,
                message: format!("the data range is {lo} to {hi}; it must be finite."),
            });
        }
        if lo > hi {
            return Err(Error::Data {
                context,
                message: format!(
                    "the data range is {lo} to {hi}; the lower bound must not exceed the upper bound."
                ),
            });
        }
        if matches!(transform, Transform::Log(_)) && lo <= 0.0 {
            return Err(Error::Data {
                context,
                message: format!(
                    "the data range starts at {lo}; a log scale can only show positive values."
                ),
            });
        }
        let (lo, hi) = if lo == hi {
            match transform {
                Transform::Linear => {
                    let pad = if lo == 0.0 { 0.5 } else { lo.abs() * 0.05 };
                    (lo - pad, lo + pad)
                }
                Transform::Log(base) => (lo / base.get(), lo * base.get()),
            }
        } else {
            (lo, hi)
        };
        Self::new(transform, lo, hi, context)
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
                    (v.ln() - self.lo.ln()) / (self.hi.ln() - self.lo.ln())
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
        let target = target.max(2);
        let widened = match self.transform {
            Transform::Linear => {
                if is_degenerate(self.lo, self.hi) {
                    None
                } else {
                    let values = ticks::extended(self.lo, self.hi, target, true);
                    values.first().copied().zip(values.last().copied())
                }
            }
            Transform::Log(base) => {
                let b = base.get();
                let lo = floor_exp(self.lo, b).map(|e| b.powi(e));
                let hi = ceil_exp(self.hi, b).map(|e| b.powi(e));
                lo.zip(hi)
            }
        };
        match widened {
            Some((lo, hi)) => {
                // Never narrow the domain, whatever rounding did.
                let (lo, hi) = (lo.min(self.lo), hi.max(self.hi));
                let positive = matches!(self.transform, Transform::Linear) || lo > 0.0;
                if lo.is_finite() && hi.is_finite() && lo < hi && positive {
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

    /// Tick positions and labels for about `target` ticks (at least 2).
    ///
    /// Linear ticks lie inside the domain. Log ticks are the powers of the base inside the
    /// domain, thinned to every n-th power when there are more than `2 * target`.
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
        let target = target.max(2);
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
        let labels = ticks::format_ticks(&values, self.hi - self.lo);
        Self::pair(values, labels)
    }

    fn linear_ticks(&self, target: usize) -> Vec<Tick> {
        let (lo, hi) = (self.lo, self.hi);
        if is_degenerate(lo, hi) {
            let values = ticks::evenly_spaced(lo, hi, target);
            let labels = ticks::format_ticks_scientific(&values);
            return Self::pair(values, labels);
        }
        let tol = 1e-9 * (hi - lo);
        for t in target..=target + 6 {
            let inside: Vec<f64> = ticks::extended(lo, hi, t, false)
                .into_iter()
                .filter(|v| *v >= lo - tol && *v <= hi + tol)
                .collect();
            if let (Some(a), Some(b)) = (inside.first(), inside.get(1)) {
                let step = b - a;
                let values: Vec<f64> = inside
                    .iter()
                    .map(|&v| {
                        if (v - lo).abs() <= tol {
                            lo
                        } else if (v - hi).abs() <= tol {
                            hi
                        } else {
                            v
                        }
                    })
                    .collect();
                let labels = ticks::format_ticks(&values, step);
                return Self::pair(values, labels);
            }
        }
        self.end_ticks()
    }

    fn log_ticks(&self, base: f64, target: usize) -> Vec<Tick> {
        let (Some(first), Some(last)) = (ceil_exp(self.lo, base), floor_exp(self.hi, base)) else {
            return self.end_ticks();
        };
        if first > last {
            return self.end_ticks();
        }
        let count = i64::from(last) - i64::from(first) + 1;
        let target_i = target as i64;
        let every = if count > target_i * 2 {
            (count + target_i - 1) / target_i
        } else {
            1
        };
        let mut values = Vec::new();
        let mut e = i64::from(first);
        while e <= i64::from(last) {
            let Ok(exp) = i32::try_from(e) else { break };
            values.push(base.powi(exp));
            e += every;
        }
        let Some(&smallest) = values.first() else {
            return self.end_ticks();
        };
        let labels = ticks::format_ticks(&values, smallest);
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
            let s = lin(lo, hi);
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
