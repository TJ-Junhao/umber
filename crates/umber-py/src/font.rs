//! Font arguments: a kind and either a bundled index or an immutable parsed font value.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use umber_core::{BundledFont, Font, Weight};

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
    paths: Option<FilePaths>,
}

#[derive(Clone)]
struct FilePaths {
    regular: String,
    bold: Option<String>,
}

type FileSource<'py> = (
    Bound<'py, PyBytes>,
    Option<Bound<'py, PyBytes>>,
    String,
    Option<String>,
);

#[pymethods]
impl FontValue {
    fn family_name(&self) -> &str {
        self.inner.family_name()
    }

    fn bold_fallback(&self) -> bool {
        self.inner.bold_fallback()
    }

    fn file_paths(&self) -> Option<(String, Option<String>)> {
        self.paths
            .as_ref()
            .map(|paths| (paths.regular.clone(), paths.bold.clone()))
    }

    fn file_source<'py>(&self, py: Python<'py>) -> Option<FileSource<'py>> {
        let Font::File(files) = &self.inner else {
            return None;
        };
        let paths = self.paths.as_ref()?;
        let regular = files.bytes(Weight::Regular)?;
        let saved = PyBytes::new(py, regular);
        let bold = files.bytes(Weight::SemiBold).map(|bytes| {
            if std::ptr::eq(regular, bytes) {
                saved.clone()
            } else {
                PyBytes::new(py, bytes)
            }
        });
        Some((saved, bold, paths.regular.clone(), paths.bold.clone()))
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

/// Checks file-font options before Python reads either file.
#[pyfunction]
pub(crate) fn check_file_bold_fallback(has_bold: bool, bold_fallback: bool) -> PyResult<()> {
    umber_core::fonts::check_file_bold_fallback(has_bold, bold_fallback).map_err(core_error)
}

/// Parses bytes at construction, before a font is assigned to a theme.
#[pyfunction]
pub(crate) fn font_from_files(
    regular: &[u8],
    bold: Option<&[u8]>,
    name: String,
    regular_path: String,
    bold_path: Option<String>,
    bold_fallback: bool,
) -> PyResult<FontValue> {
    check_file_bold_fallback(bold.is_some(), bold_fallback)?;
    Font::from_files(regular.to_vec(), bold.map(<[u8]>::to_vec), name)
        .and_then(|inner| inner.with_bold_fallback(bold_fallback))
        .map(|inner| FontValue {
            inner,
            paths: Some(FilePaths {
                regular: regular_path,
                bold: bold_path,
            }),
        })
        .map_err(core_error)
}

/// Resolves an installed family at construction.
#[pyfunction]
pub(crate) fn font_system(family: &str, bold_fallback: bool) -> PyResult<FontValue> {
    Font::system(family)
        .and_then(|inner| inner.with_bold_fallback(bold_fallback))
        .map(|inner| FontValue { inner, paths: None })
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
                ..
            }),
        )
        | (
            SYSTEM_KIND,
            FontPayload::Loaded(FontValue {
                inner: font @ Font::System(_),
                ..
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
            paths: Some(FilePaths {
                regular: "Inter.ttf".into(),
                bold: None,
            }),
        };
        assert_eq!(font((FILE_KIND, FontPayload::Loaded(value))).unwrap(), file);
    }
}
