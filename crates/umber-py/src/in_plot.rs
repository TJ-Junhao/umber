//! Positions in the plot: the validator behind `umber.InPlot`.

use pyo3::prelude::*;
use umber_core::InPlot;

use crate::errors::core_error;

/// Checks the fractions of a position for `InPlot()`.
#[pyfunction]
pub(crate) fn check_in_plot(x: f64, y: f64) -> PyResult<()> {
    InPlot::new(x, y).map(drop).map_err(core_error)
}
