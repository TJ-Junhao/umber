//! Rendering core of umber.
//!
//! This crate holds every drawing algorithm used by the `umber` Python package. It has no
//! Python dependency and can be used directly from Rust. Its API is `0.x` and may change
//! between minor versions.
//!
//! # Examples
//!
//! ```
//! assert!(!umber_core::VERSION.is_empty());
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod color;
pub mod downsample;
mod error;
pub mod fonts;
pub mod geometry;
pub mod render;
pub mod scale;
pub mod scene;
pub mod text;
pub mod ticks;

pub use color::{Alpha, ColorSpec, Rgba};
pub use downsample::m4_indices;
pub use error::{Error, Result};
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
pub use text::{FontLibrary, TextLayout, TextStyle};
pub use ticks::Tick;

/// The version of this crate, as written in `Cargo.toml`.
///
/// # Examples
///
/// ```
/// assert_eq!(umber_core::VERSION, env!("CARGO_PKG_VERSION"));
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
