//! Stacked area marks: non-negative series over shared, sorted x values.

use std::sync::Arc;

use super::bar::{Series, SeriesView};
use super::validation::check_stacked_area;
use crate::{Result, ScaleKind, Transform};

/// Stacked areas over shared x values, with the layers in series order.
///
/// The x values and series are validated together, never change and are shared by clones.
/// A NaN in x or any series breaks every layer at that index.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Mark, ScaleKind, Series, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.stacked_area(&[0.0, 1.0], vec![Series {
///     label: "Wind".into(), values: vec![2.0, 3.0], ..Series::default()
/// }])?;
/// let Some(Mark::StackedArea(mark)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(mark.x(), &[0.0, 1.0]);
/// assert_eq!(mark.series().next().map(|s| s.label()), Some("Wind"));
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A stacked area cannot be built outside the validated axes methods:
///
/// ```compile_fail,E0451
/// use umber_core::StackedAreaMark;
/// let mark = StackedAreaMark { x: unreachable!(), series: unreachable!() };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StackedAreaMark {
    x: Arc<[f64]>,
    series: Arc<[Series]>,
}

impl StackedAreaMark {
    /// Validates and stores the shared x values and the series without changing their values.
    pub(super) fn new(
        context: &'static str,
        x_kind: ScaleKind,
        y: Transform,
        x: Arc<[f64]>,
        series: Vec<Series>,
    ) -> Result<StackedAreaMark> {
        check_stacked_area(context, x_kind, y, &x, &series)?;
        Ok(StackedAreaMark {
            x,
            series: Arc::from(series),
        })
    }

    /// The x values, in non-decreasing order, NaN apart.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Mark, ScaleKind, Series, Theme};
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.stacked_area(&[0.0, 1.0], vec![Series {
    ///     values: vec![2.0, 3.0], ..Series::default()
    /// }])?;
    /// let Some(Mark::StackedArea(mark)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(mark.x(), &[0.0, 1.0]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// The non-negative series in stacking order, the first on the baseline; at least one.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Mark, ScaleKind, Series, Theme};
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.stacked_area(&[0.0], vec![Series { values: vec![2.0], ..Series::default() }])?;
    /// let Some(Mark::StackedArea(mark)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(mark.series().len(), 1);
    /// assert_eq!(mark.series().next().map(|s| s.values()), Some(&[2.0][..]));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn series(&self) -> impl ExactSizeIterator<Item = SeriesView<'_>> {
        self.series.iter().map(SeriesView::of_series)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, Figure, LogBase, Mark, Theme};

    const LIN: ScaleKind = ScaleKind::Linear;
    const LOG: ScaleKind = ScaleKind::Log { base: LogBase::TEN };

    fn series(values: &[f64]) -> Series {
        Series {
            values: values.to_vec(),
            ..Series::default()
        }
    }

    fn figure(x: ScaleKind, y: ScaleKind) -> Figure {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(x, y).unwrap();
        f
    }

    #[track_caller]
    fn refused(x_kind: ScaleKind, y_kind: ScaleKind, x: &[f64], series: Vec<Series>) -> Error {
        let mut f = figure(x_kind, y_kind);
        if x_kind != ScaleKind::Category {
            f.single_axes_mut()
                .stacked_area(&[1.0, 2.0], vec![self::series(&[2.0, 3.0])])
                .unwrap();
        }
        let before = f.clone();
        let error = f.single_axes_mut().stacked_area(x, series).unwrap_err();
        assert_eq!(f, before);
        error
    }

    #[track_caller]
    fn data(error: Error, message: &str, parameter: &'static str, hint: Option<&'static str>) {
        assert!(matches!(error, Error::Data { .. }));
        assert_eq!(error.to_string(), format!("Axes.stacked_area(): {message}"));
        assert_eq!((error.parameter(), error.hint()), (Some(parameter), hint));
    }

    #[test]
    fn negative_value_names_series_and_index() {
        for y in [LIN, LOG] {
            data(
                refused(
                    LIN,
                    y,
                    &[0.0, 1.0, 2.0],
                    vec![series(&[1.0; 3]), series(&[1.0, -2.0, -3.0])],
                ),
                "series[1].values[1] is -2; stacked areas need values of at least 0.",
                "series",
                Some("use stacked_bar for values with mixed signs."),
            );
        }
    }

    #[test]
    fn infinite_values_use_gap_hint_before_negative_checks() {
        for y in [LIN, LOG] {
            for value in [f64::INFINITY, f64::NEG_INFINITY] {
                data(
                    refused(LIN, y, &[0.0, 1.0, 2.0], vec![series(&[1.0, value, -2.0])]),
                    &format!("series[0].values[1] is {value}; values must be finite."),
                    "series",
                    Some("use NaN to leave a gap in the area."),
                );
            }
        }
    }

    #[test]
    fn lengths_and_empty_series_are_checked_before_values() {
        data(
            refused(LIN, LIN, &[], vec![]),
            "series is empty; pass at least one Series.",
            "series",
            None,
        );
        data(
            refused(
                LIN,
                LIN,
                &[f64::INFINITY, 1.0],
                vec![series(&[-1.0, 2.0]), series(&[1.0])],
            ),
            "series[1].values has 1 values; x has 2.",
            "series",
            None,
        );
    }

    #[test]
    fn category_x_is_a_usage_error_before_data_checks() {
        let e = refused(ScaleKind::Category, LIN, &[], vec![]);
        assert!(matches!(e, Error::Usage { .. }));
        assert_eq!(
            e.to_string(),
            "Axes.stacked_area(): this is a category axis; stacked areas apply to linear and log axes only."
        );
        assert_eq!(
            (e.parameter(), e.hint()),
            (
                Some("x"),
                Some("pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().")
            )
        );
    }

    #[test]
    fn sorted_x_skips_nan_and_allows_equal_values() {
        data(
            refused(LIN, LIN, &[1.0, f64::NAN, 0.0], vec![series(&[1.0; 3])]),
            "x[2] is 0, below x[0] = 1; stacked_area needs x in non-decreasing order.",
            "x",
            Some("sort the points by x first."),
        );
        figure(LIN, LIN)
            .single_axes_mut()
            .stacked_area(&[0.0, 0.0, 1.0], vec![series(&[0.0, -0.0, 1.0])])
            .unwrap();
    }

    #[test]
    fn values_obey_scale_bounds_and_totals_cannot_overflow() {
        let gap = Some("use NaN to leave a gap in the area.");
        data(
            refused(LIN, LIN, &[f64::INFINITY], vec![series(&[1.0])]),
            "x[0] is inf; values must be finite.",
            "x",
            gap,
        );
        data(
            refused(LIN, LIN, &[0.0], vec![series(&[f64::INFINITY])]),
            "series[0].values[0] is inf; values must be finite.",
            "series",
            gap,
        );
        data(
            refused(LIN, LIN, &[0.0], vec![series(&[1e301])]),
            "series[0].values[0] is 1e301; magnitudes must not exceed 1e300.",
            "series",
            None,
        );
        data(
            refused(LOG, LIN, &[0.0], vec![series(&[1.0])]),
            "x[0] is 0 on a log axis; values must be positive.",
            "x",
            Some("use a linear axis, or remove values that are not positive."),
        );
        data(
            refused(LIN, LOG, &[1.0], vec![series(&[0.0])]),
            "series[0].values[0] is 0 on a log axis; values must be positive.",
            "series",
            Some("use a linear axis, or remove values that are not positive."),
        );
        data(
            refused(LIN, LOG, &[1.0], vec![series(&[1e-301])]),
            "series[0].values[0] is 1e-301 on a log axis; values must be at least 1e-300.",
            "series",
            Some("use a linear axis, or rescale the values."),
        );
        data(
            refused(LIN, LIN, &[0.0], vec![series(&[1e300]), series(&[1e300])]),
            "the values at x[0] stack to 2e300; each stack must not exceed 1e300.",
            "series",
            None,
        );
    }

    #[test]
    fn empty_and_gapped_data_keep_their_bits_and_share_storage() {
        let gap = f64::from_bits(0xfff8_0000_0000_0001);
        let x: Arc<[f64]> = Arc::from([0.0, gap, 2.0]);
        let values = vec![-0.0, 1.0, gap];
        let original = values.as_ptr();
        let mut f = figure(LIN, LIN);
        f.single_axes_mut()
            .stacked_area(&[], vec![Series::default()])
            .unwrap()
            .stacked_area_shared(
                Arc::clone(&x),
                vec![Series {
                    values,
                    ..Series::default()
                }],
            )
            .unwrap();
        let copy = f.clone();
        let [_, Mark::StackedArea(mark)] = f.single_axes().marks() else {
            panic!()
        };
        let [_, Mark::StackedArea(cloned)] = copy.single_axes().marks() else {
            panic!()
        };
        assert!(std::ptr::eq(mark.x(), &*x));
        let view = mark.series().next().unwrap();
        assert_eq!(view.values().as_ptr(), original);
        assert_eq!(view.values()[0].to_bits(), (-0.0_f64).to_bits());
        assert_eq!(view.values()[2].to_bits(), gap.to_bits());
        assert!(std::ptr::eq(
            view.values(),
            cloned.series().next().unwrap().values()
        ));
        assert!(
            f.single_axes_mut()
                .stacked_area_shared(Arc::clone(&x), vec![Series::default()])
                .is_err()
        );
        assert_eq!(Arc::strong_count(&x), 3);
    }
}
