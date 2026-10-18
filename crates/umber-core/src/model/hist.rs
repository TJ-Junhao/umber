//! Histogram marks: the options given to the histogram methods, the bins, styles and statistics
//! they take, and the stored histogram marks of one series or of several.

use std::sync::Arc;

use super::bar::{Series, SeriesView};
use super::validation::{check_grouped_hist, check_hist};
use crate::binning::{BinSpec, HistBinning};
use crate::{ColorSpec, Result, ScaleKind};

/// The bins of a histogram: a number of equal-width bins over the range of the values, or
/// explicit edges.
///
/// # Examples
///
/// ```
/// use umber_core::Bins;
///
/// assert_eq!(Bins::default(), Bins::Count(30));
/// let uneven = Bins::Edges(vec![0.0, 1.0, 2.5, 10.0]);
/// assert_ne!(uneven, Bins::Count(3));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Bins {
    /// This many equal-width bins, from 1 to [`MAX_BINS`](crate::MAX_BINS), from the smallest
    /// to the largest finite value.
    Count(i64),
    /// The bin edges: at least two, finite, strictly increasing. Bins are half-open, `[low,
    /// high)`, except the last, which also holds its high edge. Every finite value must lie
    /// within the edges.
    Edges(Vec<f64>),
}

impl Default for Bins {
    fn default() -> Self {
        Bins::Count(30)
    }
}

/// How a histogram draws its bins.
///
/// # Examples
///
/// ```
/// use umber_core::HistStyle;
///
/// assert_eq!(HistStyle::default(), HistStyle::Bars);
/// assert_ne!(HistStyle::Step, HistStyle::Stacked);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum HistStyle {
    /// One filled bar per bin, from the baseline to its height.
    #[default]
    Bars,
    /// The outline of the bars only, as one stepped line: from the baseline up the first bin's
    /// low edge, along the top of every bin, and down the last bin's high edge. Each series of
    /// a grouped histogram is one outline, normalized on its own.
    Step,
    /// The bars of the series of a grouped histogram stacked in each bin, in series order, and
    /// normalized together, so each stack shows the whole. With one series it is
    /// [`Bars`](Self::Bars).
    Stacked,
}

/// What the height of a histogram's bar shows.
///
/// # Examples
///
/// ```
/// use umber_core::HistStat;
///
/// assert_eq!(HistStat::default(), HistStat::Count);
/// assert_ne!(HistStat::Fraction, HistStat::Density);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum HistStat {
    /// The number of values in the bin.
    #[default]
    Count,
    /// The share of the finite values in the bin: the heights sum to 1.
    Fraction,
    /// The share of the finite values in the bin over the bin's width: the bars have an area
    /// of 1.
    Density,
}

/// Options of a histogram mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::HistOptions;
/// use umber_core::{Bins, ColorSpec, HistStat, HistStyle};
///
/// let options = HistOptions::default();
/// assert_eq!((options.bins, options.color), (Bins::Count(30), ColorSpec::AUTO));
/// assert_eq!((options.style, options.stat), (HistStyle::Bars, HistStat::Count));
/// assert!(!options.cumulative && options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct HistOptions {
    /// The bins: their number, from 1 to [`MAX_BINS`](crate::MAX_BINS), or their edges.
    pub bins: Bins,
    /// How the bins are drawn.
    pub style: HistStyle,
    /// Whether each bar adds up every bin up to its own, from the left, so the last shows the
    /// whole; not with [`HistStat::Density`].
    pub cumulative: bool,
    /// What each bar's height shows.
    pub stat: HistStat,
    /// The legend label; empty for none.
    pub label: String,
    /// The bar color.
    pub color: ColorSpec,
}

impl Default for HistOptions {
    fn default() -> Self {
        HistOptions {
            bins: Bins::default(),
            style: HistStyle::Bars,
            cumulative: false,
            stat: HistStat::Count,
            label: String::new(),
            color: ColorSpec::AUTO,
        }
    }
}

/// Options of a grouped histogram, whose series bring their own labels and colors.
///
/// The default style is [`HistStyle::Step`]: [`HistStyle::Bars`] is refused for a grouped
/// histogram, since the bars of its series would hide each other.
///
/// # Examples
///
/// ```
/// use umber_core::{Bins, GroupedHistOptions, HistStat, HistStyle};
///
/// let options = GroupedHistOptions::default();
/// assert_eq!((options.bins, options.style), (Bins::Count(30), HistStyle::Step));
/// assert_eq!((options.cumulative, options.stat), (false, HistStat::Count));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct GroupedHistOptions {
    /// The bins every series shares: their number, from 1 to [`MAX_BINS`](crate::MAX_BINS),
    /// over the range of all the series, or their edges.
    pub bins: Bins,
    /// How the series are drawn: [`HistStyle::Step`] or [`HistStyle::Stacked`].
    pub style: HistStyle,
    /// Whether each bar adds up every bin up to its own, from the left; not with
    /// [`HistStat::Density`].
    pub cumulative: bool,
    /// What each bar's height shows.
    pub stat: HistStat,
}

impl Default for GroupedHistOptions {
    fn default() -> Self {
        GroupedHistOptions {
            bins: Bins::default(),
            style: HistStyle::Step,
            cumulative: false,
            stat: HistStat::Count,
        }
    }
}

/// The [`Bins`] that the validated bins `spec` were made from.
fn bins_of(spec: &BinSpec) -> Bins {
    match spec {
        // At most `MAX_BINS`, so the count fits an `i64`.
        BinSpec::Count(count) => Bins::Count(count.get() as i64),
        BinSpec::Edges(edges) => Bins::Edges(edges.to_vec()),
    }
}

/// A histogram mark: values to count in bins, with the bins, style, statistic, label and color
/// of its [`HistOptions`].
///
/// The values are validated and stored unchanged; they never change, and clones share them.
/// They are binned when the figure is rendered: in equal-width bins from their smallest to
/// their largest finite value, or in the explicit bins, which hold every finite value.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{HistOptions, Mark};
/// use umber_core::{Bins, Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let four = HistOptions { bins: Bins::Count(4), ..HistOptions::default() };
/// axes.hist(&[1.0, 2.5, f64::NAN, 2.0], four)?;
/// let Some(Mark::Hist(hist)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(hist.values().len(), 4);
/// assert_eq!(hist.options().bins, Bins::Count(4));
/// assert!(hist.options().label.is_empty());
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A histogram mark cannot be built outside [`Axes`](super::Axes), so its data is always
/// validated:
///
/// ```compile_fail,E0451
/// use umber_core::figure::{HistMark, HistOptions, Mark};
///
/// let mark = Mark::Hist(HistMark {
///     values: unreachable!(),
///     binning: unreachable!(),
///     label: HistOptions::default().label,
///     color: HistOptions::default().color,
/// });
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct HistMark {
    values: Arc<[f64]>,
    binning: HistBinning,
    label: String,
    color: ColorSpec,
}

impl HistMark {
    /// Validates `options` and `values` for axes with an x axis of scale kind `x_kind` (see
    /// [`check_hist`] for the checks and their order), then stores the values unchanged, every
    /// bit included; the stored values share the allocation of `values`. The first failure is
    /// the error, under `context`.
    pub(super) fn new(
        context: &'static str,
        x_kind: ScaleKind,
        values: Arc<[f64]>,
        options: HistOptions,
    ) -> Result<HistMark> {
        let binning = check_hist(context, x_kind, &values, &options)?;
        Ok(HistMark {
            values,
            binning,
            label: options.label,
            color: options.color,
        })
    }

    /// The values, as given. NaN is ignored when they are binned.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{HistOptions, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.hist(&[3.0, f64::NAN], HistOptions::default())?;
    /// let Some(Mark::Hist(hist)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(hist.values()[0], 3.0);
    /// assert!(hist.values()[1].is_nan());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Bins, style, statistic, label and color, as given: the [`HistOptions`] the mark was made
    /// with.
    ///
    /// The mark stores these values once; each call builds a new [`HistOptions`] from them.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{HistOptions, Mark};
    /// use umber_core::{Bins, ColorSpec, Figure, HistStat, HistStyle, Rgba, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let options = HistOptions {
    ///     bins: Bins::Edges(vec![0.0, 1.5, 3.0]),
    ///     style: HistStyle::Step,
    ///     cumulative: true,
    ///     stat: HistStat::Fraction,
    ///     label: "latency".into(),
    ///     color: ColorSpec::Fixed(Rgba::opaque(10, 20, 30)),
    /// };
    /// axes.hist(&[1.0, 2.0], options.clone())?;
    /// let Some(Mark::Hist(hist)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(hist.options(), options);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> HistOptions {
        HistOptions {
            bins: bins_of(self.binning.bins()),
            style: self.binning.style(),
            cumulative: self.binning.cumulative(),
            stat: self.binning.stat(),
            label: self.label.clone(),
            color: self.color,
        }
    }

    /// The legend label; empty for none.
    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    /// The color as given, before it is resolved against a palette.
    pub(crate) fn color(&self) -> ColorSpec {
        self.color
    }

    /// The validated bins, style, statistic and accumulation.
    pub(crate) fn binning(&self) -> &HistBinning {
        &self.binning
    }

    /// Whether `self` and `other` share their stored values.
    #[cfg(test)]
    pub(super) fn shares_data_with(&self, other: &HistMark) -> bool {
        Arc::ptr_eq(&self.values, &other.values)
    }
}

/// A grouped histogram mark: one or more series of values counted in shared bins and drawn as
/// outlines ([`HistStyle::Step`]) or stacked bars ([`HistStyle::Stacked`]), each series with its
/// own label and color.
///
/// The series are validated and stored together; they never change, and clones share them.
/// They are binned when the figure is rendered, in equal-width bins over the range of every
/// series, or in the explicit bins.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{Mark, Series};
/// use umber_core::{Figure, GroupedHistOptions, HistStyle, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let series = |label: &str, values: &[f64]| Series {
///     label: label.into(),
///     values: values.to_vec(),
///     ..Series::default()
/// };
/// let stacked = GroupedHistOptions { style: HistStyle::Stacked, ..GroupedHistOptions::default() };
/// axes.grouped_hist(vec![series("north", &[1.0, 2.0]), series("south", &[2.5])], stacked)?;
/// let Some(Mark::GroupedHist(group)) = axes.marks().first() else { unreachable!() };
/// let labels: Vec<&str> = group.series().map(|s| s.label()).collect();
/// assert_eq!(labels, ["north", "south"]);
/// assert_eq!(group.options().style, HistStyle::Stacked);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A grouped histogram cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{HistGroupMark, Mark};
///
/// let mark = Mark::GroupedHist(HistGroupMark { series: unreachable!(), binning: unreachable!() });
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct HistGroupMark {
    series: Arc<[Series]>,
    binning: HistBinning,
}

impl HistGroupMark {
    /// Validates `options` and `series` for axes with an x axis of scale kind `x_kind` (see
    /// [`check_grouped_hist`] for the checks and their order), then stores the series
    /// unchanged. The first failure is the error, under `context`.
    pub(super) fn new(
        context: &'static str,
        x_kind: ScaleKind,
        series: Vec<Series>,
        options: GroupedHistOptions,
    ) -> Result<HistGroupMark> {
        let binning = check_grouped_hist(context, x_kind, &series, &options)?;
        Ok(HistGroupMark {
            series: Arc::from(series),
            binning,
        })
    }

    /// The series, in the order given; at least one.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{Mark, Series};
    /// use umber_core::{Figure, GroupedHistOptions, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let series = Series { label: "a".into(), values: vec![1.0, f64::NAN], ..Series::default() };
    /// axes.grouped_hist(vec![series], GroupedHistOptions::default())?;
    /// let Some(Mark::GroupedHist(group)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(group.series().len(), 1);
    /// assert_eq!(group.series().next().map(|s| s.values()[0]), Some(1.0));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn series(&self) -> impl ExactSizeIterator<Item = SeriesView<'_>> {
        self.series.iter().map(SeriesView::of_series)
    }

    /// Bins, style, statistic and accumulation, as given: the [`GroupedHistOptions`] the mark
    /// was made with.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{Mark, Series};
    /// use umber_core::{Bins, Figure, GroupedHistOptions, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let options = GroupedHistOptions {
    ///     bins: Bins::Edges(vec![0.0, 2.0]),
    ///     ..GroupedHistOptions::default()
    /// };
    /// let series = Series { values: vec![1.0], ..Series::default() };
    /// axes.grouped_hist(vec![series], options.clone())?;
    /// let Some(Mark::GroupedHist(group)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(group.options(), options);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> GroupedHistOptions {
        GroupedHistOptions {
            bins: bins_of(self.binning.bins()),
            style: self.binning.style(),
            cumulative: self.binning.cumulative(),
            stat: self.binning.stat(),
        }
    }

    /// The validated bins, style, statistic and accumulation.
    pub(crate) fn binning(&self) -> &HistBinning {
        &self.binning
    }
}
