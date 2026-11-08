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
//! [`TOLERANCE`], 0.01 of a step, and at most [`RELATIVE_TOLERANCE`], 4%, of the step's length,
//! and the unit direction of the line changes over it by at most [`MAX_TURN`], about 0.05
//! radians. The relative bound holds the accuracy where the flow grows exponentially along the
//! data, as on a log axis spanning many decades in a step. The turn bound holds the accuracy to the curvature of the
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
//! or not. A step that is not clear is halved like a rejected one, down to [`MIN_STEP`]; a step
//! too inaccurate for the curve is halved further, down to [`MIN_CURVE_STEP`], so an orbit far
//! smaller than a step is still traced to the same accuracy.
//!
//! Speeds are the components times the steps per data unit, either of which can lie far beyond
//! the range of `f64`; they are kept as mantissas and powers of two ([`Wide`]), as the
//! direction of an arrow is, so a direction holds at any range of the data. The components are
//! interpolated in that form too, cell by cell ([`Field::wide_bilinear`]), so a tiny value
//! beside a huge one in another cell, or a field of subnormal numbers, keeps its size.
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
//!   two steps, away along the line, both times the radius of its curve in steps where that is
//!   below one step (the step's length over its change of direction): a closed orbit ends when
//!   it has gone once round, half a step short of closing, or a quarter of its radius on an
//!   orbit smaller than a step, and a spiral when its turns come that close, so a spiral into
//!   a sink keeps both its ways;
//! - it is [`max_length`] steps long either way, a bound that a line meeting the rules above
//!   does not reach in practice;
//! - it has taken [`MAX_TRIES`] steps that way, accepted or rejected, or the mark holds
//!   [`MAX_POINTS`] points, or has taken [`MAX_WORK`] steps over all its lines, dropped ones
//!   included, after which no further line is traced. These bound the work of any field: a line across a grid takes a few thousand steps, and the automatic lines of a
//!   300 by 300 grid at the largest density about half a million points.
//!
//! The length along a line is summed with its rounding carried ([`Length`]), so steps far
//! shorter than the line still count toward [`SELF_GAP`]. A line records a point in the
//! occupancy only every [`RECORD_SPACING`] of a step, or that share of the radius of its curve,
//! so a square holds few points of each line however short the steps; other lines are kept
//! that much further off, so drawn points keep their spacing. A very eccentric orbit smaller
//! than a step may run over part of its first turn again before it ends, which draws nothing
//! new.
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
use crate::exact::two_sum;

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
/// How far apart, in steps, the points a line leaves in the occupancy lie at least: a point
/// closer than this to the last one it left is drawn but not recorded, so a square of the
/// occupancy holds a bounded number of points of each line however short its steps. Other
/// lines are kept this much further away, so every drawn point keeps [`END_SPACING`] and
/// [`SEED_SPACING`]. On a curve of a radius below a step the spacing shrinks with the radius.
pub(crate) const RECORD_SPACING: f64 = 1.0 / 16.0;
/// How far apart along a line, in steps, two of its points must be for their nearness to end it.
pub(crate) const SELF_GAP: f64 = 2.0;
/// The shortest automatic line that is kept, in steps.
pub(crate) const MIN_LENGTH: f64 = 2.0;
/// The shortest line that carries an arrow head, in steps.
pub(crate) const HEAD_LENGTH_MIN: f64 = 1.0;
/// The longest integration step, in steps.
pub(crate) const MAX_STEP: f64 = 0.25;
/// The shortest integration step, in steps, that a blocked step is halved to before the line
/// ends: [`MAX_STEP`] halved ten times.
pub(crate) const MIN_STEP: f64 = MAX_STEP / 1024.0;
/// The shortest integration step, in steps, that a step too inaccurate for the curve is halved
/// to: [`MAX_STEP`] halved forty times, so that an orbit of a hundred-millionth of a step is
/// still traced to the accuracy of a large one.
pub(crate) const MIN_CURVE_STEP: f64 = MAX_STEP / 1_099_511_627_776.0;
/// The largest accepted distance, in steps, between the end of a Heun step and the end of the
/// Euler step it starts with.
pub(crate) const TOLERANCE: f64 = 0.01;
/// The largest accepted distance between the ends of a Heun step and of its Euler step, as a
/// share of the length of the step: it holds the accuracy where the flow grows exponentially
/// along the data, as it does on a log axis that spans many decades in a step, and seldom binds
/// elsewhere, where [`MAX_TURN`] keeps the share below about 0.025.
pub(crate) const RELATIVE_TOLERANCE: f64 = 0.04;
/// The largest accepted change of the unit direction of a line over a step, in steps: about
/// 0.05 radians, so that a step turns by at most that, whatever the radius of the curve.
pub(crate) const MAX_TURN: f64 = 0.05;
/// The share of the largest speed at a grid point at or below which the speed counts as zero.
pub(crate) const ZERO_SPEED: f64 = 1e-6;

/// The most steps, accepted or rejected, that tracing one way of one line may take; it ends
/// there, as at [`max_length`]. A line across a grid at the largest density takes a few
/// thousand.
pub(crate) const MAX_TRIES: usize = 100_000;
/// The most points one mark may hold; tracing stops there, the line being traced ending at its
/// last point. The automatic lines of a 300 by 300 grid at the largest density hold about half
/// a million; at this bound a mark holds about 80 MB.
pub(crate) const MAX_POINTS: usize = 1_000_000;
/// The most steps, accepted or rejected, that tracing all the lines of one mark may take,
/// automatic or through given starts, including the lines that are then dropped as too short;
/// tracing stops there, and the lines kept so far stand. The automatic lines of a 300 by 300
/// grid take about 400,000 at the largest density and 40,000 at a density of 3, fifty and five
/// hundred times fewer.
pub(crate) const MAX_WORK: usize = 20_000_000;

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
    /// Every step from the point is blocked at once, by the edge, a hole or a point where the
    /// flow stops, or turns faster than the shortest step can follow: a line through it would be
    /// the point alone.
    Stuck,
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
                let [p, q] = field.velocity(point, [Wide::of(grid.u[i]), Wide::of(grid.v[i])]);
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
    fn velocity(&self, [x, y]: [f64; 2], [u, v]: [Wide; 2]) -> [Wide; 2] {
        [
            u.times(self.measures[0].rate(x)),
            v.times(self.measures[1].rate(y)),
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

    /// The bilinear value of `values` at `(tx, ty)` of the cell with `corners`, as a number of
    /// any size: the corners are brought to the power of two of the largest of them, exactly,
    /// interpolated, and given that power back, so that neither tiny nor huge corners underflow
    /// or overflow on the way. A corner more than 2^1022 times smaller than the largest counts
    /// as 0, far below anything the cell can show.
    fn wide_bilinear(values: &[f64], corners: [usize; 4], tx: f64, ty: f64) -> Wide {
        let wide = corners.map(|i| Wide::of(values[i]));
        let top = wide
            .iter()
            .map(|value| value.power())
            .max()
            .unwrap_or(i32::MIN);
        let [a, b, c, d] = wide.map(|value| value.at_power(top));
        let mantissa = (a * (1.0 - tx) + b * tx) * (1.0 - ty) + (c * (1.0 - tx) + d * tx) * ty;
        Wide::scaled(mantissa, top)
    }

    /// The velocity at `point`, at `place` in an open cell.
    fn velocity_in(&self, point: [f64; 2], (column, row, tx, ty): Place) -> [Wide; 2] {
        let corners = self.corners(column, row);
        let u = Field::wide_bilinear(self.grid.u, corners, tx, ty);
        let v = Field::wide_bilinear(self.grid.v, corners, tx, ty);
        self.velocity(point, [u, v])
    }

    /// Whether a line moves at `point`, at `place` in an open cell: the fractions and the field
    /// there are finite and the speed is not zero. Anything not finite counts as blocked.
    fn moves(&self, point: [f64; 2], place @ (_, _, tx, ty): Place) -> bool {
        if !point.iter().chain(&[tx, ty]).all(|v| v.is_finite()) {
            return false;
        }
        let [p, q] = self.velocity_in(point, place);
        p.is_finite() && q.is_finite() && !self.is_still([p, q])
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
    /// open, and the speed is zero nowhere the tests find it along it. The segment is cut where
    /// it crosses the lines of the grid. At each crossing the cells on both sides of the line
    /// must be open and the speed there not zero, taken on the line itself
    /// ([`Field::clear_crossing`]), so no rounding of where the crossing lies lets a step past a
    /// cell however narrow. Each piece between crossings is then tested in its own cell
    /// ([`Field::clear_piece`]). A step never jumps a hole, nor a line where the flow stops.
    fn clear(&self, from: [f64; 2], to: [f64; 2]) -> bool {
        // The grid is a rectangle: a segment between two points on it stays on it, and crosses
        // only its inner lines.
        if self.cell(from).is_none() || self.cell(to).is_none() {
            return false;
        }
        let at = |t: f64| {
            [
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
            ]
        };
        // No allocation for the usual step, which crosses no line.
        let mut cuts = Vec::new();
        for (axis, coordinates) in [self.grid.x, self.grid.y].into_iter().enumerate() {
            let (a, b) = (from[axis], to[axis]);
            if a == b {
                continue;
            }
            let (low, high) = (a.min(b), a.max(b));
            let first = coordinates.partition_point(|&c| c <= low);
            let end = coordinates.partition_point(|&c| c < high).max(first);
            for (line, &coordinate) in coordinates.iter().enumerate().take(end).skip(first) {
                let t = (coordinate - a) / (b - a);
                if !self.clear_crossing(at(t), axis, line) {
                    return false;
                }
                cuts.push(t);
            }
        }
        if cuts.is_empty() {
            return self.clear_piece(from, to, [0.0, 1.0]);
        }
        cuts.extend([0.0, 1.0]);
        cuts.sort_by(f64::total_cmp);
        cuts.windows(2)
            .all(|pair| self.clear_piece(from, to, [pair[0], pair[1]]))
    }

    /// Whether a line can cross the line `line` of the grid along `axis`, 0 for x and 1 for y,
    /// at `point`: both cells beside it are open and the speed is not zero in either, at the
    /// point moved exactly onto the line.
    fn clear_crossing(&self, mut point: [f64; 2], axis: usize, line: usize) -> bool {
        let coordinates = [self.grid.x, self.grid.y];
        point[axis] = coordinates[axis][line];
        let other = 1 - axis;
        let Some((across, fraction)) = along(coordinates[other], point[other]) else {
            return false;
        };
        [(line - 1, 1.0), (line, 0.0)]
            .into_iter()
            .all(|(beside, at)| {
                let place = if axis == 0 {
                    (beside, across, at, fraction)
                } else {
                    (across, beside, fraction, at)
                };
                self.is_open(place.0, place.1) && self.moves(point, place)
            })
    }

    /// Whether the piece `[start, end]` of the segment from `from` to `to`, which crosses no line
    /// of the grid, lies in an open cell where the tests find no zero of the speed.
    ///
    /// The piece is parametrized by its own fractions across the cell, each from 0 to 1, so
    /// nothing grows with how narrow the cell is. Along it each component of the bilinear field
    /// is a quadratic in the parameter, and the speed is tested at the ends of the piece and at
    /// the roots and the turning point of each component's quadratic that lie within it: every
    /// point where a component vanishes or turns. That finds a zero of the speed, where both
    /// components vanish, between grid points where the field changes sign, and at a grid point
    /// or on a line of the grid, where it may not. A point where the speed comes close to zero
    /// between those points, without either component vanishing or turning there, is not looked
    /// for. Anything that is not finite blocks the piece.
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
        let fraction = |coordinates: &[f64], i: usize, v: f64| {
            ((v - coordinates[i]) / (coordinates[i + 1] - coordinates[i])).clamp(0.0, 1.0)
        };
        let (first, last) = (at(start), at(end));
        let [tx, ty] = [
            [
                fraction(self.grid.x, column, first[0]),
                fraction(self.grid.x, column, last[0]),
            ],
            [
                fraction(self.grid.y, row, first[1]),
                fraction(self.grid.y, row, last[1]),
            ],
        ];
        if !tx.iter().chain(&ty).all(|v| v.is_finite()) {
            return false;
        }
        let ([x0, x1], [y0, y1]) = (tx, ty);
        let (dx, dy) = (x1 - x0, y1 - y0);
        let corners = self.corners(column, row);
        // The ends, and at most three points of each component.
        let mut candidates = [None; 8];
        candidates[..2].copy_from_slice(&[Some(0.0), Some(1.0)]);
        for (component, values) in [self.grid.u, self.grid.v].into_iter().enumerate() {
            let [a, b, c, d] = corners.map(|i| values[i]);
            let largest = a.abs().max(b.abs()).max(c.abs()).max(d.abs());
            if largest == 0.0 {
                continue;
            }
            let [a, b, c, d] = [a, b, c, d].map(|value| value / largest);
            // f = a + (b - a) tx + (c - a) ty + (a - b - c + d) tx ty, with tx = x0 + dx s and
            // ty = y0 + dy s.
            let (linear_x, linear_y, cross) = (b - a, c - a, a - b - c + d);
            let quadratic = [
                a + linear_x * x0 + linear_y * y0 + cross * x0 * y0,
                linear_x * dx + linear_y * dy + cross * (x0 * dy + dx * y0),
                cross * dx * dy,
            ];
            if !quadratic.iter().all(|v| v.is_finite()) {
                return false;
            }
            let found = critical_points(quadratic).map(|s| s.filter(|s| (0.0..=1.0).contains(s)));
            candidates[2 + 3 * component..5 + 3 * component].copy_from_slice(&found);
        }
        candidates.into_iter().flatten().all(|s| {
            let point = at(start + (end - start) * s);
            let place = (
                column,
                row,
                (x0 + dx * s).clamp(0.0, 1.0),
                (y0 + dy * s).clamp(0.0, 1.0),
            );
            self.moves(point, place)
        })
    }
}

/// The cell of `v` among the strictly increasing `values`, at least two, and the fraction of the
/// way across it; `None` outside them. A value on a line between cells belongs to the cell after
/// it, except on the last line.
fn along(values: &[f64], v: f64) -> Option<(usize, f64)> {
    let last = values.len() - 1;
    if !(values[0]..=values[last]).contains(&v) {
        return None;
    }
    let i = (values.partition_point(|&c| c <= v).max(1) - 1).min(last - 1);
    Some((i, (v - values[i]) / (values[i + 1] - values[i])))
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
    /// The length along the line, as the sum of `length` and `carry`, so that steps far
    /// shorter than the length still count.
    length: Length,
}

/// A length along a line kept as an unevaluated sum, compensated as it grows: a step a
/// billionth of the length already traced is not lost to rounding, so the distance along the
/// line between two of its points is right however short the steps become.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Length {
    /// The rounded sum.
    sum: f64,
    /// What the rounding of `sum` left out.
    carry: f64,
}

impl Length {
    /// This length with `step` added.
    fn plus(self, step: f64) -> Length {
        let (sum, error) = two_sum(self.sum, step);
        Length {
            sum,
            carry: self.carry + error,
        }
    }

    /// The distance along the line from `other` to this length.
    fn from(self, other: Length) -> f64 {
        (self.sum - other.sum) + (self.carry - other.carry)
    }

    /// The length as one number.
    fn value(self) -> f64 {
        self.sum + self.carry
    }
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
    fn insert(&mut self, at: [f64; 2], line: u32, length: Length) {
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
    /// The most steps one way of a line may take: [`MAX_TRIES`].
    max_tries: usize,
    /// How many more points the mark may hold: [`MAX_POINTS`] less those traced.
    points_left: usize,
    /// How many more steps, accepted or rejected, the mark may take: [`MAX_WORK`] less those
    /// taken, never given back.
    work_left: usize,
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
    ) -> Vec<Traced> {
        let field = &self.field;
        let mut traced = Vec::new();
        let (mut point, mut probe) = (seed, start);
        let mut at = field.position(seed);
        let mut recorded = at;
        let mut length = Length::default();
        let mut step = MAX_STEP;
        let mut tries = 0;
        let along =
            |p: [f64; 2], slope: [f64; 2], by: f64| [p[0] + by * slope[0], p[1] + by * slope[1]];
        while tries < self.max_tries && self.points_left > 0 && self.work_left > 0 {
            tries += 1;
            self.work_left -= 1;
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
            if moved < step / 2.0 {
                step /= 2.0;
                if step < MIN_STEP {
                    break;
                }
                continue;
            }
            // A step too inaccurate for the curve shrinks further, with the radius of the curve.
            if error > TOLERANCE || error > RELATIVE_TOLERANCE * moved || turn > MAX_TURN {
                step /= 2.0;
                if step < MIN_CURVE_STEP {
                    break;
                }
                continue;
            }
            let next_length = length.plus(sign * moved);
            if next_length.value().abs() > self.max_length {
                break;
            }
            // Its own points keep a line apart by a share of the radius of its curve, up to a
            // step: an orbit or a spiral smaller than a step closes or ends at its own scale.
            let scale = if turn > 0.0 {
                (moved / turn).min(1.0)
            } else {
                1.0
            };
            let (near, gap) = (END_SPACING * scale, SELF_GAP * scale);
            let crowded = self
                .occupancy
                .any_near(next_at, END_SPACING + RECORD_SPACING, |entry| {
                    if entry.line == line {
                        entry.length.from(next_length).abs() > gap
                            && distance(entry.at, next_at) < near
                    } else {
                        company == Company::Others
                    }
                });
            if crowded {
                break;
            }
            if distance(recorded, next_at) >= RECORD_SPACING * scale {
                self.occupancy.insert(next_at, line, next_length);
                recorded = next_at;
            }
            traced.push((next, next_length.value(), next_probe.value));
            self.points_left -= 1;
            (point, probe, at, length) = (next, next_probe, next_at, next_length);
            if error < TOLERANCE / 4.0
                && error < RELATIVE_TOLERANCE / 4.0 * moved
                && turn < MAX_TURN / 2.0
                && step < MAX_STEP
            {
                step *= 2.0;
            }
        }
        traced
    }

    /// The line `line` through `seed`, where the field is `start`, traced both ways.
    fn line(&mut self, seed: [f64; 2], start: Probe, line: u32, company: Company) -> Streamline {
        self.occupancy
            .insert(self.field.position(seed), line, Length::default());
        self.points_left = self.points_left.saturating_sub(1);
        let upstream = self.half(seed, start, line, -1.0, company);
        let downstream = self.half(seed, start, line, 1.0, company);
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
            max_tries: MAX_TRIES,
            points_left: MAX_POINTS,
            work_left: MAX_WORK,
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
    automatic_with(&mut Tracer::new(grid))
}

/// The automatic streamlines of the field of `tracer`, within its bounds.
fn automatic_with(tracer: &mut Tracer<'_>) -> Vec<Streamline> {
    let side = tracer.occupancy.side;
    let pitch = tracer.field.steps / side as f64;
    let mut lines = Vec::new();
    for (column, row) in seed_order(side) {
        if tracer.points_left == 0 || tracer.work_left == 0 {
            break;
        }
        let at = [(column as f64 + 0.5) * pitch, (row as f64 + 0.5) * pitch];
        let seed = tracer.field.point(at);
        let Some(start) = tracer.field.flowing(seed) else {
            continue;
        };
        let position = tracer.field.position(seed);
        if tracer
            .occupancy
            .any_near(position, SEED_SPACING + RECORD_SPACING, |_| true)
        {
            continue;
        }
        let before = tracer.occupancy.len();
        let line = tracer.line(seed, start, lines.len() as u32, Company::Others);
        if line.length() < MIN_LENGTH {
            tracer.occupancy.truncate(before);
            tracer.points_left += line.points().len();
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
    starts
        .iter()
        .zip(probes)
        .enumerate()
        .map(|(i, (&start, probe))| {
            tracer.occupancy.truncate(0);
            let line = tracer.line(start, probe, 0, Company::Alone);
            if line.points().len() < 2 && tracer.points_left > 0 && tracer.work_left > 0 {
                return Err((i, Blocked::Stuck));
            }
            Ok(line)
        })
        .collect()
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
    fn a_length_keeps_steps_far_shorter_than_itself() {
        let mut length = Length::default().plus(2000.0);
        let start = length;
        for _ in 0..1000 {
            length = length.plus(1e-13);
        }
        assert_eq!(start.sum + 1e-13, start.sum);
        assert!((length.from(start) / 1e-10 - 1.0).abs() < 1e-6);
    }

    /// A grid of a uniform flow along x, `columns` wide and two rows high.
    fn uniform(columns: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let x: Vec<f64> = (0..columns).map(|i| i as f64).collect();
        (x, vec![1.0; 2 * columns], vec![0.0; 2 * columns])
    }

    #[test]
    fn the_work_of_a_line_and_of_a_mark_is_bounded() {
        let (x, u, v) = uniform(11);
        let grid = Grid {
            x: &x,
            y: &[0.0, 1.0],
            u: &u,
            v: &v,
            values: None,
            log: [false, false],
            density: 10.0,
        };
        // Each way of a line takes at most `max_tries` steps, accepted or not.
        let mut tracer = Tracer::new(grid);
        tracer.max_tries = 40;
        let start = tracer.field.probe([5.0, 0.5]).unwrap();
        let line = tracer.line([5.0, 0.5], start, 0, Company::Alone);
        assert_eq!(line.points().len(), 2 * 40 + 1);
        // A mark holds at most `points_left` points.
        let mut tracer = Tracer::new(grid);
        tracer.points_left = 500;
        let lines = automatic_with(&mut tracer);
        let points: usize = lines.iter().map(|line| line.points().len()).sum();
        assert!(points <= 500 && points > 400, "{points}");
        // The bounds themselves.
        assert_eq!(Tracer::new(grid).max_tries, MAX_TRIES);
        assert_eq!(Tracer::new(grid).points_left, MAX_POINTS);
        assert_eq!(Tracer::new(grid).work_left, MAX_WORK);
    }

    #[test]
    fn the_work_of_a_mark_is_bounded_even_when_every_line_is_dropped() {
        // Every third column NaN: the open cells, a third of a step wide, each holding a seed,
        // short to keep, each of which is traced and dropped.
        let columns = 91;
        let x: Vec<f64> = (0..columns).map(|i| i as f64).collect();
        let u: Vec<f64> = (0..2 * columns)
            .map(|i| if i % columns % 3 == 0 { f64::NAN } else { 1.0 })
            .collect();
        let v = vec![0.0; 2 * columns];
        let grid = Grid {
            x: &x,
            y: &[0.0, 1.0],
            u: &u,
            v: &v,
            values: None,
            log: [false, false],
            density: 1.0,
        };
        let mut free = Tracer::new(grid);
        assert!(automatic_with(&mut free).is_empty());
        let taken = MAX_WORK - free.work_left;
        assert!(taken > 200, "{taken}");
        // With a bound of 200 steps, tracing stops after exactly 200, all dropped.
        let mut bounded = Tracer::new(grid);
        bounded.work_left = 200;
        assert!(automatic_with(&mut bounded).is_empty());
        assert_eq!(bounded.work_left, 0);
        // Starts share the bound.
        let mut bounded = Tracer::new(grid);
        bounded.work_left = 10;
        let start = bounded.field.probe([1.5, 0.5]).unwrap();
        let line = bounded.line([1.5, 0.5], start, 0, Company::Alone);
        assert!(line.points().len() <= 11 && bounded.work_left == 0);
    }

    #[test]
    fn the_work_of_large_grids_stays_far_below_the_bound() {
        let count = 300;
        let x: Vec<f64> = (0..count).map(|i| -3.0 + 6.0 * i as f64 / 299.0).collect();
        let y: Vec<f64> = (0..count).map(|i| -2.0 + 4.0 * i as f64 / 299.0).collect();
        let (mut u, mut v) = (Vec::new(), Vec::new());
        for &y in &y {
            for &x in &x {
                u.push(-y + 0.4 * x * (1.0 - x * x / 9.0));
                v.push(x - 0.3 * y);
            }
        }
        for density in [3.0, 10.0] {
            let grid = Grid {
                x: &x,
                y: &y,
                u: &u,
                v: &v,
                values: None,
                log: [false, false],
                density,
            };
            let mut tracer = Tracer::new(grid);
            automatic_with(&mut tracer);
            let taken = MAX_WORK - tracer.work_left;
            assert!(taken < MAX_WORK / 10, "density {density}: {taken}");
        }
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
        occupancy.insert([1.5, 1.5], 0, Length::default());
        let count = occupancy.len();
        occupancy.insert([1.6, 1.5], 1, Length::default());
        assert!(occupancy.any_near([1.7, 1.5], 0.2, |entry| entry.line == 1));
        occupancy.truncate(count);
        assert!(!occupancy.any_near([1.7, 1.5], 0.2, |entry| entry.line == 1));
        assert!(occupancy.any_near([1.7, 1.5], 0.25, |entry| entry.line == 0));
    }
}
