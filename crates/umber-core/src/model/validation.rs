//! Input validation of the model: series, bar, histogram, ECDF and density data against their
//! axes' scales, stacked bar totals, histogram bins and statistics, density bandwidths, explicit
//! limits, explicit ticks, text and figure sizes. Every check runs before anything is stored, so
//! a failed call changes nothing.

use std::collections::HashMap;
use std::sync::Arc;

use super::axis_data::{AxisData, SharedValues};
use super::bar::{Arrangement, BarInput, Series};
use super::box_plot::Distribution;
use super::hist::{Bins, GroupedHistOptions, HistOptions, HistStat, HistStyle};
use super::share::SharedAxis;
use crate::binning::{BinCount, BinSpec, HistBinning, HistBins, MAX_BINS};
use crate::error::number;
use crate::scale::{MAX_MAGNITUDE, MIN_LOG_VALUE, too_narrow};
use crate::stacking::Totals;
use crate::text::Lines;
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
pub(crate) const MAX_FIGURE_IN: f64 = 200.0;

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
            return Err(Error::at_input(
                context,
                name,
                "",
                [(
                    Some(i),
                    format!(" is {v}; values must be finite.", v = number(v)),
                )],
            )
            .with_parameter(parameter)
            .with_hint(infinite_hint));
        }
        if v.abs() > MAX_MAGNITUDE {
            return Err(Error::at_input(
                context,
                name,
                "",
                [(
                    Some(i),
                    format!(
                        " is {v}; magnitudes must not exceed {MAX_MAGNITUDE:e}.",
                        v = number(v)
                    ),
                )],
            )
            .with_parameter(parameter));
        }
        if log {
            if v <= 0.0 {
                return Err(Error::at_input(
                    context,
                    name,
                    "",
                    [(
                        Some(i),
                        format!(
                            " is {v} on a log axis; values must be positive.",
                            v = number(v)
                        ),
                    )],
                )
                .with_parameter(parameter)
                .with_hint("use a linear axis, or remove values that are not positive."));
            }
            if v < MIN_LOG_VALUE {
                return Err(Error::at_input(
                    context,
                    name,
                    "",
                    [(
                        Some(i),
                        format!(
                            " is {v} on a log axis; values must be at least {MIN_LOG_VALUE:e}.",
                            v = number(v),
                        ),
                    )],
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
        (AxisData::Categories(names), ScaleKind::Category) => check_names(context, name, names),
        (AxisData::Times(_), ScaleKind::Time) => Ok(()),
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
    x: &SharedValues,
    gap_hint: &'static str,
) -> Result<()> {
    match x {
        SharedValues::Numbers(values) => {
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
        SharedValues::Times(_) if x_kind == ScaleKind::Time => Ok(()),
        SharedValues::Times(_) => Err(times_off_a_time_axis(context, SharedAxis::X, x_kind)),
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
            return Err(Error::at_input(
                context,
                "x",
                "",
                [
                    (Some(i), format!(" is {}, below ", t.label())),
                    (
                        Some(j),
                        format!(" = {}; {mark} needs x in non-decreasing order.", u.label()),
                    ),
                ],
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
fn check_shared_sorted(context: &'static str, mark: &str, x: &SharedValues) -> Result<()> {
    match x {
        SharedValues::Numbers(values) => check_sorted(context, mark, values),
        SharedValues::Times(times) => check_sorted_times(context, mark, times),
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
            return Err(Error::at_input(
                context,
                "x",
                "",
                [
                    (Some(i), format!(" is {}, below ", number(v))),
                    (
                        Some(j),
                        format!(" = {}; {mark} needs x in non-decreasing order.", number(u)),
                    ),
                ],
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
    (x, low, high): (&SharedValues, &[f64], &[f64]),
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
    x: &SharedValues,
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

/// The `Error::Usage` of the numeric bubble method `context` on a time axis, `axis` being `"x"`
/// or `"y"`: it takes numbers, and times go through `bubbles_time`.
pub(super) fn time_bubbles_refusal(context: &'static str, axis: &'static str) -> Error {
    Error::usage(
        context,
        format!(
            "{axis} is a time scale; {context} takes numbers, so pass times with bubbles_time()."
        ),
    )
    .with_parameter(axis)
    .with_hint("use bubbles_time() for times.")
}

/// Checks bubble coordinates and sizes before storage: numeric x, lengths, coordinates, sizes.
pub(super) fn check_bubbles(
    context: &'static str,
    x_kind: ScaleKind,
    y_transform: Transform,
    (x, y, sizes): (&[f64], &[f64], &[f64]),
) -> Result<()> {
    if x_kind == ScaleKind::Time {
        return Err(time_bubbles_refusal(context, "x"));
    }
    let Some(x_transform) = x_kind.transform() else {
        return Err(Error::usage(
            context,
            "this is a category axis; bubbles apply to linear and log axes only.",
        )
        .with_parameter("x")
        .with_hint(X_SCALE_HINT));
    };
    check_bubble_lengths(context, x.len(), y.len(), sizes.len())?;
    check_numbers(context, ("x", "x"), x, x_transform, SCATTER_GAP_HINT)?;
    check_numbers(context, ("y", "y"), y, y_transform, SCATTER_GAP_HINT)?;
    check_bubble_sizes(context, sizes)
}

/// Checks that the `y` and `sizes` of a bubble mark have as many values as its `x`.
fn check_bubble_lengths(context: &'static str, x: usize, y: usize, sizes: usize) -> Result<()> {
    for (name, other) in [("y", y), ("sizes", sizes)] {
        if x != other {
            return Err(Error::data(
                context,
                format!(
                    "x has {x} values but {name} has {other}; x and {name} must have the same length."
                ),
            ));
        }
    }
    Ok(())
}

/// Checks that every bubble size is finite, at least 0 and at most [`MAX_MAGNITUDE`].
fn check_bubble_sizes(context: &'static str, sizes: &[f64]) -> Result<()> {
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

/// Checks bubble coordinates and sizes before storage on axes that may be time axes, with an x
/// axis of scale kind `x_kind` and a y axis of scale kind `y_kind`, and returns the coordinates
/// as the mark stores them. The checks run in a fixed order and the first failure is the
/// error, under `context`: neither axis is a category axis (`Error::Usage`, x first); the
/// lengths; `x`, then `y`, against their axes (see [`check_axis_data`]); the sizes.
pub(super) fn check_time_bubbles(
    context: &'static str,
    (x_kind, y_kind): (ScaleKind, ScaleKind),
    (x, y): (AxisData, AxisData),
    sizes: &[f64],
) -> Result<(SharedValues, SharedValues)> {
    for (axis, kind) in [(SharedAxis::X, x_kind), (SharedAxis::Y, y_kind)] {
        if kind == ScaleKind::Category {
            let hint = match axis {
                SharedAxis::X => {
                    "pass x=Scale.linear(), x=Scale.log() or x=Scale.time() to Figure.add_axes()."
                }
                SharedAxis::Y => {
                    "pass y=Scale.linear(), y=Scale.log() or y=Scale.time() to Figure.add_axes()."
                }
            };
            return Err(Error::usage(
                context,
                "this is a category axis; bubbles apply to linear, log and time axes only.",
            )
            .with_parameter(axis.name())
            .with_hint(hint));
        }
    }
    check_bubble_lengths(context, x.len(), y.len(), sizes.len())?;
    check_axis_data(context, SharedAxis::X, x_kind, &x, SCATTER_GAP_HINT)?;
    check_axis_data(context, SharedAxis::Y, y_kind, &y, SCATTER_GAP_HINT)?;
    check_bubble_sizes(context, sizes)?;
    Ok((shared_of(x), shared_of(y)))
}

/// The shared values of `data`, which holds numbers or times: a category axis, the only
/// place for names, is refused before.
fn shared_of(data: AxisData) -> SharedValues {
    match data {
        AxisData::Numbers(values) => SharedValues::Numbers(values.into()),
        AxisData::Times(times) => SharedValues::Times(times.into()),
        AxisData::Categories(_) => SharedValues::Numbers(Arc::from([])),
    }
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
/// scale kind `x_kind` and a y axis of scale kind `y_kind`, before they are stored, so a failed
/// call changes nothing. The checks run in a fixed order and the first failure is the error,
/// under `context`:
///
/// 1. the x axis is not a category axis, nor a time axis (`Error::Usage`);
/// 2. `y`, `low` and `high` have the same length;
/// 3. `y` against the y axis: names on a category axis, numbers on a linear or log one, times
///    on a time one (see [`check_axis_data`]), then `low` and `high` against the x axis (see
///    [`check_numbers`]);
/// 4. no `low[i]` exceeds `high[i]`; an index where either is NaN is not compared.
pub(super) fn check_x_error_bars(
    context: &'static str,
    x_kind: ScaleKind,
    y_kind: ScaleKind,
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

/// Validates the positions and the time limits `low` and `high` of error bars whose limits lie
/// along `value_axis`, on axes with an x axis of scale kind `x_kind` and a y axis of scale kind
/// `y_kind`, before they are stored. The checks run in this order, the first failure the error
/// under `context`:
///
/// 1. the value axis is a time axis (`Error::Usage`);
/// 2. `positions`, `low` and `high` have the same length;
/// 3. `positions` against the other axis (see [`check_axis_data`]);
/// 4. no `low[i]` is after `high[i]`; an index where either is missing is not compared.
pub(super) fn check_time_error_bars(
    context: &'static str,
    value_axis: SharedAxis,
    (x_kind, y_kind): (ScaleKind, ScaleKind),
    positions: &AxisData,
    (low, high): (&[Option<Instant>], &[Option<Instant>]),
) -> Result<()> {
    let (value_kind, along, along_kind) = match value_axis {
        SharedAxis::X => (x_kind, SharedAxis::Y, y_kind),
        SharedAxis::Y => (y_kind, SharedAxis::X, x_kind),
    };
    if value_kind != ScaleKind::Time {
        return Err(Error::usage(
            context,
            format!(
                "the {} axis is not a time axis; error bars between times need one.",
                value_axis.name()
            ),
        )
        .with_hint(value_axis.time_scale_hint()));
    }
    let name = along.name();
    for (other, count) in [("low", low.len()), ("high", high.len())] {
        if positions.len() != count {
            return Err(Error::data(
                context,
                format!(
                    "{name} has {} values but {other} has {count}; {name}, low and high must have the same length.",
                    positions.len()
                ),
            ));
        }
    }
    check_axis_data(context, along, along_kind, positions, ERROR_BAR_GAP_HINT)?;
    for (i, (a, b)) in low.iter().zip(high).enumerate() {
        if let (Some(a), Some(b)) = (a, b)
            && a > b
        {
            return Err(Error::data(
                context,
                format!(
                    "low[{i}] is {}, above high[{i}] = {}; low must not exceed high.",
                    a.label(),
                    b.label()
                ),
            )
            .with_parameter("low")
            .with_hint("check that low and high are not swapped."));
        }
    }
    Ok(())
}

/// Checks one event row, retaining its group index and the positions highlight.
pub(super) fn check_event_positions(x: ScaleKind, index: usize, data: &AxisData) -> Result<()> {
    const CONTEXT: &str = "Axes.events()";
    match (data, x) {
        (AxisData::Times(_), ScaleKind::Time) => Ok(()),
        (AxisData::Numbers(values), ScaleKind::Linear | ScaleKind::Log { .. }) => {
            let transform = match x {
                ScaleKind::Log { base } => Transform::Log(base),
                _ => Transform::Linear,
            };
            check_numbers(
                CONTEXT,
                (&format!("positions[{index}]"), "positions"),
                values,
                transform,
                "use NaN to leave an event out.",
            )
        }
        _ => Err(Error::data(
            CONTEXT,
            format!(
                "positions[{index}] does not match the x axis; \
                 pass numbers on numeric axes or times on a time axis."
            ),
        )
        .with_parameter("positions")
        .with_hint(
            "use x=Scale.time() for dates and times, or a linear or log x axis for numbers.",
        )),
    }
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
        ("categories", "category", SharedAxis::X),
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
    input: BarInput,
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
        ("categories", "category", SharedAxis::X),
        x_kind,
        categories,
        "combine repeated categories and sum their heights within each series.",
    )?;
    let (parameter, hint) = match input {
        BarInput::Heights => ("heights", BAR_GAP_HINT),
        BarInput::Series => ("series", "use NaN to omit that height."),
    };
    for (i, s) in series.iter().enumerate() {
        check_numbers(
            context,
            (&format!("series[{i}].values"), parameter),
            &s.values,
            y_transform,
            hint,
        )
        .map_err(|error| error.in_group(i))?;
    }
    if arrangement == Arrangement::Stacked {
        check_stacked_totals(context, categories, series)?;
    }
    Ok(())
}

#[test]
fn bar_input_details_do_not_depend_on_context() {
    for context in ["Axes.bar()", "Axes.grouped_bar()", "Another API()"] {
        for (input, parameter, hint) in [
            (BarInput::Heights, "heights", BAR_GAP_HINT),
            (BarInput::Series, "series", "use NaN to omit that height."),
        ] {
            let error = check_bar_group(
                context,
                Arrangement::Grouped,
                input,
                ScaleKind::Category,
                Transform::Linear,
                &["A".into()],
                &[Series {
                    values: vec![f64::INFINITY],
                    ..Series::default()
                }],
            )
            .unwrap_err();
            assert_eq!(error.parameter(), Some(parameter));
            assert_eq!(error.hint(), Some(hint));
            assert!(error.to_string().starts_with(context));
        }
    }
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
        ("categories", "category", SharedAxis::X),
        x,
        categories,
        "combine the samples of repeated categories.",
    )
}

/// Checks that a distribution has one group per category, naming its container on failure.
pub(super) fn check_group_count(
    context: &'static str,
    name: &str,
    parameter: &'static str,
    count: usize,
    (category_name, categories): (&str, usize),
    hint: &'static str,
) -> Result<()> {
    if count != categories {
        return Err(Error::data(
            context,
            format!("{name} has {count} groups; {category_name} has {categories}."),
        )
        .with_parameter(parameter)
        .with_hint(hint));
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
    check_group_count(
        context,
        "groups",
        "groups",
        groups.len(),
        ("categories", categories.len()),
        "pass one group per category, using an empty group when there are no samples.",
    )?;
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
            ("categories", categories.len()),
            "pass one group per category, using an empty group when there are no samples.",
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

/// The hint for an infinite histogram value.
const HIST_GAP_HINT: &str = "a histogram ignores NaN, so use it to leave a value out.";

/// The hint for bin edges that are too few or not finite.
const EDGES_HINT: &str = "pass at least two increasing edges, or a number of bins.";

/// The hint for a linear x axis that a mark needs.
const LINEAR_X_HINT: &str = "pass x=Scale.linear() to Figure.add_axes().";

/// The hint for a linear y axis that a mark needs.
const LINEAR_Y_HINT: &str = "pass y=Scale.linear() to Figure.add_axes().";

/// Validates the options and values of a histogram for axes with an x axis of scale kind
/// `x_kind`, before it is stored, so a failed call changes nothing, and returns its binning.
/// The y axis, linear or log, takes any height. The checks run in a fixed order and the first
/// failure is the error, under `context`:
///
/// 1. the bins and the statistic, by [`check_binning`];
/// 2. the x axis, by [`check_hist_x`];
/// 3. every value is finite (NaN is allowed and ignored) and within the supported magnitudes
///    (`Error::Data`);
/// 4. explicit edges hold every finite value (`Error::Data`, naming how many fall outside);
/// 5. every bar height is finite and within the supported magnitudes, by [`check_heights`].
pub(super) fn check_hist(
    context: &'static str,
    x_kind: ScaleKind,
    values: &[f64],
    options: &HistOptions,
) -> Result<HistBinning> {
    let binning = check_binning(
        context,
        &options.bins,
        options.style,
        options.cumulative,
        options.stat,
    )?;
    check_hist_x(context, x_kind)?;
    check_numbers(
        context,
        ("values", "values"),
        values,
        Transform::Linear,
        HIST_GAP_HINT,
    )?;
    check_inside_edges(context, "values", values, binning.bins())?;
    check_heights(context, &binning, &[values])?;
    Ok(binning)
}

/// Validates the options and series of a grouped histogram for axes with an x axis of scale
/// kind `x_kind`, before it is stored, and returns its binning. The checks run in a fixed order
/// and the first failure is the error, under `context`:
///
/// 1. there is at least one series (`Error::Data`);
/// 2. the bins and the statistic, by [`check_binning`];
/// 3. the style is not [`HistStyle::Bars`], whose bars of several series would hide each other
///    (`Error::Usage`);
/// 4. the x axis, by [`check_hist_x`];
/// 5. the values of each series, in series order, as for a histogram;
/// 6. explicit edges hold every finite value of each series, in series order;
/// 7. every bar and stack height is finite and within the supported magnitudes, by
///    [`check_heights`].
pub(super) fn check_grouped_hist(
    context: &'static str,
    x_kind: ScaleKind,
    series: &[Series],
    options: &GroupedHistOptions,
) -> Result<HistBinning> {
    if series.is_empty() {
        return Err(
            Error::data(context, "series is empty; pass at least one series.")
                .with_parameter("series"),
        );
    }
    let binning = check_binning(
        context,
        &options.bins,
        options.style,
        options.cumulative,
        options.stat,
    )?;
    if options.style == HistStyle::Bars {
        return Err(Error::usage(
            context,
            "style is Hist.BARS with groups; bars of several groups would hide each other.",
        )
        .with_parameter("style")
        .with_hint("use Hist.STACKED or Hist.STEP."));
    }
    check_hist_x(context, x_kind)?;
    for (i, s) in series.iter().enumerate() {
        check_numbers(
            context,
            (&format!("series[{i}].values"), "values"),
            &s.values,
            Transform::Linear,
            HIST_GAP_HINT,
        )
        .map_err(|error| error.in_group(i))?;
    }
    for (i, s) in series.iter().enumerate() {
        check_inside_edges(
            context,
            &format!("series[{i}].values"),
            &s.values,
            binning.bins(),
        )
        .map_err(|error| error.in_group(i))?;
    }
    let values: Vec<&[f64]> = series.iter().map(|s| s.values.as_slice()).collect();
    check_heights(context, &binning, &values)?;
    Ok(binning)
}

/// Validates the bins, style, accumulation and statistic of a histogram and returns them as
/// its binning, in this order, under `context`:
///
/// 1. a bin count is within `1..=MAX_BINS` (`Error::Style`); edges are checked by
///    [`check_edges`] (`Error::Data`);
/// 2. a cumulative histogram does not show densities, which do not add up (`Error::Usage`).
fn check_binning(
    context: &'static str,
    bins: &Bins,
    style: HistStyle,
    cumulative: bool,
    stat: HistStat,
) -> Result<HistBinning> {
    let spec = match bins {
        &Bins::Count(n) => BinSpec::Count(check_bin_count(context, n)?),
        Bins::Edges(edges) => {
            check_edges(context, edges)?;
            BinSpec::Edges(Arc::from(edges.as_slice()))
        }
    };
    if cumulative && stat == HistStat::Density {
        return Err(Error::usage(
            context,
            "cumulative is True with Stat.DENSITY; a cumulative histogram shows counts or \
             fractions.",
        )
        .with_parameter("cumulative")
        .with_hint("use Stat.FRACTION, or ecdf()."));
    }
    Ok(HistBinning::new(spec, style, cumulative, stat))
}

/// The bin count `bins`, within `1..=MAX_BINS`; `Error::Style` naming `bins` otherwise.
fn check_bin_count(context: &'static str, bins: i64) -> Result<BinCount> {
    BinCount::new(bins).ok_or_else(|| {
        let error = Error::style(
            context,
            format!("bins is {bins}; bins must be between 1 and {MAX_BINS}."),
        )
        .with_parameter("bins");
        if bins < 1 {
            error.with_hint("use at least one bin.")
        } else {
            error
        }
    })
}

/// Checks explicit bin `edges`, in this order, each failure an `Error::Data` naming `bins`:
/// there are at least two; each, in order, is finite and within the supported magnitudes; each
/// is above the one before it; and together they span enough for a linear axis to show.
fn check_edges(context: &'static str, edges: &[f64]) -> Result<()> {
    if edges.len() < 2 {
        let count = edges.len();
        let noun = if count == 1 { "edge" } else { "edges" };
        return Err(Error::data(
            context,
            format!("bins has {count} {noun}; bin edges must number at least 2."),
        )
        .with_parameter("bins")
        .with_hint(EDGES_HINT));
    }
    for (i, &v) in edges.iter().enumerate() {
        if !v.is_finite() {
            return Err(Error::data(
                context,
                format!("bins[{i}] is {v}; bin edges must be finite.", v = number(v)),
            )
            .with_parameter("bins")
            .with_hint(EDGES_HINT));
        }
        if v.abs() > MAX_MAGNITUDE {
            return Err(Error::data(
                context,
                format!(
                    "bins[{i}] is {v}; magnitudes must not exceed {MAX_MAGNITUDE:e}.",
                    v = number(v)
                ),
            )
            .with_parameter("bins"));
        }
    }
    if let Some(i) = (1..edges.len()).find(|&i| edges[i] <= edges[i - 1]) {
        return Err(Error::data(
            context,
            format!(
                "bins[{i}] is {v}, not above bins[{before}], {p}; bin edges must be strictly \
                 increasing.",
                v = number(edges[i]),
                before = i - 1,
                p = number(edges[i - 1]),
            ),
        )
        .with_parameter("bins")
        .with_hint("sort the edges and remove repeated ones."));
    }
    let (lo, hi) = (edges[0], edges[edges.len() - 1]);
    if too_narrow(lo, hi) {
        return Err(Error::data(
            context,
            format!(
                "bins span {lo} to {hi}, which is narrower than a linear axis can show; bin \
                 edges must span more.",
                lo = number(lo),
                hi = number(hi),
            ),
        )
        .with_parameter("bins")
        .with_hint("widen the edges."));
    }
    Ok(())
}

/// Checks that explicit bin edges `spec` hold every finite value of `values`, shown as `name`:
/// `Error::Data` naming `values` and how many fall outside. Equal-width bins hold every value.
fn check_inside_edges(
    context: &'static str,
    name: &str,
    values: &[f64],
    spec: &BinSpec,
) -> Result<()> {
    let BinSpec::Edges(edges) = spec else {
        return Ok(());
    };
    let (lo, hi) = (edges[0], edges[edges.len() - 1]);
    let outside = values
        .iter()
        .filter(|v| v.is_finite() && !(lo <= **v && **v <= hi))
        .count();
    if outside == 0 {
        return Ok(());
    }
    let noun = if outside == 1 { "value" } else { "values" };
    Err(Error::at_input(
        context,
        name,
        "",
        [(
            None,
            format!(
                " has {outside} finite {noun} outside the bin edges, which span {lo} to {hi}; \
             every finite value must lie within the edges.",
                lo = number(lo),
                hi = number(hi),
            ),
        )],
    )
    .with_parameter("values")
    .with_hint("widen the edges or filter the values first."))
}

/// Checks that the bars of a histogram of `groups` binned by `binning` can be drawn: in bin
/// order, every bar's height, and with several stacked groups every stack's height, is finite
/// and at most [`MAX_MAGNITUDE`], or `Error::Data` naming `bins` and the bin. Only densities can
/// fail: dividing by a very narrow bin's width can overflow, while counts and fractions stay
/// small.
fn check_heights(context: &'static str, binning: &HistBinning, groups: &[&[f64]]) -> Result<()> {
    if binning.stat() != HistStat::Density {
        return Ok(());
    }
    let Some(bins) = HistBins::new(binning, groups) else {
        return Ok(());
    };
    let stacked = binning.style() == HistStyle::Stacked && bins.groups() > 1;
    let refused = |i: usize, what: &str, height: f64| {
        Error::data(
            context,
            format!(
                "bin {i}, from {lo} to {hi}, has a {what} of {h}; bar heights must be finite and \
                 at most {MAX_MAGNITUDE:e}.",
                lo = number(bins.edge(i)),
                hi = number(bins.edge(i + 1)),
                h = number(height),
            ),
        )
        .with_parameter("bins")
        .with_hint("widen the narrow bins, or use Stat.COUNT or Stat.FRACTION.")
    };
    let drawable = |height: f64| height.is_finite() && height <= MAX_MAGNITUDE;
    for (i, top) in bins.tops().enumerate() {
        for g in 0..bins.groups() {
            let height = bins.heights(g)[i];
            if !drawable(height) {
                return Err(refused(i, "height", height).in_group(g));
            }
        }
        if stacked && !drawable(top) {
            return Err(refused(i, "stack height", top));
        }
    }
    Ok(())
}

/// Checks that the x axis, of scale kind `x_kind`, is the linear axis a histogram needs: a
/// category x axis is `Error::Data`, a log x axis, which histograms do not support, is
/// `Error::Usage`, and a time x axis is refused as [`check_no_time_x`] refuses it.
fn check_hist_x(context: &'static str, x_kind: ScaleKind) -> Result<()> {
    match x_kind {
        ScaleKind::Linear => Ok(()),
        ScaleKind::Category => Err(Error::data(
            context,
            "values holds numbers but the x axis is a category axis; use a linear x axis.",
        )
        .with_hint(LINEAR_X_HINT)),
        ScaleKind::Log { .. } => Err(Error::usage(
            context,
            "the x axis is a log axis, which histograms do not support; use a linear x axis.",
        )
        .with_hint(LINEAR_X_HINT)),
        ScaleKind::Time => check_no_time_x(context, x_kind, "a linear x axis"),
    }
}

/// Checks that names lie on a category axis and that none appears twice.
pub(super) fn check_categories(
    context: &'static str,
    (parameter, singular, axis): (&'static str, &str, SharedAxis),
    kind: ScaleKind,
    categories: &[String],
    repeat_hint: &'static str,
) -> Result<()> {
    if kind != ScaleKind::Category {
        let name = axis.name();
        return Err(Error::data(
            context,
            format!(
                "{parameter} holds names but the {name} axis is not a category axis; \
                 use a category {name} axis."
            ),
        )
        .with_hint(match axis {
            SharedAxis::X => "pass x=Scale.category() to Figure.add_axes().",
            SharedAxis::Y => "pass y=Scale.category() to Figure.add_axes().",
        }));
    }
    check_names(context, parameter, categories)?;
    let mut first_index: HashMap<&str, usize> = HashMap::with_capacity(categories.len());
    for (i, name) in categories.iter().enumerate() {
        if let Some(first) = first_index.insert(name, i) {
            return Err(Error::data(
                context,
                format!(
                    "{parameter}[{i}] repeats {name:?} from {parameter}[{first}]; \
                     each {singular} must appear once."
                ),
            )
            .with_parameter(parameter)
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
pub(super) fn grouped(n: usize) -> String {
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

/// Checks a text argument of one line, shown as `name` in the message (`"prefix"`,
/// `"labels[2]"`), that the Python parameter `parameter` holds: at most 1,000 characters, and
/// no line break or other control character (see [`Lines`]), or `Error::Style` under
/// `context`. Longer text could not fit a figure. `what` names what the text is, in the plural,
/// for the message on a line break: `"tick labels"`.
pub(crate) fn check_text(
    context: &'static str,
    parameter: &'static str,
    name: &str,
    text: &str,
    what: &'static str,
) -> Result<()> {
    let count = text.chars().count();
    if count > MAX_TEXT_CHARS {
        return Err(Error::style(
            context,
            format!(
                "{name} has {count} characters; text must be at most {max} characters.",
                count = grouped(count),
                max = grouped(MAX_TEXT_CHARS),
            ),
        )
        .with_parameter(parameter)
        .with_hint("shorten the text; it would not fit the figure."));
    }
    Lines::One(what)
        .check_style(context, name, text)
        .map_err(|error| error.with_parameter(parameter))
}

/// Checks the names in `names`, the data of the Python parameter `parameter` shown as
/// `{parameter}[i]` in the message, for characters a one-line name may not hold (see
/// [`Lines`]): `Error::Data` under `context` for the first name that holds one.
pub(super) fn check_names(
    context: &'static str,
    parameter: &'static str,
    names: &[String],
) -> Result<()> {
    for (i, name) in names.iter().enumerate() {
        CATEGORY_NAMES
            .check_data(context, &format!("{parameter}[{i}]"), name)
            .map_err(|error| error.with_parameter(parameter))?;
    }
    Ok(())
}

/// Category names are one line.
const CATEGORY_NAMES: Lines = Lines::One("category names");

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

/// The transform of the x axis, of scale kind `kind`, for the mark method `context` on a
/// category y axis, where it draws beside a numeric x axis only: `Error::Usage` naming `x` on a
/// category axis. Two category axes take no mark of these methods.
pub(super) fn check_numeric_x(context: &'static str, kind: ScaleKind) -> Result<Transform> {
    let scale = match kind {
        ScaleKind::Linear => return Ok(Transform::Linear),
        ScaleKind::Log { base } => return Ok(Transform::Log(base)),
        ScaleKind::Category => "category",
        ScaleKind::Time => "time",
    };
    Err(Error::usage(
        context,
        format!("x is a {scale} scale; {context} needs a numeric x axis."),
    )
    .with_parameter("x")
    .with_hint("use a linear or log x axis."))
}

/// For the mark method `context`, which draws on a numeric y axis and also on a category y
/// axis beside a numeric x axis, on axes with an x axis of scale kind `x` and a y axis of
/// scale kind `y`: `Error::Usage` naming `x` on two category axes (see [`check_numeric_x`]).
pub(super) fn check_not_two_categories(
    context: &'static str,
    x: ScaleKind,
    y: ScaleKind,
) -> Result<()> {
    if y == ScaleKind::Category {
        check_numeric_x(context, x)?;
    }
    Ok(())
}

/// For the mark method `context` of the bar, box and violin families, on axes with an x axis
/// of scale kind `x` and a y axis of scale kind `y`: the scale kind of the axis the categories
/// lie along and the transform of the axis the values are measured on. On a numeric y axis they
/// are `x` and the y transform, and the mark's own checks then require a category x axis; on a
/// category y axis the marks are horizontal, so they are `y` and the transform of the x axis,
/// which must be numeric (see [`check_numeric_x`]). Neither family draws on a time axis: a
/// time y axis is refused as [`check_numeric_y`] refuses it, and a time x axis beside a
/// numeric y axis as [`check_no_time_x`] does.
pub(super) fn check_band_axes(
    context: &'static str,
    x: ScaleKind,
    y: ScaleKind,
) -> Result<(ScaleKind, Transform)> {
    match y {
        ScaleKind::Category => Ok((y, check_numeric_x(context, x)?)),
        ScaleKind::Time => check_numeric_y(context, y).map(|values| (x, values)),
        ScaleKind::Linear | ScaleKind::Log { .. } => {
            check_no_time_x(context, x, "a category x axis")?;
            Ok((x, check_numeric_y(context, y)?))
        }
    }
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
        check_text(
            context,
            "labels",
            &format!("labels[{i}]"),
            label,
            "tick labels",
        )?;
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

/// The hint for values without a finite one.
const NO_FINITE_HINT: &str = "pass at least one value that is not NaN.";

/// Checks the axes of an ECDF, with an x axis of scale kind `x_kind` and a y axis of scale kind
/// `y_kind`, and its `values`, before it is stored, in this order, under `context`:
///
/// 1. the y axis is linear ([`check_linear_y`]): an ECDF runs from 0 to 1, which a log axis
///    cannot hold (`Error::Usage` naming `y`);
/// 2. the values suit the x axis: numbers on a linear or log axis, checked by [`check_numbers`]
///    (NaN is allowed and ignored), times on a time axis (`Error::Data`);
/// 3. at least one value is finite, or a time (`Error::Data`).
///
/// Returns the values as the samples they are: numbers or times.
pub(super) fn check_ecdf<'a>(
    context: &'static str,
    (x_kind, y_kind): (ScaleKind, ScaleKind),
    values: &'a AxisData,
) -> Result<Samples<'a>> {
    check_linear_y(context, y_kind, "an ECDF runs from 0 to 1")?;
    let wrong_axis = |message: &str, hint: &'static str| {
        Error::data(context, message.to_owned())
            .with_parameter("values")
            .with_hint(hint)
    };
    let (samples, present) = match (values, x_kind) {
        (AxisData::Numbers(_), ScaleKind::Category) => {
            return Err(Error::data(
                context,
                "values holds numbers but the x axis is a category axis; an ECDF needs a \
                 linear, log or time x axis.",
            )
            .with_hint("use a linear, log or time x axis."));
        }
        (AxisData::Numbers(_), ScaleKind::Time) => {
            return Err(wrong_axis(
                "values holds numbers but the x axis is a time axis; pass times.",
                X_SCALE_HINT,
            ));
        }
        (AxisData::Numbers(numbers), ScaleKind::Linear) => (
            Samples::Numbers(numbers),
            check_ecdf_numbers(context, numbers, Transform::Linear)?,
        ),
        (AxisData::Numbers(numbers), ScaleKind::Log { base }) => (
            Samples::Numbers(numbers),
            check_ecdf_numbers(context, numbers, Transform::Log(base))?,
        ),
        (AxisData::Times(times), ScaleKind::Time) => {
            (Samples::Times(times), times.iter().any(Option::is_some))
        }
        (AxisData::Times(_), _) => {
            return Err(wrong_axis(
                "values holds times but the x axis is not a time axis; pass numbers.",
                SharedAxis::X.time_scale_hint(),
            ));
        }
        (AxisData::Categories(_), _) => {
            return Err(wrong_axis(
                "values holds category names; an ECDF needs numbers or times.",
                "pass numbers, or times on a time x axis.",
            ));
        }
    };
    if !present {
        return Err(Error::data(
            context,
            "values has no finite value; an ECDF needs at least one.",
        )
        .with_parameter("values")
        .with_hint(NO_FINITE_HINT));
    }
    Ok(samples)
}

/// The validated values of a distribution along a numeric or time axis: numbers, NaN for a
/// gap, or times, `None` for a gap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Samples<'a> {
    /// Numbers on a linear or log axis.
    Numbers(&'a [f64]),
    /// Times on a time axis.
    Times(&'a [Option<Instant>]),
}

/// Checks that the y axis, of scale kind `y_kind`, is the linear axis that the mark method
/// `context` needs, since `why` (such as "an ECDF runs from 0 to 1"): `Error::Usage` naming `y`
/// otherwise, with the one hint that leads to a linear axis.
fn check_linear_y(context: &'static str, y_kind: ScaleKind, why: &str) -> Result<()> {
    let message = match y_kind {
        ScaleKind::Linear => return Ok(()),
        ScaleKind::Log { .. } => {
            format!("the y axis is a log axis; {why} and needs a linear y axis.")
        }
        ScaleKind::Category => format!("y is a category scale; {context} needs a linear y axis."),
        ScaleKind::Time => format!("y is a time scale; {context} needs a linear y axis."),
    };
    Err(Error::usage(context, message)
        .with_parameter("y")
        .with_hint(LINEAR_Y_HINT))
}

/// Checks the numbers of an ECDF on an axis of `transform` with [`check_numbers`], and says
/// whether one of them is finite.
fn check_ecdf_numbers(
    context: &'static str,
    numbers: &[f64],
    transform: Transform,
) -> Result<bool> {
    check_numbers(
        context,
        ("values", "values"),
        numbers,
        transform,
        "an ECDF ignores NaN, so use it to leave a value out.",
    )?;
    Ok(numbers.iter().any(|v| v.is_finite()))
}

/// Checks the axes of a density curve, with an x axis of scale kind `x_kind` and a y axis of
/// scale kind `y_kind`, in this order, under `context`:
///
/// 1. the y axis is linear ([`check_linear_y`]): the curve runs down to 0, which a log axis
///    cannot hold (`Error::Usage` naming `y`);
/// 2. the x axis is linear: a category x axis is `Error::Data`, and a log or time x axis, which
///    density curves do not support, is `Error::Usage`.
pub(super) fn check_kde_axes(
    context: &'static str,
    (x_kind, y_kind): (ScaleKind, ScaleKind),
) -> Result<()> {
    check_linear_y(context, y_kind, "a density curve runs down to 0")?;
    let unsupported = match x_kind {
        ScaleKind::Linear => return Ok(()),
        ScaleKind::Category => {
            return Err(Error::data(
                context,
                "values holds numbers but the x axis is a category axis; use a linear x axis.",
            )
            .with_hint(LINEAR_X_HINT));
        }
        ScaleKind::Log { .. } => "log",
        ScaleKind::Time => "time",
    };
    Err(Error::usage(
        context,
        format!(
            "the x axis is a {unsupported} axis, which density curves do not support; use a \
             linear x axis."
        ),
    )
    .with_hint(LINEAR_X_HINT))
}

/// Checks a bandwidth given in data units under `context`: finite, greater than 0 and within
/// the supported magnitudes (`Error::Style` naming `bandwidth`).
pub(super) fn check_bandwidth(context: &'static str, bandwidth: f64) -> Result<()> {
    const HINT: &str = "pass a bandwidth in data units greater than 0, or ub.AUTO.";
    let required = if !(bandwidth.is_finite() && bandwidth > 0.0) {
        "bandwidth must be a finite number greater than 0".to_owned()
    } else if bandwidth > MAX_MAGNITUDE {
        format!("magnitudes must not exceed {MAX_MAGNITUDE:e}")
    } else {
        return Ok(());
    };
    Err(Error::style(
        context,
        format!("bandwidth is {v}; {required}.", v = number(bandwidth)),
    )
    .with_parameter("bandwidth")
    .with_hint(HINT))
}

/// Checks the `values` of a density curve, shown as `name`, under `context`: each is finite
/// (NaN is allowed and ignored) and within the supported magnitudes, as [`check_numbers`]
/// checks them, and at least one is finite (`Error::Data` naming `values`).
pub(super) fn check_kde_values(context: &'static str, name: &str, values: &[f64]) -> Result<()> {
    check_numbers(
        context,
        (name, "values"),
        values,
        Transform::Linear,
        "a density curve ignores NaN, so use it to leave a value out.",
    )?;
    if values.iter().all(|v| v.is_nan()) {
        return Err(Error::at_input(
            context,
            name,
            "",
            [(
                None,
                " has no finite value; a density curve needs at least one.".into(),
            )],
        )
        .with_parameter("values")
        .with_hint(NO_FINITE_HINT));
    }
    Ok(())
}
