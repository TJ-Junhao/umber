//! Rendering core of umber.
//!
//! This crate holds every drawing algorithm used by the `umber` Python package. It has no
//! Python dependency and can be used directly from Rust. Its API is `0.x` and may change
//! between minor versions.
//!
//! # Examples
//!
//! A figure with one axes, a line and a scatter, written as SVG and as PNG:
//!
//! ```
//! use umber_core::{AxisData, ColorSpec, Figure, LineOptions, Rgba, ScaleKind, ScatterOptions};
//! use umber_core::{SvgText, Theme};
//!
//! let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
//! figure
//!     .add_axes(ScaleKind::Linear, ScaleKind::Linear)?
//!     .title("Signal")
//!     .xlabel("time (s)")
//!     .ylabel("level")
//!     .line(
//!         AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0]),
//!         &[1.0, 3.0, 2.0, 4.0],
//!         LineOptions::default(),
//!     )?
//!     .scatter(
//!         AxisData::Numbers(vec![0.5, 1.5, 2.5]),
//!         &[2.0, 2.5, 3.5],
//!         ScatterOptions {
//!             color: ColorSpec::Fixed(Rgba::hex("#3366b8")?),
//!             ..ScatterOptions::default()
//!         },
//!     )?;
//!
//! let svg = figure.to_svg(SvgText::Editable)?;
//! assert!(svg.starts_with("<svg") && svg.contains(">Signal</text>"));
//!
//! let png = figure.to_png(144)?;
//! assert_eq!(&png[1..4], b"PNG");
//! # Ok::<(), umber_core::Error>(())
//! ```
//!
//! # API surface
//!
//! **Figure API.** The items the Python bindings build on, and the usual entry point from
//! Rust:
//!
//! - the figure model: [`Figure`] (size, theme and grid getters, [`Figure::with_grid`],
//!   [`Figure::add_axes`], [`Figure::add_axes_at`], [`Figure::axes_at`],
//!   [`Figure::axes_at_mut`], [`Figure::panels`], [`Figure::to_png`], [`Figure::to_svg`]), the
//!   grid: [`Grid`] with its limit [`MAX_GRID`], [`Share`], [`LegendMode`], [`GridIndex`],
//!   [`Placement`], [`CellRange`] and [`Cell`]; [`Axes`] (the getters of its cells, scales,
//!   texts, limits, ticks and marks), [`AxesMut`] (text, limit and tick setters and mark
//!   methods), [`ExplicitTicks`], [`AxisData`], [`LineOptions`],
//!   [`ScatterOptions`], [`AreaOptions`], [`StepOptions`] with its [`Step`] position,
//!   [`RuleOptions`] and [`SpanOptions`] with the [`AxisValue`] of a vertical line,
//!   [`BandOptions`], [`ErrorBarOptions`] with its [`ErrorBarOrientation`],
//!   [`BoxOptions`] and [`Distribution`], [`BarOptions`], [`Series`] and [`HistOptions`]
//!   with its bin limit [`MAX_BINS`], and the
//!   read-only [`Mark`], [`LineMark`], [`ScatterMark`], [`AreaMark`], [`StepMark`],
//!   [`HLineMark`], [`VLineMark`], [`SpanMark`], [`BarMark`], [`BarGroupMark`], [`SeriesView`],
//!   [`BoxMark`], [`BoxGroupMark`], [`DistributionView`],
//!   [`BandMark`], [`ErrorBarMark`] and [`HistMark`] that [`Axes::marks`] returns;
//! - styling: [`Theme`] (presets, getters and `with_*` builders), [`Font`], [`Dash`],
//!   [`Marker`], [`FillStyle`], [`GridLines`] and [`LegendStyle`];
//! - colors: [`Rgba`], [`ColorSpec`] and [`Alpha`];
//! - per-series line widths and marker sizes: [`SizeSpec`], with its [`Factor`] and [`Points`];
//! - tick labels: [`TickFormat`], with its [`NumberStyle`] and [`Decimals`];
//! - scales: [`ScaleKind`] and [`LogBase`];
//! - output: [`SvgText`];
//! - errors and metadata: [`Error`], [`Result`] and [`VERSION`].
//!
//! **Advanced Rust API.** Lower-level building blocks for direct Rust use. They are public and
//! documented, but the Python package does not expose them:
//!
//! - scenes and geometry: [`Scene`], its records ([`Item`], [`TextItem`], [`Paint`],
//!   [`StrokeStyle`] and the rest of [`scene`]), [`Path`], [`PathVerb`] and [`Point`];
//! - rendering a scene: [`render_png`](render::render_png) and
//!   [`render_svg`](render::render_svg), and [`Figure::to_scene`] to get a figure's scene;
//! - text: [`FontLibrary`], [`TextStyle`], [`TextLayout`] and [`Weight`];
//! - the bundled fonts: [`fonts::font_bytes`] and [`fonts::license_text`];
//! - scale and tick algorithms: [`ContinuousScale`], [`CategoryScale`], [`Transform`],
//!   [`Tick`], the numeric limits [`MAX_MAGNITUDE`] and [`MIN_LOG_VALUE`], and the M4
//!   downsampling of [`m4_indices`].
//!
//! Everything else, such as scale resolution, layout, drawing and the resolution of mark colors
//! and sizes, is internal and may change in any release.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod binning;
mod bubble_scale;
#[cfg(test)]
mod bubbles_tests;
pub mod color;
mod coverage;
pub mod downsample;
mod draw;
mod error;
mod exact;
pub mod figure;
pub mod fonts;
pub mod geometry;
mod layout;
mod legend;
#[cfg(test)]
mod legend_position_tests;
mod model;
mod pipeline;
pub mod render;
mod resolve;
pub mod scale;
pub mod scene;
mod scene_text;
pub mod size;
mod stacking;
mod stats;
#[cfg(test)]
mod test_support;
pub mod text;
pub mod theme;
pub mod tick_format;
pub mod ticks;

pub use binning::MAX_BINS;
pub use color::{Alpha, ColorSpec, Rgba};
pub use downsample::m4_indices;
pub use error::{Error, Result};
pub use figure::StackedAreaMark;
pub use figure::{
    AreaMark, AreaOptions, Axes, AxesMut, AxisData, BarGroupMark, BarMark, BarOptions, Cell,
    CellRange, ExplicitTicks, Figure, Grid, GridIndex, HistMark, HistOptions, LegendMode, LineMark,
    LineOptions, MAX_GRID, Mark, Placement, ScatterMark, ScatterOptions, Series, SeriesView, Share,
    Step, StepMark, StepOptions,
};
pub use figure::{AxisValue, HLineMark, RuleOptions, SpanMark, SpanOptions, VLineMark};
pub use figure::{BandMark, BandOptions};
pub use figure::{BoxGroupMark, BoxMark, BoxOptions, Distribution, DistributionView};
pub use figure::{BubbleMark, BubbleOptions};
pub use figure::{ErrorBarMark, ErrorBarOptions, ErrorBarOrientation};
pub use figure::{StemMark, StemOptions};
pub use fonts::{Font, Weight};
pub use geometry::{Path, PathVerb, Point};
pub use render::SvgText;
pub use scale::{
    CategoryScale, ContinuousScale, LogBase, MAX_MAGNITUDE, MIN_LOG_VALUE, ScaleKind, Transform,
};
pub use scene::{
    FillRule, GradientStop, Item, LineCap, LineJoin, LinearGradient, Paint, Rect, Scene,
    StrokeStyle, TextItem,
};
pub use size::{Factor, Points, SizeSpec};
pub use text::{FontLibrary, TextLayout, TextStyle};
pub use theme::{Align, Dash, FillStyle, GridLines, LegendPosition, LegendStyle, Marker, Theme};
pub use tick_format::{Decimals, NumberStyle, TickFormat};
pub use ticks::Tick;

/// The version of this crate, as written in `Cargo.toml`.
///
/// # Examples
///
/// ```
/// assert_eq!(umber_core::VERSION, env!("CARGO_PKG_VERSION"));
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
