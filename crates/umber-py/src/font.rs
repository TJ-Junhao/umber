//! Font arguments: a kind and either a bundled index or an immutable parsed font value.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use pyo3::prelude::*;
use umber_core::{BundledFont, Font};

use crate::errors::{core_error, internal_value};

const BUNDLED_KIND: i64 = 0;
const FILE_KIND: i64 = 1;
const SYSTEM_KIND: i64 = 2;
const BUNDLED: [BundledFont; 5] = BundledFont::ALL;

/// An immutable parsed font, shared by Python font values and native themes.
#[pyclass(frozen, from_py_object, module = "umber._umber")]
#[derive(Clone)]
pub(crate) struct FontValue {
    inner: Font,
}

#[pymethods]
impl FontValue {
    fn family_name(&self) -> &str {
        self.inner.family_name()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    fn __hash__(&self) -> u64 {
        let mut hash = DefaultHasher::new();
        self.inner.hash(&mut hash);
        hash.finish()
    }
}

/// Parses bytes at construction, before a font is assigned to a theme.
#[pyfunction]
pub(crate) fn font_from_files(
    regular: Vec<u8>,
    bold: Option<Vec<u8>>,
    name: String,
) -> PyResult<FontValue> {
    Font::from_files(regular, bold, name)
        .map(|inner| FontValue { inner })
        .map_err(core_error)
}

/// Resolves an installed family at construction.
#[pyfunction]
pub(crate) fn font_system(family: &str) -> PyResult<FontValue> {
    Font::system(family)
        .map(|inner| FontValue { inner })
        .map_err(core_error)
}

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

/// The payload of a font argument: an index or a parsed value, never raw font bytes.
#[derive(FromPyObject)]
pub(crate) enum FontPayload {
    Index(i64),
    Loaded(FontValue),
}

pub(crate) type FontArg = (i64, FontPayload);

/// Converts a checked font argument without parsing or resolving its files again.
pub(crate) fn font((kind, payload): FontArg) -> PyResult<Font> {
    match (kind, payload) {
        (BUNDLED_KIND, FontPayload::Index(index)) => usize::try_from(index)
            .ok()
            .and_then(|index| BUNDLED.get(index))
            .map(|bundled| Font::Bundled(*bundled))
            .ok_or_else(|| internal_value(format!("{index} is not a bundled font"))),
        (
            FILE_KIND,
            FontPayload::Loaded(FontValue {
                inner: font @ Font::File(_),
            }),
        )
        | (
            SYSTEM_KIND,
            FontPayload::Loaded(FontValue {
                inner: font @ Font::System(_),
            }),
        ) => Ok(font),
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
                ("JETBRAINS_MONO", 4)
            ]
        );
        for ((_, index), bundled) in table.into_iter().zip(BundledFont::ALL) {
            assert_eq!(
                font((BUNDLED_KIND, FontPayload::Index(index))).unwrap(),
                Font::Bundled(bundled)
            );
        }
    }

    #[test]
    fn loaded_fonts_round_trip_without_parsing_again() {
        let file = Font::from_files(
            umber_core::fonts::font_bytes(BundledFont::Inter, umber_core::Weight::Regular).to_vec(),
            None,
            "Inter".into(),
        )
        .unwrap();
        let value = FontValue {
            inner: file.clone(),
        };
        assert_eq!(font((FILE_KIND, FontPayload::Loaded(value))).unwrap(), file);
    }
}
