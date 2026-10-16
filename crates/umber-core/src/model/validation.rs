//! Input validation of the model: series, bar and histogram data against their axes' scales,
//! stacked bar totals, histogram bin counts, explicit limits, explicit ticks, text and figure
//! sizes. Every check runs before anything is stored, so a failed call changes nothing.

use std::collections::HashMap;

use super::axis_data::{AxisData, SharedX};
use super::bar::{Arrangement, Series};
use super::box_plot::Distribution;
use super::share::SharedAxis;
use crate::binning::{BinCount, MAX_BINS};
use crate::error::number;
use crate::scale::{MAX_MAGNITUDE, MIN_LOG_VALUE};
use crate::stacking::Totals;
use crate::{ContinuousScale, Error, Instant, Result, ScaleKind, Transform};

/// The hint for an x axis of the wrong kind for limits or numeric data.
const X_SCALE_HINT: &str = SharedAxis::X.numeric_scale_hint();

/// The hint for an infinite line value.
pub(super) const LINE_GAP_HINT: &str = "use NaN to leave a gap in the line.";

/// The hint for an infinite scatter value.
pub(super) const SCATTER_GAP_HINT: &str = "use NaN to leave a point out.";

/// The hint for an infinite area value.
pub(super) const AREA_GAP_HINT: &str = "use NaN to leave a gap in the area.";

/// The hint for an infinite bar height.
const BAR_GAP_HINT: &str = "use NaN to leave a category without a bar.";

/// The largest accepted figure side, in inches.
const MAX_FIGURE_IN: f64 = 200.0;

/// Checks `values`, shown as `name` in messages (`"y"`, `"series[1].values"`), against the
/// transform of their axis. An error names `parameter`, the Python parameter that holds them, and
/// an infinite value gets `infinite_hint`, which says what to pass instead.
fn check_numbers(
    context: &'static str,
    (name, parameter): (&str, &'static str),
    values: &[f64],
    transform: Transform,
    infinite_hint: &'static str,
) -> Result<()> {
    let log = matches!(transform, Transform::Log(_));
    for (i, &v) in values.iter().enumerate() {
        if v.is_nan() {
            continue;
        }
        if v.is_infinite() {
            return Err(Error::data(
                context,
                format!("{name}[{i}] is {v}; values must be finite.", v = number(v)),
            )
            .with_parameter(parameter)
            .with_hint(infinite_hint));
        }
        if v.abs() > MAX_MAGNITUDE {
            return Err(Error::data(
                context,
                format!(
                    "{name}[{i}] is {v}; magnitudes must not exceed {MAX_MAGNITUDE:e}.",
                    v = number(v)
                ),
            )
            .with_parameter(parameter));
        }
        if log {
            if v <= 0.0 {
                return Err(Error::data(
                    context,
                    format!(
                        "{name}[{i}] is {v} on a log axis; values must be positive.",
                        v = number(v)
                    ),
                )
                .with_parameter(parameter)
                .with_hint("use a linear axis, or remove values that are not positive."));
            }
            if v < MIN_LOG_VALUE {
                return Err(Error::data(
                    context,
                    format!(
                        "{name}[{i}] is {v} on a log axis; values must be at least {MIN_LOG_VALUE:e}.", v = number(v),
                    ),
                )
                .with_parameter(parameter)
                .with_hint("use a linear axis, or rescale the values."));
            }
        }
    }
    Ok(())
}

/// Validates a whole series for axes with an x axis of scale kind `x_kind` and a y axis of
/// scale kind `y_kind`, before it is stored, so a failed call changes nothing: the lengths, then
/// `x` against its axis, then `y` against its axis (see [`check_axis_data`]). An infinite value
/// gets `gap_hint`.
pub(super) fn check_series(
    context: &'static str,
    (x_kind, y_kind): (ScaleKind, ScaleKind),
    (x, y): (&AxisData, &AxisData),
    gap_hint: &'static str,
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
    check_axis_data(context, SharedAxis::X, x_kind, x, gap_hint)?;
    check_axis_data(context, SharedAxis::Y, y_kind, y, gap_hint)
}

/// Checks the values `data` of a series along `axis` against that axis, of scale kind `kind`:
/// names need a category axis, times a time axis, and numbers a linear or log axis, where they
/// are checked by [`check_numbers`]. Times are valid by construction; `None` marks a gap.
fn check_axis_data(
    context: &'static str,
    axis: SharedAxis,
    kind: ScaleKind,
    data: &AxisData,
    gap_hint: &'static str,
) -> Result<()> {
    let name = axis.name();
    match (data, kind) {
        (AxisData::Categories(_), ScaleKind::Category) | (AxisData::Times(_), ScaleKind::Time) => {
            Ok(())
        }
        (AxisData::Numbers(values), ScaleKind::Linear) => {
            check_numbers(context, (name, name), values, Transform::Linear, gap_hint)
        }
        (AxisData::Numbers(values), ScaleKind::Log { base }) => {
            check_numbers(context, (name, name), values, Transform::Log(base), gap_hint)
        }
        (AxisData::Numbers(_), ScaleKind::Category) => Err(Error::data(
            context,
            format!(
                "{name} holds numbers but the {name} axis is a category axis; pass category names."
            ),
        )
        .with_hint(axis.numeric_scale_hint())),
        (AxisData::Categories(_), ScaleKind::Linear | ScaleKind::Log { .. }) => Err(Error::data(
            context,
            format!(
                "{name} holds category names but the {name} axis is not a category axis; pass numbers."
            ),
        )
        .with_hint(axis.category_scale_hint())),
        (AxisData::Numbers(_), ScaleKind::Time) => Err(numbers_on_a_time_axis(context, axis)),
        (AxisData::Categories(_), ScaleKind::Time) => Err(Error::data(
            context,
            format!("{name} holds category names but the {name} axis is a time axis; pass times."),
        )
        .with_hint(axis.category_scale_hint())),
        (AxisData::Times(_), _) => Err(times_off_a_time_axis(context, axis, kind)),
    }
}

/// The `Error::Data` under `context` for numbers along `axis`, a time axis.
fn numbers_on_a_time_axis(context: &'static str, axis: SharedAxis) -> Error {
    let name = axis.name();
    Error::data(
        context,
        format!("{name} holds numbers but the {name} axis is a time axis; pass times."),
    )
    .with_hint(axis.numeric_scale_hint())
}

/// The `Error::Data` under `context` for times along `axis`, of scale kind `kind`, which is
/// not a time axis.
fn times_off_a_time_axis(context: &'static str, axis: SharedAxis, kind: ScaleKind) -> Error {
    let name = axis.name();
    let instead = if kind == ScaleKind::Category {
        "category names"
    } else {
        "numbers"
    };
    Error::data(
        context,
        format!("{name} holds times but the {name} axis is not a time axis; pass {instead}."),
    )
    .with_hint(axis.time_scale_hint())
}

/// Checks the shared x values `x` of a band or a stacked area against the x axis, of scale kind
/// `x_kind`: numbers need a linear or log axis and are checked by [`check_numbers`], times a
/// time axis. The callers refuse a category x axis before.
fn check_shared_x(
    context: &'static str,
    x_kind: ScaleKind,
    x: &SharedX,
    gap_hint: &'static str,
) -> Result<()> {
    match x {
        SharedX::Numbers(values) => {
            if x_kind == ScaleKind::Time {
                return Err(numbers_on_a_time_axis(context, SharedAxis::X));
            }
            let transform = x_kind.transform().ok_or_else(|| {
                Error::data(
                    context,
                    "x holds numbers but the x axis is a category axis; pass category names.",
                )
                .with_hint(X_SCALE_HINT)
            })?;
            check_numbers(context, ("x", "x"), values, transform, gap_hint)
        }
        SharedX::Times(_) if x_kind == ScaleKind::Time => Ok(()),
        SharedX::Times(_) => Err(times_off_a_time_axis(context, SharedAxis::X, x_kind)),
    }
}

/// Checks that the numeric x values of a step mark, `x`, are in non-decreasing order, before the
/// mark is stored. NaN marks a gap and is skipped: each value is compared with the last value
/// before it that is not NaN. The first value below its predecessor is the error, under
/// `context`. Category names have no order to check.
pub(super) fn check_step_order(context: &'static str, x: &AxisData) -> Result<()> {
    match x {
        AxisData::Numbers(values) => check_sorted(context, "step", values),
        AxisData::Times(times) => check_sorted_times(context, "step", times),
        AxisData::Categories(_) => Ok(()),
    }
}

/// Checks that the times `x` are in non-decreasing order for the mark `mark`, skipping missing
/// times as [`check_sorted`] skips NaN; the error is `Error::Data` under `context`.
fn check_sorted_times(context: &'static str, mark: &str, x: &[Option<Instant>]) -> Result<()> {
    let mut previous: Option<(usize, Instant)> = None;
    for (i, &t) in x.iter().enumerate() {
        let Some(t) = t else {
            continue;
        };
        if let Some((j, u)) = previous
            && t < u
        {
            return Err(Error::data(
                context,
                format!(
                    "x[{i}] is {}, below x[{j}] = {}; {mark} needs x in non-decreasing order.",
                    t.label(),
                    u.label()
                ),
            )
            .with_parameter("x")
            .with_hint("sort the points by x first."));
        }
        previous = Some((i, t));
    }
    Ok(())
}

/// Checks that the shared x values `x` are in non-decreasing order for the mark `mark`, as
/// [`check_sorted`] and [`check_sorted_times`] do.
fn check_shared_sorted(context: &'static str, mark: &str, x: &SharedX) -> Result<()> {
    match x {
        SharedX::Numbers(values) => check_sorted(context, mark, values),
        SharedX::Times(times) => check_sorted_times(context, mark, times),
    }
}

/// Checks that `x` is in non-decreasing order for the mark `mark` ("step", "band"), skipping
/// NaN as [`check_step_order`] does; the error is `Error::Data` under `context`.
fn check_sorted(context: &'static str, mark: &str, x: &[f64]) -> Result<()> {
    let mut previous: Option<(usize, f64)> = None;
    for (i, &v) in x.iter().enumerate() {
        if v.is_nan() {
            continue;
        }
        if let Some((j, u)) = previous
            && v < u
        {
            return Err(Error::data(
                context,
                format!(
                    "x[{i}] is {v}, below x[{j}] = {u}; {mark} needs x in non-decreasing order.",
                    v = number(v),
                    u = number(u)
                ),
            )
            .with_parameter("x")
            .with_hint("sort the points by x first."));
        }
        previous = Some((i, v));
    }
    Ok(())
}

/// Validates the x, `low` and `high` values of a band for axes with an x axis of scale kind
/// `x_kind` and a y axis of transform `y_transform`, before it is stored, so a failed call
/// changes nothing. The checks run in a fixed order and the first failure is the error, under
/// `context`:
///
/// 1. the x axis is not a category axis (`Error::Usage`);
/// 2. `x`, `low` and `high` have the same length;
/// 3. `x` against the x axis (numbers on a linear or log axis, times on a time axis), then
///    `low` and `high` against the y axis (see [`check_numbers`]);
/// 4. `x` is in non-decreasing order, gaps apart (see [`check_step_order`]);
/// 5. no `low[i]` exceeds `high[i]`; an index where either is NaN is not compared.
pub(super) fn check_band(
    context: &'static str,
    x_kind: ScaleKind,
    y_transform: Transform,
    (x, low, high): (&SharedX, &[f64], &[f64]),
) -> Result<()> {
    const GAP_HINT: &str = "use NaN to leave a gap in the band.";
    if x_kind == ScaleKind::Category {
        return Err(Error::usage(
            context,
            "this is a category axis; bands along x apply to linear and log axes only.",
        )
        .with_parameter("x")
        .with_hint(X_SCALE_HINT));
    }
    for (name, other) in [("low", low), ("high", high)] {
        if x.len() != other.len() {
            return Err(Error::data(
                context,
                format!(
                    "x has {} values but {name} has {}; x, low and high must have the same length.",
                    x.len(),
                    other.len()
                ),
            ));
        }
    }
    check_shared_x(context, x_kind, x, GAP_HINT)?;
    check_numbers(context, ("low", "low"), low, y_transform, GAP_HINT)?;
    check_numbers(context, ("high", "high"), high, y_transform, GAP_HINT)?;
    check_shared_sorted(context, "band", x)?;
    check_interval_order(context, (low, high))
}

/// Validates stacked areas before storage: continuous x, a non-empty series list, lengths,
/// x values, each series' finite non-negative values and y scale, sorted x, then totals.
/// A gap in any series breaks the whole stack at that index.
pub(super) fn check_stacked_area(
    context: &'static str,
    x_kind: ScaleKind,
    y_transform: Transform,
    x: &SharedX,
    series: &[Series],
) -> Result<()> {
    if x_kind == ScaleKind::Category {
        return Err(Error::usage(
            context,
            "this is a category axis; stacked areas apply to linear and log axes only.",
        )
        .with_parameter("x")
        .with_hint(X_SCALE_HINT));
    }
    if series.is_empty() {
        return Err(
            Error::data(context, "series is empty; pass at least one Series.")
                .with_parameter("series"),
        );
    }
    for (k, s) in series.iter().enumerate() {
        if s.values.len() != x.len() {
            return Err(Error::data(
                context,
                format!(
                    "series[{k}].values has {} values; x has {}.",
                    s.values.len(),
                    x.len()
                ),
            )
            .with_parameter("series"));
        }
    }
    check_shared_x(context, x_kind, x, AREA_GAP_HINT)?;
    for (k, s) in series.iter().enumerate() {
        let name = format!("series[{k}].values");
        let negative = s.values.iter().position(|&v| v.is_finite() && v < 0.0);
        // Check the prefix first so an earlier invalid value wins. A finite negative gets the
        // stacked-area hint even on log y, rather than the generic log-axis error.
        check_numbers(
            context,
            (&name, "series"),
            &s.values[..negative.unwrap_or(s.values.len())],
            y_transform,
            AREA_GAP_HINT,
        )?;
        if let Some(i) = negative {
            return Err(Error::data(
                context,
                format!(
                    "{name}[{i}] is {}; stacked areas need values of at least 0.",
                    number(s.values[i])
                ),
            )
            .with_parameter("series")
            .with_hint("use stacked_bar for values with mixed signs."));
        }
    }
    check_shared_sorted(context, "stacked_area", x)?;
    for i in 0..x.len() {
        if x.is_gap(i) || series.iter().any(|s| s.values[i].is_nan()) {
            continue;
        }
        let mut totals = Totals::default();
        for s in series {
            totals.push(s.values[i]);
        }
        let total = totals.positive();
        if total > MAX_MAGNITUDE {
            return Err(Error::data(context, format!(
                "the values at x[{i}] stack to {}; each stack must not exceed {MAX_MAGNITUDE:e}.", number(total)
            )).with_parameter("series"));
        }
    }
    Ok(())
}

/// Checks bubble coordinates and sizes before storage: numeric x, lengths, coordinates, sizes.
pub(super) fn check_bubbles(
    context: &'static str,
    x_kind: ScaleKind,
    y_transform: Transform,
    (x, y, sizes): (&[f64], &[f64], &[f64]),
) -> Result<()> {
    if x_kind == ScaleKind::Time {
        return Err(Error::usage(
            context,
            format!("x is a time scale; {context} needs a linear or log x axis."),
        )
        .with_parameter("x")
        .with_hint("convert the times to numbers first, such as days since the first one."));
    }
    let Some(x_transform) = x_kind.transform() else {
        return Err(Error::usage(
            context,
            "this is a category axis; bubbles apply to linear and log axes only.",
        )
        .with_parameter("x")
        .with_hint(X_SCALE_HINT));
    };
    for (name, other) in [("y", y), ("sizes", sizes)] {
        if x.len() != other.len() {
            return Err(Error::data(
                context,
                format!(
                    "x has {} values but {name} has {}; x and {name} must have the same length.",
                    x.len(),
                    other.len()
                ),
            ));
        }
    }
    check_numbers(context, ("x", "x"), x, x_transform, SCATTER_GAP_HINT)?;
    check_numbers(context, ("y", "y"), y, y_transform, SCATTER_GAP_HINT)?;
    for (i, &value) in sizes.iter().enumerate() {
        if !value.is_finite() || value < 0.0 {
            return Err(Error::data(
                context,
                format!(
                    "sizes[{i}] is {}; bubble sizes must be finite and at least 0.",
                    number(value)
                ),
            )
            .with_parameter("sizes")
            .with_hint("use 0 for a point to leave out."));
        }
        if value > MAX_MAGNITUDE {
            return Err(Error::data(
                context,
                format!(
                    "sizes[{i}] is {}; magnitudes must not exceed {MAX_MAGNITUDE:e}.",
                    number(value)
                ),
            )
            .with_parameter("sizes"));
        }
    }
    Ok(())
}

/// Checks that no lower limit exceeds its upper limit; NaN in either limit is not compared.
fn check_interval_order(context: &'static str, (low, high): (&[f64], &[f64])) -> Result<()> {
    for (i, (&a, &b)) in low.iter().zip(high).enumerate() {
        if a > b {
            return Err(Error::data(
                context,
                format!(
                    "low[{i}] is {a}, above high[{i}] = {b}; low must not exceed high.",
                    a = number(a),
                    b = number(b)
                ),
            )
            .with_parameter("low")
            .with_hint("check that low and high are not swapped."));
        }
    }
    Ok(())
}

/// The hint for an infinite error bar limit.
pub(super) const ERROR_BAR_GAP_HINT: &str = "use NaN to skip a bar.";

/// Checks that `positions`, `low` and `high` of an error bar mark have one length. `name` is
/// what the positions are called in messages (`"x"`, `"y"`).
fn check_interval_lengths(
    context: &'static str,
    name: &str,
    count: usize,
    (low, high): (&[f64], &[f64]),
) -> Result<()> {
    for (other_name, other) in [("low", low), ("high", high)] {
        if count != other.len() {
            return Err(Error::data(
                context,
                format!(
                    "{name} has {count} values but {other_name} has {}; {name}, low and high must have the same length.",
                    other.len()
                ),
            ));
        }
    }
    Ok(())
}

/// Checks the `low` and `high` limits of error bars against the axis with the transform
/// `transform` they are values of, `low` and then `high` by [`check_numbers`], and then that no
/// `low[i]` exceeds `high[i]`; an index where either is NaN is not compared.
fn check_intervals(
    context: &'static str,
    transform: Transform,
    (low, high): (&[f64], &[f64]),
) -> Result<()> {
    check_numbers(context, ("low", "low"), low, transform, ERROR_BAR_GAP_HINT)?;
    check_numbers(
        context,
        ("high", "high"),
        high,
        transform,
        ERROR_BAR_GAP_HINT,
    )?;
    check_interval_order(context, (low, high))
}

/// Validates the x, `low` and `high` values of vertical error bars for axes with an x axis of
/// scale kind `x_kind` and a y axis of transform `y_transform`, before they are stored, so a
/// failed call changes nothing. The checks run in a fixed order and the first failure is the
/// error, under `context`:
///
/// 1. `x`, `low` and `high` have the same length;
/// 2. `x` against the x axis (see [`check_axis_data`]);
/// 3. `low`, then `high`, against the y axis (see [`check_numbers`]);
/// 4. no `low[i]` exceeds `high[i]`; an index where either is NaN is not compared.
pub(super) fn check_y_error_bars(
    context: &'static str,
    x_kind: ScaleKind,
    y_transform: Transform,
    x: &AxisData,
    limits: (&[f64], &[f64]),
) -> Result<()> {
    check_interval_lengths(context, "x", x.len(), limits)?;
    check_axis_data(context, SharedAxis::X, x_kind, x, ERROR_BAR_GAP_HINT)?;
    check_intervals(context, y_transform, limits)
}

/// Validates the y, `low` and `high` values of horizontal error bars for axes with an x axis of
/// scale kind `x_kind` and a y axis of scale kind `y_kind`, before they are stored, so a
/// failed call changes nothing. The checks run in a fixed order and the first failure is the
/// error, under `context`:
///
/// 1. the x axis is not a category axis, nor a time axis (`Error::Usage`);
/// 2. `y`, `low` and `high` have the same length;
/// 3. `y` against the y axis (see [`check_axis_data`]), then `low` and `high` against the x
///    axis (see [`check_numbers`]);
/// 4. no `low[i]` exceeds `high[i]`; an index where either is NaN is not compared.
pub(super) fn check_x_error_bars(
    context: &'static str,
    (x_kind, y_kind): (ScaleKind, ScaleKind),
    y: &AxisData,
    limits: (&[f64], &[f64]),
) -> Result<()> {
    if x_kind == ScaleKind::Time {
        return Err(Error::usage(
            context,
            format!("x is a time scale; {context} needs a numeric x axis."),
        )
        .with_parameter("x")
        .with_hint("use a linear or log x axis."));
    }
    let Some(x_transform) = x_kind.transform() else {
        return Err(Error::usage(
            context,
            "this is a category axis; error bars along x apply to linear and log axes only.",
        )
        .with_hint("x_error_bars needs a numeric x axis; use y_error_bars on a category."));
    };
    check_interval_lengths(context, "y", y.len(), limits)?;
    check_axis_data(context, SharedAxis::Y, y_kind, y, ERROR_BAR_GAP_HINT)?;
    check_intervals(context, x_transform, limits)
}

/// The words that finish the message for a reference line whose value is not finite.
pub(super) const REFERENCE_LINE: &str = "a reference line needs a finite value";

/// The hint for a reference value that is not finite.
const REFERENCE_VALUE_HINT: &str = "pass a finite number.";

/// Checks the value `value` of a reference line or span, shown as `name` in messages and held by
/// the Python parameter `name`, against the transform of the axis it sits on, before the mark is
/// stored. `requirement` finishes the message for a value that is not finite (`"a reference line
/// needs a finite value"`). Unlike the values of a series, NaN is refused: there is no gap to
/// leave in a single line.
pub(super) fn check_reference_value(
    context: &'static str,
    name: &'static str,
    requirement: &str,
    value: f64,
    transform: Transform,
) -> Result<()> {
    if !value.is_finite() {
        return Err(Error::data(
            context,
            format!("{name} is {v}; {requirement}.", v = number(value)),
        )
        .with_parameter(name)
        .with_hint(REFERENCE_VALUE_HINT));
    }
    if value.abs() > MAX_MAGNITUDE {
        return Err(Error::data(
            context,
            format!(
                "{name} is {v}; magnitudes must not exceed {MAX_MAGNITUDE:e}.",
                v = number(value)
            ),
        )
        .with_parameter(name));
    }
    if matches!(transform, Transform::Log(_)) {
        if value <= 0.0 {
            return Err(Error::data(
                context,
                format!(
                    "{name} is {v} on a log axis; values must be positive.",
                    v = number(value)
                ),
            )
            .with_parameter(name)
            .with_hint("use a linear axis, or pass a positive value."));
        }
        if value < MIN_LOG_VALUE {
            return Err(Error::data(
                context,
                format!(
                    "{name} is {v} on a log axis; values must be at least {MIN_LOG_VALUE:e}.",
                    v = number(value)
                ),
            )
            .with_parameter(name)
            .with_hint("use a linear axis, or rescale the value."));
        }
    }
    Ok(())
}

/// Checks the ends of a span, `low` and `high`, of the axis with the transform `transform`,
/// before it is stored: each end is checked as [`check_reference_value`] checks a value, `low`
/// first, and then `low` must be below `high`.
pub(super) fn check_span(
    context: &'static str,
    (low, high): (f64, f64),
    transform: Transform,
) -> Result<()> {
    const REQUIREMENT: &str = "a reference span needs finite values";
    check_reference_value(context, "low", REQUIREMENT, low, transform)?;
    check_reference_value(context, "high", REQUIREMENT, high, transform)?;
    if low >= high {
        return Err(Error::style(
            context,
            format!(
                "low is {low}, high is {high}; low must be below high.",
                low = number(low),
                high = number(high)
            ),
        )
        .with_parameter("high")
        .with_hint("swap the two values."));
    }
    Ok(())
}

/// Checks the ends of a span of times, `low` and `high`, before it is stored: `low` must be
/// below `high`, as [`check_span`] requires of numbers.
pub(super) fn check_time_span(
    context: &'static str,
    (low, high): (Instant, Instant),
) -> Result<()> {
    if low >= high {
        return Err(Error::style(
            context,
            format!(
                "low is {}, high is {}; low must be below high.",
                low.label(),
                high.label()
            ),
        )
        .with_parameter("high")
        .with_hint("swap the two values."));
    }
    Ok(())
}

/// Checks the x of a vertical reference line against the x axis of scale kind `kind`, before the
/// line is stored: a number needs a linear or log axis and is checked as
/// [`check_reference_value`] checks a value, a category name needs a category axis, and a time
/// a time axis. Whether the name is one of the axis's categories is not known until the figure
/// is rendered.
pub(super) fn check_vline_x(
    context: &'static str,
    kind: ScaleKind,
    x: &super::rule::AxisValue,
) -> Result<()> {
    use super::rule::AxisValue;
    match (x, kind) {
        (AxisValue::Number(x), ScaleKind::Linear) => {
            check_reference_value(context, "x", REFERENCE_LINE, *x, Transform::Linear)
        }
        (AxisValue::Number(x), ScaleKind::Log { base }) => {
            check_reference_value(context, "x", REFERENCE_LINE, *x, Transform::Log(base))
        }
        (AxisValue::Category(_), ScaleKind::Category) | (AxisValue::Time(_), ScaleKind::Time) => {
            Ok(())
        }
        (AxisValue::Number(_), ScaleKind::Category) => Err(Error::data(
            context,
            "x holds a number but the x axis is a category axis; pass a category name.",
        )
        .with_parameter("x")
        .with_hint(X_SCALE_HINT)),
        (AxisValue::Category(_), ScaleKind::Linear | ScaleKind::Log { .. }) => Err(Error::data(
            context,
            "x holds a category name but the x axis is not a category axis; pass a number.",
        )
        .with_parameter("x")
        .with_hint("pass x=Scale.category() to Figure.add_axes().")),
        (AxisValue::Number(_), ScaleKind::Time) => Err(Error::data(
            context,
            "x holds a number but the x axis is a time axis; pass a time.",
        )
        .with_parameter("x")
        .with_hint(X_SCALE_HINT)),
        (AxisValue::Category(_), ScaleKind::Time) => Err(Error::data(
            context,
            "x holds a category name but the x axis is a time axis; pass a time.",
        )
        .with_parameter("x")
        .with_hint("pass x=Scale.category() to Figure.add_axes().")),
        (AxisValue::Time(_), _) => {
            let instead = if kind == ScaleKind::Category {
                "a category name"
            } else {
                "a number"
            };
            Err(Error::data(
                context,
                format!("x holds a time but the x axis is not a time axis; pass {instead}."),
            )
            .with_parameter("x")
            .with_hint(SharedAxis::X.time_scale_hint()))
        }
    }
}

/// Validates the categories and heights of a bar mark for axes with an x axis of scale kind
/// `x_kind` and a y axis of transform `y_transform`, before it is stored, so a failed call
/// changes nothing. The checks run in a fixed order and the first failure is the error, under
/// `context`:
///
/// 1. `categories` and `heights` have the same length;
/// 2. the categories, by [`check_categories`];
/// 3. every height is finite (NaN is allowed and draws no bar), within the supported
///    magnitudes, and within the log range on a log y axis.
pub(super) fn check_bars(
    context: &'static str,
    x_kind: ScaleKind,
    y_transform: Transform,
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
    check_categories(
        context,
        x_kind,
        categories,
        "combine repeated categories and sum their heights.",
    )?;
    check_numbers(
        context,
        ("heights", "heights"),
        heights,
        y_transform,
        BAR_GAP_HINT,
    )
}

/// Validates the categories and series of a grouped or stacked bar mark for axes with an x axis
/// of scale kind `x_kind` and a y axis of transform `y_transform`, before it is stored, so a
/// failed call changes nothing. The checks run in a fixed order and the first failure is the
/// error, under `context`:
///
/// 1. there is at least one series;
/// 2. every series has one value per category, checked in series order;
/// 3. the categories, by [`check_categories`];
/// 4. the values of each series, in series order, as for a bar mark;
/// 5. for stacked series only: in every category, in category order, the positive heights and
///    the negative heights each add up to at most [`MAX_MAGNITUDE`] in magnitude, summed in
///    series order as the stacks are.
pub(super) fn check_bar_group(
    context: &'static str,
    arrangement: Arrangement,
    x_kind: ScaleKind,
    y_transform: Transform,
    categories: &[String],
    series: &[Series],
) -> Result<()> {
    if series.is_empty() {
        return Err(
            Error::data(context, "series is empty; pass at least one series.")
                .with_parameter("series"),
        );
    }
    for (i, s) in series.iter().enumerate() {
        if s.values.len() != categories.len() {
            return Err(Error::data(
                context,
                format!(
                    "categories has {} values but series[{i}].values has {}; each series must have one value per category.",
                    categories.len(),
                    s.values.len()
                ),
            )
            .with_parameter("series"));
        }
    }
    check_categories(
        context,
        x_kind,
        categories,
        "combine repeated categories and sum their heights within each series.",
    )?;
    for (i, s) in series.iter().enumerate() {
        check_numbers(
            context,
            (&format!("series[{i}].values"), "series"),
            &s.values,
            y_transform,
            "use NaN to omit that height.",
        )?;
    }
    if arrangement == Arrangement::Stacked {
        check_stacked_totals(context, categories, series)?;
    }
    Ok(())
}

/// Checks the category axis before any distribution data, then the unique category names.
fn check_box_categories(
    context: &'static str,
    kind: &'static str,
    x: ScaleKind,
    categories: &[String],
) -> Result<()> {
    if x != ScaleKind::Category {
        return Err(Error::usage(
            context,
            format!("the x axis is not a category axis; {kind} need a category x axis."),
        )
        .with_parameter("categories")
        .with_hint("pass x=Scale.category() to Figure.add_axes()."));
    }
    check_categories(
        context,
        x,
        categories,
        "combine the samples of repeated categories.",
    )
}

/// Checks that a distribution has one group per category, naming its container on failure.
fn check_group_count(
    context: &'static str,
    name: &str,
    parameter: &'static str,
    count: usize,
    categories: usize,
) -> Result<()> {
    if count != categories {
        return Err(Error::data(
            context,
            format!("{name} has {count} groups; categories has {categories}."),
        )
        .with_parameter(parameter)
        .with_hint(
            "pass one group per category, using an empty group when there are no samples.",
        ));
    }
    Ok(())
}

/// Validates each group's values, keeping group and sample indices in the first error.
fn check_group_values(
    context: &'static str,
    name: &str,
    parameter: &'static str,
    y: Transform,
    groups: &[Vec<f64>],
) -> Result<()> {
    for (k, values) in groups.iter().enumerate() {
        check_numbers(
            context,
            (&format!("{name}[{k}]"), parameter),
            values,
            y,
            "use NaN to leave a sample out.",
        )?;
    }
    Ok(())
}

/// Validates plain box data: axis kind, group count, category uniqueness, then all samples.
pub(super) fn check_box_groups(
    context: &'static str,
    kind: &'static str,
    x: ScaleKind,
    y: Transform,
    categories: &[String],
    groups: &[Vec<f64>],
) -> Result<()> {
    check_box_categories(context, kind, x, &[])?;
    check_group_count(context, "groups", "groups", groups.len(), categories.len())?;
    check_box_categories(context, kind, x, categories)?;
    check_group_values(context, "groups", "groups", y, groups)
}

/// Validates grouped boxes: axis kind, non-empty distributions, all group counts, unique
/// categories, then every sample, without storing any part of a failed call.
pub(super) fn check_distributions(
    context: &'static str,
    kind: &'static str,
    x: ScaleKind,
    y: Transform,
    categories: &[String],
    distributions: &[Distribution],
) -> Result<()> {
    check_box_categories(context, kind, x, &[])?;
    if distributions.is_empty() {
        return Err(Error::data(
            context,
            "distributions is empty; pass at least one Distribution.",
        )
        .with_parameter("distributions"));
    }
    for (k, d) in distributions.iter().enumerate() {
        check_group_count(
            context,
            &format!("distributions[{k}].groups"),
            "distributions",
            d.groups.len(),
            categories.len(),
        )?;
    }
    check_box_categories(context, kind, x, categories)?;
    for (k, d) in distributions.iter().enumerate() {
        check_group_values(
            context,
            &format!("distributions[{k}].groups"),
            "distributions",
            y,
            &d.groups,
        )?;
    }
    Ok(())
}

/// Validates the values and bin count of a histogram for axes with an x axis of scale kind
/// `x_kind`, before it is stored, so a failed call changes nothing, and returns the bin count.
/// The y axis, linear or log, takes any count. The checks run in a fixed order and the first
/// failure is the error, under `context`:
///
/// 1. `bins` is within `1..=MAX_BINS` (`Error::Style`);
/// 2. the x axis is a linear axis: a category x axis is `Error::Data`, and a log x axis, which
///    histograms do not support, is `Error::Usage`;
/// 3. every value is finite (NaN is allowed and ignored) and within the supported magnitudes
///    (`Error::Data`).
pub(super) fn check_hist(
    context: &'static str,
    x_kind: ScaleKind,
    values: &[f64],
    bins: i64,
) -> Result<BinCount> {
    let Some(count) = BinCount::new(bins) else {
        let error = Error::style(
            context,
            format!("bins is {bins}; bins must be between 1 and {MAX_BINS}."),
        )
        .with_parameter("bins");
        return Err(if bins < 1 {
            error.with_hint("use at least one bin.")
        } else {
            error
        });
    };
    match x_kind {
        ScaleKind::Linear => {}
        ScaleKind::Category => {
            return Err(Error::data(
                context,
                "values holds numbers but the x axis is a category axis; use a linear x axis.",
            )
            .with_hint("pass x=Scale.linear() to Figure.add_axes()."));
        }
        ScaleKind::Log { .. } => {
            return Err(Error::usage(
                context,
                "the x axis is a log axis, which histograms do not support; use a linear x axis.",
            )
            .with_hint("pass x=Scale.linear() to Figure.add_axes()."));
        }
        ScaleKind::Time => check_no_time_x(context, x_kind, "a linear x axis")?,
    }
    check_numbers(
        context,
        ("values", "values"),
        values,
        Transform::Linear,
        "a histogram ignores NaN, so use it to leave a value out.",
    )?;
    Ok(count)
}

/// Checks the categories of a bar mark: the x axis is a category axis, and no category appears
/// twice.
fn check_categories(
    context: &'static str,
    x_kind: ScaleKind,
    categories: &[String],
    repeat_hint: &'static str,
) -> Result<()> {
    if x_kind != ScaleKind::Category {
        return Err(Error::data(
            context,
            "categories holds names but the x axis is not a category axis; use a category x axis.",
        )
        .with_hint("pass x=Scale.category() to Figure.add_axes()."));
    }
    let mut first_index: HashMap<&str, usize> = HashMap::with_capacity(categories.len());
    for (i, name) in categories.iter().enumerate() {
        if let Some(first) = first_index.insert(name, i) {
            return Err(Error::data(
                context,
                format!(
                    "categories[{i}] repeats {name:?} from categories[{first}]; each category must appear once."
                ),
            )
            .with_parameter("categories")
            .with_hint(repeat_hint));
        }
    }
    Ok(())
}

/// Checks that the positive and the negative heights of every category, stacked in series
/// order with [`Totals`], stay within [`MAX_MAGNITUDE`]. The heights are finite already.
fn check_stacked_totals(
    context: &'static str,
    categories: &[String],
    series: &[Series],
) -> Result<()> {
    for (i, name) in categories.iter().enumerate() {
        let mut totals = Totals::default();
        for s in series {
            totals.push(s.values.get(i).copied().unwrap_or(f64::NAN));
        }
        for total in [totals.positive(), totals.negative()] {
            // The heights are finite, so a total is a number, and an overflow is infinite.
            if total.abs() > MAX_MAGNITUDE {
                return Err(Error::data(
                    context,
                    format!(
                        "the values of categories[{i}] ({name:?}) stack to {total}; each stack must stay within {MAX_MAGNITUDE:e} on both sides of 0.", total = number(total),
                    ),
                )
                .with_parameter("series"));
            }
        }
    }
    Ok(())
}

/// Checks explicit limits for the setter `context` of `axis`, of scale kind `kind`, and returns
/// the scale they define, with the limits unchanged as its domain: `Error::Usage` on a category
/// axis, whose extent is its categories, then the limits as [`ContinuousScale::new`] checks
/// them.
pub(super) fn check_limits(
    context: &'static str,
    axis: SharedAxis,
    kind: ScaleKind,
    (low, high): (f64, f64),
) -> Result<ContinuousScale> {
    if kind == ScaleKind::Time {
        return Err(Error::usage(
            context,
            "this is a time axis; its limits must be times.",
        ));
    }
    let Some(transform) = kind.transform() else {
        return Err(Error::usage(
            context,
            "this is a category axis; limits apply to linear and log axes only.",
        )
        .with_hint(axis.numeric_scale_hint()));
    };
    ContinuousScale::new(transform, low, high, context)
}

/// Checks the explicit limits of a time axis, `low` and `high`, for the setter `context` of
/// `axis`, of scale kind `kind`: `Error::Usage` unless it is a time axis, then `Error::Style`
/// unless `low` is below `high`.
pub(super) fn check_time_limits(
    context: &'static str,
    axis: SharedAxis,
    kind: ScaleKind,
    (low, high): (Instant, Instant),
) -> Result<(Instant, Instant)> {
    match kind {
        ScaleKind::Time => {}
        ScaleKind::Category => {
            return Err(Error::usage(
                context,
                "this is a category axis; limits apply to linear, log and time axes only.",
            )
            .with_hint(axis.time_scale_hint()));
        }
        ScaleKind::Linear | ScaleKind::Log { .. } => {
            return Err(Error::usage(
                context,
                "this is a linear or log axis; its limits must be numbers.",
            ));
        }
    }
    if low >= high {
        return Err(Error::style(
            context,
            format!(
                "the limits are {} and {}; the lower limit must be below the upper limit.",
                low.label(),
                high.label()
            ),
        ));
    }
    Ok((low, high))
}

/// The longest text, in characters, of a tick label or of a tick format's prefix or suffix.
const MAX_TEXT_CHARS: usize = 1000;

/// `n` with a comma between groups of three digits, as `1,000`.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Checks a text argument, shown as `name` in the message (`"prefix"`, `"labels[2]"`), that the
/// Python parameter `parameter` holds: at most 1,000 characters, or `Error::Style` under
/// `context`. Longer text could not fit a figure.
pub(crate) fn check_text(
    context: &'static str,
    parameter: &'static str,
    name: &str,
    text: &str,
) -> Result<()> {
    let count = text.chars().count();
    if count <= MAX_TEXT_CHARS {
        return Ok(());
    }
    Err(Error::style(
        context,
        format!(
            "{name} has {count} characters; text must be at most {max} characters.",
            count = grouped(count),
            max = grouped(MAX_TEXT_CHARS),
        ),
    )
    .with_parameter(parameter)
    .with_hint("shorten the text; it would not fit the figure."))
}

/// The transform of `axis`, of scale kind `kind`, for the tick setter `context`, or
/// `Error::Usage` on a category axis, whose ticks are its categories. `what` names what the
/// setter sets, as `"ticks"` or `"tick formats"`.
pub(super) fn check_tick_axis(
    context: &'static str,
    axis: SharedAxis,
    kind: ScaleKind,
    what: &str,
) -> Result<Transform> {
    if kind == ScaleKind::Time {
        return Err(Error::usage(
            context,
            "this is a time axis; its tick positions must be times.",
        ));
    }
    kind.transform().ok_or_else(|| {
        Error::usage(
            context,
            format!("this is a category axis; {what} apply to linear and log axes only."),
        )
        .with_hint(axis.numeric_scale_hint())
    })
}

/// Checks that `axis`, of scale kind `kind`, takes the tick format the setter `context` sets:
/// `Error::Usage` on a category axis, whose ticks are its categories, as
/// [`check_tick_axis`] reports it. Linear, log and time axes take a format.
pub(super) fn check_format_axis(
    context: &'static str,
    axis: SharedAxis,
    kind: ScaleKind,
) -> Result<()> {
    if kind == ScaleKind::Category {
        return check_tick_axis(context, axis, kind, "tick formats").map(drop);
    }
    Ok(())
}

/// Checks that `axis`, of scale kind `kind`, takes the time tick positions the setter `context`
/// sets: `Error::Usage` unless it is a time axis.
pub(super) fn check_time_tick_axis(
    context: &'static str,
    axis: SharedAxis,
    kind: ScaleKind,
) -> Result<()> {
    match kind {
        ScaleKind::Time => Ok(()),
        ScaleKind::Category => Err(Error::usage(
            context,
            "this is a category axis; ticks apply to linear, log and time axes only.",
        )
        .with_hint(axis.time_scale_hint())),
        ScaleKind::Linear | ScaleKind::Log { .. } => Err(Error::usage(
            context,
            "this is a linear or log axis; its tick positions must be numbers.",
        )),
    }
}

/// The transform of the y axis, of scale kind `kind`, for the mark method `context`, which
/// draws on a numeric y axis only: `Error::Usage` naming `y` on a category or a time axis.
pub(super) fn check_numeric_y(context: &'static str, kind: ScaleKind) -> Result<Transform> {
    let scale = match kind {
        ScaleKind::Linear => return Ok(Transform::Linear),
        ScaleKind::Log { base } => return Ok(Transform::Log(base)),
        ScaleKind::Category => "category",
        ScaleKind::Time => "time",
    };
    Err(Error::usage(
        context,
        format!("y is a {scale} scale; {context} needs a numeric y axis."),
    )
    .with_parameter("y")
    .with_hint("use a linear or log y axis."))
}

/// Checks the y axis, of scale kind `kind`, for the series mark method `context`, whose y values
/// are numbers or times: `Error::Usage` naming `y` on a category axis, as
/// [`check_numeric_y`] reports it.
pub(super) fn check_series_y(context: &'static str, kind: ScaleKind) -> Result<()> {
    if kind == ScaleKind::Category {
        return check_numeric_y(context, kind).map(drop);
    }
    Ok(())
}

/// Refuses the mark method `context`, which needs `needs` along x, such as `"a category x
/// axis"`, on a time x axis, of scale kind `kind`: `Error::Usage` naming `x`. Other kinds pass,
/// for the method's own checks.
pub(super) fn check_no_time_x(context: &'static str, kind: ScaleKind, needs: &str) -> Result<()> {
    if kind != ScaleKind::Time {
        return Ok(());
    }
    Err(Error::usage(
        context,
        format!("x is a time scale; {context} needs {needs}."),
    )
    .with_parameter("x")
    .with_hint("use a category axis with formatted labels."))
}

/// Checks explicit tick `positions` for an axis of `transform`, index by index, and reports the
/// first bad one as `Error::Style` under `context`: each position must be finite, within the
/// supported magnitudes (positive and at least [`MIN_LOG_VALUE`] on a log axis), and above the
/// one before it. The positions are used in the order given, never sorted.
pub(super) fn check_tick_positions(
    context: &'static str,
    transform: Transform,
    positions: &[f64],
) -> Result<()> {
    let log = matches!(transform, Transform::Log(_));
    let fault = |i: usize, message: String| {
        Error::style(context, format!("positions[{i}] is {message}")).with_parameter("positions")
    };
    for (i, &v) in positions.iter().enumerate() {
        if !v.is_finite() {
            return Err(fault(
                i,
                format!("{}; tick positions must be finite.", number(v)),
            ));
        }
        if log && v <= 0.0 {
            return Err(fault(
                i,
                format!(
                    "{} on a log axis; tick positions must be positive.",
                    number(v)
                ),
            )
            .with_hint("use a linear axis, or remove positions that are not positive."));
        }
        if log && !(MIN_LOG_VALUE..=MAX_MAGNITUDE).contains(&v) {
            return Err(fault(
                i,
                format!(
                    "{} on a log axis; tick positions must lie from {MIN_LOG_VALUE:e} to {MAX_MAGNITUDE:e}.",
                    number(v)
                ),
            ));
        }
        if v.abs() > MAX_MAGNITUDE {
            return Err(fault(
                i,
                format!(
                    "{}; tick positions must not exceed {MAX_MAGNITUDE:e} in magnitude.",
                    number(v)
                ),
            ));
        }
        if let Some(&before) = i.checked_sub(1).and_then(|j| positions.get(j))
            && v <= before
        {
            return Err(fault(
                i,
                format!(
                    "{} but positions[{}] is {}; tick positions must be strictly increasing.",
                    number(v),
                    i - 1,
                    number(before)
                ),
            )
            .with_hint(
                "sort the positions and remove repeats; they are used in the order given.",
            ));
        }
    }
    Ok(())
}

/// Checks explicit time tick `positions` index by index and reports the first one that is not
/// above the one before it as `Error::Style` under `context`, as [`check_tick_positions`] does
/// for numbers. The positions are used in the order given, never sorted.
pub(super) fn check_time_tick_positions(
    context: &'static str,
    positions: &[Instant],
) -> Result<()> {
    for (i, pair) in positions.windows(2).enumerate() {
        if pair[1] <= pair[0] {
            return Err(Error::style(
                context,
                format!(
                    "positions[{}] is {} but positions[{i}] is {}; tick positions must be strictly increasing.",
                    i + 1,
                    pair[1].label(),
                    pair[0].label()
                ),
            )
            .with_parameter("positions")
            .with_hint("sort the positions and remove repeats; they are used in the order given."));
        }
    }
    Ok(())
}

/// Checks the `labels` given with `count` tick positions under `context`: one label per
/// position, each at most 1,000 characters, else `Error::Style`.
pub(super) fn check_tick_labels(
    context: &'static str,
    count: usize,
    labels: &[String],
) -> Result<()> {
    if labels.len() != count {
        return Err(Error::style(
            context,
            format!(
                "positions has {count} values but labels has {}; pass one label per position.",
                labels.len()
            ),
        )
        .with_parameter("labels"));
    }
    for (i, label) in labels.iter().enumerate() {
        check_text(context, "labels", &format!("labels[{i}]"), label)?;
    }
    Ok(())
}

/// Checks the sides of a figure, in inches: each must be finite, above 0 and at most 200.
pub(super) fn check_figure_size(width_in: f64, height_in: f64) -> Result<()> {
    for (name, v) in [("width", width_in), ("height", height_in)] {
        if !(v.is_finite() && v > 0.0 && v <= MAX_FIGURE_IN) {
            return Err(Error::style(
                "Figure()",
                format!(
                    "{name} is {v}; it must be finite, above 0 and at most {MAX_FIGURE_IN} inches.",
                    v = number(v),
                ),
            )
            .with_parameter(name));
        }
    }
    Ok(())
}
