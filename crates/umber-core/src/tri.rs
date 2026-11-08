//! Triangle meshes: points joined into triangles, by Delaunay triangulation ([`delaunay`]) or
//! as given ([`Triangles`]), and validated into a [`TriMesh`] that the triangle marks draw.
//!
//! **Delaunay.** [`delaunay`] triangulates the raw coordinates with the `delaunator` crate,
//! whose orientation tests are Shewchuk's exact predicates. Exact duplicates are refused
//! first, naming the first point that repeats an earlier one, and then points all on one line,
//! decided exactly on the raw coordinates. The library compares near-duplicate points with a
//! fixed absolute tolerance, so the coordinates are multiplied by one power of two that brings
//! the largest magnitude to between 0.5 and 1: that changes no comparison between coordinates,
//! and every product only by an exact power of two, so the triangulation is that of the raw
//! coordinates, at any scale, as long as the scaling rounds no coordinate, which only a
//! coordinate that becomes subnormal and loses bits can make; such coordinates span too wide a
//! range of scales and are refused. The result is then
//! checked: every point used, every triangle turning the same way with an area, decided
//! exactly, and as many triangles as a triangulation of the points has (`2n - 2 - h` for `n`
//! points, `h` of them on the hull). Points so close that the library leaves one out (within
//! about 1e-15 of the largest coordinate along both axes) are refused naming both; any other
//! failure, which only coordinates whose squares or cubes leave the range of `f64` can cause,
//! is refused as a range too wide to triangulate, with a hint to rescale.
//!
//! **Order.** Rows of [`delaunay`] are counter-clockwise in data coordinates with y up, each
//! starting at its smallest index, and sorted. A [`TriMesh`] stores each triangle with its
//! corners in increasing index order and the triangles sorted, so that two equal sets of
//! triangles, in any row order and with any winding, make the same mesh and the same drawing.
//! Winding is decided where a triangle is drawn, on the page.
//!
//! **Validation.** Given triangles are checked in one pass over the rows (indices in range,
//! three different corners, an area) and then, on the sorted triangles and their sorted edges,
//! for a triangle given twice and an edge that borders more than two triangles: `O(n log n)`
//! for `n` triangles.
//!
//! **Exact turns.** Whether three points lie on one line, and which way they turn, is decided
//! exactly on the raw coordinates, so the answer never depends on the order of the corners.
//! Shewchuk's `orient2d` is exact unless an intermediate value overflows or underflows. Where
//! every coordinate of the three points is 0 or an integer multiple of 2^-400 below 2^400 in
//! magnitude ([`Exact`]), its exact arithmetic stays on a lattice: the differences of
//! coordinates and their rounding errors are multiples of 2^-400 below 2^401, and their
//! products, the sums of those and the errors of both are multiples of 2^-800 below 2^810, so
//! none of these underflows (a nonzero one is at least 2^-800) or overflows. Its error bounds
//! are not on the lattice: each is a constant from about 2^-106 to 2^-51 times a sum of
//! magnitudes of values on it, so it is either 0 or at least about 2^-910, far above the
//! smallest normal `f64` (2^-1022), and below 2^812; none underflows or overflows, so each bound
//! holds as Shewchuk proves it. So the sign is exact. Any other triple is decided
//! in exact binary fractions ([`Dyadic`]).

use std::cmp::Ordering;
use std::sync::Arc;

use robust::{Coord, orient2d};

use crate::error::number;
use crate::exact::Dyadic;
use crate::{ContinuousScale, Error, MAX_MAGNITUDE, MIN_LOG_VALUE, Result, Transform};

/// The context of every error of [`delaunay`].
const DELAUNAY_CONTEXT: &str = "delaunay()";

/// How far apart, along both axes and after the coordinates are scaled to a largest magnitude
/// between 0.5 and 1, two points must lie for the triangulation to keep both
/// (`delaunator::EPSILON`).
const NEAR: f64 = delaunator::EPSILON;

/// Which triangles join the points of a triangle mesh.
///
/// # Examples
///
/// ```
/// use umber_core::Triangles;
///
/// let given = [[0, 1, 2]];
/// assert_ne!(Triangles::Given(&given), Triangles::Auto);
/// assert_eq!(Triangles::default(), Triangles::Auto);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Triangles<'a> {
    /// The Delaunay triangulation of the points, as [`delaunay`] returns it.
    #[default]
    Auto,
    /// These triangles, each three indices into the points, in any order and with either
    /// winding. Indices are `i64`, as [`delaunay`] returns them, so that a negative index is
    /// reported rather than wrapped.
    Given(&'a [[i64; 3]]),
}

/// The Delaunay triangulation of the points `(x[i], y[i])`: one row per triangle, three indices
/// into the points, counter-clockwise in data coordinates with y up.
///
/// It works on the raw coordinates, as `scipy.spatial.Delaunay` does: for x and y in very
/// different units, triangulate scaled coordinates and pass the result to
/// [`Triangles::Given`]. Each row starts at its smallest index, and the rows are sorted. Which
/// diagonal splits four points on one circle is the library's choice; it is the same on every
/// platform.
///
/// # Errors
///
/// `Error::Data` under the context `"delaunay()"`, in this order: `x` and `y` of different
/// lengths; fewer than three points; a coordinate that is not finite (naming it); a point that
/// repeats an earlier one (naming both); all points on one line, decided exactly; coordinates
/// over so wide a range of scales that they cannot be triangulated in double precision (naming
/// the axis of the smaller span, with a hint to rescale); two points too close to tell apart,
/// within about 1e-15 of the largest coordinate along both axes (naming both).
///
/// # Examples
///
/// ```
/// use umber_core::delaunay;
///
/// let triangles = delaunay(&[0.0, 1.0, 1.0, 0.0], &[0.0, 0.0, 1.0, 1.0])?;
/// assert_eq!(triangles.len(), 2);
/// assert!(triangles.iter().all(|row| row[0] < row[1] && row[0] < row[2]));
///
/// let err = delaunay(&[0.0, 1.0, 2.0], &[0.0, 1.0, 2.0]).unwrap_err();
/// assert!(err.to_string().contains("on one line"));
/// # Ok::<(), umber_core::Error>(())
/// ```
pub fn delaunay(x: &[f64], y: &[f64]) -> Result<Vec<[i64; 3]>> {
    check_lengths(DELAUNAY_CONTEXT, x, y)?;
    check_count(DELAUNAY_CONTEXT, x.len())?;
    for (name, values) in [("x", x), ("y", y)] {
        if let Some((i, &v)) = values.iter().enumerate().find(|(_, v)| !v.is_finite()) {
            return Err(Error::data(
                DELAUNAY_CONTEXT,
                format!("{name}[{i}] is {}; coordinates must be finite.", number(v)),
            )
            .with_parameter(name)
            .with_hint(
                "leave the point out before triangulating; the indices of the triangles count \
                 every point, so there are no missing points.",
            ));
        }
    }
    let rows = triangulate(DELAUNAY_CONTEXT, x, y)?;
    Ok(rows
        .into_iter()
        // An index is below the number of points, at most `isize::MAX`, so it fits.
        .map(|row| row.map(|i| i as i64))
        .collect())
}

/// Checks that `x` and `y` have the same length (`Error::Data` about `y`).
fn check_lengths(context: &'static str, x: &[f64], y: &[f64]) -> Result<()> {
    if x.len() == y.len() {
        return Ok(());
    }
    Err(Error::data(
        context,
        format!(
            "x has {} values but y has {}; x and y must have the same length.",
            x.len(),
            y.len()
        ),
    )
    .with_parameter("y")
    .with_hint("pass one x and one y coordinate per point."))
}

/// Checks that there are at least three points (`Error::Data` about `x`).
fn check_count(context: &'static str, count: usize) -> Result<()> {
    if count >= 3 {
        return Ok(());
    }
    let points = match count {
        1 => "1 point".to_owned(),
        n => format!("{n} points"),
    };
    Err(Error::data(
        context,
        format!("x and y hold {points}; a triangle needs at least three."),
    )
    .with_parameter("x")
    .with_hint("pass at least three points that do not lie on one line."))
}

/// The Delaunay triangles of the finite points `(x[i], y[i])`, at least three, as [`delaunay`]
/// returns them, with the errors of the call `context`.
pub(crate) fn triangulate(context: &'static str, x: &[f64], y: &[f64]) -> Result<Vec<[usize; 3]>> {
    check_duplicates(context, x, y)?;
    let points = Exact::new(x, y);
    check_not_one_line(context, &points)?;
    let scaled = scaled(context, x, y)?;
    let input: Vec<delaunator::Point> = scaled
        .iter()
        .map(|&[x, y]| delaunator::Point { x, y })
        .collect();
    let found = delaunator::triangulate(&input);
    let mut rows: Vec<[usize; 3]> = found
        .triangles
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    check_all_used(context, (x, y), &scaled, &rows)?;
    // A triangulation of n points, h of them on its hull, has 2n - 2 - h triangles, all turning
    // the same way; anything else is a numerical failure of the library.
    let expected = (2 * x.len()).checked_sub(2 + found.hull.len());
    let turns = rows.first().map(|&[a, b, c]| points.turn(a, b, c));
    let consistent = turns.is_some_and(|first| {
        first != Ordering::Equal && rows.iter().all(|&[a, b, c]| points.turn(a, b, c) == first)
    });
    if !consistent || expected != Some(rows.len()) {
        return Err(too_wide(context, x, y));
    }
    for row in &mut rows {
        if points.turn(row[0], row[1], row[2]) == Ordering::Less {
            row.swap(1, 2);
        }
        let first = (0..3).min_by_key(|&k| row[k]).unwrap_or(0);
        row.rotate_left(first);
    }
    rows.sort_unstable();
    Ok(rows)
}

/// Refuses points that all lie on one line, decided exactly (`Error::Data` about `x`). The
/// points are different, so a second point other than the first exists.
fn check_not_one_line(context: &'static str, points: &Exact<'_>) -> Result<()> {
    let n = points.x.len();
    let second = (1..n)
        .find(|&i| points.raw(i) != points.raw(0))
        .unwrap_or(1);
    if (1..n).any(|i| points.turn(0, second, i) != Ordering::Equal) {
        return Ok(());
    }
    Err(Error::data(
        context,
        format!("all {n} points lie on one line; a triangle needs three points that do not."),
    )
    .with_parameter("x")
    .with_hint("add a point off the line, or draw the points with line() or scatter()."))
}

/// The error for points the library cannot triangulate in the range of `f64`, though they do
/// not lie on one line: their coordinates span too wide a range of scales (`Error::Data` about
/// the axis of the smaller span).
fn too_wide(context: &'static str, x: &[f64], y: &[f64]) -> Error {
    let span = |values: &[f64]| {
        let low = values.iter().copied().fold(f64::INFINITY, f64::min);
        let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (low, high)
    };
    let ((x_low, x_high), (y_low, y_high)) = (span(x), span(y));
    let parameter = if y_high - y_low < x_high - x_low {
        "y"
    } else {
        "x"
    };
    Error::data(
        context,
        format!(
            "x spans {} to {} and y spans {} to {}; coordinates spread over so wide a range of \
             scales cannot be triangulated in double precision.",
            number(x_low),
            number(x_high),
            number(y_low),
            number(y_high)
        ),
    )
    .with_parameter(parameter)
    .with_hint(
        "rescale x and y to similar spans, such as delaunay(x / np.ptp(x), y / np.ptp(y)), and \
         pass the result as triangles=.",
    )
}

/// Refuses a point that repeats an earlier one (`Error::Data` about `x`): of all repeats, the
/// one whose index is lowest, named with the lowest index of the same point. `-0.0` and `0.0`
/// are the same coordinate.
fn check_duplicates(context: &'static str, x: &[f64], y: &[f64]) -> Result<()> {
    // Adding 0.0 turns -0.0 into 0.0, so that equal coordinates have equal bits.
    let key = |i: usize| ((x[i] + 0.0).to_bits(), (y[i] + 0.0).to_bits(), i);
    let mut order: Vec<(u64, u64, usize)> = (0..x.len()).map(key).collect();
    order.sort_unstable();
    // The repeat with the lowest index: within each run of equal points, the second index is
    // the first repeat, and the first index the point it repeats.
    let repeat = order
        .windows(2)
        .filter(|pair| (pair[0].0, pair[0].1) == (pair[1].0, pair[1].1))
        .map(|pair| (pair[1].2, pair[0].2))
        .fold(None::<(usize, usize)>, |best, (later, first)| match best {
            Some((b, _)) if b <= later => best,
            _ => Some((later, first)),
        });
    // Each run is sorted by index, so the pair found for a run's second member is the run's
    // first two; a later member of the same run has a higher index and never wins.
    let Some((later, first)) = repeat else {
        return Ok(());
    };
    Err(Error::data(
        context,
        format!(
            "points {first} and {later} are both at ({}, {}); every point must be different.",
            number(x[first]),
            number(y[first])
        ),
    )
    .with_parameter("x")
    .with_hint(
        "leave the repeated point out, for example with np.unique(np.column_stack([x, y]), \
         axis=0).",
    ))
}

/// Refuses a triangulation that leaves a point out: the triangulation joins every point of a
/// set not all on one line, unless it found two points too close to tell apart and kept one
/// (`Error::Data` about `x`, naming both), or failed in the range of `f64` ([`too_wide`]).
/// `scaled` holds the coordinates as the library got them.
fn check_all_used(
    context: &'static str,
    (x, y): (&[f64], &[f64]),
    scaled: &[[f64; 2]],
    rows: &[[usize; 3]],
) -> Result<()> {
    let mut used = vec![false; scaled.len()];
    for &i in rows.iter().flatten() {
        if let Some(slot) = used.get_mut(i) {
            *slot = true;
        }
    }
    let Some(left) = used.iter().position(|&used| !used) else {
        return Ok(());
    };
    let [lx, ly] = scaled[left];
    let close = (0..scaled.len()).find(|&j| {
        let [sx, sy] = scaled[j];
        j != left && (sx - lx).abs() <= NEAR && (sy - ly).abs() <= NEAR
    });
    let Some(other) = close else {
        return Err(too_wide(context, x, y));
    };
    let (first, later) = (other.min(left), other.max(left));
    Err(Error::data(
        context,
        format!(
            "points {first} and {later}, at ({}, {}) and ({}, {}), are too close to tell apart; \
             points must differ by more than about 1e-15 of the largest coordinate along x or y.",
            number(x[first]),
            number(y[first]),
            number(x[later]),
            number(y[later])
        ),
    )
    .with_parameter("x")
    .with_hint("merge points that are this close, or rescale the coordinates."))
}

/// The coordinates multiplied by one power of two that brings the largest magnitude to between
/// 0.5 and 1 (see the module documentation), `[x, y]` per point. Refuses coordinates that the
/// scaling would round, those whose scaled value, scaled back, is not the coordinate again
/// ([`too_wide`]); a subnormal result that keeps every bit is kept. No coordinate can overflow.
fn scaled(context: &'static str, x: &[f64], y: &[f64]) -> Result<Vec<[f64; 2]>> {
    let largest = x.iter().chain(y).fold(0.0_f64, |m, v| m.max(v.abs()));
    let exponent = binary_exponent(largest);
    let mut points = Vec::with_capacity(x.len());
    for (&x0, &y0) in x.iter().zip(y) {
        let point = [x0, y0].map(|v| times_power_of_two(v, -exponent));
        let lost = [x0, y0]
            .iter()
            .zip(&point)
            .any(|(&raw, &v)| times_power_of_two(v, exponent) != raw);
        if lost {
            return Err(too_wide(context, x, y));
        }
        points.push(point);
    }
    Ok(points)
}

/// Points with what decides exactly which way three of them turn (see the module
/// documentation).
pub(crate) struct Exact<'a> {
    x: &'a [f64],
    y: &'a [f64],
    /// Whether both coordinates of each point are 0 or integer multiples of 2^-400 below 2^400
    /// in magnitude, so that `orient2d` decides any three such points exactly.
    fast: Vec<bool>,
}

impl<'a> Exact<'a> {
    /// The finite points `(x[i], y[i])`.
    pub(crate) fn new(x: &'a [f64], y: &'a [f64]) -> Exact<'a> {
        let fast = x
            .iter()
            .zip(y)
            .map(|(&x, &y)| on_grid(x) && on_grid(y))
            .collect();
        Exact { x, y, fast }
    }

    /// The point `i` as given.
    fn raw(&self, i: usize) -> [f64; 2] {
        [self.x[i], self.y[i]]
    }

    /// Which way the points `a`, `b` and `c` turn, decided exactly: `Greater` counter-clockwise
    /// with y up, `Less` clockwise, `Equal` on one line. The answer for a reordering of the
    /// three points follows from this one by the parity of the reordering.
    pub(crate) fn turn(&self, a: usize, b: usize, c: usize) -> Ordering {
        if self.fast[a] && self.fast[b] && self.fast[c] {
            let coord = |i: usize| {
                let [x, y] = self.raw(i);
                Coord { x, y }
            };
            let turn = orient2d(coord(a), coord(b), coord(c));
            return if turn > 0.0 {
                Ordering::Greater
            } else if turn < 0.0 {
                Ordering::Less
            } else {
                Ordering::Equal
            };
        }
        let d = |i: usize| self.raw(i).map(Dyadic::from_f64);
        let ([ax, ay], [bx, by], [cx, cy]) = (d(a), d(b), d(c));
        let cross = bx
            .sub(&ax)
            .mul(&cy.sub(&ay))
            .sub(&by.sub(&ay).mul(&cx.sub(&ax)));
        cross.sign()
    }
}

/// Whether `v` is 0, or an integer multiple of 2^-400 below 2^400 in magnitude.
fn on_grid(v: f64) -> bool {
    if v == 0.0 {
        return true;
    }
    let bits = v.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    // `v = m * 2^(e - 1075)` with the integer `m`; its lowest set bit says how fine it is.
    let (m, e) = if biased == 0 {
        (fraction, 1)
    } else {
        (fraction | (1_u64 << 52), biased)
    };
    let lowest = e - 1075 + m.trailing_zeros() as i32;
    let highest = e - 1075 + 52;
    lowest >= -400 && highest < 400
}

/// The exponent `e` with `v = f * 2^e` and `f` from 0.5 to below 1, for a finite `v > 0`; 0 for
/// 0.
fn binary_exponent(v: f64) -> i32 {
    if v == 0.0 || !v.is_finite() {
        return 0;
    }
    if v < f64::MIN_POSITIVE {
        // Subnormal: scale it into the normal range first.
        return binary_exponent(v * power_of_two(64)) - 64;
    }
    let biased = ((v.to_bits() >> 52) & 0x7ff) as i32;
    biased - 1022
}

/// `v * 2^e`, in steps whose factors are normal powers of two, so that the result is exact
/// unless it is subnormal.
fn times_power_of_two(mut v: f64, mut e: i32) -> f64 {
    const STEP: i32 = 1000;
    while e > STEP {
        v *= power_of_two(STEP);
        e -= STEP;
    }
    while e < -STEP {
        v *= power_of_two(-STEP);
        e += STEP;
    }
    v * power_of_two(e)
}

/// `2^e` for `e` from -1000 to 1000, a normal `f64`, built from its bits.
fn power_of_two(e: i32) -> f64 {
    let biased = u64::try_from(1023 + e.clamp(-1000, 1000)).unwrap_or(1023);
    f64::from_bits(biased << 52)
}

/// Points joined into triangles, validated: one triangle per row of three point indices, each
/// row in increasing order and the rows sorted (see the module documentation), so that the
/// same triangles given in any order and winding make the same mesh. Clones share their data.
///
/// The points are finite, at most 1e300 in magnitude, positive and at least 1e-300 on a log
/// axis, and span a range each axis can show. Every triangle has three different corners that
/// do not lie on one line, no triangle is given twice, and no edge borders more than two
/// triangles. A point that no triangle uses still counts in the automatic limits.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Mark, ScaleKind, Theme, Triangles, TriplotOptions};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let (x, y) = ([0.0, 1.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
/// axes.triplot(&x, &y, Triangles::Given(&[[3, 2, 0], [0, 2, 1]]), TriplotOptions::default())?;
/// let Some(Mark::Triplot(mark)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(mark.mesh().triangles(), [[0, 1, 2], [0, 2, 3]]);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TriMesh {
    x: Arc<[f64]>,
    y: Arc<[f64]>,
    triangles: Arc<[[usize; 3]]>,
}

impl TriMesh {
    /// Validates the points `(x[i], y[i])` on axes of the transforms `transforms` and joins
    /// them with `triangles`, for the call `context`. Returns the mesh and, for each of its
    /// triangles, the row it came from: of the given triangles, or of [`delaunay`]'s result.
    ///
    /// The checks run in this order, and the first failure is the error (all `Error::Data`):
    ///
    /// 1. `x` and `y` have the same length;
    /// 2. every coordinate is finite, at most 1e300 in magnitude, and positive and at least
    ///    1e-300 on a log axis (naming it);
    /// 3. there are at least three points;
    /// 4. the extent of the points along each axis is a range the axis can show;
    /// 5. given triangles: at least one; each index from 0 to the last point (naming it); three
    ///    different corners and corners that do not lie on one line (naming the row); then no
    ///    triangle given twice (naming both rows) and no edge bordering more than two triangles
    ///    (naming the edge and three of them), each the first in sorted order;
    /// 6. [`Triangles::Auto`]: as [`delaunay`] checks the points.
    pub(crate) fn new(
        context: &'static str,
        transforms: [Transform; 2],
        (x, y): (&[f64], &[f64]),
        triangles: Triangles<'_>,
    ) -> Result<(TriMesh, Vec<usize>)> {
        check_lengths(context, x, y)?;
        for ((name, values), transform) in [("x", x), ("y", y)].into_iter().zip(transforms) {
            check_coordinates(context, name, values, transform)?;
        }
        check_count(context, x.len())?;
        for ((name, values), transform) in [("x", x), ("y", y)].into_iter().zip(transforms) {
            check_extent(context, name, values, transform)?;
        }
        let rows = match triangles {
            Triangles::Auto => triangulate(context, x, y)?,
            Triangles::Given(given) => check_given(context, (x, y), given)?,
        };
        let mut sorted: Vec<([usize; 3], usize)> = rows
            .into_iter()
            .enumerate()
            .map(|(row, mut corners)| {
                corners.sort_unstable();
                (corners, row)
            })
            .collect();
        sorted.sort_unstable();
        if let Triangles::Given(_) = triangles {
            check_repeats(context, &sorted)?;
            check_edges(context, &sorted)?;
        }
        let (triangles, order): (Vec<[usize; 3]>, Vec<usize>) = sorted.into_iter().unzip();
        Ok((
            TriMesh {
                x: x.into(),
                y: y.into(),
                triangles: triangles.into(),
            },
            order,
        ))
    }

    /// The x coordinate of each point.
    #[must_use]
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// The y coordinate of each point.
    #[must_use]
    pub fn y(&self) -> &[f64] {
        &self.y
    }

    /// The triangles: three point indices each, in increasing order, the rows sorted.
    #[must_use]
    pub fn triangles(&self) -> &[[usize; 3]] {
        &self.triangles
    }

    /// Every edge of the triangles once, as its two point indices in increasing order, the
    /// edges sorted.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, Mark, ScaleKind, Theme, Triangles, TriplotOptions};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let (x, y) = ([0.0, 1.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
    /// axes.triplot(&x, &y, Triangles::Given(&[[0, 1, 2], [0, 2, 3]]), TriplotOptions::default())?;
    /// let Some(Mark::Triplot(mark)) = axes.marks().first() else { unreachable!() };
    /// assert_eq!(mark.mesh().edges(), [[0, 1], [0, 2], [0, 3], [1, 2], [2, 3]]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn edges(&self) -> Vec<[usize; 2]> {
        let mut edges: Vec<[usize; 2]> = self
            .triangles
            .iter()
            .flat_map(|&[a, b, c]| [[a, b], [a, c], [b, c]])
            .collect();
        edges.sort_unstable();
        edges.dedup();
        edges
    }
}

/// Checks the coordinates `values`, the argument `name`, on an axis of `transform`: finite, at
/// most 1e300 in magnitude, and positive and at least 1e-300 on a log axis (`Error::Data`).
fn check_coordinates(
    context: &'static str,
    name: &'static str,
    values: &[f64],
    transform: Transform,
) -> Result<()> {
    let log = matches!(transform, Transform::Log(_));
    for (i, &v) in values.iter().enumerate() {
        let (required, hint) = if !v.is_finite() {
            (
                "coordinates of a triangle mesh must be finite".to_owned(),
                "leave the point out and renumber the triangles; every index counts every point.",
            )
        } else if v.abs() > MAX_MAGNITUDE {
            (
                format!("coordinates must not exceed {MAX_MAGNITUDE:e} in magnitude"),
                "rescale the coordinates to the supported numeric range.",
            )
        } else if log && v <= 0.0 {
            (
                "coordinates on a log axis must be positive".to_owned(),
                "use a linear axis, or leave out the points that are not positive.",
            )
        } else if log && v < MIN_LOG_VALUE {
            (
                format!("coordinates on a log axis must be at least {MIN_LOG_VALUE:e}"),
                "use a linear axis, or rescale the coordinates.",
            )
        } else {
            continue;
        };
        return Err(Error::data(
            context,
            format!("{name}[{i}] is {}; {required}.", number(v)),
        )
        .with_parameter(name)
        .with_hint(hint));
    }
    Ok(())
}

/// Checks that the extent of the coordinates `values`, the argument `name`, is a range an axis
/// of `transform` can show (`Error::Data`).
fn check_extent(
    context: &'static str,
    name: &'static str,
    values: &[f64],
    transform: Transform,
) -> Result<()> {
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if ContinuousScale::new(transform, low, high, context).is_ok() {
        return Ok(());
    }
    let required = if matches!(transform, Transform::Log(_)) {
        "the largest coordinate must be at least 1 + 1e-9 times the smallest"
    } else {
        "the range must be at least 1e-10 of their magnitude (and at least 1e-290)"
    };
    Err(Error::data(
        context,
        format!(
            "{name} spans {} to {}; {required}.",
            number(low),
            number(high)
        ),
    )
    .with_parameter(name)
    .with_hint("rescale the coordinates, or spread the points wider."))
}

/// The given triangles, each checked on its own: at least one, indices in range, three
/// different corners that do not lie on one line (`Error::Data` about `triangles`).
fn check_given(
    context: &'static str,
    (x, y): (&[f64], &[f64]),
    given: &[[i64; 3]],
) -> Result<Vec<[usize; 3]>> {
    let fault = |message: String, hint: &'static str| {
        Error::data(context, message)
            .with_parameter("triangles")
            .with_hint(hint)
    };
    if given.is_empty() {
        return Err(fault(
            "triangles is empty; pass at least one triangle.".to_owned(),
            "pass triangles=AUTO for the Delaunay triangulation of the points.",
        ));
    }
    let count = x.len();
    let points = Exact::new(x, y);
    let mut rows = Vec::with_capacity(given.len());
    for (row, corners) in given.iter().enumerate() {
        let mut checked = [0_usize; 3];
        for (k, &index) in corners.iter().enumerate() {
            checked[k] = usize::try_from(index)
                .ok()
                .filter(|&i| i < count)
                .ok_or_else(|| {
                    fault(
                        format!(
                            "triangles[{row}][{k}] is {index}; indices must be from 0 to {}, \
                             one per point.",
                            count - 1
                        ),
                        "index the points from 0, in the order of x and y.",
                    )
                })?;
        }
        let [a, b, c] = checked;
        if a == b || b == c || a == c {
            return Err(fault(
                format!(
                    "triangles[{row}] is [{a}, {b}, {c}]; its three corners must be different \
                     points."
                ),
                "leave the triangle out, or give it three different corners.",
            ));
        }
        if points.turn(a, b, c) == Ordering::Equal {
            let at = |i: usize| format!("({}, {})", number(x[i]), number(y[i]));
            return Err(fault(
                format!(
                    "triangles[{row}] is [{a}, {b}, {c}]; its corners {}, {} and {} lie on one \
                     line, so it has no area.",
                    at(a),
                    at(b),
                    at(c)
                ),
                "leave the triangle out; a triangle needs corners that do not lie on one line.",
            ));
        }
        rows.push(checked);
    }
    Ok(rows)
}

/// Refuses a triangle given twice: of the sorted triangles `sorted`, each with its row, the
/// first that repeats, with the lowest two rows (`Error::Data` about `triangles`).
fn check_repeats(context: &'static str, sorted: &[([usize; 3], usize)]) -> Result<()> {
    let Some(pair) = sorted.windows(2).find(|pair| pair[0].0 == pair[1].0) else {
        return Ok(());
    };
    let ([a, b, c], first, second) = (pair[0].0, pair[0].1, pair[1].1);
    Err(Error::data(
        context,
        format!(
            "triangles[{first}] and triangles[{second}] join the same points, {a}, {b} and {c}; \
             each triangle must be given once."
        ),
    )
    .with_parameter("triangles")
    .with_hint("leave the repeated triangle out."))
}

/// Refuses an edge that borders more than two triangles: of the edges of the sorted triangles
/// `sorted`, each with its row, the first in sorted order, named with its lowest three rows
/// (`Error::Data` about `triangles`).
fn check_edges(context: &'static str, sorted: &[([usize; 3], usize)]) -> Result<()> {
    let mut edges: Vec<(usize, usize, usize)> = sorted
        .iter()
        .flat_map(|&([a, b, c], row)| [(a, b, row), (a, c, row), (b, c, row)])
        .collect();
    edges.sort_unstable();
    let Some(three) = edges
        .windows(3)
        .find(|w| (w[0].0, w[0].1) == (w[2].0, w[2].1))
    else {
        return Ok(());
    };
    let (a, b) = (three[0].0, three[0].1);
    let (r0, r1, r2) = (three[0].2, three[1].2, three[2].2);
    Err(Error::data(
        context,
        format!(
            "the edge from point {a} to point {b} borders triangles[{r0}], triangles[{r1}] and \
             triangles[{r2}]; an edge borders at most two triangles."
        ),
    )
    .with_parameter("triangles")
    .with_hint("leave overlapping triangles out; an edge has at most one triangle on each side."))
}
