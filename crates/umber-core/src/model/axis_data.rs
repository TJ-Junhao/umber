//! The values of a series: [`AxisData`] as given by the caller, and the validated, shared
//! series that a mark stores.

use std::fmt;
use std::sync::Arc;

use super::validation::check_series;
use crate::{Instant, Result, ScaleKind};

/// The values of a mark along one axis: numbers for a linear or log axis, names for a category
/// axis, times for a time axis.
///
/// # Examples
///
/// ```
/// use umber_core::figure::AxisData;
/// use umber_core::Instant;
///
/// let names = AxisData::Categories(vec!["a".into(), "b".into()]);
/// assert_ne!(names, AxisData::Numbers(vec![0.0, 1.0]));
/// let times = AxisData::Times(vec![Some(Instant::from_micros(0)?), None]);
/// assert_ne!(times, names);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum AxisData {
    /// Values on a linear or log axis. NaN marks a gap.
    Numbers(Vec<f64>),
    /// Category names on a category axis.
    Categories(Vec<String>),
    /// Times on a time axis. `None` marks a gap, as NaN does among numbers.
    Times(Vec<Option<Instant>>),
}

impl AxisData {
    /// The number of values.
    pub(crate) fn len(&self) -> usize {
        match self {
            AxisData::Numbers(v) => v.len(),
            AxisData::Categories(v) => v.len(),
            AxisData::Times(v) => v.len(),
        }
    }

    /// The numbers; none when these are category names or times.
    pub(crate) fn numbers(&self) -> &[f64] {
        match self {
            AxisData::Numbers(values) => values,
            AxisData::Categories(_) | AxisData::Times(_) => &[],
        }
    }

    /// The category names; none when these are numbers or times.
    pub(crate) fn categories(&self) -> &[String] {
        match self {
            AxisData::Categories(names) => names,
            AxisData::Numbers(_) | AxisData::Times(_) => &[],
        }
    }

    /// The times, `None` for a gap; none when these are numbers or category names.
    pub(crate) fn times(&self) -> &[Option<Instant>] {
        match self {
            AxisData::Times(times) => times,
            AxisData::Numbers(_) | AxisData::Categories(_) => &[],
        }
    }
}

/// The x values of a mark that stores them shared, as bands and stacked areas do: numbers on a
/// linear or log axis, times on a time axis. Clones share the allocation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SharedX {
    /// Numbers; NaN marks a gap.
    Numbers(Arc<[f64]>),
    /// Times; `None` marks a gap.
    Times(Arc<[Option<Instant>]>),
}

impl SharedX {
    /// The number of values.
    pub(crate) fn len(&self) -> usize {
        match self {
            SharedX::Numbers(values) => values.len(),
            SharedX::Times(times) => times.len(),
        }
    }

    /// Whether the value at `i` is a gap: NaN or a missing time.
    pub(crate) fn is_gap(&self, i: usize) -> bool {
        match self {
            SharedX::Numbers(values) => values.get(i).is_none_or(|v| v.is_nan()),
            SharedX::Times(times) => times.get(i).is_none_or(Option::is_none),
        }
    }

    /// The numbers; none when these are times.
    pub(crate) fn numbers(&self) -> &[f64] {
        match self {
            SharedX::Numbers(values) => values,
            SharedX::Times(_) => &[],
        }
    }

    /// The times, `None` for a gap; none when these are numbers.
    pub(crate) fn times(&self) -> &[Option<Instant>] {
        match self {
            SharedX::Times(times) => times,
            SharedX::Numbers(_) => &[],
        }
    }
}

/// One validated series: x and y of equal length, each compatible with its axis. The pair is
/// stored together and never changes, so clones share both arrays and an x is never paired
/// with the y of another series.
#[derive(Clone, PartialEq)]
pub(super) struct XySeries {
    x: Arc<AxisData>,
    y: Arc<AxisData>,
}

impl XySeries {
    /// Validates `x` and `y` for axes with an x axis of scale kind `x_kind` and a y axis of
    /// scale kind `y_kind`, then stores both unchanged: every bit, NaN gaps included, in the
    /// given order. Both are moved into the series, without a copy. The checks run in a fixed
    /// order (lengths, then x against its axis, then y against its axis) and the first failure
    /// is the error, under `context`. An infinite value gets `gap_hint`, which says what to pass
    /// instead.
    pub(super) fn new(
        context: &'static str,
        (x_kind, y_kind): (ScaleKind, ScaleKind),
        x: AxisData,
        y: AxisData,
        gap_hint: &'static str,
    ) -> Result<XySeries> {
        check_series(context, (x_kind, y_kind), (&x, &y), gap_hint)?;
        Ok(XySeries {
            x: Arc::new(x),
            y: Arc::new(y),
        })
    }

    /// The x values.
    pub(super) fn x(&self) -> &AxisData {
        &self.x
    }

    /// The y values, as many as the x values.
    pub(super) fn y(&self) -> &AxisData {
        &self.y
    }
}

impl fmt::Debug for XySeries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XySeries")
            .field("x", &self.x)
            .field("y", &self.y)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::super::validation::LINE_GAP_HINT;
    use super::*;
    use crate::model::{
        AreaOptions, BarOptions, Figure, HistOptions, LineOptions, Mark, ScatterOptions,
    };
    use crate::{Error, LogBase, Theme};

    const LIN: ScaleKind = ScaleKind::Linear;
    const LINEAR: (ScaleKind, ScaleKind) = (LIN, LIN);

    fn bits(values: &[f64]) -> Vec<u64> {
        values.iter().map(|v| v.to_bits()).collect()
    }

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    fn series_of(mark: &Mark) -> &XySeries {
        match mark {
            Mark::Line(line) => line.series(),
            Mark::Scatter(scatter) => scatter.series(),
            Mark::Area(area) => area.series(),
            Mark::Step(step) => step.series(),
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
        let series = XySeries::new("t", LINEAR, nums(&x), nums(&y), LINE_GAP_HINT).unwrap();
        assert_eq!(bits(series.x().numbers()), bits(&x));
        assert_eq!(bits(series.y().numbers()), bits(&y));

        let names: Vec<String> = ["b", "a", "b"].iter().map(|s| (*s).to_owned()).collect();
        let cats = AxisData::Categories(names.clone());
        let series = XySeries::new(
            "t",
            (ScaleKind::Category, LIN),
            cats.clone(),
            nums(&[1.0, 2.0, 3.0]),
            LINE_GAP_HINT,
        )
        .unwrap();
        assert_eq!(series.x(), &cats);
        // Names along y need a category y axis, as names along x need a category x axis.
        let series = XySeries::new(
            "t",
            (LIN, ScaleKind::Category),
            nums(&[1.0, 2.0, 3.0]),
            cats.clone(),
            LINE_GAP_HINT,
        )
        .unwrap();
        assert_eq!(series.y(), &cats);
    }

    #[test]
    fn series_checks_run_in_order() {
        let log = ScaleKind::Log { base: LogBase::TEN };
        let message = |r: Result<XySeries>| match r {
            Err(Error::Data {
                context, message, ..
            }) => {
                assert_eq!(context, "t");
                message
            }
            other => panic!("expected Data error, got {other:?}"),
        };
        // Lengths before values, x before y, the first bad index within an axis.
        assert!(
            message(XySeries::new(
                "t",
                LINEAR,
                nums(&[f64::INFINITY]),
                nums(&[]),
                LINE_GAP_HINT
            ))
            .starts_with("x has 1 values but y has 0")
        );
        assert!(
            message(XySeries::new(
                "t",
                (log, log),
                nums(&[1.0, -1.0]),
                nums(&[0.0, 1.0]),
                LINE_GAP_HINT
            ))
            .starts_with("x[1] is -1 on a log axis")
        );
        assert!(
            message(XySeries::new(
                "t",
                LINEAR,
                nums(&[0.0, 1.0]),
                nums(&[1e301, f64::INFINITY]),
                LINE_GAP_HINT
            ))
            .starts_with("y[0] is 1e301")
        );
        // y of the wrong kind for its axis, as for x.
        let names = AxisData::Categories(vec!["a".into()]);
        let wrong = XySeries::new("t", LINEAR, nums(&[0.0]), names, LINE_GAP_HINT).unwrap_err();
        assert_eq!(
            wrong.to_string(),
            "t: y holds category names but the y axis is not a category axis; pass numbers."
        );
        assert_eq!(
            wrong.hint(),
            Some("pass y=Scale.category() to Figure.add_axes().")
        );
        let category_y = (LIN, ScaleKind::Category);
        let wrong =
            XySeries::new("t", category_y, nums(&[0.0]), nums(&[1.0]), LINE_GAP_HINT).unwrap_err();
        assert_eq!(
            wrong.to_string(),
            "t: y holds numbers but the y axis is a category axis; pass category names."
        );
        assert_eq!(
            wrong.hint(),
            Some("pass y=Scale.linear() or y=Scale.log() to Figure.add_axes().")
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
        let original = series_of(&source.single_axes().marks()[0]);
        let cloned = series_of(&copy.single_axes().marks()[0]);
        assert!(Arc::ptr_eq(&original.x, &cloned.x));
        assert!(Arc::ptr_eq(&original.y, &cloned.y));

        // Changing the clone leaves the source as it was.
        copy.single_axes_mut()
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
        let axes = source.single_axes();
        assert_eq!(axes.marks().len(), 1);
        assert_eq!(axes.x_limits(), Some((0.0, 4.0)));
        assert_eq!(axes.y_limits(), None);
        assert_eq!(axes.title_text(), "");
        assert_eq!(copy.single_axes().marks().len(), 2);
    }

    #[test]
    fn areas_store_their_series_as_lines_do() {
        // Every bit, NaN payloads and signed zeros included, under the area's own context.
        let gap = f64::from_bits(0xfff0_0000_dead_beef);
        let x = vec![0.0, gap, -0.0];
        let y = [gap, 1.0, -0.0];
        let mut figure = Figure::new(6.0, 4.0, Theme::midnight()).unwrap();
        let mut axes = figure.add_axes(LIN, LIN).unwrap();
        axes.area(AxisData::Numbers(x.clone()), &y, AreaOptions::default())
            .unwrap();
        let [mark @ Mark::Area(area)] = axes.marks() else {
            panic!("expected one area, got {:?}", axes.marks())
        };
        let series = series_of(mark);
        assert_eq!(bits(series.x().numbers()), bits(&x));
        assert_eq!(bits(series.y().numbers()), bits(&y));
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

    #[test]
    fn shared_values_are_stored_without_a_copy() {
        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = figure.add_axes(LIN, LIN).unwrap();
        let x = || AxisData::Numbers(vec![0.0, 1.0]);
        let values = [vec![2.0, f64::NAN], vec![3.0, 4.0], vec![5.0, 6.0]];
        let pointers: Vec<*const f64> = values.iter().map(|y| y.as_ptr()).collect();
        let [line, scatter, area] = values.map(AxisData::Numbers);
        axes.line_shared(x(), line, LineOptions::default())
            .unwrap()
            .scatter_shared(x(), scatter, ScatterOptions::default())
            .unwrap()
            .area_shared(x(), area, AreaOptions::default())
            .unwrap();
        // Each mark holds the caller's allocation, not a copy of it.
        for (mark, pointer) in axes.marks().iter().zip(pointers) {
            assert_eq!(series_of(mark).y().numbers().as_ptr(), pointer, "{mark:?}");
        }
        // A failed call adds no mark.
        let err = axes
            .line_shared(
                AxisData::Numbers(vec![0.0]),
                AxisData::Numbers(vec![1.0, 2.0]),
                LineOptions::default(),
            )
            .unwrap_err();
        assert!(err.to_string().starts_with("Axes.line(): x has 1 values"));
        assert_eq!(axes.marks().len(), 3);

        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = figure.add_axes(ScaleKind::Category, LIN).unwrap();
        let heights: Arc<[f64]> = Arc::from([3.0, -1.0]);
        axes.bar_shared(
            vec!["a".into(), "b".into()],
            Arc::clone(&heights),
            BarOptions::default(),
        )
        .unwrap();
        let [Mark::Bar(bar)] = axes.marks() else {
            panic!("expected one bar mark, got {:?}", axes.marks())
        };
        assert!(std::ptr::eq(bar.heights(), &*heights));
        assert_eq!(Arc::strong_count(&heights), 2);
        assert!(
            axes.bar_shared(
                vec!["a".into()],
                Arc::clone(&heights),
                BarOptions::default()
            )
            .is_err()
        );
        assert_eq!((Arc::strong_count(&heights), axes.marks().len()), (2, 1));

        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = figure.add_axes(LIN, LIN).unwrap();
        let values: Arc<[f64]> = Arc::from([1.0, 2.5, f64::NAN]);
        axes.hist_shared(Arc::clone(&values), HistOptions::default())
            .unwrap();
        let [Mark::Hist(hist)] = axes.marks() else {
            panic!("expected one histogram, got {:?}", axes.marks())
        };
        assert!(std::ptr::eq(hist.values(), &*values));
        assert_eq!(Arc::strong_count(&values), 2);
        let bad = HistOptions {
            bins: 0,
            ..HistOptions::default()
        };
        assert!(axes.hist_shared(Arc::clone(&values), bad).is_err());
        assert_eq!((Arc::strong_count(&values), axes.marks().len()), (2, 1));
    }
}
