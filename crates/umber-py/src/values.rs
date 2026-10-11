//! Extraction of data arguments: float64 buffers and lists of names.
//!
//! The Python layer converts every data argument before the native call, to a one-dimensional,
//! native-endian float64 array or to a `list` of `str`. These functions only read those two
//! forms; anything else is an internal `TypeError`. All checks on the values themselves
//! (finiteness, lengths, duplicates) belong to the core.

use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyString};
use umber_core::AxisData;

use crate::errors::internal_type;

/// Copies a one-dimensional, native-endian float64 buffer, such as a NumPy `float64` array, into
/// a vector. Any layout is accepted, so a strided view is copied in logical order.
#[allow(
    dead_code,
    reason = "the figure methods convert data arguments through it"
)]
pub(crate) fn floats(obj: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    let unexpected = || {
        internal_type(format!(
            "expected a one-dimensional float64 buffer, got {}",
            type_name(obj)
        ))
    };
    let buffer = PyBuffer::<f64>::get(obj).map_err(|_| unexpected())?;
    if buffer.dimensions() != 1 || !is_native_f64(buffer.format().to_bytes()) {
        return Err(unexpected());
    }
    buffer.to_vec(obj.py())
}

/// Copies a `list` of `str` into a vector. A bare `str`, a tuple or a list holding anything but
/// strings is refused.
#[allow(
    dead_code,
    reason = "the figure methods convert name arguments through it"
)]
pub(crate) fn names(obj: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let list = obj
        .cast::<PyList>()
        .map_err(|_| internal_type(format!("expected a list of str, got {}", type_name(obj))))?;
    list.iter()
        .map(|item| {
            let text = item.cast::<PyString>().map_err(|_| {
                internal_type(format!(
                    "expected a list of str, got an item of {}",
                    type_name(&item)
                ))
            })?;
            Ok(text.to_str()?.to_owned())
        })
        .collect()
}

/// Converts x values: a `list` becomes [`AxisData::Categories`], anything else is read as a
/// float64 buffer and becomes [`AxisData::Numbers`].
#[allow(
    dead_code,
    reason = "the figure methods convert x arguments through it"
)]
pub(crate) fn axis_data(obj: &Bound<'_, PyAny>) -> PyResult<AxisData> {
    if obj.is_instance_of::<PyList>() {
        names(obj).map(AxisData::Categories)
    } else {
        floats(obj).map(AxisData::Numbers)
    }
}

/// Whether a buffer format string describes one native-endian `f64`. Only native byte order is
/// accepted: the Python layer converts other byte orders before the call.
fn is_native_f64(format: &[u8]) -> bool {
    match format {
        [b'd'] | [b'@' | b'=', b'd'] => true,
        [b'<', b'd'] => cfg!(target_endian = "little"),
        [b'>' | b'!', b'd'] => cfg!(target_endian = "big"),
        _ => false,
    }
}

fn type_name(obj: &Bound<'_, PyAny>) -> String {
    obj.get_type()
        .qualname()
        .map_or_else(|_| "an unknown type".to_owned(), |name| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_native_endian_doubles_are_accepted() {
        let (native, foreign): (&[&[u8]], &[&[u8]]) = if cfg!(target_endian = "little") {
            (&[b"<d"], &[b">d", b"!d"])
        } else {
            (&[b">d", b"!d"], &[b"<d"])
        };
        for format in [&b"d"[..], b"@d", b"=d"].iter().chain(native) {
            assert!(is_native_f64(format), "{format:?}");
        }
        for format in [&b"f"[..], b"<f", b"q", b"dd", b"", b"2d"]
            .iter()
            .chain(foreign)
        {
            assert!(!is_native_f64(format), "{format:?}");
        }
    }
}
