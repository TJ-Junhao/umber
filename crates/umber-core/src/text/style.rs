//! The style a piece of text is laid out with, and the face it names.

use crate::fonts::Weight;

/// A font of one scene, by its index in the scene's font list.
///
/// The list is the one given to
/// [`FontLibrary::with_figure_fonts`](super::FontLibrary::with_figure_fonts) when text is laid
/// out, and [`Scene::fonts`](crate::scene::Scene::fonts) in a scene. A face is a plain index,
/// so styles and the scene items that name it stay small and `Copy`.
///
/// # Examples
///
/// ```
/// use umber_core::FaceId;
///
/// assert_eq!(FaceId::new(1).index(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FaceId(usize);

impl FaceId {
    /// The face at `index` in a font list.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::FaceId;
    ///
    /// assert_eq!(FaceId::new(0), FaceId::new(0));
    /// ```
    #[must_use]
    pub const fn new(index: usize) -> FaceId {
        FaceId(index)
    }

    /// The index of this face in its font list.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::FaceId;
    ///
    /// assert_eq!(FaceId::new(4).index(), 4);
    /// ```
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// How a piece of text is set: face, weight, size and figure style.
///
/// # Examples
///
/// ```
/// use umber_core::{FaceId, TextStyle, Weight};
///
/// let style = TextStyle { face: FaceId::new(0), weight: Weight::SemiBold, size: 13.0, tabular_figures: true };
/// assert_eq!(style.size, 13.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// The primary font: a face of the fonts the text is laid out with.
    pub face: FaceId,
    /// The weight, used for the primary family and for bundled fallbacks.
    pub weight: Weight,
    /// The font size in points. NaN, infinite, zero and negative sizes are treated as 0.
    pub size: f32,
    /// Whether to request tabular (fixed-width) figures with the OpenType `tnum` feature.
    pub tabular_figures: bool,
}
