//! Colormaps: the immutable `Cmap` class with the built-in colormaps.
//!
//! A colormap is built from concrete color arguments (see the `color` module) and compared by
//! its stops. The façade reads the stops back as `0xRRGGBBAA` integers to hash and print a
//! colormap.

use pyo3::prelude::*;

use crate::color::{ColorArg, concrete_list};
use crate::errors::core_error;

/// An immutable colormap. Equal colormaps have equal stops.
#[pyclass(module = "umber._umber", frozen, eq)]
#[derive(PartialEq)]
pub(crate) struct Cmap(umber_core::Cmap);

impl Cmap {
    /// The core colormap.
    pub(crate) fn core(&self) -> &umber_core::Cmap {
        &self.0
    }
}

#[pymethods]
impl Cmap {
    /// The perceptually uniform sequential colormap from dark purple to yellow.
    #[staticmethod]
    fn viridis() -> Cmap {
        Cmap(umber_core::Cmap::viridis())
    }

    /// The sequential colormap from black through purple and orange to pale yellow.
    #[staticmethod]
    fn magma() -> Cmap {
        Cmap(umber_core::Cmap::magma())
    }

    /// The sequential colormap from dark blue to yellow for color-blind readers.
    #[staticmethod]
    fn cividis() -> Cmap {
        Cmap(umber_core::Cmap::cividis())
    }

    /// The diverging colormap from blue through near-white to red.
    #[staticmethod]
    fn blue_red() -> Cmap {
        Cmap(umber_core::Cmap::blue_red())
    }

    /// A colormap through `colors`, each of which must be concrete. The message of a color that
    /// is not concrete names its index, e.g. `colors[2]`.
    #[staticmethod]
    fn from_colors(colors: Vec<ColorArg>) -> PyResult<Cmap> {
        let colors = concrete_list(colors, "Cmap.from_colors()", "colormap")?;
        umber_core::Cmap::from_colors(&colors)
            .map(Cmap)
            .map_err(core_error)
    }

    /// This colormap read backwards.
    fn reversed(&self) -> Cmap {
        Cmap(self.0.reversed())
    }

    /// The stops as opaque `0xRRGGBBAA` integers, in order.
    fn colors(&self) -> Vec<u32> {
        self.0
            .colors()
            .into_iter()
            .map(|color| u32::from_be_bytes([color.r, color.g, color.b, color.a]))
            .collect()
    }
}
