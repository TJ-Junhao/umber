//! The axes: scales, texts, explicit limits, tick settings and marks, with the mutators that
//! validate every input before storing it.

use super::axis_data::{AxisData, Series};
use super::bar::{Arrangement, BarGroupMark, BarMark, BarOptions, BarSeries};
use super::hist::{HistMark, HistOptions};
use super::mark::{AreaOptions, LineOptions, Mark, ScatterOptions};
use super::ticks::{AxisTicks, ExplicitTicks};
use super::validation::{
    AREA_GAP_HINT, LINE_GAP_HINT, SCATTER_GAP_HINT, check_x_limits, check_x_tick_axis,
};
use crate::{ContinuousScale, LegendStyle, Result, ScaleKind, TickFormat, Transform};

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
    /// The y axis is always linear or log, so it is stored as its transform.
    y: Transform,
    title: String,
    subtitle: String,
    xlabel: String,
    ylabel: String,
    /// The explicit x limits, validated as a scale for the x axis.
    xlim: Option<ContinuousScale>,
    /// The explicit y limits, validated as a scale for the y axis.
    ylim: Option<ContinuousScale>,
    /// The explicit ticks and the tick format of the x axis; always unset on a category axis.
    xticks: AxisTicks,
    /// The explicit ticks and the tick format of the y axis.
    yticks: AxisTicks,
    /// The legend style set for these axes; the theme's applies when unset.
    legend: Option<LegendStyle>,
    marks: Vec<Mark>,
}

impl Axes {
    /// Empty axes with an x axis of scale kind `x` and a y axis of transform `y`.
    pub(super) fn new(x: ScaleKind, y: Transform) -> Axes {
        Axes {
            x,
            y,
            title: String::new(),
            subtitle: String::new(),
            xlabel: String::new(),
            ylabel: String::new(),
            xlim: None,
            ylim: None,
            xticks: AxisTicks::default(),
            yticks: AxisTicks::default(),
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

    /// The kind of the y scale: linear or log, never a category scale.
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
        self.y.kind()
    }

    /// The transform of the y axis.
    pub(crate) fn y_transform(&self) -> Transform {
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
        self.xlim = Some(check_x_limits(self.x, low, high)?);
        Ok(self)
    }

    /// Sets the y limits. Same rules as [`Axes::xlim`]; the y axis is never a category axis.
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
        self.ylim = Some(ContinuousScale::new(self.y, low, high, "Axes.ylim()")?);
        Ok(self)
    }

    /// Replaces the automatic x ticks with ticks at `positions`, labelled by the x tick format
    /// ([`Axes::xtick_format`]). The positions are used in the order given and never sorted;
    /// an empty list leaves the x axis without ticks and gridlines. Calling any x tick setter
    /// again replaces the ticks, and drops labels set with [`Axes::xticks_labeled`].
    ///
    /// Without explicit x limits, the x domain covers both the data and the ticks, with no
    /// rounding to round values, and is then padded as auto limits are; the lowest tick is not
    /// moved below the data. With explicit limits, ticks outside them are not drawn.
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged. `Error::Usage` on a category x axis. `Error::Style`
    /// naming the first bad index, unless every position is finite, within the supported
    /// magnitudes (positive on a log axis) and above the one before it.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ExplicitTicks, Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.xticks(&[0.0, 12.0, 24.0])?;
    /// assert_eq!(axes.x_ticks(), Some(&ExplicitTicks::Positions(vec![0.0, 12.0, 24.0])));
    /// let err = axes.xticks(&[0.0, 24.0, 12.0]).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "Axes.xticks(): positions[2] is 12 but positions[1] is 24; tick positions must be strictly increasing."
    /// );
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xticks(&mut self, positions: &[f64]) -> Result<&mut Axes> {
        const CONTEXT: &str = "Axes.xticks()";
        let transform = check_x_tick_axis(CONTEXT, self.x, "ticks")?;
        self.xticks.set_positions(CONTEXT, transform, positions)?;
        Ok(self)
    }

    /// Replaces the automatic y ticks with ticks at `positions`. Same rules as
    /// [`Axes::xticks`]; the y axis is never a category axis. Bars and histograms keep 0 in a
    /// linear y domain.
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged: `Error::Style` as for [`Axes::xticks`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, LogBase, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let log = ScaleKind::Log { base: LogBase::TEN };
    /// let axes = figure.add_axes(ScaleKind::Linear, log)?;
    /// assert!(axes.yticks(&[1.0, 3.0, 10.0, 30.0]).is_ok());
    /// assert!(axes.yticks(&[0.0, 1.0]).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn yticks(&mut self, positions: &[f64]) -> Result<&mut Axes> {
        self.yticks
            .set_positions("Axes.yticks()", self.y, positions)?;
        Ok(self)
    }

    /// Replaces the automatic x ticks with ticks at `positions`, each labelled with the label
    /// at the same index of `labels`, drawn as given. Otherwise the same as [`Axes::xticks`].
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged. They are checked in this order: `Error::Usage` on a
    /// category x axis; the positions as for [`Axes::xticks`]; `Error::Style` unless there is
    /// one label per position, each at most 1,000 characters; `Error::Usage` when the x tick
    /// format is not plain [`TickFormat::AUTO`], since given labels take no format.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme, TickFormat};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let labels = vec!["start".to_owned(), "launch".to_owned()];
    /// axes.xticks_labeled(&[0.0, 7.0], labels.clone())?;
    /// assert!(axes.xticks_labeled(&[0.0], labels.clone()).is_err());
    /// // Given labels and a format other than AUTO conflict, in either order.
    /// assert!(axes.xtick_format(TickFormat::fixed(1)?).is_err());
    /// axes.xticks(&[0.0, 7.0])?.xtick_format(TickFormat::fixed(1)?)?;
    /// assert!(axes.xticks_labeled(&[0.0, 7.0], labels).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xticks_labeled(&mut self, positions: &[f64], labels: Vec<String>) -> Result<&mut Axes> {
        const CONTEXT: &str = "Axes.xticks_labeled()";
        let transform = check_x_tick_axis(CONTEXT, self.x, "ticks")?;
        self.xticks
            .set_labeled(CONTEXT, transform, positions, labels)?;
        Ok(self)
    }

    /// Replaces the automatic y ticks with ticks at `positions`, each labelled with the label
    /// at the same index of `labels`, drawn as given. Same rules as [`Axes::xticks_labeled`];
    /// the y axis is never a category axis.
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged: as for [`Axes::xticks_labeled`], against the y tick
    /// format.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.yticks_labeled(&[0.0, 50.0, 100.0], vec!["low".into(), "mid".into(), "high".into()])?;
    /// assert!(axes.yticks_labeled(&[f64::NAN], vec!["x".into()]).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn yticks_labeled(&mut self, positions: &[f64], labels: Vec<String>) -> Result<&mut Axes> {
        self.yticks
            .set_labeled("Axes.yticks_labeled()", self.y, positions, labels)?;
        Ok(self)
    }

    /// Sets the format of the x tick labels: of the automatic ticks, and of positions set with
    /// [`Axes::xticks`]. Calling it again replaces the format; the ticks stay as they are.
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged. `Error::Usage` on a category x axis, and when the x
    /// ticks have labels given with [`Axes::xticks_labeled`] and `format` is not plain
    /// [`TickFormat::AUTO`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme, TickFormat};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.xtick_format(TickFormat::fixed(2)?.affix("", " s")?)?;
    /// assert_eq!(axes.x_tick_format().suffix(), " s");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xtick_format(&mut self, format: TickFormat) -> Result<&mut Axes> {
        const CONTEXT: &str = "Axes.xtick_format()";
        check_x_tick_axis(CONTEXT, self.x, "tick formats")?;
        self.xticks.set_format(CONTEXT, format)?;
        Ok(self)
    }

    /// Sets the format of the y tick labels. Same rules as [`Axes::xtick_format`]; the y axis
    /// is never a category axis.
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged: `Error::Usage` when the y ticks have labels given with
    /// [`Axes::yticks_labeled`] and `format` is not plain [`TickFormat::AUTO`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme, TickFormat};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.ytick_format(TickFormat::percent(0)?)?;
    /// assert_eq!(axes.y_tick_format(), &TickFormat::percent(0)?);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn ytick_format(&mut self, format: TickFormat) -> Result<&mut Axes> {
        self.yticks.set_format("Axes.ytick_format()", format)?;
        Ok(self)
    }

    /// The explicit x ticks; `None` when the x axis uses the automatic ticks.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.x_ticks(), None);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_ticks(&self) -> Option<&ExplicitTicks> {
        self.xticks.explicit()
    }

    /// The explicit y ticks; `None` when the y axis uses the automatic ticks.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ExplicitTicks, Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.yticks(&[])?;
    /// assert_eq!(axes.y_ticks(), Some(&ExplicitTicks::Positions(Vec::new())));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y_ticks(&self) -> Option<&ExplicitTicks> {
        self.yticks.explicit()
    }

    /// The format of the x tick labels; [`TickFormat::AUTO`] when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme, TickFormat};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.x_tick_format(), &TickFormat::AUTO);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x_tick_format(&self) -> &TickFormat {
        self.xticks.format()
    }

    /// The format of the y tick labels; [`TickFormat::AUTO`] when unset.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme, TickFormat};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert_eq!(axes.y_tick_format(), &TickFormat::AUTO);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y_tick_format(&self) -> &TickFormat {
        self.yticks.format()
    }

    /// The tick settings of the x axis.
    pub(crate) fn x_tick_settings(&self) -> &AxisTicks {
        &self.xticks
    }

    /// The tick settings of the y axis.
    pub(crate) fn y_tick_settings(&self) -> &AxisTicks {
        &self.yticks
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
        let series = Series::new("Axes.line()", self.x, self.y, x, y, LINE_GAP_HINT)?;
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
        let series = Series::new("Axes.scatter()", self.x, self.y, x, y, SCATTER_GAP_HINT)?;
        self.marks.push(Mark::scatter(series, options));
        Ok(self)
    }

    /// Adds an area: the line through the points, drawn as [`Axes::line`] draws it, over the
    /// region between the line and the baseline, filled in the line's color as the theme's
    /// [`FillStyle`](crate::FillStyle) paints it. The fill is drawn first and the line over it.
    /// Like bars, an area puts 0 into a linear y axis, so the filled region is proportional to
    /// the values; on a log y axis it fills down to the baseline, at the lowest y tick (or the
    /// explicit lower limit). Same rules as [`Axes::line`]: NaN breaks both the line and the
    /// fill.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AreaOptions, AxisData};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let x = AxisData::Numbers(vec![0.0, 1.0, 2.0]);
    /// axes.area(x, &[3.0, f64::NAN, 4.0], AreaOptions::default())?;
    /// assert_eq!(axes.marks().len(), 1);
    /// // An infinite value is an error, and the axes keep their one mark.
    /// assert!(axes
    ///     .area(AxisData::Numbers(vec![0.0]), &[f64::INFINITY], AreaOptions::default())
    ///     .is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn area(&mut self, x: AxisData, y: &[f64], options: AreaOptions) -> Result<&mut Axes> {
        let series = Series::new("Axes.area()", self.x, self.y, x, y, AREA_GAP_HINT)?;
        self.marks.push(Mark::area(series, options));
        Ok(self)
    }

    /// Adds bars: one bar per category, centered in the category's band, from the baseline to
    /// its height. On a linear y axis bars grow from 0 and negative heights grow downwards; on a
    /// log y axis they grow from the baseline, at the lowest y tick (or the explicit lower
    /// limit). Bar marks added one after another are drawn at the same positions, in the order
    /// they were added; categories join the x axis in order of first appearance across all
    /// marks.
    ///
    /// Errors leave the axes unchanged. They are `Error::Data`, checked in this order:
    /// `categories` and `heights` of different lengths; an x axis that is not a category axis;
    /// a category that appears twice; a height that is infinite, beyond [`MAX_MAGNITUDE`], or
    /// outside the log range on a log y axis. NaN is allowed and draws no bar.
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
    /// series' index); then the categories and each series' heights as in [`Axes::bar`]. NaN is
    /// allowed and draws no bar.
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
    /// Only the outermost segment on each side has its end rounded. On a log y axis the first
    /// segment starts at the baseline, at the lowest y tick (or the explicit lower limit).
    /// Colors and categories work as in [`Axes::grouped_bar`].
    ///
    /// Errors leave the axes unchanged. They are `Error::Data`, checked in the order of
    /// [`Axes::grouped_bar`], with one more check at the end: in each category, the positive
    /// heights and the negative heights must each add up to at most
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

    /// Adds a histogram: the finite `values` counted in `options.bins` equal-width bins from
    /// their smallest to their largest value, drawn as one bar per non-empty bin from the
    /// baseline to its count. Bins are half-open, `[low, high)`, except the last, which also
    /// holds the largest value. NaN is ignored. Values that are all the same, or that span less
    /// than a linear axis can show, are binned over `v ± max(0.5, |v| × 1e-9)` around their
    /// midpoint `v`. The values are stored as given and binned when the figure is rendered.
    ///
    /// Neighbouring bars are the theme's [`hist_gap`](crate::Theme::hist_gap) apart, at most
    /// half a bin, and have square corners. With [`FillStyle::Gradient`](crate::FillStyle), all
    /// bars share one vertical gradient from the top of the tallest bar to the baseline, and
    /// each bar gets a solid top edge. On a linear y axis the bars grow from 0; on a log y axis
    /// they grow from the baseline, at the lowest y tick (or the explicit lower limit). Auto
    /// limits put that tick below the smallest count, so every non-empty bin has a height.
    ///
    /// Errors leave the axes unchanged. They are checked in this order: `Error::Style` for
    /// `options.bins` outside `1..=`[`MAX_BINS`]; `Error::Data` for a category x axis, and
    /// `Error::Usage` for a log x axis, which histograms do not support; `Error::Data` for a
    /// value that is infinite or beyond [`MAX_MAGNITUDE`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::HistOptions;
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.hist(&[1.2, 3.4, 2.2, f64::NAN, 2.9], HistOptions { bins: 3, ..HistOptions::default() })?;
    /// assert_eq!(axes.marks().len(), 1);
    /// // No bins is an error, and the axes keep their one mark.
    /// assert!(axes.hist(&[1.0], HistOptions { bins: 0, ..HistOptions::default() }).is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    ///
    /// [`MAX_BINS`]: crate::MAX_BINS
    /// [`MAX_MAGNITUDE`]: crate::MAX_MAGNITUDE
    pub fn hist(&mut self, values: &[f64], options: HistOptions) -> Result<&mut Axes> {
        let mark = HistMark::new("Axes.hist()", self.x, values, options)?;
        self.marks.push(Mark::Hist(mark));
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
