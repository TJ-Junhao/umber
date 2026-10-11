//! Input validation of the model: series and bar data against their axes' scale kinds, stacked
//! bar totals, explicit limits and figure sizes. Every check runs before anything is stored, so
//! a failed call changes nothing.

use std::collections::HashMap;

use super::axis_data::AxisData;
use super::bar::{Arrangement, BarSeries};
use crate::scale::{MAX_MAGNITUDE, MIN_LOG_VALUE};
use crate::stacking::Totals;
use crate::{ContinuousScale, Error, Result, ScaleKind};

/// The largest accepted figure side, in inches.
const MAX_FIGURE_IN: f64 = 200.0;

/// Checks numeric values of the axis `name` (`"x"` or `"y"`) against its scale kind.
fn check_numbers(context: &'static str, name: &str, values: &[f64], kind: ScaleKind) -> Result<()> {
    let log = matches!(kind, ScaleKind::Log { .. });
    for (i, &v) in values.iter().enumerate() {
        if v.is_nan() {
            continue;
        }
        if v.is_infinite() {
            return Err(Error::data(
                context,
                format!("{name}[{i}] is {v}; values must be finite (NaN marks a gap)."),
            ));
        }
        if v.abs() > MAX_MAGNITUDE {
            return Err(Error::data(
                context,
                format!("{name}[{i}] is {v:e}; magnitudes must not exceed {MAX_MAGNITUDE:e}."),
            ));
        }
        if log {
            if v <= 0.0 {
                return Err(Error::data(
                    context,
                    format!("{name}[{i}] is {v} on a log axis; values must be positive."),
                ));
            }
            if v < MIN_LOG_VALUE {
                return Err(Error::data(
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
        return Err(Error::data(
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
            return Err(Error::data(
                context,
                "x holds numbers but the x axis is a category axis; pass category names.",
            ));
        }
        (AxisData::Categories(_), _) => {
            return Err(Error::data(
                context,
                "x holds category names but the x axis is not a category axis; pass numbers.",
            ));
        }
    }
    if y_kind == ScaleKind::Category {
        return Err(Error::data(
            context,
            "y holds numbers but the y axis is a category axis; use a linear or log y axis.",
        ));
    }
    check_numbers(context, "y", y, y_kind)
}

/// Validates the categories and heights of a bar mark for axes with scale kinds `x_kind` and
/// `y_kind`, before it is stored, so a failed call changes nothing. The checks run in a fixed
/// order and the first failure is the error, under `context`:
///
/// 1. `categories` and `heights` have the same length;
/// 2. the categories, by [`check_categories`];
/// 3. every height is finite (NaN is allowed and draws no bar), within the supported
///    magnitudes, and within the log range on a log y axis;
/// 4. the y axis is a linear or log axis.
pub(super) fn check_bars(
    context: &'static str,
    x_kind: ScaleKind,
    y_kind: ScaleKind,
    categories: &[String],
    heights: &[f64],
) -> Result<()> {
    if categories.len() != heights.len() {
        return Err(Error::data(
            context,
            format!(
                "categories has {} values but heights has {}; categories and heights must have the same length.",
                categories.len(),
                heights.len()
            ),
        ));
    }
    check_categories(context, x_kind, categories)?;
    check_numbers(context, "heights", heights, y_kind)?;
    check_bar_y_axis(context, y_kind, "heights holds numbers")
}

/// Validates the categories and series of a grouped or stacked bar mark for axes with scale
/// kinds `x_kind` and `y_kind`, before it is stored, so a failed call changes nothing. The
/// checks run in a fixed order and the first failure is the error, under `context`:
///
/// 1. there is at least one series;
/// 2. every series has one height per category, checked in series order;
/// 3. the categories, by [`check_categories`];
/// 4. the heights of each series, in series order, as for a bar mark;
/// 5. for stacked series only: in every category, in category order, the positive heights and
///    the negative heights each add up to at most [`MAX_MAGNITUDE`] in magnitude, summed in
///    series order as the stacks are;
/// 6. the y axis is a linear or log axis.
pub(super) fn check_bar_group(
    context: &'static str,
    arrangement: Arrangement,
    x_kind: ScaleKind,
    y_kind: ScaleKind,
    categories: &[String],
    series: &[BarSeries],
) -> Result<()> {
    if series.is_empty() {
        return Err(Error::data(
            context,
            "series is empty; pass at least one series.",
        ));
    }
    for (i, s) in series.iter().enumerate() {
        if s.heights.len() != categories.len() {
            return Err(Error::data(
                context,
                format!(
                    "categories has {} values but series[{i}].heights has {}; each series must have one height per category.",
                    categories.len(),
                    s.heights.len()
                ),
            ));
        }
    }
    check_categories(context, x_kind, categories)?;
    for (i, s) in series.iter().enumerate() {
        check_numbers(context, &format!("series[{i}].heights"), &s.heights, y_kind)?;
    }
    if arrangement == Arrangement::Stacked {
        check_stacked_totals(context, categories, series)?;
    }
    check_bar_y_axis(context, y_kind, "the series hold numbers")
}

/// Checks the categories of a bar mark: the x axis is a category axis, and no category appears
/// twice.
fn check_categories(context: &'static str, x_kind: ScaleKind, categories: &[String]) -> Result<()> {
    if x_kind != ScaleKind::Category {
        return Err(Error::data(
            context,
            "categories holds names but the x axis is not a category axis; use a category x axis.",
        ));
    }
    let mut first_index: HashMap<&str, usize> = HashMap::with_capacity(categories.len());
    for (i, name) in categories.iter().enumerate() {
        if let Some(first) = first_index.insert(name, i) {
            return Err(Error::data(
                context,
                format!(
                    "categories[{i}] repeats {name:?} from categories[{first}]; each category must appear once."
                ),
            ));
        }
    }
    Ok(())
}

/// Checks that the positive and the negative heights of every category, stacked in series
/// order with [`Totals`], stay within [`MAX_MAGNITUDE`]. The heights are finite already.
fn check_stacked_totals(
    context: &'static str,
    categories: &[String],
    series: &[BarSeries],
) -> Result<()> {
    for (i, name) in categories.iter().enumerate() {
        let mut totals = Totals::default();
        for s in series {
            totals.push(s.heights.get(i).copied().unwrap_or(f64::NAN));
        }
        for total in [totals.positive(), totals.negative()] {
            // The heights are finite, so a total is a number, and an overflow is infinite.
            if total.abs() > MAX_MAGNITUDE {
                return Err(Error::data(
                    context,
                    format!(
                        "the heights of categories[{i}] ({name:?}) stack to {total:e}; each stack must stay within {MAX_MAGNITUDE:e} on both sides of 0."
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Checks that bar heights, described by `subject`, can be placed on the y axis: it must be a
/// linear or log axis.
fn check_bar_y_axis(context: &'static str, y_kind: ScaleKind, subject: &str) -> Result<()> {
    if y_kind == ScaleKind::Category {
        return Err(Error::data(
            context,
            format!("{subject} but the y axis is a category axis; use a linear or log y axis."),
        ));
    }
    Ok(())
}

/// Checks explicit limits for an axis of `kind` and returns the scale they define, with the
/// limits unchanged as its domain.
pub(super) fn check_limits(
    context: &'static str,
    kind: ScaleKind,
    low: f64,
    high: f64,
) -> Result<ContinuousScale> {
    let Some(transform) = kind.transform() else {
        return Err(Error::usage(
            context,
            "this is a category axis; limits apply to linear and log axes only.",
        ));
    };
    ContinuousScale::new(transform, low, high, context)
}

/// Checks the sides of a figure, in inches: each must be finite, above 0 and at most 200.
pub(super) fn check_figure_size(width_in: f64, height_in: f64) -> Result<()> {
    for (name, v) in [("width", width_in), ("height", height_in)] {
        if !(v.is_finite() && v > 0.0 && v <= MAX_FIGURE_IN) {
            return Err(Error::style(
                "Figure()",
                format!(
                    "{name} is {v}; it must be finite, above 0 and at most {MAX_FIGURE_IN} inches."
                ),
            ));
        }
    }
    Ok(())
}
