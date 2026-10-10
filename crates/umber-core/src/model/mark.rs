//! Marks and their options.

use super::axis_data::{Series, XValues, YValues};
use crate::{ColorSpec, Dash, Marker};

/// Options of a line mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::LineOptions;
/// use umber_core::{ColorSpec, Dash};
///
/// let options = LineOptions::default();
/// assert_eq!((options.color, options.dash), (ColorSpec::AUTO, Dash::Solid));
/// assert!(options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LineOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The line color.
    pub color: ColorSpec,
    /// The dash style.
    pub dash: Dash,
}

impl Default for LineOptions {
    fn default() -> Self {
        LineOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
            dash: Dash::Solid,
        }
    }
}

/// Options of a scatter mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::ScatterOptions;
/// use umber_core::{ColorSpec, Marker};
///
/// let options = ScatterOptions::default();
/// assert_eq!((options.color, options.marker), (ColorSpec::AUTO, Marker::Circle));
/// assert!(options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ScatterOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The marker color.
    pub color: ColorSpec,
    /// The marker shape.
    pub marker: Marker,
}

impl Default for ScatterOptions {
    fn default() -> Self {
        ScatterOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
            marker: Marker::Circle,
        }
    }
}

/// One series drawn in an [`Axes`](super::Axes).
///
/// Marks are created by [`Axes::line`](super::Axes::line) and
/// [`Axes::scatter`](super::Axes::scatter), which validate the data; the stored values
/// ([`XValues`], [`YValues`]) cannot be built elsewhere. Read a mark by matching on its variant.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, LineOptions, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.line(AxisData::Numbers(vec![0.0, 1.0]), &[2.0, 3.0], LineOptions::default())?;
/// match axes.marks() {
///     [Mark::Line { x, y, options }] => {
///         assert_eq!(**x, AxisData::Numbers(vec![0.0, 1.0]));
///         assert_eq!(&y[..], &[2.0, 3.0]);
///         assert!(options.label.is_empty());
///     }
///     other => panic!("unexpected marks {other:?}"),
/// }
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive] // M5b adds the bar and histogram marks
pub enum Mark {
    /// A polyline through the points; NaN breaks it.
    Line {
        /// The x values.
        x: XValues,
        /// The y values, as many as `x`.
        y: YValues,
        /// Label, color and dash.
        options: LineOptions,
    },
    /// Individual markers; NaN skips a point.
    Scatter {
        /// The x values.
        x: XValues,
        /// The y values, as many as `x`.
        y: YValues,
        /// Label, color and marker shape.
        options: ScatterOptions,
    },
}

impl Mark {
    /// A line through a validated series.
    pub(super) fn line(series: Series, options: LineOptions) -> Mark {
        let Series { x, y } = series;
        Mark::Line { x, y, options }
    }

    /// Markers at the points of a validated series.
    pub(super) fn scatter(series: Series, options: ScatterOptions) -> Mark {
        let Series { x, y } = series;
        Mark::Scatter { x, y, options }
    }

    /// The color as given, before it is resolved against a palette.
    pub(crate) fn color(&self) -> ColorSpec {
        match self {
            Mark::Line { options, .. } => options.color,
            Mark::Scatter { options, .. } => options.color,
        }
    }
}
