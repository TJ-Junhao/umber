//! Text written at data positions or at positions in the plot: the options of
//! [`AxesMut::text`](super::AxesMut::text) and [`AxesMut::annotate`](super::AxesMut::annotate),
//! and the stored [`Annotation`].
//!
//! Annotations are not marks: they take no palette color, no legend entry and no part in the
//! limits, and the layout reserves no room for them. They are drawn over the axes' marks.

use super::rule::AxisValue;
use crate::error::number;
use crate::text::Lines;
use crate::{
    Align, ColorSpec, Error, MAX_MAGNITUDE, MIN_LOG_VALUE, Result, ScaleKind, SizeSpec, Transform,
};

/// The hint of a coordinate whose kind does not suit its axis.
const KIND_HINT: &str = "pass a number on a linear or log axis, a category name on a category \
                         axis and a time on a time axis.";

/// The offset of [`AxesMut::annotate`](super::AxesMut::annotate) unless the options give one.
const DEFAULT_OFFSET: (f64, f64) = (0.0, 12.0);

/// A position given as fractions of the plot area: `(0, 0)` is its lower left corner and
/// `(1, 1)` its upper right, whatever the scales and limits of the axes are.
///
/// # Examples
///
/// ```
/// use umber_core::InPlot;
///
/// let corner = InPlot::new(0.02, 0.96)?;
/// assert_eq!((corner.x(), corner.y()), (0.02, 0.96));
/// assert!(InPlot::new(1.5, 0.5).is_err());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InPlot {
    x: f64,
    y: f64,
}

impl InPlot {
    /// The position at the fractions `x` of the plot's width from its left edge and `y` of its
    /// height from its bottom edge.
    ///
    /// # Errors
    ///
    /// `Error::Data`, naming the parameter `x` or `y`, for the first fraction, x first, that is
    /// not finite or lies outside `[0, 1]`: text placed there would leave the plot.
    pub fn new(x: f64, y: f64) -> Result<InPlot> {
        for (name, value) in [("x", x), ("y", y)] {
            if !(0.0..=1.0).contains(&value) {
                return Err(Error::data(
                    "InPlot()",
                    format!(
                        "{name} is {}; a position in the plot is a fraction from 0 to 1.",
                        number(value)
                    ),
                )
                .with_parameter(name)
                .with_hint(
                    "pass a fraction in [0, 1], or use caption() for text outside the plot.",
                ));
            }
        }
        // `-0.0` is a zero like any other.
        Ok(InPlot {
            x: x + 0.0,
            y: y + 0.0,
        })
    }

    /// The fraction of the plot's width, from its left edge: finite, in `[0, 1]`.
    #[must_use]
    pub fn x(&self) -> f64 {
        self.x
    }

    /// The fraction of the plot's height, from its bottom edge: finite, in `[0, 1]`.
    #[must_use]
    pub fn y(&self) -> f64 {
        self.y
    }
}

/// Where [`AxesMut::text`](super::AxesMut::text) writes: at a data position, or at a position
/// in the plot that no scale or limit moves.
///
/// # Examples
///
/// ```
/// use umber_core::{AxisValue, InPlot, TextPosition};
///
/// let data = TextPosition::from((AxisValue::Number(1.0), AxisValue::Number(2.0)));
/// let corner = TextPosition::from(InPlot::new(0.0, 1.0)?);
/// assert_ne!(data, corner);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum TextPosition {
    /// A point `(x, y)`, each in the values of its axis.
    Data(AxisValue, AxisValue),
    /// A position as fractions of the plot area.
    InPlot(InPlot),
}

impl From<(AxisValue, AxisValue)> for TextPosition {
    fn from((x, y): (AxisValue, AxisValue)) -> TextPosition {
        TextPosition::Data(x, y)
    }
}

impl From<InPlot> for TextPosition {
    fn from(position: InPlot) -> TextPosition {
        TextPosition::InPlot(position)
    }
}

/// Options of [`AxesMut::text`](super::AxesMut::text).
///
/// # Examples
///
/// ```
/// use umber_core::{Align, ColorSpec, SizeSpec, TextOptions};
///
/// let options = TextOptions::default();
/// assert_eq!(options.align, Align::Left);
/// assert_eq!((options.size, options.color), (SizeSpec::Auto, ColorSpec::AUTO));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextOptions {
    /// Which edge of the text, or its centre, is at the point.
    pub align: Align,
    /// The text size; [`SizeSpec::Auto`] for the theme's body size.
    pub size: SizeSpec,
    /// The text color; [`ColorSpec::AUTO`] for the theme's text color.
    pub color: ColorSpec,
}

impl Default for TextOptions {
    fn default() -> Self {
        TextOptions {
            align: Align::Left,
            size: SizeSpec::Auto,
            color: ColorSpec::AUTO,
        }
    }
}

/// Options of [`AxesMut::annotate`](super::AxesMut::annotate).
///
/// # Examples
///
/// ```
/// use umber_core::AnnotateOptions;
///
/// let options = AnnotateOptions::default();
/// assert_eq!(options.offset, (0.0, 12.0));
/// assert!(options.arrow);
/// assert_eq!(options.text_at, None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnnotateOptions {
    /// How far the text is from the point, in points: x to the right, y upwards. Its direction
    /// decides the side of the text (see [`AnnotationPlacement::Offset`]). The default is
    /// `(0, 12)`; any other offset together with `text_at` is an error.
    pub offset: (f64, f64),
    /// Where in the plot the text is written instead of at `offset` from the point, with the
    /// arrow, if any, running to the point (see [`AnnotationPlacement::InPlot`]); `None` places
    /// the text by `offset`.
    pub text_at: Option<InPlot>,
    /// Whether an arrow runs from the text to the point.
    pub arrow: bool,
    /// The text size; [`SizeSpec::Auto`] for the theme's body size.
    pub size: SizeSpec,
    /// The color of the text and the arrow; [`ColorSpec::AUTO`] for the theme's text color.
    pub color: ColorSpec,
}

impl Default for AnnotateOptions {
    fn default() -> Self {
        AnnotateOptions {
            offset: DEFAULT_OFFSET,
            text_at: None,
            arrow: true,
            size: SizeSpec::Auto,
            color: ColorSpec::AUTO,
        }
    }
}

impl AnnotateOptions {
    /// Checks that `text_at` and `offset` do not both place the text: `Error::Usage` under
    /// `context`, parameter `text_at`, for a `text_at` together with an offset other than the
    /// default.
    pub(super) fn check_text_at(&self, context: &'static str) -> Result<()> {
        if self.text_at.is_some() && self.offset != DEFAULT_OFFSET {
            return Err(Error::usage(
                context,
                format!(
                    "text_at and offset ({}, {}) both place the text, in different ways.",
                    number(self.offset.0),
                    number(self.offset.1)
                ),
            )
            .with_parameter("text_at")
            .with_hint("pass text_at to put the text at a position in the plot, or offset to put it near the point, not both."));
        }
        Ok(())
    }
}

/// Where the text of an [`Annotation`] sits relative to its point.
///
/// # Examples
///
/// ```
/// use umber_core::{Align, AnnotationPlacement};
///
/// let at = AnnotationPlacement::At(Align::Center);
/// assert_ne!(at, AnnotationPlacement::Offset { offset: (0.0, 12.0), arrow: true });
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnnotationPlacement {
    /// At the position, a data point or a position in the plot: the aligned edge of the text, or
    /// its centre, is at the position, and so is the vertical centre of the text. Made by
    /// [`AxesMut::text`](super::AxesMut::text).
    At(Align),
    /// Away from the point by `offset` points (x to the right, y upwards), on the side the offset
    /// points to, with an arrow from the text to the point when `arrow` holds. Made by
    /// [`AxesMut::annotate`](super::AxesMut::annotate).
    Offset {
        /// The offset, finite and not zero.
        offset: (f64, f64),
        /// Whether an arrow runs from the text to the point.
        arrow: bool,
    },
    /// At a position in the plot, away from the data point of the annotation: the centre of the
    /// text, and so its vertical centre, is at `at`, with an arrow from the text to the data
    /// point when `arrow` holds. Made by [`AxesMut::annotate`](super::AxesMut::annotate) with
    /// [`AnnotateOptions::text_at`].
    InPlot {
        /// The position of the text in the plot.
        at: InPlot,
        /// Whether an arrow runs from the text to the data point.
        arrow: bool,
    },
}

/// Text at a position of an axes, with its placement, size and color.
///
/// The text is not empty and a data position suits the axes' scales; both are checked when the
/// annotation is added, and never change.
///
/// # Examples
///
/// ```
/// use umber_core::{
///     AnnotationPlacement, AxisValue, Figure, ScaleKind, TextOptions, TextPosition, Theme,
/// };
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.text("Peak", (AxisValue::Number(2.0), AxisValue::Number(7.5)), TextOptions::default())?;
/// let note = &axes.annotations()[0];
/// assert_eq!(note.text(), "Peak");
/// assert_eq!(
///     note.at(),
///     &TextPosition::Data(AxisValue::Number(2.0), AxisValue::Number(7.5))
/// );
/// assert!(matches!(note.placement(), AnnotationPlacement::At(_)));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    text: String,
    at: TextPosition,
    placement: AnnotationPlacement,
    size: SizeSpec,
    color: ColorSpec,
}

impl Annotation {
    /// The annotation of `text` at `at`, placed as `placement` in `size` and `color`, checked
    /// under `context` for axes of the scale kinds `kinds` before it is stored, so a failed call
    /// changes nothing. The checks run in this order, and the first failure is the error:
    ///
    /// 1. the text is not empty (`Error::Usage`, parameter `text`);
    /// 2. each coordinate of a data position `at`, x first, suits the scale kind of its axis: a
    ///    number on a linear or log axis, finite, at most [`MAX_MAGNITUDE`] in magnitude and on a
    ///    log axis at least [`MIN_LOG_VALUE`]; a category name on a category axis; a time on a
    ///    time axis (`Error::Data`, parameter `at`). A position in the plot needs no check;
    /// 3. an offset is finite and not zero (`Error::Style`, parameter `offset`).
    pub(super) fn new(
        context: &'static str,
        (x_kind, y_kind): (ScaleKind, ScaleKind),
        text: &str,
        at: TextPosition,
        placement: AnnotationPlacement,
        size: SizeSpec,
        color: ColorSpec,
    ) -> Result<Annotation> {
        if text.is_empty() {
            return Err(Error::usage(
                context,
                "text is empty; an annotation needs at least one character.",
            )
            .with_parameter("text")
            .with_hint("pass the text to write."));
        }
        Lines::Many
            .check_style(context, "text", text)
            .map_err(|error| error.with_parameter("text"))?;
        if let TextPosition::Data(x, y) = &at {
            check_coordinate(context, Coordinate::X, x_kind, x)?;
            check_coordinate(context, Coordinate::Y, y_kind, y)?;
        }
        if let AnnotationPlacement::Offset { offset, .. } = placement {
            check_offset(context, offset)?;
        }
        Ok(Annotation {
            text: text.to_owned(),
            at,
            placement,
            size,
            color,
        })
    }

    /// The text, never empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{AnnotateOptions, AxisValue, Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let at = (AxisValue::Number(1.0), AxisValue::Number(2.0));
    /// axes.annotate("Dip", at, AnnotateOptions::default())?;
    /// assert_eq!(axes.annotations()[0].text(), "Dip");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The position: for [`AxesMut::text`](super::AxesMut::text) where the text is written, for
    /// [`AxesMut::annotate`](super::AxesMut::annotate) the data point, whose coordinates are each
    /// in the values of its axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{AxisValue, Figure, ScaleKind, TextOptions, TextPosition, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let at = (AxisValue::Category("Mar".into()), AxisValue::Number(42.0));
    /// axes.text("launch", at.clone(), TextOptions::default())?;
    /// assert_eq!(axes.annotations()[0].at(), &TextPosition::Data(at.0, at.1));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn at(&self) -> &TextPosition {
        &self.at
    }

    /// Where the text sits relative to the point.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{AnnotateOptions, AnnotationPlacement, AxisValue, Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let at = (AxisValue::Number(1.0), AxisValue::Number(2.0));
    /// axes.annotate("Dip", at, AnnotateOptions { arrow: false, ..AnnotateOptions::default() })?;
    /// let placement = axes.annotations()[0].placement();
    /// assert_eq!(placement, AnnotationPlacement::Offset { offset: (0.0, 12.0), arrow: false });
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn placement(&self) -> AnnotationPlacement {
        self.placement
    }

    /// The text size as given.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{AxisValue, Figure, ScaleKind, SizeSpec, TextOptions, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let options = TextOptions { size: SizeSpec::pt(12.0)?, ..TextOptions::default() };
    /// axes.text("x", (AxisValue::Number(1.0), AxisValue::Number(2.0)), options)?;
    /// assert_eq!(axes.annotations()[0].size(), SizeSpec::pt(12.0)?);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn size(&self) -> SizeSpec {
        self.size
    }

    /// The color as given, before it is resolved against the theme.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{AxisValue, ColorSpec, Figure, ScaleKind, TextOptions, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.text("x", (AxisValue::Number(1.0), AxisValue::Number(2.0)), TextOptions::default())?;
    /// assert_eq!(axes.annotations()[0].color(), ColorSpec::AUTO);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn color(&self) -> ColorSpec {
        self.color
    }
}

/// One coordinate of a point: which axis it is on, and its place in `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Coordinate {
    /// The x coordinate, `at[0]`.
    X,
    /// The y coordinate, `at[1]`.
    Y,
}

impl Coordinate {
    /// How messages name the coordinate.
    fn label(self) -> &'static str {
        match self {
            Coordinate::X => "at[0]",
            Coordinate::Y => "at[1]",
        }
    }

    /// The axis the coordinate is on.
    fn axis(self) -> &'static str {
        match self {
            Coordinate::X => "x",
            Coordinate::Y => "y",
        }
    }
}

/// Checks that `value`, the coordinate `which` of `at`, suits an axis of the scale kind `kind`
/// (see [`Annotation::new`]): `Error::Data` under `context`, parameter `at`.
fn check_coordinate(
    context: &'static str,
    which: Coordinate,
    kind: ScaleKind,
    value: &AxisValue,
) -> Result<()> {
    let (label, axis) = (which.label(), which.axis());
    let wrong = |holds: &str, needs: &str| {
        Error::data(
            context,
            format!(
                "{label} holds {holds} but the {axis} axis is {needs}; pass a value of its kind."
            ),
        )
        .with_parameter("at")
        .with_hint(KIND_HINT)
    };
    let transform = match (value, kind) {
        (AxisValue::Number(_), ScaleKind::Linear) => Transform::Linear,
        (AxisValue::Number(_), ScaleKind::Log { base }) => Transform::Log(base),
        (AxisValue::Category(_), ScaleKind::Category) | (AxisValue::Time(_), ScaleKind::Time) => {
            return Ok(());
        }
        (AxisValue::Number(_), ScaleKind::Category) => {
            return Err(wrong("a number", "a category axis"));
        }
        (AxisValue::Number(_), ScaleKind::Time) => return Err(wrong("a number", "a time axis")),
        (AxisValue::Category(_), ScaleKind::Time) => {
            return Err(wrong("a category name", "a time axis"));
        }
        (AxisValue::Category(_), ScaleKind::Linear | ScaleKind::Log { .. }) => {
            return Err(wrong("a category name", "a numeric axis"));
        }
        (AxisValue::Time(_), ScaleKind::Category) => {
            return Err(wrong("a time", "a category axis"));
        }
        (AxisValue::Time(_), ScaleKind::Linear | ScaleKind::Log { .. }) => {
            return Err(wrong("a time", "a numeric axis"));
        }
    };
    let AxisValue::Number(v) = *value else {
        return Ok(());
    };
    let refuse = |requirement: String| {
        Error::data(context, format!("{label} is {}; {requirement}.", number(v)))
            .with_parameter("at")
    };
    if !v.is_finite() {
        return Err(
            refuse("a position must be finite".to_owned()).with_hint("pass a finite number.")
        );
    }
    if v.abs() > MAX_MAGNITUDE {
        return Err(
            refuse(format!("magnitudes must not exceed {MAX_MAGNITUDE:e}"))
                .with_hint("rescale the value."),
        );
    }
    if matches!(transform, Transform::Log(_)) && v < MIN_LOG_VALUE {
        return Err(refuse(format!(
            "a position on a log axis must be at least {MIN_LOG_VALUE:e}"
        ))
        .with_hint("use a linear axis, or pass a positive value."));
    }
    Ok(())
}

/// Checks the offset of an annotation: finite, and not zero, so the text has a side
/// (`Error::Style` under `context`, parameter `offset`).
fn check_offset(context: &'static str, (dx, dy): (f64, f64)) -> Result<()> {
    let shown = format!("({}, {})", number(dx), number(dy));
    if !(dx.is_finite() && dy.is_finite()) {
        return Err(Error::style(
            context,
            format!("offset is {shown}; both parts must be finite points."),
        )
        .with_parameter("offset")
        .with_hint("pass finite numbers, such as (0.0, 12.0)."));
    }
    if dx == 0.0 && dy == 0.0 {
        return Err(Error::style(
            context,
            format!("offset is {shown}; annotate() writes its text away from the point."),
        )
        .with_parameter("offset")
        .with_hint("pass a non-zero offset, or use text() to write at the point."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Instant, LogBase};

    const CONTEXT: &str = "Axes.text()";

    fn check(kind: ScaleKind, value: AxisValue) -> Result<()> {
        check_coordinate(CONTEXT, Coordinate::Y, kind, &value)
    }

    #[test]
    fn coordinates_must_suit_their_axis() {
        let log = ScaleKind::Log { base: LogBase::TEN };
        let time = AxisValue::Time(Instant::from_micros(0).unwrap());
        assert!(check(ScaleKind::Linear, AxisValue::Number(-3.0)).is_ok());
        assert!(check(log, AxisValue::Number(0.5)).is_ok());
        assert!(check(ScaleKind::Category, AxisValue::Category("A".into())).is_ok());
        assert!(check(ScaleKind::Time, time.clone()).is_ok());
        for (kind, value, message) in [
            (
                ScaleKind::Linear,
                AxisValue::Number(f64::NAN),
                "at[1] is NaN; a position must be finite.",
            ),
            (
                ScaleKind::Linear,
                AxisValue::Number(2e300),
                "at[1] is 2e300; magnitudes must not exceed 1e300.",
            ),
            (
                log,
                AxisValue::Number(0.0),
                "at[1] is 0; a position on a log axis must be at least 1e-300.",
            ),
            (
                ScaleKind::Category,
                AxisValue::Number(1.0),
                "at[1] holds a number but the y axis is a category axis; pass a value of its kind.",
            ),
            (
                ScaleKind::Linear,
                time,
                "at[1] holds a time but the y axis is a numeric axis; pass a value of its kind.",
            ),
        ] {
            let err = check(kind, value).unwrap_err();
            assert!(matches!(err, Error::Data { .. }));
            assert_eq!(err.to_string(), format!("{CONTEXT}: {message}"));
            assert_eq!(err.parameter(), Some("at"));
            assert!(err.hint().is_some());
        }
    }

    #[test]
    fn offsets_must_be_finite_and_not_zero() {
        assert!(check_offset(CONTEXT, (0.0, -1.0)).is_ok());
        for (offset, message) in [
            (
                (0.0, 0.0),
                "offset is (0, 0); annotate() writes its text away from the point.",
            ),
            (
                (f64::INFINITY, 1.0),
                "offset is (inf, 1); both parts must be finite points.",
            ),
            (
                (1.0, f64::NAN),
                "offset is (1, NaN); both parts must be finite points.",
            ),
        ] {
            let err = check_offset(CONTEXT, offset).unwrap_err();
            assert!(matches!(err, Error::Style { .. }));
            assert_eq!(err.to_string(), format!("{CONTEXT}: {message}"));
            assert_eq!(err.parameter(), Some("offset"));
        }
    }
}
