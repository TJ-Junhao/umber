//! Individual samples within category bands, jittered or packed without overlap.

use std::sync::Arc;

use super::box_plot::store_groups;
use super::validation::check_box_groups;
use crate::{ColorSpec, Error, Result, ScaleKind, SizeSpec, Transform};

/// Options of a strip mark.
///
/// # Examples
///
/// ```
/// use umber_core::{StripOptions, SizeSpec};
/// assert_eq!(StripOptions::default().size, SizeSpec::Auto);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StripOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The color of the circular markers.
    pub color: ColorSpec,
    /// The marker diameter; automatic uses the theme's marker size.
    pub size: SizeSpec,
    /// Maximum displacement as a fraction of the category band, from zero to one half.
    pub jitter: f64,
}

impl Default for StripOptions {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: ColorSpec::AUTO,
            size: SizeSpec::Auto,
            jitter: 0.4,
        }
    }
}

/// Validated strip samples; clones share the category names and sample groups.
///
/// # Examples
///
/// ```
/// use umber_core::{StripOptions, Figure, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// ax.strip(vec!["A".into()], vec![vec![1.0, 2.0]], StripOptions::default())?;
/// let Mark::Strip(mark) = &ax.marks()[0] else { unreachable!() };
/// assert_eq!(mark.categories(), ["A"]);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StripMark {
    categories: Arc<[String]>,
    groups: Arc<[Arc<[f64]>]>,
    options: StripOptions,
}

impl StripMark {
    /// Validates the groups and options before storage.
    pub(super) fn new(
        along: ScaleKind,
        values: Transform,
        categories: Vec<String>,
        groups: Vec<Vec<f64>>,
        options: StripOptions,
    ) -> Result<Self> {
        check_box_groups(
            "Axes.strip()",
            "strip plots",
            along,
            values,
            &categories,
            &groups,
        )?;
        if !options.jitter.is_finite() || !(0.0..=0.5).contains(&options.jitter) {
            return Err(Error::style(
                "Axes.strip()",
                format!(
                    "jitter is {}; it must be finite and between 0 and 0.5.",
                    crate::error::number(options.jitter)
                ),
            )
            .with_parameter("jitter")
            .with_hint("use 0 for a centre line, or a fraction up to 0.5 for jitter."));
        }
        Ok(Self {
            categories: categories.into(),
            groups: store_groups(groups),
            options,
        })
    }

    /// The category names in input order, including empty groups.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{StripOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// # ax.strip(vec!["A".into()], vec![vec![1.0, 2.0]], StripOptions::default())?;
    /// # let Mark::Strip(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.categories(), ["A"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    /// One group per category; NaN samples are kept and are not drawn.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{StripOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// # ax.strip(vec!["A".into()], vec![vec![1.0, 2.0]], StripOptions::default())?;
    /// # let Mark::Strip(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(&*mark.groups()[0], &[1.0, 2.0]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn groups(&self) -> &[Arc<[f64]>] {
        &self.groups
    }

    /// The drawing options as given.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{StripOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// # ax.strip(vec!["A".into()], vec![vec![1.0, 2.0]], StripOptions::default())?;
    /// # let Mark::Strip(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert!(mark.options().label.is_empty());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &StripOptions {
        &self.options
    }
}

/// Options of a swarm mark.
///
/// # Examples
///
/// ```
/// use umber_core::{SwarmOptions, SizeSpec};
/// assert_eq!(SwarmOptions::default().size, SizeSpec::Auto);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SwarmOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The color of the circular markers.
    pub color: ColorSpec,
    /// The marker diameter; automatic uses the theme's marker size.
    pub size: SizeSpec,
}

impl Default for SwarmOptions {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: ColorSpec::AUTO,
            size: SizeSpec::Auto,
        }
    }
}

/// Validated swarm samples; clones share the category names and sample groups.
///
/// # Examples
///
/// ```
/// use umber_core::{SwarmOptions, Figure, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// ax.swarm(vec!["A".into()], vec![vec![1.0, 2.0]], SwarmOptions::default())?;
/// let Mark::Swarm(mark) = &ax.marks()[0] else { unreachable!() };
/// assert_eq!(mark.categories(), ["A"]);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SwarmMark {
    categories: Arc<[String]>,
    groups: Arc<[Arc<[f64]>]>,
    options: SwarmOptions,
}

impl SwarmMark {
    /// Validates the groups and options before storage.
    pub(super) fn new(
        along: ScaleKind,
        values: Transform,
        categories: Vec<String>,
        groups: Vec<Vec<f64>>,
        options: SwarmOptions,
    ) -> Result<Self> {
        check_box_groups(
            "Axes.swarm()",
            "swarm plots",
            along,
            values,
            &categories,
            &groups,
        )?;
        Ok(Self {
            categories: categories.into(),
            groups: store_groups(groups),
            options,
        })
    }

    /// The category names in input order, including empty groups.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{SwarmOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// # ax.swarm(vec!["A".into()], vec![vec![1.0, 2.0]], SwarmOptions::default())?;
    /// # let Mark::Swarm(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(mark.categories(), ["A"]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    /// One group per category; NaN samples are kept and are not drawn.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{SwarmOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// # ax.swarm(vec!["A".into()], vec![vec![1.0, 2.0]], SwarmOptions::default())?;
    /// # let Mark::Swarm(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert_eq!(&*mark.groups()[0], &[1.0, 2.0]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn groups(&self) -> &[Arc<[f64]>] {
        &self.groups
    }

    /// The drawing options as given.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umber_core::{SwarmOptions, Figure, Mark, ScaleKind, Theme};
    /// # let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// # let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// # ax.swarm(vec!["A".into()], vec![vec![1.0, 2.0]], SwarmOptions::default())?;
    /// # let Mark::Swarm(mark) = &ax.marks()[0] else { unreachable!() };
    /// assert!(mark.options().label.is_empty());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &SwarmOptions {
        &self.options
    }
}
