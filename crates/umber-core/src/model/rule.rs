//! Reference lines and spans: the value types, the options and the stored marks.

use std::fmt;
use std::slice;

use crate::{ColorSpec, Dash, SizeSpec};

/// A position on the x axis: a number on a linear or log axis, a category name on a category
/// axis.
///
/// # Examples
///
/// ```
/// use umber_core::AxisValue;
///
/// assert_ne!(AxisValue::Number(2.0), AxisValue::Category("2".into()));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum AxisValue {
    /// A value on a linear or log axis.
    Number(f64),
    /// A category name.
    Category(String),
}

/// Options of a reference line, horizontal or vertical.
///
/// # Examples
///
/// ```
/// use umber_core::figure::RuleOptions;
/// use umber_core::{ColorSpec, Dash, SizeSpec};
///
/// let options = RuleOptions::default();
/// assert_eq!((options.color, options.dash), (ColorSpec::AUTO, Dash::Solid));
/// assert_eq!(options.width, SizeSpec::Auto);
/// assert!(options.label.is_empty());
///
/// let dashed = RuleOptions { dash: Dash::Dashed, ..RuleOptions::default() };
/// assert_ne!(dashed, options);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RuleOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The line color.
    pub color: ColorSpec,
    /// The dash style.
    pub dash: Dash,
    /// The line width; [`SizeSpec::Auto`] for the theme's line width. Dashes scale with it.
    pub width: SizeSpec,
}

impl Default for RuleOptions {
    fn default() -> Self {
        RuleOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
            dash: Dash::Solid,
            width: SizeSpec::Auto,
        }
    }
}

/// Options of a reference span, horizontal or vertical.
///
/// # Examples
///
/// ```
/// use umber_core::figure::SpanOptions;
/// use umber_core::ColorSpec;
///
/// let options = SpanOptions::default();
/// assert_eq!(options.color, ColorSpec::AUTO);
/// assert!(options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SpanOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The fill color, drawn at the area opacity.
    pub color: ColorSpec,
}

impl Default for SpanOptions {
    fn default() -> Self {
        SpanOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
        }
    }
}

/// A horizontal reference line: a validated y and its [`RuleOptions`].
///
/// # Examples
///
/// ```
/// use umber_core::figure::{Mark, RuleOptions};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.hline(66.0, RuleOptions::default())?;
/// let Some(Mark::HLine(rule)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(rule.y(), 66.0);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A reference line cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{HLineMark, Mark, RuleOptions};
///
/// let mark = Mark::HLine(HLineMark { y: 1.0, options: RuleOptions::default() });
/// ```
#[derive(Clone, PartialEq)]
pub struct HLineMark {
    y: f64,
    options: RuleOptions,
}

impl HLineMark {
    /// A horizontal line at the validated `y`.
    pub(super) fn new(y: f64, options: RuleOptions) -> HLineMark {
        HLineMark { y, options }
    }

    /// The y value the line sits at.
    #[must_use]
    pub fn y(&self) -> f64 {
        self.y
    }

    /// Label, color, dash and width.
    #[must_use]
    pub fn options(&self) -> &RuleOptions {
        &self.options
    }

    /// The y value as a one-value slice, which is what the line contributes to the y domain.
    pub(crate) fn y_values(&self) -> &[f64] {
        slice::from_ref(&self.y)
    }
}

impl fmt::Debug for HLineMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HLineMark")
            .field("y", &self.y)
            .field("options", &self.options)
            .finish()
    }
}

/// A vertical reference line: a validated x and its [`RuleOptions`]. The x is a number on a
/// linear or log x axis and a category name on a category x axis; whether a name is one of the
/// axis's categories is known only when the figure is rendered.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{Mark, RuleOptions};
/// use umber_core::{AxisValue, Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// axes.vline(AxisValue::Category("Q3".into()), RuleOptions::default())?;
/// let Some(Mark::VLine(rule)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(rule.x(), &AxisValue::Category("Q3".into()));
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A reference line cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{AxisValue, Mark, RuleOptions, VLineMark};
///
/// let mark = Mark::VLine(VLineMark {
///     x: AxisValue::Number(1.0),
///     options: RuleOptions::default(),
/// });
/// ```
#[derive(Clone, PartialEq)]
pub struct VLineMark {
    x: AxisValue,
    options: RuleOptions,
}

impl VLineMark {
    /// A vertical line at the validated `x`.
    pub(super) fn new(x: AxisValue, options: RuleOptions) -> VLineMark {
        VLineMark { x, options }
    }

    /// The x value the line sits at.
    #[must_use]
    pub fn x(&self) -> &AxisValue {
        &self.x
    }

    /// Label, color, dash and width.
    #[must_use]
    pub fn options(&self) -> &RuleOptions {
        &self.options
    }

    /// The number the line sits at, as a one-value slice, which is what the line contributes to
    /// a continuous x domain; empty for a category.
    pub(crate) fn x_numbers(&self) -> &[f64] {
        match &self.x {
            AxisValue::Number(x) => slice::from_ref(x),
            AxisValue::Category(_) => &[],
        }
    }
}

impl fmt::Debug for VLineMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VLineMark")
            .field("x", &self.x)
            .field("options", &self.options)
            .finish()
    }
}

/// A reference span, the region between two values across the whole plot, along y for
/// [`Mark::HSpan`](super::Mark::HSpan) and along x for [`Mark::VSpan`](super::Mark::VSpan): two
/// validated values, the lower first, and the [`SpanOptions`].
///
/// # Examples
///
/// ```
/// use umber_core::figure::{Mark, SpanOptions};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.hspan(42.0, 58.0, SpanOptions::default())?;
/// let Some(Mark::HSpan(span)) = axes.marks().first() else { unreachable!() };
/// assert_eq!((span.low(), span.high()), (42.0, 58.0));
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A span cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{Mark, SpanMark, SpanOptions};
///
/// let mark = Mark::HSpan(SpanMark {
///     bounds: [1.0, 2.0],
///     options: SpanOptions::default(),
/// });
/// ```
#[derive(Clone, PartialEq)]
pub struct SpanMark {
    bounds: [f64; 2],
    options: SpanOptions,
}

impl SpanMark {
    /// A span between the validated `low` and `high`, with `low` below `high`.
    pub(super) fn new(low: f64, high: f64, options: SpanOptions) -> SpanMark {
        SpanMark {
            bounds: [low, high],
            options,
        }
    }

    /// The lower end of the span.
    #[must_use]
    pub fn low(&self) -> f64 {
        self.bounds[0]
    }

    /// The upper end of the span; above [`SpanMark::low`].
    #[must_use]
    pub fn high(&self) -> f64 {
        self.bounds[1]
    }

    /// Label and color.
    #[must_use]
    pub fn options(&self) -> &SpanOptions {
        &self.options
    }

    /// The two ends, low first, which is what the span contributes to the domain it runs along.
    pub(crate) fn bounds(&self) -> &[f64] {
        &self.bounds
    }
}

impl fmt::Debug for SpanMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpanMark")
            .field("low", &self.low())
            .field("high", &self.high())
            .field("options", &self.options)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxesMut, Figure, Mark};
    use crate::{Error, LogBase, ScaleKind, Theme};

    const LIN: ScaleKind = ScaleKind::Linear;
    const LOG: ScaleKind = ScaleKind::Log { base: LogBase::TEN };

    fn figure(x: ScaleKind, y: ScaleKind) -> Figure {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(x, y).unwrap();
        f
    }

    fn number(x: f64) -> AxisValue {
        AxisValue::Number(x)
    }

    fn name(x: &str) -> AxisValue {
        AxisValue::Category(x.to_owned())
    }

    /// What a refused call reports: the error, its text, its parameter and its hint. The figure
    /// must come out as it went in.
    type Refusal = (Error, String, Option<&'static str>, Option<&'static str>);

    fn refused(
        x: ScaleKind,
        y: ScaleKind,
        call: impl FnOnce(&mut AxesMut<'_>) -> crate::Result<()>,
    ) -> Refusal {
        let mut f = figure(x, y);
        // A mark that is already there stays, and a failed call adds none.
        f.single_axes_mut()
            .hline(1.0, RuleOptions::default())
            .unwrap();
        let before = f.clone();
        let err = call(&mut f.single_axes_mut()).unwrap_err();
        assert_eq!(f, before, "a failed call changes nothing");
        let (text, parameter, hint) = (err.to_string(), err.parameter(), err.hint());
        (err, text, parameter, hint)
    }

    #[test]
    fn the_four_methods_store_their_marks_in_call_order() {
        let mut f = figure(LIN, LIN);
        let options = RuleOptions {
            label: "Threshold".into(),
            dash: crate::Dash::Dashed,
            ..RuleOptions::default()
        };
        let span = SpanOptions {
            label: "Window".into(),
            ..SpanOptions::default()
        };
        f.single_axes_mut()
            .vspan(40.0, 50.0, span.clone())
            .unwrap()
            .hline(66.0, options.clone())
            .unwrap()
            .vline(number(31.0), RuleOptions::default())
            .unwrap()
            .hspan(42.0, 58.0, SpanOptions::default())
            .unwrap();
        let [
            Mark::VSpan(vspan),
            Mark::HLine(hline),
            Mark::VLine(vline),
            Mark::HSpan(hspan),
        ] = f.single_axes().marks()
        else {
            panic!("{:?}", f.single_axes().marks())
        };
        assert_eq!(
            (vspan.low(), vspan.high(), vspan.options()),
            (40.0, 50.0, &span)
        );
        assert_eq!((hline.y(), hline.options()), (66.0, &options));
        assert_eq!(vline.x(), &number(31.0));
        assert_eq!((hspan.low(), hspan.high()), (42.0, 58.0));
    }

    #[test]
    fn a_non_finite_value_is_a_data_error_naming_it() {
        let hint = Some("pass a finite number.");
        for (bad, shown) in [
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "NaN"),
        ] {
            let (err, text, parameter, found) =
                refused(LIN, LIN, |a| a.hline(bad, RuleOptions::default()).map(drop));
            assert!(matches!(err, Error::Data { .. }));
            assert_eq!(
                text,
                format!("Axes.hline(): y is {shown}; a reference line needs a finite value.")
            );
            assert_eq!((parameter, found), (Some("y"), hint));
            let (_, text, parameter, found) = refused(LIN, LIN, |a| {
                a.vline(number(bad), RuleOptions::default()).map(drop)
            });
            assert_eq!(
                text,
                format!("Axes.vline(): x is {shown}; a reference line needs a finite value.")
            );
            assert_eq!((parameter, found), (Some("x"), hint));
            let (_, text, parameter, found) = refused(LIN, LIN, |a| {
                a.hspan(0.0, bad, SpanOptions::default()).map(drop)
            });
            assert_eq!(
                text,
                format!("Axes.hspan(): high is {shown}; a reference span needs finite values.")
            );
            assert_eq!((parameter, found), (Some("high"), hint));
            let (_, text, parameter, _) = refused(LIN, LIN, |a| {
                a.vspan(bad, 1.0, SpanOptions::default()).map(drop)
            });
            assert_eq!(
                text,
                format!("Axes.vspan(): low is {shown}; a reference span needs finite values.")
            );
            assert_eq!(parameter, Some("low"));
        }
    }

    #[test]
    fn magnitudes_and_log_axes_are_checked_as_for_data() {
        let (_, text, parameter, hint) = refused(LIN, LIN, |a| {
            a.hline(1e301, RuleOptions::default()).map(drop)
        });
        assert_eq!(
            text,
            "Axes.hline(): y is 1e301; magnitudes must not exceed 1e300."
        );
        assert_eq!((parameter, hint), (Some("y"), None));
        let (_, text, parameter, hint) =
            refused(LIN, LOG, |a| a.hline(0.0, RuleOptions::default()).map(drop));
        assert_eq!(
            text,
            "Axes.hline(): y is 0 on a log axis; values must be positive."
        );
        assert_eq!(
            (parameter, hint),
            (
                Some("y"),
                Some("use a linear axis, or pass a positive value.")
            )
        );
        let (_, text, ..) = refused(LOG, LIN, |a| {
            a.vline(number(-2.0), RuleOptions::default()).map(drop)
        });
        assert_eq!(
            text,
            "Axes.vline(): x is -2 on a log axis; values must be positive."
        );
        let (_, text, _, hint) = refused(LOG, LIN, |a| {
            a.vspan(1e-301, 1.0, SpanOptions::default()).map(drop)
        });
        assert_eq!(
            text,
            "Axes.vspan(): low is 1e-301 on a log axis; values must be at least 1e-300."
        );
        assert_eq!(hint, Some("use a linear axis, or rescale the value."));
        // A positive value is fine on a log axis.
        let mut f = figure(LOG, LOG);
        f.single_axes_mut()
            .hline(0.5, RuleOptions::default())
            .unwrap()
            .vspan(1.0, 10.0, SpanOptions::default())
            .unwrap();
    }

    #[test]
    fn a_span_needs_low_below_high() {
        let (err, text, parameter, hint) = refused(LIN, LIN, |a| {
            a.hspan(5.0, 2.0, SpanOptions::default()).map(drop)
        });
        assert!(matches!(err, Error::Style { .. }));
        assert_eq!(
            text,
            "Axes.hspan(): low is 5, high is 2; low must be below high."
        );
        assert_eq!(
            (parameter, hint),
            (Some("high"), Some("swap the two values."))
        );
        let (_, text, ..) = refused(LIN, LIN, |a| {
            a.vspan(2.0, 2.0, SpanOptions::default()).map(drop)
        });
        assert_eq!(
            text,
            "Axes.vspan(): low is 2, high is 2; low must be below high."
        );
        // A bad value is reported before the order of the two.
        let (err, text, ..) = refused(LIN, LIN, |a| {
            a.hspan(5.0, f64::NAN, SpanOptions::default()).map(drop)
        });
        assert!(matches!(err, Error::Data { .. }), "{text}");
        let (_, text, ..) = refused(LIN, LIN, |a| {
            a.hspan(f64::NAN, f64::NAN, SpanOptions::default())
                .map(drop)
        });
        assert!(text.contains("low is NaN"), "{text}");
        // Large magnitudes are written as everywhere else.
        let (_, text, ..) = refused(LIN, LIN, |a| {
            a.hspan(1e7, 1e6, SpanOptions::default()).map(drop)
        });
        assert_eq!(
            text,
            "Axes.hspan(): low is 1e7, high is 1e6; low must be below high."
        );
    }

    #[test]
    fn a_vspan_on_a_category_axis_is_a_usage_error() {
        let (err, text, parameter, hint) = refused(ScaleKind::Category, LIN, |a| {
            a.vspan(0.0, 1.0, SpanOptions::default()).map(drop)
        });
        assert!(matches!(err, Error::Usage { .. }));
        assert_eq!(
            text,
            "Axes.vspan(): this is a category axis; spans along x apply to linear and log axes only."
        );
        assert_eq!(parameter, None);
        assert_eq!(
            hint,
            Some("vspan needs a numeric x axis; use vline on a category.")
        );
        // The kind is checked before the values.
        let (err, ..) = refused(ScaleKind::Category, LIN, |a| {
            a.vspan(f64::NAN, 1.0, SpanOptions::default()).map(drop)
        });
        assert!(matches!(err, Error::Usage { .. }));
        // hspan is fine on a category axis.
        let mut f = figure(ScaleKind::Category, LIN);
        f.single_axes_mut()
            .hspan(1.0, 2.0, SpanOptions::default())
            .unwrap();
    }

    #[test]
    fn a_vline_must_match_the_kind_of_the_x_axis() {
        let (err, text, parameter, hint) = refused(ScaleKind::Category, LIN, |a| {
            a.vline(number(1.0), RuleOptions::default()).map(drop)
        });
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(
            text,
            "Axes.vline(): x holds a number but the x axis is a category axis; pass a category name."
        );
        assert_eq!(
            (parameter, hint),
            (
                Some("x"),
                Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().")
            )
        );
        let (_, text, parameter, hint) = refused(LIN, LIN, |a| {
            a.vline(name("Q3"), RuleOptions::default()).map(drop)
        });
        assert_eq!(
            text,
            "Axes.vline(): x holds a category name but the x axis is not a category axis; pass a number."
        );
        assert_eq!(
            (parameter, hint),
            (
                Some("x"),
                Some("pass x=Scale.category() to Figure.add_axes().")
            )
        );
    }

    #[test]
    fn an_unknown_category_is_not_an_error_when_the_line_is_added() {
        // Whether the category exists is decided when the figure is rendered, since a mark
        // added later may bring it.
        let mut f = figure(ScaleKind::Category, LIN);
        f.single_axes_mut()
            .vline(name("Q5"), RuleOptions::default())
            .unwrap();
        assert_eq!(f.single_axes().marks().len(), 1);
    }

    #[test]
    fn domain_slices_hold_the_values_the_marks_contribute() {
        let mut f = figure(LIN, LIN);
        f.single_axes_mut()
            .hline(1.0, RuleOptions::default())
            .unwrap()
            .vline(number(2.0), RuleOptions::default())
            .unwrap()
            .vspan(3.0, 4.0, SpanOptions::default())
            .unwrap();
        let [Mark::HLine(h), Mark::VLine(v), Mark::VSpan(s)] = f.single_axes().marks() else {
            panic!()
        };
        assert_eq!(h.y_values(), [1.0]);
        assert_eq!(v.x_numbers(), [2.0]);
        assert_eq!(s.bounds(), [3.0, 4.0]);
        let mut f = figure(ScaleKind::Category, LIN);
        f.single_axes_mut()
            .vline(name("a"), RuleOptions::default())
            .unwrap();
        let [Mark::VLine(v)] = f.single_axes().marks() else {
            panic!()
        };
        assert!(v.x_numbers().is_empty());
    }
}
