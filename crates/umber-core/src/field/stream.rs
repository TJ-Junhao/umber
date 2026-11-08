//! Streamlines: lines that follow a vector field sampled on a grid, chosen and traced in the
//! same way on every platform and for every figure size.
//!
//! # Where a line lives
//!
//! A line is traced in data coordinates through the field, which is bilinear in each cell of
//! the grid, so it is a true streamline of that field on any axes. Lengths along it are measured
//! in *steps*: the extent of the grid along each axis, on the axis scale (in logarithms on a log
//! axis), is divided into `30 × density` steps ([`STEPS_PER_DENSITY`]). Spacing between lines,
//! the size of an integration step and the length of a line are all counted in steps, so
//! nothing about which lines are drawn, or where their arrow heads sit, depends on the figure
//! size or the layout.
//!
//! # Integrator
//!
//! A line is parametrized by its length in steps, `dp/ds = f(p) / |J f(p)|`, where `f` is the
//! bilinear field and `J` the scale of a data unit in steps along each axis. Each step is
//! Heun's method (the explicit trapezoidal rule, second order). A step is accepted when the
//! distance in steps between its end and the end of the Euler step it starts with is at most
//! [`TOLERANCE`], 0.01 of a step, and the unit direction of the line changes over it by at most
//! [`MAX_TURN`], about 0.05 radians. The second bound holds the accuracy to the curvature of the
//! line, however small its radius: Heun's error over a step that turns by `θ` is of the order of
//! `θ³` of the radius, so through a rotation, whose bilinear field is exact, every point of an
//! orbit stays within 0.1% of its radius, from orbits a hundredth of a step across to orbits
//! many steps across. A step starts at [`MAX_STEP`], a quarter of a step; a rejected step is
//! halved, and a step well within both bounds doubles, back up to [`MAX_STEP`]. Halving and
//! doubling are exact in binary, so step sizes are the same everywhere.
//!
//! A step must also be clear: the straight segment it draws, and the Euler step it starts with,
//! may cross no hole and pass no point where the speed is zero ([`Field::clear`]). The segment
//! is cut at the lines of the grid and each piece tested in its own cell, so a step never jumps
//! a hole, however narrow, nor a line where the flow stops, whether the field changes sign there
//! or not. A step that is not clear is halved like a rejected one.
//!
//! Speeds are the components times the steps per data unit, either of which can lie far beyond
//! the range of `f64`; they are kept as mantissas and powers of two ([`Wide`]), as the
//! direction of an arrow is, so a direction holds at any range of the data.
//!
//! Only addition, subtraction, multiplication, division and the square root are used, which
//! IEEE 754 rounds correctly; the logarithm and the exponential of a log axis are computed from
//! those ([`ln`], [`exp`]), with a fixed number of terms. The output is the same, bit for bit,
//! on every platform.
//!
//! # Where a line ends
//!
//! A line is traced from its seed upstream, then downstream, each way until:
//!
//! - it would leave the grid, or enter a hole: the four cells around a NaN of `u`, `v` or a
//!   mapped color value. Steps that would cross are halved until they fall below
//!   [`MIN_STEP`], 1/4096 of a step, so the line ends within 1/2048 of a step of the edge;
//! - it would reach a point where the speed is zero: at or below 1e-6 of the largest speed at a
//!   grid point, both measured in steps per unit of time ([`ZERO_SPEED`]); it ends within
//!   1/2048 of a step of it, as at a hole. A step that moves less than half its length turns
//!   back on itself, as at a point where the flow runs in from both sides, and is halved too;
//! - an automatic line would come within [`END_SPACING`], half a step, of another line;
//! - it would come within [`END_SPACING`] of a point of its own line more than [`SELF_GAP`],
//!   two steps, away along the line: a closed orbit ends when it has gone once round, half a
//!   step short of closing, and a spiral when its turns come that close;
//! - it has turned once round and come back within [`END_SPACING`] of its seed: an orbit too
//!   short for the rule above, less than two steps round, ends after one turn
//!   ([`Turning`]). Its other way is not traced;
//! - it is [`max_length`] steps long either way, a bound that a line meeting the rules above
//!   does not reach in practice.
//!
//! # Seeds
//!
//! Automatic seeds lie at the centres of a lattice of `ceil(30 × density)` squares along each
//! axis over the grid, measured in steps. They are tried ring by ring from the edge of the grid
//! inward; each ring starts at its lower left square and runs right along its bottom, up its
//! right side, left along its top and down its left side. A seed is used when it lies in a cell
//! that is not a hole, the speed there is not zero, and no line comes within [`SEED_SPACING`],
//! 0.9 of a step. An automatic line shorter than [`MIN_LENGTH`], two steps, is dropped. A given
//! start draws one line through its point, both ways, regardless of other lines.
//!
//! # Arrow heads
//!
//! A line at least [`HEAD_LENGTH_MIN`] long, one step, carries one head at its middle, measured
//! in steps along the line, pointing downstream ([`Streamline::head`]).

use super::{Wide, length, unit};

/// The steps along each axis per unit of density.
pub(crate) const STEPS_PER_DENSITY: f64 = 30.0;
/// The smallest density.
pub(crate) const MIN_DENSITY: f64 = 0.1;
/// The largest density.
pub(crate) const MAX_DENSITY: f64 = 10.0;
/// How close, in steps, an automatic seed may come to a line: a little less than the pitch of
/// the seeds, one step, so that in a uniform flow every row of seeds starts a line.
pub(crate) const SEED_SPACING: f64 = 0.9;
/// How close, in steps, a line may come to another line, or to its own line further than
/// [`SELF_GAP`] away along it, before it ends.
pub(crate) const END_SPACING: f64 = 0.5;
/// How far apart along a line, in steps, two of its points must be for their nearness to end it.
pub(crate) const SELF_GAP: f64 = 2.0;
/// The shortest automatic line that is kept, in steps.
pub(crate) const MIN_LENGTH: f64 = 2.0;
/// The shortest line that carries an arrow head, in steps.
pub(crate) const HEAD_LENGTH_MIN: f64 = 1.0;
/// The longest integration step, in steps.
pub(crate) const MAX_STEP: f64 = 0.25;
/// The shortest integration step, in steps: [`MAX_STEP`] halved ten times.
pub(crate) const MIN_STEP: f64 = MAX_STEP / 1024.0;
/// The largest accepted distance, in steps, between the end of a Heun step and the end of the
/// Euler step it starts with.
pub(crate) const TOLERANCE: f64 = 0.01;
/// The largest accepted change of the unit direction of a line over a step, in steps: about
/// 0.05 radians, so that a step turns by at most that, whatever the radius of the curve.
pub(crate) const MAX_TURN: f64 = 0.05;
/// The share of the largest speed at a grid point at or below which the speed counts as zero.
pub(crate) const ZERO_SPEED: f64 = 1e-6;

/// The longest a line may grow either way from its seed, in steps, on a grid of `steps` steps
/// along each axis: eight times the area of the grid in square steps, and at least 64.
pub(crate) fn max_length(steps: f64) -> f64 {
    (8.0 * steps * steps).max(64.0)
}

/// `ln(2)`, the `f64` nearest to it.
const LN_2: f64 = std::f64::consts::LN_2;
/// The leading bits of `ln(2)`, few enough that a whole multiple of up to 2^11 is exact.
const LN_2_HIGH: f64 = f64::from_bits(0x3fe6_2e42_fee0_0000);
/// `ln(2)` less [`LN_2_HIGH`], to the precision of `f64`.
const LN_2_LOW: f64 = 1.908_214_929_270_587_7e-10;
/// `sqrt(2)`, the `f64` nearest to it.
const SQRT_2: f64 = std::f64::consts::SQRT_2;

/// The natural logarithm of the positive `x`, of any size, from operations that IEEE 754 rounds
/// correctly, so it is the same on every platform: `x` is a mantissa between `sqrt(1/2)` and
/// `sqrt(2)` times a power of two, and the logarithm of the mantissa is `2 atanh(t)` for
/// `t = (m - 1) / (m + 1)`, a series in `t²` summed to the term in `t^27`, within a few units in
/// the last place.
pub(crate) fn ln(x: Wide) -> f64 {
    let (mantissa, power) = x.parts();
    let (m, power) = if mantissa > SQRT_2 {
        (mantissa / 2.0, power + 1)
    } else {
        (mantissa, power)
    };
    let t = (m - 1.0) / (m + 1.0);
    let square = t * t;
    let mut sum = 0.0;
    for k in (0..=13_u32).rev() {
        sum = sum * square + 1.0 / f64::from(2 * k + 1);
    }
    let power = f64::from(power);
    power * LN_2_HIGH + (2.0 * t * sum + power * LN_2_LOW)
}

/// `e^x` for a finite `x`, as a number of any size, from operations that IEEE 754 rounds
/// correctly, as [`ln`] is: `x = k ln(2) + r` with a whole `k` and `|r| <= ln(2) / 2`, taking
/// `k ln(2)` in two parts so that `r` keeps its precision, `e^r` as its Taylor series to the term
/// in `r^20`, and `2^k` exact.
pub(crate) fn exp(x: f64) -> Wide {
    let k = (x / LN_2).round();
    let r = (x - k * LN_2_HIGH) - k * LN_2_LOW;
    let mut sum = 1.0;
    for n in (1..=20_u32).rev() {
        sum = 1.0 + r * sum / f64::from(n);
    }
    // The cast saturates; a power that large is far past any coordinate.
    Wide::from_parts(sum, k as i32)
}

/// One axis of the grid as steps measure it.
#[derive(Debug, Clone, Copy)]
struct Measure {
    /// Whether the axis is a log axis, on which steps are measured in logarithms.
    log: bool,
    /// The steps per unit of the axis scale.
    per_unit: f64,
    /// The first and the last coordinate.
    ends: [f64; 2],
}

impl Measure {
    /// The measure of the strictly increasing `coordinates`, at least two, divided into `steps`.
    /// On a log axis the span is the logarithm of the ratio of the ends, which holds its
    /// precision however close the ends lie.
    fn new(coordinates: &[f64], log: bool, steps: f64) -> Measure {
        let ends = [coordinates[0], coordinates[coordinates.len() - 1]];
        let span = if log {
            ln(Wide::of(ends[1]).over(Wide::of(ends[0])))
        } else {
            ends[1] - ends[0]
        };
        Measure {
            log,
            per_unit: steps / span,
            ends,
        }
    }

    /// The position of the coordinate `v` in steps from the first coordinate.
    fn at(&self, v: f64) -> f64 {
        let scaled = if self.log {
            ln(Wide::of(v).over(Wide::of(self.ends[0])))
        } else {
            v - self.ends[0]
        };
        scaled * self.per_unit
    }

    /// The coordinate at `steps` from the first one, within the grid.
    fn value(&self, steps: f64) -> f64 {
        let v = if self.log {
            exp(steps / self.per_unit)
                .times(Wide::of(self.ends[0]))
                .value()
        } else {
            self.ends[0] + steps / self.per_unit
        };
        v.clamp(self.ends[0], self.ends[1])
    }

    /// The steps per data unit at the coordinate `v`, of any size.
    fn rate(&self, v: f64) -> Wide {
        let per_unit = Wide::of(self.per_unit);
        if self.log {
            per_unit.over(Wide::of(v))
        } else {
            per_unit
        }
    }
}

/// A grid field to trace streamlines through: validated coordinates and components.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Grid<'a> {
    /// The x coordinates, finite and strictly increasing, at least two.
    pub x: &'a [f64],
    /// The y coordinates, finite and strictly increasing, at least two.
    pub y: &'a [f64],
    /// The component along x, row by row: NaN or finite.
    pub u: &'a [f64],
    /// The component along y, row by row: NaN or finite.
    pub v: &'a [f64],
    /// Mapped color values, row by row, whose NaN makes a hole as one in `u` does; `None` for a
    /// line in one color.
    pub values: Option<&'a [f64]>,
    /// Whether each axis, x then y, is a log axis.
    pub log: [bool; 2],
    /// The density: `30 × density` steps along each axis.
    pub density: f64,
}

/// Why no line can pass at a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Blocked {
    /// The point lies outside the grid.
    Outside,
    /// The point lies in a hole: a cell with a NaN corner.
    Hole,
    /// The speed is zero there.
    Still,
}

/// Where an arrow head sits on a line: the fraction `fraction` of the way along the segment
/// from point `segment` to the next, measured in steps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Head {
    /// The index of the point the segment starts at.
    pub segment: usize,
    /// How far along the segment, from 0 to 1.
    pub fraction: f64,
}

/// A streamline: its points in data coordinates, from upstream to downstream.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Mark, ScaleKind, StreamplotOptions, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let (x, y) = ([0.0, 1.0, 2.0], [0.0, 1.0]);
/// axes.streamplot(&x, &y, &[1.0; 6], &[0.0; 6], StreamplotOptions::default())?;
/// let Some(Mark::Streamplot(mark)) = axes.marks().first() else { unreachable!() };
/// let line = &mark.lines()[0];
/// // A uniform flow to the right: each line runs straight from the left edge to the right.
/// let (first, last) = (line.points()[0], line.points()[line.points().len() - 1]);
/// assert!(first[0] < last[0] && first[1] == last[1]);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Streamline {
    points: Vec<[f64; 2]>,
    lengths: Vec<f64>,
    values: Vec<f64>,
    head: Option<Head>,
}

impl Streamline {
    /// The points of the line in data coordinates, `[x, y]`, from upstream to downstream.
    #[must_use]
    pub fn points(&self) -> &[[f64; 2]] {
        &self.points
    }

    /// The length of the line, in steps, from its first point to each of its points.
    pub(crate) fn lengths(&self) -> &[f64] {
        &self.lengths
    }

    /// The mapped color value at each point; empty for a line in one color.
    pub(crate) fn values(&self) -> &[f64] {
        &self.values
    }

    /// The length of the line, in steps.
    pub(crate) fn length(&self) -> f64 {
        self.lengths.last().copied().unwrap_or(0.0)
    }

    /// Where its arrow head sits: at the middle of the line, measured in steps; `None` for a
    /// line shorter than [`HEAD_LENGTH_MIN`].
    pub(crate) fn head(&self) -> Option<Head> {
        self.head
    }
}

/// The field of a [`Grid`] ready to be traced, with its holes found.
///
/// Speeds, in steps per unit of time, are the components times the steps per data unit, which
/// either can make far larger or smaller than an `f64` holds: they are kept as [`Wide`] numbers,
/// so a direction is exact at any range of the data, as the direction of an arrow is.
struct Field<'a> {
    /// The grid.
    grid: Grid<'a>,
    /// Whether each cell, row by row, has four finite corners.
    open: Vec<bool>,
    /// The measures of the x and the y axis.
    measures: [Measure; 2],
    /// The speed at or below which the speed is zero.
    still: Wide,
    /// The steps along each axis.
    steps: f64,
}

/// The field at a point: the unit vector of the line there in steps, the same direction in data
/// units per step, and the mapped color value there.
#[derive(Debug, Clone, Copy)]
struct Probe {
    /// The unit vector of the line, in steps.
    unit: [f64; 2],
    /// `dp/ds`, in data units per step.
    slope: [f64; 2],
    /// The mapped color value; `None` for a line in one color.
    value: Option<f64>,
}

/// Where a point lies in its cell: the column and row of the cell, and the fractions of the way
/// across it along x and along y.
type Place = (usize, usize, f64, f64);

impl<'a> Field<'a> {
    /// The field of `grid`.
    fn new(grid: Grid<'a>) -> Field<'a> {
        let steps = STEPS_PER_DENSITY * grid.density;
        let measures = [
            Measure::new(grid.x, grid.log[0], steps),
            Measure::new(grid.y, grid.log[1], steps),
        ];
        let (columns, rows) = (grid.x.len(), grid.y.len());
        let finite = |i: usize| {
            grid.u[i].is_finite()
                && grid.v[i].is_finite()
                && grid.values.is_none_or(|values| values[i].is_finite())
        };
        let mut open = Vec::with_capacity((columns - 1) * (rows - 1));
        for row in 0..rows - 1 {
            for column in 0..columns - 1 {
                let corner = row * columns + column;
                open.push(
                    finite(corner)
                        && finite(corner + 1)
                        && finite(corner + columns)
                        && finite(corner + columns + 1),
                );
            }
        }
        let mut field = Field {
            grid,
            open,
            measures,
            still: Wide::ZERO,
            steps,
        };
        let mut fastest = Wide::ZERO;
        for row in 0..rows {
            for column in 0..columns {
                let i = row * columns + column;
                if !finite(i) {
                    continue;
                }
                let point = [grid.x[column], grid.y[row]];
                let [p, q] = field.velocity(point, [grid.u[i], grid.v[i]]);
                let speed = length(p, q);
                if !speed.at_most(fastest) {
                    fastest = speed;
                }
            }
        }
        field.still = Wide::of(ZERO_SPEED).times(fastest);
        field
    }

    /// The velocity in steps per unit of time of the vector `(u, v)` at `point`.
    fn velocity(&self, [x, y]: [f64; 2], [u, v]: [f64; 2]) -> [Wide; 2] {
        [
            Wide::of(u).times(self.measures[0].rate(x)),
            Wide::of(v).times(self.measures[1].rate(y)),
        ]
    }

    /// Whether the velocity `[p, q]` has zero speed: at most [`ZERO_SPEED`] of the largest speed
    /// at a grid point.
    fn is_still(&self, [p, q]: [Wide; 2]) -> bool {
        length(p, q).at_most(self.still)
    }

    /// The position of `point` in steps from the first coordinates.
    fn position(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        [self.measures[0].at(x), self.measures[1].at(y)]
    }

    /// The point at the position `at` in steps, within the grid.
    fn point(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        [self.measures[0].value(x), self.measures[1].value(y)]
    }

    /// The cell of `point` and the fractions of the way across it; `None` outside the grid. A
    /// point on a line between cells belongs to the cell after it, except on the last line.
    fn cell(&self, [x, y]: [f64; 2]) -> Option<Place> {
        let along = |values: &[f64], v: f64| {
            let last = values.len() - 1;
            if !(values[0]..=values[last]).contains(&v) {
                return None;
            }
            let i = (values.partition_point(|&c| c <= v).max(1) - 1).min(last - 1);
            Some((i, (v - values[i]) / (values[i + 1] - values[i])))
        };
        let (column, tx) = along(self.grid.x, x)?;
        let (row, ty) = along(self.grid.y, y)?;
        Some((column, row, tx, ty))
    }

    /// Whether the cell at `column` and `row` has four finite corners.
    fn is_open(&self, column: usize, row: usize) -> bool {
        self.open[row * (self.grid.x.len() - 1) + column]
    }

    /// The indices in `u` and `v` of the corners of the cell at `column` and `row`: lower left,
    /// lower right, upper left, upper right.
    fn corners(&self, column: usize, row: usize) -> [usize; 4] {
        let columns = self.grid.x.len();
        let corner = row * columns + column;
        [corner, corner + 1, corner + columns, corner + columns + 1]
    }

    /// The bilinear value of `values` at `(tx, ty)` of the cell with `corners`.
    fn bilinear(values: &[f64], corners: [usize; 4], tx: f64, ty: f64) -> f64 {
        let [a, b, c, d] = corners.map(|i| values[i]);
        (a * (1.0 - tx) + b * tx) * (1.0 - ty) + (c * (1.0 - tx) + d * tx) * ty
    }

    /// The velocity at `point`, at `place` in an open cell.
    fn velocity_in(&self, point: [f64; 2], (column, row, tx, ty): Place) -> [Wide; 2] {
        let corners = self.corners(column, row);
        let u = Field::bilinear(self.grid.u, corners, tx, ty);
        let v = Field::bilinear(self.grid.v, corners, tx, ty);
        self.velocity(point, [u, v])
    }

    /// The field at `point`, or why no line can pass there.
    fn probe(&self, point: [f64; 2]) -> Result<Probe, Blocked> {
        let place @ (column, row, tx, ty) = self.cell(point).ok_or(Blocked::Outside)?;
        if !self.is_open(column, row) {
            return Err(Blocked::Hole);
        }
        let [p, q] = self.velocity_in(point, place);
        if self.is_still([p, q]) {
            return Err(Blocked::Still);
        }
        let unit = unit(p, q);
        // The unit vector in steps over the steps per data unit: the data units per step.
        let slope = [
            Wide::of(unit[0])
                .over(self.measures[0].rate(point[0]))
                .value(),
            Wide::of(unit[1])
                .over(self.measures[1].rate(point[1]))
                .value(),
        ];
        let corners = self.corners(column, row);
        let value = self
            .grid
            .values
            .map(|values| Field::bilinear(values, corners, tx, ty));
        Ok(Probe { unit, slope, value })
    }

    /// The field at `point` where a line can pass there.
    fn flowing(&self, point: [f64; 2]) -> Option<Probe> {
        self.probe(point).ok()
    }

    /// Whether a line can run straight from `from` to `to`: every cell the segment crosses is
    /// open, and the speed is zero nowhere along it ([`Field::clear_piece`]). The segment is cut
    /// where it crosses the lines of the grid, and each piece is tested in its own cell, so a
    /// step never jumps a hole, however narrow, nor a point where the flow stops.
    fn clear(&self, from: [f64; 2], to: [f64; 2]) -> bool {
        let mut cuts = vec![0.0, 1.0];
        for (axis, coordinates) in [self.grid.x, self.grid.y].into_iter().enumerate() {
            let (a, b) = (from[axis], to[axis]);
            if a == b {
                continue;
            }
            let (low, high) = (a.min(b), a.max(b));
            let first = coordinates.partition_point(|&c| c <= low);
            let end = coordinates.partition_point(|&c| c < high).max(first);
            cuts.extend(coordinates[first..end].iter().map(|&c| (c - a) / (b - a)));
        }
        cuts.sort_by(f64::total_cmp);
        cuts.windows(2)
            .all(|pair| self.clear_piece(from, to, [pair[0], pair[1]]))
    }

    /// Whether the piece `[start, end]` of the segment from `from` to `to`, which crosses no line
    /// of the grid, lies in an open cell where the speed is zero nowhere along it.
    ///
    /// Along a straight piece each component of the bilinear field is a quadratic in the
    /// parameter, so the speed can only be zero where a component is: the piece is tested at its
    /// ends, and at the roots and the turning point of each component's quadratic that lie
    /// within it. A zero between grid points, where the field changes sign, and one at a grid
    /// point or on a line of the grid, where it may not, are both found.
    fn clear_piece(&self, from: [f64; 2], to: [f64; 2], [start, end]: [f64; 2]) -> bool {
        let at = |t: f64| {
            [
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
            ]
        };
        let Some((column, row, _, _)) = self.cell(at(start / 2.0 + end / 2.0)) else {
            return false;
        };
        if !self.is_open(column, row) {
            return false;
        }
        // The fractions across the cell along the piece: `tx = alpha + beta t`, and the same for
        // `ty` with `gamma` and `delta`.
        let local = |axis: usize, coordinates: &[f64], i: usize| {
            let width = coordinates[i + 1] - coordinates[i];
            (
                (from[axis] - coordinates[i]) / width,
                (to[axis] - from[axis]) / width,
            )
        };
        let (alpha, beta) = local(0, self.grid.x, column);
        let (gamma, delta) = local(1, self.grid.y, row);
        let corners = self.corners(column, row);
        let mut candidates = vec![start, end];
        for values in [self.grid.u, self.grid.v] {
            let [a, b, c, d] = corners.map(|i| values[i]);
            let largest = a.abs().max(b.abs()).max(c.abs()).max(d.abs());
            if largest == 0.0 {
                continue;
            }
            let [a, b, c, d] = [a, b, c, d].map(|value| value / largest);
            // f = a + (b - a) tx + (c - a) ty + (a - b - c + d) tx ty.
            let (linear_x, linear_y, cross) = (b - a, c - a, a - b - c + d);
            let quadratic = [
                a + linear_x * alpha + linear_y * gamma + cross * alpha * gamma,
                linear_x * beta + linear_y * delta + cross * (alpha * delta + beta * gamma),
                cross * beta * delta,
            ];
            candidates.extend(
                critical_points(quadratic)
                    .into_iter()
                    .flatten()
                    .filter(|t| (start..=end).contains(t)),
            );
        }
        candidates.into_iter().all(|t| {
            let point = at(t);
            let tx = (alpha + beta * t).clamp(0.0, 1.0);
            let ty = (gamma + delta * t).clamp(0.0, 1.0);
            !self.is_still(self.velocity_in(point, (column, row, tx, ty)))
        })
    }
}

/// The roots and the turning point of `c0 + c1 t + c2 t²`, given as `[c0, c1, c2]`, where it
/// has them.
fn critical_points([c0, c1, c2]: [f64; 3]) -> [Option<f64>; 3] {
    if c2 == 0.0 {
        return [(c1 != 0.0).then(|| -c0 / c1), None, None];
    }
    let turning = Some(-c1 / (2.0 * c2));
    let discriminant = c1 * c1 - 4.0 * c2 * c0;
    if discriminant < 0.0 {
        return [None, None, turning];
    }
    // The root of larger magnitude first, then the other from their product, without
    // cancellation.
    let q = -0.5 * (c1 + c1.signum() * discriminant.sqrt());
    if q == 0.0 {
        return [Some(0.0), None, turning];
    }
    [Some(q / c2), Some(c0 / q), turning]
}

/// The distance between two positions in steps.
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
    (dx * dx + dy * dy).sqrt()
}

/// A point of a line in the occupancy: its position in steps, its line and its length along the
/// line from the seed, negative upstream.
#[derive(Debug, Clone, Copy)]
struct Entry {
    at: [f64; 2],
    line: u32,
    length: f64,
}

/// The points of the lines drawn so far, in squares one step wide, so that a test of nearness
/// reads only the nine squares around a point: tracing stays linear in the number of points.
struct Occupancy {
    /// The squares along each axis.
    side: usize,
    /// The indices in `entries` of the points in each square, row by row, in the order they
    /// were added.
    squares: Vec<Vec<usize>>,
    /// The entries, in the order they were added, each with its square.
    entries: Vec<(Entry, usize)>,
}

impl Occupancy {
    /// The occupancy of a grid `steps` steps wide along each axis.
    fn new(steps: f64) -> Occupancy {
        let side = (steps.ceil() as usize).max(1);
        Occupancy {
            side,
            squares: vec![Vec::new(); side * side],
            entries: Vec::new(),
        }
    }

    /// The column and row of the square of the position `at`.
    fn square(&self, at: [f64; 2]) -> (usize, usize) {
        let index = |v: f64| (v.max(0.0) as usize).min(self.side - 1);
        (index(at[0]), index(at[1]))
    }

    /// Adds the point at `at` of `line`, `length` along it.
    fn insert(&mut self, at: [f64; 2], line: u32, length: f64) {
        let (column, row) = self.square(at);
        let square = row * self.side + column;
        self.squares[square].push(self.entries.len());
        self.entries.push((Entry { at, line, length }, square));
    }

    /// How many entries there are.
    fn len(&self) -> usize {
        self.entries.len()
    }

    /// Removes the entries added after the first `count`: each is the last of its square.
    fn truncate(&mut self, count: usize) {
        while self.entries.len() > count {
            if let Some((_, square)) = self.entries.pop() {
                self.squares[square].pop();
            }
        }
    }

    /// Whether `test` holds for an entry within `radius`, at most one step, of `at`.
    fn any_near(&self, at: [f64; 2], radius: f64, test: impl Fn(&Entry) -> bool) -> bool {
        let (column, row) = self.square(at);
        let around = |i: usize| i.saturating_sub(1)..=(i + 1).min(self.side - 1);
        around(row).any(|row| {
            around(column).any(|column| {
                self.squares[row * self.side + column].iter().any(|&i| {
                    let (entry, _) = &self.entries[i];
                    let (dx, dy) = (entry.at[0] - at[0], entry.at[1] - at[1]);
                    dx * dx + dy * dy < radius * radius && test(entry)
                })
            })
        })
    }
}

/// Which lines a line keeps away from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Company {
    /// Every other line: an automatic line.
    Others,
    /// Only its own points: a line through a given start.
    Alone,
}

/// One accepted point of a half line: the point, its length from the seed and its mapped value.
type Traced = ([f64; 2], f64, Option<f64>);

/// The tracer of one field: the field and the occupancy of the lines traced so far.
struct Tracer<'a> {
    field: Field<'a>,
    occupancy: Occupancy,
    /// The longest a line may grow either way.
    max_length: f64,
}

/// One way of a line from its seed: its points, and whether it ended by closing an orbit, in
/// which case the other way is not traced.
struct Half {
    /// The accepted points, from the seed outward.
    traced: Vec<Traced>,
    /// Whether the line turned once round and came back to its seed.
    closed: bool,
}

/// How the direction of a half line has turned since its seed, to find a full turn without an
/// angle: the line has turned once round when its direction, having pointed away from its first
/// direction, comes back across it from the side opposite the one it first turned to.
#[derive(Debug, Clone, Copy)]
struct Turning {
    /// The unit vector of the line at its seed, in steps.
    first: [f64; 2],
    /// Whether it first turned to the left of `first`; `None` while it has not turned.
    left: Option<bool>,
    /// Whether it has pointed more than a right angle away from `first` since the last turn.
    away: bool,
    /// The cross product of `first` and the last direction.
    cross: f64,
}

impl Turning {
    /// The turning of a line whose first direction is `first`.
    fn new(first: [f64; 2]) -> Turning {
        Turning {
            first,
            left: None,
            away: false,
            cross: 0.0,
        }
    }

    /// Takes the next direction `unit`; whether the line has just completed a turn.
    fn next(&mut self, unit: [f64; 2]) -> bool {
        let [a, b] = self.first;
        let cross = a * unit[1] - b * unit[0];
        let dot = a * unit[0] + b * unit[1];
        let before = self.cross;
        self.cross = cross;
        if self.left.is_none() && cross != 0.0 {
            self.left = Some(cross > 0.0);
        }
        if dot < 0.0 {
            self.away = true;
        }
        let Some(left) = self.left else {
            return false;
        };
        let crossed = if left {
            before < 0.0 && cross >= 0.0
        } else {
            before > 0.0 && cross <= 0.0
        };
        let turned = self.away && dot > 0.0 && crossed;
        if turned {
            self.away = false;
        }
        turned
    }
}

impl Tracer<'_> {
    /// Traces `line` from `seed` one way, `sign` 1 downstream and -1 upstream, adding its points
    /// to the occupancy (see the module documentation for where it ends).
    fn half(
        &mut self,
        seed: [f64; 2],
        start: Probe,
        line: u32,
        sign: f64,
        company: Company,
    ) -> Half {
        let field = &self.field;
        let mut traced = Vec::new();
        let (mut point, mut probe) = (seed, start);
        let origin = field.position(seed);
        let mut at = origin;
        let mut length = 0.0_f64;
        let mut step = MAX_STEP;
        let mut turning = Turning::new([sign * start.unit[0], sign * start.unit[1]]);
        let along =
            |p: [f64; 2], slope: [f64; 2], by: f64| [p[0] + by * slope[0], p[1] + by * slope[1]];
        loop {
            let euler = along(point, probe.slope, sign * step);
            let next = field
                .clear(point, euler)
                .then(|| field.flowing(euler))
                .flatten()
                .map(|middle| {
                    let slope = [
                        (probe.slope[0] + middle.slope[0]) / 2.0,
                        (probe.slope[1] + middle.slope[1]) / 2.0,
                    ];
                    along(point, slope, sign * step)
                })
                .filter(|&heun| field.clear(point, heun))
                .and_then(|heun| field.flowing(heun).map(|probe| (heun, probe)));
            let Some((next, next_probe)) = next else {
                step /= 2.0;
                if step < MIN_STEP {
                    break;
                }
                continue;
            };
            let next_at = field.position(next);
            let error = distance(field.position(euler), next_at);
            let turn = distance(probe.unit, next_probe.unit);
            let moved = distance(at, next_at);
            // A step that moves less than half its length turns back on itself: the line has run
            // into a point where the flow stops, and the step shrinks until the line ends.
            if error > TOLERANCE || turn > MAX_TURN || moved < step / 2.0 {
                step /= 2.0;
                if step < MIN_STEP {
                    break;
                }
                continue;
            }
            let next_length = length + sign * moved;
            if next_length.abs() > self.max_length {
                break;
            }
            let crowded = self.occupancy.any_near(next_at, END_SPACING, |entry| {
                if entry.line == line {
                    (entry.length - next_length).abs() > SELF_GAP
                } else {
                    company == Company::Others
                }
            });
            if crowded {
                break;
            }
            self.occupancy.insert(next_at, line, next_length);
            traced.push((next, next_length, next_probe.value));
            let unit = [sign * next_probe.unit[0], sign * next_probe.unit[1]];
            if turning.next(unit) && distance(next_at, origin) < END_SPACING {
                return Half {
                    traced,
                    closed: true,
                };
            }
            (point, probe, at, length) = (next, next_probe, next_at, next_length);
            if error < TOLERANCE / 4.0 && turn < MAX_TURN / 2.0 && step < MAX_STEP {
                step *= 2.0;
            }
        }
        Half {
            traced,
            closed: false,
        }
    }

    /// The line `line` through `seed`, where the field is `start`, traced both ways; only
    /// upstream when that closes an orbit.
    fn line(&mut self, seed: [f64; 2], start: Probe, line: u32, company: Company) -> Streamline {
        self.occupancy.insert(self.field.position(seed), line, 0.0);
        let upstream = self.half(seed, start, line, -1.0, company);
        let downstream = if upstream.closed {
            Vec::new()
        } else {
            self.half(seed, start, line, 1.0, company).traced
        };
        let upstream = upstream.traced;
        let first = upstream.last().map_or(0.0, |&(_, length, _)| length);
        let traced: Vec<Traced> = upstream
            .into_iter()
            .rev()
            .chain(std::iter::once((seed, 0.0, start.value)))
            .chain(downstream)
            .collect();
        let mut streamline = Streamline {
            points: traced.iter().map(|&(point, _, _)| point).collect(),
            lengths: traced
                .iter()
                .map(|&(_, length, _)| length - first)
                .collect(),
            // Every point of a mapped line has a value, and a line in one color has none.
            values: traced.iter().filter_map(|&(_, _, value)| value).collect(),
            head: None,
        };
        streamline.head = head_of(&streamline.lengths);
        streamline
    }
}

/// Where the head of a line with the cumulative `lengths` sits: at half its length; `None` for a
/// line shorter than [`HEAD_LENGTH_MIN`].
fn head_of(lengths: &[f64]) -> Option<Head> {
    let total = lengths.last().copied()?;
    if total < HEAD_LENGTH_MIN {
        return None;
    }
    let middle = total / 2.0;
    let segment = (lengths.partition_point(|&length| length <= middle).max(1) - 1)
        .min(lengths.len().saturating_sub(2));
    let (from, to) = (lengths[segment], lengths[segment + 1]);
    let fraction = if to > from {
        ((middle - from) / (to - from)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some(Head { segment, fraction })
}

impl<'a> Tracer<'a> {
    /// The tracer of `grid`.
    fn new(grid: Grid<'a>) -> Tracer<'a> {
        let field = Field::new(grid);
        Tracer {
            occupancy: Occupancy::new(field.steps),
            max_length: max_length(field.steps),
            field,
        }
    }
}

/// The squares of a lattice `side` squares wide along each axis, ring by ring from the edge
/// inward, each ring from its lower left square right along its bottom, up its right side, left
/// along its top and down its left side.
fn seed_order(side: usize) -> Vec<(usize, usize)> {
    let mut order = Vec::with_capacity(side * side);
    let mut seen = vec![false; side * side];
    for ring in 0..side.div_ceil(2) {
        let (low, high) = (ring, side - 1 - ring);
        let bottom = (low..=high).map(|column| (column, low));
        let right = (low..=high).map(|row| (high, row));
        let top = (low..=high).rev().map(|column| (column, high));
        let left = (low..=high).rev().map(|row| (low, row));
        for (column, row) in bottom.chain(right).chain(top).chain(left) {
            let index = row * side + column;
            if !seen[index] {
                seen[index] = true;
                order.push((column, row));
            }
        }
    }
    order
}

/// The automatic streamlines of `grid`, in the order their seeds were tried (see the module
/// documentation).
pub(crate) fn automatic(grid: Grid<'_>) -> Vec<Streamline> {
    let mut tracer = Tracer::new(grid);
    let side = tracer.occupancy.side;
    let pitch = tracer.field.steps / side as f64;
    let mut lines = Vec::new();
    for (column, row) in seed_order(side) {
        let at = [(column as f64 + 0.5) * pitch, (row as f64 + 0.5) * pitch];
        let seed = tracer.field.point(at);
        let Some(start) = tracer.field.flowing(seed) else {
            continue;
        };
        let position = tracer.field.position(seed);
        if tracer.occupancy.any_near(position, SEED_SPACING, |_| true) {
            continue;
        }
        let before = tracer.occupancy.len();
        let line = tracer.line(seed, start, lines.len() as u32, Company::Others);
        if line.length() < MIN_LENGTH {
            tracer.occupancy.truncate(before);
        } else {
            lines.push(line);
        }
    }
    lines
}

/// One streamline through each of `starts`, regardless of the others; or the index of the first
/// start where no line can pass, and why.
pub(crate) fn through(
    grid: Grid<'_>,
    starts: &[[f64; 2]],
) -> Result<Vec<Streamline>, (usize, Blocked)> {
    let mut tracer = Tracer::new(grid);
    let probes = starts
        .iter()
        .enumerate()
        .map(|(i, &start)| tracer.field.probe(start).map_err(|blocked| (i, blocked)))
        .collect::<Result<Vec<Probe>, _>>()?;
    Ok(starts
        .iter()
        .zip(probes)
        .map(|(&start, probe)| {
            tracer.occupancy.truncate(0);
            tracer.line(start, probe, 0, Company::Alone)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logarithm_and_the_exponential_match_the_library_closely() {
        for x in [1e-300, 1e-5, 0.3, 1.0, 1.5, 2.0, 7.25, 1e3, 1e300] {
            let relative = (ln(Wide::of(x)) - x.ln()).abs() / x.ln().abs().max(1.0);
            assert!(relative < 4e-16, "{x}: {} {}", ln(Wide::of(x)), x.ln());
        }
        assert_eq!(ln(Wide::of(1.0)), 0.0);
        // A ratio beyond the range of `f64` keeps its logarithm.
        let ratio = Wide::of(1e300).over(Wide::of(1e-300));
        assert!((ln(ratio) / (600.0 * 10_f64.ln()) - 1.0).abs() < 1e-15);
        for x in [-690.0, -10.0, -0.5, 0.0, 0.2, 1.0, 3.0, 700.0] {
            let relative = (exp(x).value() - x.exp()).abs() / x.exp();
            assert!(relative < 1e-15, "{x}: {} {}", exp(x).value(), x.exp());
        }
        assert_eq!(LN_2_HIGH + LN_2_LOW, LN_2);
        assert_eq!(exp(0.0).value(), 1.0);
        assert_eq!(exp(1000.0).times(exp(-1000.0)).value(), 1.0);
    }

    #[test]
    fn critical_points_are_roots_and_the_turning_point() {
        // (t - 1)(t - 3) = 3 - 4t + t².
        let found = critical_points([3.0, -4.0, 1.0]);
        let mut roots: Vec<f64> = found[..2].iter().flatten().copied().collect();
        roots.sort_by(f64::total_cmp);
        assert_eq!((roots, found[2]), (vec![1.0, 3.0], Some(2.0)));
        assert_eq!(critical_points([1.0, 0.0, 1.0]), [None, None, Some(0.0)]);
        assert_eq!(critical_points([2.0, -4.0, 0.0]), [Some(0.5), None, None]);
        assert_eq!(critical_points([2.0, 0.0, 0.0]), [None, None, None]);
    }

    #[test]
    fn a_turn_is_counted_once_round_and_not_on_the_way_back() {
        let directions = |angles: &[f64]| -> Vec<[f64; 2]> {
            angles.iter().map(|&a: &f64| [a.cos(), a.sin()]).collect()
        };
        let count = |angles: &[f64]| {
            let mut turning = Turning::new([1.0, 0.0]);
            directions(angles)
                .into_iter()
                .filter(|&unit| turning.next(unit))
                .count()
        };
        let round: Vec<f64> = (1..=70).map(|i| f64::from(i) * 0.1).collect();
        assert_eq!(count(&round), 1);
        let clockwise: Vec<f64> = round.iter().map(|a| -a).collect();
        assert_eq!(count(&clockwise), 1);
        // Out to 2 radians and back the same way is no turn.
        let back: Vec<f64> = (1..=20)
            .chain((0..20).rev())
            .map(|i| f64::from(i) * 0.1)
            .collect();
        assert_eq!(count(&back), 0);
    }

    #[test]
    fn seeds_go_ring_by_ring_from_the_edge() {
        assert_eq!(
            seed_order(3),
            [
                (0, 0),
                (1, 0),
                (2, 0),
                (2, 1),
                (2, 2),
                (1, 2),
                (0, 2),
                (0, 1),
                (1, 1)
            ]
        );
        for side in 1..=7 {
            let order = seed_order(side);
            assert_eq!(order.len(), side * side);
        }
    }

    #[test]
    fn a_head_sits_at_half_the_length() {
        assert_eq!(head_of(&[0.0, 0.5]), None);
        assert_eq!(
            head_of(&[0.0, 1.0, 2.0, 4.0]),
            Some(Head {
                segment: 2,
                fraction: 0.0
            })
        );
        assert_eq!(
            head_of(&[0.0, 1.0, 3.0]),
            Some(Head {
                segment: 1,
                fraction: 0.25
            })
        );
    }

    #[test]
    fn the_occupancy_forgets_what_it_truncates() {
        let mut occupancy = Occupancy::new(4.0);
        occupancy.insert([1.5, 1.5], 0, 0.0);
        let count = occupancy.len();
        occupancy.insert([1.6, 1.5], 1, 0.0);
        assert!(occupancy.any_near([1.7, 1.5], 0.2, |entry| entry.line == 1));
        occupancy.truncate(count);
        assert!(!occupancy.any_near([1.7, 1.5], 0.2, |entry| entry.line == 1));
        assert!(occupancy.any_near([1.7, 1.5], 0.25, |entry| entry.line == 0));
    }
}
