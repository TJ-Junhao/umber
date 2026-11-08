//! The Delaunay triangulation of points, for `ub.delaunay()`.

use pyo3::prelude::*;
use pyo3::types::PyByteArray;

use crate::errors::core_error;
use crate::values::floats;

/// The Delaunay triangles of the points `(x[i], y[i])`, both float64 buffers, as the bytes of
/// int64 indices in native byte order, three per triangle, as `umber_core::delaunay` returns
/// them. The Python layer views them as an array shaped `(n, 3)`.
#[pyfunction]
pub(crate) fn delaunay<'py>(
    py: Python<'py>,
    x: &Bound<'py, PyAny>,
    y: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyByteArray>> {
    let (x, y) = (floats(x)?, floats(y)?);
    let triangles = umber_core::delaunay(&x, &y).map_err(core_error)?;
    let bytes: Vec<u8> = triangles
        .iter()
        .flatten()
        .flat_map(|index| index.to_ne_bytes())
        .collect();
    Ok(PyByteArray::new(py, &bytes))
}
