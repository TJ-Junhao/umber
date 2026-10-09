//! PyO3 bindings that expose umber-core to Python as `umber._umber`.

use pyo3::prelude::*;

/// Native extension module behind the `umber` Python package. Private: use `umber` instead.
#[pymodule]
fn _umber(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", umber_core::VERSION)?;
    Ok(())
}
