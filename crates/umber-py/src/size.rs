//! Sizes: the validators behind `umber.Size` and the conversion of a size argument to a
//! [`SizeSpec`].
//!
//! A size argument of a mark method travels as a tuple `(kind, value)`:
//!
//! | kind | meaning | value |
//! |---|---|---|
//! | 0 | `Size.AUTO` | unused (0.0) |
//! | 1 | `Size.times(factor)` | the factor |
//! | 2 | `Size.pt(points)` | the size in points |

use pyo3::prelude::*;
use umber_core::SizeSpec;

use crate::errors::{core_error, internal_value};

/// The `kind` of a size argument for `Size.AUTO`.
const AUTO: i64 = 0;
/// The `kind` of a size argument for `Size.times()`.
const TIMES: i64 = 1;
/// The `kind` of a size argument for `Size.pt()`.
const POINTS: i64 = 2;

/// A size argument `(kind, value)`, as the module documentation describes.
pub(crate) type SizeArg = (i64, f64);

/// Checks a factor for `Size.times()`.
#[pyfunction]
pub(crate) fn check_size_times(factor: f64) -> PyResult<()> {
    SizeSpec::times(factor).map(drop).map_err(core_error)
}

/// Checks a size in points for `Size.pt()`.
#[pyfunction]
pub(crate) fn check_size_points(points: f64) -> PyResult<()> {
    SizeSpec::pt(points).map(drop).map_err(core_error)
}

/// Converts a size argument `(kind, value)` to a [`SizeSpec`]. An unknown kind is an internal
/// `ValueError`.
pub(crate) fn size_spec((kind, value): SizeArg) -> PyResult<SizeSpec> {
    match kind {
        AUTO => Ok(SizeSpec::Auto),
        TIMES => SizeSpec::times(value).map_err(core_error),
        POINTS => SizeSpec::pt(value).map_err(core_error),
        _ => Err(internal_value(format!("{kind} is not a size kind"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_arguments_convert_to_core_sizes() {
        assert_eq!(size_spec((0, 0.0)).unwrap(), SizeSpec::Auto);
        assert_eq!(size_spec((1, 2.0)).unwrap(), SizeSpec::times(2.0).unwrap());
        assert_eq!(size_spec((2, 6.0)).unwrap(), SizeSpec::pt(6.0).unwrap());
    }
}
