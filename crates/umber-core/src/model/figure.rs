//! The figure: a size, a theme and at most one axes.

use super::axes::Axes;
use super::validation::check_figure_size;
use crate::{Error, Result, ScaleKind, Theme};

/// A figure: a size, a theme and at most one [`Axes`].
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Theme};
///
/// let figure = Figure::new(6.0, 4.0, Theme::graphite())?;
/// assert_eq!((figure.width_in(), figure.height_in()), (6.0, 4.0));
/// assert!(figure.axes().is_none());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Figure {
    width_in: f64,
    height_in: f64,
    theme: Theme,
    axes: Option<Axes>,
}

impl Figure {
    /// Creates an empty figure. Sides are in inches; `Error::Style` unless each is finite,
    /// above 0 and at most 200.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// assert!(Figure::new(6.0, 4.0, Theme::pigment()).is_ok());
    /// assert!(Figure::new(0.0, 4.0, Theme::pigment()).is_err());
    /// ```
    pub fn new(width_in: f64, height_in: f64, theme: Theme) -> Result<Figure> {
        check_figure_size(width_in, height_in)?;
        Ok(Figure {
            width_in,
            height_in,
            theme,
            axes: None,
        })
    }

    /// Adds the figure's axes. `Error::Usage` if it already has some (v0.1 allows one).
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert!(figure.add_axes(ScaleKind::Linear, ScaleKind::Linear).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn add_axes(&mut self, x: ScaleKind, y: ScaleKind) -> Result<&mut Axes> {
        if self.axes.is_some() {
            return Err(Error::usage(
                "Figure.add_axes()",
                "the figure already has axes; v0.1 supports one axes per figure.",
            )
            .with_hint("create a new Figure for another chart."));
        }
        Ok(self.axes.insert(Axes::new(x, y)))
    }

    /// The axes, if added.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// assert!(Figure::new(6.0, 4.0, Theme::pigment())?.axes().is_none());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn axes(&self) -> Option<&Axes> {
        self.axes.as_ref()
    }

    /// The axes, mutably, if added.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// figure.axes_mut().map(|a| a.title("again"));
    /// assert_eq!(figure.axes().map(|a| a.title_text()), Some("again"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn axes_mut(&mut self) -> Option<&mut Axes> {
        self.axes.as_mut()
    }

    /// The theme.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// let figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// assert_eq!(figure.theme(), &Theme::midnight());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// The width in inches.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// assert_eq!(Figure::new(6.0, 4.0, Theme::pigment())?.width_in(), 6.0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn width_in(&self) -> f64 {
        self.width_in
    }

    /// The height in inches.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// assert_eq!(Figure::new(6.0, 4.0, Theme::pigment())?.height_in(), 4.0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn height_in(&self) -> f64 {
        self.height_in
    }
}
