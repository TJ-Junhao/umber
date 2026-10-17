//! Renderers that turn a [`Scene`](crate::scene::Scene) into bytes.
//!
//! The PNG and SVG backends are pure functions of a scene. They share no state with each other
//! or with layout. This module holds the options they have in common.
//!
//! # Examples
//!
//! ```
//! use umber_core::render::SvgText;
//!
//! assert_eq!(SvgText::default(), SvgText::Outlines);
//! ```

mod image;
pub(crate) mod normalize;
mod svg;

pub use svg::render_svg;

/// How the SVG backend writes text. `Outlines` is the default.
///
/// # Examples
///
/// ```
/// use umber_core::render::SvgText;
///
/// assert_ne!(SvgText::Outlines, SvgText::Editable);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SvgText {
    /// Text is drawn as glyph outlines, so it looks the same everywhere.
    #[default]
    Outlines,
    /// Text is written as `<text>` elements that can be edited in a vector editor.
    Editable,
}

mod png;

pub(crate) use png::check_dpi;
pub use png::render_png;
