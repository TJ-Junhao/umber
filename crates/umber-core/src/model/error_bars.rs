//! Error bar marks: the options given to the error bar methods and the stored error bar mark.

use std::fmt;
use std::sync::Arc;

use super::axis_data::AxisData;
use crate::{ColorSpec, SizeSpec};

/// The direction of the bars of an error bar mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::ErrorBarOrientation;
///
/// assert_ne!(ErrorBarOrientation::Vertical, ErrorBarOrientation::Horizontal);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorBarOrientation {
    /// Each bar runs along y at an x position; its limits are y values.
    Vertical,
    /// Each bar runs along x at a y position; its limits are x values.
    Horizontal,
}

/// Options of an error bar mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::ErrorBarOptions;
/// use umber_core::{ColorSpec, SizeSpec};
///
/// let options = ErrorBarOptions::default();
/// assert_eq!((options.color, options.width), (ColorSpec::AUTO, SizeSpec::Auto));
/// assert!(options.caps && options.label.is_empty());
///
/// let bare = ErrorBarOptions { caps: false, ..ErrorBarOptions::default() };
/// assert_ne!(bare, options);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorBarOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The color of the bars and their caps.
    pub color: ColorSpec,
    /// The line width of the bars and their caps; [`SizeSpec::Auto`] for the theme's line
    /// width.
    pub width: SizeSpec,
    /// Whether each end of a bar has a cap, a short line across it,
    /// [`Theme::cap_width`](crate::Theme::cap_width) long.
    pub caps: bool,
}

impl Default for ErrorBarOptions {
    fn default() -> Self {
        ErrorBarOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
            width: SizeSpec::Auto,
            caps: true,
        }
    }
}

/// An error bar mark: one bar per position, from a lower to an upper limit, and its
/// [`ErrorBarOptions`]. The bars of [`AxesMut::y_error_bars`](super::AxesMut::y_error_bars) are
/// [vertical](ErrorBarOrientation::Vertical), those of
/// [`AxesMut::x_error_bars`](super::AxesMut::x_error_bars)
/// [horizontal](ErrorBarOrientation::Horizontal).
///
/// The positions and limits are stored together and never change; clones share them. The
/// positions of horizontal bars are numbers. At every index where both limits are finite, low
/// does not exceed high.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, ErrorBarOptions, ErrorBarOrientation, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.y_error_bars(
///     AxisData::Numbers(vec![1.0, 2.0]),
///     &[0.5, 1.5],
///     &[1.5, 2.5],
///     ErrorBarOptions::default(),
/// )?;
/// let Some(Mark::ErrorBars(bars)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(bars.orientation(), ErrorBarOrientation::Vertical);
/// assert_eq!(bars.positions(), &AxisData::Numbers(vec![1.0, 2.0]));
/// assert_eq!(bars.low(), &[0.5, 1.5]);
/// assert_eq!(bars.high(), &[1.5, 2.5]);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// An error bar mark cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{ErrorBarMark, ErrorBarOptions, ErrorBarOrientation, Mark};
///
/// let mark = Mark::ErrorBars(ErrorBarMark {
///     orientation: ErrorBarOrientation::Vertical,
///     positions: unreachable!(),
///     low: unreachable!(),
///     high: unreachable!(),
///     options: ErrorBarOptions::default(),
/// });
/// ```
#[derive(Clone, PartialEq)]
pub struct ErrorBarMark {
    orientation: ErrorBarOrientation,
    positions: AxisData,
    low: Arc<[f64]>,
    high: Arc<[f64]>,
    options: ErrorBarOptions,
}

impl ErrorBarMark {
    /// Bars of `orientation` at validated `positions`, between validated limits of the same
    /// count.
    pub(super) fn new(
        orientation: ErrorBarOrientation,
        positions: AxisData,
        low: Arc<[f64]>,
        high: Arc<[f64]>,
        options: ErrorBarOptions,
    ) -> ErrorBarMark {
        ErrorBarMark {
            orientation,
            positions,
            low,
            high,
            options,
        }
    }

    /// The direction of the bars.
    #[must_use]
    pub fn orientation(&self) -> ErrorBarOrientation {
        self.orientation
    }

    /// Where the bars are: the x values of vertical bars, which are category names on a category
    /// x axis, and the y values of horizontal bars, which are numbers.
    #[must_use]
    pub fn positions(&self) -> &AxisData {
        &self.positions
    }

    /// The lower limit of each bar, as many as the positions.
    #[must_use]
    pub fn low(&self) -> &[f64] {
        &self.low
    }

    /// The upper limit of each bar, as many as the positions.
    #[must_use]
    pub fn high(&self) -> &[f64] {
        &self.high
    }

    /// Label, color, width and caps.
    #[must_use]
    pub fn options(&self) -> &ErrorBarOptions {
        &self.options
    }
}

impl fmt::Debug for ErrorBarMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorBarMark")
            .field("orientation", &self.orientation)
            .field("positions", &self.positions)
            .field("low", &self.low())
            .field("high", &self.high())
            .field("options", &self.options)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Figure, Mark};
    use crate::{Error, LogBase, ScaleKind, Theme};

    const LIN: ScaleKind = ScaleKind::Linear;
    const NAN: f64 = f64::NAN;

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    fn figure(x: ScaleKind, y: ScaleKind) -> Figure {
        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        figure.add_axes(x, y).unwrap();
        figure
    }

    /// What a refused call left to see: the error's variant name, text, parameter and hint.
    type Refusal = (
        &'static str,
        String,
        Option<&'static str>,
        Option<&'static str>,
    );

    #[track_caller]
    fn describe(f: &Figure, before: &Figure, err: &Error) -> Refusal {
        let kind = match err {
            Error::Data { .. } => "data",
            Error::Usage { .. } => "usage",
            Error::Style { .. } => "style",
            _ => "other",
        };
        assert_eq!(f, before);
        (kind, err.to_string(), err.parameter(), err.hint())
    }

    /// The error of `y_error_bars` on axes of scale kinds `kinds`, after one mark: it leaves the
    /// figure unchanged.
    #[track_caller]
    fn refused_y(
        (kind_x, kind_y): (ScaleKind, ScaleKind),
        x: AxisData,
        (low, high): (&[f64], &[f64]),
    ) -> Refusal {
        let mut f = figure(kind_x, kind_y);
        f.single_axes_mut()
            .y_error_bars(
                match kind_x {
                    ScaleKind::Category => AxisData::Categories(vec!["z".into()]),
                    _ => nums(&[1.0]),
                },
                &[1.0],
                &[2.0],
                ErrorBarOptions::default(),
            )
            .unwrap();
        let before = f.clone();
        let err = f
            .single_axes_mut()
            .y_error_bars(x, low, high, ErrorBarOptions::default())
            .unwrap_err();
        describe(&f, &before, &err)
    }

    /// The error of `x_error_bars` on axes of scale kinds `kinds`, after one mark.
    #[track_caller]
    fn refused_x(
        (kind_x, kind_y): (ScaleKind, ScaleKind),
        y: &[f64],
        (low, high): (&[f64], &[f64]),
    ) -> Refusal {
        let mut f = figure(kind_x, kind_y);
        if kind_x.transform().is_some() {
            f.single_axes_mut()
                .x_error_bars(&[1.0], &[1.0], &[2.0], ErrorBarOptions::default())
                .unwrap();
        }
        let before = f.clone();
        let err = f
            .single_axes_mut()
            .x_error_bars(y, low, high, ErrorBarOptions::default())
            .unwrap_err();
        describe(&f, &before, &err)
    }

    #[test]
    fn low_above_high_names_the_first_index() {
        let swapped = "check that low and high are not swapped.";
        let (kind, text, parameter, hint) = refused_y(
            (LIN, LIN),
            nums(&[0.0; 3]),
            (&[0.0, 5.0, 1.0], &[1.0, 4.0, 2.0]),
        );
        assert_eq!(kind, "data");
        assert_eq!(
            text,
            "Axes.y_error_bars(): low[1] is 5, above high[1] = 4; low must not exceed high."
        );
        assert_eq!((parameter, hint), (Some("low"), Some(swapped)));
        let (_, text, parameter, hint) =
            refused_x((LIN, LIN), &[0.0; 3], (&[0.0, 5.0, 1.0], &[1.0, 4.0, 2.0]));
        assert_eq!(
            text,
            "Axes.x_error_bars(): low[1] is 5, above high[1] = 4; low must not exceed high."
        );
        assert_eq!((parameter, hint), (Some("low"), Some(swapped)));
        // A gap in either limit is not compared; equal limits are allowed.
        let (_, text, ..) = refused_y(
            (LIN, LIN),
            nums(&[0.0; 4]),
            (&[NAN, 3.0, 9.0, 8.0], &[0.0, NAN, 2.0, 1.0]),
        );
        assert!(text.contains("low[2] is 9, above high[2] = 2"), "{text}");
        let mut f = figure(LIN, LIN);
        f.single_axes_mut()
            .y_error_bars(
                nums(&[0.0, 1.0]),
                &[2.0, NAN],
                &[2.0, 1.0],
                ErrorBarOptions::default(),
            )
            .unwrap();
    }

    #[test]
    fn lengths_come_first_then_positions_then_limits() {
        let (kind, text, parameter, hint) =
            refused_y((LIN, LIN), nums(&[0.0, 1.0]), (&[1.0], &[2.0, 3.0]));
        assert_eq!(kind, "data");
        assert_eq!(
            text,
            "Axes.y_error_bars(): x has 2 values but low has 1; x, low and high must have the same length."
        );
        assert_eq!((parameter, hint), (None, None));
        let (_, text, ..) = refused_y((LIN, LIN), nums(&[0.0, 1.0]), (&[1.0, 2.0], &[2.0]));
        assert_eq!(
            text,
            "Axes.y_error_bars(): x has 2 values but high has 1; x, low and high must have the same length."
        );
        let (_, text, ..) = refused_x((LIN, LIN), &[0.0, 1.0], (&[1.0, 2.0], &[2.0]));
        assert_eq!(
            text,
            "Axes.x_error_bars(): y has 2 values but high has 1; y, low and high must have the same length."
        );
        let gap = Some("use NaN to skip a bar.");
        let (_, text, parameter, hint) = refused_y(
            (LIN, LIN),
            nums(&[0.0, f64::INFINITY]),
            (&[1.0, 2.0], &[2.0, 3.0]),
        );
        assert_eq!(
            text,
            "Axes.y_error_bars(): x[1] is inf; values must be finite."
        );
        assert_eq!((parameter, hint), (Some("x"), gap));
        let (_, text, parameter, hint) = refused_y(
            (LIN, LIN),
            nums(&[0.0, 1.0]),
            (&[1.0, f64::NEG_INFINITY], &[2.0, 3.0]),
        );
        assert_eq!(
            text,
            "Axes.y_error_bars(): low[1] is -inf; values must be finite."
        );
        assert_eq!((parameter, hint), (Some("low"), gap));
        let (_, text, parameter, hint) = refused_y(
            (LIN, LIN),
            nums(&[0.0, 1.0]),
            (&[1.0, 1.0], &[f64::INFINITY, 3.0]),
        );
        assert_eq!(
            text,
            "Axes.y_error_bars(): high[0] is inf; values must be finite."
        );
        assert_eq!((parameter, hint), (Some("high"), gap));
        let (_, text, parameter, hint) = refused_x((LIN, LIN), &[f64::INFINITY], (&[1.0], &[2.0]));
        assert_eq!(
            text,
            "Axes.x_error_bars(): y[0] is inf; values must be finite."
        );
        assert_eq!((parameter, hint), (Some("y"), gap));
        // Length, then positions, then low, then high, then the order of the limits.
        let (_, text, ..) = refused_y((LIN, LIN), nums(&[f64::INFINITY]), (&[], &[]));
        assert!(text.contains("x has 1 values"), "{text}");
        let (_, text, ..) = refused_y(
            (LIN, LIN),
            nums(&[f64::INFINITY]),
            (&[f64::INFINITY], &[0.0]),
        );
        assert!(text.contains("x[0] is inf"), "{text}");
        let (_, text, ..) = refused_y(
            (LIN, LIN),
            nums(&[0.0, 1.0]),
            (&[5.0, f64::INFINITY], &[1.0, f64::INFINITY]),
        );
        assert!(text.contains("low[1] is inf"), "{text}");
        let (_, text, ..) = refused_y(
            (LIN, LIN),
            nums(&[0.0, 1.0]),
            (&[5.0, 1.0], &[1.0, f64::INFINITY]),
        );
        assert!(text.contains("high[1] is inf"), "{text}");
    }

    #[test]
    fn values_are_checked_on_the_axis_they_belong_to() {
        let log = ScaleKind::Log { base: LogBase::TEN };
        // y_error_bars: x on the x axis, limits on the y axis.
        let (_, text, parameter, _) =
            refused_y((log, LIN), nums(&[1.0, -1.0]), (&[0.0; 2], &[1.0; 2]));
        assert!(text.contains("x[1] is -1 on a log axis"), "{text}");
        assert_eq!(parameter, Some("x"));
        let (_, text, parameter, _) =
            refused_y((LIN, log), nums(&[1.0, 2.0]), (&[1.0, 0.0], &[2.0; 2]));
        assert!(text.contains("low[1] is 0 on a log axis"), "{text}");
        assert_eq!(parameter, Some("low"));
        // x_error_bars: y on the y axis, limits on the x axis.
        let (_, text, parameter, _) = refused_x((LIN, log), &[1.0, 0.0], (&[1.0; 2], &[2.0; 2]));
        assert!(text.contains("y[1] is 0 on a log axis"), "{text}");
        assert_eq!(parameter, Some("y"));
        let (_, text, parameter, _) =
            refused_x((log, LIN), &[1.0, 2.0], (&[1.0, 1.0], &[2.0, -3.0]));
        assert!(text.contains("high[1] is -3 on a log axis"), "{text}");
        assert_eq!(parameter, Some("high"));
    }

    #[test]
    fn x_error_bars_reject_a_category_x_axis_as_a_usage_error() {
        let (kind, text, parameter, hint) = refused_x(
            (ScaleKind::Category, LIN),
            &[0.0, 1.0],
            (&[0.0, 1.0], &[1.0, 2.0]),
        );
        assert_eq!(kind, "usage");
        assert_eq!(
            text,
            "Axes.x_error_bars(): this is a category axis; error bars along x apply to linear and log axes only."
        );
        assert_eq!(parameter, None);
        assert_eq!(
            hint,
            Some("x_error_bars needs a numeric x axis; use y_error_bars on a category.")
        );
        // The axis kind is checked before any value.
        let (kind, ..) = refused_x((ScaleKind::Category, LIN), &[0.0], (&[], &[]));
        assert_eq!(kind, "usage");
    }

    #[test]
    fn y_error_bars_check_the_kind_of_x_against_the_axis() {
        let (kind, text, _, hint) =
            refused_y((ScaleKind::Category, LIN), nums(&[0.0]), (&[0.0], &[1.0]));
        assert_eq!(kind, "data");
        assert_eq!(
            text,
            "Axes.y_error_bars(): x holds numbers but the x axis is a category axis; pass category names."
        );
        assert_eq!(
            hint,
            Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().")
        );
        let (_, text, ..) = refused_y(
            (LIN, LIN),
            AxisData::Categories(vec!["a".into()]),
            (&[0.0], &[1.0]),
        );
        assert!(text.contains("x holds category names"), "{text}");
    }

    #[test]
    fn error_bars_accept_empty_and_gapped_data_and_keep_every_bit() {
        let gap = f64::from_bits(0xfff8_0000_0000_0001);
        let low: Arc<[f64]> = Arc::from([1.0, gap, -0.0, 0.5]);
        let high: Arc<[f64]> = Arc::from([2.0, 3.0, 3.0, gap]);
        let options = ErrorBarOptions {
            label: "interval".into(),
            caps: false,
            ..ErrorBarOptions::default()
        };
        let mut f = figure(ScaleKind::Category, LIN);
        let names: Vec<String> = ["a", "b", "a", "c"].map(String::from).to_vec();
        f.single_axes_mut()
            .y_error_bars(
                AxisData::Categories(vec![]),
                &[],
                &[],
                ErrorBarOptions::default(),
            )
            .unwrap()
            .y_error_bars_shared(
                AxisData::Categories(names.clone()),
                Arc::clone(&low),
                Arc::clone(&high),
                options.clone(),
            )
            .unwrap();
        let [_, Mark::ErrorBars(bars)] = f.single_axes().marks() else {
            panic!("two error bar marks expected")
        };
        let bits = |v: &[f64]| v.iter().map(|b| b.to_bits()).collect::<Vec<_>>();
        assert_eq!(bars.positions(), &AxisData::Categories(names));
        assert_eq!(bits(bars.low()), bits(&low));
        assert_eq!(bits(bars.high()), bits(&high));
        assert_eq!(bars.options(), &options);
        assert_eq!(bars.orientation(), ErrorBarOrientation::Vertical);
        // The mark holds the caller's arrays, and clones read the same ones.
        assert!(std::ptr::eq(bars.low(), &*low));
        assert!(std::ptr::eq(bars.high(), &*high));
        let copy = f.clone();
        let [_, Mark::ErrorBars(cloned)] = copy.single_axes().marks() else {
            panic!("two error bar marks expected")
        };
        assert!(std::ptr::eq(bars.high(), cloned.high()));

        let mut f = figure(LIN, LIN);
        f.single_axes_mut()
            .x_error_bars_shared(
                nums(&[1.0, gap]),
                Arc::from([0.0, 1.0]),
                Arc::from([2.0, 3.0]),
                ErrorBarOptions::default(),
            )
            .unwrap();
        let [Mark::ErrorBars(bars)] = f.single_axes().marks() else {
            panic!("one error bar mark expected")
        };
        assert_eq!(bars.orientation(), ErrorBarOrientation::Horizontal);
        assert!(matches!(bars.positions(), AxisData::Numbers(y) if y[0] == 1.0 && y[1].is_nan()));
    }

    #[test]
    fn a_failed_shared_call_keeps_no_reference_to_the_values() {
        let low: Arc<[f64]> = Arc::from([3.0, 1.0]);
        let high: Arc<[f64]> = Arc::from([2.0, 2.0]);
        let mut f = figure(LIN, LIN);
        assert!(
            f.single_axes_mut()
                .y_error_bars_shared(
                    nums(&[0.0, 1.0]),
                    Arc::clone(&low),
                    Arc::clone(&high),
                    ErrorBarOptions::default()
                )
                .is_err()
        );
        assert!(
            f.single_axes_mut()
                .x_error_bars_shared(
                    nums(&[0.0, 1.0]),
                    Arc::clone(&low),
                    Arc::clone(&high),
                    ErrorBarOptions::default()
                )
                .is_err()
        );
        assert_eq!((Arc::strong_count(&low), Arc::strong_count(&high)), (1, 1));
        assert!(f.single_axes().marks().is_empty());
    }
}
