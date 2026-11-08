//! Contours on a triangle mesh: values at the points of a [`TriMesh`], linear within each
//! triangle. Everything here is specific to the mesh.
//!
//! **Triangles.** Only a triangle whose three values are finite takes part, so a NaN at a point
//! removes every triangle around it and leaves a hole. Each triangle is walked
//! counter-clockwise in data coordinates with y up, decided exactly ([`Exact`]), from its
//! lowest point index, so neither the row order nor the winding of the given triangles reaches
//! the result.
//!
//! **Crossings.** A point is *high* when its value is at or above the level. A level crosses an
//! edge whose ends differ (one high, one not) at the fraction `t = (level - a) / (b - a)` of the
//! way from its end with the lower index, of value `a`, to the other, of value `b`: computed
//! from the edge alone, so the two triangles that share it agree bit for bit. Within a triangle
//! the field is linear, so the level crosses it in at most one straight piece, from the edge
//! where its outline, walked counter-clockwise, leaves the high corners to the edge where it
//! reaches them again: the high side is on the line's left. There are no saddles. A crossing at
//! `t = 0` or `t = 1`, such as on an edge whose high end lies exactly on the level, is that end
//! itself, a point of the mesh, so lines meeting there join.
//!
//! **Levels equal to values.** The part of a triangle at or above a level is the polygon of its
//! high corners and its crossings. It is *solid* when it has at least three different vertices,
//! so an area; it has none when only one corner lies exactly on the level and the others are
//! below it (the part is that point), or two corners lie on it and the third below (the part is
//! their edge). The region of a level is the union of the solid parts, and its boundary is made
//! of the pieces of their outlines that no other solid part shares: the line through each solid
//! triangle that the level cuts, and the parts of its edges that face no solid part across
//! them. A triangle on the level everywhere (a flat triangle) is wholly high, so it lies in the
//! region and inside the band above the level. So:
//!
//! - an edge with both ends on the level carries a line, once, when it separates a solid part
//!   from a triangle without one, as the edge between a flat triangle and one below the level
//!   does; a ridge on the level with lower values on both sides carries none, nor does a point on
//!   the level with lower values all around it, since the region has no area there;
//! - lines have no piece of zero length, and no piece is drawn twice;
//! - lines are exactly where two bands meet, so the bands still nest.
//!
//! **Lines.** The pieces through the triangles and along the edges facing a triangle without a
//! solid part are joined by their vertices ([`join::lines`]): open lines first, each ending on
//! the edge of the mesh or of a hole, then closed loops, each repeating its first point at its
//! end. Where lines touch at a point of the mesh on the level, the first unused piece in order
//! leaving it is taken. Pieces come in the order of the mesh's triangles (sorted, see
//! [`TriMesh`]), each triangle's line first and then its edges counter-clockwise from its
//! lowest point index.
//!
//! **Regions.** The same pieces, with the parts of the high edges that face no triangle (the
//! edge of the mesh or of a hole), close into rings ([`join::rings`]): counter-clockwise around
//! the region and clockwise around its holes, in data coordinates with y up, filled with the
//! even-odd rule. The first band, below every level, is every triangle that takes part,
//! bounded by its edges that face none.
//!
//! Where given triangles overlap one another, which the checks of [`TriMesh`] allow, a level
//! may cross a shared edge the same way from both sides; lines then break into open pieces
//! there, and the overlap is filled as the even-odd rule does it.

use std::cmp::Ordering;

use super::join;
use super::{ContourField, Polyline, Traced};
use crate::ContourStyle;
use crate::tri::{Exact, TriMesh};

/// A point of contour geometry on a triangle mesh: a point of the mesh, or a point on an edge,
/// where a level crosses it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MeshPoint {
    /// The point of the mesh with this index.
    Corner(usize),
    /// The point the fraction `t` of the way along the edge from the point `from` to the point
    /// `to`, with `from < to` and `t` strictly between 0 and 1.
    Edge {
        /// The end of the edge with the lower index.
        from: usize,
        /// The end with the higher index.
        to: usize,
        /// How far along the edge the point lies.
        t: f64,
    },
}

impl MeshPoint {
    /// The vertex this point is at. At one level each edge has one crossing, so two points of
    /// one level are equal exactly when their vertices are.
    fn vertex(self) -> Vertex {
        match self {
            MeshPoint::Corner(i) => Vertex::Corner(i),
            MeshPoint::Edge { from, to, .. } => Vertex::Crossing(from, to),
        }
    }
}

/// A vertex of contour geometry at one level, named exactly: a point of the mesh, or the
/// crossing of the level on an edge, strictly between its ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Vertex {
    /// The point of the mesh with this index.
    Corner(usize),
    /// The crossing on the edge between these points, the lower index first.
    Crossing(usize, usize),
}

/// The values at the points of a triangle mesh, with the triangles that take part.
#[derive(Debug, Clone)]
pub(crate) struct TriField<'a> {
    /// The value at each point.
    values: &'a [f64],
    /// The triangles whose three values are finite, in the mesh's order, each counter-clockwise
    /// in data coordinates from its lowest point index.
    triangles: Vec<[usize; 3]>,
    /// For each triangle of `triangles` and each of its sides, from its corner `k` to its
    /// corner `k + 1`, the triangle of `triangles` across that side, if there is one.
    across: Vec<[Option<usize>; 3]>,
}

/// One oriented piece of a region's boundary at a level.
#[derive(Debug, Clone, Copy)]
struct Piece {
    /// Where it starts.
    from: MeshPoint,
    /// Where it ends.
    to: MeshPoint,
    /// Whether it is part of a line: a piece through a triangle, or along an edge that faces a
    /// triangle without a solid part. The other pieces run along the edge of the mesh or of a
    /// hole.
    line: bool,
}

impl<'a> TriField<'a> {
    /// The field of `values`, one per point of `mesh`.
    pub(crate) fn new(mesh: &TriMesh, values: &'a [f64]) -> TriField<'a> {
        let exact = Exact::new(mesh.x(), mesh.y());
        let finite = |i: usize| values.get(i).is_some_and(|v| v.is_finite());
        let triangles: Vec<[usize; 3]> = mesh
            .triangles()
            .iter()
            .filter(|corners| corners.iter().all(|&i| finite(i)))
            // The corners are in increasing index order; a clockwise triangle swaps its last two.
            .map(|&[a, b, c]| match exact.turn(a, b, c) {
                Ordering::Less => [a, c, b],
                Ordering::Greater | Ordering::Equal => [a, b, c],
            })
            .collect();
        // Each side once per triangle, by its ends in increasing order: a side shared by two
        // triangles appears twice, side by side once sorted.
        let mut sides: Vec<([usize; 2], usize, usize)> = triangles
            .iter()
            .enumerate()
            .flat_map(|(t, corners)| {
                (0..3).map(move |k| {
                    let (p, q) = (corners[k], corners[(k + 1) % 3]);
                    ([p.min(q), p.max(q)], t, k)
                })
            })
            .collect();
        sides.sort_unstable();
        let mut across = vec![[None; 3]; triangles.len()];
        for pair in sides.windows(2) {
            let [(edge, t, k), (other, u, l)] = [pair[0], pair[1]];
            if edge == other {
                across[t][k] = Some(u);
                across[u][l] = Some(t);
            }
        }
        TriField {
            values,
            triangles,
            across,
        }
    }

    /// Where `level` crosses the edge between the points `p` and `q`, one high and one not, at
    /// the fraction of the way from the end with the lower index: that end or the other when
    /// the fraction is 0 or 1.
    fn crossing(&self, p: usize, q: usize, level: f64) -> MeshPoint {
        let (from, to) = (p.min(q), p.max(q));
        let (a, b) = (self.values[from], self.values[to]);
        // The ends differ, one at or above the level and one below it, so `b - a` is not zero;
        // both are at most 1e300 in magnitude, so it is finite.
        let t = ((level - a) / (b - a)).clamp(0.0, 1.0);
        if t <= 0.0 {
            MeshPoint::Corner(from)
        } else if t >= 1.0 {
            MeshPoint::Corner(to)
        } else {
            MeshPoint::Edge { from, to, t }
        }
    }

    /// The triangle with the corners `corners` (counter-clockwise) cut at `level`; every corner
    /// is high for `None`.
    fn cut(&self, corners: [usize; 3], level: Option<f64>) -> Cut {
        let high = corners.map(|i| level.is_none_or(|level| self.values[i] >= level));
        let mut crossings = [None; 3];
        if let Some(level) = level {
            for (k, slot) in crossings.iter_mut().enumerate() {
                if high[k] != high[(k + 1) % 3] {
                    *slot = Some(self.crossing(corners[k], corners[(k + 1) % 3], level));
                }
            }
        }
        Cut {
            corners,
            high,
            crossings,
        }
    }

    /// The pieces of the boundary of the region at or above `level`, or of every triangle for
    /// `None`, in order (see the module documentation).
    fn pieces(&self, level: Option<f64>) -> Vec<Piece> {
        let cuts: Vec<Cut> = self
            .triangles
            .iter()
            .map(|&corners| self.cut(corners, level))
            .collect();
        let solid: Vec<bool> = cuts.iter().map(Cut::solid).collect();
        let mut pieces = Vec::new();
        let mut push = |(from, to): (MeshPoint, MeshPoint), line: bool| {
            if from.vertex() != to.vertex() {
                pieces.push(Piece { from, to, line });
            }
        };
        for ((cut, sides), &solid_cut) in cuts.iter().zip(&self.across).zip(&solid) {
            if !solid_cut {
                continue;
            }
            if let Some(piece) = cut.through() {
                push(piece, true);
            }
            for (k, &across) in sides.iter().enumerate() {
                if across.is_some_and(|other| solid.get(other) == Some(&true)) {
                    continue;
                }
                if let Some(piece) = cut.high_side(k) {
                    push(piece, across.is_some());
                }
            }
        }
        pieces
    }
}

/// The lines that `pieces` make.
fn lines(pieces: &[Piece]) -> Vec<Polyline<MeshPoint>> {
    let pieces: Vec<&Piece> = pieces.iter().filter(|piece| piece.line).collect();
    let ends: Vec<(Vertex, Vertex)> = pieces
        .iter()
        .map(|piece| (piece.from.vertex(), piece.to.vertex()))
        .collect();
    join::lines(&ends)
        .into_iter()
        .map(|chain| {
            let start = chain
                .pieces
                .first()
                .and_then(|&first| pieces.get(first))
                .map(|piece| piece.from);
            let ends = chain
                .pieces
                .iter()
                .filter_map(|&index| pieces.get(index))
                .map(|piece| piece.to);
            Polyline {
                points: start.into_iter().chain(ends).collect(),
                closed: chain.closed,
            }
        })
        .collect()
}

/// The closed rings that `pieces` make, each of at least three points.
fn rings(pieces: &[Piece]) -> Vec<Vec<MeshPoint>> {
    let ends: Vec<(Vertex, Vertex)> = pieces
        .iter()
        .map(|piece| (piece.from.vertex(), piece.to.vertex()))
        .collect();
    join::rings(&ends)
        .into_iter()
        .map(|ring| {
            ring.iter()
                .filter_map(|&index| pieces.get(index))
                .map(|piece| piece.from)
                .collect::<Vec<MeshPoint>>()
        })
        .filter(|ring| ring.len() >= 3)
        .collect()
}

/// The mesh traced as [`ContourField`] says (see the module documentation).
impl ContourField for TriField<'_> {
    type Point = MeshPoint;

    fn whole(&self) -> Vec<Vec<MeshPoint>> {
        rings(&self.pieces(None))
    }

    fn trace(&self, level: f64, style: ContourStyle) -> Traced<MeshPoint> {
        let pieces = self.pieces(Some(level));
        Traced {
            lines: match style {
                ContourStyle::Lines | ContourStyle::Both => lines(&pieces),
                ContourStyle::Filled => Vec::new(),
            },
            rings: match style {
                ContourStyle::Filled | ContourStyle::Both => rings(&pieces),
                ContourStyle::Lines => Vec::new(),
            },
        }
    }
}

/// One triangle cut at a level.
#[derive(Debug, Clone, Copy)]
struct Cut {
    /// The corners, counter-clockwise.
    corners: [usize; 3],
    /// Whether each corner is high.
    high: [bool; 3],
    /// Where the level crosses each side, from corner `k` to corner `k + 1`, when one end is
    /// high and the other not.
    crossings: [Option<MeshPoint>; 3],
}

impl Cut {
    /// Whether the part of the triangle at or above the level is solid: whether its outline,
    /// the high corners and the crossings, has at least three different vertices.
    fn solid(&self) -> bool {
        // At most the three corners and the three crossings.
        let mut outline = [None::<Vertex>; 6];
        let mut count = 0;
        for k in 0..3 {
            let corner = Vertex::Corner(self.corners[k]);
            let crossing = self.crossings[k].map(MeshPoint::vertex);
            for vertex in self.high[k].then_some(corner).into_iter().chain(crossing) {
                if !outline[..count].contains(&Some(vertex)) {
                    outline[count] = Some(vertex);
                    count += 1;
                }
            }
        }
        count >= 3
    }

    /// The piece of line through the triangle: from the crossing where its outline, walked
    /// counter-clockwise, leaves the high corners to the one where it reaches them again.
    /// `None` unless the triangle has both high corners and others.
    fn through(&self) -> Option<(MeshPoint, MeshPoint)> {
        let side =
            |high: bool| (0..3).find(|&k| self.high[k] == high && self.high[(k + 1) % 3] != high);
        let (exit, entry) = (side(true)?, side(false)?);
        Some((self.crossings[exit]?, self.crossings[entry]?))
    }

    /// The part of the side `k` at or above the level, oriented counter-clockwise: the whole
    /// side, from a high corner to the crossing, or from the crossing to a high corner; `None`
    /// when both ends are below the level.
    fn high_side(&self, k: usize) -> Option<(MeshPoint, MeshPoint)> {
        let next = (k + 1) % 3;
        let (start, end) = (
            MeshPoint::Corner(self.corners[k]),
            MeshPoint::Corner(self.corners[next]),
        );
        match (self.high[k], self.high[next]) {
            (true, true) => Some((start, end)),
            (true, false) => Some((start, self.crossings[k]?)),
            (false, true) => Some((self.crossings[k]?, end)),
            (false, false) => None,
        }
    }
}
