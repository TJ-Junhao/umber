//! Box marks and their immutable distributions, over category x axes.

use std::sync::Arc;

use super::validation::{check_box_groups, check_distributions};
use crate::{ColorSpec, Result, ScaleKind, Transform};

/// One named distribution, with one group of samples per category.
///
/// # Examples
///
/// ```
/// use umber_core::{ColorSpec, Distribution};
/// let samples = Distribution {
///     label: "North".into(), groups: vec![vec![1.0, 2.0]], color: ColorSpec::AUTO,
/// };
/// assert_eq!(samples.groups.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Distribution {
    /// The legend label; empty for none.
    pub label: String,
    /// One group per category. NaN is ignored; an empty group draws nothing.
    pub groups: Vec<Vec<f64>>,
    /// The distribution color.
    pub color: ColorSpec,
}

impl Default for Distribution {
    fn default() -> Self {
        Self {
            label: String::new(),
            groups: Vec::new(),
            color: ColorSpec::AUTO,
        }
    }
}

/// Options of a box mark.
///
/// # Examples
///
/// ```
/// use umber_core::{BoxOptions, ColorSpec};
/// let options = BoxOptions::default();
/// assert!(options.label.is_empty());
/// assert_eq!(options.color, ColorSpec::AUTO);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BoxOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The color of the box, median, whiskers and outliers.
    pub color: ColorSpec,
}

impl Default for BoxOptions {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: ColorSpec::AUTO,
        }
    }
}

/// A validated box mark. Clones share the categories and samples.
///
/// # Examples
///
/// ```
/// use umber_core::{BoxOptions, Figure, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// ax.box_plot(vec!["A".into()], vec![vec![1.0, 2.0]], BoxOptions::default())?;
/// let Some(Mark::Box(mark)) = ax.marks().first() else { unreachable!() };
/// assert_eq!(mark.categories(), ["A"]);
/// assert_eq!(&*mark.groups()[0], &[1.0, 2.0]);
/// assert_eq!(mark.options(), &BoxOptions::default());
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// Box data can only be added through the validating axes method:
///
/// ```compile_fail,E0451
/// use umber_core::{BoxMark, BoxOptions};
/// let mark = BoxMark {
///     categories: Default::default(),
///     groups: Default::default(),
///     options: BoxOptions::default(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BoxMark {
    categories: Arc<[String]>,
    groups: Arc<[Arc<[f64]>]>,
    options: BoxOptions,
}

impl BoxMark {
    /// Validates all input before storing it, without changing its samples.
    pub(super) fn new(
        x: ScaleKind,
        y: Transform,
        categories: Vec<String>,
        groups: Vec<Vec<f64>>,
        options: BoxOptions,
    ) -> Result<Self> {
        check_box_groups("Axes.box()", x, y, &categories, &groups)?;
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
    /// use umber_core::{BoxOptions, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.box_plot(vec!["A".into()], vec![vec![1.0]], BoxOptions::default())?;
    /// let Mark::Box(mark) = &ax.marks()[0] else { unreachable!() };
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
    /// use umber_core::{BoxOptions, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.box_plot(vec!["A".into()], vec![vec![1.0]], BoxOptions::default())?;
    /// let Mark::Box(mark) = &ax.marks()[0] else { unreachable!() };
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
    /// use umber_core::{BoxOptions, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.box_plot(vec!["A".into()], vec![vec![]], BoxOptions::default())?;
    /// let Mark::Box(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert!(mark.options().label.is_empty());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &BoxOptions {
        &self.options
    }
}

/// One stored distribution; the input's owned groups become shared, immutable slices.
#[derive(Debug, Clone, PartialEq)]
struct StoredDistribution {
    label: String,
    groups: Arc<[Arc<[f64]>]>,
    color: ColorSpec,
}

/// A validated grouped box mark. Clones share categories and distributions.
///
/// # Examples
///
/// ```
/// use umber_core::{Distribution, Figure, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// let d = Distribution { label: "North".into(), groups: vec![vec![1.0, 2.0]], ..Default::default() };
/// ax.grouped_box_plot(vec!["A".into()], vec![d])?;
/// let Some(Mark::GroupedBox(mark)) = ax.marks().first() else { unreachable!() };
/// assert_eq!(mark.categories(), ["A"]);
/// let view = mark.distributions().next().unwrap();
/// assert_eq!(view.label(), "North");
/// assert_eq!(&*view.groups()[0], &[1.0, 2.0]);
/// assert_eq!(view.color(), umber_core::ColorSpec::AUTO);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// Stored distributions cannot be replaced without validation:
///
/// ```compile_fail,E0451
/// use umber_core::BoxGroupMark;
/// let mark = BoxGroupMark {
///     categories: Default::default(),
///     distributions: Default::default(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BoxGroupMark {
    categories: Arc<[String]>,
    distributions: Arc<[StoredDistribution]>,
}

impl BoxGroupMark {
    /// Validates all distributions before storing any of them.
    pub(super) fn new(
        x: ScaleKind,
        y: Transform,
        categories: Vec<String>,
        distributions: Vec<Distribution>,
    ) -> Result<Self> {
        check_distributions("Axes.grouped_box()", x, y, &categories, &distributions)?;
        let distributions: Vec<_> = distributions
            .into_iter()
            .map(|d| StoredDistribution {
                label: d.label,
                groups: store_groups(d.groups),
                color: d.color,
            })
            .collect();
        Ok(Self {
            categories: categories.into(),
            distributions: distributions.into(),
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
    /// let d = Distribution { groups: vec![vec![]], ..Default::default() };
    /// ax.grouped_box_plot(vec!["A".into()], vec![d])?;
    /// let Mark::GroupedBox(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.categories(), ["A"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    /// The distributions in input order; at least one.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Distribution, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.grouped_box_plot(vec![], vec![Distribution::default()])?;
    /// let Mark::GroupedBox(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.distributions().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn distributions(&self) -> impl ExactSizeIterator<Item = DistributionView<'_>> {
        self.distributions.iter().map(|d| DistributionView {
            label: &d.label,
            groups: &d.groups,
            color: d.color,
        })
    }
}

/// A read-only distribution of a [`BoxGroupMark`].
///
/// # Examples
///
/// ```
/// use umber_core::{Distribution, DistributionView, Figure, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// ax.grouped_box_plot(vec![], vec![Distribution::default()])?;
/// let Mark::GroupedBox(mark) = &ax.marks()[0] else { unreachable!() };
/// let view: DistributionView<'_> = mark.distributions().next().unwrap();
/// assert!(view.groups().is_empty());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DistributionView<'a> {
    label: &'a str,
    groups: &'a [Arc<[f64]>],
    color: ColorSpec,
}

impl<'a> DistributionView<'a> {
    /// The legend label; empty for none.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Distribution, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.grouped_box_plot(vec![], vec![Distribution::default()])?;
    /// let Mark::GroupedBox(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.distributions().next().unwrap().label(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn label(&self) -> &'a str {
        self.label
    }

    /// The samples in category order, including NaN as given.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Distribution, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let d = Distribution { groups: vec![vec![1.0]], ..Default::default() };
    /// ax.grouped_box_plot(vec!["A".into()], vec![d])?;
    /// let Mark::GroupedBox(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(&*mark.distributions().next().unwrap().groups()[0], &[1.0]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn groups(&self) -> &'a [Arc<[f64]>] {
        self.groups
    }

    /// The color as given, before palette resolution.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ColorSpec, Distribution, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// ax.grouped_box_plot(vec![], vec![Distribution::default()])?;
    /// let Mark::GroupedBox(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.distributions().next().unwrap().color(), ColorSpec::AUTO);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn color(&self) -> ColorSpec {
        self.color
    }
}

/// Moves each group into a shared slice, without changing its values.
fn store_groups(groups: Vec<Vec<f64>>) -> Arc<[Arc<[f64]>]> {
    groups.into_iter().map(Arc::from).collect()
}
