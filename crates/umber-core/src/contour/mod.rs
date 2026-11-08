//! Contour geometry: the lines where a field crosses a level, the regions of the field at or
//! above each level (the bands of a filled contour), and where a level label sits on a drawn
//! line ([`labels`]). Two fields are traced through one interface, [`ContourField`]: values on
//! a rectilinear grid ([`Grid`], in `march` and `bands`) and values at the points of a
//! triangle mesh ([`TriField`], in `triangles`).
//!
//! **Shared.** Both fields follow the same rules: a point is *high* when its value is at or
//! above the level; a level crosses a side or an edge between a high end and a low one, at the
//! point interpolated linearly from its end with the lower index, so neighbours agree on it bit
//! for bit; each line keeps the high side on its left; and only cells or triangles whose values
//! are all finite take part, so NaN makes holes. Each field cuts its own oriented pieces
//! between vertices that name points exactly, and one joining step (`join`) joins them, the
//! same way for both, into lines (open ones first, then closed ones) and into the closed rings
//! of a region. [`ContourShapes::of`] asks a field for the lines and regions that a
//! [`ContourStyle`] draws, and makes nested bands of the regions ([`BandPolygons`]). Level
//! labels ([`labels`]) are placed on the drawn lines on the page, whatever the field.
//!
//! **Grid only.** Index space, marching squares and the saddle rule, the sides of cells as
//! vertices, and the simplification of rings along grid lines.
//!
//! **Mesh only.** Triangles walked counter-clockwise, the rule for levels equal to values (only
//! parts with an area count), and the points and edges of the mesh as vertices.

mod bands;
mod join;
pub(crate) mod labels;
mod march;
mod triangles;

pub(crate) use march::{Grid, GridPoint};
pub(crate) use triangles::{MeshPoint, TriField};

use crate::ContourStyle;

/// One line where the field crosses a level: points of the field's space in order along the
/// line, which keeps the high side of the field on its left (counter-clockwise around the high
/// regions, with `y` up). A closed line, one that returns to where it starts, repeats its first
/// point at the end; an open line ends on the edge of the grid or mesh, or of a hole.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Polyline<P> {
    /// The points, at least two.
    pub points: Vec<P>,
    /// Whether the line returns to its first point.
    pub closed: bool,
}

/// One band of a filled contour: the region of the field at or above the band's lower level,
/// or every finite cell or triangle for the first band, the one below the first level. It is
/// given by its boundary rings in the field's space, each listed once without repeating its
/// first point, to be filled with the even-odd rule.
///
/// The bands are filled in order, each over the ones before it, so band `j` shows where the
/// field lies between its lower level and the next one. Each band lies under the next, so
/// neighbouring bands meet without a seam.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BandPolygons<P> {
    /// The boundary rings, each of at least three points.
    pub rings: Vec<Vec<P>>,
}

/// What a field gives for one level: the lines along it and the boundary rings of the region
/// at or above it, each empty when the style does not draw it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Traced<P> {
    /// The lines along the level.
    pub lines: Vec<Polyline<P>>,
    /// The boundary rings of the region at or above the level.
    pub rings: Vec<Vec<P>>,
}

/// A field that contours are traced on, whose geometry is given in points of type `Point`: grid
/// index space for a grid, points and edges of the mesh for a triangle mesh.
pub(crate) trait ContourField {
    /// A point of the field's space.
    type Point;

    /// The boundary rings of every finite cell or triangle: the region of the first band.
    fn whole(&self) -> Vec<Vec<Self::Point>>;

    /// The lines along `level`, when `style` draws lines, and the boundary rings of the region
    /// at or above it, when `style` fills bands, from one pass over the field.
    fn trace(&self, level: f64, style: ContourStyle) -> Traced<Self::Point>;
}

/// The geometry of one contour in its field's space: the lines along each of its levels, when
/// it is drawn with lines, and its bands, when it is filled.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ContourShapes<P> {
    /// The lines of each level, in the order of the levels; none for a contour without lines.
    pub lines: Vec<Vec<Polyline<P>>>,
    /// The bands, one more than the levels ([`BandPolygons`]); none for a contour without fill.
    pub bands: Vec<BandPolygons<P>>,
}

impl<P> ContourShapes<P> {
    /// The lines and bands that `style` draws for `field` at the increasing `levels`: for
    /// filled bands, the whole field first, then for each level the region at or above it.
    pub(crate) fn of<F>(field: &F, levels: &[f64], style: ContourStyle) -> ContourShapes<P>
    where
        F: ContourField<Point = P>,
    {
        let (lines, filled) = match style {
            ContourStyle::Lines => (true, false),
            ContourStyle::Filled => (false, true),
            ContourStyle::Both => (true, true),
        };
        let mut shapes = ContourShapes::empty();
        if filled {
            shapes.bands.push(BandPolygons {
                rings: field.whole(),
            });
        }
        for &level in levels {
            let traced = field.trace(level, style);
            if lines {
                shapes.lines.push(traced.lines);
            }
            if filled {
                shapes.bands.push(BandPolygons {
                    rings: traced.rings,
                });
            }
        }
        shapes
    }

    /// No lines and no bands.
    pub(crate) fn empty() -> ContourShapes<P> {
        ContourShapes {
            lines: Vec::new(),
            bands: Vec::new(),
        }
    }
}

/// The width of the lines of [`ContourStyle::Lines`], in points.
pub(crate) const LINE_WIDTH: f32 = 1.25;
/// The width of the lines of [`ContourStyle::Both`], in points.
pub(crate) const BOTH_LINE_WIDTH: f32 = 0.6;

/// The width of the lines a contour of `style` draws, in points; `None` for bands alone.
pub(crate) fn line_width(style: ContourStyle) -> Option<f32> {
    match style {
        ContourStyle::Lines => Some(LINE_WIDTH),
        ContourStyle::Both => Some(BOTH_LINE_WIDTH),
        ContourStyle::Filled => None,
    }
}

#[cfg(test)]
mod mesh_tests;
#[cfg(test)]
mod tests;
