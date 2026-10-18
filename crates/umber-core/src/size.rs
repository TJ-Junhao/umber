//! Sizes: line widths, markers and text, relative to the theme's or in points.
//!
//! A [`SizeSpec`] says how wide a line or how large a marker or text should be. It is
//! resolved at render time against the figure's theme, so [`SizeSpec::times`] keeps emphasis
//! under any theme. Text uses its role size after font scaling; fixed points ignore scaling.
//!
//! # Examples
//!
//! ```
//! use umber_core::figure::LineOptions;
//! use umber_core::SizeSpec;
//!
//! let emphasized = LineOptions { width: SizeSpec::times(2.0)?, ..LineOptions::default() };
//! assert_eq!(emphasized.width, SizeSpec::times(2.0)?);
//! assert_eq!(LineOptions::default().width, SizeSpec::Auto);
//! # Ok::<(), umber_core::Error>(())
//! ```

use crate::error::number;
use crate::{Error, Result};

/// The largest factor [`SizeSpec::times`] accepts.
const MAX_FACTOR: f64 = 10.0;
/// The largest size in points [`SizeSpec::pt`] accepts: one inch.
const MAX_POINTS: f64 = 72.0;

/// A multiple of the theme's size, greater than 0 and at most 10. Made by [`SizeSpec::times`].
///
/// # Examples
///
/// ```
/// use umber_core::SizeSpec;
///
/// let SizeSpec::Times(factor) = SizeSpec::times(1.5)? else { unreachable!() };
/// assert_eq!(factor.get(), 1.5);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Factor(f64);

impl Factor {
    /// The factor, greater than 0 and at most 10.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::SizeSpec;
    ///
    /// let SizeSpec::Times(factor) = SizeSpec::times(0.5)? else { unreachable!() };
    /// assert_eq!(factor.get(), 0.5);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn get(self) -> f64 {
        self.0
    }
}

/// A size in points, greater than 0 and at most 72 (one inch). Made by [`SizeSpec::pt`].
///
/// # Examples
///
/// ```
/// use umber_core::SizeSpec;
///
/// let SizeSpec::Points(points) = SizeSpec::pt(3.5)? else { unreachable!() };
/// assert_eq!(points.get(), 3.5);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Points(f64);

impl Points {
    /// The size in points, greater than 0 and at most 72.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::SizeSpec;
    ///
    /// let SizeSpec::Points(points) = SizeSpec::pt(72.0)? else { unreachable!() };
    /// assert_eq!(points.get(), 72.0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn get(self) -> f64 {
        self.0
    }
}

/// The width of a line, the size of a marker or a text role: the line width of a
/// line or area ([`LineOptions::width`](crate::LineOptions::width)), the marker diameter of a
/// scatter ([`ScatterOptions::size`](crate::ScatterOptions::size)), or a text size such as
/// [`AxesMut::title_sized`](crate::AxesMut::title_sized).
///
/// The size is resolved at render time against the theme of the figure: [`SizeSpec::Auto`] is
/// the theme's value ([`Theme::line_width`](crate::Theme::line_width) or
/// [`Theme::marker_size`](crate::Theme::marker_size)), [`SizeSpec::Times`] a multiple of it, and
/// [`SizeSpec::Points`] a fixed size that ignores the theme. Prefer [`SizeSpec::times`]: an
/// emphasized series then stays emphasized under any theme. A drawn size never exceeds
/// 10 000 pt, whatever the theme says. Text resolves against the role size after font
/// scaling, except fixed points, which ignore font scaling.
///
/// # Examples
///
/// ```
/// use umber_core::SizeSpec;
///
/// assert_eq!(SizeSpec::default(), SizeSpec::Auto);
/// assert!(matches!(SizeSpec::times(2.0)?, SizeSpec::Times(_)));
/// assert!(matches!(SizeSpec::pt(0.75)?, SizeSpec::Points(_)));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SizeSpec {
    /// The theme's value.
    #[default]
    Auto,
    /// The theme's value times a factor.
    Times(Factor),
    /// A fixed size in points.
    Points(Points),
}

impl SizeSpec {
    /// A size relative to the theme's line width, marker size or scaled text role size.
    ///
    /// # Errors
    ///
    /// `Error::Style` (context `"Size.times()"`, parameter `factor`) unless `factor` is
    /// greater than 0 and at most 10.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::SizeSpec;
    ///
    /// assert!(SizeSpec::times(2.5).is_ok());
    /// assert!(SizeSpec::times(10.0).is_ok());
    /// let err = SizeSpec::times(0.0).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "Size.times(): factor is 0; a size factor must be greater than 0 and at most 10."
    /// );
    /// assert_eq!(err.parameter(), Some("factor"));
    /// ```
    pub fn times(factor: f64) -> Result<SizeSpec> {
        if factor > 0.0 && factor <= MAX_FACTOR {
            return Ok(SizeSpec::Times(Factor(factor)));
        }
        let err = Error::style(
            "Size.times()",
            format!(
                "factor is {}; a size factor must be greater than 0 and at most 10.",
                number(factor)
            ),
        )
        .with_parameter("factor");
        Err(if factor <= 0.0 {
            err.with_hint(
                "use Size.AUTO for the theme's size, or a factor below 1 for a smaller one.",
            )
        } else if factor.is_finite() {
            err.with_hint("use Size.pt() for a size in points.")
        } else {
            err
        })
    }

    /// A fixed size of `points`, whatever the theme.
    ///
    /// # Errors
    ///
    /// `Error::Style` (context `"Size.pt()"`, parameter `points`) unless `points` is greater
    /// than 0 and at most 72.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::SizeSpec;
    ///
    /// assert!(SizeSpec::pt(0.5).is_ok());
    /// assert!(SizeSpec::pt(72.0).is_ok());
    /// let err = SizeSpec::pt(72.1).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "Size.pt(): points is 72.1; a size in points must be greater than 0 and at most 72."
    /// );
    /// assert_eq!(err.parameter(), Some("points"));
    /// ```
    pub fn pt(points: f64) -> Result<SizeSpec> {
        if points > 0.0 && points <= MAX_POINTS {
            return Ok(SizeSpec::Points(Points(points)));
        }
        let err = Error::style(
            "Size.pt()",
            format!(
                "points is {}; a size in points must be greater than 0 and at most 72.",
                number(points)
            ),
        )
        .with_parameter("points");
        Err(if points <= 0.0 {
            err.with_hint("use Size.AUTO for the theme's size.")
        } else {
            err
        })
    }

    /// The size this spec asks for, in points, where the theme's value is `theme`: `theme`
    /// itself, `theme` times the factor, or the fixed points. A product beyond the range of
    /// `f32` is `f32::MAX`, so it stays finite for the cap the caller applies.
    pub(crate) fn resolve(self, theme: f32) -> f32 {
        match self {
            SizeSpec::Auto => theme,
            SizeSpec::Times(factor) => {
                (factor.get() * f64::from(theme)).min(f64::from(f32::MAX)) as f32
            }
            SizeSpec::Points(points) => points.get() as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The message, parameter and hint of a `Style` error with `context`.
    #[track_caller]
    fn style_error(
        r: Result<SizeSpec>,
        context: &str,
    ) -> (String, Option<&'static str>, Option<&'static str>) {
        match r {
            Err(e @ Error::Style { .. }) => {
                assert!(e.to_string().starts_with(&format!("{context}: ")), "{e}");
                let Error::Style { message, .. } = &e else {
                    unreachable!()
                };
                (message.clone(), e.parameter(), e.hint())
            }
            other => panic!("expected a Style error, got {other:?}"),
        }
    }

    #[test]
    fn times_accepts_factors_above_0_up_to_10() {
        for ok in [f64::MIN_POSITIVE, 1e-300, 0.5, 1.0, 9.999_999, 10.0] {
            assert_eq!(
                SizeSpec::times(ok).unwrap(),
                SizeSpec::Times(Factor(ok)),
                "{ok}"
            );
        }
    }

    #[test]
    fn times_rejects_factors_outside_its_range_naming_the_bound() {
        let below =
            Some("use Size.AUTO for the theme's size, or a factor below 1 for a smaller one.");
        let above = Some("use Size.pt() for a size in points.");
        for (factor, shown, hint) in [
            (0.0, "0", below),
            (-0.0, "-0", below),
            (-1.0, "-1", below),
            (f64::NEG_INFINITY, "-inf", below),
            (10.000_001, "10.000001", above),
            (1e301, "1e301", above),
            (f64::INFINITY, "inf", None),
            (f64::NAN, "NaN", None),
        ] {
            let (message, parameter, got_hint) =
                style_error(SizeSpec::times(factor), "Size.times()");
            assert_eq!(
                message,
                format!("factor is {shown}; a size factor must be greater than 0 and at most 10."),
            );
            assert_eq!(parameter, Some("factor"), "{factor}");
            assert_eq!(got_hint, hint, "{factor}");
        }
    }

    #[test]
    fn pt_accepts_points_above_0_up_to_72() {
        for ok in [1e-300, 0.25, 1.0, 71.999, 72.0] {
            assert_eq!(
                SizeSpec::pt(ok).unwrap(),
                SizeSpec::Points(Points(ok)),
                "{ok}"
            );
        }
    }

    #[test]
    fn pt_rejects_points_outside_its_range_naming_the_bound() {
        let below = Some("use Size.AUTO for the theme's size.");
        for (points, shown, hint) in [
            (0.0, "0", below),
            (-0.0, "-0", below),
            (-3.0, "-3", below),
            (f64::NEG_INFINITY, "-inf", below),
            (72.1, "72.1", None),
            (1e6, "1e6", None),
            (f64::INFINITY, "inf", None),
            (f64::NAN, "NaN", None),
        ] {
            let (message, parameter, got_hint) = style_error(SizeSpec::pt(points), "Size.pt()");
            assert_eq!(
                message,
                format!(
                    "points is {shown}; a size in points must be greater than 0 and at most 72."
                ),
            );
            assert_eq!(parameter, Some("points"), "{points}");
            assert_eq!(got_hint, hint, "{points}");
        }
    }

    #[test]
    fn each_variant_resolves_against_the_theme_value() {
        assert_eq!(SizeSpec::Auto.resolve(1.8), 1.8);
        assert_eq!(SizeSpec::times(2.5).unwrap().resolve(2.0), 5.0);
        assert_eq!(SizeSpec::times(0.5).unwrap().resolve(6.2), 3.1);
        assert_eq!(SizeSpec::pt(3.0).unwrap().resolve(2.0), 3.0);
        assert_eq!(SizeSpec::pt(3.0).unwrap().resolve(40.0), 3.0);
        // A product beyond f32 stays finite.
        assert_eq!(SizeSpec::times(10.0).unwrap().resolve(f32::MAX), f32::MAX);
    }
}
