//! An embedded interpreter for unit tests that need Python objects.

use std::ffi::CString;
use std::sync::{Mutex, Once, PoisonError};

use pyo3::prelude::*;
use pyo3::types::IntoPyDict;

/// Runs `f` with the embedded interpreter attached, one test at a time, so tests that change
/// module state cannot interleave.
///
/// When the tests run inside a virtual environment (`uv run cargo test`), its packages, such as
/// NumPy and the editable `umber` package, are importable.
pub(crate) fn with_python<R>(f: impl for<'py> FnOnce(Python<'py>) -> R) -> R {
    static SERIAL: Mutex<()> = Mutex::new(());
    static SITE: Once = Once::new();
    // Both locks are taken before attaching: waiting on them while attached would block the
    // thread that holds them from attaching in turn.
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    Python::initialize();
    SITE.call_once(|| {
        if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
            Python::attach(|py| {
                let paths = [("base", &venv), ("platbase", &venv)]
                    .into_py_dict(py)
                    .unwrap();
                let purelib = py
                    .import("sysconfig")
                    .unwrap()
                    .call_method1("get_path", ("purelib", "venv", paths))
                    .unwrap();
                py.import("site")
                    .unwrap()
                    .call_method1("addsitedir", (purelib,))
                    .unwrap();
            });
        }
    });
    Python::attach(f)
}

/// Evaluates one Python expression, with `numpy` imported as `np` and `ctypes` available.
pub(crate) fn eval<'py>(py: Python<'py>, expression: &str) -> Bound<'py, PyAny> {
    let globals = [
        ("np", py.import("numpy").unwrap()),
        ("ctypes", py.import("ctypes").unwrap()),
    ]
    .into_py_dict(py)
    .unwrap();
    let code = CString::new(expression).unwrap();
    py.eval(&code, Some(&globals), None).unwrap()
}
