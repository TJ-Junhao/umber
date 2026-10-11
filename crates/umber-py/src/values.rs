//! Extraction of data arguments: float64 buffers and lists of names.
//!
//! The Python layer converts every data argument before the native call, to a one-dimensional,
//! aligned, native-endian float64 array or to a `list` of `str`. These functions only read those
//! two forms and convert nothing else; any other input is an internal `TypeError`. All checks on
//! the values themselves (finiteness, lengths, duplicates) belong to the core.

use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyString};
use umber_core::AxisData;

use crate::errors::internal_type;

/// Copies a float64 buffer, such as a NumPy `float64` array, into a vector, in logical order.
///
/// The buffer must be one-dimensional, C-contiguous or strided, and aligned: its start and every
/// stride are multiples of 8 bytes. Its format must be `d`, `@d` or `=d` (float64 in native byte
/// order, as NumPy exports it); a format with an explicit byte order, such as the `<d` of a
/// ctypes array, is refused even when it matches the native order. Anything else is an internal
/// `TypeError`. The Python layer meets this contract with
/// `np.require(values, dtype="=f8", requirements=["A"]).view(np.float64)`: `np.require` makes the
/// data aligned float64 in native order, and the view drops an explicit byte order that NumPy
/// keeps for arrays made from such buffers, without copying.
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
    // `get` also refuses a buffer whose start is not aligned for `f64`.
    let buffer = PyBuffer::<f64>::get(obj).map_err(|_| unexpected())?;
    let aligned_strides = buffer.strides().iter().all(|stride| stride % 8 == 0);
    if buffer.dimensions() != 1 || !aligned_strides || !is_native_f64(buffer.format().to_bytes()) {
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
            let text = text.to_str().map_err(|_| {
                internal_type("a name is not valid UTF-8: it contains a lone surrogate")
            })?;
            Ok(text.to_owned())
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

/// Whether a buffer format string is `d`, `@d` or `=d`: one `f64` in native byte order, with no
/// explicit byte order.
fn is_native_f64(format: &[u8]) -> bool {
    matches!(format, [b'd'] | [b'@' | b'=', b'd'])
}

fn type_name(obj: &Bound<'_, PyAny>) -> String {
    obj.get_type()
        .qualname()
        .map_or_else(|_| "an unknown type".to_owned(), |name| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{eval, with_python};

    const DATA: &str = "np.array([1.5, -2.0, 3.25])";

    fn internal_type_message(err: &PyErr, py: Python<'_>) -> String {
        assert!(
            err.get_type(py)
                .is(py.get_type::<pyo3::exceptions::PyTypeError>())
        );
        assert!(!err.value(py).hasattr("_umber_parameter").unwrap());
        err.value(py).to_string()
    }

    #[test]
    fn only_formats_without_an_explicit_byte_order_are_accepted() {
        for format in [&b"d"[..], b"@d", b"=d"] {
            assert!(is_native_f64(format), "{format:?}");
        }
        for format in [
            &b"<d"[..],
            b">d",
            b"!d",
            b"f",
            b"<f",
            b"q",
            b"dd",
            b"",
            b"2d",
        ] {
            assert!(!is_native_f64(format), "{format:?}");
        }
    }

    #[test]
    fn floats_copies_contiguous_and_strided_arrays_in_logical_order() {
        with_python(|py| {
            let cases = [
                (DATA.to_owned(), vec![1.5, -2.0, 3.25]),
                (format!("{DATA}[::2]"), vec![1.5, 3.25]),
                (format!("{DATA}[::-1]"), vec![3.25, -2.0, 1.5]),
                (
                    "np.arange(6.0).reshape(2, 3)[:, 1]".to_owned(),
                    vec![1.0, 4.0],
                ),
                ("np.empty(0)".to_owned(), vec![]),
            ];
            for (expression, expected) in cases {
                assert_eq!(
                    floats(&eval(py, &expression)).unwrap(),
                    expected,
                    "{expression}"
                );
            }
        });
    }

    #[test]
    fn floats_refuses_other_buffers_and_objects() {
        with_python(|py| {
            for expression in [
                "np.zeros((2, 2))",
                "np.zeros(3, dtype='>f8')",
                "np.zeros(3, dtype=np.float32)",
                "np.zeros(3, dtype=np.int64)",
                "np.float64(1.0)",
                "[1.0, 2.0]",
                "np.zeros(3, dtype=[('a', 'f8'), ('b', 'u1')])['a']",
            ] {
                let err = floats(&eval(py, expression)).unwrap_err();
                let message = internal_type_message(&err, py);
                assert!(
                    message.starts_with("internal error: expected a one-dimensional float64"),
                    "{expression}: {message}"
                );
            }
        });
    }

    #[test]
    fn unaligned_and_explicitly_ordered_buffers_are_refused_until_converted() {
        with_python(|py| {
            let unaligned =
                format!("np.frombuffer(b'\\0' + {DATA}.tobytes(), dtype=np.float64, offset=1)");
            let ctypes = "(ctypes.c_double * 3)(1.5, -2.0, 3.25)".to_owned();
            assert!(
                !eval(py, &format!("{unaligned}.flags.aligned"))
                    .is_truthy()
                    .unwrap()
            );
            let native = if cfg!(target_endian = "little") {
                "<d"
            } else {
                ">d"
            };
            let format = eval(py, &format!("memoryview({ctypes}).format"));
            assert_eq!(format.to_string(), native);

            let required = |expression: &str| {
                format!("np.require({expression}, dtype='=f8', requirements=['A'])")
            };
            for expression in [&unaligned, &ctypes] {
                let err = floats(&eval(py, expression)).unwrap_err();
                internal_type_message(&err, py);
                let converted = format!("{}.view(np.float64)", required(expression));
                assert_eq!(
                    floats(&eval(py, &converted)).unwrap(),
                    [1.5, -2.0, 3.25],
                    "{converted}"
                );
            }
            // `np.require` alone aligns the data, but an array made from a ctypes buffer keeps the
            // explicit byte order in its format, so only the view makes it acceptable.
            assert_eq!(
                floats(&eval(py, &required(&unaligned))).unwrap(),
                [1.5, -2.0, 3.25]
            );
            let err = floats(&eval(py, &required(&ctypes))).unwrap_err();
            internal_type_message(&err, py);
        });
    }

    #[test]
    fn names_copies_a_list_of_strings() {
        with_python(|py| {
            let names = names(&eval(py, "['North', 'Süd', np.str_('東')]")).unwrap();
            assert_eq!(names, ["North", "Süd", "東"]);
        });
    }

    #[test]
    fn names_refuses_anything_but_a_list_of_strings() {
        with_python(|py| {
            for (expression, expected) in [
                ("'North'", "internal error: expected a list of str, got str"),
                (
                    "('North',)",
                    "internal error: expected a list of str, got tuple",
                ),
                (
                    "['North', 1]",
                    "internal error: expected a list of str, got an item of int",
                ),
                (
                    "['North', '\\ud800']",
                    "internal error: a name is not valid UTF-8: it contains a lone surrogate",
                ),
            ] {
                let err = names(&eval(py, expression)).unwrap_err();
                assert_eq!(internal_type_message(&err, py), expected, "{expression}");
            }
        });
    }

    #[test]
    fn axis_data_reads_lists_as_categories_and_buffers_as_numbers() {
        with_python(|py| {
            let categories = axis_data(&eval(py, "['a', 'b']")).unwrap();
            assert_eq!(
                categories,
                AxisData::Categories(vec!["a".into(), "b".into()])
            );
            let numbers = axis_data(&eval(py, DATA)).unwrap();
            assert_eq!(numbers, AxisData::Numbers(vec![1.5, -2.0, 3.25]));
            let err = axis_data(&eval(py, "['a', '\\udfff']")).unwrap_err();
            internal_type_message(&err, py);
        });
    }
}
