//! Bar marks: the options and series given to the bar methods, and the stored plain, grouped
//! and stacked bar marks.

use std::sync::Arc;

use super::validation::{check_bar_group, check_bars};
use crate::{ColorSpec, Error, NumberStyle, Result, ScaleKind, SizeSpec, TickFormat, Transform};

/// The values written on bars: whether each bar shows its value, and in which format and size.
///
/// A plain or grouped bar shows its value just beyond its end; a stacked segment shows its
/// value inside, centred. Only bars with `annotate` take a format or a size.
///
/// # Examples
///
/// ```
/// use umber_core::{SizeSpec, TickFormat, ValueLabels};
///
/// let labels = ValueLabels::default();
/// assert!(!labels.annotate);
/// assert_eq!((labels.format, labels.size), (None, SizeSpec::Auto));
/// let shown = ValueLabels { annotate: true, format: Some(TickFormat::fixed(1)?), ..labels };
/// assert!(shown.annotate);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ValueLabels {
    /// Whether each bar shows its value.
    pub annotate: bool,
    /// The format of the values; `None` for the tick format of the value axis. The automatic
    /// format, [`TickFormat::AUTO`], writes each value on its own to six significant digits in
    /// the shortest form, as C's `%g` does (`42`, `28.4`, `0.12345`), in scientific form outside
    /// `1e-4 <= |v| < 1e6`.
    pub format: Option<TickFormat>,
    /// The text size; [`SizeSpec::Auto`] for the theme's tick label size.
    pub size: SizeSpec,
}

impl ValueLabels {
    /// Checks the labels under `context`: a format or a size without `annotate` would be dropped
    /// (`Error::Usage`, parameter `format` or `size`), and the values are numbers, which
    /// [`TickFormat::iso`] does not write (`Error::Style`, parameter `format`).
    pub(super) fn check(&self, context: &'static str) -> Result<()> {
        if !self.annotate {
            let given = if self.format.is_some() {
                Some("format")
            } else if self.size != SizeSpec::Auto {
                Some("size")
            } else {
                None
            };
            if let Some(parameter) = given {
                return Err(Error::usage(
                    context,
                    format!(
                        "{parameter} is given but annotate is false; only annotated bars show \
                         their values."
                    ),
                )
                .with_parameter(parameter)
                .with_hint("pass annotate=True, or leave the label options out."));
            }
        }
        if self
            .format
            .as_ref()
            .is_some_and(|format| format.style() == NumberStyle::Iso)
        {
            return Err(Error::style(
                context,
                "format is TickFormat.iso(), which writes times; bar values are numbers.",
            )
            .with_parameter("format")
            .with_hint("use TickFormat.AUTO or a number format such as TickFormat.fixed(1)."));
        }
        Ok(())
    }
}

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
/// assert!(!options.annotation.annotate);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BarOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The bar color.
    pub color: ColorSpec,
    /// The values written on the bars; none by default.
    pub annotation: ValueLabels,
}

impl Default for BarOptions {
    fn default() -> Self {
        BarOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
            annotation: ValueLabels::default(),
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
/// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
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
/// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
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
    /// a y axis of transform `y` (see [`check_bars`] for the checks and their order), then the
    /// value labels of `options` ([`ValueLabels::check`]), then stores both unchanged, every
    /// bit of the heights included; the stored heights share the allocation of `heights`. The
    /// first failure is the error, under `context`.
    pub(super) fn new(
        context: &'static str,
        x_kind: ScaleKind,
        y: Transform,
        categories: Vec<String>,
        heights: Arc<[f64]>,
        options: BarOptions,
    ) -> Result<BarMark> {
        check_bars(context, x_kind, y, &categories, &heights)?;
        options.annotation.check(context)?;
        Ok(BarMark {
            categories: Arc::from(categories),
            heights,
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
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
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
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
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
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
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

/// One series of grouped bars, stacked bars, stacked areas, a grouped histogram or grouped
/// density lines, as given to [`AxesMut::grouped_bar`](super::AxesMut::grouped_bar),
/// [`AxesMut::stacked_bar`](super::AxesMut::stacked_bar),
/// [`AxesMut::stacked_area`](super::AxesMut::stacked_area),
/// [`AxesMut::grouped_hist`](super::AxesMut::grouped_hist) or
/// [`AxesMut::grouped_kde`](super::AxesMut::grouped_kde): its label, values and color.
///
/// # Examples
///
/// ```
/// use umber_core::figure::Series;
/// use umber_core::ColorSpec;
///
/// let series = Series { label: "2025".into(), values: vec![3.0, 4.5], color: ColorSpec::AUTO };
/// assert_eq!(series.values.len(), 2);
/// let empty = Series::default();
/// assert!(empty.label.is_empty() && empty.values.is_empty());
/// assert_eq!(empty.color, ColorSpec::AUTO);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// The legend label; empty for none.
    pub label: String,
    /// One value per category for bars or per x value for areas, in the same order, or the
    /// values to count or to estimate a density of. NaN skips a bar, breaks every stacked area
    /// layer, or is left out of a histogram or a density.
    pub values: Vec<f64>,
    /// The series color.
    pub color: ColorSpec,
}

impl Default for Series {
    fn default() -> Self {
        Series {
            label: String::new(),
            values: Vec::new(),
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
struct StoredSeries {
    label: String,
    values: Box<[f64]>,
    color: ColorSpec,
}

/// A grouped or stacked bar mark: one or more series over shared categories, each series with
/// one value per category.
///
/// The categories and series are validated together and stored together; they never change,
/// and clones share them. Every category appears once, and there is at least one series.
/// [`Mark::GroupedBar`](super::Mark::GroupedBar) draws the series side by side,
/// [`Mark::StackedBar`](super::Mark::StackedBar) stacks them.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{Series, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// let series = |label: &str, values: &[f64]| Series {
///     label: label.into(),
///     values: values.to_vec(),
///     ..Series::default()
/// };
/// axes.grouped_bar(
///     vec!["a".into(), "b".into()],
///     vec![series("x", &[1.0, 2.0]), series("y", &[3.0, -1.0])], umber_core::ValueLabels::default(),
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
///     annotation: unreachable!(),
/// });
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BarGroupMark {
    categories: Arc<[String]>,
    series: Arc<[StoredSeries]>,
    annotation: ValueLabels,
}

impl BarGroupMark {
    /// Validates `categories` and `series` for axes with an x axis of scale kind `x_kind` and a
    /// y axis of transform `y`, arranged as `arrangement` (see [`check_bar_group`] for the checks and their order), then
    /// the value labels `annotation` ([`ValueLabels::check`]), then stores them unchanged, every
    /// bit of the values included. The first failure is the error, under `context`.
    pub(super) fn new(
        context: &'static str,
        arrangement: Arrangement,
        x_kind: ScaleKind,
        y: Transform,
        categories: Vec<String>,
        series: Vec<Series>,
        annotation: ValueLabels,
    ) -> Result<BarGroupMark> {
        check_bar_group(context, arrangement, x_kind, y, &categories, &series)?;
        annotation.check(context)?;
        let series: Vec<StoredSeries> = series
            .into_iter()
            .map(|s| StoredSeries {
                label: s.label,
                values: s.values.into_boxed_slice(),
                color: s.color,
            })
            .collect();
        Ok(BarGroupMark {
            categories: Arc::from(categories),
            series: Arc::from(series),
            annotation,
        })
    }

    /// The values written on the bars.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{Mark, Series};
    /// use umber_core::{Figure, ScaleKind, Theme, ValueLabels};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = Series { values: vec![2.0], ..Series::default() };
    /// let labels = ValueLabels { annotate: true, ..ValueLabels::default() };
    /// axes.stacked_bar(vec!["north".into()], vec![series], labels.clone())?;
    /// let Some(Mark::StackedBar(stack)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(stack.annotation(), &labels);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn annotation(&self) -> &ValueLabels {
        &self.annotation
    }

    /// The categories, each appearing once.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{Series, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = Series { values: vec![2.0], ..Series::default() };
    /// axes.stacked_bar(vec!["north".into()], vec![series], umber_core::ValueLabels::default())?;
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
    /// use umber_core::figure::{Series, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = |h: f64| Series { values: vec![h], ..Series::default() };
    /// axes.grouped_bar(vec!["a".into()], vec![series(1.0), series(f64::NAN)], umber_core::ValueLabels::default())?;
    /// let Some(Mark::GroupedBar(group)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(group.series().len(), 2);
    /// assert_eq!(group.series().next().map(|s| s.values()), Some(&[1.0][..]));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn series(&self) -> impl ExactSizeIterator<Item = SeriesView<'_>> {
        self.series.iter().map(|s| SeriesView {
            label: &s.label,
            values: &s.values,
            color: s.color,
        })
    }

    /// Whether `self` and `other` share their stored categories and series.
    #[cfg(test)]
    pub(super) fn shares_data_with(&self, other: &BarGroupMark) -> bool {
        Arc::ptr_eq(&self.categories, &other.categories) && Arc::ptr_eq(&self.series, &other.series)
    }
}

/// A read-only view of one series of a [`BarGroupMark`],
/// [`StackedAreaMark`](super::StackedAreaMark) or [`HistGroupMark`](super::HistGroupMark): its
/// label, values and color as given.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{Series, Mark};
/// use umber_core::{ColorSpec, Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// let series = Series { label: "y".into(), values: vec![2.0, -1.0], color: ColorSpec::AUTO };
/// axes.stacked_bar(vec!["a".into(), "b".into()], vec![series], umber_core::ValueLabels::default())?;
/// let Some(Mark::StackedBar(stack)) = axes.marks().first() else { unreachable!() };
/// let view = stack.series().next().unwrap();
/// assert_eq!((view.label(), view.values(), view.color()), ("y", &[2.0, -1.0][..], ColorSpec::AUTO));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeriesView<'a> {
    label: &'a str,
    values: &'a [f64],
    color: ColorSpec,
}

impl<'a> SeriesView<'a> {
    /// A read-only view of a stored series.
    pub(super) fn of_series(series: &'a Series) -> SeriesView<'a> {
        SeriesView {
            label: &series.label,
            values: &series.values,
            color: series.color,
        }
    }

    /// The legend label; empty for none.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{Series, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = Series { label: "2026".into(), values: vec![1.0], ..Series::default() };
    /// axes.grouped_bar(vec!["a".into()], vec![series], umber_core::ValueLabels::default())?;
    /// let Some(Mark::GroupedBar(group)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(group.series().next().map(|s| s.label()), Some("2026"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn label(&self) -> &'a str {
        self.label
    }

    /// The values, one per category or x value of the mark, in the same order. NaN skips a
    /// bar or breaks all stacked area layers.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{Series, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = Series { values: vec![1.0, 4.0], ..Series::default() };
    /// axes.grouped_bar(vec!["a".into(), "b".into()], vec![series], umber_core::ValueLabels::default())?;
    /// let Some(Mark::GroupedBar(group)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(group.series().next().map(|s| s.values()), Some(&[1.0, 4.0][..]));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn values(&self) -> &'a [f64] {
        self.values
    }

    /// The color as given, before it is resolved against a palette.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{Series, Mark};
    /// use umber_core::{ColorSpec, Figure, Rgba, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let color = ColorSpec::Fixed(Rgba::opaque(10, 20, 30));
    /// let series = Series { values: vec![1.0], color, ..Series::default() };
    /// axes.grouped_bar(vec!["a".into()], vec![series], umber_core::ValueLabels::default())?;
    /// let Some(Mark::GroupedBar(group)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(group.series().next().map(|s| s.color()), Some(color));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn color(&self) -> ColorSpec {
        self.color
    }
}
