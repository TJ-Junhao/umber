//! Bubble marks with areas proportional to their non-negative size values.

use std::sync::Arc;

use crate::{ColorSpec, Marker};

/// Options of a bubble mark.
///
/// # Examples
///
/// ```
/// use umber_core::{BubbleOptions, ColorSpec, Marker};
/// let options = BubbleOptions::default();
/// assert_eq!((options.color, options.marker), (ColorSpec::AUTO, Marker::Circle));
/// assert!(options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BubbleOptions {
    /// The legend label; the first labelled bubble entry is followed by the size references.
    pub label: String,
    /// The marker color.
    pub color: ColorSpec,
    /// The marker shape.
    pub marker: Marker,
}

impl Default for BubbleOptions {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: ColorSpec::AUTO,
            marker: Marker::Circle,
        }
    }
}

/// Validated bubble coordinates, size values and options, shared by clones.
///
/// Sizes are finite values from 0 to 1e300. A zero size or NaN coordinate draws nothing.
/// A zero-size point's position still counts in the axis domains like any point. All bubble
/// marks on an axes share one size scale; the largest size among points with finite x and y
/// coordinates maps to the theme's maximum radius. Automatic axis limits fit the whole
/// circles and their outlines using the final plot size, across all panels sharing that axis;
/// explicit limits still clip. A diameter at least the automatic axis's plot length is a
/// render-time `Error::Style`, as is a fit outside the supported numeric range.
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
    x: Arc<[f64]>,
    y: Arc<[f64]>,
    sizes: Arc<[f64]>,
    options: BubbleOptions,
}

impl BubbleMark {
    /// Stores validated arrays without copying them.
    pub(super) fn new(
        x: Arc<[f64]>,
        y: Arc<[f64]>,
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

    /// The numeric x coordinates, in data order.
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
        &self.x
    }

    /// The y coordinates, one per x coordinate.
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
        &self.y
    }

    /// The non-negative size values, one per coordinate pair.
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
