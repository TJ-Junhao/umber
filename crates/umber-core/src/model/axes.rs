//! The axes: their cells on the grid, scales, texts, explicit limits, tick settings and marks,
//! with their getters. [`AxesMut`](super::AxesMut) holds the setters, which validate every
//! input before storing it.

use std::sync::Arc;

use super::grid::CellRange;
use super::mark::Mark;
use super::share::AxisSettings;
use super::ticks::{AxisTicks, ExplicitTicks};
use crate::error::number;
use crate::{ContinuousScale, Error, LegendStyle, Result, ScaleKind, TickFormat, Transform};

/// A plotting area with its scales, texts, limits and marks.
///
/// Create one with [`Figure::add_axes`](super::Figure::add_axes) or
/// [`Figure::add_axes_at`](super::Figure::add_axes_at), which return an
/// [`AxesMut`](super::AxesMut) to change it through; get it back with
/// [`Figure::axes_at`](super::Figure::axes_at).
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.xlim(0.0, 10.0)?.xlabel("time");
/// assert_eq!(axes.x_limits(), Some((0.0, 10.0)));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Axes {
    /// The cells the axes cover on the figure's grid.
    pub(super) cells: CellRange,
    pub(super) x: ScaleKind,
    /// The y axis is always linear or log, so it is stored as its transform.
    pub(super) y: Transform,
    pub(super) title: String,
    pub(super) subtitle: String,
    pub(super) xlabel: String,
    pub(super) ylabel: String,
    /// The x group's settings, the same allocation held by the figure and every member.
    pub(super) x_settings: Arc<AxisSettings>,
    /// The y group's settings.
    pub(super) y_settings: Arc<AxisSettings>,
    /// The legend style set for these axes; the theme's applies when unset.
    pub(super) legend: Option<LegendStyle>,
    pub(super) marks: Vec<Mark>,
}

impl Axes {
    /// Empty axes covering `cells`, with an x axis of scale kind `x` and a y axis of transform
    /// `y`.
    pub(super) fn new(
        x: ScaleKind,
        y: Transform,
        cells: CellRange,
        x_settings: Arc<AxisSettings>,
        y_settings: Arc<AxisSettings>,
    ) -> Axes {
        Axes {
            cells,
            x,
            y,
            title: String::new(),
            subtitle: String::new(),
            xlabel: String::new(),
            ylabel: String::new(),
            x_settings,
            y_settings,
            legend: None,
            marks: Vec::new(),
        }
    }

    /// The kind of the x scale.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// assert_eq!(axes.x_scale(), ScaleKind::Category);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_scale(&self) -> ScaleKind {
        self.x
    }

    /// The kind of the y scale: linear or log, never a category scale.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.y_scale(), ScaleKind::Linear);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y_scale(&self) -> ScaleKind {
        self.y.kind()
    }

    /// The transform of the y axis.
    pub(crate) fn y_transform(&self) -> Transform {
        self.y
    }

    /// The cells the axes cover on the figure's grid.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Grid, GridIndex, Placement, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::with_grid(9.0, 6.0, Theme::pigment(), Grid::new(2, 3)?)?;
    /// let at = Placement {
    ///     row: GridIndex::At(1),
    ///     col: GridIndex::At(1),
    ///     col_span: 2,
    ///     ..Placement::default()
    /// };
    /// let cells = figure.add_axes_at(at, ScaleKind::Linear, ScaleKind::Linear)?.cells();
    /// assert_eq!((cells.row(), cells.col(), cells.row_span(), cells.col_span()), (1, 1, 1, 2));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn cells(&self) -> CellRange {
        self.cells
    }

    /// Checks that the axes were created with the x scale `x` and the y scale `y`, kind and log
    /// base alike, as [`Figure::axes_at`](super::Figure::axes_at) asks its caller to state
    /// them. `Error::Usage` in `"Figure.axes_at()"` naming the first scale that differs, `x`
    /// before `y`, and the scale the axes have.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, LogBase, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let log2 = ScaleKind::Log { base: LogBase::new(2.0)? };
    /// let axes = figure.add_axes(ScaleKind::Category, log2)?;
    /// assert!(axes.check_scales(ScaleKind::Category, log2).is_ok());
    /// let err = axes.check_scales(ScaleKind::Linear, log2).unwrap_err();
    /// let message = "x is Scale.linear(); the axes at row 0, col 0 has x Scale.category().";
    /// assert_eq!(err.to_string(), format!("Figure.axes_at(): {message}"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn check_scales(&self, x: ScaleKind, y: ScaleKind) -> Result<()> {
        for (name, given, actual) in [("x", x, self.x), ("y", y, self.y.kind())] {
            if given != actual {
                return Err(Error::usage(
                    "Figure.axes_at()",
                    format!(
                        "{name} is {}; the axes at row {}, col {} has {name} {}.",
                        scale_name(given),
                        self.cells.row(),
                        self.cells.col(),
                        scale_name(actual),
                    ),
                )
                .with_parameter(name)
                .with_hint("pass the scales the axes was created with."));
            }
        }
        Ok(())
    }

    /// The title text; empty when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.title_text(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn title_text(&self) -> &str {
        &self.title
    }

    /// The subtitle text; empty when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.subtitle_text(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn subtitle_text(&self) -> &str {
        &self.subtitle
    }

    /// The x label text; empty when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.xlabel_text(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn xlabel_text(&self) -> &str {
        &self.xlabel
    }

    /// The y label text; empty when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.ylabel_text(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn ylabel_text(&self) -> &str {
        &self.ylabel
    }

    /// The explicit x limits, if set.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.x_limits(), None);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_limits(&self) -> Option<(f64, f64)> {
        self.x_settings.limits.map(|scale| scale.domain())
    }

    /// The explicit y limits, if set.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.ylim(0.0, 5.0)?;
    /// assert_eq!(axes.y_limits(), Some((0.0, 5.0)));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y_limits(&self) -> Option<(f64, f64)> {
        self.y_settings.limits.map(|scale| scale.domain())
    }

    /// The x scale that the explicit x limits define, if set.
    pub(crate) fn x_limit_scale(&self) -> Option<ContinuousScale> {
        self.x_settings.limits
    }

    /// The y scale that the explicit y limits define, if set.
    pub(crate) fn y_limit_scale(&self) -> Option<ContinuousScale> {
        self.y_settings.limits
    }

    /// The explicit x ticks; `None` when the x axis uses the automatic ticks.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.x_ticks(), None);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_ticks(&self) -> Option<&ExplicitTicks> {
        self.x_settings.ticks.explicit()
    }

    /// The explicit y ticks; `None` when the y axis uses the automatic ticks.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ExplicitTicks, Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.yticks(&[])?;
    /// assert_eq!(axes.y_ticks(), Some(&ExplicitTicks::Positions(Vec::new())));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y_ticks(&self) -> Option<&ExplicitTicks> {
        self.y_settings.ticks.explicit()
    }

    /// The format of the x tick labels; [`TickFormat::AUTO`] when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme, TickFormat};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.x_tick_format(), &TickFormat::AUTO);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_tick_format(&self) -> &TickFormat {
        self.x_settings.ticks.format()
    }

    /// The format of the y tick labels; [`TickFormat::AUTO`] when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme, TickFormat};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.y_tick_format(), &TickFormat::AUTO);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y_tick_format(&self) -> &TickFormat {
        self.y_settings.ticks.format()
    }

    /// The tick settings of the x axis.
    pub(crate) fn x_tick_settings(&self) -> &AxisTicks {
        &self.x_settings.ticks
    }

    /// The tick settings of the y axis.
    pub(crate) fn y_tick_settings(&self) -> &AxisTicks {
        &self.y_settings.ticks
    }

    /// The legend style set with [`AxesMut::legend`](super::AxesMut::legend); `None` when
    /// unset, in which case the theme's style applies.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.legend_style(), None);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn legend_style(&self) -> Option<LegendStyle> {
        self.legend
    }

    /// The marks, in the order they were added.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert!(axes.marks().is_empty());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }
}

/// `kind` as the Python `Scale` call that makes it: `Scale.linear()`, `Scale.log(2)` or
/// `Scale.category()`.
pub(super) fn scale_name(kind: ScaleKind) -> String {
    match kind {
        ScaleKind::Linear => "Scale.linear()".to_owned(),
        ScaleKind::Log { base } => format!("Scale.log({})", number(base.get())),
        ScaleKind::Category => "Scale.category()".to_owned(),
    }
}
