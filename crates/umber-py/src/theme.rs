//! Themes: the immutable `Theme` class with the built-in presets and every bound `with_*` method.
//!
//! Each `with_*` method returns a new theme and leaves the original unchanged. Color values
//! arrive as color arguments (see the `color` module) and must be concrete: `Color.AUTO` and
//! `Color.palette()` refer to a theme's palette, so a theme cannot use them. Enum values arrive
//! as the integers of the `enums` module, and numbers are validated by the core.

use pyo3::prelude::*;
use umber_core::{Align, Dash, FillStyle, Font, GridLines, LegendPosition, LegendStyle, Rgba};

use crate::cmap::Cmap;
use crate::color::{ColorArg, color_spec, concrete, concrete_list};
use crate::enums::{IntEnum, enum_value};
use crate::errors::core_error;

/// A `with_*` method of the core theme.
type With<T> = fn(&umber_core::Theme, T) -> umber_core::Result<umber_core::Theme>;

/// An immutable theme. Equal themes compare equal; themes are not hashable.
#[pyclass(module = "umber._umber", frozen, eq)]
#[derive(PartialEq)]
pub(crate) struct Theme(umber_core::Theme);

impl Theme {
    /// The core theme.
    pub(crate) fn core(&self) -> &umber_core::Theme {
        &self.0
    }

    /// A new theme with `value`, a concrete color, set by `with`. A color that is not concrete is
    /// a `StyleError` in `context` about `value`.
    fn with_color(
        &self,
        context: &'static str,
        value: ColorArg,
        with: With<Rgba>,
    ) -> PyResult<Theme> {
        let color =
            concrete(color_spec(value)?, context, "value", "value", "theme").map_err(core_error)?;
        self.with(with, color)
    }

    /// A new theme with the enum value `value` set by `with`.
    fn with_enum<E: IntEnum>(&self, value: i64, with: With<E>) -> PyResult<Theme> {
        self.with(with, enum_value(value)?)
    }

    /// A new theme with `value` set by `with`, which validates it.
    fn with<T>(&self, with: With<T>, value: T) -> PyResult<Theme> {
        with(&self.0, value).map(Theme).map_err(core_error)
    }
}

#[pymethods]
impl Theme {
    /// The default theme: warm paper, serif titles, dotted horizontal grid.
    #[staticmethod]
    fn pigment() -> Theme {
        Theme(umber_core::Theme::pigment())
    }

    /// A clean white theme with a navy-led palette.
    #[staticmethod]
    fn graphite() -> Theme {
        Theme(umber_core::Theme::graphite())
    }

    /// A dark theme with a monospaced body font.
    #[staticmethod]
    fn midnight() -> Theme {
        Theme(umber_core::Theme::midnight())
    }

    /// This theme with the palette `colors`, each of which must be concrete. The message of a
    /// color that is not concrete names its index, e.g. `colors[2]`.
    fn with_palette(&self, colors: Vec<ColorArg>) -> PyResult<Theme> {
        let colors = concrete_list(colors, "Theme.with_palette()", "theme")?;
        self.with(umber_core::Theme::with_palette, colors.as_slice())
    }

    /// This theme with the background color `value`.
    fn with_background(&self, value: ColorArg) -> PyResult<Theme> {
        let with = umber_core::Theme::with_background;
        self.with_color("Theme.with_background()", value, with)
    }

    /// This theme with the text color `value`.
    fn with_text_color(&self, value: ColorArg) -> PyResult<Theme> {
        let with = umber_core::Theme::with_text_color;
        self.with_color("Theme.with_text_color()", value, with)
    }

    /// This theme with the muted text color `value`.
    fn with_muted_color(&self, value: ColorArg) -> PyResult<Theme> {
        let with = umber_core::Theme::with_muted_color;
        self.with_color("Theme.with_muted_color()", value, with)
    }

    /// This theme with the gridline color `value`.
    fn with_grid_color(&self, value: ColorArg) -> PyResult<Theme> {
        let with = umber_core::Theme::with_grid_color;
        self.with_color("Theme.with_grid_color()", value, with)
    }

    /// This theme with the baseline color `value`.
    fn with_baseline_color(&self, value: ColorArg) -> PyResult<Theme> {
        let with = umber_core::Theme::with_baseline_color;
        self.with_color("Theme.with_baseline_color()", value, with)
    }

    /// This theme with the gridlines `value` (a `GridLines` value).
    fn with_grid_lines(&self, value: i64) -> PyResult<Theme> {
        self.with_enum::<GridLines>(value, umber_core::Theme::with_grid_lines)
    }

    /// This theme with the gridline dash `value` (a `Dash` value).
    fn with_grid_dash(&self, value: i64) -> PyResult<Theme> {
        self.with_enum::<Dash>(value, umber_core::Theme::with_grid_dash)
    }

    /// This theme with the title font `value` (a `Font` value).
    fn with_title_font(&self, value: i64) -> PyResult<Theme> {
        self.with_enum::<Font>(value, umber_core::Theme::with_title_font)
    }

    /// This theme with the title alignment `value` (an `Align` value).
    fn with_title_align(&self, value: i64) -> PyResult<Theme> {
        self.with_enum::<Align>(value, umber_core::Theme::with_title_align)
    }

    /// This theme with the body font `value` (a `Font` value).
    fn with_body_font(&self, value: i64) -> PyResult<Theme> {
        self.with_enum::<Font>(value, umber_core::Theme::with_body_font)
    }

    /// This theme with the legend style `value` (a `LegendStyle` value).
    fn with_legend_style(&self, value: i64) -> PyResult<Theme> {
        self.with_enum::<LegendStyle>(value, umber_core::Theme::with_legend_style)
    }

    /// This theme with the panel legend position `value` (a `LegendPosition` value).
    fn with_legend_position(&self, value: i64) -> PyResult<Theme> {
        self.with_enum::<LegendPosition>(value, umber_core::Theme::with_legend_position)
    }

    /// This theme with the fill style `value` (a `FillStyle` value).
    fn with_fill_style(&self, value: i64) -> PyResult<Theme> {
        self.with_enum::<FillStyle>(value, umber_core::Theme::with_fill_style)
    }

    /// This theme with the font scale `value`.
    fn with_font_scale(&self, value: f64) -> PyResult<Theme> {
        self.with(umber_core::Theme::with_font_scale, value)
    }

    /// This theme with the line width `value`, in points.
    fn with_line_width(&self, value: f64) -> PyResult<Theme> {
        self.with(umber_core::Theme::with_line_width, value)
    }

    /// This theme with the marker size `value`, in points.
    fn with_marker_size(&self, value: f64) -> PyResult<Theme> {
        self.with(umber_core::Theme::with_marker_size, value)
    }

    /// This theme with the marker opacity `value`.
    fn with_marker_alpha(&self, value: f64) -> PyResult<Theme> {
        self.with(umber_core::Theme::with_marker_alpha, value)
    }

    /// This theme with the baseline width `value`, in points.
    fn with_baseline_width(&self, value: f64) -> PyResult<Theme> {
        self.with(umber_core::Theme::with_baseline_width, value)
    }

    /// This theme with the bar corner radius `value`, in points.
    fn with_bar_corner_radius(&self, value: f64) -> PyResult<Theme> {
        self.with(umber_core::Theme::with_bar_corner_radius, value)
    }

    /// This theme with the gap between histogram bars `value`, in points.
    fn with_hist_gap(&self, value: f64) -> PyResult<Theme> {
        self.with(umber_core::Theme::with_hist_gap, value)
    }

    /// This theme with the default sequential colormap `cmap`.
    fn with_cmap(&self, cmap: &Cmap) -> Theme {
        Theme(self.0.with_cmap(cmap.core().clone()))
    }

    /// This theme with the colorbar position `position`.
    fn with_colorbar_position(&self, position: i64) -> PyResult<Theme> {
        self.with_enum::<umber_core::ColorbarPosition>(
            position,
            umber_core::Theme::with_colorbar_position,
        )
    }

    /// This theme with the default diverging colormap `cmap`.
    fn with_diverging_cmap(&self, cmap: &Cmap) -> Theme {
        Theme(self.0.with_diverging_cmap(cmap.core().clone()))
    }
}
