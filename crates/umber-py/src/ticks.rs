//! Tick formats: the validator behind `umber.TickFormat` and the conversion of a format argument
//! to a [`TickFormat`].
//!
//! A tick format argument of an axes method travels as a tuple `(kind, decimals, prefix, suffix)`:
//!
//! | kind | meaning | decimals |
//! |---|---|---|
//! | 0 | `TickFormat.AUTO` | unused (0) |
//! | 1 | `TickFormat.fixed(decimals)` | the decimals |
//! | 2 | `TickFormat.percent(decimals)` | the decimals |
//! | 3 | `TickFormat.scientific()` | unused (0) |
//!
//! The prefix and the suffix apply to every kind. All bounds are checked by the core.

use pyo3::prelude::*;
use umber_core::TickFormat;

use crate::errors::{core_error, internal_value};

/// The `kind` of a tick format argument for `TickFormat.AUTO`.
const AUTO: i64 = 0;
/// The `kind` of a tick format argument for `TickFormat.fixed()`.
const FIXED: i64 = 1;
/// The `kind` of a tick format argument for `TickFormat.percent()`.
const PERCENT: i64 = 2;
/// The `kind` of a tick format argument for `TickFormat.scientific()`.
const SCIENTIFIC: i64 = 3;

/// A tick format argument `(kind, decimals, prefix, suffix)`, as the module documentation
/// describes.
pub(crate) type TickFormatArg = (i64, i64, String, String);

/// Checks a tick format argument without using it: the decimals of `TickFormat.fixed()` and
/// `TickFormat.percent()`, and the text of `TickFormat.affix()`.
#[pyfunction]
pub(crate) fn check_tick_format(format: TickFormatArg) -> PyResult<()> {
    tick_format(format).map(drop)
}

/// Converts a tick format argument to a [`TickFormat`]. An unknown kind is an internal
/// `ValueError`.
pub(crate) fn tick_format((kind, decimals, prefix, suffix): TickFormatArg) -> PyResult<TickFormat> {
    let style = match kind {
        AUTO => TickFormat::AUTO,
        FIXED => TickFormat::fixed(decimals).map_err(core_error)?,
        PERCENT => TickFormat::percent(decimals).map_err(core_error)?,
        SCIENTIFIC => TickFormat::scientific(),
        _ => return Err(internal_value(format!("{kind} is not a tick format kind"))),
    };
    style.affix(&prefix, &suffix).map_err(core_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(kind: i64, decimals: i64, prefix: &str, suffix: &str) -> TickFormatArg {
        (kind, decimals, prefix.to_owned(), suffix.to_owned())
    }

    #[test]
    fn tick_format_arguments_convert_to_core_formats() {
        assert_eq!(tick_format(arg(0, 0, "", "")).unwrap(), TickFormat::AUTO);
        assert_eq!(
            tick_format(arg(1, 2, "$", "")).unwrap(),
            TickFormat::fixed(2).unwrap().affix("$", "").unwrap()
        );
        assert_eq!(
            tick_format(arg(2, 1, "", " pts")).unwrap(),
            TickFormat::percent(1).unwrap().affix("", " pts").unwrap()
        );
        assert_eq!(
            tick_format(arg(3, 0, "", "")).unwrap(),
            TickFormat::scientific()
        );
    }
}
