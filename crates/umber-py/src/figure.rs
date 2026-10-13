//! Figures: the `Figure` class with its axes, texts, limits, legend, marks and rendering.
//!
//! A figure holds at most one axes, which the Python layer adds before it calls any axes method.
//! An axes method on a figure without axes is an internal `RuntimeError`. Every argument is
//! converted before the core call, and the core validates before it stores anything, so a failed
//! call leaves the figure unchanged.
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

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use umber_core::{
    AreaOptions, Axes, BarOptions, Dash, HistOptions, LegendStyle, LineOptions, Marker,
    ScatterOptions, Series, SvgText,
};

use crate::color::{ColorArg, color_spec};
use crate::enums::enum_value;
use crate::errors::{core_error, internal_state};
use crate::scale::scale_kind;
use crate::size::{SizeArg, size_spec};
use crate::theme::Theme;
use crate::ticks::{TickFormatArg, tick_format};
use crate::values::{axis_data, floats, names, shared_floats};

/// The series of a grouped or stacked bar chart: `(label, values, color)` for each series.
type SeriesArgs<'py> = Vec<(String, Bound<'py, PyAny>, ColorArg)>;

/// A figure: a size in inches, a theme and at most one axes with its marks.
#[pyclass(module = "umber._umber")]
pub(crate) struct Figure(umber_core::Figure);

impl Figure {
    /// The axes; an internal `RuntimeError` when the figure has none.
    fn axes(&mut self) -> PyResult<&mut Axes> {
        self.0
            .axes_mut()
            .ok_or_else(|| internal_state("the figure has no axes"))
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
    /// An empty figure of `width` by `height` inches with `theme`.
    #[new]
    fn new(width: f64, height: f64, theme: &Theme) -> PyResult<Figure> {
        umber_core::Figure::new(width, height, theme.core().clone())
            .map(Figure)
            .map_err(core_error)
    }

    /// An independent copy of this figure. Marks share their data, which never changes, so the
    /// copy is cheap.
    #[pyo3(name = "clone")]
    fn clone_figure(&self) -> Figure {
        Figure(self.0.clone())
    }

    /// Adds the axes with the x scale `(x_kind, x_base)` and the y scale `(y_kind, y_base)`.
    /// The core refuses a second axes and a category y scale, and a refused call adds nothing.
    fn add_axes(&mut self, x_kind: i64, x_base: f64, y_kind: i64, y_base: f64) -> PyResult<()> {
        let x = scale_kind(x_kind, x_base)?;
        let y = scale_kind(y_kind, y_base)?;
        self.0.add_axes(x, y).map(drop).map_err(core_error)
    }

    /// Sets the title.
    fn title(&mut self, text: &str) -> PyResult<()> {
        self.axes()?.title(text);
        Ok(())
    }

    /// Sets the subtitle.
    fn subtitle(&mut self, text: &str) -> PyResult<()> {
        self.axes()?.subtitle(text);
        Ok(())
    }

    /// Sets the x axis label.
    fn xlabel(&mut self, text: &str) -> PyResult<()> {
        self.axes()?.xlabel(text);
        Ok(())
    }

    /// Sets the y axis label.
    fn ylabel(&mut self, text: &str) -> PyResult<()> {
        self.axes()?.ylabel(text);
        Ok(())
    }

    /// Sets the x limits.
    fn xlim(&mut self, low: f64, high: f64) -> PyResult<()> {
        self.axes()?.xlim(low, high).map(drop).map_err(core_error)
    }

    /// Sets the y limits.
    fn ylim(&mut self, low: f64, high: f64) -> PyResult<()> {
        self.axes()?.ylim(low, high).map(drop).map_err(core_error)
    }

    /// Replaces the x ticks with ticks at `positions`.
    fn xticks(&mut self, positions: &Bound<'_, PyAny>) -> PyResult<()> {
        let positions = floats(positions)?;
        self.axes()?
            .xticks(&positions)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the y ticks with ticks at `positions`.
    fn yticks(&mut self, positions: &Bound<'_, PyAny>) -> PyResult<()> {
        let positions = floats(positions)?;
        self.axes()?
            .yticks(&positions)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the x ticks with ticks at `positions`, each with the label at its index.
    fn xticks_labeled(
        &mut self,
        positions: &Bound<'_, PyAny>,
        labels: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let (positions, labels) = (floats(positions)?, names(labels)?);
        self.axes()?
            .xticks_labeled(&positions, labels)
            .map(drop)
            .map_err(core_error)
    }

    /// Replaces the y ticks with ticks at `positions`, each with the label at its index.
    fn yticks_labeled(
        &mut self,
        positions: &Bound<'_, PyAny>,
        labels: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let (positions, labels) = (floats(positions)?, names(labels)?);
        self.axes()?
            .yticks_labeled(&positions, labels)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the format of the x tick labels.
    fn xtick_format(&mut self, format: TickFormatArg) -> PyResult<()> {
        let format = tick_format(format)?;
        self.axes()?
            .xtick_format(format)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the format of the y tick labels.
    fn ytick_format(&mut self, format: TickFormatArg) -> PyResult<()> {
        let format = tick_format(format)?;
        self.axes()?
            .ytick_format(format)
            .map(drop)
            .map_err(core_error)
    }

    /// Sets the legend style (a `LegendStyle` value).
    fn legend(&mut self, style: i64) -> PyResult<()> {
        let style = enum_value::<LegendStyle>(style)?;
        self.axes()?.legend(style);
        Ok(())
    }

    /// Adds a line.
    fn line(
        &mut self,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let (x, y) = (axis_data(x)?, shared_floats(y)?);
        let options = LineOptions {
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
            ..Default::default()
        };
        self.axes()?
            .line_shared(x, y, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds an area: a line with the region down to the baseline filled.
    fn area(
        &mut self,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
        dash: i64,
        width: SizeArg,
    ) -> PyResult<()> {
        let (x, y) = (axis_data(x)?, shared_floats(y)?);
        let options = AreaOptions {
            label,
            color: color_spec(color)?,
            dash: enum_value::<Dash>(dash)?,
            width: size_spec(width)?,
            ..Default::default()
        };
        self.axes()?
            .area_shared(x, y, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a scatter series.
    fn scatter(
        &mut self,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        label: String,
        color: ColorArg,
        marker: i64,
        size: SizeArg,
    ) -> PyResult<()> {
        let (x, y) = (axis_data(x)?, shared_floats(y)?);
        let options = ScatterOptions {
            label,
            color: color_spec(color)?,
            marker: enum_value::<Marker>(marker)?,
            size: size_spec(size)?,
            ..Default::default()
        };
        self.axes()?
            .scatter_shared(x, y, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds bars, one per category.
    fn bar(
        &mut self,
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
        self.axes()?
            .bar_shared(categories, heights, options)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds grouped bars: in each category, one bar per series.
    fn grouped_bar(
        &mut self,
        categories: &Bound<'_, PyAny>,
        series: SeriesArgs<'_>,
    ) -> PyResult<()> {
        let (categories, series) = (names(categories)?, Self::series(series)?);
        self.axes()?
            .grouped_bar(categories, series)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds stacked bars: in each category, one stack of the series.
    fn stacked_bar(
        &mut self,
        categories: &Bound<'_, PyAny>,
        series: SeriesArgs<'_>,
    ) -> PyResult<()> {
        let (categories, series) = (names(categories)?, Self::series(series)?);
        self.axes()?
            .stacked_bar(categories, series)
            .map(drop)
            .map_err(core_error)
    }

    /// Adds a histogram of `values` in `bins` bins.
    fn hist(
        &mut self,
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
        self.axes()?
            .hist_shared(values, options)
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
}
