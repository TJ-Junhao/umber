//! Marks and their options.

use std::fmt;

use super::axis_data::{AxisData, Series};
use super::bar::BarMark;
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
/// Marks are created by [`Axes::line`](super::Axes::line),
/// [`Axes::scatter`](super::Axes::scatter) and [`Axes::bar`](super::Axes::bar), which validate
/// the data. Each variant holds a payload ([`LineMark`], [`ScatterMark`], [`BarMark`]) that
/// keeps the validated values together and exposes them read-only; it has no public
/// constructor. Read a mark by matching on its variant.
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
///     [Mark::Line(line)] => {
///         assert_eq!(line.x(), &AxisData::Numbers(vec![0.0, 1.0]));
///         assert_eq!(line.y(), &[2.0, 3.0]);
///         assert!(line.options().label.is_empty());
///     }
///     other => panic!("unexpected marks {other:?}"),
/// }
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A mark cannot be built outside [`Axes`](super::Axes), so its data is always validated:
///
/// ```compile_fail,E0451
/// use umber_core::figure::{LineMark, LineOptions, Mark};
///
/// let mark = Mark::Line(LineMark {
///     series: unreachable!(),
///     options: LineOptions::default(),
/// });
/// ```
///
/// Nor can it be assembled from loose x and y values:
///
/// ```compile_fail,E0559
/// use umber_core::figure::{AxisData, LineOptions, Mark};
///
/// let mark = Mark::Line {
///     x: AxisData::Numbers(vec![0.0]),
///     y: vec![1.0, 2.0],
///     options: LineOptions::default(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive] // further mark kinds, such as grouped bars and histograms, are added as variants
pub enum Mark {
    /// A polyline through the points; NaN breaks it.
    Line(LineMark),
    /// Individual markers; NaN skips a point.
    Scatter(ScatterMark),
    /// One bar per category, from the baseline to its height; NaN draws no bar.
    Bar(BarMark),
}

impl Mark {
    /// A line through a validated series.
    pub(super) fn line(series: Series, options: LineOptions) -> Mark {
        Mark::Line(LineMark { series, options })
    }

    /// Markers at the points of a validated series.
    pub(super) fn scatter(series: Series, options: ScatterOptions) -> Mark {
        Mark::Scatter(ScatterMark { series, options })
    }

    /// The color as given, before it is resolved against a palette.
    pub(crate) fn color(&self) -> ColorSpec {
        match self {
            Mark::Line(line) => line.options.color,
            Mark::Scatter(scatter) => scatter.options.color,
            Mark::Bar(bar) => bar.options().color,
        }
    }
}

/// A line mark: a validated series and its [`LineOptions`].
///
/// The x and y values are stored together and never change; clones share them.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, LineOptions, Mark};
/// use umber_core::{Dash, Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// axes.line(AxisData::Categories(vec!["a".into()]), &[1.0], LineOptions::default())?;
/// let Some(Mark::Line(line)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(line.x(), &AxisData::Categories(vec!["a".into()]));
/// assert_eq!(line.y(), &[1.0]);
/// assert_eq!(line.options().dash, Dash::Solid);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// The x values of one line cannot be paired with the y values of another:
///
/// ```compile_fail,E0616
/// use umber_core::figure::{AxisData, LineOptions, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.line(AxisData::Numbers(vec![0.0]), &[1.0], LineOptions::default())?
///     .line(AxisData::Numbers(vec![0.0, 1.0]), &[1.0, 2.0], LineOptions::default())?;
/// let [Mark::Line(short), Mark::Line(long)] = axes.marks() else { unreachable!() };
/// let mut mixed = short.clone();
/// mixed.series = long.series.clone();
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Clone, PartialEq)]
pub struct LineMark {
    series: Series,
    options: LineOptions,
}

impl LineMark {
    /// The x values.
    #[must_use]
    pub fn x(&self) -> &AxisData {
        self.series.x()
    }

    /// The y values, as many as the x values.
    #[must_use]
    pub fn y(&self) -> &[f64] {
        self.series.y()
    }

    /// Label, color and dash.
    #[must_use]
    pub fn options(&self) -> &LineOptions {
        &self.options
    }

    /// The validated series.
    #[cfg(test)]
    pub(super) fn series(&self) -> &Series {
        &self.series
    }
}

impl fmt::Debug for LineMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LineMark")
            .field("x", self.x())
            .field("y", &self.y())
            .field("options", &self.options)
            .finish()
    }
}

/// A scatter mark: a validated series and its [`ScatterOptions`].
///
/// The x and y values are stored together and never change; clones share them.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, Mark, ScatterOptions};
/// use umber_core::{Figure, Marker, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.scatter(AxisData::Numbers(vec![0.0, 1.0]), &[2.0, 3.0], ScatterOptions::default())?;
/// let Some(Mark::Scatter(scatter)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(scatter.x(), &AxisData::Numbers(vec![0.0, 1.0]));
/// assert_eq!(scatter.y(), &[2.0, 3.0]);
/// assert_eq!(scatter.options().marker, Marker::Circle);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A scatter mark cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{Mark, ScatterMark, ScatterOptions};
///
/// let mark = Mark::Scatter(ScatterMark {
///     series: unreachable!(),
///     options: ScatterOptions::default(),
/// });
/// ```
///
/// The x values of one scatter mark cannot be paired with the y values of another:
///
/// ```compile_fail,E0616
/// use umber_core::figure::{AxisData, Mark, ScatterOptions};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.scatter(AxisData::Numbers(vec![0.0]), &[1.0], ScatterOptions::default())?
///     .scatter(AxisData::Numbers(vec![0.0, 1.0]), &[1.0, 2.0], ScatterOptions::default())?;
/// let [Mark::Scatter(short), Mark::Scatter(long)] = axes.marks() else { unreachable!() };
/// let mut mixed = short.clone();
/// mixed.series = long.series.clone();
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Clone, PartialEq)]
pub struct ScatterMark {
    series: Series,
    options: ScatterOptions,
}

impl ScatterMark {
    /// The x values.
    #[must_use]
    pub fn x(&self) -> &AxisData {
        self.series.x()
    }

    /// The y values, as many as the x values.
    #[must_use]
    pub fn y(&self) -> &[f64] {
        self.series.y()
    }

    /// Label, color and marker shape.
    #[must_use]
    pub fn options(&self) -> &ScatterOptions {
        &self.options
    }

    /// The validated series.
    #[cfg(test)]
    pub(super) fn series(&self) -> &Series {
        &self.series
    }
}

impl fmt::Debug for ScatterMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScatterMark")
            .field("x", self.x())
            .field("y", &self.y())
            .field("options", &self.options)
            .finish()
    }
}
