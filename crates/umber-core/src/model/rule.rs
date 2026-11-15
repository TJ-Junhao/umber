//! Reference lines and spans: the value types, the options and the stored marks.

use std::fmt;
use std::slice;

use crate::error::quoted;
use crate::text::Lines;
use crate::{ColorSpec, Dash, Error, Instant, Result, SizeSpec};

/// A position on an axis: a number on a linear or log axis, a category name on a category axis,
/// a time on a time axis.
///
/// # Examples
///
/// ```
/// use umber_core::{AxisValue, Instant};
///
/// assert_ne!(AxisValue::Number(2.0), AxisValue::Category("2".into()));
/// assert_ne!(AxisValue::Time(Instant::from_micros(2)?), AxisValue::Number(2.0));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum AxisValue {
    /// A value on a linear or log axis.
    Number(f64),
    /// A category name.
    Category(String),
    /// A time on a time axis.
    Time(Instant),
}

impl AxisValue {
    /// The number, as a one-value slice; empty for a category name or a time.
    pub(crate) fn numbers(&self) -> &[f64] {
        match self {
            AxisValue::Number(value) => slice::from_ref(value),
            AxisValue::Category(_) | AxisValue::Time(_) => &[],
        }
    }

    /// The time, as a one-value slice; empty for a number or a category name.
    pub(crate) fn times(&self) -> &[Instant] {
        match self {
            AxisValue::Time(t) => slice::from_ref(t),
            AxisValue::Number(_) | AxisValue::Category(_) => &[],
        }
    }
}

/// The point halfway between the centres of two neighbouring categories of a category axis:
/// where a reference line divides them, or where a reference span covering whole categories
/// ends. The two names may come in either order. Whether they are categories of the axis, and
/// neighbours there, is known only when the figure is rendered, since a mark added later may
/// bring them.
///
/// # Examples
///
/// ```
/// use umber_core::Between;
///
/// let divider = Between::new("2013", "2014")?;
/// assert_eq!((divider.first(), divider.second()), ("2013", "2014"));
/// assert_eq!(divider.to_string(), r#"Between("2013", "2014")"#);
/// assert!(Between::new("2013", "2013").is_err());
/// assert!(Between::new("2026\n(to Oct)", "2025").is_ok());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Between {
    first: String,
    second: String,
}

impl Between {
    /// The point between the categories `first` and `second`.
    ///
    /// # Errors
    ///
    /// `Error::Data`, naming the parameter, when a name holds a control character other than
    /// `"\n"` or a line or paragraph separator, as the name of a category may not; then
    /// `Error::Usage`, naming `second`, when both names are the same.
    pub fn new(first: impl Into<String>, second: impl Into<String>) -> Result<Between> {
        const CONTEXT: &str = "Between()";
        let (first, second) = (first.into(), second.into());
        for (parameter, name) in [("first", &first), ("second", &second)] {
            Lines::Many
                .check_data(CONTEXT, parameter, name)
                .map_err(|error| error.with_parameter(parameter))?;
        }
        if first == second {
            return Err(Error::usage(
                CONTEXT,
                format!(
                    "first and second are both {}; a point between categories needs two \
                     categories.",
                    quoted(&first)
                ),
            )
            .with_parameter("second")
            .with_hint("use two neighbouring categories."));
        }
        Ok(Between { first, second })
    }

    /// The first name.
    #[must_use]
    pub fn first(&self) -> &str {
        &self.first
    }

    /// The second name.
    #[must_use]
    pub fn second(&self) -> &str {
        &self.second
    }

    /// The two names, first then second.
    pub(crate) fn names(&self) -> [&str; 2] {
        [&self.first, &self.second]
    }
}

impl fmt::Display for Between {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Between({}, {})",
            quoted(&self.first),
            quoted(&self.second)
        )
    }
}

/// Where a reference line sits along its axis: at a value of the axis, or between two
/// neighbouring categories of a category axis.
///
/// # Examples
///
/// ```
/// use umber_core::{AxisValue, Between, RulePosition};
///
/// let at = RulePosition::At(AxisValue::Category("2013".into()));
/// let between = RulePosition::Between(Between::new("2013", "2014")?);
/// assert_ne!(at, between);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum RulePosition {
    /// At a value: a number, a category's centre or a time.
    At(AxisValue),
    /// Halfway between the centres of two neighbouring categories.
    Between(Between),
}

impl RulePosition {
    /// The number, as a one-value slice; empty for any other position.
    pub(crate) fn numbers(&self) -> &[f64] {
        match self {
            RulePosition::At(value) => value.numbers(),
            RulePosition::Between(_) => &[],
        }
    }

    /// The time, as a one-value slice; empty for any other position.
    pub(crate) fn times(&self) -> &[Instant] {
        match self {
            RulePosition::At(value) => value.times(),
            RulePosition::Between(_) => &[],
        }
    }
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
    /// The fill color, drawn at the area opacity; a translucent color is used as given.
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

/// A horizontal reference line: a validated y and its [`RuleOptions`]. The y is a number on a
/// linear or log y axis, a time on a time y axis, and a [`Between`] on a category y axis.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{Mark, RuleOptions};
/// use umber_core::{AxisValue, Figure, RulePosition, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.hline(66.0, RuleOptions::default())?;
/// let Some(Mark::HLine(rule)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(rule.y(), &RulePosition::At(AxisValue::Number(66.0)));
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A reference line cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{AxisValue, HLineMark, Mark, RuleOptions, RulePosition};
///
/// let y = RulePosition::At(AxisValue::Number(1.0));
/// let mark = Mark::HLine(HLineMark { y, options: RuleOptions::default() });
/// ```
#[derive(Clone, PartialEq)]
pub struct HLineMark {
    y: RulePosition,
    options: RuleOptions,
}

impl HLineMark {
    /// A horizontal line at the validated `y`.
    pub(super) fn new(y: RulePosition, options: RuleOptions) -> HLineMark {
        HLineMark { y, options }
    }

    /// Where the line sits along y: at a number, at a time on a time axis, or between two
    /// categories on a category axis.
    #[must_use]
    pub fn y(&self) -> &RulePosition {
        &self.y
    }

    /// Label, color, dash and width.
    #[must_use]
    pub fn options(&self) -> &RuleOptions {
        &self.options
    }

    /// The number the line sits at, as a one-value slice, which is what the line contributes to
    /// a linear or log y domain; empty for a time or a point between categories.
    pub(crate) fn y_values(&self) -> &[f64] {
        self.y.numbers()
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
/// linear or log x axis, a time on a time axis, and a category name or a [`Between`] on a
/// category x axis; whether a name is one of the axis's categories is known only when the figure
/// is rendered.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{Mark, RuleOptions};
/// use umber_core::{AxisValue, Figure, RulePosition, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// axes.vline(AxisValue::Category("Q3".into()), RuleOptions::default())?;
/// let Some(Mark::VLine(rule)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(rule.x(), &RulePosition::At(AxisValue::Category("Q3".into())));
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A reference line cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{AxisValue, Mark, RuleOptions, RulePosition, VLineMark};
///
/// let mark = Mark::VLine(VLineMark {
///     x: RulePosition::At(AxisValue::Number(1.0)),
///     options: RuleOptions::default(),
/// });
/// ```
#[derive(Clone, PartialEq)]
pub struct VLineMark {
    x: RulePosition,
    options: RuleOptions,
}

impl VLineMark {
    /// A vertical line at the validated `x`.
    pub(super) fn new(x: RulePosition, options: RuleOptions) -> VLineMark {
        VLineMark { x, options }
    }

    /// Where the line sits along x: at a value, or between two categories on a category axis.
    #[must_use]
    pub fn x(&self) -> &RulePosition {
        &self.x
    }

    /// Label, color, dash and width.
    #[must_use]
    pub fn options(&self) -> &RuleOptions {
        &self.options
    }

    /// The number the line sits at, as a one-value slice, which is what the line contributes to
    /// a continuous x domain; empty for any other position.
    pub(crate) fn x_numbers(&self) -> &[f64] {
        self.x.numbers()
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

/// The ends of a reference span: two numbers on a linear or log axis, two times on a time axis,
/// the lower first, or two points between categories on a category axis, given first as `low`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SpanBounds {
    /// Two numbers.
    Numbers([f64; 2]),
    /// Two times.
    Times([Instant; 2]),
    /// Two points between categories, `low` then `high`; their order along the axis is checked
    /// when the figure is rendered.
    Between([Between; 2]),
}

/// A reference span, the region between two values across the whole plot, along y for
/// [`Mark::HSpan`](super::Mark::HSpan) and along x for [`Mark::VSpan`](super::Mark::VSpan): two
/// validated values, the lower first, numbers, times or points between categories, and the
/// [`SpanOptions`].
///
/// # Examples
///
/// ```
/// use umber_core::figure::{Mark, SpanOptions};
/// use umber_core::{AxisValue, Figure, RulePosition, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.hspan(42.0, 58.0, SpanOptions::default())?;
/// let Some(Mark::HSpan(span)) = axes.marks().first() else { unreachable!() };
/// let (low, high) = (AxisValue::Number(42.0), AxisValue::Number(58.0));
/// assert_eq!((span.low(), span.high()), (RulePosition::At(low), RulePosition::At(high)));
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A span cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{Mark, SpanMark, SpanOptions};
///
/// let mark = Mark::HSpan(SpanMark { options: SpanOptions::default() });
/// ```
#[derive(Clone, PartialEq)]
pub struct SpanMark {
    bounds: SpanBounds,
    options: SpanOptions,
}

impl SpanMark {
    /// A span between the validated `bounds`, the lower first.
    pub(super) fn new(bounds: SpanBounds, options: SpanOptions) -> SpanMark {
        SpanMark { bounds, options }
    }

    /// The lower end of the span: a number, a time on a time axis, or a point between
    /// categories on a category axis.
    #[must_use]
    pub fn low(&self) -> RulePosition {
        match &self.bounds {
            SpanBounds::Numbers([low, _]) => RulePosition::At(AxisValue::Number(*low)),
            SpanBounds::Times([low, _]) => RulePosition::At(AxisValue::Time(*low)),
            SpanBounds::Between(ends) => RulePosition::Between(ends[0].clone()),
        }
    }

    /// The upper end of the span, above [`SpanMark::low`].
    #[must_use]
    pub fn high(&self) -> RulePosition {
        match &self.bounds {
            SpanBounds::Numbers([_, high]) => RulePosition::At(AxisValue::Number(*high)),
            SpanBounds::Times([_, high]) => RulePosition::At(AxisValue::Time(*high)),
            SpanBounds::Between(ends) => RulePosition::Between(ends[1].clone()),
        }
    }

    /// The two points between categories the span runs between, `low` first; `None` for a
    /// span between numbers or times.
    pub(crate) fn between(&self) -> Option<&[Between; 2]> {
        match &self.bounds {
            SpanBounds::Between(ends) => Some(ends),
            SpanBounds::Numbers(_) | SpanBounds::Times(_) => None,
        }
    }

    /// Label and color.
    #[must_use]
    pub fn options(&self) -> &SpanOptions {
        &self.options
    }

    /// The two ends as numbers, low first, which is what the span contributes to the linear or
    /// log domain it runs along; empty for times.
    pub(crate) fn bounds(&self) -> &[f64] {
        match &self.bounds {
            SpanBounds::Numbers(bounds) => bounds,
            SpanBounds::Times(_) | SpanBounds::Between(_) => &[],
        }
    }

    /// The two ends as times, low first, which is what the span contributes to the time domain
    /// it runs along; empty for numbers.
    pub(crate) fn time_bounds(&self) -> &[Instant] {
        match &self.bounds {
            SpanBounds::Times(bounds) => bounds,
            SpanBounds::Numbers(_) | SpanBounds::Between(_) => &[],
        }
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
        let at = |x: f64| RulePosition::At(number(x));
        assert_eq!(
            (vspan.low(), vspan.high(), vspan.options()),
            (at(40.0), at(50.0), &span)
        );
        assert_eq!((hline.y(), hline.options()), (&at(66.0), &options));
        assert_eq!(vline.x(), &at(31.0));
        assert_eq!((hspan.low(), hspan.high()), (at(42.0), at(58.0)));
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
            "Axes.vspan(): low is a number but the x axis is a category axis; place it between two \
             categories instead."
        );
        assert_eq!(parameter, Some("low"));
        assert_eq!(
            hint,
            Some("pass Between(a, b), with a and b two neighbouring categories.")
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

    fn between(first: &str, second: &str) -> Between {
        Between::new(first, second).unwrap()
    }

    #[test]
    fn a_between_takes_two_different_category_names() {
        let divider = between("2026\n(to Oct)", "2025");
        assert_eq!(divider.names(), ["2026\n(to Oct)", "2025"]);
        assert_eq!(
            divider.to_string(),
            "Between(\"2026\\n(to Oct)\", \"2025\")"
        );
        let err = Between::new("2013", "2013").unwrap_err();
        assert!(matches!(err, Error::Usage { .. }), "{err}");
        assert_eq!(
            err.to_string(),
            "Between(): first and second are both \"2013\"; a point between categories needs two \
             categories."
        );
        assert_eq!(
            (err.parameter(), err.hint()),
            (Some("second"), Some("use two neighbouring categories."))
        );
        // A name follows the rules of category names: line breaks, and no other control
        // character.
        let err = Between::new("a\tb", "c").unwrap_err();
        assert!(matches!(err, Error::Data { .. }), "{err}");
        assert_eq!(
            err.to_string(),
            "Between(): first holds U+0009 (a tab) at index 1; text cannot hold control characters."
        );
        assert_eq!(err.parameter(), Some("first"));
        let err = Between::new("a", "b\u{2028}").unwrap_err();
        assert_eq!(err.parameter(), Some("second"));
    }

    #[test]
    fn lines_and_spans_between_categories_are_stored_in_call_order() {
        let mut f = figure(ScaleKind::Category, LIN);
        let span = SpanOptions {
            label: "Years".into(),
            ..SpanOptions::default()
        };
        f.single_axes_mut()
            .vline_between(between("b", "a"), RuleOptions::default())
            .unwrap()
            .vspan_between(between("a", "b"), between("c", "d"), span.clone())
            .unwrap();
        let [Mark::VLine(line), Mark::VSpan(stored)] = f.single_axes().marks() else {
            panic!("{:?}", f.single_axes().marks())
        };
        assert_eq!(line.x(), &RulePosition::Between(between("b", "a")));
        assert!(line.x_numbers().is_empty());
        assert_eq!(
            (stored.low(), stored.high(), stored.options()),
            (
                RulePosition::Between(between("a", "b")),
                RulePosition::Between(between("c", "d")),
                &span
            )
        );
        assert!(stored.bounds().is_empty() && stored.time_bounds().is_empty());

        let mut f = figure(LIN, ScaleKind::Category);
        f.single_axes_mut()
            .hline_between(between("a", "b"), RuleOptions::default())
            .unwrap()
            .hspan_between(between("a", "b"), between("b", "c"), SpanOptions::default())
            .unwrap();
        let [Mark::HLine(line), Mark::HSpan(stored)] = f.single_axes().marks() else {
            panic!("{:?}", f.single_axes().marks())
        };
        assert_eq!(line.y(), &RulePosition::Between(between("a", "b")));
        assert!(line.y_values().is_empty());
        assert_eq!(
            stored.between(),
            Some(&[between("a", "b"), between("b", "c")])
        );
    }

    #[test]
    fn a_between_needs_a_category_axis_beside_a_numeric_one() {
        let refuse =
            |x: ScaleKind, y: ScaleKind, call: &dyn Fn(&mut AxesMut<'_>) -> crate::Result<()>| {
                let mut f = figure(x, y);
                let before = f.clone();
                let err = call(&mut f.single_axes_mut()).unwrap_err();
                assert_eq!(f, before, "a failed call changes nothing");
                assert!(matches!(err, Error::Usage { .. }), "{err}");
                (err.to_string(), err.parameter(), err.hint())
            };
        let vline = |a: &mut AxesMut<'_>| {
            a.vline_between(between("a", "b"), RuleOptions::default())
                .map(drop)
        };
        let hline = |a: &mut AxesMut<'_>| {
            a.hline_between(between("a", "b"), RuleOptions::default())
                .map(drop)
        };
        let vspan = |a: &mut AxesMut<'_>| {
            a.vspan_between(between("a", "b"), between("b", "c"), SpanOptions::default())
                .map(drop)
        };
        let hspan = |a: &mut AxesMut<'_>| {
            a.hspan_between(between("a", "b"), between("b", "c"), SpanOptions::default())
                .map(drop)
        };
        let x_hint =
            Some("pass a value of the x axis, or pass x=Scale.category() to Figure.add_axes().");
        let y_hint =
            Some("pass a value of the y axis, or pass y=Scale.category() to Figure.add_axes().");
        for (kind, shown) in [(LIN, "linear"), (LOG, "log"), (ScaleKind::Time, "time")] {
            assert_eq!(
                refuse(kind, LIN, &vline),
                (
                    format!(
                        "Axes.vline(): x is a point between categories but the x axis is a \
                         {shown} axis; Between needs a category axis."
                    ),
                    Some("x"),
                    x_hint
                )
            );
            assert_eq!(
                refuse(kind, LIN, &vspan),
                (
                    format!(
                        "Axes.vspan(): low is a point between categories but the x axis is a \
                         {shown} axis; Between needs a category axis."
                    ),
                    Some("low"),
                    x_hint
                )
            );
            assert_eq!(
                refuse(LIN, kind, &hline),
                (
                    format!(
                        "Axes.hline(): y is a point between categories but the y axis is a \
                         {shown} axis; Between needs a category axis."
                    ),
                    Some("y"),
                    y_hint
                )
            );
            assert_eq!(refuse(LIN, kind, &hspan).1, Some("low"));
        }
        // Two category axes, as of a heatmap, take no reference line or span, and a category y
        // axis needs a linear or log x axis beside it, as horizontal bars do.
        let numeric_x = Some("use a linear or log x axis.");
        let cat = ScaleKind::Category;
        for (call, context) in [
            (
                &vline as &dyn Fn(&mut AxesMut<'_>) -> crate::Result<()>,
                "Axes.vline()",
            ),
            (&hline, "Axes.hline()"),
            (&vspan, "Axes.vspan()"),
            (&hspan, "Axes.hspan()"),
        ] {
            assert_eq!(
                refuse(cat, cat, call),
                (
                    format!("{context}: x is a category scale; {context} needs a numeric x axis."),
                    Some("x"),
                    numeric_x
                )
            );
        }
        // Beside a category y axis the x axis may be linear, log or time, as for events in rows.
        for x in [LIN, LOG, ScaleKind::Time] {
            let mut f = figure(x, cat);
            let mut a = f.single_axes_mut();
            hline(&mut a).unwrap();
            hspan(&mut a).unwrap();
            assert_eq!(f.single_axes().marks().len(), 2, "{x:?}");
        }
        // A number on a category axis is refused with the way to place it between categories.
        let (text, parameter, hint) = refuse(LIN, cat, &|a: &mut AxesMut<'_>| {
            a.hspan(0.0, 1.0, SpanOptions::default()).map(drop)
        });
        assert_eq!(
            text,
            "Axes.hspan(): low is a number but the y axis is a category axis; place it between \
             two categories instead."
        );
        assert_eq!(
            (parameter, hint),
            (
                Some("low"),
                Some("pass Between(a, b), with a and b two neighbouring categories.")
            )
        );
        // Whether the names are neighbouring categories is decided when the figure is rendered.
        let mut f = figure(cat, LIN);
        f.single_axes_mut()
            .vline_between(between("x", "z"), RuleOptions::default())
            .unwrap();
        assert_eq!(f.single_axes().marks().len(), 1);
    }
}
