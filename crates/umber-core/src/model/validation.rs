//! Input validation of the model: series against their axes' scale kinds, explicit limits and
//! figure sizes. Every check runs before anything is stored, so a failed call changes nothing.

use super::axis_data::AxisData;
use crate::scale::{MAX_MAGNITUDE, MIN_LOG_VALUE};
use crate::{ContinuousScale, Error, Result, ScaleKind, Transform};

/// The largest accepted figure side, in inches.
const MAX_FIGURE_IN: f64 = 200.0;

/// Which axis a value belongs to, for messages.
fn transform_of(kind: ScaleKind) -> Option<Transform> {
    match kind {
        ScaleKind::Linear => Some(Transform::Linear),
        ScaleKind::Log { base } => Some(Transform::Log(base)),
        _ => None,
    }
}

fn data_error(context: &'static str, message: String) -> Error {
    Error::Data { context, message }
}

/// Checks numeric values of the axis `name` (`"x"` or `"y"`) against its scale kind.
fn check_numbers(context: &'static str, name: &str, values: &[f64], kind: ScaleKind) -> Result<()> {
    let log = matches!(kind, ScaleKind::Log { .. });
    for (i, &v) in values.iter().enumerate() {
        if v.is_nan() {
            continue;
        }
        if v.is_infinite() {
            return Err(data_error(
                context,
                format!("{name}[{i}] is {v}; values must be finite (NaN marks a gap)."),
            ));
        }
        if v.abs() > MAX_MAGNITUDE {
            return Err(data_error(
                context,
                format!("{name}[{i}] is {v:e}; magnitudes must not exceed {MAX_MAGNITUDE:e}."),
            ));
        }
        if log {
            if v <= 0.0 {
                return Err(data_error(
                    context,
                    format!("{name}[{i}] is {v} on a log axis; values must be positive."),
                ));
            }
            if v < MIN_LOG_VALUE {
                return Err(data_error(
                    context,
                    format!(
                        "{name}[{i}] is {v:e} on a log axis; values must be at least {MIN_LOG_VALUE:e}."
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Validates a whole series for axes with scale kinds `x_kind` and `y_kind`, before it is
/// stored, so a failed call changes nothing.
pub(super) fn check_series(
    context: &'static str,
    x_kind: ScaleKind,
    y_kind: ScaleKind,
    x: &AxisData,
    y: &[f64],
) -> Result<()> {
    if x.len() != y.len() {
        return Err(data_error(
            context,
            format!(
                "x has {} values but y has {}; x and y must have the same length.",
                x.len(),
                y.len()
            ),
        ));
    }
    match (x, x_kind) {
        (AxisData::Categories(_), ScaleKind::Category) => {}
        (AxisData::Numbers(values), kind) if kind != ScaleKind::Category => {
            check_numbers(context, "x", values, kind)?;
        }
        (AxisData::Numbers(_), _) => {
            return Err(data_error(
                context,
                "x holds numbers but the x axis is a category axis; pass category names.".into(),
            ));
        }
        (AxisData::Categories(_), _) => {
            return Err(data_error(
                context,
                "x holds category names but the x axis is not a category axis; pass numbers."
                    .into(),
            ));
        }
    }
    if y_kind == ScaleKind::Category {
        return Err(data_error(
            context,
            "y holds numbers but the y axis is a category axis; use a linear or log y axis.".into(),
        ));
    }
    check_numbers(context, "y", y, y_kind)
}

/// Checks explicit limits for an axis of `kind` and returns them unchanged.
pub(super) fn check_limits(
    context: &'static str,
    kind: ScaleKind,
    low: f64,
    high: f64,
) -> Result<(f64, f64)> {
    let Some(transform) = transform_of(kind) else {
        return Err(Error::Usage {
            context,
            message: "this is a category axis; limits apply to linear and log axes only.".into(),
        });
    };
    ContinuousScale::new(transform, low, high, context)?;
    Ok((low, high))
}

/// Checks the sides of a figure, in inches: each must be finite, above 0 and at most 200.
pub(super) fn check_figure_size(width_in: f64, height_in: f64) -> Result<()> {
    for (name, v) in [("width", width_in), ("height", height_in)] {
        if !(v.is_finite() && v > 0.0 && v <= MAX_FIGURE_IN) {
            return Err(Error::Style {
                context: "Figure()",
                message: format!(
                    "{name} is {v}; it must be finite, above 0 and at most {MAX_FIGURE_IN} inches."
                ),
            });
        }
    }
    Ok(())
}
