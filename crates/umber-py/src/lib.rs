//! PyO3 bindings that expose umber-core to Python as `umber._umber`.
//!
//! The extension is private: the `umber` package wraps it in typed, documented Python classes.
//! It takes plain values (integers, floats, strings, lists of strings and float64 buffers),
//! converts them to core types, and maps core errors to umber's Python exceptions. All rules
//! about valid values live in the core.

#![forbid(unsafe_code)]

mod cmap;
mod color;
mod enums;
mod errors;
mod figure;
mod scale;
mod size;
mod test_support;
mod theme;
mod ticks;
mod values;

use pyo3::prelude::*;

/// Native extension module behind the `umber` Python package. Private: use `umber` instead.
///
/// The module runs without the GIL on free-threaded Python (`gil_used = false`):
///
/// - `Figure` is the only mutable class. PyO3 checks its borrows at run time, so a call that
///   changes a figure while another thread uses it raises `RuntimeError` instead of racing.
///   Rendering copies the figure under a short borrow and renders the copy.
/// - `Theme` is frozen, and the functions keep no state between calls.
/// - The core keeps no mutable global state other than its font library, whose caches sit
///   behind locks and atomics.
#[pymodule(gil_used = false)]
fn _umber(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", umber_core::VERSION)?;
    m.add("MAX_BINS", umber_core::MAX_BINS)?;
    m.add_class::<theme::Theme>()?;
    m.add_class::<cmap::Cmap>()?;
    m.add_class::<figure::Figure>()?;
    m.add_class::<color::ColorBy>()?;
    m.add_function(wrap_pyfunction!(enums::enum_members, m)?)?;
    m.add_function(wrap_pyfunction!(figure::grid_auto_size, m)?)?;
    m.add_function(wrap_pyfunction!(figure::grid_cells, m)?)?;
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
    m.add_function(wrap_pyfunction!(test_support::box_stats, m)?)?;
    m.add_function(wrap_pyfunction!(test_support::kde_scott, m)?)?;
    Ok(())
}
