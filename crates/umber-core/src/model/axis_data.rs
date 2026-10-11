//! The values of a series: [`AxisData`] as given by the caller, and the validated, shared
//! series that a mark stores.

use std::fmt;
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
    pub(crate) fn len(&self) -> usize {
        match self {
            AxisData::Numbers(v) => v.len(),
            AxisData::Categories(v) => v.len(),
        }
    }
}

/// One validated series: x and y of equal length, each compatible with its axis. The pair is
/// stored together and never changes, so clones share both arrays and an x is never paired
/// with the y of another series.
#[derive(Clone, PartialEq)]
pub(super) struct Series {
    x: Arc<AxisData>,
    y: Arc<[f64]>,
}

impl Series {
    /// Validates `x` and `y` for axes with scale kinds `x_kind` and `y_kind`, then stores both
    /// unchanged: every bit, NaN gaps included, in the given order. The checks run in a fixed
    /// order (lengths, then x against its axis, then y against its axis) and the first failure
    /// is the error, under `context`. An infinite value gets `gap_hint`, which says what to pass
    /// instead.
    pub(super) fn new(
        context: &'static str,
        x_kind: ScaleKind,
        y_kind: ScaleKind,
        x: AxisData,
        y: &[f64],
        gap_hint: &'static str,
    ) -> Result<Series> {
        check_series(context, x_kind, y_kind, &x, y, gap_hint)?;
        Ok(Series {
            x: Arc::new(x),
            y: Arc::from(y),
        })
    }

    /// The x values.
    pub(super) fn x(&self) -> &AxisData {
        &self.x
    }

    /// The y values, as many as the x values.
    pub(super) fn y(&self) -> &[f64] {
        &self.y
    }
}

impl fmt::Debug for Series {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Series")
            .field("x", &self.x)
            .field("y", &self.y)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::super::validation::LINE_GAP_HINT;
    use super::*;
    use crate::model::{AreaOptions, Figure, LineOptions, Mark, ScatterOptions};
    use crate::{Error, LogBase, Theme};

    const LIN: ScaleKind = ScaleKind::Linear;

    fn bits(values: &[f64]) -> Vec<u64> {
        values.iter().map(|v| v.to_bits()).collect()
    }

    fn series_of(mark: &Mark) -> &Series {
        match mark {
            Mark::Line(line) => line.series(),
            Mark::Scatter(scatter) => scatter.series(),
            Mark::Area(area) => area.series(),
            other => panic!("{other:?} holds no series"),
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
        let series = Series::new(
            "t",
            LIN,
            LIN,
            AxisData::Numbers(x.clone()),
            &y,
            LINE_GAP_HINT,
        )
        .unwrap();
        match series.x() {
            AxisData::Numbers(stored) => assert_eq!(bits(stored), bits(&x)),
            other => panic!("unexpected x {other:?}"),
        }
        assert_eq!(bits(series.y()), bits(&y));

        let names: Vec<String> = ["b", "a", "b"].iter().map(|s| (*s).to_owned()).collect();
        let cats = AxisData::Categories(names.clone());
        let series = Series::new(
            "t",
            ScaleKind::Category,
            LIN,
            cats,
            &[1.0, 2.0, 3.0],
            LINE_GAP_HINT,
        )
        .unwrap();
        assert_eq!(series.x(), &AxisData::Categories(names));
    }

    #[test]
    fn series_checks_run_in_order() {
        let log = ScaleKind::Log { base: LogBase::TEN };
        let message = |r: Result<Series>| match r {
            Err(Error::Data {
                context, message, ..
            }) => {
                assert_eq!(context, "t");
                message
            }
            other => panic!("expected Data error, got {other:?}"),
        };
        let nums = |v: &[f64]| AxisData::Numbers(v.to_vec());
        // Lengths before values, x before y, the first bad index within an axis.
        assert!(
            message(Series::new(
                "t",
                LIN,
                LIN,
                nums(&[f64::INFINITY]),
                &[],
                LINE_GAP_HINT
            ))
            .starts_with("x has 1 values but y has 0")
        );
        assert!(
            message(Series::new(
                "t",
                log,
                log,
                nums(&[1.0, -1.0]),
                &[0.0, 1.0],
                LINE_GAP_HINT
            ))
            .starts_with("x[1] is -1 on a log axis")
        );
        assert!(
            message(Series::new(
                "t",
                LIN,
                LIN,
                nums(&[0.0, 1.0]),
                &[1e301, f64::INFINITY],
                LINE_GAP_HINT
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
        let original = series_of(&source.axes().unwrap().marks()[0]);
        let cloned = series_of(&copy.axes().unwrap().marks()[0]);
        assert!(Arc::ptr_eq(&original.x, &cloned.x));
        assert!(Arc::ptr_eq(&original.y, &cloned.y));

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

    #[test]
    fn areas_store_their_series_as_lines_do() {
        // Every bit, NaN payloads and signed zeros included, under the area's own context.
        let gap = f64::from_bits(0xfff0_0000_dead_beef);
        let x = vec![0.0, gap, -0.0];
        let y = [gap, 1.0, -0.0];
        let mut figure = Figure::new(6.0, 4.0, Theme::midnight()).unwrap();
        let axes = figure.add_axes(LIN, LIN).unwrap();
        axes.area(AxisData::Numbers(x.clone()), &y, AreaOptions::default())
            .unwrap();
        let [mark @ Mark::Area(area)] = axes.marks() else {
            panic!("expected one area, got {:?}", axes.marks())
        };
        let series = series_of(mark);
        match series.x() {
            AxisData::Numbers(stored) => assert_eq!(bits(stored), bits(&x)),
            other => panic!("unexpected x {other:?}"),
        }
        assert_eq!(bits(series.y()), bits(&y));
        assert_eq!(area.options(), &AreaOptions::default());
        // A failed call adds nothing.
        let err = axes
            .area(
                AxisData::Numbers(vec![0.0]),
                &[1.0, 2.0],
                AreaOptions::default(),
            )
            .unwrap_err();
        assert!(err.to_string().starts_with("Axes.area(): x has 1 values"));
        assert_eq!(axes.marks().len(), 1);
    }
}
