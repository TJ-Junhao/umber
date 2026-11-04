//! The values written next to scatter points: whether a scatter writes them, in which format
//! and size, and where they go ([`Place`]); and the title of the column that
//! [`Place::Column`] writes them in ([`ValueColumn`]).

use crate::{Error, NumberStyle, Result, ScaleKind, SizeSpec, TickFormat};

/// Where the value label of a scatter point goes.
///
/// A fixed side puts every label of the mark on that side, 3 pt beyond the marker's drawn
/// edge, outline included: [`Place::Above`] and [`Place::Below`] centre it horizontally on the
/// point, [`Place::Left`] and [`Place::Right`] centre it vertically. A fixed-side label never
/// moves, and automatic limits grow so that it fits inside the plot.
///
/// [`Place::Auto`] tries above, right, below and left, in that order, and takes the first
/// position that lies inside the plot and is clear of every mark and label already placed.
/// When none is, it takes the first side, in the same order, that is clear and leaves the plot
/// only across automatic limits, and those limits grow just enough to hold the label; the
/// labels are then placed again against the grown limits. With explicit limits, or on a
/// category axis, the plot edge stays where it is.
///
/// [`Place::Column`] writes the values outside the plot, at the far end of the value axis: in a
/// column right of the plot when x is the value axis, each value level with its point and
/// centred on it vertically; in a row above the plot when y is, each value centred on its
/// point's x. Each scatter that writes its values there, and each group of a grouped scatter,
/// has a sub-column of its own, in mark order: left to right in a column, top to bottom in a
/// row. The layout reserves the column's room as it does a tick label column, so the plot
/// narrows, or gets shorter, to make room. Values in a column never change limits and are no
/// obstacles of labels inside the plot. [`ValueColumn`] sets the column's title.
///
/// # Examples
///
/// ```
/// use umber_core::{Place, PointLabels};
///
/// assert_eq!(Place::default(), Place::Auto);
/// let below = PointLabels { annotate: true, place: Place::Below, ..PointLabels::default() };
/// assert_eq!(below.place, Place::Below);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    /// The first clear side of above, right, below and left.
    #[default]
    Auto,
    /// Above the point, centred horizontally.
    Above,
    /// Below the point, centred horizontally.
    Below,
    /// Left of the point, centred vertically.
    Left,
    /// Right of the point, centred vertically.
    Right,
    /// In a column outside the plot, at the far end of the value axis.
    Column,
}

/// The title of the value column of an axes and its size, set by
/// [`AxesMut::value_column`](crate::AxesMut::value_column): the column of the values that
/// scatters with [`Place::Column`] write outside the plot.
///
/// The title is set in the axis label style, at `size`; [`SizeSpec::Auto`] is the axis label
/// size. It is centred over a column right of the plot, its baseline level with the y axis
/// label's, and written before a row above the plot, right-aligned at the left edge of the
/// plot, in the y tick label column, at the row's vertical centre. An empty title writes
/// nothing.
///
/// # Examples
///
/// ```
/// use umber_core::{SizeSpec, ValueColumn};
///
/// let column = ValueColumn::new("Mean (%)", SizeSpec::Auto);
/// assert_eq!((column.title(), column.size()), ("Mean (%)", SizeSpec::Auto));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ValueColumn {
    /// The title; empty for none.
    title: String,
    /// The title's size.
    size: SizeSpec,
}

impl ValueColumn {
    /// A column titled `title` at `size`.
    #[must_use]
    pub fn new(title: &str, size: SizeSpec) -> ValueColumn {
        ValueColumn {
            title: title.to_owned(),
            size,
        }
    }

    /// The title; empty for none.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The title's size.
    #[must_use]
    pub fn size(&self) -> SizeSpec {
        self.size
    }
}

/// The values written next to the points of a scatter mark.
///
/// The value of a point is its coordinate on the numeric axis: x on a category y axis, and y
/// otherwise (a scatter never has two category axes). On a time axis it is the point's time.
/// A point that is not drawn gets no label: a NaN or missing coordinate, a color-mapped value
/// that maps to no color, or a centre outside explicit limits.
///
/// Labels take the resolved color of their mark at full opacity, whatever the marker alpha;
/// a color-mapped mark's labels take the theme's text color.
///
/// # Examples
///
/// ```
/// use umber_core::{Place, PointLabels, SizeSpec, TickFormat};
///
/// let labels = PointLabels::default();
/// assert!(!labels.annotate);
/// assert_eq!((labels.format, labels.size, labels.place), (None, SizeSpec::Auto, Place::Auto));
/// let shown = PointLabels { annotate: true, format: Some(TickFormat::fixed(1)?), ..labels };
/// assert!(shown.annotate);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PointLabels {
    /// Whether each point shows its value.
    pub annotate: bool,
    /// The format of the values; `None` for the tick format of the value axis. On a numeric
    /// axis the automatic format, [`TickFormat::AUTO`], writes each value on its own to six
    /// significant digits in the shortest form, as bar labels do (`42`, `28.4`, `0.12345`).
    /// On a time axis it writes each time as the axis writes a lone explicit tick, down to the
    /// finest unit the mark's labelled times need (`2026`, `Mar 2026`, `Mar 3`,
    /// `Mar 3 06:30`), and [`TickFormat::iso`] writes it in ISO 8601 down to that unit.
    pub format: Option<TickFormat>,
    /// The text size; [`SizeSpec::Auto`] for the theme's tick label size.
    pub size: SizeSpec,
    /// Where each label goes.
    pub place: Place,
}

impl PointLabels {
    /// Checks the labels under `context`, for a scatter on axes of `x` and `y`, which are not
    /// both category axes:
    ///
    /// - a format, a size or a place without `annotate` would be dropped (`Error::Usage`,
    ///   parameter `format`, `label_size` or `place`);
    /// - the format must suit the value axis (`Error::Style`, parameter `format`): numbers take
    ///   no [`TickFormat::iso`], and times take only [`TickFormat::AUTO`] or
    ///   [`TickFormat::iso`], with any prefix and suffix.
    pub(super) fn check(&self, context: &'static str, x: ScaleKind, y: ScaleKind) -> Result<()> {
        if !self.annotate {
            let given = if self.format.is_some() {
                Some("format")
            } else if self.size != SizeSpec::Auto {
                Some("label_size")
            } else if self.place != Place::Auto {
                Some("place")
            } else {
                None
            };
            if let Some(parameter) = given {
                return Err(Error::usage(
                    context,
                    format!(
                        "{parameter} is given but annotate is false; only annotated points show \
                         their values."
                    ),
                )
                .with_parameter(parameter)
                .with_hint("pass annotate=True, or leave the label options out."));
            }
            return Ok(());
        }
        let Some(format) = &self.format else {
            return Ok(());
        };
        let (axis, kind) = value_axis(x, y);
        let style = format.style();
        if kind == ScaleKind::Time {
            if !matches!(style, NumberStyle::Auto | NumberStyle::Iso) {
                return Err(Error::style(
                    context,
                    format!(
                        "format is a number format; the values on the time {axis} axis are times."
                    ),
                )
                .with_parameter("format")
                .with_hint("use TickFormat.AUTO or TickFormat.iso() on a time axis."));
            }
        } else if style == NumberStyle::Iso {
            return Err(Error::style(
                context,
                format!(
                    "format is TickFormat.iso(), which writes times; the values on the {axis} \
                     axis are numbers."
                ),
            )
            .with_parameter("format")
            .with_hint("use TickFormat.AUTO or a number format such as TickFormat.fixed(1)."));
        }
        Ok(())
    }

    /// Checks under `context` that the labels may go where they are placed on axes that are
    /// a twin (`twin`) or not: a column belongs to the plot's primary axes, so a twin y axis
    /// refuses [`Place::Column`] (`Error::Usage`, parameter `place`).
    pub(super) fn check_twin(&self, context: &'static str, twin: bool) -> Result<()> {
        if twin && self.annotate && self.place == Place::Column {
            return Err(Error::usage(
                context,
                "place is Place.COLUMN on twin axes; the value column belongs to the primary \
                 axes of the plot."
                    .to_owned(),
            )
            .with_parameter("place")
            .with_hint("draw this scatter on the primary axes, or use another place."));
        }
        Ok(())
    }
}

/// Whether the values of a scatter on a y axis of kind `y` lie on its x axis: exactly when
/// the y axis is a category axis.
pub(crate) fn values_on_x(y: ScaleKind) -> bool {
    y == ScaleKind::Category
}

/// The value axis of a scatter on axes of `x` and `y`, by name and kind (see
/// [`values_on_x`]).
fn value_axis(x: ScaleKind, y: ScaleKind) -> (&'static str, ScaleKind) {
    if values_on_x(y) { ("x", x) } else { ("y", y) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn annotated(format: TickFormat) -> PointLabels {
        PointLabels {
            annotate: true,
            format: Some(format),
            ..PointLabels::default()
        }
    }

    #[test]
    fn options_without_annotate_are_refused_by_name() {
        let cases = [
            (
                PointLabels {
                    format: Some(TickFormat::AUTO),
                    ..PointLabels::default()
                },
                "format",
            ),
            (
                PointLabels {
                    size: SizeSpec::pt(9.0).unwrap(),
                    ..PointLabels::default()
                },
                "label_size",
            ),
            (
                PointLabels {
                    place: Place::Left,
                    ..PointLabels::default()
                },
                "place",
            ),
        ];
        for (labels, parameter) in cases {
            let err = labels
                .check("Axes.scatter()", ScaleKind::Linear, ScaleKind::Linear)
                .unwrap_err();
            assert_eq!(err.parameter(), Some(parameter));
            assert_eq!(
                err.to_string(),
                format!(
                    "Axes.scatter(): {parameter} is given but annotate is false; only annotated \
                     points show their values."
                )
            );
        }
        assert!(
            PointLabels::default()
                .check("Axes.scatter()", ScaleKind::Linear, ScaleKind::Linear)
                .is_ok()
        );
    }

    #[test]
    fn formats_must_suit_the_value_axis() {
        let (lin, cat, time) = (ScaleKind::Linear, ScaleKind::Category, ScaleKind::Time);
        let iso = annotated(TickFormat::iso());
        let err = iso.check("Axes.scatter()", lin, lin).unwrap_err();
        assert!(err.to_string().contains("values on the y axis are numbers"));
        assert!(iso.check("Axes.scatter()", time, cat).is_ok());
        assert!(iso.check("Axes.scatter()", cat, time).is_ok());
        let fixed = annotated(TickFormat::fixed(1).unwrap());
        let err = fixed.check("Axes.scatter()", time, cat).unwrap_err();
        assert!(err.to_string().contains("time x axis are times"));
        assert_eq!(err.parameter(), Some("format"));
        assert!(fixed.check("Axes.scatter()", time, lin).is_ok());
        let affixed = annotated(TickFormat::AUTO.affix("", " UTC").unwrap());
        assert!(affixed.check("Axes.scatter()", lin, time).is_ok());
    }

    #[test]
    fn the_value_axis_is_x_only_on_a_category_y_axis() {
        let (lin, cat, time) = (ScaleKind::Linear, ScaleKind::Category, ScaleKind::Time);
        assert_eq!(value_axis(lin, cat), ("x", lin));
        assert_eq!(value_axis(cat, time), ("y", time));
        assert_eq!(value_axis(lin, lin), ("y", lin));
    }
}
