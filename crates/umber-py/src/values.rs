//! Extraction of data arguments: float64 buffers, lists of names and int64 buffers of times.
//!
//! The Python layer converts every data argument before the native call, to a one-dimensional,
//! aligned, native-endian float64 array, to a `list` of `str`, or, for times, to such an int64
//! array of microseconds since 1970-01-01T00:00 with the smallest int64 for a missing time
//! ([`umber_core::MISSING_TIME`]). These functions only read those forms and convert nothing
//! else; any other input is an internal `TypeError`. All checks on the values themselves
//! (finiteness, lengths, duplicates, the range of times) belong to the core.

use std::iter;
use std::sync::Arc;

use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::{PyFloat, PyList, PyString};
use umber_core::{AxisData, AxisValue, Instant, times_from_micros};

use crate::errors::{core_error, internal_state, internal_type};

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
pub(crate) fn floats(obj: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    float_buffer(obj)?.to_vec(obj.py())
}

/// Copies a float64 buffer into a new shared slice, in logical order, for values the core
/// stores as they are: the slice is allocated once, at its final size, and the buffer is copied
/// into it once. Accepts the buffers [`floats`] accepts, with the same errors.
///
/// Collecting `n` zeros from `repeat_n`, whose length is known exactly, allocates the slice
/// once, with room for the reference counts. Reading the buffer with `PyBuffer::to_vec` and
/// then converting the vector with `Arc::from` would allocate twice and copy the values twice.
///
/// Keep it that way: build the `Arc` straight from the buffer, never through [`floats`] or any
/// other `Vec`, which would add a second copy as large as the data. No test counts the
/// allocations here, so a second copy would not fail any test.
pub(crate) fn shared_floats(obj: &Bound<'_, PyAny>) -> PyResult<Arc<[f64]>> {
    let buffer = float_buffer(obj)?;
    let mut values: Arc<[f64]> = iter::repeat_n(0.0, buffer.item_count()).collect();
    let slice = Arc::get_mut(&mut values)
        .ok_or_else(|| internal_state("a new shared slice is already shared"))?;
    buffer.copy_to_slice(obj.py(), slice)?;
    Ok(values)
}

/// The buffer of `obj`, checked against the contract of [`floats`].
fn float_buffer(obj: &Bound<'_, PyAny>) -> PyResult<PyBuffer<f64>> {
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
    Ok(buffer)
}

/// Copies a `list` of `str` into a vector. A bare `str`, a tuple or a list holding anything but
/// strings is refused.
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
pub(crate) fn axis_data(obj: &Bound<'_, PyAny>) -> PyResult<AxisData> {
    if obj.is_instance_of::<PyList>() {
        names(obj).map(AxisData::Categories)
    } else {
        floats(obj).map(AxisData::Numbers)
    }
}

/// Converts the values of a series along one axis, named `name` in the messages of `context`:
/// an int64 buffer holds times and becomes [`AxisData::Times`], checked by the core; anything
/// else converts as [`axis_data`] converts it.
pub(crate) fn series_data(
    context: &'static str,
    name: &'static str,
    obj: &Bound<'_, PyAny>,
) -> PyResult<AxisData> {
    if is_int64_buffer(obj) {
        times(context, name, obj).map(AxisData::Times)
    } else {
        axis_data(obj)
    }
}

/// Converts times, named `name` in the messages of `context`: an int64 buffer of
/// microseconds, read as [`micros`] reads it, with the range checked by the core.
pub(crate) fn times(
    context: &'static str,
    name: &'static str,
    obj: &Bound<'_, PyAny>,
) -> PyResult<Vec<Option<Instant>>> {
    times_from_micros(context, name, &micros(obj)?).map_err(core_error)
}

/// Copies an int64 buffer of microseconds, such as a NumPy `int64` array, into a vector, in
/// logical order, with the contract of [`floats`] for an int64 element: one-dimensional,
/// aligned, and of format `q` or `l` (eight bytes) in native byte order, with or without `@` or
/// `=`. Anything else is an internal `TypeError`.
pub(crate) fn micros(obj: &Bound<'_, PyAny>) -> PyResult<Vec<i64>> {
    let unexpected = || {
        internal_type(format!(
            "expected a one-dimensional int64 buffer, got {}",
            type_name(obj)
        ))
    };
    let buffer = PyBuffer::<i64>::get(obj).map_err(|_| unexpected())?;
    let aligned_strides = buffer.strides().iter().all(|stride| stride % 8 == 0);
    if buffer.dimensions() != 1 || !aligned_strides || !is_native_i64(buffer.format().to_bytes()) {
        return Err(unexpected());
    }
    buffer.to_vec(obj.py())
}

/// Whether `obj` exports a one-dimensional int64 buffer that [`micros`] reads.
fn is_int64_buffer(obj: &Bound<'_, PyAny>) -> bool {
    PyBuffer::<i64>::get(obj)
        .is_ok_and(|buffer| buffer.dimensions() == 1 && is_native_i64(buffer.format().to_bytes()))
}

/// Whether a buffer format string is one eight-byte signed integer in native byte order, with
/// no explicit byte order: `q`, or `l` where a C `long` has eight bytes, with or without `@` or
/// `=`.
fn is_native_i64(format: &[u8]) -> bool {
    let code = match format {
        [code] | [b'@' | b'=', code] => *code,
        _ => return false,
    };
    code == b'q' || (code == b'l' && std::mem::size_of::<std::os::raw::c_long>() == 8)
}

/// Converts the x of a vertical reference line: a `str` becomes [`AxisValue::Category`], a
/// `float` [`AxisValue::Number`]. The Python layer passes the one that matches the x axis; any
/// other object is an internal `TypeError`.
pub(crate) fn axis_value(obj: &Bound<'_, PyAny>) -> PyResult<AxisValue> {
    if let Ok(text) = obj.cast::<PyString>() {
        let name = text.to_str().map_err(|_| {
            internal_type("a name is not valid UTF-8: it contains a lone surrogate")
        })?;
        return Ok(AxisValue::Category(name.to_owned()));
    }
    obj.cast::<PyFloat>()
        .map(|number| AxisValue::Number(number.value()))
        .map_err(|_| internal_type(format!("expected a str or a float, got {}", type_name(obj))))
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
    fn shared_floats_reads_what_floats_reads_into_an_unshared_slice() {
        with_python(|py| {
            let bits = |values: &[f64]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
            for expression in [
                DATA.to_owned(),
                format!("{DATA}[::-1]"),
                "np.arange(6.0).reshape(2, 3)[:, 1]".to_owned(),
                "np.array([np.nan, -0.0, 1e300])".to_owned(),
                "np.empty(0)".to_owned(),
            ] {
                let obj = eval(py, &expression);
                let shared = shared_floats(&obj).unwrap();
                assert_eq!(Arc::strong_count(&shared), 1, "{expression}");
                assert_eq!(bits(&shared), bits(&floats(&obj).unwrap()), "{expression}");
            }
            for expression in ["np.zeros((2, 2))", "np.zeros(3, dtype='>f8')", "[1.0, 2.0]"] {
                let obj = eval(py, expression);
                let shared = internal_type_message(&shared_floats(&obj).unwrap_err(), py);
                let copied = internal_type_message(&floats(&obj).unwrap_err(), py);
                assert_eq!(shared, copied, "{expression}");
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

    #[test]
    fn series_data_reads_int64_buffers_as_times() {
        with_python(|py| {
            let times = series_data(
                "Axes.line()",
                "x",
                &eval(py, "np.array([0, 1], dtype=np.int64)"),
            )
            .unwrap();
            assert_eq!(
                times,
                AxisData::Times(vec![
                    Some(Instant::from_micros(0).unwrap()),
                    Some(Instant::from_micros(1).unwrap())
                ])
            );
            let missing = format!("np.array([{}], dtype=np.int64)", umber_core::MISSING_TIME);
            assert_eq!(
                series_data("Axes.line()", "x", &eval(py, &missing)).unwrap(),
                AxisData::Times(vec![None])
            );
            // Strided int64 views are read in logical order.
            let strided = series_data(
                "Axes.line()",
                "x",
                &eval(py, "np.array([5, 6, 7, 8], dtype=np.int64)[::2]"),
            )
            .unwrap();
            assert_eq!(
                strided,
                AxisData::Times(vec![
                    Some(Instant::from_micros(5).unwrap()),
                    Some(Instant::from_micros(7).unwrap())
                ])
            );
            // Other buffers and lists convert as before.
            assert_eq!(
                series_data("Axes.line()", "y", &eval(py, DATA)).unwrap(),
                AxisData::Numbers(vec![1.5, -2.0, 3.25])
            );
            // A time out of range is the core's error.
            let beyond = "np.array([0, 2**62], dtype=np.int64)";
            assert!(series_data("Axes.line()", "x", &eval(py, beyond)).is_err());
        });
    }

    #[test]
    fn micros_refuses_other_buffers() {
        with_python(|py| {
            for expression in [
                "np.zeros(2, dtype=np.int32)",
                "np.zeros((2, 2), dtype=np.int64)",
                "np.zeros(2, dtype='>i8')",
                "np.zeros(2)",
                "[1, 2]",
            ] {
                let err = micros(&eval(py, expression)).unwrap_err();
                let message = internal_type_message(&err, py);
                assert!(
                    message.starts_with("internal error: expected a one-dimensional int64"),
                    "{expression}: {message}"
                );
            }
        });
    }

    #[test]
    fn axis_value_reads_a_str_as_a_category_and_a_float_as_a_number() {
        with_python(|py| {
            assert_eq!(
                axis_value(&eval(py, "'Q3'")).unwrap(),
                AxisValue::Category("Q3".into())
            );
            assert_eq!(
                axis_value(&eval(py, "2.5")).unwrap(),
                AxisValue::Number(2.5)
            );
            for refused in ["3", "None", "[1.0]", "b'x'"] {
                let err = axis_value(&eval(py, refused)).unwrap_err();
                internal_type_message(&err, py);
            }
            let err = axis_value(&eval(py, "'\\udfff'")).unwrap_err();
            internal_type_message(&err, py);
        });
    }
}
