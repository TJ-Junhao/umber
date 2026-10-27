//! Bubble marks with areas proportional to their non-negative size values.

use std::sync::Arc;

use super::axis_data::SharedValues;
use super::color_by::PointColor;
use crate::{Instant, Marker};

/// Options of a bubble mark.
///
/// # Examples
///
/// ```
/// use umber_core::{BubbleOptions, Marker, PointColor};
/// let options = BubbleOptions::default();
/// assert_eq!((&options.color, options.marker), (&PointColor::AUTO, Marker::Circle));
/// assert!(options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BubbleOptions {
    /// The legend label; the first labelled bubble entry is followed by the size references.
    pub label: String,
    /// The marker color: one color, or a color per point mapped from values. The legend entry
    /// and size references of a color-mapped mark are drawn in the theme's text color.
    pub color: PointColor,
    /// The marker shape.
    pub marker: Marker,
}

impl Default for BubbleOptions {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: PointColor::AUTO,
            marker: Marker::Circle,
        }
    }
}

/// Validated bubble coordinates, size values and options, shared by clones.
///
/// Sizes are finite values from 0 to 1e300, or NaN. A zero size, a NaN size, a NaN coordinate
/// or a missing time draws nothing.
/// A zero-size point's position still counts in the axis domains like any point; a point with
/// a NaN size counts in neither axis domain. All bubble marks on an axes share one size
/// scale; the largest size among points with finite x, y and size maps to the theme's maximum
/// radius. Automatic axis limits fit the whole
/// markers and their outlines using the final plot size, across all panels sharing that axis;
/// explicit limits clip the bubbles that reach past them. Bubbles that no automatic limits
/// hold (a marker at least as long as the plot along the axis, or limits beyond the supported
/// numeric range) fail the render; only limits that do not settle fall back to the data
/// positions alone, and the plot clips the bubbles.
///
/// # Examples
///
/// ```
/// use umber_core::{BubbleOptions, Figure, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.bubbles(&[1.0], &[2.0], &[100.0], BubbleOptions::default())?;
/// let Some(Mark::Bubbles(mark)) = axes.marks().first() else { unreachable!() };
/// assert_eq!((mark.x(), mark.y(), mark.sizes()), (&[1.0][..], &[2.0][..], &[100.0][..]));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BubbleMark {
    x: SharedValues,
    y: SharedValues,
    sizes: Arc<[f64]>,
    options: BubbleOptions,
}

impl BubbleMark {
    /// Stores validated arrays without copying them.
    pub(super) fn new(
        x: SharedValues,
        y: SharedValues,
        sizes: Arc<[f64]>,
        options: BubbleOptions,
    ) -> Self {
        Self {
            x,
            y,
            sizes,
            options,
        }
    }

    /// The numeric x coordinates, in data order; none on a time x axis.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{BubbleOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.bubbles(&[1.0], &[2.0], &[100.0], BubbleOptions::default())?;
    /// let Some(Mark::Bubbles(mark)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(mark.x(), &[1.0]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x(&self) -> &[f64] {
        self.x.numbers()
    }

    /// The x times, in data order, `None` for a missing time; none unless the x axis is a time
    /// axis.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use umber_core::{AxisData, BubbleOptions, Figure, Instant, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut axes = f.add_axes(ScaleKind::Time, ScaleKind::Linear)?;
    /// let x = [Some(Instant::from_micros(0)?), None];
    /// let y = AxisData::Numbers(vec![1.0, 2.0]);
    /// axes.bubbles_time(AxisData::Times(x.to_vec()), y, Arc::from([1.0, 2.0]), BubbleOptions::default())?;
    /// let Some(Mark::Bubbles(mark)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(mark.x_times(), &x);
    /// assert!(mark.x().is_empty());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_times(&self) -> &[Option<Instant>] {
        self.x.times()
    }

    /// The x coordinates as stored: numbers or times.
    pub(crate) fn x_values(&self) -> &SharedValues {
        &self.x
    }

    /// The numeric y coordinates, one per x coordinate; none on a time y axis.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{BubbleOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.bubbles(&[1.0], &[2.0], &[100.0], BubbleOptions::default())?;
    /// let Some(Mark::Bubbles(mark)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(mark.y(), &[2.0]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y(&self) -> &[f64] {
        self.y.numbers()
    }

    /// The y times, one per x coordinate, `None` for a missing time; none unless the y axis is
    /// a time axis.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use umber_core::{AxisData, BubbleOptions, Figure, Instant, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Time)?;
    /// let y = [Some(Instant::from_micros(0)?)];
    /// let x = AxisData::Numbers(vec![1.0]);
    /// axes.bubbles_time(x, AxisData::Times(y.to_vec()), Arc::from([1.0]), BubbleOptions::default())?;
    /// let Some(Mark::Bubbles(mark)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(mark.y_times(), &y);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y_times(&self) -> &[Option<Instant>] {
        self.y.times()
    }

    /// The y coordinates as stored: numbers or times.
    pub(crate) fn y_values(&self) -> &SharedValues {
        &self.y
    }

    /// The size values, one per coordinate pair: non-negative, or NaN for a bubble left out.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{BubbleOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.bubbles(&[1.0], &[2.0], &[100.0], BubbleOptions::default())?;
    /// let Some(Mark::Bubbles(mark)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(mark.sizes(), &[100.0]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn sizes(&self) -> &[f64] {
        &self.sizes
    }

    /// Whether any size is NaN, which leaves its bubble out.
    pub(crate) fn has_missing_size(&self) -> bool {
        self.sizes.iter().any(|size| size.is_nan())
    }

    /// The members of `values` whose bubble has a size, in data order: the points that count in
    /// the axis domains.
    pub(crate) fn sized<'a, T>(&'a self, values: &'a [T]) -> impl Iterator<Item = &'a T> + 'a {
        values
            .iter()
            .zip(self.sizes.iter())
            .filter(|(_, size)| !size.is_nan())
            .map(|(value, _)| value)
    }

    /// The label, color and marker shape.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{BubbleOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.bubbles(&[1.0], &[2.0], &[100.0], BubbleOptions::default())?;
    /// let Some(Mark::Bubbles(mark)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(mark.options(), &BubbleOptions::default());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &BubbleOptions {
        &self.options
    }
}
