//! Streamlines: the lines of a [`StreamplotMark`] and their arrow heads.
//!
//! The lines come in data coordinates ([`crate::field::stream`]) and are mapped onto the axes
//! point by point, so a straight segment between two points is straight on the plot; they are
//! clipped in axis space to the plot expanded by a margin, as lines are ([`LineClip`]), before
//! they are mapped to plot points. Lines are as wide as contour lines ([`LINE_WIDTH`]).
//!
//! Lines of one color are one stroked path with round ends and joins, and their heads one
//! filled path after it: two scene items for the whole mark. A color mapping draws each line as
//! pieces [`PIECE`] long, in steps, each starting half a piece after the one before, with flat
//! ends: every end of a piece but the line's own two lies inside the next piece or the one
//! before, which covers it whatever the order, so no seam shows between them. Each piece takes
//! the color of the value at its middle, interpolated along the line, and the pieces of one
//! color are one path; the heads follow, over all the lines, each in the color at its point.
//! Mapped colors that come close to the background move away from it, as the lines of a contour
//! do ([`super::contrast`]).
//!
//! A head is the head of a quiver arrow ([`Outline::head`]) centred on its point of the line
//! ([`Streamline::head`]) and turned along the segment it lies on, as that segment is drawn. A
//! head whose point lies far outside the plot is left out before anything is mapped to points;
//! the marks clip cuts the rest at the plot edge, as it cuts the lines.

use super::MarkContext;
use super::clip::clip_polyline;
use super::field::{Batches, FieldFill, culling_bounds, push_polygon};
use super::line::LineClip;
use super::mapping::{AxisPoint, axis_value};
use crate::Rgba;
use crate::contour::LINE_WIDTH;
use crate::field::arrow::{HEAD_LENGTH, Outline};
use crate::field::{SCREEN_SCALES, direction};
use crate::geometry::Path;
use crate::model::{Streamline, StreamplotMark};
use crate::resolve::Resolved;
use crate::scene::{Item, LineCap, LineJoin, StrokeStyle};

/// The length of a piece of a color-mapped line, in steps; each piece starts half of this after
/// the one before.
pub(crate) const PIECE: f64 = 0.5;

/// The stroke of the lines in `color` with ends `cap`.
fn line_stroke(color: Rgba, cap: LineCap) -> StrokeStyle {
    StrokeStyle {
        color,
        width: LINE_WIDTH,
        dash: Vec::new(),
        cap,
        join: LineJoin::Round,
    }
}

/// The axis-space points of `line` on the axes `resolved`.
fn axis_points(resolved: &Resolved, line: &Streamline) -> Vec<AxisPoint> {
    line.points()
        .iter()
        .map(|&[x, y]| AxisPoint::new(axis_value(&resolved.x, x), axis_value(&resolved.y, y)))
        .collect()
}

/// Appends the polyline through the axis-space `points`, clipped by `clip`, to `path`. A piece
/// of one point is a zero-length subpath, which a round cap draws as a dot.
fn push_line(path: &mut Path, clip: &LineClip, points: &[AxisPoint]) {
    let (xs, ys): (Vec<f64>, Vec<f64>) = points.iter().map(|p| (p.x, p.y)).unzip();
    for piece in clip_polyline(&xs, &ys, &clip.bounds) {
        let mut scene = piece.iter().map(|&p| clip.plot_point(p).to_scene());
        let Some(first) = scene.next() else {
            continue;
        };
        path.move_to(first);
        match scene.next() {
            None => path.line_to(first),
            Some(second) => {
                path.line_to(second);
                scene.for_each(|p| path.line_to(p));
            }
        }
    }
}

/// The point the fraction `t` of the way from `a` to `b`, in axis space.
fn between(a: AxisPoint, b: AxisPoint, t: f64) -> AxisPoint {
    AxisPoint::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Where `length`, in steps from the start of a line with the cumulative `lengths`, lies: the
/// index of the segment and the fraction of the way along it.
fn locate(lengths: &[f64], length: f64) -> (usize, f64) {
    let segment =
        (lengths.partition_point(|&l| l <= length).max(1) - 1).min(lengths.len().saturating_sub(2));
    match lengths.get(segment + 1) {
        Some(&to) if to > lengths[segment] => {
            let t = (length - lengths[segment]) / (to - lengths[segment]);
            (segment, t.clamp(0.0, 1.0))
        }
        _ => (segment, 0.0),
    }
}

/// The value the fraction `t` of the way from `values[segment]` to the value after it.
fn lerp(values: &[f64], segment: usize, t: f64) -> f64 {
    match values.get(segment + 1) {
        Some(&next) => values[segment] + (next - values[segment]) * t,
        None => values[segment],
    }
}

/// The axis-space point of `points` at `length`, in steps along the line of `lengths`.
fn point_at(points: &[AxisPoint], lengths: &[f64], length: f64) -> AxisPoint {
    let (segment, t) = locate(lengths, length);
    match points.get(segment + 1) {
        Some(&next) => between(points[segment], next, t),
        None => points[segment],
    }
}

/// The color in `fill` the fraction `t` along `segment` of a line whose mapped values are
/// `values`, one per point of a mapped line, before it moves away from the background; `None`
/// for a value that maps to no color.
fn color_at(fill: FieldFill<'_>, values: &[f64], (segment, t): (usize, f64)) -> Option<Rgba> {
    match fill {
        FieldFill::Uniform(color) => Some(color),
        FieldFill::Mapped { scale, .. } => scale.color(lerp(values, segment, t)),
    }
}

/// Appends the pieces of the color-mapped `line`, whose axis-space points are `points`, to the
/// paths of their colors in `batches` (see the module documentation).
fn push_pieces(
    batches: &mut Batches,
    clip: &LineClip,
    fill: FieldFill<'_>,
    (line, points): (&Streamline, &[AxisPoint]),
) {
    let (lengths, values) = (line.lengths(), line.values());
    let total = line.length();
    for k in 0_u32.. {
        let start = f64::from(k) * (PIECE / 2.0);
        if start >= total {
            break;
        }
        let end = (start + PIECE).min(total);
        let middle = locate(lengths, (start + end) / 2.0);
        let Some(color) = color_at(fill, values, middle) else {
            continue;
        };
        // The points strictly between the ends of the piece.
        let inside =
            lengths.partition_point(|&l| l <= start)..lengths.partition_point(|&l| l < end);
        let mut piece = Vec::with_capacity(inside.len() + 2);
        piece.push(point_at(points, lengths, start));
        piece.extend_from_slice(points.get(inside).unwrap_or_default());
        piece.push(point_at(points, lengths, end));
        let (stroked, _) = batches.of(color);
        push_line(stroked, clip, &piece);
    }
}

/// Appends the head of `line`, whose axis-space points are `points`, to the filled path of its
/// color in `batches`, unless it lies far outside the plot.
fn push_head(
    context: &MarkContext<'_>,
    batches: &mut Batches,
    fill: FieldFill<'_>,
    (line, points): (&Streamline, &[AxisPoint]),
) {
    let Some(head) = line.head() else {
        return;
    };
    let (Some(&from), Some(&to)) = (points.get(head.segment), points.get(head.segment + 1)) else {
        return;
    };
    let center = between(from, to, head.fraction);
    if !culling_bounds(context, HEAD_LENGTH).contains(center) {
        return;
    }
    let Some(color) = color_at(fill, line.values(), (head.segment, head.fraction)) else {
        return;
    };
    let (a, b) = (context.maps.to_plot(from), context.maps.to_plot(to));
    let (dx, dy) = (b.x - a.x, a.y - b.y);
    if !(dx.is_finite() && dy.is_finite()) || (dx == 0.0 && dy == 0.0) {
        return;
    }
    let outline = Outline::head(direction(dx, dy, SCREEN_SCALES));
    let (_, filled) = batches.of(color);
    push_polygon(filled, context.maps.to_plot(center), outline.corners());
}

/// Appends the lines of `mark` and their heads in the `fill` (see the module documentation).
pub(super) fn streamline_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &StreamplotMark,
    fill: FieldFill<'_>,
    items: &mut Vec<Item>,
) {
    let clip = LineClip::of(context, LINE_WIDTH);
    let arrows = mark.options().arrows;
    let lines: Vec<(&Streamline, Vec<AxisPoint>)> = mark
        .lines()
        .iter()
        .map(|line| (line, axis_points(resolved, line)))
        .collect();
    let mut drawn = Batches::new(context.theme, fill);
    match fill {
        FieldFill::Uniform(color) => {
            let (stroked, _) = drawn.of(color);
            for (_, points) in &lines {
                push_line(stroked, &clip, points);
            }
            if arrows {
                for (line, points) in &lines {
                    push_head(context, &mut drawn, fill, (line, points));
                }
            }
            drawn.append(|color| line_stroke(color, LineCap::Round), items);
        }
        FieldFill::Mapped { .. } => {
            for (line, points) in &lines {
                push_pieces(&mut drawn, &clip, fill, (line, points));
            }
            drawn.append(|color| line_stroke(color, LineCap::Butt), items);
            if arrows {
                let mut heads = Batches::new(context.theme, fill);
                for (line, points) in &lines {
                    push_head(context, &mut heads, fill, (line, points));
                }
                heads.append(|color| line_stroke(color, LineCap::Butt), items);
            }
        }
    }
}
