//! Bar marks: the options and series given to the bar methods, and the stored plain, grouped
//! and stacked bar marks.

use std::sync::Arc;

use super::validation::{check_bar_group, check_bars};
use crate::{ColorSpec, Result, ScaleKind, Transform};

/// Options of a bar mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::BarOptions;
/// use umber_core::ColorSpec;
///
/// let options = BarOptions::default();
/// assert_eq!(options.color, ColorSpec::AUTO);
/// assert!(options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BarOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The bar color.
    pub color: ColorSpec,
}

impl Default for BarOptions {
    fn default() -> Self {
        BarOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
        }
    }
}

/// A bar mark: one bar per category, each as high as its height, with its [`BarOptions`].
///
/// The categories and heights are validated together and stored together; they never change,
/// and clones share them. Every category appears once.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{BarOptions, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// axes.bar(vec!["a".into(), "b".into()], &[3.0, -1.0], BarOptions::default())?;
/// let Some(Mark::Bar(bar)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(bar.categories(), ["a", "b"]);
/// assert_eq!(bar.heights(), &[3.0, -1.0]);
/// assert!(bar.options().label.is_empty());
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A bar mark cannot be built outside [`Axes`](super::Axes), so its data is always validated:
///
/// ```compile_fail,E0451
/// use umber_core::figure::{BarMark, BarOptions, Mark};
///
/// let mark = Mark::Bar(BarMark {
///     categories: unreachable!(),
///     heights: unreachable!(),
///     options: BarOptions::default(),
/// });
/// ```
///
/// The heights of one bar mark cannot be paired with the categories of another:
///
/// ```compile_fail,E0616
/// use umber_core::figure::{BarOptions, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// axes.bar(vec!["a".into()], &[1.0], BarOptions::default())?
///     .bar(vec!["a".into(), "b".into()], &[1.0, 2.0], BarOptions::default())?;
/// let [Mark::Bar(short), Mark::Bar(long)] = axes.marks() else { unreachable!() };
/// let mut mixed = short.clone();
/// mixed.heights = long.heights.clone();
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BarMark {
    categories: Arc<[String]>,
    heights: Arc<[f64]>,
    options: BarOptions,
}

impl BarMark {
    /// Validates `categories` and `heights` for axes with an x axis of scale kind `x_kind` and
    /// a y axis of transform `y` (see [`check_bars`] for the checks and their order), then stores both unchanged, every
    /// bit of the heights included. The first failure is the error, under `context`.
    pub(super) fn new(
        context: &'static str,
        x_kind: ScaleKind,
        y: Transform,
        categories: Vec<String>,
        heights: &[f64],
        options: BarOptions,
    ) -> Result<BarMark> {
        check_bars(context, x_kind, y, &categories, heights)?;
        Ok(BarMark {
            categories: Arc::from(categories),
            heights: Arc::from(heights),
            options,
        })
    }

    /// The categories, one per bar, each appearing once.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{BarOptions, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// axes.bar(vec!["north".into()], &[2.0], BarOptions::default())?;
    /// let Some(Mark::Bar(bar)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(bar.categories(), ["north"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    /// The bar heights, as many as the categories. NaN draws no bar.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{BarOptions, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// axes.bar(vec!["a".into(), "b".into()], &[2.0, f64::NAN], BarOptions::default())?;
    /// let Some(Mark::Bar(bar)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(bar.heights()[0], 2.0);
    /// assert!(bar.heights()[1].is_nan());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn heights(&self) -> &[f64] {
        &self.heights
    }

    /// Label and color.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{BarOptions, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let options = BarOptions { label: "sales".into(), ..BarOptions::default() };
    /// axes.bar(vec!["a".into()], &[2.0], options)?;
    /// let Some(Mark::Bar(bar)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(bar.options().label, "sales");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &BarOptions {
        &self.options
    }

    /// Whether `self` and `other` share their stored categories and heights.
    #[cfg(test)]
    pub(super) fn shares_data_with(&self, other: &BarMark) -> bool {
        Arc::ptr_eq(&self.categories, &other.categories)
            && Arc::ptr_eq(&self.heights, &other.heights)
    }
}

/// One series of a grouped or stacked bar mark, as given to
/// [`Axes::grouped_bar`](super::Axes::grouped_bar) and
/// [`Axes::stacked_bar`](super::Axes::stacked_bar): its label, one height per category, and
/// its color.
///
/// # Examples
///
/// ```
/// use umber_core::figure::BarSeries;
/// use umber_core::ColorSpec;
///
/// let series = BarSeries { label: "2025".into(), heights: vec![3.0, 4.5], color: ColorSpec::AUTO };
/// assert_eq!(series.heights.len(), 2);
/// let empty = BarSeries::default();
/// assert!(empty.label.is_empty() && empty.heights.is_empty());
/// assert_eq!(empty.color, ColorSpec::AUTO);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BarSeries {
    /// The legend label; empty for none.
    pub label: String,
    /// One height per category, in the order of the mark's categories. NaN draws no bar.
    pub heights: Vec<f64>,
    /// The series color.
    pub color: ColorSpec,
}

impl Default for BarSeries {
    fn default() -> Self {
        BarSeries {
            label: String::new(),
            heights: Vec::new(),
            color: ColorSpec::AUTO,
        }
    }
}

/// How the series of a [`BarGroupMark`] share a category: side by side or stacked. The model
/// checks stacked totals only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Arrangement {
    /// Side by side within the category's band.
    Grouped,
    /// Stacked on each other, positive and negative heights apart.
    Stacked,
}

/// One stored series of a [`BarGroupMark`].
#[derive(Debug, Clone, PartialEq)]
struct StoredBarSeries {
    label: String,
    heights: Box<[f64]>,
    color: ColorSpec,
}

/// A grouped or stacked bar mark: one or more series over shared categories, each series with
/// one height per category.
///
/// The categories and series are validated together and stored together; they never change,
/// and clones share them. Every category appears once, and there is at least one series.
/// [`Mark::GroupedBar`](super::Mark::GroupedBar) draws the series side by side,
/// [`Mark::StackedBar`](super::Mark::StackedBar) stacks them.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{BarSeries, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// let series = |label: &str, heights: &[f64]| BarSeries {
///     label: label.into(),
///     heights: heights.to_vec(),
///     ..BarSeries::default()
/// };
/// axes.grouped_bar(
///     vec!["a".into(), "b".into()],
///     vec![series("x", &[1.0, 2.0]), series("y", &[3.0, -1.0])],
/// )?;
/// let Some(Mark::GroupedBar(group)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(group.categories(), ["a", "b"]);
/// let labels: Vec<&str> = group.series().map(|s| s.label()).collect();
/// assert_eq!(labels, ["x", "y"]);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A group cannot be built outside [`Axes`](super::Axes), so its data is always validated:
///
/// ```compile_fail,E0451
/// use umber_core::figure::{BarGroupMark, Mark};
///
/// let mark = Mark::StackedBar(BarGroupMark {
///     categories: unreachable!(),
///     series: unreachable!(),
/// });
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BarGroupMark {
    categories: Arc<[String]>,
    series: Arc<[StoredBarSeries]>,
}

impl BarGroupMark {
    /// Validates `categories` and `series` for axes with an x axis of scale kind `x_kind` and a
    /// y axis of transform `y`, arranged as `arrangement` (see [`check_bar_group`] for the checks and their order), then
    /// stores them unchanged, every bit of the heights included. The first failure is the
    /// error, under `context`.
    pub(super) fn new(
        context: &'static str,
        arrangement: Arrangement,
        x_kind: ScaleKind,
        y: Transform,
        categories: Vec<String>,
        series: Vec<BarSeries>,
    ) -> Result<BarGroupMark> {
        check_bar_group(context, arrangement, x_kind, y, &categories, &series)?;
        let series: Vec<StoredBarSeries> = series
            .into_iter()
            .map(|s| StoredBarSeries {
                label: s.label,
                heights: s.heights.into_boxed_slice(),
                color: s.color,
            })
            .collect();
        Ok(BarGroupMark {
            categories: Arc::from(categories),
            series: Arc::from(series),
        })
    }

    /// The categories, each appearing once.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{BarSeries, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = BarSeries { heights: vec![2.0], ..BarSeries::default() };
    /// axes.stacked_bar(vec!["north".into()], vec![series])?;
    /// let Some(Mark::StackedBar(stack)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(stack.categories(), ["north"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    /// The series, in the order given; at least one.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{BarSeries, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = |h: f64| BarSeries { heights: vec![h], ..BarSeries::default() };
    /// axes.grouped_bar(vec!["a".into()], vec![series(1.0), series(f64::NAN)])?;
    /// let Some(Mark::GroupedBar(group)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(group.series().len(), 2);
    /// assert_eq!(group.series().next().map(|s| s.heights()), Some(&[1.0][..]));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn series(&self) -> impl ExactSizeIterator<Item = BarSeriesView<'_>> {
        self.series.iter().map(|s| BarSeriesView {
            label: &s.label,
            heights: &s.heights,
            color: s.color,
        })
    }

    /// Whether `self` and `other` share their stored categories and series.
    #[cfg(test)]
    pub(super) fn shares_data_with(&self, other: &BarGroupMark) -> bool {
        Arc::ptr_eq(&self.categories, &other.categories) && Arc::ptr_eq(&self.series, &other.series)
    }
}

/// A read-only view of one series of a [`BarGroupMark`]: its label, its heights (one per
/// category of the mark) and its color as given.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{BarSeries, Mark};
/// use umber_core::{ColorSpec, Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// let series = BarSeries { label: "y".into(), heights: vec![2.0, -1.0], color: ColorSpec::AUTO };
/// axes.stacked_bar(vec!["a".into(), "b".into()], vec![series])?;
/// let Some(Mark::StackedBar(stack)) = axes.marks().first() else { unreachable!() };
/// let view = stack.series().next().unwrap();
/// assert_eq!((view.label(), view.heights(), view.color()), ("y", &[2.0, -1.0][..], ColorSpec::AUTO));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarSeriesView<'a> {
    label: &'a str,
    heights: &'a [f64],
    color: ColorSpec,
}

impl<'a> BarSeriesView<'a> {
    /// The legend label; empty for none.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{BarSeries, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = BarSeries { label: "2026".into(), heights: vec![1.0], ..BarSeries::default() };
    /// axes.grouped_bar(vec!["a".into()], vec![series])?;
    /// let Some(Mark::GroupedBar(group)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(group.series().next().map(|s| s.label()), Some("2026"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn label(&self) -> &'a str {
        self.label
    }

    /// The heights, one per category of the mark, in the mark's category order. NaN draws no
    /// bar.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{BarSeries, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = BarSeries { heights: vec![1.0, 4.0], ..BarSeries::default() };
    /// axes.grouped_bar(vec!["a".into(), "b".into()], vec![series])?;
    /// let Some(Mark::GroupedBar(group)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(group.series().next().map(|s| s.heights()), Some(&[1.0, 4.0][..]));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn heights(&self) -> &'a [f64] {
        self.heights
    }

    /// The color as given, before it is resolved against a palette.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{BarSeries, Mark};
    /// use umber_core::{ColorSpec, Figure, Rgba, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let color = ColorSpec::Fixed(Rgba::opaque(10, 20, 30));
    /// let series = BarSeries { heights: vec![1.0], color, ..BarSeries::default() };
    /// axes.grouped_bar(vec!["a".into()], vec![series])?;
    /// let Some(Mark::GroupedBar(group)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(group.series().next().map(|s| s.color()), Some(color));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn color(&self) -> ColorSpec {
        self.color
    }
}
