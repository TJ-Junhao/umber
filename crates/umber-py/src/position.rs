//! Positions: the validators behind `umber.InPlot` and `umber.Between`.

use pyo3::prelude::*;
use umber_core::{Between, InPlot};

use crate::errors::core_error;

/// Checks the fractions of a position for `InPlot()`.
#[pyfunction]
pub(crate) fn check_in_plot(x: f64, y: f64) -> PyResult<()> {
    InPlot::new(x, y).map(drop).map_err(core_error)
}

/// Checks the two category names of a position for `Between()`.
#[pyfunction]
pub(crate) fn check_between(first: String, second: String) -> PyResult<()> {
    between((first, second)).map(drop)
}

/// The core position between the categories `names`, first then second.
pub(crate) fn between((first, second): (String, String)) -> PyResult<Between> {
    Between::new(first, second).map_err(core_error)
}
