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
//! use umber_core::{AxisData, ColorSpec, Figure, LineOptions, PointColor, Rgba, ScaleKind};
//! use umber_core::{ScatterOptions, SvgText, Theme};
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
//!             color: PointColor::Uniform(ColorSpec::Fixed(Rgba::hex("#3366b8")?)),
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
//!   [`ScatterOptions`], [`BubbleOptions`], [`AreaOptions`], [`StepOptions`] with its [`Step`]
//!   position, [`StemOptions`], [`RuleOptions`] and [`SpanOptions`] with the [`AxisValue`] of
//!   a vertical line, [`BandOptions`], [`ErrorBarOptions`] with its [`ErrorBarOrientation`],
//!   [`BoxOptions`], [`ViolinOptions`] and [`Distribution`], [`BarOptions`], [`Series`],
//!   [`HistOptions`] with its [`Bins`], bin limit [`MAX_BINS`], [`HistStyle`] and [`HistStat`],
//!   [`GroupedHistOptions`], [`EcdfOptions`], [`KdeOptions`] and [`GroupedKdeOptions`] with
//!   their [`Bandwidth`], and [`HeatmapOptions`], and the
//!   read-only [`Mark`], [`LineMark`], [`ScatterMark`], [`BubbleMark`], [`AreaMark`],
//!   [`StackedAreaMark`], [`StepMark`], [`StemMark`], [`HLineMark`], [`VLineMark`], [`SpanMark`],
//!   [`BarMark`], [`BarGroupMark`], [`SeriesView`], [`BoxMark`], [`BoxGroupMark`],
//!   [`ViolinMark`], [`ViolinGroupMark`], [`DistributionView`], [`BandMark`], [`ErrorBarMark`],
//!   [`HistMark`], [`HistGroupMark`], [`EcdfMark`], [`KdeMark`] and [`HeatmapMark`] that
//!   [`Axes::marks`] returns;
//! - numeric cells: [`MeshOptions`], [`Hist2dOptions`] and [`HexbinOptions`], with the
//!   read-only [`MeshMark`], [`Hist2dMark`] and [`HexbinMark`] payloads of [`Mark`], and
//!   [`Render`], which draws the cells of heatmaps, 2-D histograms and meshes as vector shapes
//!   or as one image;
//! - styling: [`Theme`] (presets, getters and `with_*` builders), [`Font`], [`Dash`],
//!   [`Marker`], [`FillStyle`], [`GridLines`], [`Align`], [`LegendStyle`] and [`LegendPosition`];
//! - colors: [`Rgba`], [`ColorSpec`] and [`Alpha`], the colormap [`Cmap`], and the color
//!   mapping of points: [`ColorBy`] with its [`CmapChoice`], and the [`PointColor`] of scatter
//!   and bubble marks; colorbars: [`ColorbarOptions`] and [`ColorbarPosition`];
//! - per-series line widths and marker sizes: [`SizeSpec`], with its [`Factor`] and [`Points`];
//! - tick labels: [`TickFormat`], with its [`NumberStyle`] and [`Decimals`];
//! - times: [`Instant`] and its [`Civil`] date and time, and [`times_from_micros`] and
//!   [`time_from_micros`], which read times given as microseconds, with [`MISSING_TIME`];
//! - scales: [`ScaleKind`] and [`LogBase`];
//! - output: [`SvgText`];
//! - event rows: [`EventsMark`] and [`EventsOptions`]; category points: [`StripMark`],
//!   [`StripOptions`], [`SwarmMark`] and [`SwarmOptions`];
//! - panel kinds: [`Panel`] distinguishes Cartesian axes from [`PiePanel`]; circular panels
//!   are edited with [`PieAxesMut`] and [`PieOptions`];
//! - errors and metadata: [`Error`], [`Result`] and [`VERSION`].
//!
//! **Advanced Rust API.** Lower-level building blocks for direct Rust use. They are public and
//! documented, but the Python package does not expose them:
//!
//! - scenes and geometry: [`Scene`], its records ([`Item`], [`TextItem`], [`Paint`],
//!   [`StrokeStyle`] and the rest of [`scene`]), [`Path`], [`PathVerb`] and [`Point`];
//! - rendering a scene: [`render_png`](render::render_png) and
//!   [`render_svg`](render::render_svg), and [`Figure::to_scene`] to get a figure's scene;
//! - text: [`FontLibrary`], the [`SceneFonts`] of one scene with its [`FaceId`], [`TextStyle`],
//!   [`TextLayout`] and [`Weight`];
//! - the bundled fonts: [`BundledFont`], [`fonts::font_bytes`] and [`fonts::license_text`];
//! - scale and tick algorithms: [`ContinuousScale`], [`CategoryScale`], [`Transform`],
//!   [`Tick`], the numeric limits [`MAX_MAGNITUDE`] and [`MIN_LOG_VALUE`], and the M4
//!   downsampling of [`m4_indices`].
//!
//! Everything else, such as scale resolution, layout, drawing and the resolution of mark colors
//! and sizes, is internal and may change in any release.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[path = "stats/bin2d.rs"]
mod bin2d;
mod binning;
mod bubble_fit;
#[cfg(test)]
mod bubble_fit_tests;
mod bubble_scale;
#[cfg(test)]
mod bubbles_tests;
pub mod cmap;
pub mod color;
#[cfg(test)]
mod color_by_tests;
mod color_scale;
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
#[cfg(test)]
mod legend_swatch_tests;
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
mod time;
#[cfg(test)]
mod time_axes_tests;

pub use binning::MAX_BINS;
pub use cmap::Cmap;
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
pub use figure::{
    Bandwidth, Bins, EcdfMark, EcdfOptions, GroupedHistOptions, GroupedKdeOptions, HistGroupMark,
    HistStat, HistStyle, KdeMark, KdeOptions,
};
pub use figure::{BoxGroupMark, BoxMark, BoxOptions, Distribution, DistributionView};
pub use figure::{BubbleMark, BubbleOptions};
pub use figure::{CmapChoice, ColorBy, PointColor};
pub use figure::{ErrorBarMark, ErrorBarOptions, ErrorBarOrientation};
pub use figure::{EventsMark, EventsOptions, StripMark, StripOptions, SwarmMark, SwarmOptions};
pub use figure::{HeatmapMark, HeatmapOptions};
pub use figure::{
    HexbinMark, HexbinOptions, Hist2dMark, Hist2dOptions, MeshMark, MeshOptions, Render,
};
pub use figure::{Panel, PieAxesMut, PieOptions, PiePanel};
pub use figure::{StemMark, StemOptions};
pub use figure::{ViolinGroupMark, ViolinMark, ViolinOptions};
pub use fonts::{BundledFont, Font, Weight};
pub use geometry::{Path, PathVerb, Point};
pub use model::{BaselineVisibility, ColorbarOptions};
pub use render::SvgText;
pub use scale::{
    CategoryScale, ContinuousScale, LogBase, MAX_MAGNITUDE, MIN_LOG_VALUE, ScaleKind, Transform,
};
pub use scene::{
    FillRule, GradientStop, Item, LineCap, LineJoin, LinearGradient, Paint, Rect, Scene,
    StrokeStyle, TextItem,
};
pub use size::{Factor, Points, SizeSpec};
pub use text::{FaceId, FontLibrary, SceneFonts, TextLayout, TextStyle};
pub use theme::ColorbarPosition;
pub use theme::{Align, Dash, FillStyle, GridLines, LegendPosition, LegendStyle, Marker, Theme};
pub use tick_format::{Decimals, NumberStyle, TickFormat};
pub use ticks::Tick;
pub use time::{Civil, Instant, MISSING_TIME, time_from_micros, times_from_micros};

/// The version of this crate, as written in `Cargo.toml`.
///
/// # Examples
///
/// ```
/// assert_eq!(umber_core::VERSION, env!("CARGO_PKG_VERSION"));
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
