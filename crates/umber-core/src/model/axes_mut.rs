//! The handle through which an axes on a figure is changed: its text, limit and tick setters
//! and its mark methods.

use std::ops::Deref;
use std::sync::Arc;

use super::axes::Axes;
use super::axis_data::{AxisData, XySeries};
use super::band::{BandMark, BandOptions};
use super::bar::{Arrangement, BarGroupMark, BarMark, BarOptions, Series};
use super::box_plot::{BoxGroupMark, BoxMark, BoxOptions, Distribution};
use super::color_by::{check_point_color, check_scatter_label};
use super::error_bars::{ErrorBarMark, ErrorBarOptions, ErrorBarOrientation};
use super::figure::Figure;
use super::hist::{HistMark, HistOptions};
use super::mark::{AreaOptions, LineOptions, Mark, ScatterOptions};
use super::rule::{AxisValue, HLineMark, RuleOptions, SpanMark, SpanOptions, VLineMark};
use super::share::{Setting, SharedAxis};
use super::stacked_area::StackedAreaMark;
use super::stem::{StemMark, StemOptions};
use super::step::{StepMark, StepOptions};
use super::validation::{
    AREA_GAP_HINT, LINE_GAP_HINT, REFERENCE_LINE, SCATTER_GAP_HINT, check_band, check_limits,
    check_numeric_y, check_reference_value, check_span, check_step_order, check_tick_axis,
    check_vline_x, check_x_error_bars, check_y_error_bars,
};
use super::validation::{check_band_axes, check_not_two_categories};
use super::violin::{ViolinGroupMark, ViolinMark, ViolinOptions};
use crate::{Error, LegendPosition, LegendStyle, Result, TickFormat};

/// One axes of a figure, held for changing: the setters of its texts, limits, ticks and legend
/// style, and its mark methods. It borrows the whole figure, and dereferences to the [`Axes`]
/// for its getters.
///
/// [`Figure::add_axes`], [`Figure::add_axes_at`] and [`Figure::axes_at_mut`] return one.
/// Every setter checks its input before it stores anything, so a failed call leaves the figure
/// unchanged. Setters return the handle, so calls chain.
///
/// Most mark methods need a linear or log y axis: on a category y axis they return
/// `Error::Usage`, naming `y`, before any other check. The bar, box and violin families,
/// [`scatter`](AxesMut::scatter), [`x_error_bars`](AxesMut::x_error_bars),
/// [`vline`](AxesMut::vline) and [`vspan`](AxesMut::vspan) also draw on a category y axis
/// beside a linear or log x axis: bars, boxes and violins then lie along y and grow along x,
/// and a scatter is a dot plot. On two category axes these return `Error::Usage`, naming `x`,
/// before any other check.
///
/// Limits, explicit ticks and tick formats apply to the whole shared group. A member may
/// replace its own setting, but a different value already set by another member is an
/// `Error::Usage`. Setting the same value again is allowed. Members added later inherit the
/// group's settings; text, legend and mark methods always apply to this axes alone.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, LineOptions};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.title("Growth")
///     .xlim(0.0, 10.0)?
///     .line(AxisData::Numbers(vec![0.0, 10.0]), &[1.0, 2.0], LineOptions::default())?;
/// assert_eq!((axes.title_text(), axes.marks().len()), ("Growth", 1));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug)]
pub struct AxesMut<'a> {
    /// The figure that holds the axes.
    figure: &'a mut Figure,
    /// The index of the axes among the figure's panels; valid for as long as the handle lives,
    /// since the handle holds the only borrow of the figure.
    index: usize,
}

impl Deref for AxesMut<'_> {
    type Target = Axes;

    fn deref(&self) -> &Axes {
        self.figure.panel(self.index)
    }
}

impl<'a> AxesMut<'a> {
    /// The handle of the axes at `index` among the panels of `figure`, which must hold it.
    pub(super) fn new(figure: &'a mut Figure, index: usize) -> AxesMut<'a> {
        AxesMut { figure, index }
    }

    /// The axes, for a setter to change.
    fn axes_mut(&mut self) -> &mut Axes {
        self.figure.panel_mut(self.index)
    }

    /// Sets the title.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.title("Revenue");
    /// assert_eq!(axes.title_text(), "Revenue");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn title(&mut self, text: &str) -> &mut Self {
        let axes = self.axes_mut();
        axes.title = text.to_owned();
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
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.subtitle("2026");
    /// assert_eq!(axes.subtitle_text(), "2026");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn subtitle(&mut self, text: &str) -> &mut Self {
        let axes = self.axes_mut();
        axes.subtitle = text.to_owned();
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
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.xlabel("time");
    /// assert_eq!(axes.xlabel_text(), "time");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xlabel(&mut self, text: &str) -> &mut Self {
        let axes = self.axes_mut();
        axes.xlabel = text.to_owned();
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
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.ylabel("USD");
    /// assert_eq!(axes.ylabel_text(), "USD");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn ylabel(&mut self, text: &str) -> &mut Self {
        let axes = self.axes_mut();
        axes.ylabel = text.to_owned();
        self
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
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert!(axes.xlim(0.0, 1.0).is_ok());
    /// assert!(axes.xlim(1.0, 1.0).is_err());
    /// assert!(axes.xlim(1.0, 1.0 + 1e-12).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xlim(&mut self, low: f64, high: f64) -> Result<&mut Self> {
        let mut settings = (*self.x_settings).clone();
        settings.limits = Some(check_limits(
            "Axes.xlim()",
            SharedAxis::X,
            self.x,
            (low, high),
        )?);
        self.figure.set_group(
            self.index,
            SharedAxis::X,
            Setting::Limits,
            settings,
            "Axes.xlim()",
        )?;
        Ok(self)
    }

    /// Sets the y limits. Same rules as [`AxesMut::xlim`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert!(axes.ylim(-1.0, 1.0).is_ok());
    /// assert!(axes.ylim(f64::NAN, 1.0).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn ylim(&mut self, low: f64, high: f64) -> Result<&mut Self> {
        let mut settings = (*self.y_settings).clone();
        settings.limits = Some(check_limits(
            "Axes.ylim()",
            SharedAxis::Y,
            self.y,
            (low, high),
        )?);
        self.figure.set_group(
            self.index,
            SharedAxis::Y,
            Setting::Limits,
            settings,
            "Axes.ylim()",
        )?;
        Ok(self)
    }

    /// Replaces the automatic x ticks with ticks at `positions`, labelled by the x tick format
    /// ([`AxesMut::xtick_format`]). The positions are used in the order given and never sorted;
    /// an empty list leaves the x axis without ticks and gridlines. Calling any x tick setter
    /// again replaces the ticks, and drops labels set with [`AxesMut::xticks_labeled`].
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
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.xticks(&[0.0, 12.0, 24.0])?;
    /// assert_eq!(axes.x_ticks(), Some(&ExplicitTicks::Positions(vec![0.0, 12.0, 24.0])));
    /// let err = axes.xticks(&[0.0, 24.0, 12.0]).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "Axes.xticks(): positions[2] is 12 but positions[1] is 24; tick positions must be strictly increasing."
    /// );
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xticks(&mut self, positions: &[f64]) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.xticks()";
        let transform = check_tick_axis(CONTEXT, SharedAxis::X, self.x, "ticks")?;
        let mut settings = (*self.x_settings).clone();
        settings
            .ticks
            .set_positions(CONTEXT, transform, positions)?;
        self.figure
            .set_group(self.index, SharedAxis::X, Setting::Ticks, settings, CONTEXT)?;
        Ok(self)
    }

    /// Replaces the automatic y ticks with ticks at `positions`. Same rules as
    /// [`AxesMut::xticks`]. Bars and histograms keep 0 in a linear y domain.
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged: as for [`AxesMut::xticks`], on the y axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, LogBase, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let log = ScaleKind::Log { base: LogBase::TEN };
    /// let mut axes = figure.add_axes(ScaleKind::Linear, log)?;
    /// assert!(axes.yticks(&[1.0, 3.0, 10.0, 30.0]).is_ok());
    /// assert!(axes.yticks(&[0.0, 1.0]).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn yticks(&mut self, positions: &[f64]) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.yticks()";
        let transform = check_tick_axis(CONTEXT, SharedAxis::Y, self.y, "ticks")?;
        let mut settings = (*self.y_settings).clone();
        settings
            .ticks
            .set_positions(CONTEXT, transform, positions)?;
        self.figure
            .set_group(self.index, SharedAxis::Y, Setting::Ticks, settings, CONTEXT)?;
        Ok(self)
    }

    /// Replaces the automatic x ticks with ticks at `positions`, each labelled with the label
    /// at the same index of `labels`, drawn as given. Otherwise the same as [`AxesMut::xticks`].
    /// Labels too wide to stay inside the figure make rendering fail (see
    /// [`Figure::to_scene`](crate::Figure::to_scene)).
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged. They are checked in this order: `Error::Usage` on a
    /// category x axis; the positions as for [`AxesMut::xticks`]; `Error::Style` unless there is
    /// one label per position, each at most 1,000 characters; `Error::Usage` when the x tick
    /// format is not plain [`TickFormat::AUTO`], since given labels take no format.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme, TickFormat};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let labels = vec!["start".to_owned(), "launch".to_owned()];
    /// axes.xticks_labeled(&[0.0, 7.0], labels.clone())?;
    /// assert!(axes.xticks_labeled(&[0.0], labels.clone()).is_err());
    /// // Given labels and a format other than AUTO conflict, in either order.
    /// assert!(axes.xtick_format(TickFormat::fixed(1)?).is_err());
    /// axes.xticks(&[0.0, 7.0])?.xtick_format(TickFormat::fixed(1)?)?;
    /// assert!(axes.xticks_labeled(&[0.0, 7.0], labels).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xticks_labeled(&mut self, positions: &[f64], labels: Vec<String>) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.xticks_labeled()";
        let transform = check_tick_axis(CONTEXT, SharedAxis::X, self.x, "ticks")?;
        let mut settings = (*self.x_settings).clone();
        settings
            .ticks
            .set_labeled(CONTEXT, transform, positions, labels)?;
        self.figure
            .set_group(self.index, SharedAxis::X, Setting::Ticks, settings, CONTEXT)?;
        Ok(self)
    }

    /// Replaces the automatic y ticks with ticks at `positions`, each labelled with the label
    /// at the same index of `labels`, drawn as given. Same rules as [`AxesMut::xticks_labeled`].
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged: as for [`AxesMut::xticks_labeled`], on the y axis and
    /// against the y tick format.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.yticks_labeled(&[0.0, 50.0, 100.0], vec!["low".into(), "mid".into(), "high".into()])?;
    /// assert!(axes.yticks_labeled(&[f64::NAN], vec!["x".into()]).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn yticks_labeled(&mut self, positions: &[f64], labels: Vec<String>) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.yticks_labeled()";
        let transform = check_tick_axis(CONTEXT, SharedAxis::Y, self.y, "ticks")?;
        let mut settings = (*self.y_settings).clone();
        settings
            .ticks
            .set_labeled(CONTEXT, transform, positions, labels)?;
        self.figure
            .set_group(self.index, SharedAxis::Y, Setting::Ticks, settings, CONTEXT)?;
        Ok(self)
    }

    /// Sets the format of the x tick labels: of the automatic ticks, and of positions set with
    /// [`AxesMut::xticks`]. Calling it again replaces the format; the ticks stay as they are.
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged. `Error::Usage` on a category x axis, for
    /// [`TickFormat::iso`] on a linear or log axis, and when the x ticks have labels given with
    /// [`AxesMut::xticks_labeled`] and `format` is not plain [`TickFormat::AUTO`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme, TickFormat};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.xtick_format(TickFormat::fixed(2)?.affix("", " s")?)?;
    /// assert_eq!(axes.x_tick_format().suffix(), " s");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xtick_format(&mut self, format: TickFormat) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.xtick_format()";
        check_tick_axis(CONTEXT, SharedAxis::X, self.x, "tick formats")?;
        let mut settings = (*self.x_settings).clone();
        settings.ticks.set_format(CONTEXT, format)?;
        self.figure.set_group(
            self.index,
            SharedAxis::X,
            Setting::Format,
            settings,
            CONTEXT,
        )?;
        Ok(self)
    }

    /// Sets the format of the y tick labels. Same rules as [`AxesMut::xtick_format`].
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged: `Error::Usage` on a category y axis, for
    /// [`TickFormat::iso`] on a linear or log axis, and when the y ticks have labels given with
    /// [`AxesMut::yticks_labeled`] and `format` is not plain [`TickFormat::AUTO`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme, TickFormat};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.ytick_format(TickFormat::percent(0)?)?;
    /// assert_eq!(axes.y_tick_format(), &TickFormat::percent(0)?);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn ytick_format(&mut self, format: TickFormat) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.ytick_format()";
        check_tick_axis(CONTEXT, SharedAxis::Y, self.y, "tick formats")?;
        let mut settings = (*self.y_settings).clone();
        settings.ticks.set_format(CONTEXT, format)?;
        self.figure.set_group(
            self.index,
            SharedAxis::Y,
            Setting::Format,
            settings,
            CONTEXT,
        )?;
        Ok(self)
    }

    /// Sets the legend style of these axes, overriding the theme's style.
    /// [`LegendStyle::None`] hides the legend. Calling it again replaces the style.
    /// A swatch shows a mark's fill as drawn: composited over the plot background, which
    /// leaves it opaque over an opaque background. Gradients show their most opaque end: 0.32
    /// of the series opacity for areas, 0.95 for histograms. Area and stacked-area swatches
    /// are filled rectangles under an edge in the line's color, dash and width; scatter, stem
    /// and bubble swatches show the marker fill at the theme's marker alpha. Lines, steps,
    /// reference lines and error bars keep the series color, as do direct labels and the
    /// outlines of box and violin swatches. Panel and merged figure legends use the same rule.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, LegendStyle, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.legend(LegendStyle::Chips).legend(LegendStyle::None);
    /// assert_eq!(axes.legend_style(), Some(LegendStyle::None));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn legend(&mut self, style: LegendStyle) -> &mut Self {
        let axes = self.axes_mut();
        axes.legend = Some(style);
        axes.legend_position = None;
        self
    }

    /// Sets the legend style and position of these axes, overriding the theme.
    ///
    /// `Error::Usage` for [`LegendStyle::Direct`]: direct labels sit at their line ends and
    /// take no position. A failed call changes nothing. [`AxesMut::legend`] resets the
    /// position to the theme's default.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, LegendPosition, LegendStyle, ScaleKind, Theme};
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.legend_at(LegendStyle::Header, LegendPosition::Bottom)?;
    /// assert_eq!(axes.legend_position(), Some(LegendPosition::Bottom));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn legend_at(&mut self, style: LegendStyle, position: LegendPosition) -> Result<&mut Self> {
        if style == LegendStyle::Direct {
            return Err(Error::usage(
                "Axes.legend()",
                format!("position is {}; direct labels sit at the ends of their lines and take no position.", position.display()),
            ).with_parameter("position").with_hint("drop position=, or use another legend style."));
        }
        let axes = self.axes_mut();
        axes.legend = Some(style);
        axes.legend_position = Some(position);
        Ok(self)
    }

    /// Adds a line. Errors leave the axes unchanged: `Error::Usage` on a category y axis, then
    /// `Error::Data` for mismatched lengths, an infinite value, a magnitude beyond
    /// [`MAX_MAGNITUDE`], a value outside the log range on a log axis, or `x` or `y` of the wrong
    /// kind for its axis. NaN is allowed and breaks the line.
    ///
    /// The mark stores its own copy of `y`, made once. To store values already held in an
    /// [`AxisData`] without copying them, use [`AxesMut::line_shared`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, LineOptions};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.line(AxisData::Numbers(vec![0.0, 1.0]), &[1.0, f64::NAN], LineOptions::default())?;
    /// assert!(axes
    ///     .line(AxisData::Numbers(vec![0.0]), &[1.0, 2.0], LineOptions::default())
    ///     .is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    ///
    /// [`MAX_MAGNITUDE`]: crate::MAX_MAGNITUDE
    pub fn line(&mut self, x: AxisData, y: &[f64], options: LineOptions) -> Result<&mut Self> {
        self.line_shared(x, AxisData::Numbers(y.to_vec()), options)
    }

    /// Adds a line as [`AxesMut::line`] does, but stores `y` itself rather than a copy of it, as
    /// it stores `x`: the mark, and every clone of the figure, share the allocation of `y`. Use
    /// it for values that are already in an [`AxisData`], so they are not copied again. Same
    /// rules and errors as [`AxesMut::line`]; a failed call stores nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, LineOptions, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let y = vec![1.0, f64::NAN];
    /// let values = y.as_ptr();
    /// let x = AxisData::Numbers(vec![0.0, 1.0]);
    /// axes.line_shared(x, AxisData::Numbers(y), LineOptions::default())?;
    /// let Some(Mark::Line(line)) = axes.marks().first() else { unreachable!() };
    /// let AxisData::Numbers(stored) = line.y() else { unreachable!() };
    /// assert!(std::ptr::eq(stored.as_ptr(), values));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn line_shared(
        &mut self,
        x: AxisData,
        y: AxisData,
        options: LineOptions,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.line()";
        let axes = self.axes_mut();
        check_numeric_y(CONTEXT, axes.y)?;
        let series = XySeries::new(CONTEXT, (axes.x, axes.y), x, y, LINE_GAP_HINT)?;
        axes.marks.push(Mark::line(series, options));
        Ok(self)
    }

    /// Adds a scatter series. Same rules as [`AxesMut::line`]; NaN skips a point. On a category
    /// y axis beside a linear or log x axis the scatter is a dot plot: `y` holds names, passed
    /// to [`AxesMut::scatter_shared`], and each point sits at the centre of its category's band;
    /// two category axes are `Error::Usage`, naming `x`.
    ///
    /// With [`PointColor::By`](crate::PointColor::By), each point takes the color its value
    /// maps to, and a NaN value skips the point (its position still counts in the axis
    /// limits). The values are checked by [`ColorBy::check`](crate::ColorBy::check) and must
    /// number one per point (`Error::Data`); the colormap, scale and center must be those of
    /// the axes' other color-mapped marks (`Error::Usage`) and suit the color limits of
    /// [`AxesMut::clim`] (`Error::Style`). A color-mapped scatter has no legend entry: a
    /// non-empty label is an `Error::Usage`.
    ///
    /// The mark stores its own copy of `y`, made once. To store values already held in an
    /// [`AxisData`] without copying them, use [`AxesMut::scatter_shared`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, ScatterOptions};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.scatter(AxisData::Numbers(vec![0.0, 1.0]), &[1.0, 2.0], ScatterOptions::default())?;
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn scatter(
        &mut self,
        x: AxisData,
        y: &[f64],
        options: ScatterOptions,
    ) -> Result<&mut Self> {
        self.scatter_shared(x, AxisData::Numbers(y.to_vec()), options)
    }

    /// Adds a scatter series as [`AxesMut::scatter`] does, but stores `y` itself rather than a
    /// copy of it, as [`AxesMut::line_shared`] does. Same rules and errors as [`AxesMut::scatter`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, Mark, ScatterOptions};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let y = vec![1.0, 2.0];
    /// let values = y.as_ptr();
    /// let x = AxisData::Numbers(vec![0.0, 1.0]);
    /// axes.scatter_shared(x, AxisData::Numbers(y), ScatterOptions::default())?;
    /// let Some(Mark::Scatter(scatter)) = axes.marks().first() else { unreachable!() };
    /// let AxisData::Numbers(stored) = scatter.y() else { unreachable!() };
    /// assert!(std::ptr::eq(stored.as_ptr(), values));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn scatter_shared(
        &mut self,
        x: AxisData,
        y: AxisData,
        options: ScatterOptions,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.scatter()";
        let axes = self.axes_mut();
        check_not_two_categories(CONTEXT, axes.x, axes.y)?;
        let series = XySeries::new(CONTEXT, (axes.x, axes.y), x, y, SCATTER_GAP_HINT)?;
        check_point_color(CONTEXT, axes, &options.color, series.x().len())?;
        check_scatter_label(CONTEXT, &options.color, &options.label)?;
        axes.marks.push(Mark::scatter(series, options));
        Ok(self)
    }

    /// Adds markers whose areas are proportional to `sizes`, on a numeric x axis.
    ///
    /// All bubble marks on these axes share one size scale. The largest size among points with
    /// finite x and y coordinates maps to [`Theme::bubble_max_radius`](crate::Theme::bubble_max_radius).
    /// Zero sizes and NaN coordinates draw nothing; a zero-size point's position still counts
    /// in the axis domains like any point. The remaining markers draw in input order with the
    /// scatter outline. Up to three true-size reference circles appear once, after the first
    /// labelled bubble entry, using the axes' y tick format. Repeated labels keep the larger
    /// circle; labels that round to zero are omitted. If all labels round to zero, only the
    /// largest reference circle remains, with an automatic label for its size, without the y
    /// format or its affixes. An all-zero or empty scale has only label entries.
    /// Automatic limits fit each whole marker and its outline at the final plot size, along
    /// each axis by the marker shape's own extent, before tick rounding and padding, in log
    /// space on log axes. Shared axes fit all their panels' bubbles. When the bubbles cannot
    /// all fit on an automatic axis (a marker, outline included, at least as long as the plot
    /// along it, limits beyond the supported numeric range, or limits that do not settle in
    /// the final layout), that axis's limits come from the data positions alone, and the plot
    /// clips the bubbles that overflow; rendering does not fail. Explicit limits still clip,
    /// and scatter limits are unchanged. Legend swatches and size circles show the marker fill
    /// composited over the plot background.
    ///
    /// A merged figure legend also shows only one set, after its first labelled bubble entry,
    /// using that panel's scale and y tick format. Panels do not share a size scale unless
    /// their size values give them the same maximum.
    ///
    /// A category x axis gives `Error::Usage`. Different lengths, non-finite or negative sizes,
    /// or sizes above 1e300 give `Error::Data`; coordinates follow the scatter rules.
    /// A color mapping follows the scatter rules too, except that a color-mapped bubble mark
    /// keeps its label: its legend entry and size references are drawn in the theme's text
    /// color. Errors change nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{BubbleOptions, Figure, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.bubbles(&[1.0, 2.0], &[3.0, 4.0], &[100.0, 400.0], BubbleOptions::default())?;
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn bubbles(
        &mut self,
        x: &[f64],
        y: &[f64],
        sizes: &[f64],
        options: super::bubbles::BubbleOptions,
    ) -> Result<&mut Self> {
        self.bubbles_shared(Arc::from(x), Arc::from(y), Arc::from(sizes), options)
    }

    /// Adds bubbles as [`AxesMut::bubbles`] does, storing existing shared arrays without copying.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use umber_core::{BubbleOptions, Figure, Mark, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let sizes: Arc<[f64]> = Arc::from([100.0]);
    /// axes.bubbles_shared(Arc::from([1.0]), Arc::from([2.0]), Arc::clone(&sizes), BubbleOptions::default())?;
    /// let Some(Mark::Bubbles(mark)) = axes.marks().first() else { unreachable!() };
    /// assert!(std::ptr::eq(mark.sizes(), &*sizes));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn bubbles_shared(
        &mut self,
        x: Arc<[f64]>,
        y: Arc<[f64]>,
        sizes: Arc<[f64]>,
        options: super::bubbles::BubbleOptions,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.bubbles()";
        let axes = self.axes_mut();
        let y_transform = check_numeric_y(CONTEXT, axes.y)?;
        super::validation::check_bubbles(CONTEXT, axes.x, y_transform, (&x, &y, &sizes))?;
        check_point_color(CONTEXT, axes, &options.color, x.len())?;
        axes.marks
            .push(Mark::Bubbles(super::bubbles::BubbleMark::new(
                x, y, sizes, options,
            )));
        Ok(self)
    }

    /// Adds a line from the baseline to each value and a scatter marker at the value. Same
    /// validation as [`AxesMut::scatter`]; NaN skips the point. Category x values sit at band
    /// centres. A linear y domain includes 0; a log y axis starts stems at its baseline.
    /// Explicit limits clip stems and markers as they do other marks. Points are not reduced.
    ///
    /// The line width is [`Theme::stem_width`](crate::Theme::stem_width); `options.size` sets
    /// only the marker diameter. Labelled stems take a legend row entry, never a direct label.
    /// The mark copies `y` once; [`AxesMut::stem_shared`] can store an existing [`AxisData`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{AxisData, Figure, ScaleKind, StemOptions, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.stem(AxisData::Numbers(vec![0.0, 1.0]), &[2.0, -1.0], StemOptions::default())?;
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn stem(&mut self, x: AxisData, y: &[f64], options: StemOptions) -> Result<&mut Self> {
        self.stem_shared(x, AxisData::Numbers(y.to_vec()), options)
    }

    /// Adds stems as [`AxesMut::stem`] does, storing `y` without copying it. Same rules and
    /// errors as [`AxesMut::stem`]; a failed call leaves the figure unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{AxisData, Figure, Mark, ScaleKind, StemOptions, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let y = vec![2.0, -1.0];
    /// let values = y.as_ptr();
    /// let x = AxisData::Numbers(vec![0.0, 1.0]);
    /// axes.stem_shared(x, AxisData::Numbers(y), StemOptions::default())?;
    /// let Some(Mark::Stem(stem)) = axes.marks().first() else { unreachable!() };
    /// let AxisData::Numbers(stored) = stem.y() else { unreachable!() };
    /// assert!(std::ptr::eq(stored.as_ptr(), values));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn stem_shared(
        &mut self,
        x: AxisData,
        y: AxisData,
        options: StemOptions,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.stem()";
        let axes = self.axes_mut();
        check_numeric_y(CONTEXT, axes.y)?;
        let series = XySeries::new(CONTEXT, (axes.x, axes.y), x, y, SCATTER_GAP_HINT)?;
        axes.marks.push(Mark::Stem(StemMark::new(series, options)));
        Ok(self)
    }

    /// Adds an area: the line through the points, drawn as [`AxesMut::line`] draws it, over the
    /// region between the line and the baseline, filled in the line's color as the theme's
    /// [`FillStyle`](crate::FillStyle) paints it. The fill is drawn first and the line over it.
    /// Like bars, an area puts 0 into a linear y axis, so the filled region is proportional to
    /// the values; on a log y axis it fills down to the baseline, at the lowest y tick (or the
    /// explicit lower limit). Same rules as [`AxesMut::line`]: NaN breaks both the line and the
    /// fill.
    ///
    /// The mark stores its own copy of `y`, made once. To store values already held in an
    /// [`AxisData`] without copying them, use [`AxesMut::area_shared`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AreaOptions, AxisData};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
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
    pub fn area(&mut self, x: AxisData, y: &[f64], options: AreaOptions) -> Result<&mut Self> {
        self.area_shared(x, AxisData::Numbers(y.to_vec()), options)
    }

    /// Adds an area as [`AxesMut::area`] does, but stores `y` itself rather than a copy of it, as
    /// [`AxesMut::line_shared`] does. Same rules and errors as [`AxesMut::area`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AreaOptions, AxisData, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let y = vec![3.0, 4.0];
    /// let values = y.as_ptr();
    /// let x = AxisData::Numbers(vec![0.0, 1.0]);
    /// axes.area_shared(x, AxisData::Numbers(y), AreaOptions::default())?;
    /// let Some(Mark::Area(area)) = axes.marks().first() else { unreachable!() };
    /// let AxisData::Numbers(stored) = area.y() else { unreachable!() };
    /// assert!(std::ptr::eq(stored.as_ptr(), values));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn area_shared(
        &mut self,
        x: AxisData,
        y: AxisData,
        options: AreaOptions,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.area()";
        let axes = self.axes_mut();
        check_numeric_y(CONTEXT, axes.y)?;
        let series = XySeries::new(CONTEXT, (axes.x, axes.y), x, y, AREA_GAP_HINT)?;
        axes.marks.push(Mark::area(series, options));
        Ok(self)
    }

    /// Adds non-negative series stacked in order over shared x values.
    ///
    /// The first layer starts at the baseline, each next layer at the previous cumulative
    /// curve. Every layer is filled flat at the area opacity, whatever the fill style, and
    /// its upper curve is drawn at the theme's line width. Its legend swatch and direct label
    /// follow that curve as an area's do. Linear y includes 0; log y starts at its lower limit.
    /// Long inputs are reduced per device column, keeping every cumulative curve's envelope.
    ///
    /// A category x axis is a usage error. There must be at least one series, each as long as
    /// x. Values must be finite (NaN is allowed), non-negative and within the supported scale
    /// bounds; log values must be positive. The cumulative totals must not exceed 1e300.
    /// x must be in non-decreasing order, NaN apart. A NaN in x or any series breaks all layers
    /// at that index. A failed call changes nothing.
    ///
    /// x is copied; to share its allocation use [`AxesMut::stacked_area_shared`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Series, Theme};
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.stacked_area(&[0.0, 1.0], vec![
    ///     Series { label: "Hydro".into(), values: vec![3.0, 4.0], ..Series::default() },
    ///     Series { label: "Wind".into(), values: vec![1.0, 2.0], ..Series::default() },
    /// ])?;
    /// assert!(axes.stacked_area(&[0.0], vec![]).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn stacked_area(&mut self, x: &[f64], series: Vec<Series>) -> Result<&mut Self> {
        self.stacked_area_shared(Arc::from(x), series)
    }

    /// Adds stacked areas as [`AxesMut::stacked_area`] does, storing `x` itself, not a copy.
    /// Same rules and errors as [`AxesMut::stacked_area`]. Clones share the x and series data.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use umber_core::{Figure, Mark, ScaleKind, Series, Theme};
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let x: Arc<[f64]> = Arc::from([0.0, 1.0]);
    /// axes.stacked_area_shared(Arc::clone(&x), vec![Series {
    ///     values: vec![2.0, 3.0], ..Series::default()
    /// }])?;
    /// let Some(Mark::StackedArea(mark)) = axes.marks().first() else { unreachable!() };
    /// assert!(std::ptr::eq(mark.x(), &*x));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn stacked_area_shared(&mut self, x: Arc<[f64]>, series: Vec<Series>) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.stacked_area()";
        let axes = self.axes_mut();
        let y = check_numeric_y(CONTEXT, axes.y)?;
        let mark = StackedAreaMark::new(CONTEXT, axes.x, y, x, series)?;
        axes.marks.push(Mark::StackedArea(mark));
        Ok(self)
    }

    /// Adds bars: one bar per category, centered in the category's band, from the baseline to
    /// its height. On a linear y axis bars grow from 0 and negative heights grow downwards; on a
    /// log y axis they grow from the baseline, at the lowest y tick (or the explicit lower
    /// limit). Bar marks added one after another are drawn at the same positions, in the order
    /// they were added; categories join the x axis in order of first appearance across all
    /// marks.
    ///
    /// On a category y axis beside a linear or log x axis the bars are horizontal: the same
    /// bars with the axes swapped. The categories run down the y axis, the first at the top,
    /// and the bars grow along x from the baseline, rightwards for positive heights.
    ///
    /// Errors leave the axes unchanged. Two category axes are `Error::Usage`, naming `x`. The
    /// others are `Error::Data`, checked in this order: `categories` and `heights` of different
    /// lengths; two numeric axes, whose x axis is not a category axis; a category that appears
    /// twice; a height that is infinite, beyond [`MAX_MAGNITUDE`], or outside the log range on
    /// a log value axis. NaN is allowed and draws no bar.
    ///
    /// The mark stores its own copy of `heights`, made once. To store heights already held in
    /// an `Arc<[f64]>` without copying them, use [`AxesMut::bar_shared`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::BarOptions;
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// axes.bar(vec!["a".into(), "b".into()], &[3.0, f64::NAN], BarOptions::default())?;
    /// assert_eq!(axes.marks().len(), 1);
    /// // A repeated category is an error, and the axes keep their one mark.
    /// assert!(axes.bar(vec!["a".into(), "a".into()], &[1.0, 2.0], BarOptions::default()).is_err());
    /// assert_eq!(axes.marks().len(), 1);
    ///
    /// // On a category y axis the bars are horizontal.
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut rows = figure.add_axes(ScaleKind::Linear, ScaleKind::Category)?;
    /// rows.bar(vec!["a".into(), "b".into()], &[3.0, -1.0], BarOptions::default())?;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    ///
    /// [`MAX_MAGNITUDE`]: crate::MAX_MAGNITUDE
    pub fn bar(
        &mut self,
        categories: Vec<String>,
        heights: &[f64],
        options: BarOptions,
    ) -> Result<&mut Self> {
        self.bar_shared(categories, Arc::from(heights), options)
    }

    /// Adds bars as [`AxesMut::bar`] does, but stores `heights` itself rather than a copy of it,
    /// as [`AxesMut::line_shared`] does. Same rules and errors as [`AxesMut::bar`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use umber_core::figure::{BarOptions, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let heights: Arc<[f64]> = Arc::from([3.0, -1.0]);
    /// axes.bar_shared(vec!["a".into(), "b".into()], Arc::clone(&heights), BarOptions::default())?;
    /// let Some(Mark::Bar(bar)) = axes.marks().first() else { unreachable!() };
    /// assert!(std::ptr::eq(bar.heights(), &*heights));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn bar_shared(
        &mut self,
        categories: Vec<String>,
        heights: Arc<[f64]>,
        options: BarOptions,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.bar()";
        let axes = self.axes_mut();
        let (along, values) = check_band_axes(CONTEXT, axes.x, axes.y)?;
        let mark = BarMark::new(CONTEXT, along, values, categories, heights, options)?;
        axes.marks.push(Mark::Bar(mark));
        Ok(self)
    }

    /// Adds grouped bars: in each category, one bar per series, side by side in series order.
    /// The group takes 72 % of the category's band, centered in it, with a 2 pt gap between
    /// its bars; when the bars would be narrower than the gap, they touch. Each bar grows from
    /// the baseline as in [`AxesMut::bar`]. Automatic colors share one palette slot per label
    /// across the figure; unlabelled series take slots after all labels, per axes.
    /// Categories join the x axis in order of first appearance
    /// across all marks. On a category y axis the bars are horizontal, as in [`AxesMut::bar`],
    /// and the series run down each category in series order, the order of the legend.
    ///
    /// Errors leave the axes unchanged. Two category axes are `Error::Usage`, naming `x`. The
    /// others are `Error::Data`, checked in this order: an empty `series`; a series whose values
    /// are not one per category (the message names the series' index); then the categories and
    /// each series' values as in [`AxesMut::bar`]. NaN is allowed and draws no bar.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::Series;
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
        series: Vec<Series>,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.grouped_bar()";
        let axes = self.axes_mut();
        let (along, values) = check_band_axes(CONTEXT, axes.x, axes.y)?;
        let mark = BarGroupMark::new(
            CONTEXT,
            Arrangement::Grouped,
            along,
            values,
            categories,
            series,
        )?;
        axes.marks.push(Mark::GroupedBar(mark));
        Ok(self)
    }

    /// Adds one box per category: type-7 quartiles, 1.5-IQR Tukey whiskers and outliers.
    /// NaN is ignored; empty groups draw nothing, and constant groups draw only a median line.
    /// Boxes occupy the theme's [`box_width`](crate::Theme::box_width) fraction of each band.
    /// The automatic y domain includes whiskers and outliers, without forcing zero. On a
    /// category y axis the boxes are horizontal, the axes swapped as for [`AxesMut::bar`].
    ///
    /// Two category axes are `Error::Usage`, naming `x`. A numeric x axis beside a numeric y
    /// axis is `Error::Usage`. Group counts, repeated categories, infinities,
    /// unsupported magnitudes and non-positive log values are `Error::Data`.
    /// Errors leave the figure unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{BoxOptions, Figure, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// f.add_axes(ScaleKind::Category, ScaleKind::Linear)?
    ///     .box_plot(vec!["North".into()], vec![vec![1.0, 2.0, 4.0]], BoxOptions::default())?;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn box_plot(
        &mut self,
        categories: Vec<String>,
        groups: Vec<Vec<f64>>,
        options: BoxOptions,
    ) -> Result<&mut Self> {
        let axes = self.axes_mut();
        let (along, values) = check_band_axes("Axes.box()", axes.x, axes.y)?;
        let mark = BoxMark::new(along, values, categories, groups, options)?;
        axes.marks.push(Mark::Box(mark));
        Ok(self)
    }

    /// Adds distributions side by side in each category, in input order.
    /// The group occupies the theme's box width fraction of the band. Neighbours are 2 pt
    /// apart; they touch when the gap would leave boxes narrower than it, as grouped bars do.
    /// Each distribution has its own color and legend entry. Statistics and errors follow
    /// [`AxesMut::box_plot`]; empty distributions are `Error::Data`, and every distribution
    /// must have one group per category. A failed call leaves the figure unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Distribution, Figure, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let d = Distribution { label: "North".into(), groups: vec![vec![1.0, 3.0]], ..Default::default() };
    /// f.add_axes(ScaleKind::Category, ScaleKind::Linear)?
    ///     .grouped_box_plot(vec!["2026".into()], vec![d])?;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn grouped_box_plot(
        &mut self,
        categories: Vec<String>,
        distributions: Vec<Distribution>,
    ) -> Result<&mut Self> {
        let axes = self.axes_mut();
        let (along, values) = check_band_axes("Axes.grouped_box()", axes.x, axes.y)?;
        let mark = BoxGroupMark::new(along, values, categories, distributions)?;
        axes.marks.push(Mark::GroupedBox(mark));
        Ok(self)
    }

    /// Adds a Gaussian density per category, using Scott bandwidth and 128 points from min to max.
    /// NaN is ignored; empty groups draw nothing, and constant groups draw only a median line.
    /// Violins occupy the theme's [`violin_width`](crate::Theme::violin_width) fraction of each band.
    /// Each density has the same maximum width. The median spans its local width; quartile
    /// lines span half their local width. The y domain includes all samples, without forcing zero.
    /// On a category y axis the violins are horizontal, the axes swapped as for
    /// [`AxesMut::bar`].
    ///
    /// Two category axes are `Error::Usage`, naming `x`. A numeric x axis beside a numeric y
    /// axis is `Error::Usage`. Group counts, repeated categories, infinities,
    /// unsupported magnitudes and non-positive log values are `Error::Data`.
    /// Errors leave the figure unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ViolinOptions, Figure, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// f.add_axes(ScaleKind::Category, ScaleKind::Linear)?
    ///     .violin(vec!["North".into()], vec![vec![1.0, 2.0, 4.0]], ViolinOptions::default())?;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn violin(
        &mut self,
        categories: Vec<String>,
        groups: Vec<Vec<f64>>,
        options: ViolinOptions,
    ) -> Result<&mut Self> {
        let axes = self.axes_mut();
        let (along, values) = check_band_axes("Axes.violin()", axes.x, axes.y)?;
        let mark = ViolinMark::new(along, values, categories, groups, options)?;
        axes.marks.push(Mark::Violin(mark));
        Ok(self)
    }

    /// Adds distributions side by side in each category, in input order.
    /// The group occupies the theme's violin width fraction of the band. Neighbours are 2 pt
    /// apart; they touch when the gap would leave violins narrower than it, as grouped bars do.
    /// Each distribution has its own color and legend entry. Statistics and errors follow
    /// [`AxesMut::violin`]; empty distributions are `Error::Data`, and every distribution
    /// must have one group per category. A failed call leaves the figure unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Distribution, Figure, ScaleKind, Theme};
    /// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let d = Distribution { label: "North".into(), groups: vec![vec![1.0, 3.0]], ..Default::default() };
    /// f.add_axes(ScaleKind::Category, ScaleKind::Linear)?
    ///     .grouped_violin(vec!["2026".into()], vec![d])?;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn grouped_violin(
        &mut self,
        categories: Vec<String>,
        distributions: Vec<Distribution>,
    ) -> Result<&mut Self> {
        let axes = self.axes_mut();
        let (along, values) = check_band_axes("Axes.grouped_violin()", axes.x, axes.y)?;
        let mark = ViolinGroupMark::new(along, values, categories, distributions)?;
        axes.marks.push(Mark::GroupedViolin(mark));
        Ok(self)
    }

    /// Adds stacked bars: in each category, one stack, as wide as a bar of [`AxesMut::bar`], with
    /// the series stacked in series order. Positive values stack upward from the baseline and
    /// negative ones downward, each side separately; NaN and 0 add no segment. Neighbouring
    /// segments on one side are 1 pt apart, and a segment too thin for that gap is not drawn.
    /// Only the outermost segment on each side has its end rounded. On a log y axis the first
    /// segment starts at the baseline, at the lowest y tick (or the explicit lower limit).
    /// Colors and categories work as in [`AxesMut::grouped_bar`]. On a category y axis the
    /// stacks are horizontal, as in [`AxesMut::bar`]: positive values stack rightwards.
    ///
    /// Errors leave the axes unchanged. They are `Error::Data`, checked in the order of
    /// [`AxesMut::grouped_bar`], with one more check at the end: in each category, the positive
    /// values and the negative values must each add up to at most
    /// [`MAX_MAGNITUDE`] in magnitude (the message names the category).
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::Series;
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let series = |values: &[f64]| Series { values: values.to_vec(), ..Series::default() };
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
        series: Vec<Series>,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.stacked_bar()";
        let axes = self.axes_mut();
        let (along, values) = check_band_axes(CONTEXT, axes.x, axes.y)?;
        let mark = BarGroupMark::new(
            CONTEXT,
            Arrangement::Stacked,
            along,
            values,
            categories,
            series,
        )?;
        axes.marks.push(Mark::StackedBar(mark));
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
    /// The mark stores its own copy of `values`, made once. To store values already held in an
    /// `Arc<[f64]>` without copying them, use [`AxesMut::hist_shared`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::HistOptions;
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
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
    pub fn hist(&mut self, values: &[f64], options: HistOptions) -> Result<&mut Self> {
        self.hist_shared(Arc::from(values), options)
    }

    /// Adds a histogram as [`AxesMut::hist`] does, but stores `values` itself rather than a copy
    /// of it, as [`AxesMut::line_shared`] does. Same rules and errors as [`AxesMut::hist`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use umber_core::figure::{HistOptions, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let values: Arc<[f64]> = Arc::from([1.2, 3.4, f64::NAN]);
    /// axes.hist_shared(Arc::clone(&values), HistOptions::default())?;
    /// let Some(Mark::Hist(hist)) = axes.marks().first() else { unreachable!() };
    /// assert!(std::ptr::eq(hist.values(), &*values));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn hist_shared(&mut self, values: Arc<[f64]>, options: HistOptions) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.hist()";
        let axes = self.axes_mut();
        check_numeric_y(CONTEXT, axes.y)?;
        let mark = HistMark::new(CONTEXT, axes.x, values, options)?;
        axes.marks.push(Mark::Hist(mark));
        Ok(self)
    }

    /// Adds a step line: horizontal and vertical segments through the points, which change level
    /// where `options.position` says (see [`Step`](crate::Step)). Same rules as
    /// [`AxesMut::line`], and NaN breaks the line; on a numeric x axis `x` must also be in
    /// non-decreasing order, NaN apart. Names on a category x axis are drawn at the centres of
    /// their bands, in the order given.
    ///
    /// Errors leave the axes unchanged. They are `Error::Data`, checked in this order as for
    /// [`AxesMut::line`], then: an x value below the one before it, naming the first such
    /// index.
    ///
    /// The mark stores its own copy of `y`, made once. To store values already held in an
    /// [`AxisData`] without copying them, use [`AxesMut::step_shared`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, StepOptions};
    /// use umber_core::{Figure, ScaleKind, Step, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let mid = StepOptions { position: Step::Mid, ..StepOptions::default() };
    /// axes.step(AxisData::Numbers(vec![0.0, 1.0, 3.0]), &[1.0, 2.0, f64::NAN], mid)?;
    /// // Unsorted x is an error, and the axes keep their one mark.
    /// assert!(axes
    ///     .step(AxisData::Numbers(vec![1.0, 0.0]), &[1.0, 2.0], StepOptions::default())
    ///     .is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn step(&mut self, x: AxisData, y: &[f64], options: StepOptions) -> Result<&mut Self> {
        self.step_shared(x, AxisData::Numbers(y.to_vec()), options)
    }

    /// Adds a step line as [`AxesMut::step`] does, but stores `y` itself rather than a copy of
    /// it, as [`AxesMut::line_shared`] does. Same rules and errors as [`AxesMut::step`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, Mark, StepOptions};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let y = vec![1.0, 2.0];
    /// let values = y.as_ptr();
    /// let x = AxisData::Numbers(vec![0.0, 1.0]);
    /// axes.step_shared(x, AxisData::Numbers(y), StepOptions::default())?;
    /// let Some(Mark::Step(step)) = axes.marks().first() else { unreachable!() };
    /// let AxisData::Numbers(stored) = step.y() else { unreachable!() };
    /// assert!(std::ptr::eq(stored.as_ptr(), values));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn step_shared(
        &mut self,
        x: AxisData,
        y: AxisData,
        options: StepOptions,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.step()";
        let axes = self.axes_mut();
        check_numeric_y(CONTEXT, axes.y)?;
        let series = XySeries::new(CONTEXT, (axes.x, axes.y), x, y, LINE_GAP_HINT)?;
        check_step_order(CONTEXT, series.x())?;
        axes.marks.push(Mark::Step(StepMark::new(series, options)));
        Ok(self)
    }

    /// Adds a horizontal reference line across the whole plot at `y`. It is drawn above all data
    /// marks, and `y` extends the automatic y domain; explicit y limits clip it like any mark.
    ///
    /// Errors leave the axes unchanged. They are `Error::Data`: `y` is not finite, exceeds
    /// [`MAX_MAGNITUDE`](crate::MAX_MAGNITUDE) in magnitude, or is not positive on a log axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::RuleOptions;
    /// use umber_core::{Dash, Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let dashed = RuleOptions { dash: Dash::Dashed, ..RuleOptions::default() };
    /// axes.hline(66.0, dashed)?;
    /// assert!(axes.hline(f64::NAN, RuleOptions::default()).is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn hline(&mut self, y: f64, options: RuleOptions) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.hline()";
        let axes = self.axes_mut();
        let y_transform = check_numeric_y(CONTEXT, axes.y)?;
        check_reference_value(CONTEXT, "y", REFERENCE_LINE, y, y_transform)?;
        axes.marks.push(Mark::HLine(HLineMark::new(y, options)));
        Ok(self)
    }

    /// Adds a vertical reference line across the plot at `x`: a number on a linear or log
    /// x axis, where it extends the automatic x domain, and a category name on a category x
    /// axis, where it sits at the centre of the category's band. It runs the full height of the
    /// plot, except when the baseline sits at the y axis's lower limit, that is when 0 is not
    /// inside a linear y domain: then it ends at the baseline, since only padding lies below it.
    /// On a category y axis beside a linear or log x axis, where the baseline is vertical too,
    /// it runs the full height. It is drawn above all data marks; explicit x limits clip it like
    /// any mark. Two category axes are `Error::Usage`, naming `x`.
    ///
    /// A category name does not extend a category axis. Whether the name is one of the axis's
    /// categories, the names of its other marks, is known only when the figure is rendered:
    /// [`Figure::to_scene`](crate::Figure::to_scene), [`Figure::to_png`](crate::Figure::to_png)
    /// and [`Figure::to_svg`](crate::Figure::to_svg) fail with `Error::Data`, in their own
    /// context, when it is not.
    ///
    /// Errors leave the axes unchanged. They are `Error::Data`: a number on a category axis, a
    /// name on a linear or log axis, or a number that is not finite, exceeds
    /// [`MAX_MAGNITUDE`](crate::MAX_MAGNITUDE) in magnitude or is not positive on a log axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::RuleOptions;
    /// use umber_core::{AxisValue, Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.vline(AxisValue::Number(31.0), RuleOptions::default())?;
    /// // A name needs a category axis.
    /// assert!(axes
    ///     .vline(AxisValue::Category("Q3".into()), RuleOptions::default())
    ///     .is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn vline(&mut self, x: AxisValue, options: RuleOptions) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.vline()";
        let axes = self.axes_mut();
        check_not_two_categories(CONTEXT, axes.x, axes.y)?;
        check_vline_x(CONTEXT, axes.x, &x)?;
        axes.marks.push(Mark::VLine(VLineMark::new(x, options)));
        Ok(self)
    }

    /// Adds a horizontal reference span: the region between the y values `low` and `high`,
    /// across the whole plot, filled at the area opacity. It is drawn below all data marks, and
    /// both values extend the automatic y domain; explicit y limits clip it.
    ///
    /// Errors leave the axes unchanged. `Error::Data` when `low` or `high`, checked in that
    /// order, is not finite, exceeds [`MAX_MAGNITUDE`](crate::MAX_MAGNITUDE) in magnitude or is
    /// not positive on a log axis; then `Error::Style`, parameter `high`, when `low` is not
    /// below `high`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::SpanOptions;
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.hspan(42.0, 58.0, SpanOptions::default())?;
    /// assert!(axes.hspan(5.0, 2.0, SpanOptions::default()).is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn hspan(&mut self, low: f64, high: f64, options: SpanOptions) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.hspan()";
        let axes = self.axes_mut();
        let y = check_numeric_y(CONTEXT, axes.y)?;
        check_span(CONTEXT, (low, high), y)?;
        axes.marks
            .push(Mark::HSpan(SpanMark::new(low, high, options)));
        Ok(self)
    }

    /// Adds a vertical reference span: the region between the x values `low` and `high`, across
    /// the full height of the plot, or only down to the baseline when that sits at the y axis's
    /// lower limit (see [`AxesMut::vline`]), filled at the area opacity. It is drawn below all data marks, and both
    /// values extend the automatic x domain; explicit x limits clip it. A span needs a numeric
    /// x axis: on a category x axis use [`AxesMut::vline`]. On a category y axis it runs the
    /// full height, as [`AxesMut::vline`] does.
    ///
    /// Errors leave the axes unchanged. `Error::Usage` on a category x axis, naming `x` when the
    /// y axis is a category axis too; otherwise as [`AxesMut::hspan`], for the x axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::SpanOptions;
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.vspan(40.0, 50.0, SpanOptions::default())?;
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut categories = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// assert!(categories.vspan(0.0, 1.0, SpanOptions::default()).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn vspan(&mut self, low: f64, high: f64, options: SpanOptions) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.vspan()";
        let axes = self.axes_mut();
        check_not_two_categories(CONTEXT, axes.x, axes.y)?;
        let Some(transform) = axes.x.transform() else {
            return Err(Error::usage(
                CONTEXT,
                "this is a category axis; spans along x apply to linear and log axes only.",
            )
            .with_hint("vspan needs a numeric x axis; use vline on a category."));
        };
        check_span(CONTEXT, (low, high), transform)?;
        axes.marks
            .push(Mark::VSpan(SpanMark::new(low, high, options)));
        Ok(self)
    }

    /// Adds a band: the region between the lower curve `(x[i], low[i])` and the upper curve
    /// `(x[i], high[i])`, filled flat at the area opacity with no outline, whatever the theme's
    /// [`FillStyle`](crate::FillStyle). Both curves extend the automatic y domain, and `x` the
    /// automatic x domain as the x values of a line do. A NaN in `x`, `low` or `high` breaks the
    /// band there into separate polygons. A band needs a linear or log x axis.
    ///
    /// Errors leave the axes unchanged. `Error::Usage` on a category x axis; otherwise
    /// `Error::Data`, checked in this order: the three slices differ in length; a value of `x`,
    /// then of `low`, then of `high` is infinite, beyond [`MAX_MAGNITUDE`](crate::MAX_MAGNITUDE)
    /// or not valid on its axis (positive on a log axis); `low[i]` is above `high[i]`, naming the
    /// first such index. Where either is NaN, the index is not compared.
    ///
    /// `x` must be in non-decreasing order, NaN apart, as for [`AxesMut::step`]: the first
    /// value below the one before it is an `Error::Data` naming its index, checked after the
    /// values and before the curves are compared.
    ///
    /// The mark stores its own copies of the values, made once. To store values already held in
    /// `Arc<[f64]>`s without copying them, use [`AxesMut::band_shared`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::BandOptions;
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.band(&[0.0, 1.0, 2.0], &[1.0, 2.0, 1.5], &[3.0, 4.0, 3.5], BandOptions::default())?;
    /// // A lower curve above the upper one is an error, and the axes keep their one mark.
    /// assert!(axes
    ///     .band(&[0.0, 1.0], &[5.0, 5.0], &[1.0, 1.0], BandOptions::default())
    ///     .is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn band(
        &mut self,
        x: &[f64],
        low: &[f64],
        high: &[f64],
        options: BandOptions,
    ) -> Result<&mut Self> {
        self.band_shared(Arc::from(x), Arc::from(low), Arc::from(high), options)
    }

    /// Adds a band as [`AxesMut::band`] does, but stores the three arrays themselves rather
    /// than copies of them, as [`AxesMut::line_shared`] does for `y`. Same rules and errors as
    /// [`AxesMut::band`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use umber_core::figure::{BandOptions, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let x: Arc<[f64]> = Arc::from([0.0, 1.0]);
    /// let low: Arc<[f64]> = Arc::from([1.0, 2.0]);
    /// let high: Arc<[f64]> = Arc::from([3.0, 4.0]);
    /// axes.band_shared(Arc::clone(&x), low, high, BandOptions::default())?;
    /// let Some(Mark::Band(band)) = axes.marks().first() else { unreachable!() };
    /// assert!(std::ptr::eq(band.x(), &*x));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn band_shared(
        &mut self,
        x: Arc<[f64]>,
        low: Arc<[f64]>,
        high: Arc<[f64]>,
        options: BandOptions,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.band()";
        let axes = self.axes_mut();
        let y = check_numeric_y(CONTEXT, axes.y)?;
        check_band(CONTEXT, axes.x, y, (&x, &low, &high))?;
        axes.marks
            .push(Mark::Band(BandMark::new(x, low, high, options)));
        Ok(self)
    }

    /// Adds vertical error bars: for each `i`, a line along y from `low[i]` to `high[i]` at
    /// `x[i]`, at the band centre of the category on a category x axis. With `caps` set in
    /// `options`, each end has a cap, a line across the bar of [`Theme::cap_width`] centred on
    /// it. The line width is the theme's line width times the options' width. Both limits
    /// extend the automatic y domain, and the positions the automatic x domain as the x values
    /// of a scatter do. A NaN in `x`, `low` or `high` skips that bar. With explicit limits a bar
    /// partly outside the plot keeps the part inside, and loses the cap that is outside.
    ///
    /// The bars of a grouped bar mark cannot be addressed: there is no way to place a bar at the
    /// offset of one series of a group.
    ///
    /// Errors leave the axes unchanged. They are `Error::Data`, checked in this order: `x`,
    /// `low` and `high` differ in length; a value of `x` does not suit the x axis (names on a
    /// category axis, numbers otherwise) or is infinite, beyond
    /// [`MAX_MAGNITUDE`](crate::MAX_MAGNITUDE) or not positive on a log axis; the same for `low`
    /// and then `high` on the y axis; `low[i]` is above `high[i]`, naming the first such index.
    /// Where either is NaN, the index is not compared.
    ///
    /// The mark stores its own copies of the limits, made once. To store limits already held in
    /// `Arc<[f64]>`s without copying them, use [`AxesMut::y_error_bars_shared`].
    ///
    /// [`Theme::cap_width`]: crate::Theme::cap_width
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, ErrorBarOptions};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// let x = AxisData::Categories(vec!["a".into(), "b".into()]);
    /// axes.y_error_bars(x, &[3.0, 4.0], &[5.0, 6.5], ErrorBarOptions::default())?;
    /// // A lower limit above the upper one is an error, and the axes keep their one mark.
    /// let x = AxisData::Categories(vec!["a".into()]);
    /// assert!(axes.y_error_bars(x, &[5.0], &[1.0], ErrorBarOptions::default()).is_err());
    /// assert_eq!(axes.marks().len(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn y_error_bars(
        &mut self,
        x: AxisData,
        low: &[f64],
        high: &[f64],
        options: ErrorBarOptions,
    ) -> Result<&mut Self> {
        self.y_error_bars_shared(x, Arc::from(low), Arc::from(high), options)
    }

    /// Adds vertical error bars as [`AxesMut::y_error_bars`] does, but stores `low` and `high`
    /// themselves rather than copies of them, as [`AxesMut::line_shared`] does for `y`. Same
    /// rules and errors as [`AxesMut::y_error_bars`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use umber_core::figure::{AxisData, ErrorBarOptions, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let low: Arc<[f64]> = Arc::from([1.0, 2.0]);
    /// let high: Arc<[f64]> = Arc::from([3.0, 4.0]);
    /// let x = AxisData::Numbers(vec![0.0, 1.0]);
    /// axes.y_error_bars_shared(x, Arc::clone(&low), high, ErrorBarOptions::default())?;
    /// let Some(Mark::ErrorBars(bars)) = axes.marks().first() else { unreachable!() };
    /// assert!(std::ptr::eq(bars.low(), &*low));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn y_error_bars_shared(
        &mut self,
        x: AxisData,
        low: Arc<[f64]>,
        high: Arc<[f64]>,
        options: ErrorBarOptions,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.y_error_bars()";
        let axes = self.axes_mut();
        let y = check_numeric_y(CONTEXT, axes.y)?;
        check_y_error_bars(CONTEXT, axes.x, y, &x, (&low, &high))?;
        axes.marks.push(Mark::ErrorBars(ErrorBarMark::new(
            ErrorBarOrientation::Vertical,
            x,
            low,
            high,
            options,
        )));
        Ok(self)
    }

    /// Adds horizontal error bars: for each `i`, a line along x from `low[i]` to `high[i]` at
    /// `y[i]`. With `caps` set in `options`, each end has a cap, a line across the bar of
    /// [`Theme::cap_width`] centred on it. Both limits extend the automatic x domain, and the
    /// positions the automatic y domain as the y values of a scatter do. Otherwise as
    /// [`AxesMut::y_error_bars`], with the roles of the axes swapped. They need a linear or log
    /// x axis. The positions of this method are numbers, for a linear or log y axis; on a
    /// category y axis they are names, passed to [`AxesMut::x_error_bars_shared`], and each bar
    /// sits at the centre of its category's band.
    ///
    /// Errors leave the axes unchanged. `Error::Usage` on two category axes, naming `x`, and on
    /// a category x axis; otherwise `Error::Data`, checked in this order: `y`, `low` and `high`
    /// differ in length; `y` holds numbers on a category y axis or names on a linear or log one;
    /// a value of `y` is infinite, beyond [`MAX_MAGNITUDE`](crate::MAX_MAGNITUDE) or not
    /// positive on a log axis; the same for `low` and then `high` on the x axis; `low[i]` is
    /// above `high[i]`, naming the first such index.
    ///
    /// The mark stores its own copies of the values, made once. To store limits already held in
    /// `Arc<[f64]>`s without copying them, use [`AxesMut::x_error_bars_shared`].
    ///
    /// [`Theme::cap_width`]: crate::Theme::cap_width
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::ErrorBarOptions;
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.x_error_bars(&[1.0, 2.0], &[0.5, 1.5], &[1.5, 2.8], ErrorBarOptions::default())?;
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut categories = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
    /// assert!(categories
    ///     .x_error_bars(&[1.0], &[0.0], &[2.0], ErrorBarOptions::default())
    ///     .is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn x_error_bars(
        &mut self,
        y: &[f64],
        low: &[f64],
        high: &[f64],
        options: ErrorBarOptions,
    ) -> Result<&mut Self> {
        self.x_error_bars_shared(
            AxisData::Numbers(y.to_vec()),
            Arc::from(low),
            Arc::from(high),
            options,
        )
    }

    /// Adds horizontal error bars as [`AxesMut::x_error_bars`] does, but stores `low` and `high`
    /// themselves rather than copies of them. The positions `y` are stored as given, without a
    /// copy: numbers on a linear or log y axis, names on a category y axis. Same rules and
    /// errors as [`AxesMut::x_error_bars`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use umber_core::figure::{AxisData, ErrorBarOptions, Mark};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let high: Arc<[f64]> = Arc::from([3.0, 4.0]);
    /// axes.x_error_bars_shared(
    ///     AxisData::Numbers(vec![0.0, 1.0]),
    ///     Arc::from([1.0, 2.0]),
    ///     Arc::clone(&high),
    ///     ErrorBarOptions::default(),
    /// )?;
    /// let Some(Mark::ErrorBars(bars)) = axes.marks().first() else { unreachable!() };
    /// assert!(std::ptr::eq(bars.high(), &*high));
    ///
    /// // On a category y axis the positions are names.
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut rows = figure.add_axes(ScaleKind::Linear, ScaleKind::Category)?;
    /// let names = AxisData::Categories(vec!["North".into(), "South".into()]);
    /// rows.x_error_bars_shared(names, Arc::from([1.0, 2.0]), high, ErrorBarOptions::default())?;
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn x_error_bars_shared(
        &mut self,
        y: AxisData,
        low: Arc<[f64]>,
        high: Arc<[f64]>,
        options: ErrorBarOptions,
    ) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.x_error_bars()";
        let axes = self.axes_mut();
        check_not_two_categories(CONTEXT, axes.x, axes.y)?;
        check_x_error_bars(CONTEXT, axes.x, axes.y, &y, (&low, &high))?;
        axes.marks.push(Mark::ErrorBars(ErrorBarMark::new(
            ErrorBarOrientation::Horizontal,
            y,
            low,
            high,
            options,
        )));
        Ok(self)
    }

    /// Sets the color range of the axes' color-mapped marks: values outside it take the
    /// colormap's end colors. Without it, the range is the extent of their finite values. On
    /// a centered color scale, the range is symmetric about the center and reaches the
    /// farther of the two limits.
    ///
    /// # Errors
    ///
    /// Errors leave the axes unchanged. `Error::Style` unless both limits are finite, at most
    /// 1e300 in magnitude, `low < high`, and the range is at least
    /// `max(1e-10 * max(|low|, |high|), 1e-290)` wide; on a log color scale unless both are at
    /// least 1e-300; on a centered color scale unless the range holds the center.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.clim(0.0, 100.0)?;
    /// assert_eq!(
    ///     axes.clim(5.0, 2.0).unwrap_err().to_string(),
    ///     "Axes.clim(): low is 5, high is 2; low must be below high."
    /// );
    /// assert_eq!(axes.color_limits(), Some((0.0, 100.0)));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn clim(&mut self, low: f64, high: f64) -> Result<&mut Self> {
        const CONTEXT: &str = "Axes.clim()";
        crate::color_scale::check_limits(CONTEXT, (low, high))?;
        if let Some(spec) = self.marks().iter().find_map(Mark::color_scale) {
            spec.check_range((low, high), CONTEXT, None)?;
        }
        self.axes_mut().color_limits = Some((low, high));
        Ok(self)
    }
}
