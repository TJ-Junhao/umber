//! Figures: the `Figure` class with its grid, axes, texts, limits, legend, marks and rendering,
//! and the grid helpers `grid_auto_size` and `grid_cells`.
//!
//! A figure holds a grid of cells and the axes placed on it. Every axes method takes the row
//! and column of the axes' top-left cell first; the core reports a cell that holds no axes
//! (a `UsageError`). A grid index travels as two values, `(kind, index)`: kind 0 is `AUTO`
//! and 1 is the index given. Every argument is converted before the core call, and the core
//! validates before it stores anything, so a failed call leaves the figure unchanged.
//!
//! Rendering takes a clone of the core figure, which is cheap because marks share their data, and
//! renders it with the interpreter released. No borrow of the Python object is held while
//! rendering, so other threads can keep using the figure.

// Core option structs are built with `..Default::default()` even where every field is given, so
// fields added to them later, each with a default, keep this module compiling.
#![allow(
    clippy::needless_update,
    reason = "core option structs gain fields that have defaults"
)]
// Native methods take every argument as a plain value, the axes' cell first.
#![allow(
    clippy::too_many_arguments,
    reason = "native methods take a cell, the data and every option as plain values"
)]

use crate::cmap::Cmap;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use umber_core::HeatmapOptions;
use umber_core::{
    AreaOptions, AxesMut, BandOptions, BarOptions, Dash, ErrorBarOptions, Grid, GridIndex,
    HistOptions, LegendMode, LegendPosition, LegendStyle, LineOptions, Marker, Placement,
    RuleOptions, ScatterOptions, Series, Share, SpanOptions, Step, StepOptions, SvgText,
};
use umber_core::{AxisValue, BoxOptions, Distribution, StemOptions, time_from_micros};
use umber_core::{
    CmapChoice, HexbinOptions, Hist2dOptions, MeshOptions, ScaleKind, Transform, ViolinOptions,
};

use crate::color::{ColorArg, cmap_choice, color_spec, color_transform, point_color};
use crate::enums::enum_value;
use crate::errors::{core_error, internal_value};
use crate::scale::scale_kind;
use crate::size::{SizeArg, size_spec};
use crate::theme::Theme;
use crate::ticks::{TickFormatArg, tick_format};
use crate::values::{
    axis_value, floats, is_int64_buffer, micros, names, series_data, shared_floats, times,
};

/// The series of a grouped or stacked bar chart: `(label, values, color)` for each series.
type SeriesArgs<'py> = Vec<(String, Bound<'py, PyAny>, ColorArg)>;

/// Named sample groups and their color, in distribution order.
type DistributionArgs<'py> = Vec<(String, Vec<Bound<'py, PyAny>>, ColorArg)>;

/// The `kind` of a grid index argument that is `AUTO`.
const AUTO_INDEX: i64 = 0;
/// The `kind` of a grid index argument that is the index given.
const AT_INDEX: i64 = 1;

/// Converts a grid index argument `(kind, index)` to a [`GridIndex`]. An unknown kind is an
/// internal `ValueError`.
fn grid_index(kind: i64, index: i64) -> PyResult<GridIndex> {
    match kind {
        AUTO_INDEX => Ok(GridIndex::Auto),
        AT_INDEX => Ok(GridIndex::At(index)),
        _ => Err(internal_value(format!("{kind} is not a grid index kind"))),
    }
}

/// Converts the shared numeric color scale argument of cell marks.
fn cell_colors(
    cmap: Option<PyRef<'_, Cmap>>,
    kind: i64,
    base: f64,
) -> PyResult<(CmapChoice, Transform)> {
    let transform = match scale_kind(kind, base)? {
        ScaleKind::Linear => Transform::Linear,
        ScaleKind::Log { base } => Transform::Log(base),
        _ => {
            return Err(internal_value(
                "a color mapping needs a linear or log scale",
            ));
        }
    };
    Ok((
        cmap.map_or(CmapChoice::Theme, |cmap| {
            CmapChoice::Fixed(cmap.core().clone())
        }),
        transform,
    ))
}

/// The grid of `rows × cols` cells.
fn grid(rows: i64, cols: i64) -> PyResult<Grid> {
    Grid::new(rows, cols).map_err(core_error)
}

/// The figure size in inches that fits a `rows × cols` grid when no size is given.
#[pyfunction]
pub(crate) fn grid_auto_size(rows: i64, cols: i64) -> PyResult<(f64, f64)> {
    Ok(grid(rows, cols)?.auto_size_in())
}

/// The first `count` cells of a `rows × cols` grid in row-major order, as `(row, col)`.
#[pyfunction]
pub(crate) fn grid_cells(rows: i64, cols: i64, count: i64) -> PyResult<Vec<(usize, usize)>> {
    let cells = grid(rows, cols)?.cells(count).map_err(core_error)?;
    Ok(cells.into_iter().map(|cell| (cell.row, cell.col)).collect())
}

/// A figure: a size in inches, a theme, a grid and the axes placed on it with their marks.
#[pyclass(module = "umber._umber")]
pub(crate) struct Figure(umber_core::Figure);

impl Figure {
    /// The axes whose top-left cell is at `row`, `col`; the core's `UsageError` when no axes
    /// starts there.
    fn axes(&mut self, row: i64, col: i64) -> PyResult<AxesMut<'_>> {
        self.0
            .axes_at_mut(GridIndex::At(row), GridIndex::At(col))
            .map_err(core_error)
    }

    /// A copy of the core figure, taken under a short borrow that ends before the copy is
    /// rendered.
    fn snapshot(slf: &Bound<'_, Self>) -> PyResult<umber_core::Figure> {
        Ok(slf.try_borrow()?.0.clone())
    }

    /// Renders the figure as a PNG with the interpreter released. Errors are reported in
    /// `context`.
    fn png<'py>(
        slf: &Bound<'py, Self>,
        dpi: i64,
        context: &'static str,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let figure = Self::snapshot(slf)?;
        let py = slf.py();
        let png = py
            .detach(move || figure.to_png(dpi))
            .map_err(|err| core_error(err.with_context(context)))?;
        Ok(PyBytes::new(py, &png))
    }

    /// Renders the figure as an SVG document with the interpreter released. Errors are
    /// reported in `context`.
    fn svg(slf: &Bound<'_, Self>, text: i64, context: &'static str) -> PyResult<String> {
        let text = enum_value::<SvgText>(text)?;
        let figure = Self::snapshot(slf)?;
        slf.py()
            .detach(move || figure.to_svg(text))
            .map_err(|err| core_error(err.with_context(context)))
    }

    /// The tick positions `obj` of a time axis, an int64 buffer of microseconds, named
    /// `positions` in the messages of `context`. A missing time cannot be a tick, so the core
    /// refuses it as a time out of range.
    fn tick_times(
        context: &'static str,
        obj: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<umber_core::Instant>> {
        micros(obj)?
            .into_iter()
            .map(|us| time_from_micros(context, "positions", us).map_err(core_error))
            .collect()
    }

    /// The core series of a grouped or stacked bar chart.
    fn series(series: SeriesArgs<'_>) -> PyResult<Vec<Series>> {
        series
            .into_iter()
            .map(|(label, values, color)| {
                Ok(Series {
                    label,
                    values: floats(&values)?,
                    color: color_spec(color)?,
                    ..Default::default()
                })
            })
            .collect()
    }
}

#[pymethods]
impl Figure {
    /// An empty figure of `width` by `height` inches with `theme`, on a grid of `rows × cols`
    /// cells whose x and y scales are shared as `share_x` and `share_y` say (`Share` values)
    /// and whose legend is shown as `legend` says (a `Legend` value). A missing size is the
    /// Python facade's `AUTO`; explicit sizes are checked before the grid dimensions.
    #[new]
    #[pyo3(signature = (width, height, theme, rows, cols, share_x, share_y, legend))]
    fn new(
        width: Option<f64>,
        height: Option<f64>,
        theme: &Theme,
        rows: i64,
        cols: i64,
        share_x: i64,
        share_y: i64,
        legend: i64,
    ) -> PyResult<Figure> {
        // Use the core's size checks before asking it to validate rows and columns. AUTO
        // sides use a valid single-cell size for this check, not a Python-side size rule.
        let (single_width, single_height) = grid_auto_size(1, 1)?;
        umber_core::Figure::new(
            width.unwrap_or(single_width),
            height.unwrap_or(single_height),
            theme.core().clone(),
        )
        .map_err(core_error)?;
        let (auto_width, auto_height) = grid_auto_size(rows, cols)?;
        let (share_x, share_y) = (enum_value::<Share>(share_x)?, enum_value::<Share>(share_y)?);
        let legend = enum_value::<LegendMode>(legend)?;
        let grid = grid(rows, cols)?
            .with_share_x(share_x)
            .with_share_y(share_y)
            .with_legend(legend);
        umber_core::Figure::with_grid(
            width.unwrap_or(auto_width),
            height.unwrap_or(auto_height),
            theme.core().clone(),
            grid,
        )
        .map(Figure)
        .map_err(core_error)
    }

    /// An independent copy of this figure. Marks share their data, which never changes, so the
    /// copy is cheap.
    #[pyo3(name = "clone")]
    fn clone_figure(&self) -> Figure {
        Figure(self.0.clone())
    }

    /// Adds axes at the row `(row_kind, row)` and the column `(col_kind, col)`, spanning
    /// `row_span × col_span` cells, with the x scale `(x_kind, x_base)` and the y scale
    /// `(y_kind, y_base)`, and returns the `(row, col)` of their top-left cell. The core checks
    /// the position and the scales, and a refused call adds nothing. A time scale is allowed
    /// on either axis.
    fn add_axes(
        &mut self,
        row_kind: i64,
        row: i64,
        col_kind: i64,
        col: i64,
        row_span: i64,
        col_span: i64,
        x_kind: i64,
        x_base: f64,
        y_kind: i64,
        y_base: f64,
    ) -> PyResult<(usize, usize)> {
        let at = Placement {
            row: grid_index(row_kind, row)?,
            col: grid_index(col_kind, col)?,
            row_span,
            col_span,
        };
        let x = scale_kind(x_kind, x_base)?;
        let y = scale_kind(y_kind, y_base)?;
        let axes = self.0.add_axes_at(at, x, y).map_err(core_error)?;
        Ok((axes.cells().row(), axes.cells().col()))
    }

    /// The `(row, col)` of the top-left cell of the axes found at the row `(row_kind, row)`
    /// and the column `(col_kind, col)`, after the core has checked that they were created
    /// with the x scale `(x_kind, x_base)` and the y scale `(y_kind, y_base)`.
    fn axes_at(
        &self,
        row_kind: i64,
        row: i64,
        col_kind: i64,
        col: i64,
        x_kind: i64,
        x_base: f64,
        y_kind: i64,
        y_base: f64,
    ) -> PyResult<(usize, usize)> {
        let (row, col) = (grid_index(row_kind, row)?, grid_index(col_kind, col)?);
        let x = scale_kind(x_kind, x_base)?;
        let y = scale_kind(y_kind, y_base)?;
        let axes = self.0.axes_at(row, col).map_err(core_error)?;
        axes.check_scales(x, y).map_err(core_error)?;
        Ok((axes.cells().row(), axes.cells().col()))
    }

    /// Sets the figure title above every panel.
    fn figure_title(&mut self, text: &str) {
        self.0.title(text);
    }

    /// Sets the figure subtitle below its title.
    fn figure_subtitle(&mut self, text: &str) {
        self.0.subtitle(text);
    }

    /// Sets the title.
    fn title(&mut self, row: i64, col: i64, text: &str) -> PyResult<()> {
        self.axes(row, col)?.title(text);
        Ok(())
    }

    /// Sets the subtitle.
    fn subtitle(&mut self, row: i64, col: i64, text: &str) -> PyResult<()> {
        self.axes(row, col)?.subtitle(text);
        Ok(())
    }

    /// Sets the x axis label.
    fn xlabel(&mut self, row: i64, col: i64, text: &str) -> PyResult<()> {
        self.axes(row, col)?.xlabel(text);
        Ok(())
    }

    /// Sets the y axis label.
    fn ylabel(&mut self, row: i64, col: i64, text: &str) -> PyResult<()> {
        self.axes(row, col)?.ylabel(text);
        Ok(())
    }

    /// Sets the x limits.
    fn xlim(&mut self, row: i64, col: i64, low: f64, high: f64) -> PyResult<()> {
        self.axes(row, col)?
            .xlim(low, high)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the y limits.
    fn ylim(&mut self, row: i64, col: i64, low: f64, high: f64) -> PyResult<()> {
        self.axes(row, col)?
            .ylim(low, high)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the x ticks with ticks at `positions`.
    fn xticks(&mut self, row: i64, col: i64, positions: &Bound<'_, PyAny>) -> PyResult<()> {
        let positions = floats(positions)?;
        self.axes(row, col)?
            .xticks(&positions)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the y ticks with ticks at `positions`.
    fn yticks(&mut self, row: i64, col: i64, positions: &Bound<'_, PyAny>) -> PyResult<()> {
        let positions = floats(positions)?;
        self.axes(row, col)?
            .yticks(&positions)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the x ticks with ticks at `positions`, each with the label at its index.
    fn xticks_labeled(
        &mut self,
        row: i64,
        col: i64,
        positions: &Bound<'_, PyAny>,
        labels: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let (positions, labels) = (floats(positions)?, names(labels)?);
        self.axes(row, col)?
            .xticks_labeled(&positions, labels)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the y ticks with ticks at `positions`, each with the label at its index.
    fn yticks_labeled(
        &mut self,
        row: i64,
        col: i64,
        positions: &Bound<'_, PyAny>,
        labels: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let (positions, labels) = (floats(positions)?, names(labels)?);
        self.axes(row, col)?
            .yticks_labeled(&positions, labels)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the format of the x tick labels.
    fn xtick_format(&mut self, row: i64, col: i64, format: TickFormatArg) -> PyResult<()> {
        let format = tick_format(format)?;
        self.axes(row, col)?
            .xtick_format(format)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the format of the y tick labels.
    fn ytick_format(&mut self, row: i64, col: i64, format: TickFormatArg) -> PyResult<()> {
        let format = tick_format(format)?;
        self.axes(row, col)?
            .ytick_format(format)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the legend style (a `LegendStyle` value).
    fn legend(&mut self, row: i64, col: i64, style: i64) -> PyResult<()> {
        let style = enum_value::<LegendStyle>(style)?;
        self.axes(row, col)?.legend(style);
        Ok(())
    }

    /// Sets the legend style and position (`LegendStyle` and `LegendPosition` values).
    fn legend_at(&mut self, row: i64, col: i64, style: i64, position: i64) -> PyResult<()> {
        let style = enum_value::<LegendStyle>(style)?;
        let position = enum_value::<LegendPosition>(position)?;
        self.axes(row, col)?
            .legend_at(style, position)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a line.
    fn line(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.line()";
        let (x, y) = (series_data(CONTEXT, "x", x)?, series_data(CONTEXT, "y", y)?);
        let options = LineOptions {
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .line_shared(x, y, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds an area: a line with the region down to the baseline filled.
    fn area(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.area()";
        let (x, y) = (series_data(CONTEXT, "x", x)?, series_data(CONTEXT, "y", y)?);
        let options = AreaOptions {
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .area_shared(x, y, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a scatter series; `x` and `y` each hold numbers, or names on a category axis.
    fn scatter(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        label: String,
        color: &Bound<'_, PyAny>,
        marker: i64,
        size: SizeArg,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.scatter()";
        let (x, y) = (series_data(CONTEXT, "x", x)?, series_data(CONTEXT, "y", y)?);
        let options = ScatterOptions {
            label,
            color: point_color(color)?,
            marker: enum_value::<Marker>(marker)?,
            size: size_spec(size)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .scatter_shared(x, y, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds bubbles sharing an area scale on these axes.
    fn bubbles(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        sizes: &Bound<'_, PyAny>,
        label: String,
        color: &Bound<'_, PyAny>,
        marker: i64,
    ) -> PyResult<()> {
        let (x, y, sizes) = (shared_floats(x)?, shared_floats(y)?, shared_floats(sizes)?);
        let options = umber_core::BubbleOptions {
            label,
            color: point_color(color)?,
            marker: enum_value::<Marker>(marker)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .bubbles_shared(x, y, sizes, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds stems from the baseline with a marker at each value.
    fn stem(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
        marker: i64,
        size: SizeArg,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.stem()";
        let (x, y) = (series_data(CONTEXT, "x", x)?, series_data(CONTEXT, "y", y)?);
        let options = StemOptions {
            label,
            color: color_spec(color)?,
            marker: enum_value::<Marker>(marker)?,
            size: size_spec(size)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .stem_shared(x, y, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds bars, one per category.
    fn bar(
        &mut self,
        row: i64,
        col: i64,
        categories: &Bound<'_, PyAny>,
        heights: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        let (categories, heights) = (names(categories)?, shared_floats(heights)?);
        let options = BarOptions {
            label,
            color: color_spec(color)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .bar_shared(categories, heights, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds grouped bars: in each category, one bar per series.
    fn grouped_bar(
        &mut self,
        row: i64,
        col: i64,
        categories: &Bound<'_, PyAny>,
        series: SeriesArgs<'_>,
    ) -> PyResult<()> {
        let (categories, series) = (names(categories)?, Self::series(series)?);
        self.axes(row, col)?
            .grouped_bar(categories, series)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a box plot for each category.
    fn box_plot(
        &mut self,
        row: i64,
        col: i64,
        categories: &Bound<'_, PyAny>,
        groups: Vec<Bound<'_, PyAny>>,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        let categories = names(categories)?;
        let groups = groups.iter().map(floats).collect::<PyResult<Vec<_>>>()?;
        let options = BoxOptions {
            label,
            color: color_spec(color)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .box_plot(categories, groups, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds distributions side by side in each category.
    fn grouped_box_plot(
        &mut self,
        row: i64,
        col: i64,
        categories: &Bound<'_, PyAny>,
        distributions: DistributionArgs<'_>,
    ) -> PyResult<()> {
        let categories = names(categories)?;
        let distributions = distributions
            .into_iter()
            .map(|(label, groups, color)| {
                Ok(Distribution {
                    label,
                    groups: groups.iter().map(floats).collect::<PyResult<Vec<_>>>()?,
                    color: color_spec(color)?,
                    ..Default::default()
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        self.axes(row, col)?
            .grouped_box_plot(categories, distributions)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a violin for each category.
    fn violin(
        &mut self,
        row: i64,
        col: i64,
        categories: &Bound<'_, PyAny>,
        groups: Vec<Bound<'_, PyAny>>,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        let categories = names(categories)?;
        let groups = groups.iter().map(floats).collect::<PyResult<Vec<_>>>()?;
        let options = ViolinOptions {
            label,
            color: color_spec(color)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .violin(categories, groups, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds distributions side by side in each category.
    fn grouped_violin(
        &mut self,
        row: i64,
        col: i64,
        categories: &Bound<'_, PyAny>,
        distributions: DistributionArgs<'_>,
    ) -> PyResult<()> {
        let categories = names(categories)?;
        let distributions = distributions
            .into_iter()
            .map(|(label, groups, color)| {
                Ok(Distribution {
                    label,
                    groups: groups.iter().map(floats).collect::<PyResult<Vec<_>>>()?,
                    color: color_spec(color)?,
                    ..Default::default()
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        self.axes(row, col)?
            .grouped_violin(categories, distributions)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds stacked bars: in each category, one stack of the series.
    fn stacked_bar(
        &mut self,
        row: i64,
        col: i64,
        categories: &Bound<'_, PyAny>,
        series: SeriesArgs<'_>,
    ) -> PyResult<()> {
        let (categories, series) = (names(categories)?, Self::series(series)?);
        self.axes(row, col)?
            .stacked_bar(categories, series)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds non-negative area series stacked over shared x values.
    fn stacked_area(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        series: SeriesArgs<'_>,
    ) -> PyResult<()> {
        let (x, series) = (shared_floats(x)?, Self::series(series)?);
        self.axes(row, col)?
            .stacked_area_shared(x, series)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a histogram of `values` in `bins` bins.
    fn hist(
        &mut self,
        row: i64,
        col: i64,
        values: &Bound<'_, PyAny>,
        bins: i64,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        let values = shared_floats(values)?;
        let options = HistOptions {
            bins,
            label,
            color: color_spec(color)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .hist_shared(values, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds rectangular cells with flattened row-major color values.
    #[pyo3(signature = (row, col, x_edges, y_edges, values, cmap, scale, base, center))]
    fn mesh(
        &mut self,
        row: i64,
        col: i64,
        x_edges: &Bound<'_, PyAny>,
        y_edges: &Bound<'_, PyAny>,
        values: &Bound<'_, PyAny>,
        cmap: Option<PyRef<'_, Cmap>>,
        scale: i64,
        base: f64,
        center: Option<f64>,
    ) -> PyResult<()> {
        let (x, y, values) = (floats(x_edges)?, floats(y_edges)?, floats(values)?);
        let (cmap, scale) = cell_colors(cmap, scale, base)?;
        self.axes(row, col)?
            .mesh(
                &x,
                &y,
                &values,
                MeshOptions {
                    cmap,
                    scale,
                    center,
                    ..Default::default()
                },
            )
            .map(drop)
            .map_err(core_error)
    }

    /// Counts paired coordinates in rectangular bins.
    #[pyo3(signature = (row, col, x, y, bins, cmap, scale, base))]
    fn hist2d(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        bins: (usize, usize),
        cmap: Option<PyRef<'_, Cmap>>,
        scale: i64,
        base: f64,
    ) -> PyResult<()> {
        let (x, y) = (floats(x)?, floats(y)?);
        let (cmap, scale) = cell_colors(cmap, scale, base)?;
        self.axes(row, col)?
            .hist2d(
                &x,
                &y,
                Hist2dOptions {
                    bins,
                    cmap,
                    scale,
                    ..Default::default()
                },
            )
            .map(drop)
            .map_err(core_error)
    }

    /// Counts paired coordinates in regular pointy-top hexagons.
    #[pyo3(signature = (row, col, x, y, gridsize, cmap, scale, base))]
    fn hexbin(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        gridsize: usize,
        cmap: Option<PyRef<'_, Cmap>>,
        scale: i64,
        base: f64,
    ) -> PyResult<()> {
        let (x, y) = (floats(x)?, floats(y)?);
        let (cmap, scale) = cell_colors(cmap, scale, base)?;
        self.axes(row, col)?
            .hexbin(
                &x,
                &y,
                HexbinOptions {
                    gridsize,
                    cmap,
                    scale,
                    ..Default::default()
                },
            )
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a step line; `position` is a `Step` value.
    fn step(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        position: i64,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.step()";
        let (x, y) = (series_data(CONTEXT, "x", x)?, series_data(CONTEXT, "y", y)?);
        let options = StepOptions {
            position: enum_value::<Step>(position)?,
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
        };
        self.axes(row, col)?
            .step_shared(x, y, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a horizontal reference line at `y`.
    fn hline(
        &mut self,
        row: i64,
        col: i64,
        y: f64,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let options = RuleOptions {
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
        };
        self.axes(row, col)?
            .hline(y, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a vertical reference line at `x`, a `float` or, on a category x axis, a `str`.
    fn vline(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let x = axis_value(x)?;
        let options = RuleOptions {
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
        };
        self.axes(row, col)?
            .vline(x, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a horizontal reference span between the y values `low` and `high`.
    fn hspan(
        &mut self,
        row: i64,
        col: i64,
        low: f64,
        high: f64,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        let options = SpanOptions {
            label,
            color: color_spec(color)?,
        };
        self.axes(row, col)?
            .hspan(low, high, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a vertical reference span between the x values `low` and `high`.
    fn vspan(
        &mut self,
        row: i64,
        col: i64,
        low: f64,
        high: f64,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        let options = SpanOptions {
            label,
            color: color_spec(color)?,
        };
        self.axes(row, col)?
            .vspan(low, high, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a band between the curves `low` and `high` over `x`.
    fn band(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        low: &Bound<'_, PyAny>,
        high: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        let (x, low, high) = (shared_floats(x)?, shared_floats(low)?, shared_floats(high)?);
        let options = BandOptions {
            label,
            color: color_spec(color)?,
        };
        self.axes(row, col)?
            .band_shared(x, low, high, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds vertical error bars at `x` between the limits `low` and `high`.
    fn y_error_bars(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        low: &Bound<'_, PyAny>,
        high: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
        width: SizeArg,
        caps: bool,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.y_error_bars()";
        let x = series_data(CONTEXT, "x", x)?;
        let options = ErrorBarOptions {
            label,
            color: color_spec(color)?,
            width: size_spec(width)?,
            caps,
        };
        // Limits in an int64 buffer are times, of a time y axis.
        if is_int64_buffer(low) {
            let (low, high) = (times(CONTEXT, "low", low)?, times(CONTEXT, "high", high)?);
            return self
                .axes(row, col)?
                .y_error_bars_time(x, low.into(), high.into(), options)
                .map(drop)
                .map_err(core_error);
        }
        let (low, high) = (shared_floats(low)?, shared_floats(high)?);
        self.axes(row, col)?
            .y_error_bars_shared(x, low, high, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds horizontal error bars at `y`, numbers or names on a category y axis, between the
    /// limits `low` and `high`.
    fn x_error_bars(
        &mut self,
        row: i64,
        col: i64,
        y: &Bound<'_, PyAny>,
        low: &Bound<'_, PyAny>,
        high: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
        width: SizeArg,
        caps: bool,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.x_error_bars()";
        let y = series_data(CONTEXT, "y", y)?;
        let options = ErrorBarOptions {
            label,
            color: color_spec(color)?,
            width: size_spec(width)?,
            caps,
        };
        // Limits in an int64 buffer are times, of a time x axis.
        if is_int64_buffer(low) {
            let (low, high) = (times(CONTEXT, "low", low)?, times(CONTEXT, "high", high)?);
            return self
                .axes(row, col)?
                .x_error_bars_time(y, low.into(), high.into(), options)
                .map(drop)
                .map_err(core_error);
        }
        let (low, high) = (shared_floats(low)?, shared_floats(high)?);
        self.axes(row, col)?
            .x_error_bars_shared(y, low, high, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Renders the figure as a PNG at `dpi`.
    fn to_png<'py>(slf: &Bound<'py, Self>, dpi: i64) -> PyResult<Bound<'py, PyBytes>> {
        Self::png(slf, dpi, "Figure.to_png()")
    }

    /// Renders the figure as an SVG document, with text as `text` (an `SvgText` value).
    fn to_svg(slf: &Bound<'_, Self>, text: i64) -> PyResult<String> {
        Self::svg(slf, text, "Figure.to_svg()")
    }

    /// Renders the figure as a PNG at `dpi` for `Figure.save_png()`, which reports its errors.
    fn png_for_file<'py>(slf: &Bound<'py, Self>, dpi: i64) -> PyResult<Bound<'py, PyBytes>> {
        Self::png(slf, dpi, "Figure.save_png()")
    }

    /// Renders the figure as an SVG document for `Figure.save_svg()`, which reports its errors.
    fn svg_for_file(slf: &Bound<'_, Self>, text: i64) -> PyResult<String> {
        Self::svg(slf, text, "Figure.save_svg()")
    }

    /// Replaces the axes' colorbar label and optional overrides.
    #[pyo3(signature = (row, col, label, position, format))]
    fn colorbar(
        &mut self,
        row: i64,
        col: i64,
        label: String,
        position: Option<i64>,
        format: Option<TickFormatArg>,
    ) -> PyResult<()> {
        let options = umber_core::ColorbarOptions {
            label,
            position: position
                .map(enum_value::<umber_core::ColorbarPosition>)
                .transpose()?,
            format: format.map(tick_format).transpose()?,
        };
        self.axes(row, col)?
            .colorbar(options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a heatmap: `values` holds `len(y_categories) * len(x_categories)` numbers, row by
    /// row, one row per y category; the colormap (`None` for the theme's), the linear or log
    /// scale argument `(scale, base)`, the center and the format (`None` for the y tick format)
    /// are converted as those of a `ColorBy` and of `colorbar`.
    #[pyo3(signature = (row, col, x_categories, y_categories, values, cmap, scale, base, center, annotate, format))]
    fn heatmap(
        &mut self,
        row: i64,
        col: i64,
        x_categories: &Bound<'_, PyAny>,
        y_categories: &Bound<'_, PyAny>,
        values: &Bound<'_, PyAny>,
        cmap: Option<PyRef<'_, Cmap>>,
        scale: i64,
        base: f64,
        center: Option<f64>,
        annotate: bool,
        format: Option<TickFormatArg>,
    ) -> PyResult<()> {
        let options = HeatmapOptions {
            cmap: cmap_choice(cmap),
            scale: color_transform(scale, base)?,
            center,
            annotate,
            format: format.map(tick_format).transpose()?,
        };
        let (x, y) = (names(x_categories)?, names(y_categories)?);
        self.axes(row, col)?
            .heatmap_shared(x, y, shared_floats(values)?, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the color range of the axes' color-mapped marks.
    fn clim(&mut self, row: i64, col: i64, low: f64, high: f64) -> PyResult<()> {
        self.axes(row, col)?
            .clim(low, high)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the limits of a time x axis to the times `low` and `high`, in microseconds.
    fn xlim_time(&mut self, row: i64, col: i64, low: i64, high: i64) -> PyResult<()> {
        const CONTEXT: &str = "Axes.xlim()";
        let low = time_from_micros(CONTEXT, "low", low).map_err(core_error)?;
        let high = time_from_micros(CONTEXT, "high", high).map_err(core_error)?;
        self.axes(row, col)?
            .xlim_time(low, high)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the limits of a time y axis to the times `low` and `high`, in microseconds.
    fn ylim_time(&mut self, row: i64, col: i64, low: i64, high: i64) -> PyResult<()> {
        const CONTEXT: &str = "Axes.ylim()";
        let low = time_from_micros(CONTEXT, "low", low).map_err(core_error)?;
        let high = time_from_micros(CONTEXT, "high", high).map_err(core_error)?;
        self.axes(row, col)?
            .ylim_time(low, high)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the ticks of a time x axis with ticks at the times `positions`.
    fn xticks_time(&mut self, row: i64, col: i64, positions: &Bound<'_, PyAny>) -> PyResult<()> {
        let positions = Self::tick_times("Axes.xticks()", positions)?;
        self.axes(row, col)?
            .xticks_time(&positions)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the ticks of a time y axis with ticks at the times `positions`.
    fn yticks_time(&mut self, row: i64, col: i64, positions: &Bound<'_, PyAny>) -> PyResult<()> {
        let positions = Self::tick_times("Axes.yticks()", positions)?;
        self.axes(row, col)?
            .yticks_time(&positions)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the ticks of a time x axis with ticks at the times `positions`, each with the
    /// label at its index.
    fn xticks_labeled_time(
        &mut self,
        row: i64,
        col: i64,
        positions: &Bound<'_, PyAny>,
        labels: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let positions = Self::tick_times("Axes.xticks_labeled()", positions)?;
        let labels = names(labels)?;
        self.axes(row, col)?
            .xticks_labeled_time(&positions, labels)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the ticks of a time y axis with ticks at the times `positions`, each with the
    /// label at its index.
    fn yticks_labeled_time(
        &mut self,
        row: i64,
        col: i64,
        positions: &Bound<'_, PyAny>,
        labels: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let positions = Self::tick_times("Axes.yticks_labeled()", positions)?;
        let labels = names(labels)?;
        self.axes(row, col)?
            .yticks_labeled_time(&positions, labels)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a vertical reference line at the time `x`, in microseconds.
    fn vline_time(
        &mut self,
        row: i64,
        col: i64,
        x: i64,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let x = time_from_micros("Axes.vline()", "x", x).map_err(core_error)?;
        let options = RuleOptions {
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
        };
        self.axes(row, col)?
            .vline(AxisValue::Time(x), options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a horizontal reference line at the time `y`, in microseconds.
    fn hline_time(
        &mut self,
        row: i64,
        col: i64,
        y: i64,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let y = time_from_micros("Axes.hline()", "y", y).map_err(core_error)?;
        let options = RuleOptions {
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
        };
        self.axes(row, col)?
            .hline_time(y, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a horizontal reference span between the times `low` and `high`, in microseconds.
    fn hspan_time(
        &mut self,
        row: i64,
        col: i64,
        low: i64,
        high: i64,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.hspan()";
        let low = time_from_micros(CONTEXT, "low", low).map_err(core_error)?;
        let high = time_from_micros(CONTEXT, "high", high).map_err(core_error)?;
        let options = SpanOptions {
            label,
            color: color_spec(color)?,
        };
        self.axes(row, col)?
            .hspan_time(low, high, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a vertical reference span between the times `low` and `high`, in microseconds.
    fn vspan_time(
        &mut self,
        row: i64,
        col: i64,
        low: i64,
        high: i64,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.vspan()";
        let low = time_from_micros(CONTEXT, "low", low).map_err(core_error)?;
        let high = time_from_micros(CONTEXT, "high", high).map_err(core_error)?;
        let options = SpanOptions {
            label,
            color: color_spec(color)?,
        };
        self.axes(row, col)?
            .vspan_time(low, high, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a band between the curves `low` and `high` over the times `x`.
    fn band_time(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        low: &Bound<'_, PyAny>,
        high: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        let x = times("Axes.band()", "x", x)?;
        let (low, high) = (shared_floats(low)?, shared_floats(high)?);
        let options = BandOptions {
            label,
            color: color_spec(color)?,
        };
        self.axes(row, col)?
            .band_time(x.into(), low, high, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds non-negative area series stacked over the times `x`.
    fn stacked_area_time(
        &mut self,
        row: i64,
        col: i64,
        x: &Bound<'_, PyAny>,
        series: SeriesArgs<'_>,
    ) -> PyResult<()> {
        let (x, series) = (times("Axes.stacked_area()", "x", x)?, Self::series(series)?);
        self.axes(row, col)?
            .stacked_area_time(x.into(), series)
            .map(drop)
            .map_err(core_error)
    }
}
