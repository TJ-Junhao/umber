//! Triangle meshes on the axes: the edges of a mesh ([`TriplotMark`]), its triangles filled
//! from the color scale ([`TripcolorMark`]), and the contours of values at its points
//! ([`TricontourMark`]). All read their points and triangles through a [`crate::TriMesh`],
//! whose validation and order [`crate::tri`] describes.

use std::sync::Arc;

use super::color_by::{CmapChoice, ColorBy, check_center, check_mapped_values};
use super::contour::ContourSpec;
use super::figure::Figure;
use super::mesh::check_mapping;
use super::streamlines::check_linear_or_log;
use crate::tri::{TriMesh, Triangles};
use crate::{ColorSpec, ContourOptions, Error, Result, ScaleKind, Transform};

/// What triangle meshes need, for the messages that refuse a time or category axis.
const NEEDS: &str = "triangle meshes need";

/// The context of every error of [`AxesMut::triplot`](super::AxesMut::triplot).
pub(super) const TRIPLOT_CONTEXT: &str = "Axes.triplot()";

/// The context of every error of [`AxesMut::tripcolor`](super::AxesMut::tripcolor).
const TRIPCOLOR_CONTEXT: &str = "Axes.tripcolor()";

/// The context of every error of [`AxesMut::tricontour`](super::AxesMut::tricontour).
const TRICONTOUR_CONTEXT: &str = "Axes.tricontour()";

/// Options of the edges of a triangle mesh.
///
/// # Examples
///
/// ```
/// use umber_core::{ColorSpec, TriplotOptions};
///
/// let options = TriplotOptions::default();
/// assert!(options.label.is_empty());
/// assert_eq!(options.color, ColorSpec::AUTO);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TriplotOptions {
    /// The legend label; empty for none. The entry is a line swatch.
    pub label: String,
    /// The edge color; [`ColorSpec::AUTO`] takes the next palette color.
    pub color: ColorSpec,
}

/// The edges of a triangle mesh: each edge once, as one stroked path, at half the automatic
/// line width, with round ends and joins.
///
/// The edges are drawn in sorted order of their point indices ([`TriMesh::edges`]), each a
/// straight segment on the plot between its points, on linear and log axes alike, clipped as
/// lines are. No point is drawn; add a scatter for those. Automatic limits are the extent of
/// the points, without padding.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Mark, ScaleKind, Theme, Triangles, TriplotOptions};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let (x, y) = ([0.0, 1.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
/// axes.triplot(&x, &y, Triangles::Auto, TriplotOptions::default())?;
/// let Some(Mark::Triplot(mark)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(mark.mesh().triangles().len(), 2);
/// assert_eq!(mark.edges().len(), 5);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TriplotMark {
    mesh: TriMesh,
    edges: Arc<[[usize; 2]]>,
    options: TriplotOptions,
}

impl TriplotMark {
    /// Validates the edges of a mesh on axes of the scale kinds `kinds`: both linear or log
    /// (`Error::Usage`, naming `x` or `y`), then the points and triangles as [`TriMesh`] checks
    /// them (`Error::Data`).
    pub(super) fn new(
        kinds: (ScaleKind, ScaleKind),
        points: (&[f64], &[f64]),
        triangles: Triangles<'_>,
        options: TriplotOptions,
    ) -> Result<TriplotMark> {
        let transforms = check_linear_or_log(TRIPLOT_CONTEXT, kinds, NEEDS)?;
        let (mesh, _) = TriMesh::new(TRIPLOT_CONTEXT, transforms, points, triangles)?;
        let edges = mesh.edges().into();
        Ok(TriplotMark {
            mesh,
            edges,
            options,
        })
    }

    /// The points and triangles.
    #[must_use]
    pub fn mesh(&self) -> &TriMesh {
        &self.mesh
    }

    /// Every edge once, in the order drawn: point indices in increasing order, the edges sorted.
    #[must_use]
    pub fn edges(&self) -> &[[usize; 2]] {
        &self.edges
    }

    /// The label and color.
    #[must_use]
    pub fn options(&self) -> &TriplotOptions {
        &self.options
    }
}

impl Default for TriplotOptions {
    fn default() -> TriplotOptions {
        TriplotOptions {
            label: String::new(),
            color: ColorSpec::AUTO,
        }
    }
}

/// What the values of a [`TripcolorMark`] belong to.
///
/// # Examples
///
/// ```
/// use umber_core::TriValues;
///
/// assert_eq!(TriValues::default(), TriValues::Points);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TriValues {
    /// One value per point; a triangle takes the mean of its three corners.
    #[default]
    Points,
    /// One value per triangle, in the order of the given triangles (or of [`delaunay`]'s
    /// result), as finite-element results often are.
    ///
    /// [`delaunay`]: crate::delaunay
    Triangles,
}

/// Options of the colors of a triangle mesh.
///
/// # Examples
///
/// ```
/// use umber_core::{CmapChoice, TripcolorOptions, TriValues, Transform};
///
/// let options = TripcolorOptions::default();
/// assert_eq!((options.on, &options.cmap), (TriValues::Points, &CmapChoice::Theme));
/// assert_eq!((options.scale, options.center), (Transform::Linear, None));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TripcolorOptions {
    /// What the values belong to; their number is checked against it, never used to guess it.
    pub on: TriValues,
    /// The colormap; the theme chooses sequential or diverging according to `center`.
    pub cmap: CmapChoice,
    /// Linear or log color normalization.
    pub scale: Transform,
    /// The middle value of a symmetric, linear diverging mapping.
    pub center: Option<f64>,
}

impl Default for TripcolorOptions {
    fn default() -> TripcolorOptions {
        TripcolorOptions {
            on: TriValues::Points,
            cmap: CmapChoice::Theme,
            scale: Transform::Linear,
            center: None,
        }
    }
}

/// The triangles of a mesh, each filled with one color on the axes' color scale, as the cells
/// of a mesh are.
///
/// Each triangle takes one value: its own with [`TriValues::Triangles`], or the mean of its
/// three corners with [`TriValues::Points`]. A NaN leaves the triangle blank; with
/// [`TriValues::Points`] one NaN corner is enough. The automatic color range is that of the
/// triangles' values, the colors drawn. Triangles are straight on the plot between their
/// corners, on linear and log axes alike.
///
/// Each triangle is filled and stroked with a hairline 0.75 pt wide in its own color, with
/// round joins, so that neighbours meet without a seam; the outer boundary of the mesh, and the
/// edge of a blank triangle, grow by half the hairline, 0.375 pt. Triangles are drawn by value,
/// lowest first, so where triangles overlap the higher value is on top, whatever else the mesh
/// holds; triangles of one color drawn one after another are one filled and one stroked path,
/// two scene items per color. The mesh is always drawn as vector shapes.
/// Automatic limits are the extent of the points, without padding.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Mark, ScaleKind, Theme, Triangles, TripcolorOptions, TriValues};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let (x, y) = ([0.0, 1.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
/// let triangles = [[0, 1, 2], [0, 2, 3]];
/// axes.tripcolor(&x, &y, Triangles::Given(&triangles), &[0.0, 3.0, 6.0, f64::NAN], TripcolorOptions::default())?;
/// let Some(Mark::Tripcolor(mark)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(mark.values()[0], 3.0);
/// assert!(mark.values()[1].is_nan());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TripcolorMark {
    mesh: TriMesh,
    pub(crate) colors: ColorBy,
    options: TripcolorOptions,
}

impl TripcolorMark {
    /// Validates the colored triangles of a mesh on the axes at `index` of `figure` before
    /// anything is stored. The checks run in this order, and the first failure is the error:
    ///
    /// 1. both axes are linear or log axes (`Error::Usage`, naming `x` or `y`);
    /// 2. the points and triangles, as [`TriMesh`] checks them (`Error::Data`);
    /// 3. `values` holds one value per point or per triangle, as `on` says (`Error::Data`);
    /// 4. every value is NaN or finite, at most 1e300 in magnitude, and positive and at least
    ///    1e-300 on a log color scale (`Error::Data`, naming it);
    /// 5. the center is finite, at most 1e300 in magnitude, and not combined with a log scale
    ///    (`Error::Style`);
    /// 6. the colors map as the axes' other color-mapped marks do, within their color limits
    ///    and their color-sharing group (`Error::Usage` and `Error::Style`).
    pub(super) fn new(
        figure: &Figure,
        index: usize,
        points: (&[f64], &[f64]),
        triangles: Triangles<'_>,
        values: &[f64],
        options: TripcolorOptions,
    ) -> Result<TripcolorMark> {
        let axes = figure.panel(index);
        let kinds = (axes.x_scale(), axes.y_scale());
        let transforms = check_linear_or_log(TRIPCOLOR_CONTEXT, kinds, NEEDS)?;
        let (mesh, rows) = TriMesh::new(TRIPCOLOR_CONTEXT, transforms, points, triangles)?;
        check_length(values.len(), options.on, &mesh)?;
        check_mapped_values(
            TRIPCOLOR_CONTEXT,
            values,
            options.scale,
            |i| format!("values[{i}]"),
            "use NaN to leave a triangle blank.",
        )?;
        check_center(TRIPCOLOR_CONTEXT, options.center, options.scale)?;
        let cells: Vec<f64> = match options.on {
            // The corners are in increasing index order, whatever order they were given in.
            TriValues::Points => mesh
                .triangles()
                .iter()
                .map(|&[a, b, c]| (values[a] + values[b] + values[c]) / 3.0)
                .collect(),
            TriValues::Triangles => rows.iter().map(|&row| values[row]).collect(),
        };
        let colors = ColorBy {
            values: cells.into(),
            cmap: options.cmap.clone(),
            scale: options.scale,
            center: options.center,
        };
        check_mapping(TRIPCOLOR_CONTEXT, figure, index, &colors)?;
        Ok(TripcolorMark {
            mesh,
            colors,
            options,
        })
    }

    /// The points and triangles.
    #[must_use]
    pub fn mesh(&self) -> &TriMesh {
        &self.mesh
    }

    /// The value of each triangle of [`mesh`](Self::mesh), in its order: NaN for a blank one.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.colors.values
    }

    /// What the values belong to, and how they map to colors.
    #[must_use]
    pub fn options(&self) -> &TripcolorOptions {
        &self.options
    }
}

/// The contours of values at the points of a triangle mesh: the lines where the values cross
/// chosen levels, the bands between those levels filled from the color scale, or both, with
/// the levels, colors, colorbar and level labels of a contour on a grid
/// ([`ContourMark`](crate::ContourMark)).
///
/// The values are linear within each triangle, between its corners, so a level crosses a
/// triangle in at most one straight piece; a point is high when its value is at or above the
/// level. On a log axis they are linear along each edge as drawn, in the axis's log space, so
/// every crossing lies on the edge between its points on the plot. A NaN at a point removes
/// every triangle around it, which leaves a hole. The automatic levels and the automatic color
/// range come from the values the contour draws, those at the corners of the triangles whose
/// three values are finite, as [`TripcolorMark`] takes its range from the triangles it draws; a
/// point no such triangle uses is still checked, and still counts in the automatic limits.
/// When no triangle is left, the contour draws nothing and maps no color, as a contour on a
/// grid of NaN does.
///
/// Each band above a level is the region where the values are at or above it, made of the
/// parts of triangles that have an area: a triangle whose corners all lie on a level (a flat
/// triangle) is in the band above it, and an edge or a point on the level with lower values all
/// around it, which bounds no area, draws no line. So lines lie exactly where two bands meet;
/// no line has a piece of zero length, and no piece is drawn twice. Lines end on the edge of
/// the mesh and of its holes, and close into loops where they return to their start. The
/// lines and bands depend on the triangles, not on their order or winding. Automatic limits
/// are the extent of the points, without padding.
///
/// # Examples
///
/// ```
/// use umber_core::{ContourLevels, ContourOptions, Figure, Mark, ScaleKind, Theme, Triangles};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut axes = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// let (x, y) = ([0.0, 1.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
/// let options = ContourOptions { levels: ContourLevels::Values(vec![1.5]), ..ContourOptions::default() };
/// axes.tricontour(&x, &y, Triangles::Auto, &[0.0, 1.0, 3.0, f64::NAN], options)?;
/// let Some(Mark::Tricontour(mark)) = axes.marks().first() else { unreachable!() };
/// assert_eq!(mark.mesh().triangles().len(), 2);
/// assert_eq!(mark.levels(), [1.5]);
/// assert!(mark.z()[3].is_nan());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TricontourMark {
    mesh: TriMesh,
    z: Arc<[f64]>,
    drawn: Arc<[f64]>,
    spec: ContourSpec,
}

impl TricontourMark {
    /// Validates the contours of a mesh on the axes at `index` of `figure` before anything is
    /// stored. The checks run in this order, and the first failure is the error:
    ///
    /// 1. both axes are linear or log axes (`Error::Usage`, naming `x` or `y`);
    /// 2. the points and triangles, as [`TriMesh`] checks them (`Error::Data`);
    /// 3. `z` holds one value per point (`Error::Data`);
    /// 4. the values, levels and drawing options, as for a contour on a grid: every value is
    ///    NaN or finite, at most 1e300 in magnitude, and positive and at least 1e-300 on a log
    ///    color scale (`Error::Data`, naming it); then the levels (`Error::Style`), the drawing
    ///    options (`Error::Usage` or `Error::Style`), the center (`Error::Style`) and the color
    ///    mapping (`Error::Usage` and `Error::Style`), as
    ///    [`AxesMut::contour`](super::AxesMut::contour) states them.
    pub(super) fn new(
        figure: &Figure,
        index: usize,
        points: (&[f64], &[f64]),
        triangles: Triangles<'_>,
        z: &[f64],
        options: ContourOptions,
    ) -> Result<TricontourMark> {
        let axes = figure.panel(index);
        let kinds = (axes.x_scale(), axes.y_scale());
        let transforms = check_linear_or_log(TRICONTOUR_CONTEXT, kinds, NEEDS)?;
        let (mesh, _) = TriMesh::new(TRICONTOUR_CONTEXT, transforms, points, triangles)?;
        if z.len() != mesh.x().len() {
            return Err(Error::data(
                TRICONTOUR_CONTEXT,
                format!(
                    "z has {} values; it must have one per point, {}.",
                    z.len(),
                    mesh.x().len()
                ),
            )
            .with_parameter("z")
            .with_hint("pass one value of z per point, in the order of x and y."));
        }
        let drawn: Arc<[f64]> = drawn_values(&mesh, z).into();
        let spec = ContourSpec::new(
            (TRICONTOUR_CONTEXT, figure, index),
            (z, &drawn),
            |i| format!("z[{i}]"),
            options,
        )?;
        Ok(TricontourMark {
            mesh,
            z: Arc::from(z),
            drawn,
            spec,
        })
    }

    /// The points and triangles.
    #[must_use]
    pub fn mesh(&self) -> &TriMesh {
        &self.mesh
    }

    /// The value at each point of [`mesh`](Self::mesh), in its order, as given: NaN removes the
    /// triangles around its point.
    #[must_use]
    pub fn z(&self) -> &[f64] {
        &self.z
    }

    /// The values the contour draws, one per point: those of [`z`](Self::z) at the corners of
    /// the triangles whose three values are finite, NaN at every other point. The automatic
    /// levels and the color mapping come from them.
    pub(crate) fn drawn(&self) -> &[f64] {
        &self.drawn
    }

    /// The levels, increasing: those given, or the round values chosen for the values the
    /// contour draws (none when it draws no triangle or all its values are equal), as for a
    /// contour on a grid.
    #[must_use]
    pub fn levels(&self) -> Vec<f64> {
        self.spec.levels()
    }

    /// The style, colormap, scale, center, color, annotation and format, and the levels as
    /// given.
    #[must_use]
    pub fn options(&self) -> &ContourOptions {
        self.spec.options()
    }

    /// The levels and options, as every contour has them.
    pub(crate) fn spec(&self) -> &ContourSpec {
        &self.spec
    }
}

/// The values of `z` at the corners of the triangles of `mesh` whose three values are finite,
/// one per point, and NaN at every other point: what a contour of `z` on `mesh` draws.
fn drawn_values(mesh: &TriMesh, z: &[f64]) -> Vec<f64> {
    let mut drawn = vec![f64::NAN; z.len()];
    for corners in mesh.triangles() {
        if corners.iter().all(|&i| z[i].is_finite()) {
            for &i in corners {
                drawn[i] = z[i];
            }
        }
    }
    drawn
}

/// Checks that there are `count` values, one per point or per triangle of `mesh` as `on` says
/// (`Error::Data` about `values`).
fn check_length(count: usize, on: TriValues, mesh: &TriMesh) -> Result<()> {
    let (expected, what, hint) = match on {
        TriValues::Points => (
            mesh.x().len(),
            "on=TriValues.POINTS needs one per point",
            "pass one value per point, or on=TriValues.TRIANGLES for one value per triangle.",
        ),
        TriValues::Triangles => (
            mesh.triangles().len(),
            "on=TriValues.TRIANGLES needs one per triangle",
            "pass one value per triangle, in the order of the triangles (or of delaunay(x, y)), \
             or on=TriValues.POINTS for one value per point.",
        ),
    };
    if count == expected {
        return Ok(());
    }
    Err(Error::data(
        TRIPCOLOR_CONTEXT,
        format!("values has {count} entries; {what}: {expected}."),
    )
    .with_parameter("values")
    .with_hint(hint))
}
