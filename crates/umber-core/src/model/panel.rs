//! The distinct kinds of panel in a figure's row-major traversal.

use super::{Axes, CellRange, PiePanel};
use crate::SizeSpec;

/// An immutable view of a Cartesian or circular panel.
///
/// Each variant exposes only its own panel's state. Iterate with [`super::Figure::panels`].
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Panel, Theme};
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// figure.add_pie_axes()?.title("Parts");
/// for panel in figure.panels() {
///     match panel {
///         Panel::Cartesian(axes, _) => assert!(axes.marks().is_empty()),
///         Panel::Pie(pie) => assert_eq!(pie.title_text(), "Parts"),
///     }
/// }
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel<'a> {
    /// Cartesian axes and their optional right-hand numeric y axes, in the same grid cells.
    Cartesian(&'a Axes, Option<&'a Axes>),
    /// A pie or donut with no Cartesian scales or marks.
    Pie(&'a PiePanel),
}

impl<'a> Panel<'a> {
    /// The grid cells occupied by this panel.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// f.add_pie_axes()?;
    /// assert_eq!(f.panels().next().unwrap().cells().row(), 0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn cells(self) -> CellRange {
        match self {
            Self::Cartesian(axes, _) => axes.cells(),
            Self::Pie(pie) => pie.cells(),
        }
    }

    /// The panel's title, or an empty string when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// f.add_pie_axes()?.title("Parts");
    /// assert_eq!(f.panels().next().unwrap().title_text(), "Parts");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn title_text(self) -> &'a str {
        match self {
            Self::Cartesian(axes, _) => axes.title_text(),
            Self::Pie(pie) => pie.title_text(),
        }
    }

    pub(crate) fn cartesian(self) -> Option<&'a Axes> {
        match self {
            Self::Cartesian(axes, _) => Some(axes),
            Self::Pie(_) => None,
        }
    }

    pub(crate) fn title_size(self) -> SizeSpec {
        match self {
            Self::Cartesian(axes, _) => axes.title_size(),
            Self::Pie(pie) => pie.title_size,
        }
    }

    pub(crate) fn legend_size(self) -> SizeSpec {
        match self {
            Self::Cartesian(axes, _) => axes.legend_size(),
            Self::Pie(pie) => pie.legend_size,
        }
    }

    pub(crate) fn subtitle_text(self) -> &'a str {
        match self {
            Self::Cartesian(axes, _) => axes.subtitle_text(),
            Self::Pie(_) => "",
        }
    }

    pub(crate) fn subtitle_size(self) -> SizeSpec {
        match self {
            Self::Cartesian(axes, _) => axes.subtitle_size(),
            Self::Pie(_) => SizeSpec::Auto,
        }
    }
}

impl<'a> From<&'a Axes> for Panel<'a> {
    fn from(axes: &'a Axes) -> Self {
        Self::Cartesian(axes, None)
    }
}

/// Stable, kind-specific storage indices, ordered by top-left cell in `Figure`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum PanelIndex {
    Cartesian(usize),
    Pie(usize),
}
