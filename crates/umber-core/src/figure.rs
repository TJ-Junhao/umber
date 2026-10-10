//! The figure model: a [`Figure`] holds one [`Axes`], which holds [`Mark`]s.
//!
//! Every input is validated at the call that receives it, so a model that exists is always
//! drawable. Marks own copies of their data. Colors stay as [`ColorSpec`]s until
//! [`Figure::resolved_colors`] resolves them against the theme's palette.
//!
//! # Examples
//!
//! ```
//! use umber_core::figure::{AxisData, Figure, LineOptions};
//! use umber_core::{ScaleKind, Theme};
//!
//! let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
//! let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
//! axes.title("Growth")
//!     .line(AxisData::Numbers(vec![0.0, 1.0, 2.0]), &[1.0, 3.0, 2.0], LineOptions::default())?;
//! assert_eq!(figure.axes().map(|a| a.marks().len()), Some(1));
//! # Ok::<(), umber_core::Error>(())
//! ```

use crate::render::{SvgText, render_png, render_svg};
use crate::scale::{MAX_MAGNITUDE, MIN_LOG_VALUE};
use crate::scene::Scene;
use crate::{
    ColorSpec, ContinuousScale, Dash, Error, FontLibrary, Marker, Result, Rgba, ScaleKind, Theme,
    Transform, draw, layout,
};

/// The largest accepted figure side, in inches.
const MAX_FIGURE_IN: f64 = 200.0;

/// The x values of a mark: numbers for a continuous axis, names for a category axis.
///
/// # Examples
///
/// ```
/// use umber_core::figure::AxisData;
///
/// let names = AxisData::Categories(vec!["a".into(), "b".into()]);
/// assert_ne!(names, AxisData::Numbers(vec![0.0, 1.0]));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum AxisData {
    /// Values on a linear or log axis. NaN marks a gap.
    Numbers(Vec<f64>),
    /// Category names on a category axis.
    Categories(Vec<String>),
}

impl AxisData {
    fn len(&self) -> usize {
        match self {
            AxisData::Numbers(v) => v.len(),
            AxisData::Categories(v) => v.len(),
        }
    }
}

/// Options of a line mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::LineOptions;
/// use umber_core::{ColorSpec, Dash};
///
/// let options = LineOptions::default();
/// assert_eq!((options.color, options.dash), (ColorSpec::AUTO, Dash::Solid));
/// assert!(options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LineOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The line color.
    pub color: ColorSpec,
    /// The dash style.
    pub dash: Dash,
}

impl Default for LineOptions {
    fn default() -> Self {
        LineOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
            dash: Dash::Solid,
        }
    }
}

/// Options of a scatter mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::ScatterOptions;
/// use umber_core::{ColorSpec, Marker};
///
/// let options = ScatterOptions::default();
/// assert_eq!((options.color, options.marker), (ColorSpec::AUTO, Marker::Circle));
/// assert!(options.label.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ScatterOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The marker color.
    pub color: ColorSpec,
    /// The marker shape.
    pub marker: Marker,
}

impl Default for ScatterOptions {
    fn default() -> Self {
        ScatterOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
            marker: Marker::Circle,
        }
    }
}

/// One series drawn in an [`Axes`].
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, LineOptions, Mark};
///
/// let mark = Mark::Line {
///     x: AxisData::Numbers(vec![0.0, 1.0]),
///     y: vec![2.0, 3.0],
///     options: LineOptions::default(),
/// };
/// assert!(matches!(mark, Mark::Line { .. }));
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive] // M5b adds the bar and histogram marks
pub enum Mark {
    /// A polyline through the points; NaN breaks it.
    Line {
        /// The x values.
        x: AxisData,
        /// The y values, as many as `x`.
        y: Vec<f64>,
        /// Label, color and dash.
        options: LineOptions,
    },
    /// Individual markers; NaN skips a point.
    Scatter {
        /// The x values.
        x: AxisData,
        /// The y values, as many as `x`.
        y: Vec<f64>,
        /// Label, color and marker shape.
        options: ScatterOptions,
    },
}

impl Mark {
    fn color(&self) -> ColorSpec {
        match self {
            Mark::Line { options, .. } => options.color,
            Mark::Scatter { options, .. } => options.color,
        }
    }
}

/// A plotting area with its scales, texts, limits and marks.
///
/// Create one with [`Figure::add_axes`].
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
    xlim: Option<(f64, f64)>,
    ylim: Option<(f64, f64)>,
    marks: Vec<Mark>,
}

/// Which axis a value belongs to, for messages.
fn transform_of(kind: ScaleKind) -> Option<Transform> {
    match kind {
        ScaleKind::Linear => Some(Transform::Linear),
        ScaleKind::Log { base } => Some(Transform::Log(base)),
        _ => None,
    }
}

fn data_error(context: &'static str, message: String) -> Error {
    Error::Data { context, message }
}

/// Checks numeric values of the axis `name` (`"x"` or `"y"`) against its scale kind.
fn check_numbers(context: &'static str, name: &str, values: &[f64], kind: ScaleKind) -> Result<()> {
    let log = matches!(kind, ScaleKind::Log { .. });
    for (i, &v) in values.iter().enumerate() {
        if v.is_nan() {
            continue;
        }
        if v.is_infinite() {
            return Err(data_error(
                context,
                format!("{name}[{i}] is {v}; values must be finite (NaN marks a gap)."),
            ));
        }
        if v.abs() > MAX_MAGNITUDE {
            return Err(data_error(
                context,
                format!("{name}[{i}] is {v:e}; magnitudes must not exceed {MAX_MAGNITUDE:e}."),
            ));
        }
        if log {
            if v <= 0.0 {
                return Err(data_error(
                    context,
                    format!("{name}[{i}] is {v} on a log axis; values must be positive."),
                ));
            }
            if v < MIN_LOG_VALUE {
                return Err(data_error(
                    context,
                    format!(
                        "{name}[{i}] is {v:e} on a log axis; values must be at least {MIN_LOG_VALUE:e}."
                    ),
                ));
            }
        }
    }
    Ok(())
}

impl Axes {
    fn new(x: ScaleKind, y: ScaleKind) -> Axes {
        Axes {
            x,
            y,
            title: String::new(),
            subtitle: String::new(),
            xlabel: String::new(),
            ylabel: String::new(),
            xlim: None,
            ylim: None,
            marks: Vec::new(),
        }
    }

    /// Validates a whole series before it is stored, so a failed call changes nothing.
    fn check_series(&self, context: &'static str, x: &AxisData, y: &[f64]) -> Result<()> {
        if x.len() != y.len() {
            return Err(data_error(
                context,
                format!(
                    "x has {} values but y has {}; x and y must have the same length.",
                    x.len(),
                    y.len()
                ),
            ));
        }
        match (x, self.x) {
            (AxisData::Categories(_), ScaleKind::Category) => {}
            (AxisData::Numbers(values), kind) if kind != ScaleKind::Category => {
                check_numbers(context, "x", values, kind)?;
            }
            (AxisData::Numbers(_), _) => {
                return Err(data_error(
                    context,
                    "x holds numbers but the x axis is a category axis; pass category names."
                        .into(),
                ));
            }
            (AxisData::Categories(_), _) => {
                return Err(data_error(
                    context,
                    "x holds category names but the x axis is not a category axis; pass numbers."
                        .into(),
                ));
            }
        }
        if self.y == ScaleKind::Category {
            return Err(data_error(
                context,
                "y holds numbers but the y axis is a category axis; use a linear or log y axis."
                    .into(),
            ));
        }
        check_numbers(context, "y", y, self.y)
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
        self.xlim
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
        self.ylim
    }

    /// Sets the x limits. `Error::Usage` on a category axis; `Error::Style` unless
    /// `low < high`, both are finite and within the supported magnitudes (and positive on a
    /// log axis).
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
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn xlim(&mut self, low: f64, high: f64) -> Result<&mut Axes> {
        self.xlim = Some(Self::check_limits("Axes.xlim()", self.x, low, high)?);
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
        self.ylim = Some(Self::check_limits("Axes.ylim()", self.y, low, high)?);
        Ok(self)
    }

    fn check_limits(
        context: &'static str,
        kind: ScaleKind,
        low: f64,
        high: f64,
    ) -> Result<(f64, f64)> {
        let Some(transform) = transform_of(kind) else {
            return Err(Error::Usage {
                context,
                message: "this is a category axis; limits apply to linear and log axes only."
                    .into(),
            });
        };
        ContinuousScale::new(transform, low, high, context)?;
        Ok((low, high))
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
    pub fn line(&mut self, x: AxisData, y: &[f64], options: LineOptions) -> Result<&mut Axes> {
        self.check_series("Axes.line()", &x, y)?;
        self.marks.push(Mark::Line {
            x,
            y: y.to_vec(),
            options,
        });
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
        self.check_series("Axes.scatter()", &x, y)?;
        self.marks.push(Mark::Scatter {
            x,
            y: y.to_vec(),
            options,
        });
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

/// A figure: a size, a theme and at most one [`Axes`].
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Theme};
///
/// let figure = Figure::new(6.0, 4.0, Theme::graphite())?;
/// assert_eq!((figure.width_in(), figure.height_in()), (6.0, 4.0));
/// assert!(figure.axes().is_none());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Figure {
    width_in: f64,
    height_in: f64,
    theme: Theme,
    axes: Option<Axes>,
}

impl Figure {
    /// Creates an empty figure. Sides are in inches; `Error::Style` unless each is finite,
    /// above 0 and at most 200.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// assert!(Figure::new(6.0, 4.0, Theme::pigment()).is_ok());
    /// assert!(Figure::new(0.0, 4.0, Theme::pigment()).is_err());
    /// ```
    pub fn new(width_in: f64, height_in: f64, theme: Theme) -> Result<Figure> {
        for (name, v) in [("width", width_in), ("height", height_in)] {
            if !(v.is_finite() && v > 0.0 && v <= MAX_FIGURE_IN) {
                return Err(Error::Style {
                    context: "Figure()",
                    message: format!(
                        "{name} is {v}; it must be finite, above 0 and at most {MAX_FIGURE_IN} inches."
                    ),
                });
            }
        }
        Ok(Figure {
            width_in,
            height_in,
            theme,
            axes: None,
        })
    }

    /// Adds the figure's axes. `Error::Usage` if it already has some (v0.1 allows one).
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// assert!(figure.add_axes(ScaleKind::Linear, ScaleKind::Linear).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn add_axes(&mut self, x: ScaleKind, y: ScaleKind) -> Result<&mut Axes> {
        if self.axes.is_some() {
            return Err(Error::Usage {
                context: "Figure.add_axes()",
                message: "the figure already has axes; v0.1 supports one axes per figure.".into(),
            });
        }
        Ok(self.axes.insert(Axes::new(x, y)))
    }

    /// The axes, if added.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// assert!(Figure::new(6.0, 4.0, Theme::pigment())?.axes().is_none());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn axes(&self) -> Option<&Axes> {
        self.axes.as_ref()
    }

    /// The axes, mutably, if added.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// figure.axes_mut().map(|a| a.title("again"));
    /// assert_eq!(figure.axes().map(|a| a.title_text()), Some("again"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn axes_mut(&mut self) -> Option<&mut Axes> {
        self.axes.as_mut()
    }

    /// The theme.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// let figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// assert_eq!(figure.theme(), &Theme::midnight());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// The width in inches.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// assert_eq!(Figure::new(6.0, 4.0, Theme::pigment())?.width_in(), 6.0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn width_in(&self) -> f64 {
        self.width_in
    }

    /// The height in inches.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Theme};
    ///
    /// assert_eq!(Figure::new(6.0, 4.0, Theme::pigment())?.height_in(), 4.0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn height_in(&self) -> f64 {
        self.height_in
    }

    /// Each mark's color, resolved against the theme's palette, in mark order. `Auto` colors
    /// take palette slots in series order; `Palette` and `Fixed` colors do not consume a slot.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::figure::{AxisData, LineOptions};
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.line(AxisData::Numbers(vec![0.0]), &[1.0], LineOptions::default())?;
    /// assert_eq!(figure.resolved_colors(), vec![figure.theme().palette()[0]]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn resolved_colors(&self) -> Vec<Rgba> {
        let Some(axes) = &self.axes else {
            return Vec::new();
        };
        let palette = self.theme.palette();
        let mut next_auto = 0_usize;
        axes.marks
            .iter()
            .map(|mark| {
                let spec = mark.color();
                let color = spec.resolve(palette, next_auto);
                if matches!(spec, ColorSpec::Auto(_)) {
                    next_auto += 1;
                }
                color
            })
            .collect()
    }

    /// Builds the figure's [`Scene`]: resolve scales and ticks, lay out the axes, then draw.
    /// The scene is `width × 72` by `height × 72` pt on the theme background. Without axes it
    /// holds the background only. The same figure always gives the same scene.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, FontLibrary, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert!(figure.to_scene(FontLibrary::global()).items.is_empty());
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.title("Growth");
    /// let scene = figure.to_scene(FontLibrary::global());
    /// assert_eq!((scene.width, scene.height), (432.0, 288.0));
    /// assert!(scene.leaf_count() > 0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn to_scene(&self, library: &FontLibrary) -> Scene {
        let (width, height) = (self.width_in * 72.0, self.height_in * 72.0);
        let mut scene = Scene::new(width as f32, height as f32, self.theme.background());
        let Some(axes) = &self.axes else {
            return scene;
        };
        let (_resolved, layout) = layout::plan(width, height, &self.theme, axes, library);
        scene.push(draw::chrome(&self.theme, &layout));
        scene
    }

    /// Renders the figure as a PNG at `dpi`, with the global [`FontLibrary`]. Errors as
    /// [`render_png`] does: `Error::Style` when `dpi` is outside `10..=2400` or the image would
    /// be too large.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(2.0, 1.0, Theme::graphite())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let png = figure.to_png(72)?;
    /// assert_eq!(&png[1..4], b"PNG");
    /// assert!(figure.to_png(1).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn to_png(&self, dpi: u32) -> Result<Vec<u8>> {
        render_png(&self.to_scene(FontLibrary::global()), dpi)
    }

    /// Renders the figure as an SVG document, with the global [`FontLibrary`].
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, SvgText, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.title("Signal");
    /// let svg = figure.to_svg(SvgText::Editable);
    /// assert!(svg.contains(r#"width="432pt" height="288pt""#));
    /// assert!(svg.contains(">Signal</text>"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn to_svg(&self, text: SvgText) -> String {
        render_svg(&self.to_scene(FontLibrary::global()), text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Alpha, LogBase};

    fn figure() -> Figure {
        Figure::new(6.0, 4.0, Theme::pigment()).unwrap()
    }

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    fn log_kind() -> ScaleKind {
        ScaleKind::Log { base: LogBase::TEN }
    }

    /// Asserts a `Data` error from `context` whose message contains `needle`.
    #[track_caller]
    fn assert_data<T: std::fmt::Debug>(r: Result<T>, context: &str, needle: &str) {
        match r {
            Err(Error::Data {
                context: c,
                message,
            }) => {
                assert_eq!(c, context);
                assert!(message.contains(needle), "{message}");
            }
            other => panic!("expected Data error, got {other:?}"),
        }
    }

    #[track_caller]
    fn assert_style<T: std::fmt::Debug>(r: Result<T>, context: &str) {
        match r {
            Err(Error::Style { context: c, .. }) => assert_eq!(c, context),
            other => panic!("expected Style error, got {other:?}"),
        }
    }

    #[track_caller]
    fn assert_usage<T: std::fmt::Debug>(r: Result<T>, context: &str) {
        match r {
            Err(Error::Usage { context: c, .. }) => assert_eq!(c, context),
            other => panic!("expected Usage error, got {other:?}"),
        }
    }

    #[test]
    fn figure_size_is_validated() {
        for (w, h) in [
            (f64::NAN, 4.0),
            (6.0, f64::INFINITY),
            (0.0, 4.0),
            (6.0, -1.0),
            (200.5, 4.0),
            (6.0, 201.0),
        ] {
            assert_style(Figure::new(w, h, Theme::pigment()), "Figure()");
        }
        assert!(Figure::new(200.0, 0.01, Theme::pigment()).is_ok());
    }

    #[test]
    fn second_add_axes_is_a_usage_error() {
        let mut f = figure();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_usage(
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear),
            "Figure.add_axes()",
        );
    }

    #[test]
    fn length_mismatch_is_a_data_error() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            a.line(nums(&[0.0, 1.0, 2.0]), &[1.0, 2.0], LineOptions::default()),
            "Axes.line()",
            "x has 3 values but y has 2",
        );
        assert_data(
            a.scatter(nums(&[0.0]), &[1.0, 2.0], ScatterOptions::default()),
            "Axes.scatter()",
            "x has 1 values but y has 2",
        );
        assert!(a.marks().is_empty());
    }

    #[test]
    fn infinite_values_are_data_errors() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            a.line(
                nums(&[0.0, 1.0, 2.0, f64::INFINITY]),
                &[0.0; 4],
                LineOptions::default(),
            ),
            "Axes.line()",
            "x[3] is inf; values must be finite (NaN marks a gap).",
        );
        assert_data(
            a.scatter(
                nums(&[0.0, 1.0]),
                &[0.0, f64::NEG_INFINITY],
                ScatterOptions::default(),
            ),
            "Axes.scatter()",
            "y[1] is -inf",
        );
        assert!(a.marks().is_empty());
    }

    #[test]
    fn huge_magnitudes_are_data_errors() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            a.line(nums(&[0.0, 1e301]), &[0.0, 0.0], LineOptions::default()),
            "Axes.line()",
            "x[1] is 1e301",
        );
        assert_data(
            a.line(nums(&[0.0]), &[-1e301], LineOptions::default()),
            "Axes.line()",
            "y[0] is -1e301",
        );
        assert!(
            a.line(nums(&[0.0, 1e300]), &[-1e300, 0.0], LineOptions::default())
                .is_ok()
        );
    }

    #[test]
    fn non_positive_values_on_a_log_axis_are_data_errors() {
        let mut f = figure();
        let a = f.add_axes(log_kind(), log_kind()).unwrap();
        assert_data(
            a.line(nums(&[1.0, 0.0]), &[1.0, 1.0], LineOptions::default()),
            "Axes.line()",
            "x[1] is 0 on a log axis",
        );
        assert_data(
            a.scatter(nums(&[1.0, 2.0]), &[1.0, -3.0], ScatterOptions::default()),
            "Axes.scatter()",
            "y[1] is -3 on a log axis",
        );
        // NaN is ignored.
        assert!(
            a.line(
                nums(&[1.0, f64::NAN]),
                &[f64::NAN, 2.0],
                LineOptions::default()
            )
            .is_ok()
        );
    }

    #[test]
    fn log_axis_values_outside_the_log_range_are_data_errors() {
        let mut f = figure();
        let a = f.add_axes(log_kind(), log_kind()).unwrap();
        assert_data(
            a.line(nums(&[1.0, 1e-301]), &[1.0, 1.0], LineOptions::default()),
            "Axes.line()",
            "x[1] is 1e-301 on a log axis",
        );
        assert_data(
            a.line(nums(&[1.0]), &[1e301], LineOptions::default()),
            "Axes.line()",
            "y[0] is 1e301",
        );
        assert!(
            a.line(
                nums(&[1e-300, 1e300]),
                &[1e-300, 1e300],
                LineOptions::default()
            )
            .is_ok()
        );
    }

    #[test]
    fn axis_data_kind_must_match_the_axis() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
        assert_data(
            a.line(nums(&[0.0]), &[1.0], LineOptions::default()),
            "Axes.line()",
            "category axis",
        );
        assert!(
            a.line(
                AxisData::Categories(vec!["a".into()]),
                &[1.0],
                LineOptions::default()
            )
            .is_ok()
        );

        let mut g = figure();
        let b = g.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        assert_data(
            b.scatter(
                AxisData::Categories(vec!["a".into()]),
                &[1.0],
                ScatterOptions::default(),
            ),
            "Axes.scatter()",
            "not a category axis",
        );
        assert!(b.marks().is_empty());
    }

    #[test]
    fn numeric_y_on_a_category_y_axis_is_a_data_error() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Category).unwrap();
        assert_data(
            a.line(nums(&[0.0]), &[1.0], LineOptions::default()),
            "Axes.line()",
            "y axis is a category axis",
        );
    }

    #[test]
    fn invalid_limits_are_style_errors() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        for (lo, hi) in [
            (1.0, 1.0),
            (2.0, 1.0),
            (f64::NAN, 1.0),
            (0.0, f64::INFINITY),
            (f64::NEG_INFINITY, 0.0),
            (0.0, 1e301),
            (-1e301, 0.0),
        ] {
            assert_style(a.xlim(lo, hi), "Axes.xlim()");
            assert_style(a.ylim(lo, hi), "Axes.ylim()");
        }
        assert_eq!(a.x_limits(), None);
        assert_eq!(a.y_limits(), None);
        a.xlim(-1e300, 1e300).unwrap().ylim(0.0, 1.0).unwrap();
        assert_eq!(a.x_limits(), Some((-1e300, 1e300)));
        assert_eq!(a.y_limits(), Some((0.0, 1.0)));
    }

    #[test]
    fn non_positive_or_out_of_range_limits_on_a_log_axis_are_style_errors() {
        let mut f = figure();
        let a = f.add_axes(log_kind(), log_kind()).unwrap();
        assert_style(a.xlim(0.0, 10.0), "Axes.xlim()");
        assert_style(a.xlim(-5.0, 10.0), "Axes.xlim()");
        assert_style(a.ylim(1e-301, 10.0), "Axes.ylim()");
        assert_style(a.ylim(1.0, 1e301), "Axes.ylim()");
        assert!(a.xlim(1e-300, 1e300).is_ok());
    }

    #[test]
    fn limits_on_a_category_axis_are_usage_errors() {
        let mut f = figure();
        let a = f
            .add_axes(ScaleKind::Category, ScaleKind::Category)
            .unwrap();
        assert_usage(a.xlim(0.0, 1.0), "Axes.xlim()");
        assert_usage(a.ylim(0.0, 1.0), "Axes.ylim()");
        // Usage wins over invalid values.
        assert_usage(a.xlim(1.0, 0.0), "Axes.xlim()");
    }

    #[test]
    fn nan_is_allowed_in_lines_and_scatters() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.line(
            nums(&[0.0, f64::NAN, 2.0]),
            &[f64::NAN, 1.0, 2.0],
            LineOptions::default(),
        )
        .unwrap()
        .scatter(nums(&[f64::NAN]), &[f64::NAN], ScatterOptions::default())
        .unwrap();
        assert_eq!(a.marks().len(), 2);
    }

    #[test]
    fn auto_colors_follow_series_order() {
        let mut f = figure();
        let palette = f.theme().palette().to_vec();
        let fixed = Rgba::opaque(1, 2, 3);
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let specs = [
            ColorSpec::AUTO,
            ColorSpec::Fixed(fixed),
            ColorSpec::AUTO,
            ColorSpec::Palette {
                index: 5,
                alpha: Alpha::OPAQUE,
            },
            ColorSpec::AUTO,
        ];
        for color in specs {
            a.line(
                nums(&[0.0]),
                &[1.0],
                LineOptions {
                    color,
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        assert_eq!(
            f.resolved_colors(),
            vec![palette[0], fixed, palette[1], palette[5], palette[2]]
        );
    }

    #[test]
    fn auto_colors_count_lines_and_scatters_together() {
        let mut f = figure();
        let palette = f.theme().palette().to_vec();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.scatter(nums(&[0.0]), &[1.0], ScatterOptions::default())
            .unwrap()
            .line(nums(&[0.0]), &[1.0], LineOptions::default())
            .unwrap();
        assert_eq!(f.resolved_colors(), vec![palette[0], palette[1]]);
        assert!(figure().resolved_colors().is_empty());
    }

    #[test]
    fn alpha_on_auto_is_applied() {
        let mut f = figure();
        let first = f.theme().palette()[0];
        let alpha = Alpha::new(0.5).unwrap();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.line(
            nums(&[0.0]),
            &[1.0],
            LineOptions {
                color: ColorSpec::AUTO.with_alpha(alpha),
                ..LineOptions::default()
            },
        )
        .unwrap();
        assert_eq!(f.resolved_colors(), vec![first.with_alpha(alpha)]);
        assert_eq!(f.resolved_colors()[0].a, alpha.to_u8());
    }

    #[test]
    fn marks_store_copies_of_the_input() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let mut y = vec![1.0, 2.0];
        let x = nums(&[0.0, 1.0]);
        a.line(x.clone(), &y, LineOptions::default()).unwrap();
        y[0] = 99.0;
        match a.marks() {
            [Mark::Line { x: mx, y: my, .. }] => {
                assert_eq!(my, &vec![1.0, 2.0]);
                assert_eq!(mx, &x);
            }
            other => panic!("unexpected marks {other:?}"),
        }
    }

    #[test]
    fn text_setters_store_text() {
        let mut f = figure();
        let a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.title("t").subtitle("s").xlabel("x").ylabel("y");
        assert_eq!(
            (
                a.title_text(),
                a.subtitle_text(),
                a.xlabel_text(),
                a.ylabel_text()
            ),
            ("t", "s", "x", "y")
        );
    }
}
