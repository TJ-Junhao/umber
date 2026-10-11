//! Input validation of the model: series, bar and histogram data against their axes' scale
//! kinds, stacked bar totals, histogram bin counts, explicit limits and figure sizes. Every check
//! runs before anything is stored, so a failed call changes nothing.

use std::collections::HashMap;

use super::axis_data::AxisData;
use super::bar::{Arrangement, BarSeries};
use crate::binning::{BinCount, MAX_BINS};
use crate::scale::{MAX_MAGNITUDE, MIN_LOG_VALUE};
use crate::stacking::Totals;
use crate::{ContinuousScale, Error, Result, ScaleKind};

/// The hint for data that meets a category y axis.
const Y_SCALE_HINT: &str = "create the axes with a linear or log y scale.";

/// The hint for an infinite line value.
pub(super) const LINE_GAP_HINT: &str = "use NaN to leave a gap in the line.";

/// The hint for an infinite scatter value.
pub(super) const SCATTER_GAP_HINT: &str = "use NaN to leave a point out.";

/// The hint for an infinite bar height.
const BAR_GAP_HINT: &str = "use NaN to leave a category without a bar.";

/// The largest accepted figure side, in inches.
const MAX_FIGURE_IN: f64 = 200.0;

/// Checks `values`, shown as `name` in messages (`"y"`, `"series[1].heights"`), against the scale
/// kind of their axis. An error names `parameter`, the Python parameter that holds them, and an
/// infinite value gets `infinite_hint`, which says what to pass instead.
fn check_numbers(
    context: &'static str,
    (name, parameter): (&str, &'static str),
    values: &[f64],
    kind: ScaleKind,
    infinite_hint: &'static str,
) -> Result<()> {
    let log = matches!(kind, ScaleKind::Log { .. });
    for (i, &v) in values.iter().enumerate() {
        if v.is_nan() {
            continue;
        }
        if v.is_infinite() {
            return Err(Error::data(
                context,
                format!("{name}[{i}] is {v}; values must be finite."),
            )
            .with_parameter(parameter)
            .with_hint(infinite_hint));
        }
        if v.abs() > MAX_MAGNITUDE {
            return Err(Error::data(
                context,
                format!("{name}[{i}] is {v:e}; magnitudes must not exceed {MAX_MAGNITUDE:e}."),
            )
            .with_parameter(parameter));
        }
        if log {
            if v <= 0.0 {
                return Err(Error::data(
                    context,
                    format!("{name}[{i}] is {v} on a log axis; values must be positive."),
                )
                .with_parameter(parameter)
                .with_hint("use a linear axis, or remove values that are not positive."));
            }
            if v < MIN_LOG_VALUE {
                return Err(Error::data(
                    context,
                    format!(
                        "{name}[{i}] is {v:e} on a log axis; values must be at least {MIN_LOG_VALUE:e}."
                    ),
                )
                .with_parameter(parameter)
                .with_hint("use a linear axis, or rescale the values."));
            }
        }
    }
    Ok(())
}

/// Validates a whole series for axes with scale kinds `x_kind` and `y_kind`, before it is
/// stored, so a failed call changes nothing. An infinite value gets `gap_hint`.
pub(super) fn check_series(
    context: &'static str,
    x_kind: ScaleKind,
    y_kind: ScaleKind,
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
    match (x, x_kind) {
        (AxisData::Categories(_), ScaleKind::Category) => {}
        (AxisData::Numbers(values), kind) if kind != ScaleKind::Category => {
            check_numbers(context, ("x", "x"), values, kind, gap_hint)?;
        }
        (AxisData::Numbers(_), _) => {
            return Err(Error::data(
                context,
                "x holds numbers but the x axis is a category axis; pass category names.",
            )
            .with_hint("to plot numbers, use a linear or log x axis."));
        }
        (AxisData::Categories(_), _) => {
            return Err(Error::data(
                context,
                "x holds category names but the x axis is not a category axis; pass numbers.",
            )
            .with_hint("to plot category names, use a category x axis."));
        }
    }
    if y_kind == ScaleKind::Category {
        return Err(Error::data(
            context,
            "y holds numbers but the y axis is a category axis; use a linear or log y axis.",
        )
        .with_hint(Y_SCALE_HINT));
    }
    check_numbers(context, ("y", "y"), y, y_kind, gap_hint)
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
    check_numbers(
        context,
        ("heights", "heights"),
        heights,
        y_kind,
        BAR_GAP_HINT,
    )?;
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
    check_categories(context, x_kind, categories)?;
    for (i, s) in series.iter().enumerate() {
        check_numbers(
            context,
            (&format!("series[{i}].heights"), "series"),
            &s.heights,
            y_kind,
            BAR_GAP_HINT,
        )?;
    }
    if arrangement == Arrangement::Stacked {
        check_stacked_totals(context, categories, series)?;
    }
    check_bar_y_axis(context, y_kind, "the series hold numbers")
}

/// Validates the values and bin count of a histogram for axes with scale kinds `x_kind` and
/// `y_kind`, before it is stored, so a failed call changes nothing, and returns the bin count.
/// The checks run in a fixed order and the first failure is the error, under `context`:
///
/// 1. `bins` is within `1..=MAX_BINS` (`Error::Style`);
/// 2. the x axis is a linear axis: a category x axis is `Error::Data`, and a log x axis, which
///    histograms do not support, is `Error::Usage`;
/// 3. every value is finite (NaN is allowed and ignored) and within the supported magnitudes
///    (`Error::Data`);
/// 4. the y axis is a linear or log axis (`Error::Data`).
pub(super) fn check_hist(
    context: &'static str,
    x_kind: ScaleKind,
    y_kind: ScaleKind,
    values: &[f64],
    bins: usize,
) -> Result<BinCount> {
    let Some(count) = BinCount::new(bins) else {
        return Err(Error::style(
            context,
            format!("bins is {bins}; bins must be between 1 and {MAX_BINS}."),
        )
        .with_parameter("bins"));
    };
    match x_kind {
        ScaleKind::Linear => {}
        ScaleKind::Category => {
            return Err(Error::data(
                context,
                "values holds numbers but the x axis is a category axis; use a linear x axis.",
            )
            .with_hint("create the axes with a linear x scale."));
        }
        ScaleKind::Log { .. } => {
            return Err(Error::usage(
                context,
                "the x axis is a log axis, which histograms do not support; use a linear x axis.",
            )
            .with_hint("create the axes with a linear x scale; histograms do not support log x."));
        }
    }
    check_numbers(
        context,
        ("values", "values"),
        values,
        x_kind,
        "a histogram ignores NaN, so use it to leave a value out.",
    )?;
    check_bar_y_axis(context, y_kind, "a histogram counts values")?;
    Ok(count)
}

/// Checks the categories of a bar mark: the x axis is a category axis, and no category appears
/// twice.
fn check_categories(context: &'static str, x_kind: ScaleKind, categories: &[String]) -> Result<()> {
    if x_kind != ScaleKind::Category {
        return Err(Error::data(
            context,
            "categories holds names but the x axis is not a category axis; use a category x axis.",
        )
        .with_hint("create the axes with a category x scale."));
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
            .with_hint("add up the heights of a repeated category before passing them."));
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
                )
                .with_parameter("series"));
            }
        }
    }
    Ok(())
}

/// Checks that bar heights or histogram counts, described by `subject`, can be placed on the y
/// axis: it must be a linear or log axis.
fn check_bar_y_axis(context: &'static str, y_kind: ScaleKind, subject: &str) -> Result<()> {
    if y_kind == ScaleKind::Category {
        return Err(Error::data(
            context,
            format!("{subject} but the y axis is a category axis; use a linear or log y axis."),
        )
        .with_hint(Y_SCALE_HINT));
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
            )
            .with_parameter(name));
        }
    }
    Ok(())
}
