//! The figure: a size, a theme, a grid and the axes placed on it.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::axes::Axes;
use super::axes_mut::AxesMut;
use super::grid::{CellRange, Grid, GridIndex, Placement};
use super::share::{self, AxisSettings, Group, Setting, SharedAxis};
use super::validation::check_figure_size;
use crate::{Error, Result, ScaleKind, Theme};

/// The context of the errors of [`Figure::add_axes`] and [`Figure::add_axes_at`].
const ADD_AXES: &str = "Figure.add_axes()";

/// The context of the errors of [`Figure::axes_at`] and [`Figure::axes_at_mut`].
const AXES_AT: &str = "Figure.axes_at()";

/// The hint for a row or column out of range.
const RANGE_HINT: &str = "rows and columns count from 0.";

/// A figure: a size, a theme and a [`Grid`] of cells, each holding at most one [`Axes`]. An
/// axes may span several cells.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Grid, GridIndex, Placement, ScaleKind, Theme};
///
/// let figure = Figure::new(6.0, 4.0, Theme::graphite())?;
/// assert_eq!((figure.width_in(), figure.height_in()), (6.0, 4.0));
/// assert_eq!(figure.panels().count(), 0);
///
/// let mut grid = Figure::with_grid(12.0, 4.0, Theme::graphite(), Grid::new(1, 2)?)?;
/// let right = Placement { col: GridIndex::At(1), ..Placement::default() };
/// grid.add_axes_at(right, ScaleKind::Linear, ScaleKind::Linear)?.title("Right");
/// assert_eq!(grid.axes_at(GridIndex::Auto, GridIndex::At(1))?.title_text(), "Right");
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Figure {
    width_in: f64,
    height_in: f64,
    theme: Theme,
    grid: Grid,
    title: String,
    subtitle: String,
    /// The placed axes, in row-major order of their top-left cells. No two cover one cell.
    panels: Vec<Axes>,
    /// Settings stored once per x group, keyed independently of panel insertion order.
    x_groups: BTreeMap<usize, Group>,
    /// Settings stored once per y group.
    y_groups: BTreeMap<usize, Group>,
    /// Explicit color limits and the cells that set them.
    color_groups: BTreeMap<usize, share::ColorGroup>,
    /// Settings of every shared colorbar.
    colorbar: super::ColorbarOptions,
}

impl Figure {
    /// Creates an empty figure with one cell, a 1 × 1 grid. Sides are in inches;
    /// `Error::Style` unless each is finite, above 0 and at most 200.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// let figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert_eq!((figure.grid().rows(), figure.grid().cols()), (1, 1));
    /// assert!(Figure::new(0.0, 4.0, Theme::pigment()).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn new(width_in: f64, height_in: f64, theme: Theme) -> Result<Figure> {
        Figure::with_grid(width_in, height_in, theme, Grid::SINGLE)
    }

    /// Creates an empty figure on `grid`. Sides are in inches, the whole figure; the cells
    /// share it equally. `Error::Style` unless each side is finite, above 0 and at most 200.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Grid, Theme};
    ///
    /// let grid = Grid::new(2, 3)?;
    /// let (width, height) = grid.auto_size_in();
    /// let figure = Figure::with_grid(width, height, Theme::midnight(), grid)?;
    /// assert_eq!(figure.grid(), &grid);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_grid(width_in: f64, height_in: f64, theme: Theme, grid: Grid) -> Result<Figure> {
        check_figure_size(width_in, height_in)?;
        Ok(Figure {
            width_in,
            height_in,
            theme,
            grid,
            title: String::new(),
            subtitle: String::new(),
            panels: Vec::new(),
            x_groups: BTreeMap::new(),
            y_groups: BTreeMap::new(),
            color_groups: BTreeMap::new(),
            colorbar: super::ColorbarOptions::default(),
        })
    }

    /// Replaces the shared colorbars' label, position and tick format.
    ///
    /// Row colorbars sit on the right and column colorbars below. For all panels sharing,
    /// an unset position follows the theme. An unset format follows the first color-mapped
    /// panel in each group's row-major order. Settings may be given before adding marks.
    ///
    /// # Errors
    ///
    /// `Error::Usage` when colors are not shared; `Error::Style` for a position incompatible
    /// with row or column sharing or an invalid label. Failed calls leave the figure unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ColorbarOptions, Figure, Grid, Share, Theme};
    /// let grid = Grid::new(2, 2)?.with_share_color(Share::All);
    /// let mut figure = Figure::with_grid(12.0, 8.0, Theme::pigment(), grid)?;
    /// figure.colorbar(ColorbarOptions { label: "Intensity".into(), ..Default::default() })?;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn colorbar(&mut self, options: super::ColorbarOptions) -> Result<&mut Self> {
        use crate::{ColorbarPosition, Share};
        const CONTEXT: &str = "Figure.colorbar()";
        let (mode, required, where_) = match self.grid.share_color() {
            Share::None => {
                return Err(
                    Error::usage(CONTEXT, "colors are not shared across panels.").with_hint(
                        "set each panel's colorbar with Axes.colorbar(), or share colors.",
                    ),
                );
            }
            Share::Row => ("ROW", Some(ColorbarPosition::Right), "right of each row"),
            Share::Col => ("COL", Some(ColorbarPosition::Bottom), "below each column"),
            Share::All => ("ALL", None, ""),
        };
        if let (Some(position), Some(required)) = (options.position, required)
            && position != required
        {
            let name = match position {
                ColorbarPosition::Right => "RIGHT",
                ColorbarPosition::Bottom => "BOTTOM",
                ColorbarPosition::None => "NONE",
            };
            return Err(Error::style(
                CONTEXT,
                format!("position is {name}; with share_color={mode} the colorbar sits {where_}."),
            )
            .with_parameter("position")
            .with_hint("use AUTO for the shared colorbar position, or share colors with ALL."));
        }
        super::check_text(CONTEXT, "label", "label", &options.label)?;
        self.colorbar = options;
        Ok(self)
    }

    /// The settings of shared colorbars.
    pub(crate) fn colorbar_options(&self) -> &super::ColorbarOptions {
        &self.colorbar
    }

    /// Sets the figure title, above the panels and the merged legend.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// figure.title("Revenue");
    /// assert_eq!(figure.title_text(), "Revenue");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn title(&mut self, text: &str) -> &mut Self {
        self.title = text.to_owned();
        self
    }

    /// Sets the figure subtitle, below its title and above the merged legend.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// figure.subtitle("2026");
    /// assert_eq!(figure.subtitle_text(), "2026");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn subtitle(&mut self, text: &str) -> &mut Self {
        self.subtitle = text.to_owned();
        self
    }

    /// The figure title; empty when none is set.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// let figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert_eq!(figure.title_text(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn title_text(&self) -> &str {
        &self.title
    }

    /// The figure subtitle; empty when none is set.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// let figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert_eq!(figure.subtitle_text(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn subtitle_text(&self) -> &str {
        &self.subtitle
    }

    /// The grid.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Grid, Theme};
    ///
    /// let figure = Figure::with_grid(9.0, 6.0, Theme::pigment(), Grid::new(2, 2)?)?;
    /// assert_eq!(figure.grid().rows(), 2);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Adds an axes in the one cell of a 1 × 1 grid, with an x axis of scale `x` and a y axis
    /// of scale `y`: [`Figure::add_axes_at`] at [`Placement::default`]. On a larger grid the
    /// position must be given, so this is an error there.
    ///
    /// # Errors
    ///
    /// As [`Figure::add_axes_at`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?.title("Sales");
    /// let err = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "Figure.add_axes(): row 0, col 0 is taken by the axes added at row 0, col 0."
    /// );
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn add_axes(&mut self, x: ScaleKind, y: ScaleKind) -> Result<AxesMut<'_>> {
        self.add_axes_at(Placement::default(), x, y)
    }

    /// Adds an axes at `at`, with an x axis of scale `x` and a y axis of scale `y`, and returns
    /// it to draw on.
    ///
    /// # Errors
    ///
    /// Errors add no axes. They are `Error::Usage`, checked in this order:
    /// - `at.row` is `Auto` on a grid of more than one row, or outside the grid (parameter
    ///   `row`);
    /// - `at.col` is `Auto` on a grid of more than one column, or outside the grid (`col`);
    /// - `at.row_span` is below 1, or the rows spanned run past the last row (`row_span`);
    /// - `at.col_span` is below 1, or the columns spanned run past the last column (`col_span`);
    /// - a cell is taken by an axes already added (`row`): the message names the first taken
    ///   cell in row-major order and the top-left cell of the axes that holds it;
    /// - a shared group would mix scale kinds or log bases (`x`, then `y`);
    /// - a span would cross groups of a shared dimension (`row_span` or `col_span`, x before y).
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Grid, GridIndex, Placement, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::with_grid(12.0, 8.0, Theme::pigment(), Grid::new(2, 3)?)?;
    /// let at = |row, col, col_span| Placement {
    ///     row: GridIndex::At(row),
    ///     col: GridIndex::At(col),
    ///     col_span,
    ///     ..Placement::default()
    /// };
    /// figure.add_axes_at(at(0, 0, 2), ScaleKind::Linear, ScaleKind::Linear)?;
    /// let overlapping = figure.add_axes_at(at(0, 1, 2), ScaleKind::Linear, ScaleKind::Linear);
    /// let err = overlapping.unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "Figure.add_axes(): row 0, col 1 is taken by the axes added at row 0, col 0."
    /// );
    /// let err = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap_err();
    /// assert_eq!(err.parameter(), Some("row"));
    /// figure.add_axes_at(at(1, 0, 3), ScaleKind::Category, ScaleKind::Linear)?;
    /// assert_eq!(figure.panels().count(), 2);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn add_axes_at(
        &mut self,
        at: Placement,
        x: ScaleKind,
        y: ScaleKind,
    ) -> Result<AxesMut<'_>> {
        let cells = self.placement_cells(at)?;
        let (row, col) = (cells.row(), cells.col());
        let taken = cells.iter().find_map(|cell| {
            self.panels
                .iter()
                .find(|axes| axes.cells().contains(cell.row, cell.col))
                .map(|axes| (cell, axes.cells()))
        });
        if let Some((cell, holder)) = taken {
            return Err(Error::usage(
                ADD_AXES,
                format!(
                    "row {}, col {} is taken by the axes added at row {}, col {}.",
                    cell.row,
                    cell.col,
                    holder.row(),
                    holder.col()
                ),
            )
            .with_parameter("row")
            .with_hint("each cell holds one axes; pick an empty cell."));
        }
        share::check_add(self, cells, x, y)?;
        let x_key = self.x_group_of(row, col);
        let y_key = self.y_group_of(row, col);
        let x_settings = Arc::clone(&self.x_groups.entry(x_key).or_default().settings);
        let y_settings = Arc::clone(&self.y_groups.entry(y_key).or_default().settings);
        // Panels stay in row-major order of their top-left cells, which no two share.
        let index = self
            .panels
            .partition_point(|axes| (axes.cells().row(), axes.cells().col()) < (row, col));
        let mut axes = Axes::new(x, y, cells, x_settings, y_settings);
        axes.color_limits = self
            .color_groups
            .get(&self.color_group_of(row, col))
            .and_then(|group| group.limits);
        self.panels.insert(index, axes);
        Ok(AxesMut::new(self, index))
    }

    /// The cells that an axes added at `at` would cover on this figure's grid, checked as
    /// [`Figure::add_axes_at`] checks the position, before any other of its checks. It does not
    /// look at the axes already added: a cell may be taken.
    ///
    /// # Errors
    ///
    /// `Error::Usage` in `"Figure.add_axes()"`, as the first four rows of the error list of
    /// [`Figure::add_axes_at`]: the row, the column, the row span, then the column span.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Grid, GridIndex, Placement, Theme};
    ///
    /// let figure = Figure::with_grid(9.0, 6.0, Theme::pigment(), Grid::new(2, 3)?)?;
    /// let at = Placement {
    ///     row: GridIndex::At(1),
    ///     col: GridIndex::At(1),
    ///     col_span: 2,
    ///     ..Placement::default()
    /// };
    /// let cells = figure.placement_cells(at)?;
    /// assert_eq!((cells.row(), cells.col(), cells.col_span()), (1, 1, 2));
    /// let past = Placement { col_span: 3, ..at };
    /// assert_eq!(figure.placement_cells(past).unwrap_err().parameter(), Some("col_span"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn placement_cells(&self, at: Placement) -> Result<CellRange> {
        let (rows, cols) = (self.grid.rows(), self.grid.cols());
        let row = auto_index(ADD_AXES, Dimension::ROW, at.row, rows)?;
        let row = in_range(ADD_AXES, Dimension::ROW, row, rows)?;
        let col = auto_index(ADD_AXES, Dimension::COL, at.col, cols)?;
        let col = in_range(ADD_AXES, Dimension::COL, col, cols)?;
        let row_span = positive_span(Dimension::ROW, at.row_span)?;
        let row_span = span_in_grid(Dimension::ROW, row, row_span, rows)?;
        let col_span = positive_span(Dimension::COL, at.col_span)?;
        let col_span = span_in_grid(Dimension::COL, col, col_span, cols)?;
        Ok(CellRange::new(row, col, row_span, col_span))
    }

    /// The axes whose top-left cell is at `row`, `col`. A spanning axes is found at its
    /// top-left cell only.
    ///
    /// # Errors
    ///
    /// `Error::Usage` in `"Figure.axes_at()"`, checked in this order: `row` is `Auto` on a grid
    /// of more than one row, then likewise `col`; `row` and then `col` is outside the grid
    /// (each with that parameter); no axes covers the cell; the cell is covered by an axes
    /// whose top-left cell is elsewhere (the message names it).
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Grid, GridIndex, Placement, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::with_grid(9.0, 6.0, Theme::pigment(), Grid::new(2, 2)?)?;
    /// let tall = Placement {
    ///     row: GridIndex::At(0),
    ///     col: GridIndex::At(1),
    ///     row_span: 2,
    ///     ..Placement::default()
    /// };
    /// figure.add_axes_at(tall, ScaleKind::Linear, ScaleKind::Linear)?.title("Tall");
    /// assert_eq!(figure.axes_at(GridIndex::At(0), GridIndex::At(1))?.title_text(), "Tall");
    /// let err = figure.axes_at(GridIndex::At(1), GridIndex::At(1)).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "Figure.axes_at(): row 1, col 1 is part of the axes added at row 0, col 1."
    /// );
    /// assert!(figure.axes_at(GridIndex::At(0), GridIndex::At(0)).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn axes_at(&self, row: GridIndex, col: GridIndex) -> Result<&Axes> {
        let index = self.index_at(row, col)?;
        Ok(self.panel(index))
    }

    /// The axes whose top-left cell is at `row`, `col`, to change. Same rules and errors as
    /// [`Figure::axes_at`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, GridIndex, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// figure.axes_at_mut(GridIndex::Auto, GridIndex::Auto)?.title("again");
    /// assert_eq!(figure.axes_at(GridIndex::At(0), GridIndex::At(0))?.title_text(), "again");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn axes_at_mut(&mut self, row: GridIndex, col: GridIndex) -> Result<AxesMut<'_>> {
        let index = self.index_at(row, col)?;
        Ok(AxesMut::new(self, index))
    }

    /// The placed axes, in row-major order of their top-left cells.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Grid, GridIndex, Placement, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::with_grid(12.0, 4.0, Theme::pigment(), Grid::new(1, 3)?)?;
    /// for (col, title) in [(2, "C"), (0, "A")] {
    ///     let at = Placement { col: GridIndex::At(col), ..Placement::default() };
    ///     figure.add_axes_at(at, ScaleKind::Linear, ScaleKind::Linear)?.title(title);
    /// }
    /// let titles: Vec<&str> = figure.panels().map(|axes| axes.title_text()).collect();
    /// assert_eq!(titles, ["A", "C"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn panels(&self) -> impl Iterator<Item = &Axes> {
        self.panels.iter()
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

    /// The axes at `index` among the panels. Every [`AxesMut`] holds an index in range for as
    /// long as it borrows the figure.
    pub(super) fn panel(&self, index: usize) -> &Axes {
        &self.panels[index]
    }

    /// The axes at `index` among the panels, to change; see [`Figure::panel`].
    pub(super) fn panel_mut(&mut self, index: usize) -> &mut Axes {
        &mut self.panels[index]
    }

    /// The stable x group key of a cell.
    pub(crate) fn x_group_of(&self, row: usize, col: usize) -> usize {
        share::group_key(&self.grid, self.grid.share_x(), row, col)
    }

    /// The stable y group key of a cell.
    pub(crate) fn y_group_of(&self, row: usize, col: usize) -> usize {
        share::group_key(&self.grid, self.grid.share_y(), row, col)
    }

    /// The stable color group key of a cell.
    pub(crate) fn color_group_of(&self, row: usize, col: usize) -> usize {
        share::group_key(&self.grid, self.grid.share_color(), row, col)
    }

    /// Checks a point mapping against the other members before a mark is stored.
    pub(super) fn check_shared_point_color(
        &self,
        index: usize,
        color: &super::PointColor,
        context: &'static str,
    ) -> Result<()> {
        let super::PointColor::By(by) = color else {
            return Ok(());
        };
        self.check_shared_color(
            index,
            &crate::color_scale::ColorScaleSpec::of(by),
            context,
            "color",
        )
    }

    /// Checks a new color mapping against the existing mappings in its group.
    pub(super) fn check_shared_color(
        &self,
        index: usize,
        spec: &crate::color_scale::ColorScaleSpec,
        context: &'static str,
        parameter: &'static str,
    ) -> Result<()> {
        let cell = self.panels[index].cells();
        let key = self.color_group_of(cell.row(), cell.col());
        for other in self.panels.iter().filter(|axes| {
            let c = axes.cells();
            self.color_group_of(c.row(), c.col()) == key && c != cell
        }) {
            if let Some(existing) = other.marks().iter().find_map(super::Mark::color_scale) {
                spec.check_same(&existing, context, parameter).map_err(|error| {
                    error.with_message_prefix(&format!(
                        "colors are shared with row {}, col {}: ",
                        other.cells().row(), other.cells().col()
                    )).with_hint("use the same cmap, scale and center for every color-mapped mark in the shared group.")
                })?;
            }
        }
        Ok(())
    }

    /// Validates all members, then records and propagates shared color limits atomically.
    pub(super) fn set_color_limits(&mut self, index: usize, limits: (f64, f64)) -> Result<()> {
        let c = self.panels[index].cells();
        let cell = super::Cell {
            row: c.row(),
            col: c.col(),
        };
        let mode = self.grid.share_color();
        let key = self.color_group_of(cell.row, cell.col);
        for axes in &self.panels {
            let c = axes.cells();
            if self.color_group_of(c.row(), c.col()) == key
                && let Some(spec) = axes.marks().iter().find_map(super::Mark::color_scale)
            {
                spec.check_range(limits, "Axes.clim()", None)?;
            }
        }
        self.color_groups
            .entry(key)
            .or_default()
            .set(cell, limits)?;
        for axes in &mut self.panels {
            let c = axes.cells();
            if share::group_key(&self.grid, mode, c.row(), c.col()) == key {
                axes.color_limits = Some(limits);
            }
        }
        Ok(())
    }

    /// Checks and replaces one group setting, then points every member at the new allocation.
    /// A failed check leaves the figure, including its groups, unchanged.
    pub(super) fn set_group(
        &mut self,
        index: usize,
        axis: SharedAxis,
        setting: Setting,
        proposed: AxisSettings,
        context: &'static str,
    ) -> Result<()> {
        let cells = self.panels[index].cells();
        let cell = super::Cell {
            row: cells.row(),
            col: cells.col(),
        };
        let mode = axis.mode(&self.grid);
        let key = share::group_key(&self.grid, mode, cell.row, cell.col);
        let groups = match axis {
            SharedAxis::X => &mut self.x_groups,
            SharedAxis::Y => &mut self.y_groups,
        };
        let group = groups.entry(key).or_default();
        group.set(axis, setting, cell, proposed, context)?;
        let settings = Arc::clone(&group.settings);
        for axes in &mut self.panels {
            let c = axes.cells();
            if share::group_key(&self.grid, mode, c.row(), c.col()) == key {
                match axis {
                    SharedAxis::X => axes.x_settings = Arc::clone(&settings),
                    SharedAxis::Y => axes.y_settings = Arc::clone(&settings),
                }
            }
        }
        Ok(())
    }

    /// The index among the panels of the axes whose top-left cell is at `row`, `col`, with the
    /// errors of [`Figure::axes_at`].
    fn index_at(&self, row: GridIndex, col: GridIndex) -> Result<usize> {
        let (rows, cols) = (self.grid.rows(), self.grid.cols());
        let row = auto_index(AXES_AT, Dimension::ROW, row, rows)?;
        let col = auto_index(AXES_AT, Dimension::COL, col, cols)?;
        let row = in_range(AXES_AT, Dimension::ROW, row, rows)?;
        let col = in_range(AXES_AT, Dimension::COL, col, cols)?;
        let Some(index) = self
            .panels
            .iter()
            .position(|axes| axes.cells().contains(row, col))
        else {
            return Err(
                Error::usage(AXES_AT, format!("row {row}, col {col} holds no axes."))
                    .with_hint("add one with add_axes(row=, col=) first."),
            );
        };
        let cells = self.panel(index).cells();
        if (cells.row(), cells.col()) != (row, col) {
            return Err(Error::usage(
                AXES_AT,
                format!(
                    "row {row}, col {col} is part of the axes added at row {}, col {}.",
                    cells.row(),
                    cells.col()
                ),
            )
            .with_hint("find a spanning axes at its top-left cell."));
        }
        Ok(index)
    }
}

/// A dimension of the grid, by the names its messages use.
#[derive(Debug, Clone, Copy)]
struct Dimension {
    /// The parameter of its index: `"row"` or `"col"`.
    index: &'static str,
    /// The parameter of its span: `"row_span"` or `"col_span"`.
    span: &'static str,
    /// Its lines, plural: `"rows"` or `"columns"`.
    lines: &'static str,
    /// One of its lines: `"row"` or `"column"`.
    line: &'static str,
    /// The hint for an `Auto` index on a grid with more than one line.
    auto_hint: &'static str,
    /// The hint for a span below 1.
    span_hint: &'static str,
}

impl Dimension {
    const ROW: Dimension = Dimension {
        index: "row",
        span: "row_span",
        lines: "rows",
        line: "row",
        auto_hint: "pass row=, counting from 0.",
        span_hint: "use 1 for an axes in one row.",
    };
    const COL: Dimension = Dimension {
        index: "col",
        span: "col_span",
        lines: "columns",
        line: "column",
        auto_hint: "pass col=, counting from 0.",
        span_hint: "use 1 for an axes in one column.",
    };
}

/// `index` along `dim` with `Auto` read as 0, which only a dimension of `size` 1 allows.
fn auto_index(context: &'static str, dim: Dimension, index: GridIndex, size: usize) -> Result<i64> {
    match index {
        GridIndex::At(i) => Ok(i),
        GridIndex::Auto if size == 1 => Ok(0),
        GridIndex::Auto => Err(Error::usage(
            context,
            format!(
                "{name} is AUTO; this figure has {size} {lines}, so each axes needs a {name}.",
                name = dim.index,
                lines = dim.lines,
            ),
        )
        .with_parameter(dim.index)
        .with_hint(dim.auto_hint)),
    }
}

/// `index` along `dim`, if it lies on a dimension of `size`, which is at least 1.
fn in_range(context: &'static str, dim: Dimension, index: i64, size: usize) -> Result<usize> {
    usize::try_from(index)
        .ok()
        .filter(|&i| i < size)
        .ok_or_else(|| {
            Error::usage(
                context,
                format!(
                    "{name} is {index}; this figure has {lines} 0 to {last}.",
                    name = dim.index,
                    lines = dim.lines,
                    last = size.saturating_sub(1),
                ),
            )
            .with_parameter(dim.index)
            .with_hint(RANGE_HINT)
        })
}

/// `span` along `dim`, if it is at least 1.
fn positive_span(dim: Dimension, span: i64) -> Result<i64> {
    if span >= 1 {
        Ok(span)
    } else {
        Err(Error::usage(
            ADD_AXES,
            format!(
                "{name} is {span}; {name} must be at least 1.",
                name = dim.span
            ),
        )
        .with_parameter(dim.span)
        .with_hint(dim.span_hint))
    }
}

/// `span` lines along `dim` from `start`, if they end within a dimension of `size`; `start` is
/// on it.
fn span_in_grid(dim: Dimension, start: usize, span: i64, size: usize) -> Result<usize> {
    usize::try_from(span)
        .ok()
        .filter(|&n| n <= size.saturating_sub(start))
        .ok_or_else(|| {
            Error::usage(
                ADD_AXES,
                format!(
                    "{index} {start} with {name} {span} runs past the last {line}, {last}.",
                    index = dim.index,
                    name = dim.span,
                    line = dim.line,
                    last = size.saturating_sub(1),
                ),
            )
            .with_parameter(dim.span)
            .with_hint("use a smaller span or a larger grid.")
        })
}

#[cfg(test)]
impl Figure {
    /// Every member points to the allocation held by its x and y groups.
    pub(super) fn assert_group_settings_shared(&self) {
        for axes in &self.panels {
            let cells = axes.cells();
            let x = &self.x_groups[&self.x_group_of(cells.row(), cells.col())];
            let y = &self.y_groups[&self.y_group_of(cells.row(), cells.col())];
            assert!(Arc::ptr_eq(&axes.x_settings, &x.settings));
            assert!(Arc::ptr_eq(&axes.y_settings, &y.settings));
        }
    }

    /// The axes of a figure on a 1 × 1 grid. Panics when it has none.
    pub(crate) fn single_axes(&self) -> &Axes {
        self.axes_at(GridIndex::Auto, GridIndex::Auto)
            .expect("the figure has axes")
    }

    /// The axes of a figure on a 1 × 1 grid, to change. Panics when it has none.
    pub(crate) fn single_axes_mut(&mut self) -> AxesMut<'_> {
        self.axes_at_mut(GridIndex::Auto, GridIndex::Auto)
            .expect("the figure has axes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, LineOptions};
    use crate::{FontLibrary, LogBase, SvgText};

    fn at(row: i64, col: i64, row_span: i64, col_span: i64) -> Placement {
        Placement {
            row: GridIndex::At(row),
            col: GridIndex::At(col),
            row_span,
            col_span,
        }
    }

    fn grid(rows: i64, cols: i64) -> Grid {
        Grid::new(rows, cols).unwrap()
    }

    #[test]
    fn grid_axes_are_found_at_their_top_left_cell() -> Result<()> {
        let grid = Grid::new(2, 3)?;
        let mut f = Figure::with_grid(9.0, 6.0, Theme::pigment(), grid)?;
        f.add_axes_at(at(0, 0, 1, 2), ScaleKind::Linear, ScaleKind::Linear)?
            .title("wide");
        f.add_axes_at(at(1, 2, 1, 1), ScaleKind::Category, ScaleKind::Linear)?;
        assert_eq!(
            f.axes_at(GridIndex::At(0), GridIndex::At(0))?.title_text(),
            "wide"
        );
        let err = f.axes_at(GridIndex::At(0), GridIndex::At(1)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Figure.axes_at(): row 0, col 1 is part of the axes added at row 0, col 0."
        );
        assert_eq!(
            err.hint(),
            Some("find a spanning axes at its top-left cell.")
        );
        let err = f.axes_at(GridIndex::At(1), GridIndex::At(0)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Figure.axes_at(): row 1, col 0 holds no axes."
        );
        assert_eq!(err.hint(), Some("add one with add_axes(row=, col=) first."));
        assert_eq!(
            f.axes_at(GridIndex::At(1), GridIndex::At(2))?.x_scale(),
            ScaleKind::Category
        );
        // The handle changes the axes it was found for.
        f.axes_at_mut(GridIndex::At(1), GridIndex::At(2))?
            .title("corner");
        let titles: Vec<&str> = f.panels().map(Axes::title_text).collect();
        assert_eq!(titles, ["wide", "corner"]);
        Ok(())
    }

    /// One row of the error table of `add_axes_at`.
    struct Case {
        grid: Grid,
        place: Placement,
        y: ScaleKind,
        message: &'static str,
        parameter: &'static str,
        hint: &'static str,
    }

    /// One case per row of the error table of `add_axes_at`, in its order, plus the taken
    /// cell of a 1 × 1 grid. Each runs against [`holding_one`] on its grid.
    fn table() -> Vec<Case> {
        let case = |grid, place, y, message, parameter, hint| Case {
            grid,
            place,
            y,
            message,
            parameter,
            hint,
        };
        let auto = Placement::default();
        let linear = ScaleKind::Linear;
        let category = ScaleKind::Category;
        vec![
            case(
                grid(3, 3),
                auto,
                linear,
                "row is AUTO; this figure has 3 rows, so each axes needs a row.",
                "row",
                "pass row=, counting from 0.",
            ),
            case(
                grid(3, 3),
                at(3, -1, 0, 0),
                category,
                "row is 3; this figure has rows 0 to 2.",
                "row",
                "rows and columns count from 0.",
            ),
            case(
                grid(2, 2),
                Placement {
                    row: GridIndex::At(5),
                    ..auto
                },
                category,
                "row is 5; this figure has rows 0 to 1.",
                "row",
                "rows and columns count from 0.",
            ),
            case(
                grid(1, 4),
                auto,
                category,
                "col is AUTO; this figure has 4 columns, so each axes needs a col.",
                "col",
                "pass col=, counting from 0.",
            ),
            case(
                grid(3, 3),
                at(0, -1, 0, 0),
                linear,
                "col is -1; this figure has columns 0 to 2.",
                "col",
                "rows and columns count from 0.",
            ),
            case(
                grid(3, 3),
                at(0, 0, 0, 0),
                linear,
                "row_span is 0; row_span must be at least 1.",
                "row_span",
                "use 1 for an axes in one row.",
            ),
            case(
                grid(3, 3),
                at(1, 0, 3, 9),
                category,
                "row 1 with row_span 3 runs past the last row, 2.",
                "row_span",
                "use a smaller span or a larger grid.",
            ),
            case(
                grid(3, 3),
                at(1, 0, 3, -2),
                category,
                "row 1 with row_span 3 runs past the last row, 2.",
                "row_span",
                "use a smaller span or a larger grid.",
            ),
            case(
                grid(3, 3),
                at(0, 0, 1, -2),
                linear,
                "col_span is -2; col_span must be at least 1.",
                "col_span",
                "use 1 for an axes in one column.",
            ),
            case(
                grid(3, 3),
                at(0, 2, 1, i64::MAX),
                linear,
                "col 2 with col_span 9223372036854775807 runs past the last column, 2.",
                "col_span",
                "use a smaller span or a larger grid.",
            ),
            case(
                grid(3, 3),
                at(1, 0, 1, 1),
                category,
                "row 1, col 0 is taken by the axes added at row 1, col 0.",
                "row",
                "each cell holds one axes; pick an empty cell.",
            ),
            case(
                grid(3, 3),
                at(0, 1, 2, 2),
                linear,
                "row 1, col 1 is taken by the axes added at row 1, col 0.",
                "row",
                "each cell holds one axes; pick an empty cell.",
            ),
            case(
                Grid::SINGLE,
                auto,
                linear,
                "row 0, col 0 is taken by the axes added at row 0, col 0.",
                "row",
                "each cell holds one axes; pick an empty cell.",
            ),
        ]
    }

    /// A figure on `grid` holding one titled axes with a line: in the one cell of a 1 × 1
    /// grid, else spanning two columns from row 1, col 0 (row 0 on a grid of one row).
    fn holding_one(grid: Grid) -> Figure {
        let mut f = Figure::with_grid(9.0, 6.0, Theme::pigment(), grid).unwrap();
        let place = match (grid.rows(), grid.cols()) {
            (1, 1) => Placement::default(),
            (1, _) => at(0, 0, 1, 2),
            _ => at(1, 0, 1, 2),
        };
        f.add_axes_at(place, ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .title("held")
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions::default(),
            )
            .unwrap();
        f
    }

    #[test]
    fn add_axes_errors_follow_the_table() {
        for case in table() {
            let mut f = holding_one(case.grid);
            let err = f
                .add_axes_at(case.place, ScaleKind::Linear, case.y)
                .map(drop)
                .unwrap_err();
            assert!(matches!(err, Error::Usage { .. }), "{err:?}");
            assert_eq!(
                err.to_string(),
                format!("Figure.add_axes(): {}", case.message)
            );
            assert_eq!(err.parameter(), Some(case.parameter), "{}", case.message);
            assert_eq!(err.hint(), Some(case.hint), "{}", case.message);
        }
    }

    #[test]
    fn failed_add_axes_leaves_figure_unchanged() {
        for case in table() {
            let mut f = holding_one(case.grid);
            let before = f.clone();
            let svg = f.to_svg(SvgText::Editable).unwrap();
            let result = f
                .add_axes_at(case.place, ScaleKind::Linear, case.y)
                .map(drop);
            assert!(result.is_err(), "{}", case.message);
            assert_eq!(f, before, "{}", case.message);
            assert_eq!(
                f.to_svg(SvgText::Editable).unwrap(),
                svg,
                "{}",
                case.message
            );
        }
    }

    #[test]
    fn axes_at_errors() {
        let mut f = Figure::with_grid(9.0, 6.0, Theme::pigment(), grid(2, 3)).unwrap();
        f.add_axes_at(at(0, 1, 2, 1), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        for (row, col, message, parameter, hint) in [
            (
                GridIndex::Auto,
                GridIndex::Auto,
                "row is AUTO; this figure has 2 rows, so each axes needs a row.",
                Some("row"),
                "pass row=, counting from 0.",
            ),
            (
                GridIndex::At(0),
                GridIndex::Auto,
                "col is AUTO; this figure has 3 columns, so each axes needs a col.",
                Some("col"),
                "pass col=, counting from 0.",
            ),
            (
                GridIndex::At(2),
                GridIndex::At(0),
                "row is 2; this figure has rows 0 to 1.",
                Some("row"),
                "rows and columns count from 0.",
            ),
            (
                GridIndex::At(0),
                GridIndex::At(i64::MIN),
                "col is -9223372036854775808; this figure has columns 0 to 2.",
                Some("col"),
                "rows and columns count from 0.",
            ),
            (
                GridIndex::At(1),
                GridIndex::At(2),
                "row 1, col 2 holds no axes.",
                None,
                "add one with add_axes(row=, col=) first.",
            ),
            (
                GridIndex::At(1),
                GridIndex::At(1),
                "row 1, col 1 is part of the axes added at row 0, col 1.",
                None,
                "find a spanning axes at its top-left cell.",
            ),
        ] {
            let shared = f.axes_at(row, col).map(drop).unwrap_err();
            let exclusive = f.axes_at_mut(row, col).map(drop).unwrap_err();
            for err in [shared, exclusive] {
                assert!(matches!(err, Error::Usage { .. }), "{err:?}");
                assert_eq!(err.to_string(), format!("Figure.axes_at(): {message}"));
                assert_eq!(err.parameter(), parameter, "{message}");
                assert_eq!(err.hint(), Some(hint), "{message}");
            }
        }
    }

    #[test]
    fn check_scales_names_the_first_scale_that_differs() {
        let log2 = ScaleKind::Log {
            base: LogBase::new(2.0).unwrap(),
        };
        let ten = ScaleKind::Log { base: LogBase::TEN };
        let mut f = Figure::with_grid(9.0, 6.0, Theme::pigment(), grid(2, 2)).unwrap();
        let axes = f
            .add_axes_at(at(1, 1, 1, 1), ScaleKind::Category, log2)
            .unwrap();
        assert!(axes.check_scales(ScaleKind::Category, log2).is_ok());
        for (x, y, message, parameter) in [
            (
                ScaleKind::Linear,
                ScaleKind::Linear,
                "x is Scale.linear(); the axes at row 1, col 1 has x Scale.category().",
                "x",
            ),
            (
                ScaleKind::Category,
                ten,
                "y is Scale.log(10); the axes at row 1, col 1 has y Scale.log(2).",
                "y",
            ),
            (
                ScaleKind::Category,
                ScaleKind::Category,
                "y is Scale.category(); the axes at row 1, col 1 has y Scale.log(2).",
                "y",
            ),
        ] {
            let err = axes.check_scales(x, y).unwrap_err();
            assert!(matches!(err, Error::Usage { .. }), "{err:?}");
            assert_eq!(err.to_string(), format!("Figure.axes_at(): {message}"));
            assert_eq!(err.parameter(), Some(parameter));
            assert_eq!(
                err.hint(),
                Some("pass the scales the axes was created with.")
            );
        }
    }

    #[test]
    fn panels_are_in_row_major_order_of_top_left_cells() {
        let mut f = Figure::with_grid(9.0, 6.0, Theme::pigment(), grid(3, 3)).unwrap();
        for (row, col, row_span) in [(2, 2, 1), (0, 1, 3), (1, 0, 1), (0, 0, 1), (2, 0, 1)] {
            f.add_axes_at(
                at(row, col, row_span, 1),
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap()
            .title(&format!("{row}{col}"));
        }
        let order: Vec<&str> = f.panels().map(Axes::title_text).collect();
        assert_eq!(order, ["00", "01", "10", "20", "22"]);
        assert!(f.to_scene(FontLibrary::global()).is_ok());
    }
}
