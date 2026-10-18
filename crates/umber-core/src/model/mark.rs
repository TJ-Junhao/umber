//! Marks and their options.

use std::fmt;

use super::axis_data::{AxisData, XySeries};
use super::band::BandMark;
use super::bar::{BarGroupMark, BarMark};
use super::box_plot::{BoxGroupMark, BoxMark};
use super::color_by::PointColor;
use super::error_bars::ErrorBarMark;
use super::heatmap::HeatmapMark;
use super::hist::HistMark;
use super::rule::{HLineMark, SpanMark, VLineMark};
use super::stacked_area::StackedAreaMark;
use super::stem::StemMark;
use super::step::StepMark;
use super::violin::{ViolinGroupMark, ViolinMark};
use crate::color_scale::ColorScaleSpec;
use crate::{ColorSpec, Dash, Marker, SizeSpec};

/// Options of a line mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::LineOptions;
/// use umber_core::{ColorSpec, Dash, SizeSpec};
///
/// let options = LineOptions::default();
/// assert_eq!((options.color, options.dash), (ColorSpec::AUTO, Dash::Solid));
/// assert_eq!(options.width, SizeSpec::Auto);
/// assert!(options.label.is_empty());
///
/// let thick = LineOptions { width: SizeSpec::times(2.0)?, ..LineOptions::default() };
/// assert_ne!(thick, options);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LineOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The line color.
    pub color: ColorSpec,
    /// The dash style.
    pub dash: Dash,
    /// The line width; [`SizeSpec::Auto`] for the theme's line width. Dashes scale with it.
    pub width: SizeSpec,
}

impl Default for LineOptions {
    fn default() -> Self {
        LineOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
            dash: Dash::Solid,
            width: SizeSpec::Auto,
        }
    }
}

/// Options of an area mark: the options of the line that bounds it, so an area takes exactly
/// the label, color, dash and width of a line.
///
/// # Examples
///
/// ```
/// use umber_core::figure::AreaOptions;
/// use umber_core::{ColorSpec, Dash};
///
/// let options = AreaOptions::default();
/// assert_eq!((options.color, options.dash), (ColorSpec::AUTO, Dash::Solid));
/// assert!(options.label.is_empty());
/// ```
pub type AreaOptions = LineOptions;

/// Options of a scatter mark.
///
/// # Examples
///
/// ```
/// use umber_core::figure::ScatterOptions;
/// use umber_core::{Marker, PointColor, SizeSpec};
///
/// let options = ScatterOptions::default();
/// assert_eq!((&options.color, options.marker), (&PointColor::AUTO, Marker::Circle));
/// assert_eq!(options.size, SizeSpec::Auto);
/// assert!(options.label.is_empty());
///
/// let small = ScatterOptions { size: SizeSpec::pt(3.0)?, ..ScatterOptions::default() };
/// assert_ne!(small, options);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ScatterOptions {
    /// The legend label; empty for none.
    pub label: String,
    /// The marker color: one color, or a color per point mapped from values. A color-mapped
    /// scatter has no legend entry, so its label must be empty.
    pub color: PointColor,
    /// The marker shape.
    pub marker: Marker,
    /// The marker diameter; [`SizeSpec::Auto`] for the theme's marker size.
    pub size: SizeSpec,
}

impl Default for ScatterOptions {
    fn default() -> Self {
        ScatterOptions {
            label: String::new(),
            color: PointColor::AUTO,
            marker: Marker::Circle,
            size: SizeSpec::Auto,
        }
    }
}

/// One series drawn in an [`Axes`](super::Axes).
///
/// Marks are created by [`AxesMut::line`](super::AxesMut::line),
/// [`AxesMut::scatter`](super::AxesMut::scatter), [`AxesMut::area`](super::AxesMut::area),
/// [`AxesMut::bar`](super::AxesMut::bar), [`AxesMut::grouped_bar`](super::AxesMut::grouped_bar),
/// [`AxesMut::stacked_bar`](super::AxesMut::stacked_bar),
/// [`AxesMut::stacked_area`](super::AxesMut::stacked_area),
/// [`AxesMut::hist`](super::AxesMut::hist), [`AxesMut::step`](super::AxesMut::step),
/// [`AxesMut::hline`](super::AxesMut::hline), [`AxesMut::vline`](super::AxesMut::vline),
/// [`AxesMut::hspan`](super::AxesMut::hspan), [`AxesMut::vspan`](super::AxesMut::vspan),
/// [`AxesMut::band`](super::AxesMut::band), [`AxesMut::y_error_bars`](super::AxesMut::y_error_bars)
/// [`AxesMut::x_error_bars`](super::AxesMut::x_error_bars),
/// [`AxesMut::box_plot`](super::AxesMut::box_plot),
/// [`AxesMut::grouped_box_plot`](super::AxesMut::grouped_box_plot),
/// [`AxesMut::heatmap`](super::AxesMut::heatmap) and
/// [`AxesMut::contour`](super::AxesMut::contour),
/// which validate the data. Each variant holds a payload ([`LineMark`], [`ScatterMark`], [`AreaMark`],
/// [`BarMark`], [`BarGroupMark`], [`HistMark`], [`StepMark`], [`HLineMark`], [`VLineMark`],
/// [`SpanMark`], [`BandMark`], [`StackedAreaMark`], [`ErrorBarMark`], [`BoxMark`], [`BoxGroupMark`],
/// [`HeatmapMark`])
/// that keeps the validated values
/// together and exposes them read-only; it has no
/// public constructor. Read a mark by matching on its variant.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, LineOptions, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.line(AxisData::Numbers(vec![0.0, 1.0]), &[2.0, 3.0], LineOptions::default())?;
/// match axes.marks() {
///     [Mark::Line(line)] => {
///         assert_eq!(line.x(), &AxisData::Numbers(vec![0.0, 1.0]));
///         assert_eq!(line.y(), &AxisData::Numbers(vec![2.0, 3.0]));
///         assert!(line.options().label.is_empty());
///     }
///     other => panic!("unexpected marks {other:?}"),
/// }
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A mark cannot be built outside [`Axes`](super::Axes), so its data is always validated:
///
/// ```compile_fail,E0451
/// use umber_core::figure::{LineMark, LineOptions, Mark};
///
/// let mark = Mark::Line(LineMark {
///     series: unreachable!(),
///     options: LineOptions::default(),
/// });
/// ```
///
/// Nor can it be assembled from loose x and y values:
///
/// ```compile_fail,E0559
/// use umber_core::figure::{AxisData, LineOptions, Mark};
///
/// let mark = Mark::Line {
///     x: AxisData::Numbers(vec![0.0]),
///     y: vec![1.0, 2.0],
///     options: LineOptions::default(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive] // further mark kinds are added as variants
pub enum Mark {
    /// Markers whose areas follow non-negative size values; zero draws nothing.
    Bubbles(super::bubbles::BubbleMark),
    /// Violin densities, one per category.
    Violin(ViolinMark),
    /// Violin distributions side by side within each category.
    GroupedViolin(ViolinGroupMark),
    /// Box plots, one per category.
    Box(BoxMark),
    /// Distributions side by side within each category.
    GroupedBox(BoxGroupMark),
    /// A polyline through the points; NaN breaks it.
    Line(LineMark),
    /// Individual markers; NaN skips a point.
    Scatter(ScatterMark),
    /// A line with the region between it and the baseline filled; NaN breaks both the line and
    /// the fill.
    Area(AreaMark),
    /// One bar per category, from the baseline to its height; NaN draws no bar.
    Bar(BarMark),
    /// One bar per series in each category, side by side within the category's band; NaN draws
    /// no bar.
    GroupedBar(BarGroupMark),
    /// One stack per category, its series stacked in order, positive heights upward and
    /// negative ones downward from the baseline; NaN and 0 add no segment.
    StackedBar(BarGroupMark),
    /// Non-negative series stacked in order, filled flat and outlined on top; a NaN in any
    /// series breaks every layer.
    StackedArea(StackedAreaMark),
    /// The values counted in equal-width bins, one bar per bin from the baseline to its count;
    /// NaN is ignored and an empty bin draws nothing.
    Hist(HistMark),
    /// Horizontal and vertical segments through the points, which change level at the chosen
    /// [`Step`](crate::Step) position; NaN breaks the line.
    Step(StepMark),
    /// A horizontal line across the whole plot at a y value. It is drawn above all data marks.
    HLine(HLineMark),
    /// A vertical line across the plot at an x value, or at a category's band centre; it ends
    /// at the baseline when that sits at the y axis's lower limit. It is drawn above all data
    /// marks.
    VLine(VLineMark),
    /// The region between two y values, across the whole plot, at the area opacity. It is drawn
    /// below all data marks.
    HSpan(SpanMark),
    /// The region between two x values, across the plot (down to the baseline when that sits at
    /// the y axis's lower limit), at the area opacity. It is drawn below all data marks.
    VSpan(SpanMark),
    /// The region between a lower and an upper curve, filled flat at the area opacity with no
    /// outline; NaN in x, low or high breaks it.
    Band(BandMark),
    /// Vertical or horizontal bars from a lower to an upper limit, with optional caps; a NaN
    /// in the position or a limit skips the bar.
    ErrorBars(ErrorBarMark),
    /// A line from the baseline to each value, with a scatter marker at the value; NaN skips
    /// the point.
    Stem(StemMark),
    /// One cell per pair of an x and a y category, filled with the color its value maps to;
    /// NaN leaves the cell empty.
    Heatmap(HeatmapMark),
    /// Numeric cells filled by value, spanning their edges exactly.
    Mesh(super::mesh::MeshMark),
    /// Non-empty equal-width bins of paired numeric coordinates.
    Hist2d(super::mesh::Hist2dMark),
    /// Paired numeric coordinates counted in regular pointy-top hexagons.
    Hexbin(super::mesh::HexbinMark),
    /// A field on a rectilinear grid, drawn as lines along levels, filled bands between them,
    /// or both; NaN cuts the cells around it out.
    Contour(super::contour::ContourMark),
    /// Short vertical event ticks in category rows.
    Events(super::EventsMark),
    /// Circular samples jittered within their category bands.
    Strip(super::StripMark),
    /// Circular samples packed without overlap within their category bands.
    Swarm(super::SwarmMark),
    /// Several series of values counted in shared bins, drawn as outlines or stacked bars;
    /// NaN is ignored.
    GroupedHist(super::hist::HistGroupMark),
    /// The empirical cumulative distribution of the values, as a step line from 0 to 1 (or from
    /// 1 to 0); NaN is left out.
    Ecdf(super::ecdf::EcdfMark),
    /// The Gaussian kernel density of the values, as a line with area 1; NaN is left out.
    Kde(super::kde::KdeMark),
}

impl Mark {
    /// A line through a validated series.
    pub(super) fn line(series: XySeries, options: LineOptions) -> Mark {
        Mark::Line(LineMark::new(series, options))
    }

    /// Markers at the points of a validated series.
    pub(super) fn scatter(series: XySeries, options: ScatterOptions) -> Mark {
        Mark::Scatter(ScatterMark { series, options })
    }

    /// An area under a validated series.
    pub(super) fn area(series: XySeries, options: AreaOptions) -> Mark {
        Mark::Area(AreaMark {
            line: LineMark { series, options },
        })
    }

    /// The colormap, scale and center the mark maps its colors with; `None` for a mark that is
    /// not color-mapped.
    pub(super) fn color_scale(&self) -> Option<ColorScaleSpec> {
        let color = match self {
            Mark::Mesh(mark) => return Some(ColorScaleSpec::of(&mark.colors)),
            Mark::Hist2d(mark) => return Some(mark.colors.clone()),
            Mark::Hexbin(mark) => return Some(mark.colors.clone()),
            Mark::Scatter(mark) => &mark.options.color,
            Mark::Bubbles(mark) => &mark.options().color,
            Mark::Violin(_)
            | Mark::GroupedViolin(_)
            | Mark::Box(_)
            | Mark::GroupedBox(_)
            | Mark::Line(_)
            | Mark::Area(_)
            | Mark::Bar(_)
            | Mark::GroupedBar(_)
            | Mark::StackedBar(_)
            | Mark::StackedArea(_)
            | Mark::Hist(_)
            | Mark::Step(_)
            | Mark::HLine(_)
            | Mark::VLine(_)
            | Mark::HSpan(_)
            | Mark::VSpan(_)
            | Mark::Band(_)
            | Mark::ErrorBars(_)
            | Mark::Stem(_) => return None,
            Mark::Heatmap(mark) => return Some(mark.color_scale()),
            Mark::Contour(mark) => return mark.color_scale(),
            Mark::Events(_) | Mark::Strip(_) | Mark::Swarm(_) => return None,
            Mark::GroupedHist(_) | Mark::Ecdf(_) | Mark::Kde(_) => return None,
        };
        match color {
            PointColor::Uniform(_) => None,
            PointColor::By(by) => Some(ColorScaleSpec::of(by)),
        }
    }
}

/// A line mark: a validated series and its [`LineOptions`].
///
/// The x and y values are stored together and never change; clones share them.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, LineOptions, Mark};
/// use umber_core::{Dash, Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Category, ScaleKind::Linear)?;
/// axes.line(AxisData::Categories(vec!["a".into()]), &[1.0], LineOptions::default())?;
/// let Some(Mark::Line(line)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(line.x(), &AxisData::Categories(vec!["a".into()]));
/// assert_eq!(line.y(), &AxisData::Numbers(vec![1.0]));
/// assert_eq!(line.options().dash, Dash::Solid);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// The x values of one line cannot be paired with the y values of another:
///
/// ```compile_fail,E0616
/// use umber_core::figure::{AxisData, LineOptions, Mark};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.line(AxisData::Numbers(vec![0.0]), &[1.0], LineOptions::default())?
///     .line(AxisData::Numbers(vec![0.0, 1.0]), &[1.0, 2.0], LineOptions::default())?;
/// let [Mark::Line(short), Mark::Line(long)] = axes.marks() else { unreachable!() };
/// let mut mixed = short.clone();
/// mixed.series = long.series.clone();
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Clone, PartialEq)]
pub struct LineMark {
    series: XySeries,
    options: LineOptions,
}

impl LineMark {
    /// A line mark over a validated series.
    pub(super) fn new(series: XySeries, options: LineOptions) -> LineMark {
        LineMark { series, options }
    }

    /// The x values.
    #[must_use]
    pub fn x(&self) -> &AxisData {
        self.series.x()
    }

    /// The y values, as many as the x values.
    #[must_use]
    pub fn y(&self) -> &AxisData {
        self.series.y()
    }

    /// Label, color, dash and width.
    #[must_use]
    pub fn options(&self) -> &LineOptions {
        &self.options
    }

    /// The validated series.
    #[cfg(test)]
    pub(super) fn series(&self) -> &XySeries {
        &self.series
    }
}

impl fmt::Debug for LineMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LineMark")
            .field("x", self.x())
            .field("y", &self.y())
            .field("options", &self.options)
            .finish()
    }
}

/// A scatter mark: a validated series and its [`ScatterOptions`].
///
/// The x and y values are stored together and never change; clones share them.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, Mark, ScatterOptions};
/// use umber_core::{Figure, Marker, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.scatter(AxisData::Numbers(vec![0.0, 1.0]), &[2.0, 3.0], ScatterOptions::default())?;
/// let Some(Mark::Scatter(scatter)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(scatter.x(), &AxisData::Numbers(vec![0.0, 1.0]));
/// assert_eq!(scatter.y(), &AxisData::Numbers(vec![2.0, 3.0]));
/// assert_eq!(scatter.options().marker, Marker::Circle);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A scatter mark cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{Mark, ScatterMark, ScatterOptions};
///
/// let mark = Mark::Scatter(ScatterMark {
///     series: unreachable!(),
///     options: ScatterOptions::default(),
/// });
/// ```
///
/// The x values of one scatter mark cannot be paired with the y values of another:
///
/// ```compile_fail,E0616
/// use umber_core::figure::{AxisData, Mark, ScatterOptions};
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.scatter(AxisData::Numbers(vec![0.0]), &[1.0], ScatterOptions::default())?
///     .scatter(AxisData::Numbers(vec![0.0, 1.0]), &[1.0, 2.0], ScatterOptions::default())?;
/// let [Mark::Scatter(short), Mark::Scatter(long)] = axes.marks() else { unreachable!() };
/// let mut mixed = short.clone();
/// mixed.series = long.series.clone();
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Clone, PartialEq)]
pub struct ScatterMark {
    series: XySeries,
    options: ScatterOptions,
}

impl ScatterMark {
    /// The x values.
    #[must_use]
    pub fn x(&self) -> &AxisData {
        self.series.x()
    }

    /// The y values, as many as the x values.
    #[must_use]
    pub fn y(&self) -> &AxisData {
        self.series.y()
    }

    /// Label, color, marker shape and size.
    #[must_use]
    pub fn options(&self) -> &ScatterOptions {
        &self.options
    }

    /// The validated series.
    #[cfg(test)]
    pub(super) fn series(&self) -> &XySeries {
        &self.series
    }
}

impl fmt::Debug for ScatterMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScatterMark")
            .field("x", self.x())
            .field("y", &self.y())
            .field("options", &self.options)
            .finish()
    }
}

/// An area mark: the line that bounds it, a [`LineMark`] in every respect, whose region down to
/// the baseline is filled.
///
/// The x and y values are stored together and never change; clones share them.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AreaOptions, AxisData, Mark};
/// use umber_core::{Dash, Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::midnight())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// axes.area(AxisData::Numbers(vec![0.0, 1.0]), &[2.0, 3.0], AreaOptions::default())?;
/// let Some(Mark::Area(area)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(area.x(), &AxisData::Numbers(vec![0.0, 1.0]));
/// assert_eq!(area.y(), &AxisData::Numbers(vec![2.0, 3.0]));
/// assert_eq!(area.options().dash, Dash::Solid);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// An area mark cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::figure::{AreaMark, Mark};
///
/// let mark = Mark::Area(AreaMark { line: unreachable!() });
/// ```
#[derive(Clone, PartialEq)]
pub struct AreaMark {
    line: LineMark,
}

impl AreaMark {
    /// The x values.
    #[must_use]
    pub fn x(&self) -> &AxisData {
        self.line.x()
    }

    /// The y values, as many as the x values.
    #[must_use]
    pub fn y(&self) -> &AxisData {
        self.line.y()
    }

    /// Label, color, dash and width of the line that bounds the area.
    #[must_use]
    pub fn options(&self) -> &AreaOptions {
        self.line.options()
    }

    /// The line that bounds the area, which is drawn and labelled as a line mark is.
    pub(crate) fn line(&self) -> &LineMark {
        &self.line
    }

    /// The validated series.
    #[cfg(test)]
    pub(super) fn series(&self) -> &XySeries {
        self.line.series()
    }
}

impl fmt::Debug for AreaMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AreaMark")
            .field("x", self.x())
            .field("y", &self.y())
            .field("options", self.options())
            .finish()
    }
}
