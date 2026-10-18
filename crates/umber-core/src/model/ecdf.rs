//! ECDF marks: the options given to the ECDF method and the stored mark, which keeps the values
//! as given and the step line that draws them.

use std::fmt;
use std::sync::Arc;

use super::axis_data::{AxisData, XySeries};
use super::step::{Step, StepMark, StepOptions};
use super::validation::{LINE_GAP_HINT, Samples, check_ecdf};
use crate::stats::ecdf_steps;
use crate::{ColorSpec, Dash, Result, ScaleKind, SizeSpec};

/// Options of an ECDF mark.
///
/// # Examples
///
/// ```
/// use umber_core::{ColorSpec, Dash, EcdfOptions, SizeSpec};
///
/// let options = EcdfOptions::default();
/// assert!(!options.complementary && options.label.is_empty());
/// assert_eq!((options.color, options.dash), (ColorSpec::AUTO, Dash::Solid));
/// assert_eq!(options.width, SizeSpec::Auto);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct EcdfOptions {
    /// Whether to draw `1 − F`, the share of the values above each value, falling from 1 to 0,
    /// instead of `F`, rising from 0 to 1.
    pub complementary: bool,
    /// The legend label; empty for none.
    pub label: String,
    /// The line color.
    pub color: ColorSpec,
    /// The dash style.
    pub dash: Dash,
    /// The line width; [`SizeSpec::Auto`] for the theme's line width. Dashes scale with it.
    pub width: SizeSpec,
}

impl Default for EcdfOptions {
    fn default() -> Self {
        EcdfOptions {
            complementary: false,
            label: String::new(),
            color: ColorSpec::AUTO,
            dash: Dash::Solid,
            width: SizeSpec::Auto,
        }
    }
}

/// An ECDF mark: the empirical cumulative distribution of its values, drawn as a step line,
/// with its [`EcdfOptions`].
///
/// At each distinct value, from the smallest, the line steps up to the share of the values at or
/// below it, starting from 0 and ending at 1 at the largest value (with
/// [`complementary`](EcdfOptions::complementary), down to the share above it, from 1 to 0). NaN
/// and missing times are left out. The values are validated and stored as given, and the steps
/// are computed once, when the mark is added; neither changes, and clones share both.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, Mark};
/// use umber_core::{EcdfOptions, Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.ecdf(AxisData::Numbers(vec![2.0, 1.0, f64::NAN, 2.0]), EcdfOptions::default())?;
/// let Some(Mark::Ecdf(ecdf)) = axes.marks().first() else { unreachable!() };
/// let AxisData::Numbers(values) = ecdf.values() else { unreachable!() };
/// assert_eq!((values.len(), values[1]), (4, 1.0));
/// assert!(values[2].is_nan());
/// assert!(!ecdf.options().complementary);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// An ECDF mark cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{EcdfMark, Mark};
///
/// let mark = Mark::Ecdf(EcdfMark {
///     values: unreachable!(),
///     options: unreachable!(),
///     steps: unreachable!(),
/// });
/// ```
#[derive(Clone, PartialEq)]
pub struct EcdfMark {
    values: Arc<AxisData>,
    options: EcdfOptions,
    steps: StepMark,
}

impl EcdfMark {
    /// Validates `values` for axes with an x axis of scale kind `x_kind` and a y axis of scale
    /// kind `y_kind` (see [`check_ecdf`] for the checks and their order), then stores them as
    /// given, with the step line of their ECDF. The first failure is the error, under `context`.
    pub(super) fn new(
        context: &'static str,
        kinds: (ScaleKind, ScaleKind),
        values: AxisData,
        options: EcdfOptions,
    ) -> Result<EcdfMark> {
        let complementary = options.complementary;
        let (x, y) = match check_ecdf(context, kinds, &values)? {
            Samples::Numbers(numbers) => {
                let mut sorted: Vec<f64> =
                    numbers.iter().copied().filter(|v| !v.is_nan()).collect();
                sorted.sort_unstable_by(f64::total_cmp);
                let (x, y) = ecdf_steps(&sorted, complementary);
                (AxisData::Numbers(x), y)
            }
            Samples::Times(times) => {
                let mut sorted: Vec<_> = times.iter().flatten().copied().collect();
                sorted.sort_unstable();
                let (x, y) = ecdf_steps(&sorted, complementary);
                (AxisData::Times(x.into_iter().map(Some).collect()), y)
            }
        };
        let series = XySeries::new(
            context,
            (kinds.0, ScaleKind::Linear),
            x,
            AxisData::Numbers(y),
            LINE_GAP_HINT,
        )?;
        let steps = StepMark::new(
            series,
            StepOptions {
                position: Step::Post,
                label: options.label.clone(),
                color: options.color,
                dash: options.dash,
                width: options.width,
            },
        );
        Ok(EcdfMark {
            values: Arc::new(values),
            options,
            steps,
        })
    }

    /// The values, as given: numbers on a linear or log x axis, times on a time x axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, Mark};
    /// use umber_core::{EcdfOptions, Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.ecdf(AxisData::Numbers(vec![3.0, 1.0]), EcdfOptions::default())?;
    /// let Some(Mark::Ecdf(ecdf)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(ecdf.values(), &AxisData::Numbers(vec![3.0, 1.0]));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn values(&self) -> &AxisData {
        &self.values
    }

    /// Complementary flag, label, color, dash and width.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, Mark};
    /// use umber_core::{EcdfOptions, Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let options = EcdfOptions { complementary: true, label: "Load".into(), ..EcdfOptions::default() };
    /// axes.ecdf(AxisData::Numbers(vec![1.0]), options.clone())?;
    /// let Some(Mark::Ecdf(ecdf)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(ecdf.options(), &options);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &EcdfOptions {
        &self.options
    }

    /// The step line that draws the ECDF: through the smallest value at its start level, then
    /// every distinct value at its level, holding each level up to the next value. It takes the
    /// ECDF's label, color, dash and width, so it is drawn, listed and labelled as a step line.
    pub(crate) fn steps(&self) -> &StepMark {
        &self.steps
    }
}

impl fmt::Debug for EcdfMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EcdfMark")
            .field("values", self.values())
            .field("options", &self.options)
            .finish()
    }
}
