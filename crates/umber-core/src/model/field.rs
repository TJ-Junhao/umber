//! Vector fields: samples of a direction and a size, on points or on a grid, and the two marks
//! drawn from them, arrows ([`QuiverMark`]) and wind barbs ([`BarbsMark`]).
//!
//! [`VectorField`] is the one model both marks read: the coordinates, `u` and `v`, and whether
//! they are points or a grid. It does not know how it is drawn, so streamlines can take the
//! same grid.

use std::sync::Arc;

use super::axis_data::{AxisData, SharedValues};
use super::color_by::{PointColor, check_mapped_values};
use super::contour::check_ordered;
use super::share::SharedAxis;
use super::validation::{check_axis_data, shared_of};
use crate::error::number;
use crate::field::barb::{Feathers, MAX_SPEED};
use crate::field::magnitude;
use crate::scale::MAX_MAGNITUDE;
use crate::{Error, Instant, Result, ScaleKind, SizeSpec, Transform};

/// The context of every error of [`AxesMut::quiver`](super::AxesMut::quiver).
pub(super) const QUIVER_CONTEXT: &str = "Axes.quiver()";
/// The context of every error of [`AxesMut::barbs`](super::AxesMut::barbs).
pub(super) const BARBS_CONTEXT: &str = "Axes.barbs()";
/// The hint for an infinite coordinate or component.
const GAP_HINT: &str = "use NaN to leave a sample out.";
/// The longest arrow, in points: the side of the largest figure.
pub(crate) const MAX_ARROW_LENGTH: f64 = super::MAX_FIGURE_IN * 72.0;

/// How the samples of a field lie.
///
/// # Examples
///
/// ```
/// use umber_core::FieldForm;
///
/// assert_ne!(FieldForm::Points, FieldForm::Grid);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldForm {
    /// One sample per coordinate pair: `x`, `y`, `u` and `v` have the same length.
    Points,
    /// A sample at each crossing of a column and a row: `u` and `v` hold one row per y
    /// coordinate, each with one value per x coordinate.
    Grid,
}

/// The vectors of a field: `u` along x and `v` along y, in the order of [`FieldForm`].
///
/// # Examples
///
/// ```
/// use umber_core::{FieldForm, FieldVectors};
///
/// let vectors = FieldVectors::points(vec![1.0], vec![2.0]);
/// assert_eq!(vectors.form, FieldForm::Points);
/// assert_eq!(FieldVectors::grid(vec![1.0], vec![2.0]).form, FieldForm::Grid);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FieldVectors {
    /// How the samples lie.
    pub form: FieldForm,
    /// The component along x of each sample.
    pub u: Arc<[f64]>,
    /// The component along y of each sample.
    pub v: Arc<[f64]>,
}

impl FieldVectors {
    /// The vectors of points, one per coordinate pair.
    #[must_use]
    pub fn points(u: impl Into<Arc<[f64]>>, v: impl Into<Arc<[f64]>>) -> FieldVectors {
        FieldVectors {
            form: FieldForm::Points,
            u: u.into(),
            v: v.into(),
        }
    }

    /// The vectors of a grid, row by row.
    #[must_use]
    pub fn grid(u: impl Into<Arc<[f64]>>, v: impl Into<Arc<[f64]>>) -> FieldVectors {
        FieldVectors {
            form: FieldForm::Grid,
            u: u.into(),
            v: v.into(),
        }
    }
}

/// A validated field. The coordinates of a grid are finite and strictly increasing, as those of
/// a contour are; those of points may be NaN or missing. A sample is present when its
/// coordinates, `u` and `v` are all there: NaN or a missing time in any of them leaves the sample
/// out, of the drawing, of the limits and of the automatic scale. Everything is validated
/// together and never changes; clones share the data.
///
/// # Examples
///
/// ```
/// use umber_core::{AxisData, FieldForm, FieldVectors, Figure, Mark, QuiverOptions, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let (x, y) = (AxisData::Numbers(vec![0.0, 1.0]), AxisData::Numbers(vec![0.0]));
/// axes.quiver(x, y, FieldVectors::grid(vec![3.0, 0.0], vec![4.0, f64::NAN]), QuiverOptions::default())?;
/// let Some(Mark::Quiver(mark)) = axes.marks().first() else { unreachable!() };
/// let field = mark.field();
/// assert_eq!(field.form(), FieldForm::Grid);
/// assert_eq!((field.x(), field.y()), (&[0.0, 1.0][..], &[0.0][..]));
/// assert_eq!(field.u(), [3.0, 0.0]);
/// assert!(field.v()[1].is_nan());
/// assert_eq!(field.longest(), 5.0);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct VectorField {
    x: SharedValues,
    y: SharedValues,
    form: FieldForm,
    u: Arc<[f64]>,
    v: Arc<[f64]>,
    /// The x coordinates that have a present sample, in order.
    kept_x: SharedValues,
    /// The y coordinates that have a present sample, in order.
    kept_y: SharedValues,
    /// The largest magnitude among the present samples; 0 without one.
    longest: f64,
}

/// One present sample of a field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sample {
    /// Its index in `u` and `v`.
    pub index: usize,
    /// The index of its x coordinate.
    pub column: usize,
    /// The index of its y coordinate.
    pub row: usize,
    /// Its component along x.
    pub u: f64,
    /// Its component along y.
    pub v: f64,
}

impl VectorField {
    /// Validates a field on axes of the scale kinds `kinds`, in this order, the first failure
    /// being the error, under `context`:
    ///
    /// 1. neither axis is a category axis (`Error::Usage`, naming `x` or `y`);
    /// 2. the lengths match the form: `x`, `y`, `u` and `v` the same length for points; `u` and
    ///    `v` `y.len() * x.len()` values for a grid (`Error::Data`);
    /// 3. `x`, then `y`, suit their axes, as the coordinates of a scatter do, and on a grid they
    ///    are finite and strictly increase (`Error::Data`);
    /// 4. every value of `u`, then of `v`, is NaN or finite and at most 1e300 in magnitude
    ///    (`Error::Data`, naming the value, by row and column on a grid).
    pub(super) fn new(
        context: &'static str,
        kinds: (ScaleKind, ScaleKind),
        (x, y): (AxisData, AxisData),
        vectors: FieldVectors,
    ) -> Result<VectorField> {
        check_axes(context, kinds)?;
        let FieldVectors { form, u, v } = vectors;
        check_lengths(context, form, (x.len(), y.len()), (u.len(), v.len()))?;
        check_axis_data(context, SharedAxis::X, kinds.0, &x, GAP_HINT)?;
        check_axis_data(context, SharedAxis::Y, kinds.1, &y, GAP_HINT)?;
        let (x, y) = (shared_of(x), shared_of(y));
        if form == FieldForm::Grid {
            check_grid_axis(context, "x", &x, kinds.0)?;
            check_grid_axis(context, "y", &y, kinds.1)?;
        }
        let columns = x.len();
        let name = |component: &str, i: usize| match form {
            FieldForm::Points => format!("{component}[{i}]"),
            FieldForm::Grid => format!("{component}[{}][{}]", i / columns, i % columns),
        };
        for (component, values) in [("u", &u), ("v", &v)] {
            check_mapped_values(
                context,
                values,
                Transform::Linear,
                |i| name(component, i),
                GAP_HINT,
            )
            .map_err(|error| error.with_parameter(component))?;
        }
        let mut field = VectorField {
            kept_x: x.clone(),
            kept_y: y.clone(),
            x,
            y,
            form,
            u,
            v,
            longest: 0.0,
        };
        field.count_samples();
        Ok(field)
    }

    /// Finds the longest vector and the coordinates that have a present sample.
    fn count_samples(&mut self) {
        let mut kept = (vec![false; self.x.len()], vec![false; self.y.len()]);
        let mut longest: f64 = 0.0;
        for sample in self.samples() {
            kept.0[sample.column] = true;
            kept.1[sample.row] = true;
            longest = longest.max(magnitude(sample.u, sample.v));
        }
        self.longest = longest;
        self.kept_x = only(&self.x, &kept.0);
        self.kept_y = only(&self.y, &kept.1);
    }

    /// How many values `u` and `v` hold.
    pub(crate) fn len(&self) -> usize {
        self.u.len()
    }

    /// The present samples, in the order of `u` and `v`.
    pub(crate) fn samples(&self) -> impl Iterator<Item = Sample> + '_ {
        let columns = self.x.len();
        let grid = self.form == FieldForm::Grid;
        self.u
            .iter()
            .zip(self.v.iter())
            .enumerate()
            .filter_map(move |(index, (&u, &v))| {
                let (column, row) = if grid {
                    (index % columns, index / columns)
                } else {
                    (index, index)
                };
                let present =
                    !(u.is_nan() || v.is_nan() || self.x.is_gap(column) || self.y.is_gap(row));
                present.then_some(Sample {
                    index,
                    column,
                    row,
                    u,
                    v,
                })
            })
    }

    /// How the samples lie.
    #[must_use]
    pub fn form(&self) -> FieldForm {
        self.form
    }

    /// The numeric x coordinates, as given; none on a time x axis.
    #[must_use]
    pub fn x(&self) -> &[f64] {
        self.x.numbers()
    }

    /// The numeric y coordinates, as given; none on a time y axis.
    #[must_use]
    pub fn y(&self) -> &[f64] {
        self.y.numbers()
    }

    /// The x times, `None` for a missing time; none unless the x axis is a time axis.
    #[must_use]
    pub fn x_times(&self) -> &[Option<Instant>] {
        self.x.times()
    }

    /// The y times, `None` for a missing time; none unless the y axis is a time axis.
    #[must_use]
    pub fn y_times(&self) -> &[Option<Instant>] {
        self.y.times()
    }

    /// The x coordinates as stored.
    pub(crate) fn x_values(&self) -> &SharedValues {
        &self.x
    }

    /// The y coordinates as stored.
    pub(crate) fn y_values(&self) -> &SharedValues {
        &self.y
    }

    /// The x coordinates that have a present sample: what the field adds to the x limits.
    pub(crate) fn kept_x(&self) -> &SharedValues {
        &self.kept_x
    }

    /// The y coordinates that have a present sample: what the field adds to the y limits.
    pub(crate) fn kept_y(&self) -> &SharedValues {
        &self.kept_y
    }

    /// The component along x of each sample, in the order of [`form`](Self::form).
    #[must_use]
    pub fn u(&self) -> &[f64] {
        &self.u
    }

    /// The component along y of each sample, in the order of [`form`](Self::form).
    #[must_use]
    pub fn v(&self) -> &[f64] {
        &self.v
    }

    /// The largest magnitude among the present samples; 0 without one.
    #[must_use]
    pub fn longest(&self) -> f64 {
        self.longest
    }
}

/// The members of `values` that `kept` marks; `values` itself when it marks them all.
fn only(values: &SharedValues, kept: &[bool]) -> SharedValues {
    if kept.iter().all(|kept| *kept) {
        return values.clone();
    }
    fn chosen<T: Copy>(values: &[T], kept: &[bool]) -> Arc<[T]> {
        values
            .iter()
            .zip(kept)
            .filter(|(_, kept)| **kept)
            .map(|(value, _)| *value)
            .collect()
    }
    match values {
        SharedValues::Numbers(values) => SharedValues::Numbers(chosen(values, kept)),
        SharedValues::Times(times) => SharedValues::Times(chosen(times, kept)),
    }
}

/// Refuses a category axis: `Error::Usage` under `context`, x first.
fn check_axes(context: &'static str, (x, y): (ScaleKind, ScaleKind)) -> Result<()> {
    for (name, kind) in [("x", x), ("y", y)] {
        if kind == ScaleKind::Category {
            return Err(Error::usage(
                context,
                format!(
                    "the {name} axis is a category axis; a vector field needs linear, log or \
                     time axes."
                ),
            )
            .with_parameter(name)
            .with_hint("use Scale.linear(), Scale.log() or Scale.time() for both axes."));
        }
    }
    Ok(())
}

/// Checks the lengths of `x`, `y`, `u` and `v` against `form`: `Error::Data` under `context`.
fn check_lengths(
    context: &'static str,
    form: FieldForm,
    (x, y): (usize, usize),
    (u, v): (usize, usize),
) -> Result<()> {
    if u != v {
        return Err(Error::data(
            context,
            format!("u has {u} values but v has {v}; u and v must have the same shape."),
        )
        .with_parameter("v"));
    }
    match form {
        FieldForm::Points => {
            for (name, other) in [("y", y), ("u", u)] {
                if x != other {
                    return Err(Error::data(
                        context,
                        format!(
                            "x has {x} values but {name} has {other}; x, y, u and v must have \
                             the same length."
                        ),
                    )
                    .with_parameter(name)
                    .with_hint("pass one x, y, u and v per point."));
                }
            }
            Ok(())
        }
        FieldForm::Grid => {
            if x.checked_mul(y) == Some(u) {
                return Ok(());
            }
            let cells = (x as u128) * (y as u128);
            Err(Error::data(
                context,
                format!("u has {u} values; it must have len(y) * len(x) = {y} * {x} = {cells}."),
            )
            .with_parameter("u")
            .with_hint("pass one row of u and v per y coordinate, each with one value per x."))
        }
    }
}

/// Checks the coordinates `values` of a grid along an axis of scale kind `kind`, as `contour()`
/// does: finite, or present for times, and strictly increasing (`Error::Data` under `context`,
/// naming the first that is not).
fn check_grid_axis(
    context: &'static str,
    name: &'static str,
    values: &SharedValues,
    kind: ScaleKind,
) -> Result<()> {
    const SORT_HINT: &str = "sort the coordinates, and the rows or columns of u and v with them.";
    match (values, kind.transform()) {
        (SharedValues::Numbers(values), Some(transform)) => {
            check_ordered(context, name, values, transform, SORT_HINT)
        }
        (SharedValues::Numbers(_), None) => Ok(()),
        (SharedValues::Times(times), _) => {
            let mut last: Option<Instant> = None;
            for (i, time) in times.iter().enumerate() {
                let fault = |message: String, hint: &'static str| {
                    Error::data(context, message)
                        .with_parameter(name)
                        .with_hint(hint)
                };
                let Some(time) = *time else {
                    return Err(fault(
                        format!("{name}[{i}] is missing; coordinates must be finite."),
                        "pass finite coordinates within the supported numeric range.",
                    ));
                };
                if last.is_some_and(|last| time <= last) {
                    return Err(fault(
                        format!(
                            "{name}[{i}] is not after {name}[{}]; coordinates must be strictly \
                             increasing.",
                            i - 1
                        ),
                        SORT_HINT,
                    ));
                }
                last = Some(time);
            }
            Ok(())
        }
    }
}

/// Checks that a `color` mapping has one value per value of `u`: `Error::Data` under `context`
/// about `color`. One color passes.
pub(super) fn check_color_shape(
    context: &'static str,
    color: &PointColor,
    count: usize,
) -> Result<()> {
    match color {
        PointColor::By(by) if by.values.len() != count => Err(Error::data(
            context,
            format!(
                "color has {} values but u has {count}; color must have the shape of u.",
                by.values.len()
            ),
        )
        .with_parameter("color")
        .with_hint("pass one color value per vector, shaped as u.")),
        PointColor::By(_) | PointColor::Uniform(_) => Ok(()),
    }
}

/// The part of an arrow that sits on its sample.
///
/// # Examples
///
/// ```
/// use umber_core::Pivot;
///
/// assert_eq!(Pivot::default(), Pivot::Tail);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Pivot {
    /// The sample is where the arrow starts.
    #[default]
    Tail,
    /// The sample is at the middle of the arrow.
    Middle,
    /// The sample is where the arrow ends.
    Tip,
}

/// What the `(u, v)` of an arrow is measured against.
///
/// # Examples
///
/// ```
/// use umber_core::Direction;
///
/// assert_eq!(Direction::default(), Direction::Screen);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    /// The arrow points along `(u, v)` on the screen, positive `u` right and positive `v` up,
    /// whatever the scales of the axes: `u == v` is always 45°.
    #[default]
    Screen,
    /// The arrow points from `(x, y)` towards `(x + u, y + v)` on the plot, so it is tangent to
    /// a streamline of the same field on any plot shape. Both axes must be linear.
    Data,
}

/// How long an arrow of a given magnitude is.
///
/// # Examples
///
/// ```
/// use umber_core::ArrowLength;
///
/// assert_eq!(ArrowLength::default(), ArrowLength::Auto);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum ArrowLength {
    /// The longest arrow is as long as the typical spacing of the samples on the final plot, at
    /// most three quarters of the shorter side of the plot, and every arrow of the axes with
    /// this setting shares that scale.
    #[default]
    Auto,
    /// This many points per unit of magnitude, for this mark alone. It may draw no arrow longer
    /// than 14,400 pt, the side of the largest figure.
    PtPerUnit(f64),
}

/// A legend entry for a quiver: an arrow at the true length of a magnitude, then a text.
///
/// # Examples
///
/// ```
/// use umber_core::ArrowKey;
///
/// let key = ArrowKey::new(10.0, "10 m/s");
/// assert_eq!((key.value, key.text.as_str()), (10.0, "10 m/s"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ArrowKey {
    /// The magnitude the arrow is drawn at: greater than 0.
    pub value: f64,
    /// The text after the arrow, value and unit as the reader should see them.
    pub text: String,
}

impl ArrowKey {
    /// The entry of `value` with `text`.
    #[must_use]
    pub fn new(value: f64, text: impl Into<String>) -> ArrowKey {
        ArrowKey {
            value,
            text: text.into(),
        }
    }
}

/// Options of a quiver.
///
/// # Examples
///
/// ```
/// use umber_core::{ArrowLength, Direction, Pivot, PointColor, QuiverOptions};
///
/// let options = QuiverOptions::default();
/// assert_eq!((&options.color, options.pivot), (&PointColor::AUTO, Pivot::Tail));
/// assert_eq!((options.direction, options.length), (Direction::Screen, ArrowLength::Auto));
/// assert_eq!(options.key, None);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuiverOptions {
    /// The arrow color: one color, or a color per arrow mapped from values, which must have
    /// the shape of `u`.
    pub color: PointColor,
    /// The part of the arrow that sits on the sample.
    pub pivot: Pivot,
    /// What `(u, v)` is measured against.
    pub direction: Direction,
    /// How long an arrow of a given magnitude is.
    pub length: ArrowLength,
    /// The legend entry; `None` for no entry.
    pub key: Option<ArrowKey>,
}

/// Arrows on a field: filled, of a fixed head, and with lengths exactly proportional to the
/// magnitudes `sqrt(u² + v²)`, so a short arrow is never lengthened.
///
/// [`ArrowLength::PtPerUnit`] gives the scale directly. [`ArrowLength::Auto`] makes the longest
/// arrow as long as the typical spacing in points on the final plot: on a grid, the smaller of
/// the median column and row spacing; for points, the median distance to the nearest neighbour,
/// where points at one place count as one. All such quivers of an axes share one scale, with
/// the longest of their arrows and the smallest of their spacings. A field with no spacing,
/// a single arrow or all its points at one place, makes the longest arrow as long as an
/// automatic barb staff, 24 pt times the theme's font scale. The longest arrow is at most three
/// quarters of the shorter side of the plot, and shorter still where automatic limits could
/// not hold it, as a row of a few samples can ask.
///
/// Automatic limits hold whole arrows; explicit limits are kept and clip them. On axes with
/// equal aspect both [`Direction`]s draw the same arrows. Colors that a mapping brings close to
/// the background move away from it, as the lines of a contour do. The arrows of one color are
/// drawn together, so overlapping arrows of a mapping are not drawn in data order.
///
/// # Examples
///
/// ```
/// use umber_core::{ArrowKey, AxisData, FieldVectors, Figure, Mark, QuiverOptions, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let options = QuiverOptions { key: Some(ArrowKey::new(1.0, "1 m/s")), ..QuiverOptions::default() };
/// let x = AxisData::Numbers(vec![0.0, 1.0]);
/// axes.quiver(x.clone(), x, FieldVectors::points(vec![1.0, 0.0], vec![0.0, 1.0]), options)?;
/// let Some(Mark::Quiver(mark)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(mark.options().key.as_ref().map(|key| key.value), Some(1.0));
/// assert_eq!(mark.field().longest(), 1.0);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct QuiverMark {
    field: VectorField,
    options: QuiverOptions,
}

impl QuiverMark {
    /// Validates a quiver on axes of the scale kinds `kinds`: the field ([`VectorField::new`]),
    /// then, in this order, the first failure being the error:
    ///
    /// 1. a `pt_per_unit` is finite and greater than 0, and draws no arrow longer than 14,400 pt
    ///    (`Error::Style`);
    /// 2. a key has a value greater than 0 and at most 1e300, and a text (`Error::Style`);
    /// 3. [`Direction::Data`] comes with two linear axes (`Error::Usage`): a log or time axis
    ///    has no single scale for a step;
    /// 4. a color mapping has one value per value of `u` (`Error::Data`).
    pub(super) fn new(
        kinds: (ScaleKind, ScaleKind),
        coordinates: (AxisData, AxisData),
        vectors: FieldVectors,
        options: QuiverOptions,
    ) -> Result<QuiverMark> {
        let field = VectorField::new(QUIVER_CONTEXT, kinds, coordinates, vectors)?;
        if let ArrowLength::PtPerUnit(scale) = options.length {
            check_scale(scale, field.longest)?;
        }
        if let Some(key) = &options.key {
            check_key(key)?;
        }
        if options.direction == Direction::Data {
            check_data_direction(kinds)?;
        }
        check_color_shape(QUIVER_CONTEXT, &options.color, field.len())?;
        Ok(QuiverMark { field, options })
    }

    /// The field.
    #[must_use]
    pub fn field(&self) -> &VectorField {
        &self.field
    }

    /// The color, pivot, direction, length and key.
    #[must_use]
    pub fn options(&self) -> &QuiverOptions {
        &self.options
    }

    /// The text of the key; empty without one.
    pub(crate) fn key_text(&self) -> &str {
        self.options
            .key
            .as_ref()
            .map_or("", |key| key.text.as_str())
    }
}

/// Checks a `pt_per_unit` against the `longest` magnitude: `Error::Style`.
fn check_scale(scale: f64, longest: f64) -> Result<()> {
    let fault = |message: String| {
        Error::style(QUIVER_CONTEXT, message)
            .with_parameter("pt_per_unit")
            .with_hint("pass a number of points per unit of magnitude, such as 3, or AUTO.")
    };
    if !scale.is_finite() || scale <= 0.0 {
        return Err(fault(format!(
            "pt_per_unit is {}; it must be finite and greater than 0.",
            number(scale)
        )));
    }
    if scale * longest > MAX_ARROW_LENGTH {
        return Err(fault(format!(
            "pt_per_unit is {}, which draws the longest arrow {} pt long; arrows must be at most \
             {} pt long.",
            number(scale),
            number(scale * longest),
            number(MAX_ARROW_LENGTH)
        )));
    }
    Ok(())
}

/// Checks a key: `Error::Style` about `key`.
fn check_key(key: &ArrowKey) -> Result<()> {
    let fault = |message: String, hint: &'static str| {
        Error::style(QUIVER_CONTEXT, message)
            .with_parameter("key")
            .with_hint(hint)
    };
    if !key.value.is_finite() || key.value <= 0.0 || key.value > MAX_MAGNITUDE {
        return Err(fault(
            format!(
                "the key value is {}; it must be greater than 0 and at most {MAX_MAGNITUDE:e}.",
                number(key.value)
            ),
            "pass the magnitude the key arrow stands for, such as key=(10, \"10 m/s\").",
        ));
    }
    if key.text.is_empty() {
        return Err(fault(
            "the key text is empty; a key says what its arrow stands for.".to_owned(),
            "pass the value and its unit, such as key=(10, \"10 m/s\").",
        ));
    }
    Ok(())
}

/// Refuses [`Direction::Data`] on a log or time axis: `Error::Usage` about `direction`.
fn check_data_direction((x, y): (ScaleKind, ScaleKind)) -> Result<()> {
    for (name, kind) in [("x", x), ("y", y)] {
        if kind != ScaleKind::Linear {
            return Err(Error::usage(
                QUIVER_CONTEXT,
                format!(
                    "direction is Direction.DATA but the {name} axis is not linear; a log or \
                     time axis has no single scale for a step."
                ),
            )
            .with_parameter("direction")
            .with_hint("use Direction.SCREEN, or linear axes."));
        }
    }
    Ok(())
}

/// Checks that a barb can show every speed of `field`: `Error::Data` about `u`, naming the first
/// sample that it cannot, since more flags than a barb has would read as a lower speed.
fn check_speeds(field: &VectorField) -> Result<()> {
    for sample in field.samples() {
        let speed = magnitude(sample.u, sample.v);
        if !Feathers::of(speed).fits() {
            let at = match field.form {
                FieldForm::Points => format!("[{}]", sample.index),
                FieldForm::Grid => format!("[{}][{}]", sample.row, sample.column),
            };
            return Err(Error::data(
                BARBS_CONTEXT,
                format!(
                    "the speed at u{at}, v{at} is {}; barbs show speeds up to {MAX_SPEED}.",
                    number(speed)
                ),
            )
            .with_parameter("u")
            .with_hint("pass the wind in a unit that gives smaller values."));
        }
    }
    Ok(())
}

/// Options of wind barbs.
///
/// # Examples
///
/// ```
/// use umber_core::{BarbOptions, PointColor, SizeSpec};
///
/// let options = BarbOptions::default();
/// assert_eq!((&options.color, options.size), (&PointColor::AUTO, SizeSpec::Auto));
/// assert!(options.label.is_empty() && !options.flip);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BarbOptions {
    /// The legend label; empty for none. The entry is a line swatch.
    pub label: String,
    /// The barb color: one color, or a color per barb mapped from values, which must have the
    /// shape of `u`.
    pub color: PointColor,
    /// The length of the staff; [`SizeSpec::Auto`] for 24 pt times the theme's font scale, and
    /// [`SizeSpec::Times`] for a multiple of that. Lines and flags scale with it.
    pub size: SizeSpec,
    /// Whether lines and flags lie on the left of the staff, as in the southern hemisphere.
    pub flip: bool,
}

/// Wind barbs on a field. The speed `sqrt(u² + v²)` is rounded to the nearest 5, halves upward,
/// and written from the outer end of the staff inwards: a flag for each 50, a full line for
/// each 10, a half line for a remaining 5. The values are taken as they are; no unit is
/// converted. A speed that rounds to 0 draws an open circle.
///
/// `(u, v)` is where the wind blows to: the staff starts at the sample and runs against it,
/// always in the screen direction, since barbs are read as compass bearings. Lines and flags
/// lie to the right when looking from the sample along the staff, towards low pressure in the
/// northern hemisphere. Automatic limits hold whole barbs; explicit limits clip them. The barbs
/// of one color are drawn together, so overlapping barbs of a mapping are not drawn in data
/// order. A speed that rounds above 3245, 64 flags and all the lines, is refused.
///
/// # Examples
///
/// ```
/// use umber_core::{AxisData, BarbOptions, FieldVectors, Figure, Mark, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let x = AxisData::Numbers(vec![0.0, 1.0]);
/// axes.barbs(x.clone(), x, FieldVectors::points(vec![15.0, 0.0], vec![0.0, 0.0]), BarbOptions::default())?;
/// let Some(Mark::Barbs(mark)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(mark.field().longest(), 15.0);
/// assert!(!mark.options().flip);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BarbsMark {
    field: VectorField,
    options: BarbOptions,
}

impl BarbsMark {
    /// Validates barbs on axes of the scale kinds `kinds`: the field ([`VectorField::new`]),
    /// then that every speed can be drawn, rounding to at most 3245, and that a color mapping
    /// has one value per value of `u` (`Error::Data`).
    pub(super) fn new(
        kinds: (ScaleKind, ScaleKind),
        coordinates: (AxisData, AxisData),
        vectors: FieldVectors,
        options: BarbOptions,
    ) -> Result<BarbsMark> {
        let field = VectorField::new(BARBS_CONTEXT, kinds, coordinates, vectors)?;
        check_speeds(&field)?;
        check_color_shape(BARBS_CONTEXT, &options.color, field.len())?;
        Ok(BarbsMark { field, options })
    }

    /// The field.
    #[must_use]
    pub fn field(&self) -> &VectorField {
        &self.field
    }

    /// The label, color, size and side.
    #[must_use]
    pub fn options(&self) -> &BarbOptions {
        &self.options
    }
}
