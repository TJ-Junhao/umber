//! Step marks: the points are reduced like a line's (M4 downsampling over x), then expanded into
//! horizontal and vertical segments, and the stepped path is clipped and stroked as a line is.

use super::MarkContext;
use super::line::{downsample, line_items, line_style};
use super::mapping::{axis_x, axis_y};
use crate::Rgba;
use crate::model::{Step, StepMark};
use crate::resolve::Resolved;
use crate::scene::Item;

/// The scene items of one step mark in `color`, `width` pt wide: its stroke, and nothing when no
/// part of it is visible.
///
/// On a continuous x axis the points are first reduced as a line's are, keeping the first, last,
/// lowest and highest point of every device column, and then stepped, so a long series costs
/// what a line of the same size costs. Over the few points a column holds, the level of a
/// point is held until the next point that is kept; the columns are narrower than a pixel, so
/// that differs from stepping every point by less than a pixel in x. A category x axis is not
/// reduced.
pub(super) fn step_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &StepMark,
    (color, width): (Rgba, f32),
    items: &mut Vec<Item>,
) {
    let xs = axis_x(&resolved.x, mark.x());
    let ys = axis_y(&resolved.y, mark.y());
    let (xs, ys) = if resolved.x.is_category() {
        (xs, ys)
    } else {
        downsample(context, xs, ys)
    };
    let (xs, ys) = stepped(&xs, &ys, mark.options().position);
    let style = line_style(width, color, mark.options().dash);
    line_items(context, &xs, &ys, &style, items);
}

/// The vertices of the step path through the axis-space points `(xs, ys)`, as x and y vectors,
/// with a NaN pair between the runs that a gap separates.
///
/// Every point with finite x and y holds its level over an interval, and the path runs along
/// that interval, then up or down to the next interval's level. The interval of point `i` is
/// `[x[i], x[i + 1]]` for [`Step::Post`], `[x[i - 1], x[i]]` for [`Step::Pre`] and
/// `[(x[i - 1] + x[i]) / 2, (x[i] + x[i + 1]) / 2]` for [`Step::Mid`]; at the ends of the series
/// and next to a point without a finite x, the interval ends at the point's own x. A point
/// without a finite y has no interval and breaks the path, so does a point without a finite x.
/// Repeated vertices are dropped, so a point whose interval is empty adds one vertex.
fn stepped(xs: &[f64], ys: &[f64], position: Step) -> (Vec<f64>, Vec<f64>) {
    let n = xs.len().min(ys.len());
    let finite = |i: usize| xs[i].is_finite() && ys[i].is_finite();
    let neighbour = |i: Option<usize>| {
        i.filter(|&j| j < n)
            .map(|j| xs[j])
            .filter(|x| x.is_finite())
    };
    let (mut out_x, mut out_y) = (Vec::with_capacity(2 * n), Vec::with_capacity(2 * n));
    for i in 0..n {
        if !finite(i) {
            continue;
        }
        let (x, y) = (xs[i], ys[i]);
        let previous = neighbour(i.checked_sub(1));
        let next = neighbour(Some(i + 1));
        let (from, to) = match position {
            Step::Post => (x, next.unwrap_or(x)),
            Step::Pre => (previous.unwrap_or(x), x),
            Step::Mid => (
                previous.map_or(x, |p| (p + x) / 2.0),
                next.map_or(x, |q| (x + q) / 2.0),
            ),
        };
        if (i == 0 || !finite(i - 1)) && !out_x.is_empty() {
            out_x.push(f64::NAN);
            out_y.push(f64::NAN);
        }
        for vertex in [(from, y), (to, y)] {
            if out_x.last() != Some(&vertex.0) || out_y.last() != Some(&vertex.1) {
                out_x.push(vertex.0);
                out_y.push(vertex.1);
            }
        }
    }
    (out_x, out_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAN: f64 = f64::NAN;

    /// The vertices as pairs, with `None` for a break.
    fn path(xs: &[f64], ys: &[f64], position: Step) -> Vec<Option<(f64, f64)>> {
        let (x, y) = stepped(xs, ys, position);
        x.into_iter()
            .zip(y)
            .map(|(x, y)| (!x.is_nan()).then_some((x, y)))
            .collect()
    }

    fn points(vertices: &[(f64, f64)]) -> Vec<Option<(f64, f64)>> {
        vertices.iter().map(|&v| Some(v)).collect()
    }

    #[test]
    fn post_pre_mid_vertices() {
        let (x, y) = ([0.0, 1.0, 3.0], [1.0, 2.0, 0.0]);
        assert_eq!(
            path(&x, &y, Step::Post),
            points(&[(0.0, 1.0), (1.0, 1.0), (1.0, 2.0), (3.0, 2.0), (3.0, 0.0)])
        );
        assert_eq!(
            path(&x, &y, Step::Pre),
            points(&[(0.0, 1.0), (0.0, 2.0), (1.0, 2.0), (1.0, 0.0), (3.0, 0.0)])
        );
        assert_eq!(
            path(&x, &y, Step::Mid),
            points(&[
                (0.0, 1.0),
                (0.5, 1.0),
                (0.5, 2.0),
                (2.0, 2.0),
                (2.0, 0.0),
                (3.0, 0.0)
            ])
        );
    }

    #[test]
    fn a_gap_breaks_the_path_where_the_held_level_ends() {
        let (x, y) = ([0.0, 1.0, 2.0, 3.0, 4.0], [1.0, 2.0, NAN, 4.0, 5.0]);
        // Post: the level of point 1 holds up to x = 2, where the gap starts.
        assert_eq!(
            path(&x, &y, Step::Post),
            [
                Some((0.0, 1.0)),
                Some((1.0, 1.0)),
                Some((1.0, 2.0)),
                Some((2.0, 2.0)),
                None,
                Some((3.0, 4.0)),
                Some((4.0, 4.0)),
                Some((4.0, 5.0)),
            ]
        );
        // Pre: the level of point 3 starts at x = 2, where the gap ends.
        assert_eq!(
            path(&x, &y, Step::Pre),
            [
                Some((0.0, 1.0)),
                Some((0.0, 2.0)),
                Some((1.0, 2.0)),
                None,
                Some((2.0, 4.0)),
                Some((3.0, 4.0)),
                Some((3.0, 5.0)),
                Some((4.0, 5.0)),
            ]
        );
    }

    #[test]
    fn a_lone_point_is_one_vertex_and_nothing_is_nothing() {
        assert_eq!(path(&[2.0], &[3.0], Step::Mid), points(&[(2.0, 3.0)]));
        assert_eq!(path(&[], &[], Step::Post), []);
        assert_eq!(path(&[0.0, 1.0], &[NAN, NAN], Step::Post), []);
        assert_eq!(
            path(&[NAN, 1.0], &[1.0, 2.0], Step::Pre),
            points(&[(1.0, 2.0)])
        );
    }

    #[test]
    fn equal_x_steps_straight_up() {
        assert_eq!(
            path(&[0.0, 1.0, 1.0, 2.0], &[0.0, 1.0, 2.0, 2.0], Step::Post),
            points(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (1.0, 2.0), (2.0, 2.0)])
        );
    }
}
