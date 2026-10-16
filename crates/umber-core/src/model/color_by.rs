//! Color mapping of points: a [`ColorBy`] maps one value per point through a colormap, and a
//! [`PointColor`] is the color of a scatter or bubble mark, one color or mapped values.

use std::sync::Arc;

use super::axes::Axes;
use super::mark::Mark;
use crate::color_scale::ColorScaleSpec;
use crate::error::{number, quoted};
use crate::{Cmap, ColorSpec, Error, MAX_MAGNITUDE, MIN_LOG_VALUE, Result, Transform};

/// The context of the errors of [`ColorBy::check`].
const CONTEXT: &str = "ColorBy()";

/// The colormap of a color mapping.
///
/// # Examples
///
/// ```
/// use umber_core::{Cmap, CmapChoice};
///
/// assert_ne!(CmapChoice::Theme, CmapChoice::Fixed(Cmap::viridis()));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum CmapChoice {
    /// The theme's default: its diverging colormap when the mapping has a center, its
    /// sequential colormap otherwise.
    Theme,
    /// This colormap.
    Fixed(Cmap),
}

/// One value per point, mapped through a colormap to the point's color.
///
/// All color-mapped marks of an axes share one color scale: the same colormap, scale and
/// center, and one color range, by default from the smallest to the largest finite value
/// ([`AxesMut::clim`](super::AxesMut::clim) sets it). A NaN value leaves its point undrawn;
/// the point's position still counts in the axis limits.
///
/// With a `center`, the mapping is diverging and symmetric: the color range is `center ± d`,
/// where `d` is the larger distance from `center` to either end of the values (or of the
/// explicit color range), and `center` maps to the middle of the colormap.
///
/// # Examples
///
/// ```
/// use umber_core::{CmapChoice, ColorBy, Transform};
///
/// let by = ColorBy::new(vec![1.0, 2.0, f64::NAN]);
/// assert_eq!((&by.cmap, by.scale, by.center), (&CmapChoice::Theme, Transform::Linear, None));
/// assert!(by.check().is_ok());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ColorBy {
    /// One value per point: finite, or NaN to leave the point undrawn.
    pub values: Arc<[f64]>,
    /// The colormap.
    pub cmap: CmapChoice,
    /// How values map to the colormap: linearly, or by their logarithm.
    pub scale: Transform,
    /// The value at the middle of a diverging colormap; `None` for a sequential mapping.
    pub center: Option<f64>,
}

impl ColorBy {
    /// A sequential mapping of `values` with the theme's colormap on a linear scale.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::ColorBy;
    ///
    /// assert_eq!(&*ColorBy::new(vec![3.0, 4.0]).values, &[3.0, 4.0]);
    /// ```
    #[must_use]
    pub fn new(values: impl Into<Arc<[f64]>>) -> ColorBy {
        ColorBy {
            values: values.into(),
            cmap: CmapChoice::Theme,
            scale: Transform::Linear,
            center: None,
        }
    }

    /// Checks the values against the scale, and the center.
    ///
    /// # Errors
    ///
    /// Under the context `"ColorBy()"`: `Error::Data` naming the first value that is infinite,
    /// beyond 1e300 in magnitude, or (on a log scale) not positive or below 1e-300, with the
    /// parameter `values`; `Error::Style` for a center that is not finite or beyond 1e300 in
    /// magnitude, or a center with a log scale, with the parameter `center`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ColorBy, LogBase, Transform};
    ///
    /// let by = ColorBy { scale: Transform::Log(LogBase::TEN), ..ColorBy::new(vec![1.0, 2.0, 0.0]) };
    /// assert_eq!(
    ///     by.check().unwrap_err().to_string(),
    ///     "ColorBy(): values[2] is 0; values on a log scale must be positive."
    /// );
    /// ```
    pub fn check(&self) -> Result<()> {
        check_mapped_values(
            CONTEXT,
            &self.values,
            self.scale,
            |i| format!("values[{i}]"),
            "use NaN to leave a point undrawn.",
        )?;
        check_center(CONTEXT, self.center, self.scale)
    }
}

/// Checks the color-mapped `values` of the call `context` against the color `scale`:
/// `Error::Data` about the parameter `values` for the first value that is infinite (with
/// `infinite_hint`, which says what to pass instead), beyond 1e300 in magnitude, or (on a log
/// scale) not positive or below 1e-300. NaN passes. `name(i)` is how messages name the value at
/// index `i`, such as `values[2]`.
pub(super) fn check_mapped_values(
    context: &'static str,
    values: &[f64],
    scale: Transform,
    name: impl Fn(usize) -> String,
    infinite_hint: &'static str,
) -> Result<()> {
    let log = matches!(scale, Transform::Log(_));
    for (i, &v) in values.iter().enumerate() {
        if v.is_nan() {
            continue;
        }
        let fault = |message: String| Error::data(context, message).with_parameter("values");
        if v.is_infinite() {
            return Err(fault(format!(
                "{} is {v}; values must be finite.",
                name(i),
                v = number(v)
            ))
            .with_hint(infinite_hint));
        }
        if v.abs() > MAX_MAGNITUDE {
            return Err(fault(format!(
                "{} is {v}; magnitudes must not exceed {MAX_MAGNITUDE:e}.",
                name(i),
                v = number(v)
            )));
        }
        if log && v <= 0.0 {
            return Err(fault(format!(
                "{} is {v}; values on a log scale must be positive.",
                name(i),
                v = number(v)
            ))
            .with_hint("use a linear scale, or NaN for values that are not positive."));
        }
        if log && v < MIN_LOG_VALUE {
            return Err(fault(format!(
                "{} is {v}; values on a log scale must be at least {MIN_LOG_VALUE:e}.",
                name(i),
                v = number(v)
            ))
            .with_hint("use a linear scale, or rescale the values."));
        }
    }
    Ok(())
}

/// Checks the `center` of a color mapping on the color `scale`, for the call `context`:
/// `Error::Style` about the parameter `center` for a center that is not finite or beyond 1e300
/// in magnitude, or any center with a log scale. No center passes.
pub(super) fn check_center(
    context: &'static str,
    center: Option<f64>,
    scale: Transform,
) -> Result<()> {
    let Some(center) = center else {
        return Ok(());
    };
    let fault = |message: String| Error::style(context, message).with_parameter("center");
    if !center.is_finite() || center.abs() > MAX_MAGNITUDE {
        return Err(fault(format!(
            "center is {}; it must be finite and at most {MAX_MAGNITUDE:e} in magnitude.",
            number(center)
        )));
    }
    if matches!(scale, Transform::Log(_)) {
        return Err(fault(format!(
            "center is {} with a log scale; a centered color scale is linear.",
            number(center)
        ))
        .with_hint("remove center, or use Scale.linear()."));
    }
    Ok(())
}

/// The color of a scatter or bubble mark: one color for every point, or a color per point
/// mapped from values.
///
/// # Examples
///
/// ```
/// use umber_core::{ColorBy, ColorSpec, PointColor, Rgba};
///
/// assert_eq!(PointColor::AUTO, PointColor::Uniform(ColorSpec::AUTO));
/// assert_eq!(PointColor::from(ColorSpec::Fixed(Rgba::RED)), PointColor::Uniform(ColorSpec::Fixed(Rgba::RED)));
/// assert_ne!(PointColor::By(ColorBy::new(vec![1.0])), PointColor::AUTO);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum PointColor {
    /// Every point in this color.
    Uniform(ColorSpec),
    /// Each point in the color its value maps to.
    By(ColorBy),
}

impl PointColor {
    /// Every point in the next color of the theme palette.
    pub const AUTO: PointColor = PointColor::Uniform(ColorSpec::AUTO);
}

impl From<ColorSpec> for PointColor {
    fn from(spec: ColorSpec) -> PointColor {
        PointColor::Uniform(spec)
    }
}

impl From<ColorBy> for PointColor {
    fn from(by: ColorBy) -> PointColor {
        PointColor::By(by)
    }
}

/// Checks the color `color` of a new mark of `count` points on `axes`, for the mark method
/// `context`: a color mapping must hold valid values ([`ColorBy::check`]), one per point
/// (`Error::Data`), map with the color scale of the axes' color-mapped marks
/// (`Error::Usage`), and suit the axes' color limits (`Error::Style`). Every error but those of
/// [`ColorBy::check`] is about the parameter `color`. One color always passes.
pub(super) fn check_point_color(
    context: &'static str,
    axes: &Axes,
    color: &PointColor,
    count: usize,
) -> Result<()> {
    let PointColor::By(by) = color else {
        return Ok(());
    };
    by.check()?;
    if by.values.len() != count {
        return Err(Error::data(
            context,
            format!(
                "x has {count} values but color has {}; x and color must have the same length.",
                by.values.len()
            ),
        )
        .with_parameter("color")
        .with_hint("pass one color value per point."));
    }
    let spec = ColorScaleSpec::of(by);
    if let Some(existing) = axes.marks.iter().find_map(Mark::color_scale) {
        spec.check_same(&existing, context, "color")?;
    }
    if let Some(limits) = axes.color_limits {
        spec.check_range(limits, context, Some("color"))?;
    }
    Ok(())
}

/// Checks that a scatter whose color is `color` has an empty `label` when it is color-mapped:
/// such a scatter has no legend entry, so a label would be dropped. `Error::Usage` under
/// `context` about `label`.
pub(super) fn check_scatter_label(
    context: &'static str,
    color: &PointColor,
    label: &str,
) -> Result<()> {
    if label.is_empty() || matches!(color, PointColor::Uniform(_)) {
        return Ok(());
    }
    Err(Error::usage(
        context,
        format!(
            "label is {}; a color-mapped scatter has no legend entry.",
            quoted(label)
        ),
    )
    .with_parameter("label")
    .with_hint("remove the label; the colorbar explains the colors."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LogBase;

    fn log(values: &[f64]) -> ColorBy {
        ColorBy {
            scale: Transform::Log(LogBase::TEN),
            ..ColorBy::new(values.to_vec())
        }
    }

    #[test]
    fn nan_values_pass_and_infinite_ones_do_not() {
        assert!(ColorBy::new(vec![f64::NAN, 1.0]).check().is_ok());
        let err = ColorBy::new(vec![1.0, f64::NEG_INFINITY])
            .check()
            .unwrap_err();
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(
            err.to_string(),
            "ColorBy(): values[1] is -inf; values must be finite."
        );
        assert_eq!(err.parameter(), Some("values"));
        assert_eq!(err.hint(), Some("use NaN to leave a point undrawn."));
    }

    #[test]
    fn log_values_must_be_positive() {
        let err = log(&[1.0, 2.0, 0.0]).check().unwrap_err();
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(
            err.to_string(),
            "ColorBy(): values[2] is 0; values on a log scale must be positive."
        );
        assert_eq!(err.parameter(), Some("values"));
        assert!(log(&[1.0, f64::NAN, 100.0]).check().is_ok());
    }

    #[test]
    fn center_is_finite_and_linear() {
        let by = |center| ColorBy {
            center: Some(center),
            ..ColorBy::new(vec![1.0])
        };
        assert!(by(0.0).check().is_ok());
        let err = by(f64::NAN).check().unwrap_err();
        assert!(matches!(err, Error::Style { .. }));
        assert_eq!(err.parameter(), Some("center"));
        let err = ColorBy {
            center: Some(1.0),
            ..log(&[1.0])
        }
        .check()
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "ColorBy(): center is 1 with a log scale; a centered color scale is linear."
        );
        assert_eq!(err.parameter(), Some("center"));
    }
}
