//! Violin marks over category axes, with shared immutable samples.

use std::sync::Arc;

use super::box_plot::{BoxGroupMark, Distribution, DistributionView, store_groups};
use super::validation::check_box_groups;
use crate::{ColorSpec, Result, ScaleKind, Transform};

/// Options of a violin mark.
///
/// # Examples
///
/// ```
/// use umber_core::{ViolinOptions, ColorSpec};
/// let options = ViolinOptions::default();
/// assert!(options.label.is_empty());
/// assert_eq!(options.color, ColorSpec::AUTO);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ViolinOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The color of the violin and its quantile lines.
    pub color: ColorSpec,
}

impl Default for ViolinOptions {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: ColorSpec::AUTO,
        }
    }
}

/// A validated violin mark. Clones share the categories and samples.
///
/// # Examples
///
/// ```
/// use umber_core::{ViolinOptions, Figure, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// ax.violin(vec!["A".into()], vec![vec![1.0, 2.0]], ViolinOptions::default())?;
/// let Some(Mark::Violin(mark)) = ax.marks().first() else { unreachable!() };
/// assert_eq!(mark.categories(), ["A"]);
/// assert_eq!(&*mark.groups()[0], &[1.0, 2.0]);
/// assert_eq!(mark.options(), &ViolinOptions::default());
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// Violin data can only be added through the validating axes method:
///
/// ```compile_fail,E0451
/// use umber_core::{ViolinMark, ViolinOptions};
/// let mark = ViolinMark {
///     categories: Default::default(),
///     groups: Default::default(),
///     options: ViolinOptions::default(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ViolinMark {
    categories: Arc<[String]>,
    groups: Arc<[Arc<[f64]>]>,
    options: ViolinOptions,
}

impl ViolinMark {
    /// Validates all input before storing it, without changing its samples.
    pub(super) fn new(
        x: ScaleKind,
        y: Transform,
        categories: Vec<String>,
        groups: Vec<Vec<f64>>,
        options: ViolinOptions,
    ) -> Result<Self> {
        check_box_groups("Axes.violin()", "violins", x, y, &categories, &groups)?;
        Ok(Self {
            categories: categories.into(),
            groups: store_groups(groups),
            options,
        })
    }

    /// The category names, each appearing once.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ViolinOptions, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.violin(vec!["A".into()], vec![vec![1.0]], ViolinOptions::default())?;
    /// let Mark::Violin(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.categories(), ["A"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    /// The samples in category order, including NaN as given. Clones share each group.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ViolinOptions, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.violin(vec!["A".into()], vec![vec![1.0]], ViolinOptions::default())?;
    /// let Mark::Violin(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(&*mark.groups()[0], &[1.0]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn groups(&self) -> &[Arc<[f64]>] {
        &self.groups
    }

    /// The label and color as given.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ViolinOptions, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.violin(vec!["A".into()], vec![vec![]], ViolinOptions::default())?;
    /// let Mark::Violin(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert!(mark.options().label.is_empty());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &ViolinOptions {
        &self.options
    }
}

/// Distributions drawn as violins side by side within each category.
///
/// # Examples
///
/// ```
/// use umber_core::{Distribution, Figure, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// ax.grouped_violin(vec![], vec![Distribution::default()])?;
/// let Mark::GroupedViolin(mark) = &ax.marks()[0] else { unreachable!() };
/// assert_eq!(mark.distributions().len(), 1);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ViolinGroupMark {
    data: BoxGroupMark,
}

impl ViolinGroupMark {
    /// Validates every distribution before storing any of them.
    pub(super) fn new(
        x: ScaleKind,
        y: Transform,
        categories: Vec<String>,
        distributions: Vec<Distribution>,
    ) -> Result<Self> {
        Ok(Self {
            data: BoxGroupMark::with_context(
                "Axes.grouped_violin()",
                "violins",
                x,
                y,
                categories,
                distributions,
            )?,
        })
    }

    /// The category names, each appearing once.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Distribution, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.grouped_violin(vec!["A".into()], vec![Distribution { groups: vec![vec![]], ..Default::default() }])?;
    /// let Mark::GroupedViolin(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.categories(), ["A"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn categories(&self) -> &[String] {
        self.data.categories()
    }

    /// The distributions in input order; at least one. Clones share their samples.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Distribution, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.grouped_violin(vec![], vec![Distribution::default()])?;
    /// let Mark::GroupedViolin(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.distributions().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn distributions(&self) -> impl ExactSizeIterator<Item = DistributionView<'_>> {
        self.data.distributions()
    }
}
