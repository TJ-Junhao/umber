//! Colors: the validators behind `umber.Color`, the named colors, and the conversion of a color
//! argument to a [`ColorSpec`].
//!
//! On the Python side a concrete color travels as one integer, `0xRRGGBBAA`. A color argument
//! of a mark or theme method travels as a tuple `(kind, payload, alpha)`:
//!
//! | kind | meaning | payload | alpha |
//! |---|---|---|---|
//! | 0 | `Color.AUTO` | unused (0) | the opacity |
//! | 1 | `Color.palette(index)` | the index | the opacity |
//! | 2 | a fixed color | `0xRRGGBBAA` | ignored: the opacity is part of the payload |
//!
//! The color of a scatter or bubble mark is such a tuple, or a native `ColorBy`: values mapped
//! through a colormap, checked when it is built.

use pyo3::prelude::*;
use umber_core::{Alpha, CmapChoice, ColorSpec, Error, PointColor, Rgba, ScaleKind, Transform};

use crate::cmap::Cmap;
use crate::errors::{core_error, internal_type, internal_value};
use crate::scale::scale_kind;
use crate::values::shared_floats;

/// The `kind` of a color argument for `Color.AUTO`.
const AUTO: i64 = 0;
/// The `kind` of a color argument for `Color.palette()`.
const PALETTE: i64 = 1;
/// The `kind` of a color argument for a fixed color.
const FIXED: i64 = 2;

/// The named colors of `umber.Color`, in the order the API lists them.
const NAMED: [(&str, Rgba); 12] = [
    ("BLACK", Rgba::BLACK),
    ("WHITE", Rgba::WHITE),
    ("GRAY", Rgba::GRAY),
    ("RED", Rgba::RED),
    ("ORANGE", Rgba::ORANGE),
    ("YELLOW", Rgba::YELLOW),
    ("GREEN", Rgba::GREEN),
    ("TEAL", Rgba::TEAL),
    ("BLUE", Rgba::BLUE),
    ("PURPLE", Rgba::PURPLE),
    ("PINK", Rgba::PINK),
    ("BROWN", Rgba::BROWN),
];

/// A concrete color as `0xRRGGBBAA`.
fn pack(color: Rgba) -> u32 {
    u32::from_be_bytes([color.r, color.g, color.b, color.a])
}

/// The color packed as `0xRRGGBBAA`; a value outside `0..=0xFFFFFFFF` is an internal
/// `ValueError`.
fn unpack(rgba: i64) -> PyResult<Rgba> {
    let packed = u32::try_from(rgba)
        .map_err(|_| internal_value(format!("{rgba} is not a color packed as 0xRRGGBBAA")))?;
    let [r, g, b, a] = packed.to_be_bytes();
    Ok(Rgba { r, g, b, a })
}

/// The named colors as `(name, 0xRRGGBBAA)` pairs.
#[pyfunction]
pub(crate) fn named_colors() -> Vec<(&'static str, u32)> {
    NAMED
        .iter()
        .map(|(name, color)| (*name, pack(*color)))
        .collect()
}

/// Parses `#RRGGBB` or `#RRGGBBAA` into `0xRRGGBBAA`.
#[pyfunction]
pub(crate) fn color_hex(code: &str) -> PyResult<u32> {
    Rgba::hex(code).map(pack).map_err(core_error)
}

/// An opaque color from three channels in `0..=255`, as `0xRRGGBBAA`.
#[pyfunction]
pub(crate) fn color_rgb(r: i64, g: i64, b: i64) -> PyResult<u32> {
    Rgba::from_ints(r, g, b).map(pack).map_err(core_error)
}

/// The color `rgba` with its alpha replaced by `a`, as `0xRRGGBBAA`.
#[pyfunction]
pub(crate) fn color_with_alpha(rgba: i64, a: f64) -> PyResult<u32> {
    let color = unpack(rgba)?;
    let alpha = Alpha::new(a).map_err(core_error)?;
    Ok(pack(color.with_alpha(alpha)))
}

/// Checks an opacity for `Color.alpha()`.
#[pyfunction]
pub(crate) fn check_alpha(a: f64) -> PyResult<()> {
    Alpha::new(a).map(drop).map_err(core_error)
}

/// Checks a palette index for `Color.palette()`.
#[pyfunction]
pub(crate) fn check_palette_index(index: i64) -> PyResult<()> {
    ColorSpec::palette(index, Alpha::OPAQUE)
        .map(drop)
        .map_err(core_error)
}

/// A color argument `(kind, payload, alpha)`, as the module documentation describes.
pub(crate) type ColorArg = (i64, i64, f64);

/// Converts a color argument `(kind, payload, alpha)` to a [`ColorSpec`]. An unknown kind or a
/// payload out of range is an internal `ValueError`.
pub(crate) fn color_spec((kind, payload, alpha): ColorArg) -> PyResult<ColorSpec> {
    let opacity = || Alpha::new(alpha).map_err(core_error);
    match kind {
        AUTO => Ok(ColorSpec::Auto(opacity()?)),
        PALETTE => ColorSpec::palette(payload, opacity()?).map_err(core_error),
        FIXED => unpack(payload).map(ColorSpec::Fixed),
        _ => Err(internal_value(format!("{kind} is not a color kind"))),
    }
}

/// The concrete color of `spec`, for the colors of a theme or a colormap (`subject`, such as
/// `"theme"`). `Color.AUTO` and `Color.palette()` refer to a theme palette, so they cannot be
/// used there: they are a `Style` error in `context` about `parameter`, whose message names the
/// offending argument as `what` (such as `colors[2]`).
pub(crate) fn concrete(
    spec: ColorSpec,
    context: &'static str,
    parameter: &'static str,
    what: &str,
    subject: &str,
) -> umber_core::Result<Rgba> {
    let given = match spec {
        ColorSpec::Fixed(color) => return Ok(color),
        ColorSpec::Auto(_) => "Color.AUTO".to_owned(),
        ColorSpec::Palette { index, .. } => format!("Color.palette({index})"),
    };
    Err(Error::Style {
        context,
        message: format!("{what} is {given}; {subject} colors must be concrete colors."),
        parameter: Some(parameter),
        hint: Some("use Color.hex(), Color.rgb() or a named color."),
    })
}

/// The concrete colors of the color arguments `colors`, passed as the parameter `colors` of
/// `context`. The first color that is not concrete is a `Style` error that names its index,
/// such as `colors[2]`; `subject` says whose colors they are, as in [`concrete`].
pub(crate) fn concrete_list(
    colors: Vec<ColorArg>,
    context: &'static str,
    subject: &str,
) -> PyResult<Vec<Rgba>> {
    colors
        .into_iter()
        .enumerate()
        .map(|(index, color)| {
            concrete(
                color_spec(color)?,
                context,
                "colors",
                &format!("colors[{index}]"),
                subject,
            )
            .map_err(core_error)
        })
        .collect()
}

/// A color mapping: one value per point mapped through a colormap. It is checked when it is
/// built, so a mark method only checks it against the axes.
#[pyclass(module = "umber._umber", frozen)]
pub(crate) struct ColorBy(umber_core::ColorBy);

#[pymethods]
impl ColorBy {
    /// Maps `values` through `cmap` (`None` for the theme's colormap) on the linear or log scale
    /// argument `(scale, base)`, centered on `center` when given. A category scale is an
    /// internal `ValueError`.
    #[new]
    fn new(
        values: &Bound<'_, PyAny>,
        cmap: Option<PyRef<'_, Cmap>>,
        scale: i64,
        base: f64,
        center: Option<f64>,
    ) -> PyResult<ColorBy> {
        let scale = match scale_kind(scale, base)? {
            ScaleKind::Linear => Transform::Linear,
            ScaleKind::Log { base } => Transform::Log(base),
            _ => {
                return Err(internal_value(
                    "a color mapping needs a linear or log scale",
                ));
            }
        };
        let by = umber_core::ColorBy {
            values: shared_floats(values)?,
            cmap: cmap.map_or(CmapChoice::Theme, |cmap| {
                CmapChoice::Fixed(cmap.core().clone())
            }),
            scale,
            center,
        };
        by.check().map_err(core_error)?;
        Ok(ColorBy(by))
    }
}

/// Converts the color of a scatter or bubble mark: a color argument `(kind, payload, alpha)`
/// or a native `ColorBy`. Anything else is an internal `TypeError`.
pub(crate) fn point_color(color: &Bound<'_, PyAny>) -> PyResult<PointColor> {
    if let Ok(by) = color.cast::<ColorBy>() {
        return Ok(PointColor::By(by.get().0.clone()));
    }
    let arg: ColorArg = color
        .extract()
        .map_err(|_| internal_type("expected a color argument or a ColorBy"))?;
    color_spec(arg).map(PointColor::Uniform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_round_trips_every_channel() {
        let color = Rgba {
            r: 0x12,
            g: 0x34,
            b: 0x56,
            a: 0x78,
        };
        assert_eq!(pack(color), 0x1234_5678);
        assert_eq!(unpack(0x1234_5678).unwrap(), color);
        assert_eq!(pack(Rgba::BLUE), 0x3366_B8FF);
    }

    #[test]
    fn named_colors_are_the_core_constants() {
        assert_eq!(NAMED.len(), 12);
        assert_eq!(NAMED[0], ("BLACK", Rgba::BLACK));
        assert_eq!(NAMED[11], ("BROWN", Rgba::BROWN));
    }

    #[test]
    fn concrete_keeps_fixed_colors() {
        let red = concrete(
            ColorSpec::Fixed(Rgba::RED),
            "Theme.with_background()",
            "value",
            "value",
            "theme",
        );
        assert_eq!(red.unwrap(), Rgba::RED);
    }

    #[test]
    fn concrete_rejects_palette_based_colors() {
        let auto = concrete(
            ColorSpec::AUTO,
            "Theme.with_palette()",
            "colors",
            "colors[2]",
            "theme",
        );
        let err = auto.unwrap_err();
        assert!(matches!(err, Error::Style { .. }));
        assert_eq!(
            err.to_string(),
            "Theme.with_palette(): colors[2] is Color.AUTO; theme colors must be concrete colors."
        );
        assert_eq!(err.parameter(), Some("colors"));
        assert_eq!(
            err.hint(),
            Some("use Color.hex(), Color.rgb() or a named color.")
        );

        let slot = ColorSpec::palette(3, Alpha::OPAQUE).unwrap();
        let err = concrete(slot, "Theme.with_grid_color()", "value", "value", "theme").unwrap_err();
        assert_eq!(
            err.to_string(),
            "Theme.with_grid_color(): value is Color.palette(3); theme colors must be concrete \
             colors."
        );
        assert_eq!(err.parameter(), Some("value"));
    }
}
