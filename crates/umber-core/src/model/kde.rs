//! Kernel density marks: the options given to the density methods, the bandwidth, and the
//! stored mark, which keeps the values as given and the line that draws their density.

use std::fmt;
use std::sync::Arc;

use super::axis_data::{AxisData, XySeries};
use super::mark::{LineMark, LineOptions};
use super::validation::{LINE_GAP_HINT, check_bandwidth, check_kde_axes, check_kde_values};
use crate::error::number;
use crate::scale::MAX_MAGNITUDE;
use crate::stats::{kde_curve, scott_bandwidth};
use crate::{ColorSpec, Dash, Error, Result, ScaleKind, SizeSpec};

/// The number of points a density line is evaluated at, evenly spaced along its x range.
const KDE_POINTS: usize = 256;

/// How far a density line reaches past the smallest and the largest value, in bandwidths.
const KDE_REACH: f64 = 3.0;

/// The bandwidth of a kernel density: the standard deviation of the Gaussian kernel placed at
/// each value.
///
/// # Examples
///
/// ```
/// use umber_core::Bandwidth;
///
/// assert_eq!(Bandwidth::default(), Bandwidth::Scott);
/// assert_ne!(Bandwidth::Fixed(0.5), Bandwidth::Scott);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Bandwidth {
    /// Scott's rule, as violins use it: the sample standard deviation of the finite values
    /// times `n^(-1/5)`. Needs at least two distinct finite values.
    #[default]
    Scott,
    /// This bandwidth, in data units: finite, greater than 0, and within the supported
    /// magnitudes.
    Fixed(f64),
}

/// Options of a kernel density mark.
///
/// # Examples
///
/// ```
/// use umber_core::{Bandwidth, ColorSpec, Dash, KdeOptions, SizeSpec};
///
/// let options = KdeOptions::default();
/// assert_eq!((options.bandwidth, options.color), (Bandwidth::Scott, ColorSpec::AUTO));
/// assert_eq!((options.dash, options.width), (Dash::Solid, SizeSpec::Auto));
/// assert!(options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct KdeOptions {
    /// The bandwidth of the kernels.
    pub bandwidth: Bandwidth,
    /// The legend label; empty for none.
    pub label: String,
    /// The line color.
    pub color: ColorSpec,
    /// The dash style.
    pub dash: Dash,
    /// The line width; [`SizeSpec::Auto`] for the theme's line width. Dashes scale with it.
    pub width: SizeSpec,
}

impl Default for KdeOptions {
    fn default() -> Self {
        KdeOptions {
            bandwidth: Bandwidth::Scott,
            label: String::new(),
            color: ColorSpec::AUTO,
            dash: Dash::Solid,
            width: SizeSpec::Auto,
        }
    }
}

/// Options of the density lines of several series, whose series bring their own labels and
/// colors.
///
/// # Examples
///
/// ```
/// use umber_core::{Bandwidth, Dash, GroupedKdeOptions, SizeSpec};
///
/// let options = GroupedKdeOptions::default();
/// assert_eq!(options.bandwidth, Bandwidth::Scott);
/// assert_eq!((options.dash, options.width), (Dash::Solid, SizeSpec::Auto));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupedKdeOptions {
    /// The bandwidth of every series' kernels; [`Bandwidth::Scott`] takes each series' own.
    pub bandwidth: Bandwidth,
    /// The dash style of every line.
    pub dash: Dash,
    /// The width of every line; [`SizeSpec::Auto`] for the theme's line width.
    pub width: SizeSpec,
}

impl Default for GroupedKdeOptions {
    fn default() -> Self {
        GroupedKdeOptions {
            bandwidth: Bandwidth::Scott,
            dash: Dash::Solid,
            width: SizeSpec::Auto,
        }
    }
}

/// A kernel density mark: the Gaussian kernel density of its values, drawn as a line, with its
/// [`KdeOptions`].
///
/// The density is in data units, so the area under the line is 1. The line runs from three
/// bandwidths below the smallest finite value to three above the largest, through 256 evenly
/// spaced points. NaN is left out. The values are validated and stored as given, and the line is
/// computed once, when the mark is added; neither changes, and clones share both.
///
/// # Examples
///
/// ```
/// use umber_core::figure::Mark;
/// use umber_core::{Bandwidth, Figure, KdeOptions, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let options = KdeOptions { bandwidth: Bandwidth::Fixed(0.5), ..KdeOptions::default() };
/// axes.kde(&[1.0, 2.0, f64::NAN], options)?;
/// let Some(Mark::Kde(kde)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(kde.values().len(), 3);
/// assert_eq!(kde.bandwidth(), 0.5);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A density mark cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{KdeMark, Mark};
///
/// let mark = Mark::Kde(KdeMark {
///     values: unreachable!(),
///     options: unreachable!(),
///     bandwidth: 1.0,
///     line: unreachable!(),
/// });
/// ```
#[derive(Clone, PartialEq)]
pub struct KdeMark {
    values: Arc<[f64]>,
    options: KdeOptions,
    bandwidth: f64,
    line: LineMark,
}

impl KdeMark {
    /// Validates `values` and `options` for axes with an x axis of scale kind `x_kind` and a y
    /// axis of scale kind `y_kind`, then stores the values as given with the line of their
    /// density. The checks run in this order, and the first failure is the error, under
    /// `context`, with the values shown as `name`:
    ///
    /// 1. the axes, by [`check_kde_axes`];
    /// 2. a fixed bandwidth, by [`check_bandwidth`];
    /// 3. the values, by [`check_kde_values`];
    /// 4. for [`Bandwidth::Scott`], at least two distinct finite values (`Error::Data`);
    /// 5. the density stays within the supported magnitudes (`Error::Data` naming `bandwidth`).
    pub(super) fn new(
        context: &'static str,
        kinds: (ScaleKind, ScaleKind),
        (name, values): (&str, &[f64]),
        options: KdeOptions,
    ) -> Result<KdeMark> {
        check_kde_axes(context, kinds)?;
        if let Bandwidth::Fixed(h) = options.bandwidth {
            check_bandwidth(context, h)?;
        }
        check_kde_values(context, name, values)?;
        let bandwidth = match options.bandwidth {
            Bandwidth::Fixed(h) => h,
            Bandwidth::Scott => scott_bandwidth(values).ok_or_else(|| {
                Error::data(
                    context,
                    format!(
                        "{name} has 1 distinct finite value; the Scott bandwidth needs at least 2."
                    ),
                )
                .with_parameter("values")
                .with_hint("pass bandwidth= in data units.")
            })?,
        };
        let line = density_line(context, kinds.0, values, bandwidth, &options)?;
        Ok(KdeMark {
            values: Arc::from(values),
            options,
            bandwidth,
            line,
        })
    }

    /// The values, as given. NaN is left out of the density.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::Mark;
    /// use umber_core::{Figure, KdeOptions, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.kde(&[1.0, f64::NAN, 4.0], KdeOptions::default())?;
    /// let Some(Mark::Kde(kde)) = axes.marks().first() else { unreachable!() };
    /// assert!(kde.values()[1].is_nan());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Bandwidth, label, color, dash and width, as given.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::Mark;
    /// use umber_core::{Figure, KdeOptions, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let options = KdeOptions { label: "Density".into(), ..KdeOptions::default() };
    /// axes.kde(&[1.0, 2.0], options.clone())?;
    /// let Some(Mark::Kde(kde)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(kde.options(), &options);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &KdeOptions {
        &self.options
    }

    /// The bandwidth the density was computed with, in data units: the fixed bandwidth, or the
    /// one Scott's rule gave.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::Mark;
    /// use umber_core::{Figure, KdeOptions, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.kde(&[0.0, 2.0], KdeOptions::default())?;
    /// let Some(Mark::Kde(kde)) = axes.marks().first() else { unreachable!() };
    /// // The sample standard deviation, sqrt(2), times 2^(-1/5).
    /// assert!((kde.bandwidth() - 2.0_f64.sqrt() * 2.0_f64.powf(-0.2)).abs() < 1e-12);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn bandwidth(&self) -> f64 {
        self.bandwidth
    }

    /// The line that draws the density. It takes the mark's label, color, dash and width, so it
    /// is drawn, listed and labelled as a line.
    pub(crate) fn line(&self) -> &LineMark {
        &self.line
    }
}

impl fmt::Debug for KdeMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KdeMark")
            .field("values", &self.values())
            .field("options", &self.options)
            .field("bandwidth", &self.bandwidth)
            .finish()
    }
}

/// The line of the density of the finite `values` with `bandwidth`, on an x axis of scale kind
/// `x_kind`, drawn with `options`: [`KDE_POINTS`] points from [`KDE_REACH`] bandwidths below the
/// smallest finite value to as far above the largest, kept within the supported magnitudes.
/// `Error::Data` naming `bandwidth` when the density exceeds the supported magnitudes.
fn density_line(
    context: &'static str,
    x_kind: ScaleKind,
    values: &[f64],
    bandwidth: f64,
    options: &KdeOptions,
) -> Result<LineMark> {
    let finite = values.iter().copied().filter(|v| v.is_finite());
    let low = finite.clone().fold(f64::INFINITY, f64::min);
    let high = finite.fold(f64::NEG_INFINITY, f64::max);
    let reach = KDE_REACH * bandwidth;
    let from = (low - reach).max(-MAX_MAGNITUDE);
    let to = (high + reach).min(MAX_MAGNITUDE);
    // The values hold a finite one, as checked, so the curve has all its points.
    let curve = kde_curve(values, bandwidth, (from, to), KDE_POINTS).unwrap_or_default();
    // The largest density; NaN, from a bandwidth too narrow to divide by, wins.
    let peak = curve
        .iter()
        .map(|&(_, density)| density)
        .fold(0.0_f64, |peak, d| {
            if d.is_nan() || peak.is_nan() {
                f64::NAN
            } else {
                peak.max(d)
            }
        });
    if peak.is_nan() || peak > MAX_MAGNITUDE {
        return Err(Error::data(
            context,
            format!(
                "the density reaches {peak}; densities must not exceed {MAX_MAGNITUDE:e}.",
                peak = number(peak)
            ),
        )
        .with_parameter("bandwidth")
        .with_hint("use a wider bandwidth."));
    }
    let (x, y): (Vec<f64>, Vec<f64>) = curve.into_iter().unzip();
    let series = XySeries::new(
        context,
        (x_kind, ScaleKind::Linear),
        AxisData::Numbers(x),
        AxisData::Numbers(y),
        LINE_GAP_HINT,
    )?;
    Ok(LineMark::new(
        series,
        LineOptions {
            label: options.label.clone(),
            color: options.color,
            dash: options.dash,
            width: options.width,
        },
    ))
}
