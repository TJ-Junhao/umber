//! Helpers shared by unit tests in different modules. Compiled only for `cargo test`.

pub(crate) mod polygon;
pub(crate) mod scene;

// The integration tests include this file by path, so it only uses `std` and the `png` crate.
pub(crate) mod image;

/// The height, in points, that `err` names as needed for the plot area, if it is the layout
/// error for a figure too short.
pub(crate) fn named_height(err: &crate::Error) -> Option<f64> {
    let message = err.to_string();
    let (_, rest) = message.split_once(" pt tall; at least ")?;
    rest.strip_suffix(" pt is needed for the plot area.")?
        .parse()
        .ok()
}

/// The fonts of a figure with `theme`, laid out through the global library, as `Figure::to_scene`
/// lays the figure out with [`crate::FontLibrary::global`].
pub(crate) fn figure_fonts(theme: &crate::Theme) -> crate::text::SceneFonts<'static> {
    crate::layout::figure_fonts(crate::FontLibrary::global(), theme).expect("bundled fonts resolve")
}

/// Every bundled font, laid out through `library`: face `i` is `BundledFont::ALL[i]`.
pub(crate) fn bundled_fonts(library: &crate::FontLibrary) -> crate::text::SceneFonts<'_> {
    let fonts: Vec<crate::Font> = crate::BundledFont::ALL
        .into_iter()
        .map(crate::Font::Bundled)
        .collect();
    library
        .with_figure_fonts(&fonts)
        .expect("bundled fonts resolve")
}

/// The face of `font` in [`bundled_fonts`].
pub(crate) fn bundled_face(font: crate::BundledFont) -> crate::FaceId {
    let index = crate::BundledFont::ALL
        .iter()
        .position(|&bundled| bundled == font)
        .expect("every bundled font is listed");
    crate::FaceId::new(index)
}
