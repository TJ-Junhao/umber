//! PyO3 bindings that expose umber-core to Python as `umber._umber`.
//!
//! The extension is private: the `umber` package wraps it in typed, documented Python classes.
//! It takes plain values (integers, floats, strings, lists of strings and float64 buffers),
//! converts them to core types, and maps core errors to umber's Python exceptions. All rules
//! about valid values live in the core.

#![forbid(unsafe_code)]

mod color;
mod enums;
mod errors;
mod figure;
mod scale;
mod size;
#[cfg(test)]
mod test_support;
mod theme;
mod ticks;
mod values;

use pyo3::prelude::*;

/// Native extension module behind the `umber` Python package. Private: use `umber` instead.
#[pymodule]
fn _umber(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", umber_core::VERSION)?;
    m.add("MAX_BINS", umber_core::MAX_BINS)?;
    m.add_class::<theme::Theme>()?;
    m.add_class::<figure::Figure>()?;
    m.add_function(wrap_pyfunction!(enums::enum_members, m)?)?;
    m.add_function(wrap_pyfunction!(color::named_colors, m)?)?;
    m.add_function(wrap_pyfunction!(color::color_hex, m)?)?;
    m.add_function(wrap_pyfunction!(color::color_rgb, m)?)?;
    m.add_function(wrap_pyfunction!(color::color_with_alpha, m)?)?;
    m.add_function(wrap_pyfunction!(color::check_alpha, m)?)?;
    m.add_function(wrap_pyfunction!(color::check_palette_index, m)?)?;
    m.add_function(wrap_pyfunction!(scale::check_log_base, m)?)?;
    m.add_function(wrap_pyfunction!(size::check_size_times, m)?)?;
    m.add_function(wrap_pyfunction!(size::check_size_points, m)?)?;
    m.add_function(wrap_pyfunction!(ticks::check_tick_format, m)?)?;
    Ok(())
}
