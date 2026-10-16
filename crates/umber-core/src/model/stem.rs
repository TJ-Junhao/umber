//! Stem marks: a baseline-to-value line and a scatter marker at each point.

use std::fmt;

use super::axis_data::{AxisData, XySeries};
use crate::{ColorSpec, Marker, SizeSpec};

/// Options of a stem mark.
///
/// # Examples
///
/// ```
/// use umber_core::{ColorSpec, Marker, SizeSpec, StemOptions};
///
/// let options = StemOptions::default();
/// assert!(options.label.is_empty());
/// assert_eq!((options.color, options.marker), (ColorSpec::AUTO, Marker::Circle));
/// assert_eq!(options.size, SizeSpec::Auto);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StemOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The color of the stems and their markers.
    pub color: ColorSpec,
    /// The marker shape.
    pub marker: Marker,
    /// The marker diameter; [`SizeSpec::Auto`] for the theme's marker size.
    pub size: SizeSpec,
}

impl Default for StemOptions {
    fn default() -> Self {
        StemOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
            marker: Marker::Circle,
            size: SizeSpec::Auto,
        }
    }
}

/// A stem mark: a validated series and its [`StemOptions`].
///
/// Each point has a line from the baseline to its value and a marker at the value. NaN skips
/// the point. The x and y values stay together and never change; clones share them.
///
/// # Examples
///
/// ```
/// use umber_core::{AxisData, Figure, Mark, ScaleKind, StemOptions, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.stem(AxisData::Numbers(vec![0.0, 1.0]), &[2.0, -1.0], StemOptions::default())?;
/// let Some(Mark::Stem(stem)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(stem.x(), &AxisData::Numbers(vec![0.0, 1.0]));
/// assert_eq!(stem.y(), &AxisData::Numbers(vec![2.0, -1.0]));
/// assert_eq!(stem.options(), &StemOptions::default());
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A stem mark cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::{StemMark, StemOptions};
///
/// let mark = StemMark { series: unreachable!(), options: StemOptions::default() };
/// ```
#[derive(Clone, PartialEq)]
pub struct StemMark {
    series: XySeries,
    options: StemOptions,
}

impl StemMark {
    /// A stem mark over a validated series.
    pub(super) fn new(series: XySeries, options: StemOptions) -> StemMark {
        StemMark { series, options }
    }

    /// The x values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{AxisData, Figure, Mark, ScaleKind, StemOptions, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.stem(AxisData::Numbers(vec![1.0]), &[2.0], StemOptions::default())?;
    /// let Some(Mark::Stem(stem)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(stem.x(), &AxisData::Numbers(vec![1.0]));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x(&self) -> &AxisData {
        self.series.x()
    }

    /// The y values, as many as the x values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{AxisData, Figure, Mark, ScaleKind, StemOptions, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.stem(AxisData::Numbers(vec![1.0]), &[2.0], StemOptions::default())?;
    /// let Some(Mark::Stem(stem)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(stem.y(), &AxisData::Numbers(vec![2.0]));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y(&self) -> &AxisData {
        self.series.y()
    }

    /// Label, color, marker shape and size.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{AxisData, Figure, Mark, Marker, ScaleKind, StemOptions, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.stem(AxisData::Numbers(vec![1.0]), &[2.0], StemOptions::default())?;
    /// let Some(Mark::Stem(stem)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(stem.options().marker, Marker::Circle);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &StemOptions {
        &self.options
    }
}

impl fmt::Debug for StemMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StemMark")
            .field("x", self.x())
            .field("y", &self.y())
            .field("options", &self.options)
            .finish()
    }
}
