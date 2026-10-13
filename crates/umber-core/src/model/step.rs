//! Step marks: the options given to the step method and the stored step mark.

use std::fmt;

use super::axis_data::{AxisData, XySeries};
use crate::{ColorSpec, Dash, SizeSpec};

/// Where a step line changes level between two neighbouring points.
///
/// # Examples
///
/// ```
/// use umber_core::Step;
///
/// assert_eq!(Step::default(), Step::Post);
/// assert_ne!(Step::Pre, Step::Mid);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Step {
    /// The line holds `y[i]` from `x[i]` up to `x[i + 1]` and changes level at `x[i + 1]`.
    #[default]
    Post,
    /// The line holds `y[i]` from `x[i - 1]` up to `x[i]` and changes level at `x[i - 1]`.
    Pre,
    /// The line changes level midway between `x[i]` and `x[i + 1]`.
    Mid,
}

/// Options of a step mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::StepOptions;
/// use umber_core::{ColorSpec, Dash, SizeSpec, Step};
///
/// let options = StepOptions::default();
/// assert_eq!((options.position, options.color), (Step::Post, ColorSpec::AUTO));
/// assert_eq!((options.dash, options.width), (Dash::Solid, SizeSpec::Auto));
/// assert!(options.label.is_empty());
///
/// let mid = StepOptions { position: Step::Mid, ..StepOptions::default() };
/// assert_ne!(mid, options);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StepOptions {
    /// Where the line changes level.
    pub position: Step,
    /// The legend label; empty for none.
    pub label: String,
    /// The line color.
    pub color: ColorSpec,
    /// The dash style.
    pub dash: Dash,
    /// The line width; [`SizeSpec::Auto`] for the theme's line width. Dashes scale with it.
    pub width: SizeSpec,
}

impl Default for StepOptions {
    fn default() -> Self {
        StepOptions {
            position: Step::Post,
            label: String::new(),
            color: ColorSpec::AUTO,
            dash: Dash::Solid,
            width: SizeSpec::Auto,
        }
    }
}

/// A step mark: a validated series, drawn as horizontal and vertical segments, and its
/// [`StepOptions`].
///
/// The x and y values are stored together and never change; clones share them. On a numeric x
/// axis the x values are in non-decreasing order.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, Mark, StepOptions};
/// use umber_core::{Figure, ScaleKind, Step, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.step(AxisData::Numbers(vec![0.0, 1.0, 3.0]), &[1.0, 2.0, 0.0], StepOptions::default())?;
/// let Some(Mark::Step(step)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(step.x(), &AxisData::Numbers(vec![0.0, 1.0, 3.0]));
/// assert_eq!(step.y(), &[1.0, 2.0, 0.0]);
/// assert_eq!(step.options().position, Step::Post);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A step mark cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{Mark, StepMark, StepOptions};
///
/// let mark = Mark::Step(StepMark {
///     series: unreachable!(),
///     options: StepOptions::default(),
/// });
/// ```
#[derive(Clone, PartialEq)]
pub struct StepMark {
    series: XySeries,
    options: StepOptions,
}

impl StepMark {
    /// A step mark over a validated series.
    pub(super) fn new(series: XySeries, options: StepOptions) -> StepMark {
        StepMark { series, options }
    }

    /// The x values.
    #[must_use]
    pub fn x(&self) -> &AxisData {
        self.series.x()
    }

    /// The y values, as many as the x values.
    #[must_use]
    pub fn y(&self) -> &[f64] {
        self.series.y()
    }

    /// Position, label, color, dash and width.
    #[must_use]
    pub fn options(&self) -> &StepOptions {
        &self.options
    }

    /// The validated series.
    #[cfg(test)]
    pub(super) fn series(&self) -> &XySeries {
        &self.series
    }
}

impl fmt::Debug for StepMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StepMark")
            .field("x", self.x())
            .field("y", &self.y())
            .field("options", &self.options)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::{Figure, Mark};
    use crate::{Error, LogBase, ScaleKind, Theme};

    const LIN: ScaleKind = ScaleKind::Linear;

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    fn figure(x: ScaleKind) -> Figure {
        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        figure.add_axes(x, LIN).unwrap();
        figure
    }

    /// The Data error of `step` called with `x` and `y` on axes with the x scale `kind`: its
    /// text, parameter and hint. The figure is unchanged afterwards.
    #[track_caller]
    fn refused(
        kind: ScaleKind,
        x: AxisData,
        y: &[f64],
    ) -> (String, Option<&'static str>, Option<&'static str>) {
        let mut f = figure(kind);
        let first = match kind {
            ScaleKind::Category => AxisData::Categories(vec!["z".into()]),
            _ => nums(&[1.0]),
        };
        f.single_axes_mut()
            .step(first, &[1.0], StepOptions::default())
            .unwrap();
        let before = f.clone();
        let err = f
            .single_axes_mut()
            .step(x, y, StepOptions::default())
            .unwrap_err();
        assert!(matches!(err, Error::Data { .. }), "{err:?}");
        assert_eq!(f, before);
        (err.to_string(), err.parameter(), err.hint())
    }

    #[test]
    fn step_unsorted_x_names_index() {
        let order = "step needs x in non-decreasing order.";
        let hint = Some("sort the points by x first.");
        let (text, parameter, found) = refused(LIN, nums(&[0.0, 3.0, 2.0, 5.0, 1.0]), &[0.0; 5]);
        assert_eq!(
            text,
            format!("Axes.step(): x[2] is 2, below x[1] = 3; {order}")
        );
        assert_eq!((parameter, found), (Some("x"), hint));
        // A gap in x is skipped: the value is compared with the last one before the gap.
        let (text, ..) = refused(LIN, nums(&[0.0, f64::NAN, -1.5]), &[0.0; 3]);
        assert_eq!(
            text,
            format!("Axes.step(): x[2] is -1.5, below x[0] = 0; {order}")
        );
        // Large magnitudes are written as everywhere else.
        let (text, ..) = refused(LIN, nums(&[1e7, 1e6]), &[0.0; 2]);
        assert_eq!(
            text,
            format!("Axes.step(): x[1] is 1e6, below x[0] = 1e7; {order}")
        );
        // On a log axis too.
        let log = ScaleKind::Log { base: LogBase::TEN };
        let (text, ..) = refused(log, nums(&[10.0, 1.0]), &[0.0; 2]);
        assert_eq!(
            text,
            format!("Axes.step(): x[1] is 1, below x[0] = 10; {order}")
        );
    }

    #[test]
    fn step_accepts_equal_and_gapped_x_and_any_category_order() {
        let mut f = figure(LIN);
        f.single_axes_mut()
            .step(
                nums(&[0.0, 1.0, 1.0, f64::NAN, 1.0, 2.0]),
                &[0.0; 6],
                StepOptions::default(),
            )
            .unwrap()
            .step(
                nums(&[f64::NAN, f64::NAN]),
                &[0.0; 2],
                StepOptions::default(),
            )
            .unwrap()
            .step(nums(&[]), &[], StepOptions::default())
            .unwrap();
        assert_eq!(f.single_axes().marks().len(), 3);
        let mut f = figure(ScaleKind::Category);
        let names = AxisData::Categories(vec!["b".into(), "a".into()]);
        f.single_axes_mut()
            .step(names, &[1.0, 2.0], StepOptions::default())
            .unwrap();
        assert_eq!(f.single_axes().marks().len(), 1);
    }

    #[test]
    fn step_checks_as_a_line_does_and_names_its_own_context() {
        let gap = Some("use NaN to leave a gap in the line.");
        let (text, parameter, hint) = refused(LIN, nums(&[0.0, 1.0]), &[1.0]);
        assert_eq!(
            text,
            "Axes.step(): x has 2 values but y has 1; x and y must have the same length."
        );
        assert_eq!((parameter, hint), (None, None));
        let (text, parameter, hint) = refused(LIN, nums(&[0.0, 1.0]), &[1.0, f64::INFINITY]);
        assert_eq!(text, "Axes.step(): y[1] is inf; values must be finite.");
        assert_eq!((parameter, hint), (Some("y"), gap));
        let (text, parameter, hint) = refused(LIN, nums(&[0.0, f64::NEG_INFINITY]), &[1.0, 1.0]);
        assert_eq!(text, "Axes.step(): x[1] is -inf; values must be finite.");
        assert_eq!((parameter, hint), (Some("x"), gap));
        let names = AxisData::Categories(vec!["a".into()]);
        let (text, ..) = refused(LIN, names, &[1.0]);
        assert!(
            text.starts_with("Axes.step(): x holds category names"),
            "{text}"
        );
        let (text, ..) = refused(ScaleKind::Category, nums(&[1.0]), &[1.0]);
        assert!(text.starts_with("Axes.step(): x holds numbers"), "{text}");
        // Lengths are checked before the order of x.
        let (text, ..) = refused(LIN, nums(&[2.0, 1.0]), &[1.0]);
        assert!(text.starts_with("Axes.step(): x has 2 values"), "{text}");
        // A bad y is reported before an unsorted x.
        let (text, ..) = refused(LIN, nums(&[2.0, 1.0]), &[f64::INFINITY, 1.0]);
        assert!(text.starts_with("Axes.step(): y[0] is inf"), "{text}");
    }

    #[test]
    fn step_keeps_its_input_and_options_and_shares_y() {
        let gap = f64::from_bits(0xfff8_0000_0000_0001);
        let x = vec![-0.0, 0.0, gap, 2.0];
        let y: Arc<[f64]> = Arc::from([gap, 1.0, -0.0, 4.0]);
        let options = StepOptions {
            position: Step::Pre,
            label: "levels".into(),
            ..StepOptions::default()
        };
        let mut f = figure(LIN);
        f.single_axes_mut()
            .step_shared(
                AxisData::Numbers(x.clone()),
                Arc::clone(&y),
                options.clone(),
            )
            .unwrap();
        let [Mark::Step(step)] = f.single_axes().marks() else {
            panic!("one step expected")
        };
        let bits = |v: &[f64]| v.iter().map(|b| b.to_bits()).collect::<Vec<_>>();
        let AxisData::Numbers(stored) = step.x() else {
            panic!("numbers expected")
        };
        assert_eq!(bits(stored), bits(&x));
        assert_eq!(bits(step.y()), bits(&y));
        assert!(std::ptr::eq(step.y(), &*y));
        assert_eq!(step.options(), &options);
        // Clones read the same arrays.
        let copy = f.clone();
        let [Mark::Step(cloned)] = copy.single_axes().marks() else {
            panic!("one step expected")
        };
        assert!(std::ptr::eq(step.y(), cloned.y()));
    }
}
