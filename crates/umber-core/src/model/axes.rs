//! The axes: scale kinds, texts, explicit limits and marks, with the mutators that validate
//! every input before storing it.

use super::axis_data::{AxisData, Series};
use super::bar::{Arrangement, BarGroupMark, BarMark, BarOptions, BarSeries};
use super::mark::{LineOptions, Mark, ScatterOptions};
use super::validation::check_limits;
use crate::{ContinuousScale, LegendStyle, Result, ScaleKind};

/// A plotting area with its scales, texts, limits and marks.
///
/// Create one with [`Figure::add_axes`](super::Figure::add_axes).
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.xlim(0.0, 10.0)?.xlabel("time");
/// assert_eq!(axes.x_limits(), Some((0.0, 10.0)));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Axes {
    x: ScaleKind,
    y: ScaleKind,
    title: String,
    subtitle: String,
    xlabel: String,
    ylabel: String,
    /// The explicit x limits, validated as a scale for the x axis.
    xlim: Option<ContinuousScale>,
    /// The explicit y limits, validated as a scale for the y axis.
    ylim: Option<ContinuousScale>,
    /// The legend style set for these axes; the theme's applies when unset.
    legend: Option<LegendStyle>,
    marks: Vec<Mark>,
}

impl Axes {
    /// Empty axes with the given scale kinds.
    pub(super) fn new(x: ScaleKind, y: ScaleKind) -> Axes {
        Axes {
            x,
            y,
            title: String::new(),
            subtitle: String::new(),
            xlabel: String::new(),
            ylabel: String::new(),
            xlim: None,
            ylim: None,
            legend: None,
            marks: Vec::new(),
        }
    }

    /// The kind of the x scale.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// assert_eq!(axes.x_scale(), ScaleKind::Category);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_scale(&self) -> ScaleKind {
        self.x
    }

    /// The kind of the y scale.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.y_scale(), ScaleKind::Linear);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y_scale(&self) -> ScaleKind {
        self.y
    }

    /// Sets the title.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.title("Revenue");
    /// assert_eq!(axes.title_text(), "Revenue");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn title(&mut self, text: &str) -> &mut Axes {
        self.title = text.to_owned();
        self
    }

    /// Sets the subtitle.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.subtitle("2026");
    /// assert_eq!(axes.subtitle_text(), "2026");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn subtitle(&mut self, text: &str) -> &mut Axes {
        self.subtitle = text.to_owned();
        self
    }

    /// Sets the x axis label.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.xlabel("time");
    /// assert_eq!(axes.xlabel_text(), "time");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xlabel(&mut self, text: &str) -> &mut Axes {
        self.xlabel = text.to_owned();
        self
    }

    /// Sets the y axis label.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.ylabel("USD");
    /// assert_eq!(axes.ylabel_text(), "USD");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn ylabel(&mut self, text: &str) -> &mut Axes {
        self.ylabel = text.to_owned();
        self
    }

    /// The title text; empty when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.title_text(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn title_text(&self) -> &str {
        &self.title
    }

    /// The subtitle text; empty when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.subtitle_text(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn subtitle_text(&self) -> &str {
        &self.subtitle
    }

    /// The x label text; empty when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.xlabel_text(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn xlabel_text(&self) -> &str {
        &self.xlabel
    }

    /// The y label text; empty when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.ylabel_text(), "");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn ylabel_text(&self) -> &str {
        &self.ylabel
    }

    /// The explicit x limits, if set.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.x_limits(), None);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_limits(&self) -> Option<(f64, f64)> {
        self.xlim.map(|scale| scale.domain())
    }

    /// The explicit y limits, if set.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.ylim(0.0, 5.0)?;
    /// assert_eq!(axes.y_limits(), Some((0.0, 5.0)));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y_limits(&self) -> Option<(f64, f64)> {
        self.ylim.map(|scale| scale.domain())
    }

    /// The x scale that the explicit x limits define, if set.
    pub(crate) fn x_limit_scale(&self) -> Option<ContinuousScale> {
        self.xlim
    }

    /// The y scale that the explicit y limits define, if set.
    pub(crate) fn y_limit_scale(&self) -> Option<ContinuousScale> {
        self.ylim
    }

    /// Sets the x limits. `Error::Usage` on a category axis; `Error::Style` unless
    /// `low < high`, both are finite and within the supported magnitudes (and positive on a
    /// log axis), and the range is wide enough: on a linear axis at least
    /// `max(1e-10 * max(|low|, |high|), 1e-290)`, on a log axis a factor of `1 + 1e-9`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert!(axes.xlim(0.0, 1.0).is_ok());
    /// assert!(axes.xlim(1.0, 1.0).is_err());
    /// assert!(axes.xlim(1.0, 1.0 + 1e-12).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xlim(&mut self, low: f64, high: f64) -> Result<&mut Axes> {
        self.xlim = Some(check_limits("Axes.xlim()", self.x, low, high)?);
        Ok(self)
    }

    /// Sets the y limits. Same rules as [`Axes::xlim`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert!(axes.ylim(-1.0, 1.0).is_ok());
    /// assert!(axes.ylim(f64::NAN, 1.0).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn ylim(&mut self, low: f64, high: f64) -> Result<&mut Axes> {
        self.ylim = Some(check_limits("Axes.ylim()", self.y, low, high)?);
        Ok(self)
    }

    /// Sets the legend style of these axes, overriding the theme's style.
    /// [`LegendStyle::None`] hides the legend. Calling it again replaces the style.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, LegendStyle, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.legend(LegendStyle::Chips).legend(LegendStyle::None);
    /// assert_eq!(axes.legend_style(), Some(LegendStyle::None));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn legend(&mut self, style: LegendStyle) -> &mut Axes {
        self.legend = Some(style);
        self
    }

    /// The legend style set with [`Axes::legend`]; `None` when unset, in which case the
    /// theme's style applies.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.legend_style(), None);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn legend_style(&self) -> Option<LegendStyle> {
        self.legend
    }

    /// Adds a line. Errors leave the axes unchanged: `Error::Data` for mismatched lengths, an
    /// infinite value, a magnitude beyond [`MAX_MAGNITUDE`], a value outside the log range on a
    /// log axis, or `x` of the wrong kind for the axis. NaN is allowed and breaks the line.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, LineOptions};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.line(AxisData::Numbers(vec![0.0, 1.0]), &[1.0, f64::NAN], LineOptions::default())?;
    /// assert!(axes
    ///     .line(AxisData::Numbers(vec![0.0]), &[1.0, 2.0], LineOptions::default())
    ///     .is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    ///
    /// [`MAX_MAGNITUDE`]: crate::MAX_MAGNITUDE
    pub fn line(&mut self, x: AxisData, y: &[f64], options: LineOptions) -> Result<&mut Axes> {
        let series = Series::new("Axes.line()", self.x, self.y, x, y)?;
        self.marks.push(Mark::line(series, options));
        Ok(self)
    }

    /// Adds a scatter series. Same rules as [`Axes::line`]; NaN skips a point.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, ScatterOptions};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.scatter(AxisData::Numbers(vec![0.0, 1.0]), &[1.0, 2.0], ScatterOptions::default())?;
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn scatter(
        &mut self,
        x: AxisData,
        y: &[f64],
        options: ScatterOptions,
    ) -> Result<&mut Axes> {
        let series = Series::new("Axes.scatter()", self.x, self.y, x, y)?;
        self.marks.push(Mark::scatter(series, options));
        Ok(self)
    }

    /// Adds bars: one bar per category, centered in the category's band, from the baseline to
    /// its height. On a linear y axis bars grow from 0 and negative heights grow downwards; on a
    /// log y axis they grow from the bottom of the plot. Bar marks added one after another are
    /// drawn at the same positions, in the order they were added; categories join the x axis
    /// in order of first appearance across all marks.
    ///
    /// Errors leave the axes unchanged. They are `Error::Data`, checked in this order:
    /// `categories` and `heights` of different lengths; an x axis that is not a category axis;
    /// a category that appears twice; a height that is infinite, beyond [`MAX_MAGNITUDE`], or
    /// outside the log range on a log y axis; a category y axis. NaN is allowed and draws no
    /// bar.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::BarOptions;
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// axes.bar(vec!["a".into(), "b".into()], &[3.0, f64::NAN], BarOptions::default())?;
    /// assert_eq!(axes.marks().len(), 1);
    /// // A repeated category is an error, and the axes keep their one mark.
    /// assert!(axes.bar(vec!["a".into(), "a".into()], &[1.0, 2.0], BarOptions::default()).is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    ///
    /// [`MAX_MAGNITUDE`]: crate::MAX_MAGNITUDE
    pub fn bar(
        &mut self,
        categories: Vec<String>,
        heights: &[f64],
        options: BarOptions,
    ) -> Result<&mut Axes> {
        let mark = BarMark::new("Axes.bar()", self.x, self.y, categories, heights, options)?;
        self.marks.push(Mark::Bar(mark));
        Ok(self)
    }

    /// Adds grouped bars: in each category, one bar per series, side by side in series order.
    /// The group takes 72 % of the category's band, centered in it, with a 2 pt gap between
    /// its bars; when the bars would be narrower than the gap, they touch. Each bar grows from
    /// the baseline as in [`Axes::bar`]. Each series takes its own color; `Auto` colors take
    /// palette slots in series order. Categories join the x axis in order of first appearance
    /// across all marks.
    ///
    /// Errors leave the axes unchanged. They are `Error::Data`, checked in this order: an empty
    /// `series`; a series whose heights are not one per category (the message names the
    /// series' index); then the categories and each series' heights as in [`Axes::bar`]; a
    /// category y axis. NaN is allowed and draws no bar.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::BarSeries;
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
    ///     vec!["2025".into(), "2026".into()],
    ///     vec![series("north", &[3.0, 4.0]), series("south", &[2.5, f64::NAN])],
    /// )?;
    /// assert_eq!(axes.marks().len(), 1);
    /// // A series with a height missing is an error, and the axes keep their one mark.
    /// assert!(axes.grouped_bar(vec!["a".into()], vec![series("x", &[])]).is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn grouped_bar(
        &mut self,
        categories: Vec<String>,
        series: Vec<BarSeries>,
    ) -> Result<&mut Axes> {
        let mark = BarGroupMark::new(
            "Axes.grouped_bar()",
            Arrangement::Grouped,
            self.x,
            self.y,
            categories,
            series,
        )?;
        self.marks.push(Mark::GroupedBar(mark));
        Ok(self)
    }

    /// Adds stacked bars: in each category, one stack, as wide as a bar of [`Axes::bar`], with
    /// the series stacked in series order. Positive heights stack upward from the baseline and
    /// negative ones downward, each side separately; NaN and 0 add no segment. Neighbouring
    /// segments on one side are 1 pt apart, and a segment too thin for that gap is not drawn.
    /// Only the outermost segment on each side has its end rounded. On a log y axis the first segment starts at
    /// the bottom of the plot. Colors and categories work as in [`Axes::grouped_bar`].
    ///
    /// Errors leave the axes unchanged. They are `Error::Data`, checked in the order of
    /// [`Axes::grouped_bar`], with one more check before the y axis kind: in each category, the
    /// positive heights and the negative heights must each add up to at most
    /// [`MAX_MAGNITUDE`] in magnitude (the message names the category).
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::BarSeries;
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = |heights: &[f64]| BarSeries { heights: heights.to_vec(), ..BarSeries::default() };
    /// axes.stacked_bar(vec!["a".into(), "b".into()], vec![series(&[3.0, -1.0]), series(&[2.0, 4.0])])?;
    /// assert_eq!(axes.marks().len(), 1);
    /// // Totals beyond the supported magnitudes are an error.
    /// assert!(axes.stacked_bar(vec!["a".into()], vec![series(&[1e300]), series(&[1e300])]).is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    ///
    /// [`MAX_MAGNITUDE`]: crate::MAX_MAGNITUDE
    pub fn stacked_bar(
        &mut self,
        categories: Vec<String>,
        series: Vec<BarSeries>,
    ) -> Result<&mut Axes> {
        let mark = BarGroupMark::new(
            "Axes.stacked_bar()",
            Arrangement::Stacked,
            self.x,
            self.y,
            categories,
            series,
        )?;
        self.marks.push(Mark::StackedBar(mark));
        Ok(self)
    }

    /// The marks, in the order they were added.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert!(axes.marks().is_empty());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }
}
