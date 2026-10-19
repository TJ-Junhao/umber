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
///         Panel::Cartesian { axes, .. } => assert!(axes.marks().is_empty()),
///         Panel::Pie(pie) => assert_eq!(pie.title_text(), "Parts"),
///     }
/// }
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel<'a> {
    /// Cartesian axes and their optional right-hand numeric y axes, in the same grid cells.
    Cartesian {
        /// The primary axes, which own the shared x axis and grid lines.
        axes: &'a Axes,
        /// The optional right-hand numeric y axes in the same cells.
        twin: Option<&'a Axes>,
    },
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
            Self::Cartesian { axes, .. } => axes.cells(),
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
            Self::Cartesian { axes, .. } => axes.title_text(),
            Self::Pie(pie) => pie.title_text(),
        }
    }

    pub(crate) fn cartesian(self) -> Option<&'a Axes> {
        match self {
            Self::Cartesian { axes, .. } => Some(axes),
            Self::Pie(_) => None,
        }
    }

    pub(crate) fn title_size(self) -> SizeSpec {
        match self {
            Self::Cartesian { axes, .. } => axes.title_size(),
            Self::Pie(pie) => pie.title_size,
        }
    }

    pub(crate) fn legend_size(self) -> SizeSpec {
        match self {
            Self::Cartesian { axes, .. } => axes.legend_size(),
            Self::Pie(pie) => pie.legend_size,
        }
    }

    pub(crate) fn subtitle_text(self) -> &'a str {
        match self {
            Self::Cartesian { axes, .. } => axes.subtitle_text(),
            Self::Pie(_) => "",
        }
    }

    pub(crate) fn subtitle_size(self) -> SizeSpec {
        match self {
            Self::Cartesian { axes, .. } => axes.subtitle_size(),
            Self::Pie(_) => SizeSpec::Auto,
        }
    }
}

impl<'a> From<&'a Axes> for Panel<'a> {
    fn from(axes: &'a Axes) -> Self {
        Self::Cartesian { axes, twin: None }
    }
}

/// A draw layer retains whether it owns a panel or belongs to another layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Layer<'a> {
    /// The panel's primary axes, or its pie.
    Primary(Panel<'a>),
    /// The right y axes and the primary layer's index in the same traversal.
    Twin { axes: &'a Axes, parent: usize },
}

impl<'a> Layer<'a> {
    pub(crate) fn cells(self) -> CellRange {
        match self {
            Self::Primary(panel) => panel.cells(),
            Self::Twin { axes, .. } => axes.cells(),
        }
    }

    pub(crate) fn cartesian(self) -> Option<&'a Axes> {
        match self {
            Self::Primary(panel) => panel.cartesian(),
            Self::Twin { axes, .. } => Some(axes),
        }
    }

    pub(crate) fn title_text(self) -> &'a str {
        match self {
            Self::Primary(panel) => panel.title_text(),
            Self::Twin { .. } => "",
        }
    }

    pub(crate) fn title_size(self) -> SizeSpec {
        match self {
            Self::Primary(panel) => panel.title_size(),
            Self::Twin { axes, .. } => axes.title_size(),
        }
    }

    pub(crate) fn subtitle_text(self) -> &'a str {
        match self {
            Self::Primary(panel) => panel.subtitle_text(),
            Self::Twin { .. } => "",
        }
    }

    pub(crate) fn subtitle_size(self) -> SizeSpec {
        match self {
            Self::Primary(panel) => panel.subtitle_size(),
            Self::Twin { axes, .. } => axes.subtitle_size(),
        }
    }

    pub(crate) fn legend_size(self) -> SizeSpec {
        match self {
            Self::Primary(panel) => panel.legend_size(),
            Self::Twin { axes, .. } => axes.legend_size(),
        }
    }
}

impl<'a> From<Panel<'a>> for Layer<'a> {
    fn from(panel: Panel<'a>) -> Self {
        Self::Primary(panel)
    }
}

impl<'a> From<&'a Axes> for Layer<'a> {
    fn from(axes: &'a Axes) -> Self {
        Self::Primary(axes.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Figure, Grid, GridIndex, Placement, ScaleKind, Theme};

    #[test]
    fn layers_carry_their_parent_and_panels_keep_their_twin() {
        let mut figure =
            Figure::with_grid(12., 4., Theme::pigment(), Grid::new(1, 3).unwrap()).unwrap();
        for col in [2, 0] {
            let at = Placement {
                col: GridIndex::At(col),
                ..Placement::default()
            };
            figure
                .add_axes_at(at, ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .twin_y(ScaleKind::Linear)
                .unwrap();
        }
        figure
            .add_pie_axes_at(Placement {
                col: GridIndex::At(1),
                ..Placement::default()
            })
            .unwrap();
        let layers: Vec<_> = figure.layers().collect();
        assert_eq!(layers.len(), 5);
        for (i, layer) in layers.iter().enumerate() {
            match layer {
                Layer::Primary(Panel::Cartesian {
                    axes,
                    twin: Some(twin),
                }) => {
                    assert_eq!(axes.cells(), twin.cells());
                    assert!(layers.iter().any(|layer| matches!(layer,
                        Layer::Twin { parent, axes } if *parent == i && std::ptr::eq(*axes, *twin))));
                }
                Layer::Primary(Panel::Cartesian { twin: None, .. }) => panic!("twin expected"),
                Layer::Primary(Panel::Pie(_)) => assert_eq!(i, 2),
                Layer::Twin { axes, parent } => {
                    assert_eq!(axes.cells(), layers[*parent].cells());
                    assert!(matches!(layers[*parent], Layer::Primary(_)));
                }
            }
        }
    }
}

/// Stable, kind-specific storage indices, ordered by top-left cell in `Figure`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum PanelIndex {
    Cartesian(usize),
    Pie(usize),
}
