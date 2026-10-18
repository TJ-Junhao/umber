//! Contours: a field sampled on a rectilinear grid, drawn as the lines where it crosses chosen
//! levels, as the bands between those levels filled from a colormap, or both.

use std::fmt;
use std::sync::Arc;

use super::color_by::{CmapChoice, ColorBy, check_center, check_mapped_values};
use super::figure::Figure;
use super::mesh::{check_mapping, numeric_axes};
use crate::error::number;
use crate::ticks::{self, Fit, TickCoverage, TickValues};
use crate::{
    ColorSpec, ContinuousScale, Error, MAX_MAGNITUDE, MIN_LOG_VALUE, NumberStyle, Result, Tick,
    TickFormat, Transform,
};

/// The context of every error of [`AxesMut::contour`](super::AxesMut::contour).
const CONTEXT: &str = "Axes.contour()";

/// How many round levels [`ContourLevels::Auto`] asks for.
const AUTO_LEVELS: usize = 7;

/// The most levels a [`ContourLevels::Count`] may ask for.
pub const MAX_LEVELS: usize = ticks::MAX_TARGET;

/// How a contour is drawn.
///
/// # Examples
///
/// ```
/// use umber_core::ContourStyle;
///
/// assert_eq!(ContourStyle::default(), ContourStyle::Lines);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContourStyle {
    /// A line along each level, colored by its level on the color scale, or all in one color.
    #[default]
    Lines,
    /// The bands between the levels, filled with the colors of the color scale; the regions
    /// below the first level and above the last take the colors at the ends of the color range.
    Filled,
    /// The filled bands, then thin lines along the levels in the theme's text color.
    Both,
}

/// The levels a contour draws its lines at and its bands between.
///
/// # Examples
///
/// ```
/// use umber_core::ContourLevels;
///
/// assert_eq!(ContourLevels::default(), ContourLevels::Auto);
/// assert_ne!(ContourLevels::Count(5), ContourLevels::Values(vec![1.0, 2.0]));
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ContourLevels {
    /// About seven round values strictly inside the range of the finite values, chosen as
    /// round tick values are: on a log color scale, as log ticks.
    #[default]
    Auto,
    /// About this many round values strictly inside the range of the finite values, from 1 to
    /// [`MAX_LEVELS`]: the round values of a search for two more ticks over the range, which may
    /// fall on its ends, those strictly inside it; and the middle of the range when none is,
    /// so a field that is not constant always has a level.
    Count(usize),
    /// Exactly these values, finite and strictly increasing. Levels outside the range of the
    /// values draw no line.
    Values(Vec<f64>),
}

/// Options of a contour.
///
/// # Examples
///
/// ```
/// use umber_core::{CmapChoice, ColorSpec, ContourLevels, ContourOptions, ContourStyle, Transform};
///
/// let options = ContourOptions::default();
/// assert_eq!((&options.levels, options.style), (&ContourLevels::Auto, ContourStyle::Lines));
/// assert_eq!((&options.cmap, options.scale, options.center), (&CmapChoice::Theme, Transform::Linear, None));
/// assert_eq!(options.color, ColorSpec::AUTO);
/// assert!(!options.annotate);
/// assert_eq!(options.format, None);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ContourOptions {
    /// The levels.
    pub levels: ContourLevels,
    /// Lines, filled bands, or both.
    pub style: ContourStyle,
    /// The colormap; [`CmapChoice::Theme`] for the theme's diverging colormap when `center` is
    /// set and its sequential colormap otherwise.
    pub cmap: CmapChoice,
    /// How values map to the colormap, linearly or by their logarithm; on a log scale the
    /// automatic levels are log ticks.
    pub scale: Transform,
    /// The value at the middle of a diverging colormap; `None` for a sequential mapping.
    pub center: Option<f64>,
    /// The color of the lines of [`ContourStyle::Lines`]: [`ColorSpec::Auto`] colors each line
    /// by its level on the color scale, at the opacity it carries; any other color draws every
    /// line in it, and the contour is then not color-mapped and has no colorbar. Filled
    /// contours take [`ColorSpec::AUTO`] only.
    pub color: ColorSpec,
    /// Whether each line long enough to hold it carries its level's value.
    pub annotate: bool,
    /// The format of the level values that `annotate` writes; `None` writes each level as the
    /// colorbar writes its ticks (the colorbar's format, else the y tick format), so a level
    /// reads the same on its line and on the bar. Only annotated contours take one.
    pub format: Option<TickFormat>,
}

impl Default for ContourOptions {
    fn default() -> Self {
        ContourOptions {
            levels: ContourLevels::Auto,
            style: ContourStyle::Lines,
            cmap: CmapChoice::Theme,
            scale: Transform::Linear,
            center: None,
            color: ColorSpec::AUTO,
            annotate: false,
            format: None,
        }
    }
}

/// A contour: the grid coordinates, the values on the grid, the levels resolved from its
/// [`ContourOptions`], and the options. The values are stored row by row, one row per y
/// coordinate, each holding one value per x coordinate; a NaN value leaves the four cells
/// around it out of every line and band. Everything is validated together and never changes;
/// clones share the data.
///
/// # Examples
///
/// ```
/// use umber_core::{ContourLevels, ContourOptions, Figure, Mark, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let options = ContourOptions { levels: ContourLevels::Values(vec![1.5]), ..ContourOptions::default() };
/// axes.contour(&[0.0, 1.0], &[0.0, 2.0], &[1.0, 2.0, 2.0, 3.0], options)?;
/// let Some(Mark::Contour(contour)) = axes.marks().first() else { unreachable!() };
/// assert_eq!((contour.x(), contour.y()), (&[0.0, 1.0][..], &[0.0, 2.0][..]));
/// assert_eq!(contour.z(), [1.0, 2.0, 2.0, 3.0]);
/// assert_eq!(contour.levels(), [1.5]);
/// # Ok::<(), umber_core::Error>(())
/// ```
///
/// A contour cannot be built outside [`Axes`](super::Axes):
///
/// ```compile_fail,E0451
/// use umber_core::{ContourMark, ContourOptions};
///
/// let mark = ContourMark {
///     x: unreachable!(),
///     y: unreachable!(),
///     z: unreachable!(),
///     levels: unreachable!(),
///     options: ContourOptions::default(),
/// };
/// ```
#[derive(Clone, PartialEq)]
pub struct ContourMark {
    x: Arc<[f64]>,
    y: Arc<[f64]>,
    z: Arc<[f64]>,
    levels: Levels,
    options: ContourOptions,
}

/// The levels of a contour, increasing, with their automatic labels.
#[derive(Debug, Clone, PartialEq)]
struct Levels {
    /// The levels with their automatic labels.
    ticks: Arc<[Tick]>,
    /// Whether the levels were computed on a grid of round values, and with which step.
    values: TickValues,
}

impl ContourMark {
    /// Validates a contour on the axes at `index` of `figure` before it is stored, so a failed
    /// call changes nothing, then resolves its levels. The checks run in this order, and the
    /// first failure is the error:
    ///
    /// 1. both axes are linear or log axes (`Error::Usage`, naming `x` or `y`);
    /// 2. `x` and `y` each hold at least two finite, strictly increasing coordinates, at most
    ///    1e300 in magnitude, positive and at least 1e-300 on a log axis, over a range the axis
    ///    can show (`Error::Data`);
    /// 3. `z` holds `y.len() * x.len()` values (`Error::Data`);
    /// 4. every value is NaN or finite, at most 1e300 in magnitude, and positive and at least
    ///    1e-300 on a log color scale (`Error::Data`, naming the value by row and column);
    /// 5. a level count is from 1 to [`MAX_LEVELS`], and given levels are finite, at most 1e300
    ///    in magnitude and strictly increasing (`Error::Style`);
    /// 6. a color other than [`ColorSpec::AUTO`] comes with [`ContourStyle::Lines`] and
    ///    without a colormap or a center; `annotate` comes with lines; a format comes with
    ///    `annotate` (all `Error::Usage`) and is a number format (`Error::Style`);
    /// 7. the center is finite, at most 1e300 in magnitude, and not combined with a log scale
    ///    (`Error::Style`);
    /// 8. a color-mapped contour maps colors as the axes' other color-mapped marks do, within
    ///    their color limits and their color-sharing group (`Error::Usage` and
    ///    `Error::Style`).
    pub(super) fn new(
        figure: &Figure,
        index: usize,
        (x, y, z): (&[f64], &[f64], &[f64]),
        options: ContourOptions,
    ) -> Result<ContourMark> {
        let [x_transform, y_transform] = numeric_axes(CONTEXT, figure.panel(index))?;
        check_coordinates("x", x, x_transform)?;
        check_coordinates("y", y, y_transform)?;
        let (columns, rows) = (x.len(), y.len());
        if columns.checked_mul(rows) != Some(z.len()) {
            let cells = (columns as u128) * (rows as u128);
            return Err(Error::data(
                CONTEXT,
                format!(
                    "z has {} values; it must have len(y) * len(x) = {rows} * {columns} = {cells}.",
                    z.len()
                ),
            )
            .with_parameter("z")
            .with_hint(
                "pass one row of z per y coordinate, each with one value per x coordinate.",
            ));
        }
        check_mapped_values(
            CONTEXT,
            z,
            options.scale,
            |i| format!("z[{}][{}]", i / columns, i % columns),
            "use NaN to leave a hole in the contour.",
        )
        .map_err(|error| error.with_parameter("z"))?;
        check_levels(&options.levels)?;
        check_drawing(&options)?;
        check_center(CONTEXT, options.center, options.scale)?;
        let z: Arc<[f64]> = Arc::from(z);
        if is_mapped(options.color) {
            let colors = ColorBy {
                values: Arc::clone(&z),
                cmap: options.cmap.clone(),
                scale: options.scale,
                center: options.center,
            };
            check_mapping(CONTEXT, figure, index, &colors)?;
        }
        let levels = Levels::of(&options, &z);
        Ok(ContourMark {
            x: Arc::from(x),
            y: Arc::from(y),
            z,
            levels,
            options,
        })
    }

    /// The x coordinates of the grid's columns, strictly increasing.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContourOptions, Figure, Mark, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.contour(&[0.0, 1.0, 3.0], &[0.0, 1.0], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], ContourOptions::default())?;
    /// let Some(Mark::Contour(contour)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(contour.x(), [0.0, 1.0, 3.0]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// The y coordinates of the grid's rows, strictly increasing.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContourOptions, Figure, Mark, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.contour(&[0.0, 1.0], &[0.0, 2.0], &[1.0, 2.0, 3.0, 4.0], ContourOptions::default())?;
    /// let Some(Mark::Contour(contour)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(contour.y(), [0.0, 2.0]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn y(&self) -> &[f64] {
        &self.y
    }

    /// The values row by row: one row per y coordinate, each holding one value per x
    /// coordinate.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContourOptions, Figure, Mark, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.contour(&[0.0, 1.0], &[0.0, 2.0], &[1.0, f64::NAN, 3.0, 4.0], ContourOptions::default())?;
    /// let Some(Mark::Contour(contour)) = axes.marks().first() else { unreachable!() };
    /// assert!(contour.z()[1].is_nan());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn z(&self) -> &[f64] {
        &self.z
    }

    /// The levels, increasing: those given, or the round values chosen for the finite values
    /// (none when every value is NaN or all are equal).
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContourOptions, Figure, Mark, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// axes.contour(&[0.0, 1.0], &[0.0, 1.0], &[0.0, 1.0, 1.0, 2.0], ContourOptions::default())?;
    /// let Some(Mark::Contour(contour)) = axes.marks().first() else { unreachable!() };
    /// // Round values strictly inside the range of the values, 0 to 2.
    /// let levels = contour.levels();
    /// assert!(levels.len() >= 2 && levels.iter().all(|&level| 0.0 < level && level < 2.0));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn levels(&self) -> Vec<f64> {
        self.levels.ticks.iter().map(|tick| tick.value).collect()
    }

    /// The style, colormap, scale, center, color, annotation and format, and the levels as
    /// given.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{ContourOptions, ContourStyle, Figure, Mark, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let options = ContourOptions { style: ContourStyle::Filled, ..ContourOptions::default() };
    /// axes.contour(&[0.0, 1.0], &[0.0, 1.0], &[0.0, 1.0, 1.0, 2.0], options.clone())?;
    /// let Some(Mark::Contour(contour)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(contour.options(), &options);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn options(&self) -> &ContourOptions {
        &self.options
    }

    /// The labels that `annotate` writes on the lines, one per level: in the contour's format,
    /// or without one in `colorbar`, the format of the colorbar's ticks, so that a level reads
    /// the same on its line and on the bar.
    pub(crate) fn labels_in(&self, colorbar: &TickFormat) -> Vec<String> {
        let format = self.options.format.as_ref().unwrap_or(colorbar);
        format
            .relabel(self.levels.ticks.to_vec(), self.levels.values)
            .into_iter()
            .map(|tick| tick.label)
            .collect()
    }

    /// The levels with their automatic labels, and where their values come from: what the
    /// colorbar marks.
    pub(crate) fn level_ticks(&self) -> (&[Tick], TickValues) {
        (&self.levels.ticks, self.levels.values)
    }

    /// The colormap, scale and center of a color-mapped contour, one whose color is
    /// [`ColorSpec::Auto`]; `None` for a contour drawn in one color.
    pub(crate) fn color_scale(&self) -> Option<crate::color_scale::ColorScaleSpec> {
        is_mapped(self.options.color).then(|| crate::color_scale::ColorScaleSpec {
            cmap: self.options.cmap.clone(),
            scale: self.options.scale,
            center: self.options.center,
        })
    }
}

impl fmt::Debug for ContourMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContourMark")
            .field("x", &self.x)
            .field("y", &self.y)
            .field("z", &self.z)
            .field("levels", &self.levels())
            .field("options", &self.options)
            .finish()
    }
}

impl Levels {
    /// The levels `options` asks for over the finite `z`, labelled.
    fn of(options: &ContourOptions, z: &[f64]) -> Levels {
        let (ticks, values) = match &options.levels {
            ContourLevels::Values(levels) => {
                let labels = match options.scale {
                    Transform::Linear => ticks::format_given(levels),
                    Transform::Log(_) => ticks::format_values(levels),
                };
                (paired(levels.clone(), labels), TickValues::Exact)
            }
            ContourLevels::Auto => round_levels(z, Ask::Auto, options.scale),
            ContourLevels::Count(count) => round_levels(z, Ask::Count(*count), options.scale),
        };
        Levels {
            ticks: ticks.into(),
            values,
        }
    }
}

/// How many round levels to choose.
#[derive(Debug, Clone, Copy)]
enum Ask {
    /// The round values of a search for [`AUTO_LEVELS`] ticks over the range, those strictly
    /// inside it.
    Auto,
    /// About this many levels strictly inside the range: the round values of a search for two
    /// more ticks, which may fall on the ends, those strictly inside it; and when none is, the
    /// middle of the range, so a field that is not constant always has a level.
    Count(usize),
}

/// The round levels `ask` asks for strictly inside the range of the finite values of `z`, with
/// their labels and where their values come from: Extended Wilkinson values on a linear scale,
/// log ticks on a log scale. None when the values have no range.
fn round_levels(z: &[f64], ask: Ask, scale: Transform) -> (Vec<Tick>, TickValues) {
    let none = (Vec::new(), TickValues::Exact);
    let Some((lo, hi)) =
        z.iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(None, |extent: Option<(f64, f64)>, v| {
                Some(extent.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))))
            })
    else {
        return none;
    };
    if lo >= hi {
        return none;
    }
    let inside = |v: f64| lo < v && v < hi;
    let (target, fallback) = match ask {
        Ask::Auto => (AUTO_LEVELS, false),
        Ask::Count(count) => (count.saturating_add(2), true),
    };
    let (levels, values) = match scale {
        Transform::Linear => {
            let all = ticks::extended(lo, hi, target, TickCoverage::Free, &Fit::ANY);
            match all.as_slice() {
                [first, second, ..] => {
                    let step = second - first;
                    let levels: Vec<f64> = ticks::snapped_to_zero(&all, step)
                        .into_iter()
                        .filter(|&v| inside(v))
                        .collect();
                    let labels = ticks::format_ticks(&levels, step);
                    (paired(levels, labels), TickValues::Grid { step })
                }
                _ => none.clone(),
            }
        }
        Transform::Log(_) => match ContinuousScale::new(scale, lo, hi, CONTEXT) {
            Ok(axis) => (
                axis.ticks(target)
                    .into_iter()
                    .filter(|tick| inside(tick.value))
                    .collect(),
                TickValues::Exact,
            ),
            // A range too narrow for log ticks still has values inside it.
            Err(_) => none.clone(),
        },
    };
    if !levels.is_empty() || !fallback {
        return (levels, values);
    }
    // No round value lies inside: the middle of the range, on the scale's own terms.
    let middle = match scale {
        Transform::Linear => lo / 2.0 + hi / 2.0,
        Transform::Log(_) => (lo.ln() / 2.0 + hi.ln() / 2.0).exp(),
    };
    if !inside(middle) {
        return none;
    }
    match values {
        TickValues::Grid { step } => (
            paired(vec![middle], ticks::format_ticks(&[middle], step)),
            values,
        ),
        TickValues::Exact => (
            paired(vec![middle], ticks::format_values(&[middle])),
            TickValues::Exact,
        ),
    }
}

/// `values` paired with `labels` as ticks.
fn paired(values: Vec<f64>, labels: Vec<String>) -> Vec<Tick> {
    values
        .into_iter()
        .zip(labels)
        .map(|(value, label)| Tick { value, label })
        .collect()
}

/// Checks the grid coordinates `values`, the argument `name` (`"x"` or `"y"`), on an axis of
/// `transform`: at least two, finite, at most 1e300 in magnitude, positive and at least 1e-300
/// on a log axis, strictly increasing, and spanning a range the axis can show (`Error::Data`).
fn check_coordinates(name: &'static str, values: &[f64], transform: Transform) -> Result<()> {
    let log = matches!(transform, Transform::Log(_));
    let fault = |message: String, hint: &'static str| {
        Error::data(CONTEXT, message)
            .with_parameter(name)
            .with_hint(hint)
    };
    if values.len() < 2 {
        let count = match values.len() {
            1 => "1 coordinate".to_owned(),
            n => format!("{n} coordinates"),
        };
        return Err(fault(
            format!("{name} has {count}; a contour grid needs at least two along each axis."),
            "pass at least two coordinates for x and for y.",
        ));
    }
    let mut previous: Option<f64> = None;
    for (i, &v) in values.iter().enumerate() {
        let required = if !v.is_finite() {
            "coordinates must be finite".to_owned()
        } else if v.abs() > MAX_MAGNITUDE {
            format!("coordinates must not exceed {MAX_MAGNITUDE:e} in magnitude")
        } else if log && v <= 0.0 {
            "coordinates on a log axis must be positive".to_owned()
        } else if log && v < MIN_LOG_VALUE {
            format!("coordinates on a log axis must be at least {MIN_LOG_VALUE:e}")
        } else if let Some(before) = previous.filter(|&before| v <= before) {
            return Err(fault(
                format!(
                    "{name}[{i}] is {}, not above {name}[{}] = {}; coordinates must be strictly \
                     increasing.",
                    number(v),
                    i - 1,
                    number(before)
                ),
                "sort the coordinates, and the rows or columns of z with them.",
            ));
        } else {
            previous = Some(v);
            continue;
        };
        let hint = if log && v.is_finite() && v.abs() <= MAX_MAGNITUDE {
            "use a linear axis, or pass positive coordinates."
        } else {
            "pass finite coordinates within the supported numeric range."
        };
        return Err(fault(
            format!("{name}[{i}] is {}; {required}.", number(v)),
            hint,
        ));
    }
    let (first, last) = (values[0], values[values.len() - 1]);
    if ContinuousScale::new(transform, first, last, CONTEXT).is_err() {
        let required = if log {
            "the last coordinate must be at least 1 + 1e-9 times the first"
        } else {
            "the range must be at least 1e-10 of their magnitude (and at least 1e-290)"
        };
        return Err(fault(
            format!(
                "{name} spans {} to {}; {required}.",
                number(first),
                number(last)
            ),
            "rescale the coordinates, or pass a wider grid.",
        ));
    }
    Ok(())
}

/// Checks the levels as given: a count from 1 to [`MAX_LEVELS`], or at least one finite level,
/// at most 1e300 in magnitude, strictly increasing (`Error::Style` about `levels`).
fn check_levels(levels: &ContourLevels) -> Result<()> {
    let fault = |message: String, hint: &'static str| {
        Error::style(CONTEXT, message)
            .with_parameter("levels")
            .with_hint(hint)
    };
    match levels {
        ContourLevels::Auto => Ok(()),
        ContourLevels::Count(count) if (1..=MAX_LEVELS).contains(count) => Ok(()),
        ContourLevels::Count(count) => Err(fault(
            format!("levels is {count}; a count of levels must be between 1 and {MAX_LEVELS}."),
            "ask for a few levels, such as levels=7, or pass the level values.",
        )),
        ContourLevels::Values(values) if values.is_empty() => Err(fault(
            "levels is empty; pass at least one level.".to_owned(),
            "pass increasing level values, or a count of round levels.",
        )),
        ContourLevels::Values(values) => {
            for (i, &v) in values.iter().enumerate() {
                if !v.is_finite() || v.abs() > MAX_MAGNITUDE {
                    return Err(fault(
                        format!(
                            "levels[{i}] is {}; levels must be finite and at most \
                             {MAX_MAGNITUDE:e} in magnitude.",
                            number(v)
                        ),
                        "pass finite level values.",
                    ));
                }
                if let Some(&before) = i.checked_sub(1).and_then(|j| values.get(j))
                    && v <= before
                {
                    return Err(fault(
                        format!(
                            "levels[{i}] is {}, not above levels[{}] = {}; levels must be \
                             strictly increasing.",
                            number(v),
                            i - 1,
                            number(before)
                        ),
                        "pass the levels in increasing order, each once.",
                    ));
                }
            }
            Ok(())
        }
    }
}

/// Whether a contour of the line color `color` is color-mapped: any [`ColorSpec::Auto`].
fn is_mapped(color: ColorSpec) -> bool {
    matches!(color, ColorSpec::Auto(_))
}

/// The name of `style` as the Python enum writes it, for messages.
fn style_name(style: ContourStyle) -> &'static str {
    match style {
        ContourStyle::Lines => "Contour.LINES",
        ContourStyle::Filled => "Contour.FILLED",
        ContourStyle::Both => "Contour.BOTH",
    }
}

/// Checks that the drawing options agree: one color only for lines and without a colormap or
/// a center; level labels only on lines; a format only with level labels (`Error::Usage`), and
/// a number format (`Error::Style`).
fn check_drawing(options: &ContourOptions) -> Result<()> {
    let style = options.style;
    if options.color != ColorSpec::AUTO && style != ContourStyle::Lines {
        return Err(Error::usage(
            CONTEXT,
            format!(
                "color is given with style={}; filled bands take their colors from the colormap.",
                style_name(style)
            ),
        )
        .with_parameter("color")
        .with_hint("leave color out, or use Contour.LINES for lines in one color."));
    }
    if !is_mapped(options.color) {
        let given = if options.cmap != CmapChoice::Theme {
            Some("cmap")
        } else if options.center.is_some() {
            Some("center")
        } else {
            None
        };
        if let Some(parameter) = given {
            return Err(Error::usage(
                CONTEXT,
                format!(
                    "{parameter} is given but color draws every line in one color; the lines are \
                     colored by level only with color=Color.AUTO."
                ),
            )
            .with_parameter(parameter)
            .with_hint(
                "leave color out to color the lines by level, or leave cmap and center out.",
            ));
        }
    }
    if options.annotate && style == ContourStyle::Filled {
        return Err(Error::usage(
            CONTEXT,
            "annotate is True with style=Contour.FILLED; level labels need lines.",
        )
        .with_parameter("annotate")
        .with_hint("use Contour.LINES or Contour.BOTH."));
    }
    let Some(format) = &options.format else {
        return Ok(());
    };
    if !options.annotate {
        return Err(Error::usage(
            CONTEXT,
            "format is given but annotate is false; only annotated contours show level labels.",
        )
        .with_parameter("format")
        .with_hint("pass annotate=True, or leave format out."));
    }
    if format.style() == NumberStyle::Iso {
        return Err(Error::style(
            CONTEXT,
            "format is TickFormat.iso(), which writes times; contour levels are numbers.",
        )
        .with_parameter("format")
        .with_hint("use TickFormat.AUTO or a number format such as TickFormat.fixed(2)."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        Cmap, CmapChoice, ColorSpec, ContourLevels, ContourOptions, ContourStyle, Error, Figure,
        LogBase, Mark, MeshOptions, Rgba, ScaleKind, Theme, TickFormat, Transform,
    };

    const X: [f64; 3] = [0.0, 1.0, 2.0];
    const Y: [f64; 2] = [0.0, 1.0];
    const Z: [f64; 6] = [0.0, 1.0, 2.0, 1.0, 2.0, 3.0];

    fn figure() -> Figure {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        f
    }

    /// The error of adding a contour of `(x, y, z)` with `options` to `f`, which must leave the
    /// figure unchanged.
    fn rejected(
        f: &mut Figure,
        (x, y, z): (&[f64], &[f64], &[f64]),
        options: ContourOptions,
    ) -> Error {
        let before = f.clone();
        let error = f.single_axes_mut().contour(x, y, z, options).unwrap_err();
        assert_eq!(*f, before);
        error
    }

    fn message(error: &Error) -> (String, Option<&'static str>, Option<&'static str>) {
        (error.to_string(), error.parameter(), error.hint())
    }

    #[test]
    fn annotate_with_filled_rejected() {
        let mut f = figure();
        let options = ContourOptions {
            style: ContourStyle::Filled,
            annotate: true,
            ..ContourOptions::default()
        };
        let error = rejected(&mut f, (&X, &Y, &Z), options);
        assert!(matches!(error, Error::Usage { .. }));
        assert_eq!(
            message(&error),
            (
                "Axes.contour(): annotate is True with style=Contour.FILLED; level labels need \
                 lines."
                    .to_owned(),
                Some("annotate"),
                Some("use Contour.LINES or Contour.BOTH.")
            )
        );
        // Lines and both take labels.
        for style in [ContourStyle::Lines, ContourStyle::Both] {
            let options = ContourOptions {
                style,
                annotate: true,
                ..ContourOptions::default()
            };
            f.single_axes_mut().contour(&X, &Y, &Z, options).unwrap();
        }
    }

    #[test]
    fn grids_are_checked_in_order() {
        let mut f = figure();
        type Case<'a> = (&'a [f64], &'a [f64], &'a [f64], &'a str, &'a str);
        let cases: [Case<'_>; 7] = [
            (
                &[0.0],
                &Y,
                &Z,
                "Axes.contour(): x has 1 coordinate; a contour grid needs at least two along each axis.",
                "x",
            ),
            (
                &[0.0, 2.0, 1.0],
                &Y,
                &Z,
                "Axes.contour(): x[2] is 1, not above x[1] = 2; coordinates must be strictly \
                 increasing.",
                "x",
            ),
            (
                &X,
                &[0.0, f64::NAN],
                &Z,
                "Axes.contour(): y[1] is NaN; coordinates must be finite.",
                "y",
            ),
            (
                &X,
                &[1.0, 1.0 + 1e-14],
                &Z,
                "Axes.contour(): y spans 1 to 1.00000000000001; the range must be at least 1e-10 \
                 of their magnitude (and at least 1e-290).",
                "y",
            ),
            (
                &X,
                &Y,
                &Z[..5],
                "Axes.contour(): z has 5 values; it must have len(y) * len(x) = 2 * 3 = 6.",
                "z",
            ),
            (
                &X,
                &Y,
                &[0.0, 1.0, 2.0, 1.0, f64::INFINITY, 3.0],
                "Axes.contour(): z[1][1] is inf; values must be finite.",
                "z",
            ),
            (
                &X,
                &Y,
                &[0.0, 1.0, 2.0, 1.0, 1e301, 3.0],
                "Axes.contour(): z[1][1] is 1e301; magnitudes must not exceed 1e300.",
                "z",
            ),
        ];
        for (x, y, z, text, parameter) in cases {
            let error = rejected(&mut f, (x, y, z), ContourOptions::default());
            assert!(matches!(error, Error::Data { .. }), "{error:?}");
            assert_eq!(
                (error.to_string().as_str(), error.parameter()),
                (text, Some(parameter))
            );
        }
        // NaN values are holes, not errors.
        f.single_axes_mut()
            .contour(
                &X,
                &Y,
                &[0.0, f64::NAN, 2.0, 1.0, 2.0, 3.0],
                ContourOptions::default(),
            )
            .unwrap();
    }

    #[test]
    fn axes_must_be_numeric_and_log_coordinates_positive() {
        for (x, y) in [
            (ScaleKind::Category, ScaleKind::Linear),
            (ScaleKind::Linear, ScaleKind::Time),
        ] {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            f.add_axes(x, y).unwrap();
            let error = rejected(&mut f, (&X, &Y, &Z), ContourOptions::default());
            assert!(matches!(error, Error::Usage { .. }), "{error:?}");
        }
        let log = ScaleKind::Log { base: LogBase::TEN };
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(log, ScaleKind::Linear).unwrap();
        let error = rejected(&mut f, (&X, &Y, &Z), ContourOptions::default());
        assert_eq!(
            message(&error),
            (
                "Axes.contour(): x[0] is 0; coordinates on a log axis must be positive.".to_owned(),
                Some("x"),
                Some("use a linear axis, or pass positive coordinates.")
            )
        );
        f.single_axes_mut()
            .contour(&[1.0, 10.0, 100.0], &Y, &Z, ContourOptions::default())
            .unwrap();
        // A log color scale needs positive values.
        let mut f = figure();
        let options = ContourOptions {
            scale: Transform::Log(LogBase::TEN),
            ..ContourOptions::default()
        };
        let error = rejected(&mut f, (&X, &Y, &Z), options);
        assert_eq!(
            error.to_string(),
            "Axes.contour(): z[0][0] is 0; values on a log scale must be positive."
        );
        assert_eq!(error.parameter(), Some("z"));
    }

    #[test]
    fn levels_are_checked() {
        let mut f = figure();
        let cases = [
            (
                ContourLevels::Count(0),
                "Axes.contour(): levels is 0; a count of levels must be between 1 and 64.",
            ),
            (
                ContourLevels::Count(65),
                "Axes.contour(): levels is 65; a count of levels must be between 1 and 64.",
            ),
            (
                ContourLevels::Values(Vec::new()),
                "Axes.contour(): levels is empty; pass at least one level.",
            ),
            (
                ContourLevels::Values(vec![1.0, f64::NAN]),
                "Axes.contour(): levels[1] is NaN; levels must be finite and at most 1e300 in \
                 magnitude.",
            ),
            (
                ContourLevels::Values(vec![1.0, 2.0, 2.0]),
                "Axes.contour(): levels[2] is 2, not above levels[1] = 2; levels must be strictly \
                 increasing.",
            ),
        ];
        for (levels, text) in cases {
            let options = ContourOptions {
                levels,
                ..ContourOptions::default()
            };
            let error = rejected(&mut f, (&X, &Y, &Z), options);
            assert!(matches!(error, Error::Style { .. }), "{error:?}");
            assert_eq!(
                (error.to_string().as_str(), error.parameter()),
                (text, Some("levels"))
            );
        }
    }

    #[test]
    fn levels_are_round_and_inside_the_values() {
        let mut f = figure();
        let z: Vec<f64> = (0..6).map(|i| f64::from(i) * 1.7 - 3.1).collect();
        f.single_axes_mut()
            .contour(&X, &Y, &z, ContourOptions::default())
            .unwrap();
        f.single_axes_mut()
            .contour(
                &X,
                &Y,
                &z,
                ContourOptions {
                    levels: ContourLevels::Count(3),
                    ..ContourOptions::default()
                },
            )
            .unwrap();
        let [Mark::Contour(auto), Mark::Contour(three)] = f.single_axes().marks() else {
            unreachable!()
        };
        for contour in [auto, three] {
            let levels = contour.levels();
            assert!(levels.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(levels.iter().all(|&v| -3.1 < v && v < 5.4), "{levels:?}");
            // Evenly spaced round values, labelled with the decimals of their step.
            let step = levels[1] - levels[0];
            assert!(
                levels
                    .windows(2)
                    .all(|pair| ((pair[1] - pair[0]) - step).abs() < 1e-9)
            );
        }
        assert!(auto.levels().len() >= 5 && auto.levels().len() <= 10);
        assert!(three.levels().len() < auto.levels().len());
        // Log color scale: log ticks.
        let mut f = figure();
        let z = [1.0, 10.0, 100.0, 1000.0, 1e4, 1e5];
        let options = ContourOptions {
            scale: Transform::Log(LogBase::TEN),
            ..ContourOptions::default()
        };
        f.single_axes_mut().contour(&X, &Y, &z, options).unwrap();
        let [Mark::Contour(log)] = f.single_axes().marks() else {
            unreachable!()
        };
        assert_eq!(log.levels(), [10.0, 100.0, 1000.0, 1e4]);
        // Given levels are kept exactly, with the labels they need.
        let mut f = figure();
        let options = ContourOptions {
            levels: ContourLevels::Values(vec![-1.0, 0.25, 1.5, 9.0]),
            annotate: true,
            ..ContourOptions::default()
        };
        f.single_axes_mut().contour(&X, &Y, &Z, options).unwrap();
        let [Mark::Contour(given)] = f.single_axes().marks() else {
            unreachable!()
        };
        assert_eq!(given.levels(), [-1.0, 0.25, 1.5, 9.0]);
        assert_eq!(
            given.labels_in(&TickFormat::default()),
            ["\u{2212}1.00", "0.25", "1.50", "9.00"]
        );
    }

    #[test]
    fn a_count_always_finds_a_level_on_a_varying_field() {
        let narrow = [1.0, 1.0 + 1e-10, 1.0, 1.0 + 1e-10];
        for count in [1, 2, 3, 64] {
            for scale in [Transform::Log(LogBase::TEN), Transform::Linear] {
                let mut f = figure();
                f.single_axes_mut()
                    .contour(
                        &[0.0, 1.0],
                        &[0.0, 1.0],
                        &narrow,
                        ContourOptions {
                            levels: ContourLevels::Count(count),
                            scale,
                            ..ContourOptions::default()
                        },
                    )
                    .unwrap();
                let [Mark::Contour(contour)] = f.single_axes().marks() else {
                    unreachable!()
                };
                let levels = contour.levels();
                assert!(!levels.is_empty(), "{count} on {scale:?}");
                assert!(
                    levels.iter().all(|&v| 1.0 < v && v < 1.0 + 1e-10),
                    "{levels:?}"
                );
            }
        }
    }

    #[test]
    fn drawing_options_must_agree() {
        let mut f = figure();
        let red = ColorSpec::Fixed(Rgba::RED);
        let cases = [
            (
                ContourOptions {
                    style: ContourStyle::Filled,
                    color: red,
                    ..ContourOptions::default()
                },
                "Axes.contour(): color is given with style=Contour.FILLED; filled bands take \
                 their colors from the colormap.",
                "color",
            ),
            (
                ContourOptions {
                    style: ContourStyle::Both,
                    color: red,
                    ..ContourOptions::default()
                },
                "Axes.contour(): color is given with style=Contour.BOTH; filled bands take their \
                 colors from the colormap.",
                "color",
            ),
            (
                ContourOptions {
                    color: red,
                    cmap: CmapChoice::Fixed(Cmap::viridis()),
                    ..ContourOptions::default()
                },
                "Axes.contour(): cmap is given but color draws every line in one color; the lines \
                 are colored by level only with color=Color.AUTO.",
                "cmap",
            ),
            (
                ContourOptions {
                    color: red,
                    center: Some(0.0),
                    ..ContourOptions::default()
                },
                "Axes.contour(): center is given but color draws every line in one color; the \
                 lines are colored by level only with color=Color.AUTO.",
                "center",
            ),
            (
                ContourOptions {
                    format: Some(TickFormat::fixed(1).unwrap()),
                    ..ContourOptions::default()
                },
                "Axes.contour(): format is given but annotate is false; only annotated contours \
                 show level labels.",
                "format",
            ),
        ];
        for (options, text, parameter) in cases {
            let error = rejected(&mut f, (&X, &Y, &Z), options);
            assert!(matches!(error, Error::Usage { .. }), "{error:?}");
            assert_eq!(
                (error.to_string().as_str(), error.parameter()),
                (text, Some(parameter))
            );
        }
        let iso = ContourOptions {
            annotate: true,
            format: Some(TickFormat::iso()),
            ..ContourOptions::default()
        };
        let error = rejected(&mut f, (&X, &Y, &Z), iso);
        assert!(matches!(error, Error::Style { .. }));
        assert_eq!(error.parameter(), Some("format"));
        let log_center = ContourOptions {
            scale: Transform::Log(LogBase::TEN),
            center: Some(2.0),
            ..ContourOptions::default()
        };
        let error = rejected(&mut f, (&X, &Y, &[1.0; 6]), log_center);
        assert!(matches!(error, Error::Style { .. }));
        assert_eq!(error.parameter(), Some("center"));
        // One color for lines is not a color mapping, so it may differ from the axes' scale.
        f.single_axes_mut()
            .contour(
                &X,
                &Y,
                &Z,
                ContourOptions {
                    color: red,
                    ..ContourOptions::default()
                },
            )
            .unwrap();
        assert!(f.single_axes().marks()[0].color_scale().is_none());
    }

    #[test]
    fn contours_share_the_axes_color_scale() {
        let mut f = figure();
        f.single_axes_mut()
            .mesh(&[0.0, 1.0], &[0.0, 1.0], &[4.0], MeshOptions::default())
            .unwrap();
        let viridis = ContourOptions {
            cmap: CmapChoice::Fixed(Cmap::viridis()),
            ..ContourOptions::default()
        };
        let error = rejected(&mut f, (&X, &Y, &Z), viridis);
        assert!(matches!(error, Error::Usage { .. }));
        assert_eq!(error.parameter(), Some("cmap"));
        f.single_axes_mut()
            .contour(&X, &Y, &Z, ContourOptions::default())
            .unwrap();
        // The color limits must suit the scale.
        let mut f = figure();
        f.single_axes_mut().clim(-1.0, 4.0).unwrap();
        let log = ContourOptions {
            scale: Transform::Log(LogBase::TEN),
            ..ContourOptions::default()
        };
        let error = rejected(&mut f, (&X, &Y, &[1.0; 6]), log);
        assert!(matches!(error, Error::Style { .. }), "{error:?}");
    }
}
