//! Input validation of the model: series, bar and histogram data against their axes' scales,
//! stacked bar totals, histogram bin counts, explicit limits, explicit ticks, text and figure
//! sizes. Every check runs before anything is stored, so a failed call changes nothing.

use std::collections::HashMap;

use super::axis_data::AxisData;
use super::bar::{Arrangement, BarSeries};
use crate::binning::{BinCount, MAX_BINS};
use crate::error::number;
use crate::scale::{MAX_MAGNITUDE, MIN_LOG_VALUE};
use crate::stacking::Totals;
use crate::{ContinuousScale, Error, Result, ScaleKind, Transform};

/// The hint for an x axis of the wrong kind for limits or numeric data.
const X_SCALE_HINT: &str = "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().";

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

/// Checks `values`, shown as `name` in messages (`"y"`, `"series[1].heights"`), against the
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
/// transform `y_transform`, before it is stored, so a failed call changes nothing. An infinite
/// value gets `gap_hint`.
pub(super) fn check_series(
    context: &'static str,
    x_kind: ScaleKind,
    y_transform: Transform,
    x: &AxisData,
    y: &[f64],
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
    match (x, x_kind.transform()) {
        (AxisData::Categories(_), None) => {}
        (AxisData::Numbers(values), Some(x_transform)) => {
            check_numbers(context, ("x", "x"), values, x_transform, gap_hint)?;
        }
        (AxisData::Numbers(_), None) => {
            return Err(Error::data(
                context,
                "x holds numbers but the x axis is a category axis; pass category names.",
            )
            .with_hint(X_SCALE_HINT));
        }
        (AxisData::Categories(_), Some(_)) => {
            return Err(Error::data(
                context,
                "x holds category names but the x axis is not a category axis; pass numbers.",
            )
            .with_hint("pass x=Scale.category() to Figure.add_axes()."));
        }
    }
    check_numbers(context, ("y", "y"), y, y_transform, gap_hint)
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
/// 2. every series has one height per category, checked in series order;
/// 3. the categories, by [`check_categories`];
/// 4. the heights of each series, in series order, as for a bar mark;
/// 5. for stacked series only: in every category, in category order, the positive heights and
///    the negative heights each add up to at most [`MAX_MAGNITUDE`] in magnitude, summed in
///    series order as the stacks are.
pub(super) fn check_bar_group(
    context: &'static str,
    arrangement: Arrangement,
    x_kind: ScaleKind,
    y_transform: Transform,
    categories: &[String],
    series: &[BarSeries],
) -> Result<()> {
    if series.is_empty() {
        return Err(
            Error::data(context, "series is empty; pass at least one series.")
                .with_parameter("series"),
        );
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
            (&format!("series[{i}].heights"), "series"),
            &s.heights,
            y_transform,
            "use NaN to omit that height.",
        )?;
    }
    if arrangement == Arrangement::Stacked {
        check_stacked_totals(context, categories, series)?;
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
                        "the heights of categories[{i}] ({name:?}) stack to {total}; each stack must stay within {MAX_MAGNITUDE:e} on both sides of 0.", total = number(total),
                    ),
                )
                .with_parameter("series"));
            }
        }
    }
    Ok(())
}

/// Checks explicit limits for an x axis of `kind` and returns the scale they define, with the
/// limits unchanged as its domain. The y axis is always linear or log, so its limits need no
/// such check.
pub(super) fn check_x_limits(kind: ScaleKind, low: f64, high: f64) -> Result<ContinuousScale> {
    const CONTEXT: &str = "Axes.xlim()";
    let Some(transform) = kind.transform() else {
        return Err(Error::usage(
            CONTEXT,
            "this is a category axis; limits apply to linear and log axes only.",
        )
        .with_hint(X_SCALE_HINT));
    };
    ContinuousScale::new(transform, low, high, CONTEXT)
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

/// The transform of an x axis of `kind` for the tick setter `context`, or `Error::Usage` on a
/// category axis, whose ticks are its categories. `what` names what the setter sets, as
/// `"ticks"` or `"tick formats"`.
pub(super) fn check_x_tick_axis(
    context: &'static str,
    kind: ScaleKind,
    what: &str,
) -> Result<Transform> {
    kind.transform().ok_or_else(|| {
        Error::usage(
            context,
            format!("this is a category axis; {what} apply to linear and log axes only."),
        )
        .with_hint(X_SCALE_HINT)
    })
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
