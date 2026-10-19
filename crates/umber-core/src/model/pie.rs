//! Pie data and the restricted mutable handle of a circular panel.

use std::sync::Arc;

use super::{CellRange, Figure};
use crate::{ColorSpec, Error, LegendPosition, LegendStyle, Result, Rgba, SizeSpec, TickFormat};

const CONTEXT: &str = "PieAxes.pie()";

/// Options of a pie or donut. Colors, when supplied, have one entry per category.
///
/// # Examples
///
/// ```
/// use umber_core::PieOptions;
/// let donut = PieOptions { hole: 0.5, annotate: true, ..Default::default() };
/// assert_eq!(donut.hole, 0.5);
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PieOptions {
    /// Inner radius divided by outer radius, finite and in `0..1`.
    pub hole: f64,
    /// Whether to write the share beside each slice name.
    pub annotate: bool,
    /// Share format; unset means percent with no decimal places.
    pub format: Option<TickFormat>,
    /// One color per category; unset uses the palette without cycling.
    pub colors: Option<Vec<ColorSpec>>,
    /// The direct labels' body text size.
    pub size: SizeSpec,
}

/// A validated slice, including zero shares retained for the legend.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Slice {
    pub name: String,
    pub share: f64,
    pub color: Rgba,
}

/// Immutable pie data; copies of a figure share the slices.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pie {
    pub slices: Arc<[Slice]>,
    pub options: PieOptions,
}

/// A circular panel's cells, title, legend and optional pie, without Cartesian axis state.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Panel, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// f.add_pie_axes()?.title("Parts");
/// if let Some(Panel::Pie(pie)) = f.panels().next() {
///     assert_eq!(pie.title_text(), "Parts");
/// }
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PiePanel {
    pub(crate) cells: CellRange,
    pub(crate) title: String,
    pub(crate) title_size: SizeSpec,
    pub(crate) legend: LegendStyle,
    pub(crate) legend_position: Option<LegendPosition>,
    pub(crate) legend_size: SizeSpec,
    pub(crate) pie: Option<Pie>,
}

impl PiePanel {
    pub(super) fn new(cells: CellRange) -> Self {
        Self {
            cells,
            title: String::new(),
            title_size: SizeSpec::Auto,
            legend: LegendStyle::Direct,
            legend_position: None,
            legend_size: SizeSpec::Auto,
            pie: None,
        }
    }

    /// The cells occupied by this circular panel.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Panel, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// f.add_pie_axes()?;
    /// if let Some(Panel::Pie(pie)) = f.panels().next() {
    ///     assert_eq!(pie.cells().row(), 0);
    /// }
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn cells(&self) -> CellRange {
        self.cells
    }

    /// The title, or an empty string when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Panel, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// f.add_pie_axes()?.title("Parts");
    /// if let Some(Panel::Pie(pie)) = f.panels().next() {
    ///     assert_eq!(pie.title_text(), "Parts");
    /// }
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn title_text(&self) -> &str {
        &self.title
    }
}

/// A pie panel's mutable handle. It has no Cartesian marks or axis settings.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, PieOptions, Theme};
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// figure.add_pie_axes()?.pie(vec!["A".into(), "B".into()], &[3.0, 1.0], PieOptions::default())?;
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug)]
pub struct PieAxesMut<'a> {
    figure: &'a mut Figure,
    index: usize,
}

impl<'a> PieAxesMut<'a> {
    pub(super) fn new(figure: &'a mut Figure, index: usize) -> Self {
        Self { figure, index }
    }

    /// The cells occupied by this panel.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert_eq!(f.add_pie_axes()?.cells().row(), 0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn cells(&self) -> CellRange {
        self.figure.pie_panel(self.index).cells()
    }

    /// Adds the panel's only pie. Invalid data or options leave the panel unchanged.
    ///
    /// # Errors
    ///
    /// `Error::Data` for empty, mismatched, negative, nonfinite or all-zero values, or repeated
    /// names. `Error::Style` for an invalid hole or too few colors. `Error::Usage` for a second pie.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, PieOptions, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// f.add_pie_axes()?.pie(vec!["A".into()], &[1.0], PieOptions::default())?;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn pie(
        &mut self,
        categories: Vec<String>,
        values: &[f64],
        options: PieOptions,
    ) -> Result<&mut Self> {
        if self.figure.pie_panel(self.index).pie.is_some() {
            return Err(Error::usage(
                CONTEXT,
                "this panel already holds a pie; each pie needs its own panel.",
            )
            .with_hint("add a new panel with Figure.add_pie_axes()."));
        }
        let pie = Pie::new(categories, values, options, self.figure.theme().palette())?;
        self.figure.pie_panel_mut(self.index).pie = Some(pie);
        Ok(self)
    }

    /// Sets the panel title at the theme's title size.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// f.add_pie_axes()?.title("Orders");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn title(&mut self, text: &str) -> &mut Self {
        self.title_sized(text, SizeSpec::Auto)
    }

    /// Sets the title and its per-call size.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, SizeSpec, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// f.add_pie_axes()?.title_sized("Orders", SizeSpec::Auto);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn title_sized(&mut self, text: &str, size: SizeSpec) -> &mut Self {
        let axes = self.figure.pie_panel_mut(self.index);
        axes.title = text.into();
        axes.title_size = size;
        self
    }

    /// Chooses direct names, a panel legend, or no names. Pie legends never merge.
    /// An explicit position is incompatible with direct labels.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, LegendStyle, SizeSpec, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// f.add_pie_axes()?.legend(LegendStyle::Header, None, SizeSpec::Auto)?;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn legend(
        &mut self,
        style: LegendStyle,
        position: Option<LegendPosition>,
        size: SizeSpec,
    ) -> Result<&mut Self> {
        if style == LegendStyle::Direct && position.is_some() {
            return Err(Error::usage(
                "PieAxes.legend()",
                "DIRECT labels follow their slices; a legend position is not applicable.",
            )
            .with_parameter("position")
            .with_hint("use AUTO with DIRECT, or use HEADER or CHIPS for a positioned legend."));
        }
        let axes = self.figure.pie_panel_mut(self.index);
        axes.legend = style;
        axes.legend_position = position;
        axes.legend_size = size;
        Ok(self)
    }
}

impl Pie {
    fn new(
        categories: Vec<String>,
        values: &[f64],
        options: PieOptions,
        palette: &[Rgba],
    ) -> Result<Self> {
        if !options.hole.is_finite() || !(0.0..1.0).contains(&options.hole) {
            return Err(Error::style(
                CONTEXT,
                format!(
                    "hole is {}; it must be finite and satisfy 0 <= hole < 1.",
                    options.hole
                ),
            )
            .with_parameter("hole")
            .with_hint("use 0 for a pie, or a fraction such as 0.5 for a donut."));
        }
        if values.is_empty() || categories.len() != values.len() {
            return Err(Error::data(CONTEXT, format!("categories has {} entries and values has {}; both must have the same nonzero length.", categories.len(), values.len()))
                .with_parameter("values").with_hint("pass one nonnegative value per category, with at least one positive value."));
        }
        let mut seen = std::collections::HashSet::new();
        for (i, name) in categories.iter().enumerate() {
            if name.is_empty() {
                return Err(Error::data(
                    CONTEXT,
                    format!("categories[{i}] is empty; every slice needs a non-empty name."),
                )
                .with_parameter("categories")
                .with_hint("give every slice a non-empty name."));
            }
            super::check_text(CONTEXT, "categories", "category", name)?;
            if !seen.insert(name) {
                return Err(Error::data(
                    CONTEXT,
                    format!("categories[{i}] repeats {name:?}; each category must appear once."),
                )
                .with_parameter("categories")
                .with_hint("combine values for repeated categories first."));
            }
        }
        let mut largest = 0.0_f64;
        for (i, &value) in values.iter().enumerate() {
            if !value.is_finite() || value < 0.0 {
                return Err(Error::data(
                    CONTEXT,
                    format!("values[{i}] is {value}; pie values must be finite and nonnegative."),
                )
                .with_parameter("values")
                .with_hint("remove missing values and pass nonnegative finite values."));
            }
            largest = largest.max(value);
        }
        if largest == 0.0 {
            return Err(Error::data(
                CONTEXT,
                "all values are zero; a pie needs a positive total.",
            )
            .with_parameter("values")
            .with_hint("pass at least one positive value."));
        }
        let colors = if let Some(colors) = &options.colors {
            if colors.len() != values.len() {
                return Err(Error::style(
                    CONTEXT,
                    format!(
                        "colors has {} entries for {} slices; pass one color per slice.",
                        colors.len(),
                        values.len()
                    ),
                )
                .with_parameter("colors")
                .with_hint("pass one Color per category, or use AUTO for the theme palette."));
            }
            colors.clone()
        } else {
            vec![ColorSpec::AUTO; values.len()]
        };
        if colors
            .iter()
            .enumerate()
            .any(|(i, c)| matches!(c, ColorSpec::Auto(_)) && i >= palette.len())
        {
            return Err(Error::style(
                CONTEXT,
                format!(
                    "{} slices need colors but the palette has {}; pie colors do not cycle.",
                    values.len(),
                    palette.len()
                ),
            )
            .with_parameter("colors")
            .with_hint("merge small slices into Other, or pass explicit colors for every slice."));
        }
        // Scaling before summation avoids overflow even when finite values approach f64::MAX.
        let total: f64 = values.iter().map(|v| v / largest).sum();
        let slices = categories
            .into_iter()
            .zip(values)
            .zip(colors)
            .enumerate()
            .map(|(i, ((name, value), color))| Slice {
                name,
                share: (value / largest) / total,
                color: color.resolve(palette, i),
            })
            .collect::<Vec<_>>()
            .into();
        Ok(Self { slices, options })
    }
}
