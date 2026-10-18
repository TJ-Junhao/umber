//! Text shaping, measurement and outlines.
//!
//! A [`FontLibrary`] shapes a string with one of the bundled fonts, applying kerning, standard
//! ligatures and, on request, tabular figures. Text is laid out through the [`SceneFonts`] of one
//! scene, the fonts a [`TextStyle`] names by [`FaceId`]. Characters the requested font lacks are
//! drawn with another bundled family, then with fallback fonts: the system fonts for
//! [`FontLibrary::bundled`], or explicit font files for [`FontLibrary::with_fallbacks`].
//! Characters that no font covers are drawn as the primary font's `.notdef` glyph.
//!
//! All lengths are in points. Outlines are y-down with the baseline at `y = 0` of the origin
//! passed to [`TextLayout::outline`]. Runs are placed left to right in logical order; there is
//! no bidirectional reordering.
//!
//! Nothing here panics or fails: unreadable fonts are skipped, including fonts that make the
//! shaper panic, and missing glyphs degrade to `.notdef`. Such panics are caught, but the panic
//! hook is left alone: in debug builds the default or host hook may print a message for a
//! malformed fallback or system font.
//!
//! System fonts are memory-mapped by `fontdb` whenever they are read, including when a layout is
//! outlined. A system font file that changes or disappears between layout and outline yields the
//! new contents or nothing for those glyphs; the outline never panics. Shaping copies only the
//! tables the shaper reads from a system font, never its outlines.
//!
//! # Examples
//!
//! ```
//! use umber_core::text::{FaceId, FontLibrary, TextStyle};
//! use umber_core::{Font, Point, Weight};
//!
//! let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
//! let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
//! let layout = fonts.layout("Revenue", &style)?;
//! assert!(layout.width() > 0.0);
//! let path = layout.outline(Point { x: 0.0, y: 0.0 });
//! assert!(!path.is_empty());
//! # Ok::<(), umber_core::Error>(())
//! ```

// Module layering, lowest first; each module uses only the ones listed before it:
// - `style`, `guard`, `cmap`, `outline` and `marks` are leaves;
// - `font` owns font identity, font bytes and parsed faces (uses `cmap`, `outline`);
// - `shaping` owns the shaper callbacks, features and plan reuse (uses `font`);
// - `runs` owns splitting text into runs and grouping them by font (uses `font`, `marks`);
// - `layout` owns placed glyphs and `TextLayout` (uses `font`, `shaping`, `runs`, `outline`,
//   `guard`);
// - `system` owns system font discovery, lookup and shaping tables (uses `font`, `cmap`,
//   `guard`);
// - `library` coordinates one layout call over all of them, and resolves the fonts of a scene.
mod cmap;
mod font;
mod guard;
mod layout;
mod library;
mod marks;
mod outline;
mod runs;
mod shaping;
mod style;
mod system;

#[cfg(test)]
mod tests;

pub(crate) use font::OwnedFace;
pub use layout::TextLayout;
pub(crate) use library::CheckedStyle;
pub use library::{FontLibrary, SceneFonts};
pub use style::{FaceId, TextStyle};
pub(crate) use system::find_family;
