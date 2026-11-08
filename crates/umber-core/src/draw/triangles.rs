//! Triangle meshes: the edges of a [`TriplotMark`] and the filled triangles of a
//! [`TripcolorMark`].
//!
//! Points are mapped to axis space one by one, so a triangle's sides are straight on the plot
//! between its corners, on linear and log axes alike.
//!
//! **Edges.** Each edge of the mesh is one subpath of one stroked path, in the sorted order of
//! [`TriMesh::edges`](crate::TriMesh::edges), clipped in axis space as lines are
//! ([`LineClip`]), with round ends and joins so that edges meeting at a point leave no notch.
//!
//! **Triangles.** Two antialiased fills that only touch along an edge both cover the pixels on
//! it partly, so the background would show between neighbours. Each triangle is therefore
//! filled and also stroked with a hairline [`HAIRLINE`] points wide in its own color, with
//! round joins: the stroke reaches half the hairline across each edge under the neighbour, and
//! whichever is drawn later covers the edge, so no seam shows. The outer boundary of the mesh,
//! and the boundary of a blank triangle, grow by half the hairline, the same everywhere along
//! them. A triangle wholly inside the plot, expanded by a margin, is drawn as given; any other
//! is clipped to that box in axis space first ([`clip_ring`]). Each polygon is wound one way
//! on the page, decided from its corners as drawn, so the triangles of one color are one
//! filled path under the nonzero rule, and one stroked path: two scene items per color, in the
//! order the colors first appear among the sorted triangles of the mesh.

use std::collections::HashMap;

use super::MarkContext;
use super::clip::{clip_ring, clip_segment};
use super::field::culling_bounds;
use super::line::LineClip;
use super::mapping::{AxisPoint, axis_value, point};
use crate::color_scale::ResolvedColorScale;
use crate::geometry::{Path, Point};
use crate::model::{TripcolorMark, TriplotMark};
use crate::resolve::Resolved;
use crate::scene::{FillRule, Item, LineCap, LineJoin, Paint, StrokeStyle};
use crate::{Rgba, SizeSpec, Theme, TriMesh};

/// The width, in points, of the hairline that strokes each filled triangle in its own color.
/// At this width no seam shows between triangles of contrasting colors in PNG at 144 dpi and
/// above, on light and dark themes (`tests/triangles.rs`); at 0.5 pt a few pixels still showed
/// the background. Half of it, 0.375 pt, is how far the outer boundary grows.
pub(crate) const HAIRLINE: f32 = 0.75;

/// The width of the edges of a triangle mesh under `theme`: half the automatic line width.
pub(crate) fn triplot_width(theme: &Theme) -> f32 {
    theme.drawn_line_width(SizeSpec::Auto) / 2.0
}

/// The axis-space point of each point of `mesh` on the axes `resolved`.
fn axis_points(resolved: &Resolved, mesh: &TriMesh) -> Vec<AxisPoint> {
    mesh.x()
        .iter()
        .zip(mesh.y())
        .map(|(&x, &y)| AxisPoint::new(axis_value(&resolved.x, x), axis_value(&resolved.y, y)))
        .collect()
}

/// Appends the edges of `mark` in `color`, `width` points wide (see the module documentation).
pub(super) fn triplot_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &TriplotMark,
    (color, width): (Rgba, f32),
    items: &mut Vec<Item>,
) {
    let points = axis_points(resolved, mark.mesh());
    let clip = LineClip::of(context, width);
    let mut path = Path::new();
    for &[a, b] in mark.edges() {
        if let Some((from, to)) = clip_segment(points[a], points[b], &clip.bounds) {
            path.move_to(clip.plot_point(from).to_scene());
            path.line_to(clip.plot_point(to).to_scene());
        }
    }
    if path.is_empty() {
        return;
    }
    items.push(Item::Stroke {
        path,
        style: StrokeStyle {
            color,
            width,
            dash: Vec::new(),
            cap: LineCap::Round,
            join: LineJoin::Round,
        },
    });
}

/// Appends the triangles of `mark`, each in the color its value maps to through `scale`, and
/// leaves those whose value maps to no color blank (see the module documentation).
pub(super) fn tripcolor_items(
    context: &MarkContext<'_>,
    resolved: &Resolved,
    mark: &TripcolorMark,
    scale: &ResolvedColorScale,
    items: &mut Vec<Item>,
) {
    let mesh = mark.mesh();
    let points = axis_points(resolved, mesh);
    let bounds = culling_bounds(context, f64::from(HAIRLINE));
    let mut index: HashMap<Rgba, usize> = HashMap::new();
    let mut paths: Vec<(Rgba, Path)> = Vec::new();
    for (&[a, b, c], &value) in mesh.triangles().iter().zip(mark.values()) {
        let Some(color) = scale.color(value) else {
            continue;
        };
        let corners = [points[a], points[b], points[c]];
        let ring = if corners.iter().all(|&p| bounds.contains(p)) {
            corners.to_vec()
        } else {
            clip_ring(&corners, &bounds)
        };
        if ring.len() < 3 {
            continue;
        }
        let mut polygon: Vec<Point> = ring
            .into_iter()
            .map(|p| {
                let p = context.maps.to_plot(p);
                point(p.x, p.y)
            })
            .collect();
        if twice_area(&polygon) < 0.0 {
            polygon.reverse();
        }
        let at = *index.entry(color).or_insert_with(|| {
            paths.push((color, Path::new()));
            paths.len() - 1
        });
        let path = &mut paths[at].1;
        path.move_to(polygon[0]);
        for &p in &polygon[1..] {
            path.line_to(p);
        }
        path.close();
    }
    for (color, path) in paths {
        items.push(Item::Fill {
            path: path.clone(),
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        });
        items.push(Item::Stroke {
            path,
            style: StrokeStyle {
                color,
                width: HAIRLINE,
                dash: Vec::new(),
                cap: LineCap::Butt,
                join: LineJoin::Round,
            },
        });
    }
}

/// Twice the signed area of the closed polygon `points`, on the page (y down): positive when
/// it runs clockwise as seen.
fn twice_area(points: &[Point]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (p, q) = (points[i], points[(i + 1) % n]);
            f64::from(p.x) * f64::from(q.y) - f64::from(q.x) * f64::from(p.y)
        })
        .sum()
}
