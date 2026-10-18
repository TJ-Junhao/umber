//! Kernel density marks: the options given to the density methods, the bandwidth, and the
//! stored mark, which keeps the values as given and the line that draws their density.

use std::fmt;
use std::sync::Arc;

use super::axis_data::{AxisData, XySeries};
use super::mark::{LineMark, LineOptions};
use super::validation::{
    LINE_GAP_HINT, check_bandwidth, check_kde_axes, check_kde_values, grouped,
};
use crate::error::number;
use crate::scale::MAX_MAGNITUDE;
use crate::stats::{kde_curve, scott_bandwidth};
use crate::{ColorSpec, Dash, Error, Result, ScaleKind, SizeSpec};

/// The fewest points a density line is evaluated at, evenly spaced along its x range.
const KDE_MIN_POINTS: usize = 256;

/// The most points a density line is evaluated at; a bandwidth that needs more is refused.
const KDE_MAX_POINTS: usize = 16_384;

/// The widest spacing of a density line's points, in bandwidths: close enough that the line
/// follows every kernel and its trapezoid area is the density's mass on the drawn range.
const KDE_SPACING: f64 = 0.25;

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
/// bandwidths below the smallest finite value to three above the largest, through evenly spaced
/// points at most a quarter bandwidth apart: at least 256, at most 16,384. NaN is left out. The values are validated and stored as given, and the line is
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
    /// 5. the line, its points and its density, by [`density_line`].
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
                Error::at_input(
                    context,
                    name,
                    vec![None],
                    vec![
                        String::new(),
                        " has 1 distinct finite value; the Scott bandwidth needs at least 2."
                            .into(),
                    ],
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
/// `x_kind`, drawn with `options`: from [`KDE_REACH`] bandwidths below the smallest finite value
/// to as far above the largest, through evenly spaced points at most [`KDE_SPACING`] bandwidths
/// apart, at least [`KDE_MIN_POINTS`] of them. The checks run in this order, under `context`:
///
/// 1. the line stays within the supported magnitudes (`Error::Data` naming `values`);
/// 2. it needs at most [`KDE_MAX_POINTS`] points (`Error::Style` naming `bandwidth`);
/// 3. the density stays within the supported magnitudes (`Error::Data` naming `bandwidth`).
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
    let (from, to) = (low - reach, high + reach);
    if let Some(end) = [from, to].into_iter().find(|end| end.abs() > MAX_MAGNITUDE) {
        return Err(Error::data(
            context,
            format!(
                "the curve reaches {end}, three bandwidths past the values; magnitudes must not \
                 exceed {MAX_MAGNITUDE:e}.",
                end = number(end)
            ),
        )
        .with_parameter("values")
        .with_hint("rescale the values first."));
    }
    let points = line_points(context, (from, to), bandwidth)?;
    // The values hold a finite one, as checked, so the curve has all its points.
    let curve = kde_curve(values, bandwidth, (from, to), points).unwrap_or_default();
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

/// The number of points of a density line from `from` to `to` with `bandwidth`: enough for
/// points at most [`KDE_SPACING`] bandwidths apart, and at least [`KDE_MIN_POINTS`].
/// `Error::Style` naming `bandwidth` under `context` when that takes more than
/// [`KDE_MAX_POINTS`].
fn line_points(context: &'static str, (from, to): (f64, f64), bandwidth: f64) -> Result<usize> {
    // Divide by the bandwidth first: `KDE_SPACING * bandwidth` underflows for subnormal widths.
    let needed = ((to - from) / bandwidth / KDE_SPACING).ceil() + 1.0;
    if needed <= KDE_MAX_POINTS as f64 {
        // At most `KDE_MAX_POINTS`, and at least 1: the cast is exact.
        return Ok((needed as usize).max(KDE_MIN_POINTS));
    }
    let shown = if needed < 1e15 {
        grouped(needed as usize)
    } else {
        number(needed).to_string()
    };
    Err(Error::style(
        context,
        format!(
            "bandwidth is {h}, which needs {shown} samples over the drawn range; at most {max} \
             are drawn.",
            h = number(bandwidth),
            max = grouped(KDE_MAX_POINTS),
        ),
    )
    .with_parameter("bandwidth")
    .with_hint("use a wider bandwidth."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Theme;
    use crate::model::{Figure, Mark};

    /// The drawn line of a density of `values` with `bandwidth`, as x and y.
    fn drawn(values: &[f64], bandwidth: Bandwidth) -> (Vec<f64>, Vec<f64>) {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let options = KdeOptions {
            bandwidth,
            ..KdeOptions::default()
        };
        f.single_axes_mut().kde(values, options).unwrap();
        let [Mark::Kde(mark)] = f.single_axes().marks() else {
            panic!("expected one density line");
        };
        let line = mark.line();
        (line.x().numbers().to_vec(), line.y().numbers().to_vec())
    }

    /// The trapezoid area under the polyline `(x, y)`.
    fn area(x: &[f64], y: &[f64]) -> f64 {
        x.windows(2)
            .zip(y.windows(2))
            .map(|(x, y)| (x[1] - x[0]) * (y[0] + y[1]) / 2.0)
            .sum()
    }

    /// Φ(3): the share of a Gaussian below three standard deviations above its mean.
    const PHI_3: f64 = 0.998_650_101_968_369_9;

    #[test]
    fn the_drawn_line_resolves_narrow_bandwidths() {
        // The line reaches three bandwidths past the outer values, so each outer value loses
        // its tail beyond them and the middle one loses nothing: the mass on the drawn range.
        let mass = (2.0 * PHI_3 + 1.0) / 3.0;
        for h in [1e-3, 2e-3, 0.01, 0.05, 0.2] {
            let (x, y) = drawn(&[0.0, 0.5, 1.0], Bandwidth::Fixed(h));
            assert!(
                x.len() >= KDE_MIN_POINTS && x.len() <= KDE_MAX_POINTS,
                "{}",
                x.len()
            );
            let spacing = (x[x.len() - 1] - x[0]) / (x.len() - 1) as f64;
            assert!(spacing <= h / 4.0 * (1.0 + 1e-12), "{spacing} for {h}");
            let drawn = area(&x, &y);
            assert!(
                (drawn - mass).abs() < 1e-3,
                "area {drawn} for {h}, expected {mass}"
            );
        }
        // A wide bandwidth keeps the smallest grid.
        let (x, _) = drawn(&[0.0, 0.5, 1.0], Bandwidth::Fixed(1.0));
        assert_eq!(x.len(), KDE_MIN_POINTS);
    }

    #[test]
    fn a_span_far_below_the_bandwidth_keeps_the_gaussian() {
        // Data spans of 1e-308 and 1e-310 are far below the bandwidth: the density is one
        // Gaussian of peak 1 / sqrt(2π), not zeros or NaN from huge intermediates.
        let peak = 1.0 / (2.0 * std::f64::consts::PI).sqrt();
        for tiny in [1e-308, 1e-310] {
            let (x, y) = drawn(&[0.0, tiny], Bandwidth::Fixed(1.0));
            let top = y.iter().copied().fold(0.0, f64::max);
            assert!((top / peak - 1.0).abs() < 1e-4, "{top} for {tiny}");
            assert!((area(&x, &y) - PHI_3 * 2.0 + 1.0).abs() < 1e-3);
        }
    }
}
