//! The style a piece of text is laid out with.

use crate::fonts::{Font, Weight};

/// How a piece of text is set: font, weight, size and figure style.
///
/// # Examples
///
/// ```
/// use umber_core::{Font, TextStyle, Weight};
///
/// let style = TextStyle { font: Font::Geist, weight: Weight::SemiBold, size: 13.0, tabular_figures: true };
/// assert_eq!(style.size, 13.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// The primary font family.
    pub font: Font,
    /// The weight, used for the primary family and for bundled fallbacks.
    pub weight: Weight,
    /// The font size in points. NaN, infinite, zero and negative sizes are treated as 0.
    pub size: f32,
    /// Whether to request tabular (fixed-width) figures with the OpenType `tnum` feature.
    pub tabular_figures: bool,
}
