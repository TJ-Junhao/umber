//! Tick label formats: how the numbers next to the ticks of a linear or log axis are written.
//!
//! A [`TickFormat`] is set per axis with [`Axes::xtick_format`](crate::Axes::xtick_format) and
//! [`Axes::ytick_format`](crate::Axes::ytick_format). It applies to the automatic ticks and to
//! explicit tick positions without labels; labels given with the positions are drawn as given.
//!
//! # Examples
//!
//! ```
//! use umber_core::{Figure, NumberStyle, ScaleKind, Theme, TickFormat};
//!
//! let share = TickFormat::percent(1)?.affix("", " of sales")?;
//! assert!(matches!(share.style(), NumberStyle::Percent(d) if d.get() == 1));
//! assert_eq!(share.suffix(), " of sales");
//!
//! let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
//! let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
//! axes.ytick_format(share)?;
//! # Ok::<(), umber_core::Error>(())
//! ```

use crate::model::check_text;
use crate::ticks::{self, Tick, TickValues};
use crate::{Error, Result};

/// The most decimals [`TickFormat::fixed`] and [`TickFormat::percent`] accept.
const MAX_DECIMALS: i64 = 12;

/// A number of decimal places, from 0 to 12. Made by [`TickFormat::fixed`] and
/// [`TickFormat::percent`].
///
/// # Examples
///
/// ```
/// use umber_core::{NumberStyle, TickFormat};
///
/// let NumberStyle::Fixed(decimals) = TickFormat::fixed(2)?.style() else { unreachable!() };
/// assert_eq!(decimals.get(), 2);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decimals(u8);

impl Decimals {
    /// The number of decimal places, from 0 to 12.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{NumberStyle, TickFormat};
    ///
    /// let NumberStyle::Percent(decimals) = TickFormat::percent(0)?.style() else {
    ///     unreachable!()
    /// };
    /// assert_eq!(decimals.get(), 0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn get(self) -> usize {
        usize::from(self.0)
    }
}

/// How the number in a tick label is written. Part of a [`TickFormat`].
///
/// # Examples
///
/// ```
/// use umber_core::{NumberStyle, TickFormat};
///
/// assert_eq!(TickFormat::AUTO.style(), NumberStyle::Auto);
/// assert_eq!(TickFormat::scientific().style(), NumberStyle::Scientific);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberStyle {
    /// The automatic labels. On a linear axis the decimals follow from the tick step, and a set
    /// with a value of at least `1e6` or a non-zero value below `1e-4` is written in
    /// scientific form; on a log axis each value is written with up to 12 significant digits.
    /// When two ticks would share a label, the precision is raised until they do not.
    #[default]
    Auto,
    /// Fixed-point with this many decimals, such as `2.50`.
    Fixed(Decimals),
    /// The value times 100 in fixed-point with this many decimals, followed by `%`, such as
    /// `12.5%` for 0.125.
    Percent(Decimals),
    /// Scientific form with up to 12 significant digits, such as `2.5e6` or `1e−5`.
    Scientific,
}

/// The format of the tick labels of a linear or log axis: a [`NumberStyle`] plus text before
/// and after every label.
///
/// Every style writes negative numbers with U+2212 `−` and never signs zero. Only
/// [`NumberStyle::Auto`] raises the precision when two ticks would share a label; with the other
/// styles that is the caller's choice. On a log axis, [`TickFormat::AUTO`] keeps labelling each
/// tick by its value, and the other styles apply to the tick values as on a linear axis.
///
/// The format applies to the automatic ticks and to tick positions set without labels. It
/// cannot be combined with labels set with the positions: such labels are drawn as given, so
/// any format other than plain [`TickFormat::AUTO`] (including one with only a prefix or a
/// suffix) is refused on an axis that has them.
///
/// # Examples
///
/// ```
/// use umber_core::TickFormat;
///
/// let money = TickFormat::fixed(2)?.affix("$", "")?;
/// assert_eq!((money.prefix(), money.suffix()), ("$", ""));
/// assert_eq!(TickFormat::default(), TickFormat::AUTO);
/// assert!(TickFormat::fixed(13).is_err());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TickFormat {
    style: NumberStyle,
    prefix: String,
    suffix: String,
}

impl TickFormat {
    /// The automatic labels, with no prefix or suffix: see [`NumberStyle::Auto`]. The default.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{NumberStyle, TickFormat};
    ///
    /// assert_eq!(TickFormat::AUTO.style(), NumberStyle::Auto);
    /// assert_eq!(TickFormat::AUTO.prefix(), "");
    /// ```
    pub const AUTO: TickFormat = TickFormat {
        style: NumberStyle::Auto,
        prefix: String::new(),
        suffix: String::new(),
    };

    /// Fixed-point labels with `decimals` places: `fixed(2)` writes 2.5 as `2.50`.
    ///
    /// # Errors
    ///
    /// `Error::Style` (context `"TickFormat.fixed()"`, parameter `decimals`) unless `decimals`
    /// is between 0 and 12.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::TickFormat;
    ///
    /// assert!(TickFormat::fixed(0).is_ok());
    /// let err = TickFormat::fixed(13).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "TickFormat.fixed(): decimals is 13; decimals must be between 0 and 12."
    /// );
    /// assert_eq!(err.parameter(), Some("decimals"));
    /// ```
    pub fn fixed(decimals: i64) -> Result<TickFormat> {
        let decimals = checked_decimals("TickFormat.fixed()", decimals)?;
        Ok(TickFormat::of(NumberStyle::Fixed(decimals)))
    }

    /// Percent labels: the value times 100 with `decimals` places, then `%`. `percent(0)`
    /// writes 0.25 as `25%`, `percent(1)` writes 0.125 as `12.5%`.
    ///
    /// # Errors
    ///
    /// `Error::Style` (context `"TickFormat.percent()"`, parameter `decimals`) unless
    /// `decimals` is between 0 and 12.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::TickFormat;
    ///
    /// assert!(TickFormat::percent(12).is_ok());
    /// let err = TickFormat::percent(-1).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "TickFormat.percent(): decimals is -1; decimals must be between 0 and 12."
    /// );
    /// ```
    pub fn percent(decimals: i64) -> Result<TickFormat> {
        let decimals = checked_decimals("TickFormat.percent()", decimals)?;
        Ok(TickFormat::of(NumberStyle::Percent(decimals)))
    }

    /// Scientific labels for every tick, such as `2.5e6` and `1e−5`, with up to 12
    /// significant digits; zero is `0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{NumberStyle, TickFormat};
    ///
    /// assert_eq!(TickFormat::scientific().style(), NumberStyle::Scientific);
    /// ```
    #[must_use]
    pub fn scientific() -> TickFormat {
        TickFormat::of(NumberStyle::Scientific)
    }

    /// This format with `prefix` written before and `suffix` after every label, replacing any
    /// prefix and suffix it had. A percent label puts its `%` before the suffix.
    ///
    /// # Errors
    ///
    /// `Error::Style` (context `"TickFormat.affix()"`, parameter `prefix` or `suffix`) when
    /// either text is longer than 1,000 characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::TickFormat;
    ///
    /// let degrees = TickFormat::fixed(1)?.affix("", " °C")?;
    /// assert_eq!(degrees.suffix(), " °C");
    /// assert_eq!(degrees.affix("", "")?, TickFormat::fixed(1)?);
    /// let err = TickFormat::AUTO.affix(&"x".repeat(1001), "").unwrap_err();
    /// assert_eq!(err.parameter(), Some("prefix"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn affix(&self, prefix: &str, suffix: &str) -> Result<TickFormat> {
        const CONTEXT: &str = "TickFormat.affix()";
        check_text(CONTEXT, "prefix", "prefix", prefix)?;
        check_text(CONTEXT, "suffix", "suffix", suffix)?;
        Ok(TickFormat {
            style: self.style,
            prefix: prefix.to_owned(),
            suffix: suffix.to_owned(),
        })
    }

    /// How the number in each label is written.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{NumberStyle, TickFormat};
    ///
    /// assert_eq!(TickFormat::default().style(), NumberStyle::Auto);
    /// ```
    #[must_use]
    pub fn style(&self) -> NumberStyle {
        self.style
    }

    /// The text written before every label; empty when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::TickFormat;
    ///
    /// assert_eq!(TickFormat::AUTO.affix("~", "")?.prefix(), "~");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// The text written after every label; empty when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::TickFormat;
    ///
    /// assert_eq!(TickFormat::percent(0)?.affix("", " p.a.")?.suffix(), " p.a.");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    /// Whether this is plain [`TickFormat::AUTO`]: the automatic style with no prefix or
    /// suffix. Only this format can be combined with tick labels given with their positions.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::TickFormat;
    ///
    /// assert!(TickFormat::AUTO.is_auto());
    /// assert!(!TickFormat::AUTO.affix("", "%")?.is_auto());
    /// assert!(!TickFormat::scientific().is_auto());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn is_auto(&self) -> bool {
        *self == TickFormat::AUTO
    }

    /// The format of `style` with no prefix or suffix.
    fn of(style: NumberStyle) -> TickFormat {
        TickFormat {
            style,
            prefix: String::new(),
            suffix: String::new(),
        }
    }

    /// `ticks` labelled in this format. [`NumberStyle::Auto`] keeps the labels the ticks carry,
    /// which are the automatic ones; the other styles write each tick's value, which on a
    /// computed grid is first snapped to zero as `values` says. The prefix and suffix then go
    /// around every label.
    pub(crate) fn relabel(&self, ticks: Vec<Tick>, values: TickValues) -> Vec<Tick> {
        let exact: Vec<f64> = ticks.iter().map(|tick| tick.value).collect();
        let written = || match values {
            TickValues::Grid { step } => ticks::snapped_to_zero(&exact, step),
            TickValues::Exact => exact.clone(),
        };
        let numbers = match self.style {
            NumberStyle::Auto => None,
            NumberStyle::Fixed(decimals) => Some(ticks::format_fixed(&written(), decimals.get())),
            NumberStyle::Percent(decimals) => {
                let percents: Vec<f64> = written().iter().map(|v| v * 100.0).collect();
                let labels = ticks::format_fixed(&percents, decimals.get());
                Some(labels.into_iter().map(|label| label + "%").collect())
            }
            NumberStyle::Scientific => Some(ticks::format_scientific(&written())),
        };
        let ticks: Vec<Tick> = match numbers {
            None => ticks,
            Some(labels) => ticks
                .into_iter()
                .zip(labels)
                .map(|(tick, label)| Tick { label, ..tick })
                .collect(),
        };
        if self.prefix.is_empty() && self.suffix.is_empty() {
            return ticks;
        }
        ticks
            .into_iter()
            .map(|tick| Tick {
                label: format!("{}{}{}", self.prefix, tick.label, self.suffix),
                ..tick
            })
            .collect()
    }
}

/// `decimals` as [`Decimals`], or `Error::Style` under `context` unless it is between 0 and 12.
fn checked_decimals(context: &'static str, decimals: i64) -> Result<Decimals> {
    match u8::try_from(decimals) {
        Ok(places) if decimals <= MAX_DECIMALS => Ok(Decimals(places)),
        _ => Err(Error::style(
            context,
            format!("decimals is {decimals}; decimals must be between 0 and {MAX_DECIMALS}."),
        )
        .with_parameter("decimals")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticks_of(values: &[f64]) -> Vec<Tick> {
        values
            .iter()
            .map(|&value| Tick {
                value,
                label: format!("auto {value}"),
            })
            .collect()
    }

    /// The labels of ticks at the exact `values`.
    fn labels(format: &TickFormat, values: &[f64]) -> Vec<String> {
        relabelled(format, values, TickValues::Exact)
    }

    fn relabelled(format: &TickFormat, values: &[f64], origin: TickValues) -> Vec<String> {
        format
            .relabel(ticks_of(values), origin)
            .into_iter()
            .map(|tick| tick.label)
            .collect()
    }

    #[track_caller]
    fn style_error(r: Result<TickFormat>) -> (String, Option<&'static str>) {
        match r {
            Err(e @ Error::Style { .. }) => (e.to_string(), e.parameter()),
            other => panic!("expected a Style error, got {other:?}"),
        }
    }

    #[test]
    fn decimals_are_bounded_to_0_through_12() {
        for d in [0, 1, 12] {
            assert!(TickFormat::fixed(d).is_ok());
            assert!(TickFormat::percent(d).is_ok());
        }
        for d in [-1, 13, 255, 256, i64::MIN, i64::MAX] {
            let (message, parameter) = style_error(TickFormat::fixed(d));
            assert_eq!(
                message,
                format!("TickFormat.fixed(): decimals is {d}; decimals must be between 0 and 12.")
            );
            assert_eq!(parameter, Some("decimals"));
            let (message, _) = style_error(TickFormat::percent(d));
            assert!(message.starts_with("TickFormat.percent(): "), "{message}");
        }
    }

    #[test]
    fn affix_replaces_and_keeps_the_style() {
        let base = TickFormat::fixed(1).unwrap();
        let money = base.affix("$", " M").unwrap();
        assert_eq!(money.style(), base.style());
        assert_eq!((money.prefix(), money.suffix()), ("$", " M"));
        let replaced = money.affix("", "!").unwrap();
        assert_eq!((replaced.prefix(), replaced.suffix()), ("", "!"));
        // The original is a value and does not change.
        assert_eq!(base.prefix(), "");
        assert!(!TickFormat::AUTO.affix("", "x").unwrap().is_auto());
        assert!(TickFormat::AUTO.affix("", "").unwrap().is_auto());
    }

    #[test]
    fn affix_text_is_capped_at_1000_characters() {
        // Characters are counted, not bytes.
        let at_cap = "é".repeat(1000);
        assert!(TickFormat::AUTO.affix(&at_cap, &at_cap).is_ok());
        let long = "é".repeat(1001);
        let (message, parameter) = style_error(TickFormat::AUTO.affix(&long, ""));
        assert_eq!(
            message,
            "TickFormat.affix(): prefix has 1,001 characters; text must be at most 1,000 characters."
        );
        assert_eq!(parameter, Some("prefix"));
        let err = TickFormat::AUTO.affix("", &long).unwrap_err();
        assert_eq!(err.parameter(), Some("suffix"));
        assert_eq!(
            err.hint(),
            Some("shorten the text; it would not fit the figure.")
        );
    }

    #[test]
    fn auto_keeps_the_automatic_labels_and_adds_the_affix() {
        assert_eq!(labels(&TickFormat::AUTO, &[0.0, 1.0]), ["auto 0", "auto 1"]);
        let affixed = TickFormat::AUTO.affix("~", " kg").unwrap();
        assert_eq!(labels(&affixed, &[2.0]), ["~auto 2 kg"]);
    }

    #[test]
    fn fixed_writes_every_value_with_its_decimals() {
        let fixed = |d: i64, values: &[f64]| labels(&TickFormat::fixed(d).unwrap(), values);
        assert_eq!(
            fixed(0, &[-2.0, 0.0, 2.5, 3.5]),
            ["\u{2212}2", "0", "2", "4"]
        );
        assert_eq!(
            fixed(2, &[-0.5, 0.0, 0.5]),
            ["\u{2212}0.50", "0.00", "0.50"]
        );
        assert_eq!(
            fixed(12, &[1.0 / 3.0, -1.0 / 3.0]),
            ["0.333333333333", "\u{2212}0.333333333333"]
        );
        // Zero is never signed, also for a value that only rounds to zero.
        assert_eq!(fixed(1, &[-0.0, -0.04, 1.0]), ["0.0", "0.0", "1.0"]);
        // No precision escalation: ticks may share a label.
        assert_eq!(fixed(0, &[1.0, 1.25, 1.5]), ["1", "1", "2"]);
        // On a computed grid, a tick that misses zero by a rounding error is zero.
        let grid = TickValues::Grid { step: 0.1 };
        let format = TickFormat::fixed(1).unwrap();
        assert_eq!(
            relabelled(&format, &[-0.1, 2.7e-17, 0.1], grid),
            ["\u{2212}0.1", "0.0", "0.1"]
        );
        // Exact values are written as they are, however far from their neighbours.
        let twelve = TickFormat::fixed(12).unwrap();
        assert_eq!(
            labels(&twelve, &[1e-10, 1.0]),
            ["0.000000000100", "1.000000000000"]
        );
        // Large values stay in fixed form.
        assert_eq!(fixed(0, &[0.0, 5e6]), ["0", "5000000"]);
    }

    #[test]
    fn percent_scales_by_100_and_rounds() {
        let percent = |d: i64, values: &[f64]| labels(&TickFormat::percent(d).unwrap(), values);
        assert_eq!(
            percent(0, &[-0.25, 0.0, 0.25, 0.5]),
            ["\u{2212}25%", "0%", "25%", "50%"]
        );
        assert_eq!(percent(1, &[0.125, 0.07]), ["12.5%", "7.0%"]);
        assert_eq!(percent(0, &[0.004, -0.004, 0.996]), ["0%", "0%", "100%"]);
        assert_eq!(percent(12, &[0.5]), ["50.000000000000%"]);
        // The suffix follows the percent sign.
        let share = TickFormat::percent(0)
            .unwrap()
            .affix("+", " share")
            .unwrap();
        assert_eq!(labels(&share, &[0.1]), ["+10% share"]);
    }

    #[test]
    fn scientific_writes_every_value_in_scientific_form() {
        let scientific = TickFormat::scientific();
        assert_eq!(
            labels(&scientific, &[-2500.0, 0.0, 0.5, 150.0, 1e6]),
            ["\u{2212}2.5e3", "0", "5e\u{2212}1", "1.5e2", "1e6"]
        );
        assert_eq!(labels(&scientific, &[-0.0]), ["0"]);
        // Only computed grid ticks are snapped to zero; exact values keep their own labels.
        let grid = TickValues::Grid { step: 1.0 };
        assert_eq!(
            relabelled(&scientific, &[-1e-17, 1.0, 2.0], grid),
            ["0", "1e0", "2e0"]
        );
        assert_eq!(labels(&scientific, &[1e-12, 1.0]), ["1e\u{2212}12", "1e0"]);
        let percent = TickFormat::percent(0).unwrap();
        assert_eq!(
            relabelled(
                &percent,
                &[-0.1, -1e-18, 0.1],
                TickValues::Grid { step: 0.1 }
            ),
            ["\u{2212}10%", "0%", "10%"]
        );
        // No escalation beyond 12 significant digits.
        assert_eq!(labels(&scientific, &[1.0, 1.0 + 1e-13]), ["1e0", "1e0"]);
    }
}
