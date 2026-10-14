//! Mapping of `umber_core::Error` to Python exceptions, and the internal errors of the binding.
//!
//! Errors that umber reports on purpose carry two instance attributes, `_umber_parameter` and
//! `_umber_hint` (each a `str` or `None`), which the Python layer turns into notes. Internal
//! errors, raised when the Python layer passes a value the binding cannot convert, carry
//! neither: they signal a bug in umber, not a mistake by the caller.

use pyo3::exceptions::{PyOSError, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;

/// The Python module that defines umber's exception classes.
const ERRORS_MODULE: &str = "umber._errors";

/// Converts a core error into the Python exception the API documents.
///
/// `Data`, `Style` and `Usage` become `umber.DataError`, `umber.StyleError` and
/// `umber.UsageError`. The message is the error's `Display` text (`"{context}: {message}"`), and
/// the parameter and hint are stored in `_umber_parameter` and `_umber_hint`. `Io` becomes an
/// `OSError` without either attribute (see [`os_error`]). A kind added to the core later becomes
/// an internal `RuntimeError` until it is mapped here.
///
/// The exception class is looked up in the current interpreter's `umber._errors` module on
/// every call and never cached. A reloaded module, or the module of another interpreter that
/// shares this extension (a subinterpreter with the legacy shared state), therefore gets errors
/// of its own classes, so `except umber.StyleError` catches them. A failed lookup is raised in
/// place of the error.
pub(crate) fn core_error(err: umber_core::Error) -> PyErr {
    use umber_core::Error;

    let class = match err {
        Error::Data { .. } => "DataError",
        Error::Style { .. } => "StyleError",
        Error::Usage { .. } => "UsageError",
        Error::Io(io) => return os_error(io),
        _ => return PyRuntimeError::new_err(format!("internal error: {err}")),
    };
    Python::attach(|py| {
        reported(py, class, &err.to_string(), err.parameter(), err.hint())
            .unwrap_or_else(|failure| failure)
    })
}

/// An instance of the class `name` of `umber._errors` with `message` and both detail attributes
/// set.
fn reported(
    py: Python<'_>,
    name: &str,
    message: &str,
    parameter: Option<&str>,
    hint: Option<&str>,
) -> PyResult<PyErr> {
    let class = py.import(ERRORS_MODULE)?.getattr(name)?;
    let value = class.call1((message,))?;
    value.setattr("_umber_parameter", parameter)?;
    value.setattr("_umber_hint", hint)?;
    Ok(PyErr::from_value(value))
}

/// Converts an I/O error to an `OSError`.
///
/// An error from the operating system keeps its code: `OSError(errno, strerror)` on Unix, and
/// `OSError(code, strerror, None, winerror)` on Windows, where Python derives `errno` from the
/// Windows code. Python then picks the subclass, such as `FileNotFoundError`, from the code. An
/// error that wraps a Python exception returns that exception, and any other error becomes the
/// `OSError` subclass matching its kind, with its message.
fn os_error(io: std::io::Error) -> PyErr {
    let Some(code) = io.raw_os_error() else {
        return PyErr::from(io);
    };
    let text = io.to_string();
    let strerror = text
        .strip_suffix(&format!(" (os error {code})"))
        .unwrap_or(&text);
    Python::attach(|py| {
        let class = py.get_type::<PyOSError>();
        let value = if cfg!(windows) {
            class.call1((code, strerror, py.None(), code))
        } else {
            class.call1((code, strerror))
        };
        value.map_or_else(|failure| failure, PyErr::from_value)
    })
}

/// An internal `TypeError`: the Python layer passed an object of a type the binding does not
/// take. It carries no detail attributes.
pub(crate) fn internal_type(message: impl Into<String>) -> PyErr {
    PyTypeError::new_err(format!("internal error: {}", message.into()))
}

/// An internal `ValueError`: the Python layer passed a code the binding does not know. It
/// carries no detail attributes.
pub(crate) fn internal_value(message: impl Into<String>) -> PyErr {
    PyValueError::new_err(format!("internal error: {}", message.into()))
}

/// An internal `RuntimeError`: a binding invariant failed, such as a new shared slice already
/// being shared. It carries no detail attributes. Missing axes instead use the core's
/// `UsageError` from `axes_at`, mapped by `core_error`.
pub(crate) fn internal_state(message: impl Into<String>) -> PyErr {
    PyRuntimeError::new_err(format!("internal error: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use pyo3::exceptions::PyFileNotFoundError;
    use umber_core::Error;

    use super::*;
    use crate::test_support::with_python;

    fn errors_class<'py>(py: Python<'py>, name: &str) -> Bound<'py, PyAny> {
        py.import(ERRORS_MODULE).unwrap().getattr(name).unwrap()
    }

    #[test]
    fn reported_errors_get_their_class_message_and_details() {
        with_python(|py| {
            for (err, class) in [
                (
                    Error::Data {
                        context: "Axes.line()",
                        message: "y[17] is inf; values must be finite.".into(),
                        parameter: Some("y"),
                        hint: Some("use NaN to leave a gap in the line."),
                    },
                    "DataError",
                ),
                (
                    Error::Style {
                        context: "Color.hex()",
                        message: "\"#12\" is not a hex color; use #RRGGBB or #RRGGBBAA.".into(),
                        parameter: Some("code"),
                        hint: None,
                    },
                    "StyleError",
                ),
                (
                    Error::Usage {
                        context: "Figure.add_axes()",
                        message: "this figure already has axes; one axes per figure.".into(),
                        parameter: None,
                        hint: Some("create a new Figure for another chart."),
                    },
                    "UsageError",
                ),
            ] {
                let text = err.to_string();
                let (parameter, hint) = (err.parameter(), err.hint());
                let value = core_error(err).into_value(py).into_bound(py);
                assert!(value.get_type().is(errors_class(py, class)), "{class}");
                assert_eq!(value.to_string(), text);
                let stored: Option<String> = value
                    .getattr("_umber_parameter")
                    .unwrap()
                    .extract()
                    .unwrap();
                assert_eq!(stored.as_deref(), parameter);
                let stored: Option<String> =
                    value.getattr("_umber_hint").unwrap().extract().unwrap();
                assert_eq!(stored.as_deref(), hint);
            }
        });
    }

    #[test]
    fn a_failed_class_lookup_is_raised_in_place_of_the_error() {
        with_python(|py| {
            let module = py.import(ERRORS_MODULE).unwrap();
            let class = module.getattr("StyleError").unwrap();
            module.delattr("StyleError").unwrap();
            let err = core_error(umber_core::Alpha::new(2.0).unwrap_err());
            module.setattr("StyleError", class).unwrap();
            assert!(
                err.get_type(py)
                    .is(py.get_type::<pyo3::exceptions::PyAttributeError>())
            );
        });
    }

    /// Python's own `OSError` for a raw OS error code, built the way `os_error` documents.
    fn python_os_error<'py>(py: Python<'py>, code: i32) -> Bound<'py, PyAny> {
        let class = py.get_type::<PyOSError>();
        if cfg!(windows) {
            class.call1((code, "", py.None(), code)).unwrap()
        } else {
            class.call1((code, "")).unwrap()
        }
    }

    #[test]
    fn os_errors_keep_their_code_and_subclass() {
        with_python(|py| {
            for code in [2, 13, 17, 22] {
                let io = std::io::Error::from_raw_os_error(code);
                let text = io.to_string();
                let value = core_error(Error::Io(io)).into_value(py).into_bound(py);
                let expected = python_os_error(py, code);
                assert!(value.get_type().is(expected.get_type()), "{code}");
                for attribute in ["errno", "winerror"] {
                    let attribute_of = |v: &Bound<'_, PyAny>| {
                        v.getattr(attribute)
                            .map(|a| a.to_string())
                            .unwrap_or_default()
                    };
                    assert_eq!(
                        attribute_of(&value),
                        attribute_of(&expected),
                        "{code} {attribute}"
                    );
                }
                let strerror = value.getattr("strerror").unwrap().to_string();
                assert!(
                    !strerror.is_empty() && text.starts_with(&strerror),
                    "{code}: {strerror}"
                );
                assert!(!value.hasattr("_umber_parameter").unwrap());
            }
        });
    }

    #[cfg(unix)]
    #[test]
    fn os_errors_map_to_the_matching_subclass_on_unix() {
        use pyo3::exceptions::{PyFileExistsError, PyPermissionError};

        with_python(|py| {
            let cases = [
                (2, py.get_type::<PyFileNotFoundError>()),
                (13, py.get_type::<PyPermissionError>()),
                (17, py.get_type::<PyFileExistsError>()),
                (22, py.get_type::<PyOSError>()),
            ];
            for (code, class) in cases {
                let io = std::io::Error::from_raw_os_error(code);
                let value = core_error(Error::Io(io)).into_value(py).into_bound(py);
                assert!(value.get_type().is(&class), "{code}");
                let errno: i32 = value.getattr("errno").unwrap().extract().unwrap();
                assert_eq!(errno, code);
            }
        });
    }

    #[test]
    fn io_errors_without_a_code_map_by_kind_and_wrapped_python_errors_pass_through() {
        with_python(|py| {
            let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing.png");
            let value = core_error(Error::Io(io)).into_value(py).into_bound(py);
            assert!(value.get_type().is(py.get_type::<PyFileNotFoundError>()));
            assert_eq!(value.to_string(), "missing.png");

            let wrapped = PyValueError::new_err("from Python");
            let original = wrapped.value(py).clone();
            let err = core_error(Error::Io(std::io::Error::other(wrapped)));
            assert!(err.value(py).is(&original));
        });
    }
}
