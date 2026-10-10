//! The values of a series: [`AxisData`] as given by the caller, and the validated, shared
//! [`XValues`] and [`YValues`] that a mark stores.

use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use super::validation::check_series;
use crate::{Result, ScaleKind};

/// The x values of a mark: numbers for a continuous axis, names for a category axis.
///
/// # Examples
///
/// ```
/// use umber_core::figure::AxisData;
///
/// let names = AxisData::Categories(vec!["a".into(), "b".into()]);
/// assert_ne!(names, AxisData::Numbers(vec![0.0, 1.0]));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum AxisData {
    /// Values on a linear or log axis. NaN marks a gap.
    Numbers(Vec<f64>),
    /// Category names on a category axis.
    Categories(Vec<String>),
}

impl AxisData {
    /// The number of values.
    pub(super) fn len(&self) -> usize {
        match self {
            AxisData::Numbers(v) => v.len(),
            AxisData::Categories(v) => v.len(),
        }
    }
}

/// The x values of a stored mark, validated against the axes they were added to. They read as
/// an [`AxisData`] and never change, so clones of a figure share them.
///
/// Only [`Axes::line`](super::Axes::line) and [`Axes::scatter`](super::Axes::scatter) create
/// them.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, LineOptions, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// axes.line(AxisData::Categories(vec!["a".into()]), &[1.0], LineOptions::default())?;
/// let Some(Mark::Line { x, .. }) = axes.marks().first() else { unreachable!() };
/// assert_eq!(**x, AxisData::Categories(vec!["a".into()]));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Clone, PartialEq)]
pub struct XValues(Arc<AxisData>);

impl Deref for XValues {
    type Target = AxisData;

    fn deref(&self) -> &AxisData {
        &self.0
    }
}

impl fmt::Debug for XValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The y values of a stored mark, validated against the axes they were added to. They read as
/// a slice and never change, so clones of a figure share them.
///
/// Only [`Axes::line`](super::Axes::line) and [`Axes::scatter`](super::Axes::scatter) create
/// them.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, Mark, ScatterOptions};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.scatter(AxisData::Numbers(vec![0.0, 1.0]), &[2.0, 3.0], ScatterOptions::default())?;
/// let Some(Mark::Scatter { y, .. }) = axes.marks().first() else { unreachable!() };
/// assert_eq!(&y[..], &[2.0, 3.0]);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Clone, PartialEq)]
pub struct YValues(Arc<[f64]>);

impl Deref for YValues {
    type Target = [f64];

    fn deref(&self) -> &[f64] {
        &self.0
    }
}

impl fmt::Debug for YValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// One validated series: x and y of equal length, each compatible with its axis.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Series {
    pub(super) x: XValues,
    pub(super) y: YValues,
}

impl Series {
    /// Validates `x` and `y` for axes with scale kinds `x_kind` and `y_kind`, then stores both
    /// unchanged: every bit, NaN gaps included, in the given order. The checks run in a fixed
    /// order (lengths, then x against its axis, then y against its axis) and the first failure
    /// is the error, under `context`.
    pub(super) fn new(
        context: &'static str,
        x_kind: ScaleKind,
        y_kind: ScaleKind,
        x: AxisData,
        y: &[f64],
    ) -> Result<Series> {
        check_series(context, x_kind, y_kind, &x, y)?;
        Ok(Series {
            x: XValues(Arc::new(x)),
            y: YValues(Arc::from(y)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Figure, LineOptions, Mark, ScatterOptions};
    use crate::{Error, LogBase, Theme};

    const LIN: ScaleKind = ScaleKind::Linear;

    fn bits(values: &[f64]) -> Vec<u64> {
        values.iter().map(|v| v.to_bits()).collect()
    }

    fn series_of(mark: &Mark) -> (&XValues, &YValues) {
        match mark {
            Mark::Line { x, y, .. } | Mark::Scatter { x, y, .. } => (x, y),
        }
    }

    #[test]
    fn series_keep_every_bit_in_order() {
        // NaN payloads of both signs, quiet and signalling, next to signed zeros.
        let gaps = [
            f64::from_bits(0x7ff8_0000_0000_0001),
            f64::from_bits(0xfff8_0000_0000_0000),
            f64::from_bits(0x7ff0_0000_0000_0001),
            f64::from_bits(0xfff0_0000_dead_beef),
        ];
        let x = vec![3.0, gaps[0], -0.0, gaps[1], 1.0];
        let y = [gaps[2], 0.0, 2.0, -0.0, gaps[3]];
        let series = Series::new("t", LIN, LIN, AxisData::Numbers(x.clone()), &y).unwrap();
        match &*series.x {
            AxisData::Numbers(stored) => assert_eq!(bits(stored), bits(&x)),
            other => panic!("unexpected x {other:?}"),
        }
        assert_eq!(bits(&series.y), bits(&y));

        let names: Vec<String> = ["b", "a", "b"].iter().map(|s| (*s).to_owned()).collect();
        let cats = AxisData::Categories(names.clone());
        let series = Series::new("t", ScaleKind::Category, LIN, cats, &[1.0, 2.0, 3.0]).unwrap();
        assert_eq!(*series.x, AxisData::Categories(names));
    }

    #[test]
    fn series_checks_run_in_order() {
        let log = ScaleKind::Log { base: LogBase::TEN };
        let message = |r: Result<Series>| match r {
            Err(Error::Data { context, message }) => {
                assert_eq!(context, "t");
                message
            }
            other => panic!("expected Data error, got {other:?}"),
        };
        let nums = |v: &[f64]| AxisData::Numbers(v.to_vec());
        // Lengths before values, x before y, the first bad index within an axis.
        assert!(
            message(Series::new("t", LIN, LIN, nums(&[f64::INFINITY]), &[]))
                .starts_with("x has 1 values but y has 0")
        );
        assert!(
            message(Series::new("t", log, log, nums(&[1.0, -1.0]), &[0.0, 1.0]))
                .starts_with("x[1] is -1 on a log axis")
        );
        assert!(
            message(Series::new(
                "t",
                LIN,
                LIN,
                nums(&[0.0, 1.0]),
                &[1e301, f64::INFINITY]
            ))
            .starts_with("y[0] is 1e301")
        );
    }

    #[test]
    fn clones_share_series_and_change_independently() {
        let mut source = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        source
            .add_axes(LIN, LIN)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[2.0, 3.0],
                LineOptions::default(),
            )
            .unwrap()
            .xlim(0.0, 4.0)
            .unwrap();
        let before = source.clone();
        let mut copy = source.clone();

        // The clone reads the same arrays, not copies of them.
        let (sx, sy) = series_of(&source.axes().unwrap().marks()[0]);
        let (cx, cy) = series_of(&copy.axes().unwrap().marks()[0]);
        assert!(Arc::ptr_eq(&sx.0, &cx.0));
        assert!(Arc::ptr_eq(&sy.0, &cy.0));

        // Changing the clone leaves the source as it was.
        copy.axes_mut()
            .unwrap()
            .scatter(
                AxisData::Numbers(vec![5.0]),
                &[6.0],
                ScatterOptions::default(),
            )
            .unwrap()
            .xlim(-1.0, 1.0)
            .unwrap()
            .ylim(0.0, 9.0)
            .unwrap()
            .title("copy");
        assert_eq!(source, before);
        let axes = source.axes().unwrap();
        assert_eq!(axes.marks().len(), 1);
        assert_eq!(axes.x_limits(), Some((0.0, 4.0)));
        assert_eq!(axes.y_limits(), None);
        assert_eq!(axes.title_text(), "");
        assert_eq!(copy.axes().unwrap().marks().len(), 2);
    }
}
