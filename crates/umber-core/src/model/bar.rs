//! Bar marks: the options and series given to the bar methods, and the stored plain, grouped
//! and stacked bar marks.

use std::sync::Arc;

use super::validation::check_bars;
use crate::{ColorSpec, Result, ScaleKind};

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
    /// Validates `categories` and `heights` for axes with scale kinds `x_kind` and `y_kind`
    /// (see [`check_bars`] for the checks and their order), then stores both unchanged, every
    /// bit of the heights included. The first failure is the error, under `context`.
    pub(super) fn new(
        context: &'static str,
        x_kind: ScaleKind,
        y_kind: ScaleKind,
        categories: Vec<String>,
        heights: &[f64],
        options: BarOptions,
    ) -> Result<BarMark> {
        check_bars(context, x_kind, y_kind, &categories, heights)?;
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
