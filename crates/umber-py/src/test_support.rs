//! Private Python test hooks and an embedded interpreter for Rust unit tests.

#[cfg(test)]
use std::ffi::CString;
#[cfg(test)]
use std::sync::{Mutex, Once, PoisonError};

use pyo3::prelude::*;
#[cfg(test)]
use pyo3::types::IntoPyDict;

// Build the production implementation into the extension without making it a public core API.
#[path = "../../umber-core/src/stats.rs"]
mod stats;

/// Quartiles, whiskers and sorted outliers, in the order used by the Python test hook.
type BoxStatsTuple = (f64, f64, f64, f64, f64, Vec<f64>);

/// Computes box statistics from a float64 buffer for tests, not the public Python API.
#[pyfunction]
pub(crate) fn box_stats(values: &Bound<'_, PyAny>) -> PyResult<Option<BoxStatsTuple>> {
    Ok(stats::box_stats(&crate::values::floats(values)?).map(|s| {
        (
            s.q1,
            s.median,
            s.q3,
            s.low_whisker,
            s.high_whisker,
            s.outliers,
        )
    }))
}

/// Positions and Gaussian densities from the shared production statistics implementation.
type DensityGrid = Vec<(f64, f64)>;

/// Computes Scott kernel density from a float64 buffer for tests, not the public Python API.
#[pyfunction]
pub(crate) fn kde_scott(values: &Bound<'_, PyAny>) -> PyResult<Option<DensityGrid>> {
    Ok(stats::kde_scott(&crate::values::floats(values)?, 128))
}

/// Runs `f` with the embedded interpreter attached, one test at a time, so tests that change
/// module state cannot interleave.
///
/// When the tests run inside a virtual environment (`uv run cargo test`), its packages, such as
/// NumPy and the editable `umber` package, are importable.
#[cfg(test)]
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
#[cfg(test)]
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
