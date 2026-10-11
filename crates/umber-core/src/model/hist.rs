//! Histogram marks: the options given to the histogram method and the stored histogram mark.

use std::sync::Arc;

use super::validation::check_hist;
use crate::binning::BinCount;
use crate::{ColorSpec, Result, ScaleKind};

/// Options of a histogram mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::HistOptions;
/// use umber_core::ColorSpec;
///
/// let options = HistOptions::default();
/// assert_eq!((options.bins, options.color), (30, ColorSpec::AUTO));
/// assert!(options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct HistOptions {
    /// The number of equal-width bins, from 1 to [`MAX_BINS`](crate::MAX_BINS).
    pub bins: i64,
    /// The legend label; empty for none.
    pub label: String,
    /// The bar color.
    pub color: ColorSpec,
}

impl Default for HistOptions {
    fn default() -> Self {
        HistOptions {
            bins: 30,
            label: String::new(),
            color: ColorSpec::AUTO,
        }
    }
}

/// A histogram mark: values to count in equal-width bins, with the bin count, label and color
/// of its [`HistOptions`].
///
/// The values are validated and stored unchanged; they never change, and clones share them.
/// They are binned when the figure is rendered, from their smallest to their largest finite
/// value.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{HistOptions, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.hist(&[1.0, 2.5, f64::NAN, 2.0], HistOptions { bins: 4, ..HistOptions::default() })?;
/// let Some(Mark::Hist(hist)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(hist.values().len(), 4);
/// assert_eq!(hist.options().bins, 4);
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
///     bins: unreachable!(),
///     label: HistOptions::default().label,
///     color: HistOptions::default().color,
/// });
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct HistMark {
    values: Arc<[f64]>,
    bins: BinCount,
    label: String,
    color: ColorSpec,
}

impl HistMark {
    /// Validates `options.bins` and `values` for axes with scale kinds `x_kind` and `y_kind`
    /// (see [`check_hist`] for the checks and their order), then stores the values unchanged,
    /// every bit included. The first failure is the error, under `context`.
    pub(super) fn new(
        context: &'static str,
        x_kind: ScaleKind,
        y_kind: ScaleKind,
        values: &[f64],
        options: HistOptions,
    ) -> Result<HistMark> {
        let bins = check_hist(context, x_kind, y_kind, values, options.bins)?;
        Ok(HistMark {
            values: Arc::from(values),
            bins,
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
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
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

    /// Bin count, label and color, as given: the [`HistOptions`] the mark was made with.
    ///
    /// The mark stores these values once; each call builds a new [`HistOptions`] from them.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{HistOptions, Mark};
    /// use umber_core::{ColorSpec, Figure, Rgba, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let options = HistOptions {
    ///     bins: 12,
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
            // At most `MAX_BINS`, so the count fits an `i64`.
            bins: self.bins.get() as i64,
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

    /// The validated number of bins.
    pub(crate) fn bin_count(&self) -> BinCount {
        self.bins
    }

    /// Whether `self` and `other` share their stored values.
    #[cfg(test)]
    pub(super) fn shares_data_with(&self, other: &HistMark) -> bool {
        Arc::ptr_eq(&self.values, &other.values)
    }
}
