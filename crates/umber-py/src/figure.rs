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
use pyo3::types::{PyBytes, PyInt};
use umber_core::HeatmapOptions;
use umber_core::{Align, AnnotateOptions, TextOptions, ValueLabels};
use umber_core::{
    AreaOptions, AxesMut, BandOptions, BarOptions, Dash, ErrorBarOptions, Grid, GridIndex,
    HistOptions, LegendMode, LegendPosition, LegendStyle, LineOptions, Marker, Placement,
    RuleOptions, ScatterOptions, Series, Share, SpanOptions, Step, StepOptions, SvgText,
};
use umber_core::{AxisValue, BoxOptions, Distribution, StemOptions, time_from_micros};
use umber_core::{
    Bandwidth, Bins, EcdfOptions, GroupedHistOptions, GroupedKdeOptions, HistStat, HistStyle,
    KdeOptions,
};
use umber_core::{
    CmapChoice, HexbinOptions, Hist2dOptions, MeshOptions, ScaleKind, Transform, ViolinOptions,
};
use umber_core::{ContourLevels, ContourOptions, ContourStyle};

use crate::color::{ColorArg, cmap_choice, color_spec, color_transform, point_color};
use crate::enums::{enum_value, render_choice};
use crate::errors::{core_error, group_error, internal_value};
use crate::scale::scale_kind;
use crate::size::{SizeArg, size_spec};
use crate::theme::Theme;
use crate::ticks::{TickFormatArg, tick_format};
use crate::values::{
    axis_value, coordinate, floats, is_int64_buffer, micros, names, series_data, shared_floats,
    times,
};

/// The series of a grouped or stacked bar chart: `(label, values, color)` for each series.
type SeriesArgs<'py> = Vec<(String, Bound<'py, PyAny>, ColorArg)>;

/// Named sample groups and their color, in distribution order.
type DistributionArgs<'py> = Vec<(String, Vec<Bound<'py, PyAny>>, ColorArg)>;

/// Named coordinate pairs, already converted by the Python facade.
type XyGroupArgs<'py> = Vec<(String, Bound<'py, PyAny>, Bound<'py, PyAny>)>;

/// Copies grouped coordinates into the forms held by the core.
fn xy_groups(
    context: &'static str,
    groups: XyGroupArgs<'_>,
) -> PyResult<Vec<(String, umber_core::AxisData, umber_core::AxisData)>> {
    groups
        .into_iter()
        .enumerate()
        .map(|(i, (label, x, y))| {
            Ok((
                label,
                series_data(context, "x", &x).map_err(|error| group_error(error, i))?,
                series_data(context, "y", &y).map_err(|error| group_error(error, i))?,
            ))
        })
        .collect()
}

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

/// The bins of a histogram: an int is a bin count, a float64 buffer the edges.
fn bins_arg(bins: &Bound<'_, PyAny>) -> PyResult<Bins> {
    if bins.is_instance_of::<PyInt>() {
        return Ok(Bins::Count(bins.extract()?));
    }
    Ok(Bins::Edges(floats(bins)?))
}

/// The values written on bars: `annotate`, and a `format` of `None` for the Python facade's
/// `AUTO`, the value axis's tick format.
fn value_labels(
    annotate: bool,
    format: Option<TickFormatArg>,
    size: SizeArg,
) -> PyResult<ValueLabels> {
    Ok(ValueLabels {
        annotate,
        format: format.map(tick_format).transpose()?,
        size: size_spec(size)?,
    })
}

/// The bandwidth of a kernel density: `None` is the Python facade's `AUTO`, Scott's rule.
fn bandwidth_arg(bandwidth: Option<f64>) -> Bandwidth {
    bandwidth.map_or(Bandwidth::Scott, Bandwidth::Fixed)
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

/// An explicit handle to the right-hand y axis at a grid row.
#[derive(Clone, Copy)]
#[pyclass(frozen, from_py_object, module = "umber._umber")]
pub(crate) struct TwinAxes {
    row: i64,
}

/// The original axes' row or an explicit right-hand axes handle.
#[derive(Clone, Copy, FromPyObject)]
pub(crate) enum AxesRow {
    Original(i64),
    Twin(TwinAxes),
}

/// A figure: a size in inches, a theme, a grid and the axes placed on it with their marks.
#[pyclass(module = "umber._umber")]
pub(crate) struct Figure(umber_core::Figure);

impl Figure {
    /// The axes whose top-left cell is at `row`, `col`; the core's `UsageError` when no axes
    /// starts there.
    fn axes(&mut self, row: AxesRow, col: i64) -> PyResult<AxesMut<'_>> {
        match row {
            AxesRow::Original(row) => self.0.axes_at_mut(GridIndex::At(row), GridIndex::At(col)),
            AxesRow::Twin(twin) => self
                .0
                .twin_axes_at_mut(GridIndex::At(twin.row), GridIndex::At(col)),
        }
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
    #[pyo3(signature = (width, height, theme, rows, cols, share_x, share_y, legend, share_color))]
    fn new(
        width: Option<f64>,
        height: Option<f64>,
        theme: &Theme,
        rows: i64,
        cols: i64,
        share_x: i64,
        share_y: i64,
        legend: i64,
        share_color: i64,
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
            .with_share_color(enum_value::<Share>(share_color)?)
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

    /// Adds a circular panel without Cartesian axes.
    fn add_pie_axes(
        &mut self,
        row_kind: i64,
        row: i64,
        col_kind: i64,
        col: i64,
        row_span: i64,
        col_span: i64,
    ) -> PyResult<(usize, usize)> {
        let at = Placement {
            row: grid_index(row_kind, row)?,
            col: grid_index(col_kind, col)?,
            row_span,
            col_span,
        };
        let axes = self.0.add_pie_axes_at(at).map_err(core_error)?;
        Ok((axes.cells().row(), axes.cells().col()))
    }

    /// Finds a circular panel at its top-left cell.
    fn pie_axes_at(
        &mut self,
        row_kind: i64,
        row: i64,
        col_kind: i64,
        col: i64,
    ) -> PyResult<(usize, usize)> {
        let (row, col) = (grid_index(row_kind, row)?, grid_index(col_kind, col)?);
        let axes = self.0.pie_axes_at_mut(row, col).map_err(core_error)?;
        Ok((axes.cells().row(), axes.cells().col()))
    }

    /// Adds the circular panel's single pie from copied data.
    fn pie(
        &mut self,
        row: i64,
        col: i64,
        categories: &Bound<'_, PyAny>,
        values: &Bound<'_, PyAny>,
        hole: f64,
        annotate: bool,
        format: Option<TickFormatArg>,
        colors: Option<Vec<ColorArg>>,
        size: SizeArg,
    ) -> PyResult<()> {
        let categories = names(categories)?;
        let values = floats(values)?;
        let options = umber_core::PieOptions {
            hole,
            annotate,
            format: format.map(tick_format).transpose()?,
            colors: colors
                .map(|colors| {
                    colors
                        .into_iter()
                        .map(color_spec)
                        .collect::<PyResult<Vec<_>>>()
                })
                .transpose()?,
            size: size_spec(size)?,
        };
        self.0
            .pie_axes_at_mut(GridIndex::At(row), GridIndex::At(col))
            .map_err(core_error)?
            .pie(categories, &values, options)
            .map_err(core_error)?;
        Ok(())
    }

    /// Sets a circular panel's title.
    fn pie_title(&mut self, row: i64, col: i64, text: &str, size: SizeArg) -> PyResult<()> {
        let size = size_spec(size)?;
        self.0
            .pie_axes_at_mut(GridIndex::At(row), GridIndex::At(col))
            .map_err(core_error)?
            .title_sized(text, size);
        Ok(())
    }

    /// Sets a circular panel's own legend.
    fn pie_legend(
        &mut self,
        row: i64,
        col: i64,
        style: i64,
        position: Option<i64>,
        size: SizeArg,
    ) -> PyResult<()> {
        let style = enum_value::<LegendStyle>(style)?;
        let position = position.map(enum_value::<LegendPosition>).transpose()?;
        let size = size_spec(size)?;
        self.0
            .pie_axes_at_mut(GridIndex::At(row), GridIndex::At(col))
            .map_err(core_error)?
            .legend(style, position, size)
            .map_err(core_error)?;
        Ok(())
    }

    /// Sets the figure title above every panel.
    fn figure_title(&mut self, text: &str, size: SizeArg) -> PyResult<()> {
        self.0.title_sized(text, size_spec(size)?);
        Ok(())
    }

    /// Sets the figure subtitle below its title.
    fn figure_subtitle(&mut self, text: &str, size: SizeArg) -> PyResult<()> {
        self.0.subtitle_sized(text, size_spec(size)?);
        Ok(())
    }

    /// Adds the numeric right-hand y axes without occupying another grid cell.
    fn twin_y(&mut self, row: AxesRow, col: i64, kind: i64, base: f64) -> PyResult<TwinAxes> {
        let scale = scale_kind(kind, base)?;
        self.axes(row, col)?
            .twin_y(scale)
            .map(drop)
            .map_err(core_error)?;
        let row = match row {
            AxesRow::Original(row) => row,
            AxesRow::Twin(twin) => twin.row,
        };
        Ok(TwinAxes { row })
    }

    /// Sets the title.
    fn title(&mut self, row: AxesRow, col: i64, text: &str, size: SizeArg) -> PyResult<()> {
        let size = size_spec(size)?;
        self.axes(row, col)?.title_sized(text, size);
        Ok(())
    }

    /// Sets the subtitle.
    fn subtitle(&mut self, row: AxesRow, col: i64, text: &str, size: SizeArg) -> PyResult<()> {
        let size = size_spec(size)?;
        self.axes(row, col)?.subtitle_sized(text, size);
        Ok(())
    }

    /// Sets the x axis label.
    fn xlabel(&mut self, row: AxesRow, col: i64, text: &str, size: SizeArg) -> PyResult<()> {
        let size = size_spec(size)?;
        self.axes(row, col)?.xlabel_sized(text, size);
        Ok(())
    }

    /// Sets the y axis label.
    fn ylabel(&mut self, row: AxesRow, col: i64, text: &str, size: SizeArg) -> PyResult<()> {
        let size = size_spec(size)?;
        self.axes(row, col)?.ylabel_sized(text, size);
        Ok(())
    }

    /// Sets the x limits.
    fn xlim(&mut self, row: AxesRow, col: i64, low: f64, high: f64) -> PyResult<()> {
        self.axes(row, col)?
            .xlim(low, high)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the y limits.
    fn ylim(&mut self, row: AxesRow, col: i64, low: f64, high: f64) -> PyResult<()> {
        self.axes(row, col)?
            .ylim(low, high)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the x ticks with ticks at `positions`.
    fn xticks(&mut self, row: AxesRow, col: i64, positions: &Bound<'_, PyAny>) -> PyResult<()> {
        let positions = floats(positions)?;
        self.axes(row, col)?
            .xticks(&positions)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the y ticks with ticks at `positions`.
    fn yticks(&mut self, row: AxesRow, col: i64, positions: &Bound<'_, PyAny>) -> PyResult<()> {
        let positions = floats(positions)?;
        self.axes(row, col)?
            .yticks(&positions)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the x ticks with ticks at `positions`, each with the label at its index.
    fn xticks_labeled(
        &mut self,
        row: AxesRow,
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
        row: AxesRow,
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
    fn xtick_format(&mut self, row: AxesRow, col: i64, format: TickFormatArg) -> PyResult<()> {
        let format = tick_format(format)?;
        self.axes(row, col)?
            .xtick_format(format)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the format of the y tick labels.
    fn ytick_format(&mut self, row: AxesRow, col: i64, format: TickFormatArg) -> PyResult<()> {
        let format = tick_format(format)?;
        self.axes(row, col)?
            .ytick_format(format)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets baseline visibility: `None` follows the marks, otherwise the given boolean.
    #[pyo3(signature = (row, col, shown))]
    fn baseline(&mut self, row: AxesRow, col: i64, shown: Option<bool>) -> PyResult<()> {
        let shown = match shown {
            None => umber_core::BaselineVisibility::Auto,
            Some(true) => umber_core::BaselineVisibility::Shown,
            Some(false) => umber_core::BaselineVisibility::Hidden,
        };
        self.axes(row, col)?
            .baseline(shown)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the legend style (a `LegendStyle` value).
    fn legend(&mut self, row: AxesRow, col: i64, style: i64, size: SizeArg) -> PyResult<()> {
        let style = enum_value::<LegendStyle>(style)?;
        let size = size_spec(size)?;
        self.axes(row, col)?
            .legend(style, size)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the legend style and position (`LegendStyle` and `LegendPosition` values).
    fn legend_at(
        &mut self,
        row: AxesRow,
        col: i64,
        style: i64,
        position: i64,
        size: SizeArg,
    ) -> PyResult<()> {
        let style = enum_value::<LegendStyle>(style)?;
        let position = enum_value::<LegendPosition>(position)?;
        let size = size_spec(size)?;
        self.axes(row, col)?
            .legend_at(style, position, size)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a line.
    fn line(
        &mut self,
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        sizes: &Bound<'_, PyAny>,
        label: String,
        color: &Bound<'_, PyAny>,
        marker: i64,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.bubbles()";
        let options = umber_core::BubbleOptions {
            label,
            color: point_color(color)?,
            marker: enum_value::<Marker>(marker)?,
            ..Default::default()
        };
        // Coordinates in an int64 buffer are times, of a time axis.
        if is_int64_buffer(x) || is_int64_buffer(y) {
            let (x, y) = (series_data(CONTEXT, "x", x)?, series_data(CONTEXT, "y", y)?);
            return self
                .axes(row, col)?
                .bubbles_time(x, y, shared_floats(sizes)?, options)
                .map(drop)
                .map_err(core_error);
        }
        let (x, y, sizes) = (shared_floats(x)?, shared_floats(y)?, shared_floats(sizes)?);
        self.axes(row, col)?
            .bubbles_shared(x, y, sizes, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds stems from the baseline with a marker at each value.
    fn stem(
        &mut self,
        row: AxesRow,
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
        row: AxesRow,
        col: i64,
        categories: &Bound<'_, PyAny>,
        heights: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
        annotate: bool,
        format: Option<TickFormatArg>,
        size: SizeArg,
    ) -> PyResult<()> {
        let (categories, heights) = (names(categories)?, shared_floats(heights)?);
        let options = BarOptions {
            label,
            color: color_spec(color)?,
            annotation: value_labels(annotate, format, size)?,
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
        row: AxesRow,
        col: i64,
        categories: &Bound<'_, PyAny>,
        series: SeriesArgs<'_>,
        annotate: bool,
        format: Option<TickFormatArg>,
        size: SizeArg,
    ) -> PyResult<()> {
        let (categories, series) = (names(categories)?, Self::series(series)?);
        let labels = value_labels(annotate, format, size)?;
        self.axes(row, col)?
            .grouped_bar(categories, series, labels)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds aligned long-form bars with ordinary bar error context.
    fn bar_by(
        &mut self,
        row: AxesRow,
        col: i64,
        categories: &Bound<'_, PyAny>,
        series: SeriesArgs<'_>,
        annotate: bool,
        format: Option<TickFormatArg>,
        size: SizeArg,
    ) -> PyResult<()> {
        let (categories, series) = (names(categories)?, Self::series(series)?);
        let labels = value_labels(annotate, format, size)?;
        self.axes(row, col)?
            .bar_by(categories, series, labels)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a box plot for each category.
    fn box_plot(
        &mut self,
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
        col: i64,
        categories: &Bound<'_, PyAny>,
        series: SeriesArgs<'_>,
        annotate: bool,
        format: Option<TickFormatArg>,
        size: SizeArg,
    ) -> PyResult<()> {
        let (categories, series) = (names(categories)?, Self::series(series)?);
        let labels = value_labels(annotate, format, size)?;
        self.axes(row, col)?
            .stacked_bar(categories, series, labels)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds non-negative area series stacked over shared x values.
    fn stacked_area(
        &mut self,
        row: AxesRow,
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

    /// Adds a histogram of `values` in `bins`, an int bin count or a float64 buffer of edges,
    /// drawn in `style` (a `Hist` value) with the statistic `stat` (a `Stat` value).
    fn hist(
        &mut self,
        row: AxesRow,
        col: i64,
        values: &Bound<'_, PyAny>,
        bins: &Bound<'_, PyAny>,
        style: i64,
        cumulative: bool,
        stat: i64,
        label: String,
        color: ColorArg,
    ) -> PyResult<()> {
        let values = shared_floats(values)?;
        let options = HistOptions {
            bins: bins_arg(bins)?,
            style: enum_value::<HistStyle>(style)?,
            cumulative,
            stat: enum_value::<HistStat>(stat)?,
            label,
            color: color_spec(color)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .hist_shared(values, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds rectangular cells with flattened row-major color values; `render` is a `Render`
    /// value, or `None` for `AUTO`.
    #[pyo3(signature = (row, col, x_edges, y_edges, values, cmap, scale, base, center, render))]
    fn mesh(
        &mut self,
        row: AxesRow,
        col: i64,
        x_edges: &Bound<'_, PyAny>,
        y_edges: &Bound<'_, PyAny>,
        values: &Bound<'_, PyAny>,
        cmap: Option<PyRef<'_, Cmap>>,
        scale: i64,
        base: f64,
        center: Option<f64>,
        render: Option<i64>,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.mesh()";
        // Edges in an int64 buffer are times, of a time axis.
        let (x, y) = (
            series_data(CONTEXT, "x_edges", x_edges)?,
            series_data(CONTEXT, "y_edges", y_edges)?,
        );
        let values = floats(values)?;
        let (cmap, scale) = cell_colors(cmap, scale, base)?;
        self.axes(row, col)?
            .mesh_time(
                x,
                y,
                &values,
                MeshOptions {
                    cmap,
                    scale,
                    center,
                    render: render_choice(render)?,
                },
            )
            .map(drop)
            .map_err(core_error)
    }

    /// Counts paired coordinates in rectangular bins.
    #[pyo3(signature = (row, col, x, y, bins, cmap, scale, base, render))]
    fn hist2d(
        &mut self,
        row: AxesRow,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        bins: (usize, usize),
        cmap: Option<PyRef<'_, Cmap>>,
        scale: i64,
        base: f64,
        render: Option<i64>,
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
                    render: render_choice(render)?,
                },
            )
            .map(drop)
            .map_err(core_error)
    }

    /// Counts paired coordinates in regular pointy-top hexagons.
    #[pyo3(signature = (row, col, x, y, gridsize, cmap, scale, base))]
    fn hexbin(
        &mut self,
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
    #[pyo3(signature = (row, col, label, position, format, size))]
    fn colorbar(
        &mut self,
        row: AxesRow,
        col: i64,
        label: String,
        position: Option<i64>,
        format: Option<TickFormatArg>,
        size: SizeArg,
    ) -> PyResult<()> {
        let options = umber_core::ColorbarOptions {
            label,
            position: position
                .map(enum_value::<umber_core::ColorbarPosition>)
                .transpose()?,
            format: format.map(tick_format).transpose()?,
            size: size_spec(size)?,
        };
        self.axes(row, col)?
            .colorbar(options)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the shared colorbars' label and optional overrides.
    #[pyo3(signature = (label, position, format, size))]
    fn figure_colorbar(
        &mut self,
        label: String,
        position: Option<i64>,
        format: Option<TickFormatArg>,
        size: SizeArg,
    ) -> PyResult<()> {
        let options = umber_core::ColorbarOptions {
            label,
            position: position
                .map(enum_value::<umber_core::ColorbarPosition>)
                .transpose()?,
            format: format.map(tick_format).transpose()?,
            size: size_spec(size)?,
        };
        self.0.colorbar(options).map(drop).map_err(core_error)
    }

    /// Adds a heatmap: `values` holds `len(y_categories) * len(x_categories)` numbers, row by
    /// row, one row per y category; the colormap (`None` for the theme's), the linear or log
    /// scale argument `(scale, base)`, the center and the format (`None` for the y tick format)
    /// are converted as those of a `ColorBy` and of `colorbar`; `render` is a `Render` value,
    /// or `None` for `AUTO`.
    #[pyo3(signature = (row, col, x_categories, y_categories, values, cmap, scale, base, center, annotate, format, render))]
    fn heatmap(
        &mut self,
        row: AxesRow,
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
        render: Option<i64>,
    ) -> PyResult<()> {
        let options = HeatmapOptions {
            cmap: cmap_choice(cmap),
            scale: color_transform(scale, base)?,
            center,
            annotate,
            format: format.map(tick_format).transpose()?,
            render: render_choice(render)?,
        };
        let (x, y) = (names(x_categories)?, names(y_categories)?);
        self.axes(row, col)?
            .heatmap_shared(x, y, shared_floats(values)?, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a contour of the row-major values `z`, one row per y coordinate, on the grid of the
    /// coordinates `x` and `y`. `levels` is `None` for `AUTO`, an `int` for a count of round
    /// levels, or a float64 buffer of the levels; `style` is a `Contour` value; the colormap,
    /// the scale argument `(scale, base)` and the center are converted as those of `heatmap`,
    /// `color` as a color argument, and `format` as that of `heatmap` (`None` for the automatic
    /// level labels).
    #[pyo3(signature = (row, col, x, y, z, levels, style, cmap, scale, base, center, color, annotate, format))]
    fn contour(
        &mut self,
        row: AxesRow,
        col: i64,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        z: &Bound<'_, PyAny>,
        levels: Option<&Bound<'_, PyAny>>,
        style: i64,
        cmap: Option<PyRef<'_, Cmap>>,
        scale: i64,
        base: f64,
        center: Option<f64>,
        color: ColorArg,
        annotate: bool,
        format: Option<TickFormatArg>,
    ) -> PyResult<()> {
        let levels = match levels {
            None => ContourLevels::Auto,
            Some(count) if count.is_instance_of::<PyInt>() => {
                ContourLevels::Count(count.extract::<usize>()?)
            }
            Some(values) => ContourLevels::Values(floats(values)?),
        };
        let options = ContourOptions {
            levels,
            style: enum_value::<ContourStyle>(style)?,
            cmap: cmap_choice(cmap),
            scale: color_transform(scale, base)?,
            center,
            color: color_spec(color)?,
            annotate,
            format: format.map(tick_format).transpose()?,
        };
        let (x, y, z) = (floats(x)?, floats(y)?, floats(z)?);
        self.axes(row, col)?
            .contour(&x, &y, &z, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the color range of the axes' color-mapped marks.
    fn clim(&mut self, row: AxesRow, col: i64, low: f64, high: f64) -> PyResult<()> {
        self.axes(row, col)?
            .clim(low, high)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the limits of a time x axis to the times `low` and `high`, in microseconds.
    fn xlim_time(&mut self, row: AxesRow, col: i64, low: i64, high: i64) -> PyResult<()> {
        const CONTEXT: &str = "Axes.xlim()";
        let low = time_from_micros(CONTEXT, "low", low).map_err(core_error)?;
        let high = time_from_micros(CONTEXT, "high", high).map_err(core_error)?;
        self.axes(row, col)?
            .xlim_time(low, high)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the limits of a time y axis to the times `low` and `high`, in microseconds.
    fn ylim_time(&mut self, row: AxesRow, col: i64, low: i64, high: i64) -> PyResult<()> {
        const CONTEXT: &str = "Axes.ylim()";
        let low = time_from_micros(CONTEXT, "low", low).map_err(core_error)?;
        let high = time_from_micros(CONTEXT, "high", high).map_err(core_error)?;
        self.axes(row, col)?
            .ylim_time(low, high)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the ticks of a time x axis with ticks at the times `positions`.
    fn xticks_time(
        &mut self,
        row: AxesRow,
        col: i64,
        positions: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let positions = Self::tick_times("Axes.xticks()", positions)?;
        self.axes(row, col)?
            .xticks_time(&positions)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the ticks of a time y axis with ticks at the times `positions`.
    fn yticks_time(
        &mut self,
        row: AxesRow,
        col: i64,
        positions: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
        row: AxesRow,
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
    /// Sets the x tick label size.
    fn xtick_labels(&mut self, row: AxesRow, col: i64, size: SizeArg) -> PyResult<()> {
        let size = size_spec(size)?;
        self.axes(row, col)?
            .xtick_labels(size)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the y tick label size.
    fn ytick_labels(&mut self, row: AxesRow, col: i64, size: SizeArg) -> PyResult<()> {
        let size = size_spec(size)?;
        self.axes(row, col)?
            .ytick_labels(size)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds event positions grouped into category rows.
    #[pyo3(signature = (row, col, rows, positions, length, width, color))]
    fn events(
        &mut self,
        row: AxesRow,
        col: i64,
        rows: &Bound<'_, PyAny>,
        positions: Vec<Bound<'_, PyAny>>,
        length: Option<f64>,
        width: SizeArg,
        color: ColorArg,
    ) -> PyResult<()> {
        let rows = names(rows)?;
        let positions = positions
            .iter()
            .map(|values| series_data("Axes.events()", "positions", values))
            .collect::<PyResult<Vec<_>>>()?;
        let mut options = umber_core::EventsOptions {
            width: size_spec(width)?,
            color: color_spec(color)?,
            ..Default::default()
        };
        if let Some(length) = length {
            options.length = length;
        }
        self.axes(row, col)?
            .events(rows, positions, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds strip samples in category groups.
    #[pyo3(signature = (row, col, categories, groups, label, color, size, jitter))]
    fn strip(
        &mut self,
        row: AxesRow,
        col: i64,
        categories: &Bound<'_, PyAny>,
        groups: Vec<Bound<'_, PyAny>>,
        label: String,
        color: ColorArg,
        size: SizeArg,
        jitter: Option<f64>,
    ) -> PyResult<()> {
        let categories = names(categories)?;
        let groups = groups.iter().map(floats).collect::<PyResult<Vec<_>>>()?;
        let mut options = umber_core::StripOptions {
            label,
            color: color_spec(color)?,
            size: size_spec(size)?,
            ..Default::default()
        };
        if let Some(jitter) = jitter {
            options.jitter = jitter;
        }
        self.axes(row, col)?
            .strip(categories, groups, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds swarm samples in category groups.
    fn swarm(
        &mut self,
        row: AxesRow,
        col: i64,
        categories: &Bound<'_, PyAny>,
        groups: Vec<Bound<'_, PyAny>>,
        label: String,
        color: ColorArg,
        size: SizeArg,
    ) -> PyResult<()> {
        let categories = names(categories)?;
        let groups = groups.iter().map(floats).collect::<PyResult<Vec<_>>>()?;
        let options = umber_core::SwarmOptions {
            label,
            color: color_spec(color)?,
            size: size_spec(size)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .swarm(categories, groups, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a histogram of several series sharing their `bins`, drawn in `style` (a `Hist`
    /// value) with the statistic `stat` (a `Stat` value).
    fn grouped_hist(
        &mut self,
        row: AxesRow,
        col: i64,
        series: SeriesArgs<'_>,
        bins: &Bound<'_, PyAny>,
        style: i64,
        cumulative: bool,
        stat: i64,
    ) -> PyResult<()> {
        let options = GroupedHistOptions {
            bins: bins_arg(bins)?,
            style: enum_value::<HistStyle>(style)?,
            cumulative,
            stat: enum_value::<HistStat>(stat)?,
            ..Default::default()
        };
        let series = Self::series(series)?;
        self.axes(row, col)?
            .grouped_hist(series, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds the ECDF of `values`: a float64 buffer of numbers, or an int64 buffer of times.
    fn ecdf(
        &mut self,
        row: AxesRow,
        col: i64,
        values: &Bound<'_, PyAny>,
        complementary: bool,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let values = series_data("Axes.ecdf()", "values", values)?;
        let options = EcdfOptions {
            complementary,
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .ecdf(values, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds the kernel density line of `values`; a `bandwidth` of `None` is Scott's rule.
    fn kde(
        &mut self,
        row: AxesRow,
        col: i64,
        values: &Bound<'_, PyAny>,
        bandwidth: Option<f64>,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let values = floats(values)?;
        let options = KdeOptions {
            bandwidth: bandwidth_arg(bandwidth),
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .kde(&values, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds one kernel density line per series; a `bandwidth` of `None` is Scott's rule.
    fn grouped_kde(
        &mut self,
        row: AxesRow,
        col: i64,
        series: SeriesArgs<'_>,
        bandwidth: Option<f64>,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let options = GroupedKdeOptions {
            bandwidth: bandwidth_arg(bandwidth),
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
            ..Default::default()
        };
        let series = Self::series(series)?;
        self.axes(row, col)?
            .grouped_kde(series, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Writes `text` at the point `(x, y)`, each coordinate a `float`, a `str` category name or
    /// an `int` time in microseconds, as its axis takes.
    fn text(
        &mut self,
        row: AxesRow,
        col: i64,
        text: &str,
        at: (Bound<'_, PyAny>, Bound<'_, PyAny>),
        align: i64,
        size: SizeArg,
        color: ColorArg,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.text()";
        let at = (coordinate(CONTEXT, &at.0)?, coordinate(CONTEXT, &at.1)?);
        let options = TextOptions {
            align: enum_value::<Align>(align)?,
            size: size_spec(size)?,
            color: color_spec(color)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .text(text, at, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Writes `text` `offset` points away from the point `(x, y)`, given as for `text`, with an
    /// arrow to the point when `arrow` holds.
    fn annotate(
        &mut self,
        row: AxesRow,
        col: i64,
        text: &str,
        at: (Bound<'_, PyAny>, Bound<'_, PyAny>),
        offset: (f64, f64),
        arrow: bool,
        size: SizeArg,
        color: ColorArg,
    ) -> PyResult<()> {
        const CONTEXT: &str = "Axes.annotate()";
        let at = (coordinate(CONTEXT, &at.0)?, coordinate(CONTEXT, &at.1)?);
        let options = AnnotateOptions {
            offset,
            arrow,
            size: size_spec(size)?,
            color: color_spec(color)?,
            ..Default::default()
        };
        self.axes(row, col)?
            .annotate(text, at, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds named lines atomically, with palette colors in group order.
    fn grouped_line(
        &mut self,
        row: AxesRow,
        col: i64,
        groups: XyGroupArgs<'_>,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let groups = xy_groups("Axes.line()", groups)?;
        let (dash, width) = (enum_value::<Dash>(dash)?, size_spec(width)?);
        self.axes(row, col)?
            .grouped_line(groups, dash, width)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds named areas atomically, with palette colors in group order.
    fn grouped_area(
        &mut self,
        row: AxesRow,
        col: i64,
        groups: XyGroupArgs<'_>,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let groups = xy_groups("Axes.area()", groups)?;
        let (dash, width) = (enum_value::<Dash>(dash)?, size_spec(width)?);
        self.axes(row, col)?
            .grouped_area(groups, dash, width)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds named scatter series atomically, with palette colors in group order.
    fn grouped_scatter(
        &mut self,
        row: AxesRow,
        col: i64,
        groups: XyGroupArgs<'_>,
        marker: i64,
        size: SizeArg,
    ) -> PyResult<()> {
        let groups = xy_groups("Axes.scatter()", groups)?;
        let (marker, size) = (enum_value::<Marker>(marker)?, size_spec(size)?);
        self.axes(row, col)?
            .grouped_scatter(groups, marker, size)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds named step lines atomically, checking their order within each group.
    fn grouped_step(
        &mut self,
        row: AxesRow,
        col: i64,
        groups: XyGroupArgs<'_>,
        position: i64,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let groups = xy_groups("Axes.step()", groups)?;
        let position = enum_value::<Step>(position)?;
        let (dash, width) = (enum_value::<Dash>(dash)?, size_spec(width)?);
        self.axes(row, col)?
            .grouped_step(groups, position, dash, width)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds named ECDFs atomically, preserving numeric and time columns.
    fn grouped_ecdf(
        &mut self,
        row: AxesRow,
        col: i64,
        groups: Vec<(String, Bound<'_, PyAny>)>,
        complementary: bool,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let groups = groups
            .into_iter()
            .enumerate()
            .map(|(i, (label, values))| {
                Ok((
                    label,
                    series_data("Axes.ecdf()", "values", &values)
                        .map_err(|error| group_error(error, i))?,
                ))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let (dash, width) = (enum_value::<Dash>(dash)?, size_spec(width)?);
        self.axes(row, col)?
            .grouped_ecdf(groups, complementary, dash, width)
            .map(drop)
            .map_err(core_error)
    }
}
