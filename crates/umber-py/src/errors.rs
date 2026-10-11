//! Mapping of `umber_core::Error` to Python exceptions, and the internal errors of the binding.
//!
//! Errors that umber reports on purpose carry two instance attributes, `_umber_parameter` and
//! `_umber_hint` (each a `str` or `None`), which the Python layer turns into notes. Internal
//! errors, raised when the Python layer passes a value the binding cannot convert, carry
//! neither: they signal a bug in umber, not a mistake by the caller.

use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyType;

/// The Python module that defines umber's exception classes.
const ERRORS_MODULE: &str = "umber._errors";

static DATA_ERROR: PyOnceLock<Py<PyType>> = PyOnceLock::new();
static STYLE_ERROR: PyOnceLock<Py<PyType>> = PyOnceLock::new();
static USAGE_ERROR: PyOnceLock<Py<PyType>> = PyOnceLock::new();

/// Converts a core error into the Python exception the API documents.
///
/// `Data`, `Style` and `Usage` become `umber.DataError`, `umber.StyleError` and
/// `umber.UsageError`. The message is the error's `Display` text (`"{context}: {message}"`), and
/// the parameter and hint are stored in `_umber_parameter` and `_umber_hint`. `Io` becomes the
/// matching `OSError` subclass without either attribute. A kind added to the core later becomes
/// an internal `RuntimeError` until it is mapped here.
pub(crate) fn core_error(err: umber_core::Error) -> PyErr {
    use umber_core::Error;

    let class = match err {
        Error::Data { .. } => (&DATA_ERROR, "DataError"),
        Error::Style { .. } => (&STYLE_ERROR, "StyleError"),
        Error::Usage { .. } => (&USAGE_ERROR, "UsageError"),
        Error::Io(io) => return PyErr::from(io),
        _ => return PyRuntimeError::new_err(format!("internal error: {err}")),
    };
    Python::attach(|py| {
        reported(py, class, &err.to_string(), err.parameter(), err.hint())
            .unwrap_or_else(|failure| failure)
    })
}

/// An instance of the exception class `class` with `message` and both detail attributes set.
fn reported(
    py: Python<'_>,
    (cell, name): (&PyOnceLock<Py<PyType>>, &str),
    message: &str,
    parameter: Option<&str>,
    hint: Option<&str>,
) -> PyResult<PyErr> {
    let value = cell.import(py, ERRORS_MODULE, name)?.call1((message,))?;
    value.setattr("_umber_parameter", parameter)?;
    value.setattr("_umber_hint", hint)?;
    Ok(PyErr::from_value(value))
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
