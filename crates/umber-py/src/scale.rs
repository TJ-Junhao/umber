//! Scales: the validator behind `umber.Scale.log()` and the conversion of a scale argument to a
//! [`ScaleKind`].
//!
//! A scale argument travels as two values, `(kind, base)`: kind 0 is linear, 1 is log with the
//! given base, and 2 is category. The base is unused for kinds 0 and 2.

use pyo3::prelude::*;
use umber_core::{LogBase, ScaleKind};

use crate::errors::{core_error, internal_value};

/// The `kind` of a linear scale argument.
const LINEAR: i64 = 0;
/// The `kind` of a log scale argument.
const LOG: i64 = 1;
/// The `kind` of a category scale argument.
const CATEGORY: i64 = 2;

/// Checks a logarithm base for `Scale.log()`.
#[pyfunction]
pub(crate) fn check_log_base(base: f64) -> PyResult<()> {
    LogBase::new(base).map(drop).map_err(core_error)
}

/// Converts a scale argument `(kind, base)` to a [`ScaleKind`]. An unknown kind is an internal
/// `ValueError`.
pub(crate) fn scale_kind(kind: i64, base: f64) -> PyResult<ScaleKind> {
    match kind {
        LINEAR => Ok(ScaleKind::Linear),
        LOG => LogBase::new(base)
            .map(|base| ScaleKind::Log { base })
            .map_err(core_error),
        CATEGORY => Ok(ScaleKind::Category),
        _ => Err(internal_value(format!("{kind} is not a scale kind"))),
    }
}
