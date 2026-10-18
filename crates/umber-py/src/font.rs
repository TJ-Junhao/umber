//! Fonts: the bundled fonts of `umber.Font` and the conversion of a font argument to a core
//! [`Font`].
//!
//! A font argument travels as a tuple `(kind, payload)`:
//!
//! | kind | meaning | payload |
//! |---|---|---|
//! | 0 | a bundled font | its index in [`BUNDLED`] |

use pyo3::prelude::*;
use umber_core::{BundledFont, Font};

use crate::errors::internal_value;

/// The `kind` of a font argument for a bundled font.
const BUNDLED_KIND: i64 = 0;

/// The bundled fonts in the order of their indices, which `umber.Font` lists them in.
const BUNDLED: [BundledFont; 5] = BundledFont::ALL;

/// The name of the `umber.Font` constant of `font`.
fn constant_name(font: BundledFont) -> &'static str {
    match font {
        BundledFont::SourceSerif4 => "SOURCE_SERIF_4",
        BundledFont::IbmPlexSans => "IBM_PLEX_SANS",
        BundledFont::Geist => "GEIST",
        BundledFont::Inter => "INTER",
        BundledFont::JetBrainsMono => "JETBRAINS_MONO",
    }
}

/// The bundled fonts as `(constant name, index)` pairs, in index order.
#[pyfunction]
pub(crate) fn bundled_fonts() -> Vec<(&'static str, i64)> {
    (0_i64..)
        .zip(BUNDLED)
        .map(|(index, font)| (constant_name(font), index))
        .collect()
}

/// A font argument `(kind, payload)`, as the module documentation describes.
pub(crate) type FontArg = (i64, i64);

/// Converts a font argument to a core [`Font`]. An unknown kind or a bundled index out of range
/// is an internal `ValueError`.
pub(crate) fn font((kind, payload): FontArg) -> PyResult<Font> {
    match kind {
        BUNDLED_KIND => usize::try_from(payload)
            .ok()
            .and_then(|index| BUNDLED.get(index))
            .map(|bundled| Font::Bundled(*bundled))
            .ok_or_else(|| internal_value(format!("{payload} is not a bundled font"))),
        _ => Err(internal_value(format!("{kind} is not a font kind"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_round_trip_through_their_indices() {
        let table = bundled_fonts();
        assert_eq!(
            table,
            [
                ("SOURCE_SERIF_4", 0),
                ("IBM_PLEX_SANS", 1),
                ("GEIST", 2),
                ("INTER", 3),
                ("JETBRAINS_MONO", 4),
            ]
        );
        for ((_, index), bundled) in table.into_iter().zip(BundledFont::ALL) {
            assert_eq!(font((BUNDLED_KIND, index)).unwrap(), Font::Bundled(bundled));
        }
    }
}
