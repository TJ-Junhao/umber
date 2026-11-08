//! Streamlines: lines that follow a vector field on a grid ([`StreamplotMark`]).
//!
//! The mark reads the grid through the same [`VectorField`] as arrows and barbs, and traces its
//! lines once, when it is added, in [`crate::field::stream`]: which lines are drawn depends on
//! the data, the density and the kinds of the axes alone, never on the figure or its layout.

use std::sync::Arc;

use super::axis_data::AxisData;
use super::color_by::PointColor;
use super::contour::{check_count, check_span};
use super::field::{FieldVectors, VectorField, check_color_shape};
use crate::error::{number, quoted};
use crate::field::stream::{self, Blocked, Grid, MAX_DENSITY, MIN_DENSITY, Streamline};
use crate::{Error, Result, ScaleKind, Transform};

/// The context of every error of [`AxesMut::streamplot`](super::AxesMut::streamplot).
pub(super) const STREAMPLOT_CONTEXT: &str = "Axes.streamplot()";

/// Where the streamlines of a mark start.
///
/// # Examples
///
/// ```
/// use umber_core::StreamStarts;
///
/// assert_eq!(StreamStarts::default(), StreamStarts::Auto);
/// let given = StreamStarts::At { x: vec![0.5], y: vec![0.25] };
/// assert_ne!(given, StreamStarts::Auto);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub enum StreamStarts {
    /// Seeds tried in a fixed order from the edge of the grid inward, each line kept apart from
    /// the others by the density.
    #[default]
    Auto,
    /// One line through each point `(x[i], y[i])`, both ways, regardless of the other lines.
    /// Every point lies on the grid, outside its holes, where the field is not zero, and a line
    /// can leave it.
    At {
        /// The x coordinate of each start.
        x: Vec<f64>,
        /// The y coordinate of each start, one per x coordinate.
        y: Vec<f64>,
    },
}

/// Options of streamlines.
///
/// # Examples
///
/// ```
/// use umber_core::{PointColor, StreamStarts, StreamplotOptions};
///
/// let options = StreamplotOptions::default();
/// assert!(options.label.is_empty() && options.arrows);
/// assert_eq!((options.density, &options.color), (1.0, &PointColor::AUTO));
/// assert_eq!(options.starts, StreamStarts::Auto);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StreamplotOptions {
    /// The legend label; empty for none. The entry is a line swatch. A color-mapped mark has no
    /// legend entry, and takes no label.
    pub label: String,
    /// How close automatic lines lie: the extent of the grid along each axis is divided into
    /// `30 × density` steps, and no two automatic lines come closer than half a step. From 0.1
    /// to 10.
    pub density: f64,
    /// The line color: one color, or mapped values, one per value of `u`, interpolated along the
    /// lines.
    pub color: PointColor,
    /// Where the lines start.
    pub starts: StreamStarts,
    /// Whether each line long enough carries an arrow head at its middle, pointing downstream.
    pub arrows: bool,
}

impl Default for StreamplotOptions {
    fn default() -> StreamplotOptions {
        StreamplotOptions {
            label: String::new(),
            density: 1.0,
            color: PointColor::AUTO,
            starts: StreamStarts::Auto,
            arrows: true,
        }
    }
}

/// Streamlines on a grid field: lines that follow the field, bilinear in each cell, in data
/// coordinates, so each is a true streamline on any axes.
///
/// The extent of the grid along each axis, on the axis scale (in logarithms on a log axis), is
/// divided into `30 × density` steps. Automatic lines start from seeds tried in a fixed order,
/// ring by ring from the edge of the grid inward; a seed is used where no line comes within 0.9
/// of a step, and its line is traced both ways. A line ends at the edge of the grid; at a hole,
/// the four cells around a NaN of `u`, `v` or a mapped color value; where the speed is zero, at
/// most 1e-6 of the largest speed at a grid point, both in steps per unit of time, or where the
/// line runs into a point where the flow stops, whether the field changes sign there or not;
/// where an automatic line would come within half a step of another line; where a line would
/// come within half a step of its own points more than two steps back along it, both times the
/// radius of its curve where that is below a step, which ends a closed orbit after one turn at
/// any size. An automatic line shorter than two steps is dropped. [`StreamStarts::At`] draws one line through each start instead,
/// regardless of the others. A line is traced by Heun's method in steps of at most a quarter of
/// a step, each accepted when it ends within 0.01 of a step of the Euler step it starts with,
/// turns the line by at most about 0.05 radians, and crosses no hole and no point where the
/// speed is zero. Nothing here depends on the figure size: the same input gives the same lines,
/// point for point, on every platform, at any range of the data.
///
/// A line at least one step long carries one arrow head at its middle, measured in steps,
/// pointing downstream: the full-size head of a quiver arrow, 5 pt long and 4 pt wide, in the line's
/// color. Lines are as wide as contour lines. Lines of one color are one path, with round ends;
/// mapped colors are drawn as pieces half a step long, each starting a quarter of a step after
/// the one before, with flat ends, so each piece ends inside the next and no seam shows, and
/// each piece and head takes the color of the value at its middle, moved away from the
/// background where it comes close to it, as the lines of a contour are.
///
/// Automatic limits are the outer coordinates of the grid, without padding, as for a contour.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Mark, ScaleKind, StreamplotOptions, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let (x, y) = ([0.0, 1.0, 2.0], [0.0, 1.0]);
/// axes.streamplot(&x, &y, &[1.0; 6], &[0.5; 6], StreamplotOptions::default())?;
/// let Some(Mark::Streamplot(mark)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(mark.field().x(), [0.0, 1.0, 2.0]);
/// assert!(!mark.lines().is_empty());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StreamplotMark {
    field: VectorField,
    options: StreamplotOptions,
    lines: Arc<[Streamline]>,
}

impl StreamplotMark {
    /// Validates streamlines on axes of the scale kinds `kinds`, then traces them. The checks run
    /// in this order, and the first failure is the error:
    ///
    /// 1. both axes are linear or log axes (`Error::Usage`, naming `x` or `y`);
    /// 2. the grid, as [`VectorField`] validates one: `u` and `v` hold `y.len() * x.len()`
    ///    values, the coordinates are finite, suit their axes and strictly increase, and every
    ///    component is NaN or finite and at most 1e300 in magnitude (`Error::Data`);
    /// 3. at least two coordinates along each axis, spanning a range the axis can show
    ///    (`Error::Data`);
    /// 4. the density is finite and from 0.1 to 10 (`Error::Style`);
    /// 5. a color mapping has one value per value of `u` (`Error::Data`), and no label
    ///    (`Error::Usage`);
    /// 6. given starts have as many y as x coordinates and at least one, and each lies on the
    ///    grid, outside its holes, where the speed is not zero, and a line can be traced from it
    ///    (`Error::Data`, naming the start).
    pub(super) fn new(
        kinds: (ScaleKind, ScaleKind),
        (x, y): (&[f64], &[f64]),
        (u, v): (&[f64], &[f64]),
        options: StreamplotOptions,
    ) -> Result<StreamplotMark> {
        let transforms = check_linear_or_log(STREAMPLOT_CONTEXT, kinds, "streamlines need")?;
        let field = VectorField::new(
            STREAMPLOT_CONTEXT,
            kinds,
            (AxisData::Numbers(x.to_vec()), AxisData::Numbers(y.to_vec())),
            FieldVectors::grid(u.to_vec(), v.to_vec()),
        )?;
        for (name, values, transform) in [
            ("x", field.x(), transforms[0]),
            ("y", field.y(), transforms[1]),
        ] {
            check_count(STREAMPLOT_CONTEXT, name, values, "a streamplot grid")?;
            check_span(STREAMPLOT_CONTEXT, name, values, transform)?;
        }
        check_density(options.density)?;
        check_color_shape(STREAMPLOT_CONTEXT, &options.color, field.len())?;
        check_label(&options)?;
        let grid = Grid {
            x: field.x(),
            y: field.y(),
            u: field.u(),
            v: field.v(),
            values: match &options.color {
                PointColor::By(by) => Some(&by.values[..]),
                PointColor::Uniform(_) => None,
            },
            log: transforms.map(|transform| matches!(transform, Transform::Log(_))),
            density: options.density,
        };
        let lines = match &options.starts {
            StreamStarts::Auto => stream::automatic(grid),
            StreamStarts::At { x, y } => {
                let starts = check_starts(x, y)?;
                stream::through(grid, &starts)
                    .map_err(|(i, blocked)| start_error(&field, i, starts[i], blocked))?
            }
        };
        Ok(StreamplotMark {
            field,
            options,
            lines: lines.into(),
        })
    }

    /// The field.
    #[must_use]
    pub fn field(&self) -> &VectorField {
        &self.field
    }

    /// The label, density, color, starts and arrows.
    #[must_use]
    pub fn options(&self) -> &StreamplotOptions {
        &self.options
    }

    /// The streamlines, in the order their seeds were tried, or of the starts.
    #[must_use]
    pub fn lines(&self) -> &[Streamline] {
        &self.lines
    }
}

/// The transforms of the axes of the scale kinds `kinds`, refusing a time or category axis for
/// the call `context`, whose marks `needs` (such as `"streamlines need"`) linear or log axes:
/// `Error::Usage` naming `x` or `y`.
pub(super) fn check_linear_or_log(
    context: &'static str,
    (x, y): (ScaleKind, ScaleKind),
    needs: &str,
) -> Result<[Transform; 2]> {
    let transform = |name: &'static str, kind: ScaleKind| {
        kind.transform().ok_or_else(|| {
            let what = match kind {
                ScaleKind::Time => "a time axis",
                _ => "a category axis",
            };
            Error::usage(
                context,
                format!("the {name} axis is {what}; {needs} linear or log axes."),
            )
            .with_parameter(name)
            .with_hint("use Scale.linear() or Scale.log() for both axes.")
        })
    };
    Ok([transform("x", x)?, transform("y", y)?])
}

/// Checks the density: finite and from 0.1 to 10 (`Error::Style`).
fn check_density(density: f64) -> Result<()> {
    if density.is_finite() && (MIN_DENSITY..=MAX_DENSITY).contains(&density) {
        return Ok(());
    }
    Err(Error::style(
        STREAMPLOT_CONTEXT,
        format!(
            "density is {}; it must be from {MIN_DENSITY} to {MAX_DENSITY}.",
            number(density)
        ),
    )
    .with_parameter("density")
    .with_hint("pass 1 for lines about a thirtieth of the grid apart, 2 for twice as many."))
}

/// Refuses a label on color-mapped lines, which have no legend entry: `Error::Usage`.
fn check_label(options: &StreamplotOptions) -> Result<()> {
    if options.label.is_empty() || matches!(options.color, PointColor::Uniform(_)) {
        return Ok(());
    }
    Err(Error::usage(
        STREAMPLOT_CONTEXT,
        format!(
            "label is {}; color-mapped streamlines have no legend entry.",
            quoted(&options.label)
        ),
    )
    .with_parameter("label")
    .with_hint("remove the label; the colorbar explains the colors."))
}

/// The starts `(x[i], y[i])`: as many y as x coordinates, at least one, all finite
/// (`Error::Data` about `starts`).
fn check_starts(x: &[f64], y: &[f64]) -> Result<Vec<[f64; 2]>> {
    let fault = |message: String, hint: &'static str| {
        Error::data(STREAMPLOT_CONTEXT, message)
            .with_parameter("starts")
            .with_hint(hint)
    };
    if x.len() != y.len() {
        return Err(fault(
            format!(
                "starts has {} x coordinates but {} y coordinates; they must have the same \
                 length.",
                x.len(),
                y.len()
            ),
            "pass starts=(xs, ys) with one y per x.",
        ));
    }
    if x.is_empty() {
        return Err(fault(
            "starts is empty; pass at least one point.".to_owned(),
            "pass starts=(xs, ys), or AUTO for automatic lines.",
        ));
    }
    x.iter()
        .zip(y)
        .enumerate()
        .map(|(i, (&x, &y))| {
            if x.is_finite() && y.is_finite() {
                Ok([x, y])
            } else {
                Err(fault(
                    format!(
                        "starts[{i}] is ({}, {}); a start must be finite.",
                        number(x),
                        number(y)
                    ),
                    "pass finite starts within the grid.",
                ))
            }
        })
        .collect()
}

/// The error for the start `i` at `[x, y]` of `field`, where no line can pass, as `blocked`
/// says.
fn start_error(field: &VectorField, i: usize, [x, y]: [f64; 2], blocked: Blocked) -> Error {
    let at = format!("starts[{i}] at ({}, {})", number(x), number(y));
    let (message, hint) = match blocked {
        Blocked::Outside => {
            let span = |values: &[f64]| {
                format!(
                    "{} to {}",
                    number(values[0]),
                    number(values[values.len() - 1])
                )
            };
            (
                format!(
                    "{at} lies outside the grid, which spans x {} and y {}.",
                    span(field.x()),
                    span(field.y())
                ),
                "pass starts within the grid's coordinates.",
            )
        }
        Blocked::Hole => (
            format!("{at} lies in a hole: a cell with a NaN corner in u, v or color."),
            "move the start out of the hole, or leave it out.",
        ),
        Blocked::Still => (
            format!("the speed is zero at {at}; no line passes there."),
            "move the start to where the field flows, or leave it out.",
        ),
        Blocked::Stuck => (
            format!(
                "no line can be traced from {at}: every step from it is blocked at once, by the \
                 edge, a hole or a point where the flow stops, or turns too fast to follow."
            ),
            "move the start away from the edge, holes and points where the flow stops.",
        ),
    };
    Error::data(STREAMPLOT_CONTEXT, message)
        .with_parameter("starts")
        .with_hint(hint)
}
