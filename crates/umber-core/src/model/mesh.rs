//! Numeric cells and two-dimensional count marks.

use std::sync::Arc;

use super::{Axes, AxisData, CmapChoice, ColorBy, Figure, Mark, Render};
use crate::bin2d::RectBins;
use crate::binning::BinCount;
use crate::color_scale::ColorScaleSpec;
use crate::error::number;
use crate::scale::log_offset;
use crate::{Error, Instant, MAX_BINS, Result, ScaleKind, Transform};

/// Colormap and normalization of numeric cells.
///
/// # Examples
///
/// ```
/// use umber_core::{MeshOptions, Render, Transform};
/// assert_eq!(MeshOptions::default().scale, Transform::Linear);
/// assert_eq!(MeshOptions::default().render, Render::Auto);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MeshOptions {
    /// The colormap; the theme chooses sequential or diverging according to `center`.
    pub cmap: CmapChoice,
    /// Linear or log color normalization.
    pub scale: Transform,
    /// The middle value of a symmetric, linear diverging mapping.
    pub center: Option<f64>,
    /// Whether the cells are drawn as vector shapes or as one image (see [`Render`]); an
    /// image needs edges evenly spaced on screen.
    pub render: Render,
}

impl Default for MeshOptions {
    fn default() -> Self {
        Self {
            cmap: CmapChoice::Theme,
            scale: Transform::Linear,
            center: None,
            render: Render::Auto,
        }
    }
}

/// Equal-width bin counts and their color mapping.
///
/// # Examples
///
/// ```
/// use umber_core::{Hist2dOptions, Render};
/// assert_eq!(Hist2dOptions::default().bins, (30, 30));
/// assert_eq!(Hist2dOptions::default().render, Render::Auto);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Hist2dOptions {
    /// Number of bins along x and y, each in `1..=MAX_BINS`.
    pub bins: (usize, usize),
    /// The colormap.
    pub cmap: CmapChoice,
    /// Linear or log color normalization.
    pub scale: Transform,
    /// Whether the bins are drawn as vector shapes or as one image (see [`Render`]).
    pub render: Render,
}

impl Default for Hist2dOptions {
    fn default() -> Self {
        Self {
            bins: (30, 30),
            cmap: CmapChoice::Theme,
            scale: Transform::Linear,
            render: Render::Auto,
        }
    }
}

/// Hexagonal bin counts and their color mapping.
///
/// # Examples
///
/// ```
/// use umber_core::HexbinOptions;
/// assert_eq!(HexbinOptions::default().gridsize, 30);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct HexbinOptions {
    /// Number of hexagons across the x data range, in `1..=MAX_BINS`.
    pub gridsize: usize,
    /// The colormap.
    pub cmap: CmapChoice,
    /// Linear or log color normalization.
    pub scale: Transform,
}

impl Default for HexbinOptions {
    fn default() -> Self {
        Self {
            gridsize: 30,
            cmap: CmapChoice::Theme,
            scale: Transform::Linear,
        }
    }
}

/// The edges of the cells along one axis: numbers on a linear or log axis, times on a time axis.
/// Edges are finite, or present, and strictly increasing; clones share them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Edges {
    /// Numbers on a linear or log axis.
    Numbers(Arc<[f64]>),
    /// Times on a time axis.
    Times(Arc<[Instant]>),
}

impl Edges {
    /// The number of edges.
    pub(crate) fn len(&self) -> usize {
        match self {
            Edges::Numbers(edges) => edges.len(),
            Edges::Times(edges) => edges.len(),
        }
    }

    /// The numbers; none when these are times.
    pub(crate) fn numbers(&self) -> &[f64] {
        match self {
            Edges::Numbers(edges) => edges,
            Edges::Times(_) => &[],
        }
    }

    /// The times; none when these are numbers.
    pub(crate) fn times(&self) -> &[Instant] {
        match self {
            Edges::Times(edges) => edges,
            Edges::Numbers(_) => &[],
        }
    }
}

/// What a cell axis holds: numbers under a transform, or times.
#[derive(Debug, Clone, Copy)]
enum EdgeAxis {
    /// A linear or log axis.
    Numeric(Transform),
    /// A time axis.
    Time,
}

/// The edges as given, before they are checked against their axis.
#[derive(Debug, Clone, Copy)]
enum GivenEdges<'a> {
    /// Numbers.
    Numbers(&'a [f64]),
    /// Times; `None` is a missing time.
    Times(&'a [Option<Instant>]),
    /// Category names, which no axis of cells takes.
    Categories,
}

impl<'a> From<&'a AxisData> for GivenEdges<'a> {
    fn from(data: &'a AxisData) -> Self {
        match data {
            AxisData::Numbers(edges) => GivenEdges::Numbers(edges),
            AxisData::Times(edges) => GivenEdges::Times(edges),
            AxisData::Categories(_) => GivenEdges::Categories,
        }
    }
}

/// Validated cell edges and row-major values. Clones share their data.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Mark, MeshOptions, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// ax.mesh(&[0.0, 1.0], &[2.0, 4.0], &[5.0], MeshOptions::default())?;
/// assert!(matches!(ax.marks(), [Mark::Mesh(_)]));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MeshMark {
    pub(crate) x_edges: Edges,
    pub(crate) y_edges: Edges,
    pub(crate) colors: ColorBy,
    /// How the cells are drawn, as given.
    pub(crate) render: Render,
    /// Whether the edges along both axes are evenly spaced on screen, so that the cells can be
    /// drawn as one image.
    pub(crate) evenly_spaced: bool,
}

impl MeshMark {
    /// Validates numeric edges and cells before storing anything; both axes must be numeric.
    pub(super) fn new(
        figure: &Figure,
        index: usize,
        x: &[f64],
        y: &[f64],
        values: &[f64],
        options: MeshOptions,
    ) -> Result<Self> {
        let context = "Axes.mesh()";
        let axes = figure.panel(index);
        for (name, kind) in [("x", axes.x_scale()), ("y", axes.y_scale())] {
            if kind == ScaleKind::Time {
                return Err(Error::usage(
                    context,
                    format!(
                        "the {name} axis is a time axis; {context} takes numbers, so pass times with mesh_time()."
                    ),
                )
                .with_parameter(name)
                .with_hint("use mesh_time() for times."));
            }
        }
        let transforms = numeric_axes(context, axes)?;
        Self::checked(
            context,
            (figure, index),
            [
                (EdgeAxis::Numeric(transforms[0]), GivenEdges::Numbers(x)),
                (EdgeAxis::Numeric(transforms[1]), GivenEdges::Numbers(y)),
            ],
            values,
            options,
        )
    }

    /// Validates edges that are numbers or times, each as its axis requires, and cells before
    /// storing anything; neither axis may be a category axis.
    pub(super) fn new_time(
        figure: &Figure,
        index: usize,
        x: &AxisData,
        y: &AxisData,
        values: &[f64],
        options: MeshOptions,
    ) -> Result<Self> {
        let context = "Axes.mesh()";
        let axes = figure.panel(index);
        let mut kinds = [EdgeAxis::Time; 2];
        for (i, (name, kind)) in [("x", axes.x_scale()), ("y", axes.y_scale())]
            .into_iter()
            .enumerate()
        {
            kinds[i] = match (kind, kind.transform()) {
                (ScaleKind::Time, _) => EdgeAxis::Time,
                (_, Some(transform)) => EdgeAxis::Numeric(transform),
                (_, None) => {
                    return Err(Error::usage(
                        context,
                        format!(
                            "the {name} axis is a category axis; this mark needs numeric or time axes."
                        ),
                    )
                    .with_parameter(name)
                    .with_hint(
                        "use Scale.linear(), Scale.log() or Scale.time() for both axes.",
                    ));
                }
            };
        }
        Self::checked(
            context,
            (figure, index),
            [(kinds[0], x.into()), (kinds[1], y.into())],
            values,
            options,
        )
    }

    /// Validates every edge and cell, each axis's edges as its kind says, before storing anything.
    fn checked(
        context: &'static str,
        (figure, index): (&Figure, usize),
        given: [(EdgeAxis, GivenEdges<'_>); 2],
        values: &[f64],
        options: MeshOptions,
    ) -> Result<Self> {
        let [(x_kind, x_given), (y_kind, y_given)] = given;
        let x = check_edges(context, ("x", "x_edges"), x_kind, x_given)?;
        let y = check_edges(context, ("y", "y_edges"), y_kind, y_given)?;
        for (name, kind, edges) in [("x_edges", x_kind, &x), ("y_edges", y_kind, &y)] {
            if let (EdgeAxis::Numeric(transform), Edges::Numbers(edges)) = (kind, edges) {
                check_edge_range(context, name, transform, edges)?;
            }
        }
        let count = (x.len() - 1).checked_mul(y.len() - 1);
        if count != Some(values.len()) {
            return Err(Error::data(
                context,
                format!(
                    "values has {} entries; the edges require {} rows and {} columns.",
                    values.len(),
                    y.len() - 1,
                    x.len() - 1
                ),
            )
            .with_parameter("values")
            .with_hint("pass one row per y interval and one column per x interval."));
        }
        check_numbers(
            context,
            "values",
            values,
            options.scale,
            NumericInput::Cells(x.len() - 1),
        )?;
        let colors = ColorBy {
            values: values.into(),
            cmap: options.cmap,
            scale: options.scale,
            center: options.center,
        };
        check_mapping(context, figure, index, &colors)?;
        let uneven = [("x", x_kind, &x), ("y", y_kind, &y)]
            .into_iter()
            .find_map(|(axis, kind, edges)| uneven_edge(axis, kind, edges));
        if let (Render::Raster, Some(edge)) = (options.render, &uneven) {
            return Err(Error::usage(
                context,
                format!(
                    "{edge}; drawing the cells as one image needs evenly spaced edges on the {} \
                     axis.",
                    edge.axis
                ),
            )
            .with_parameter("render")
            .with_hint("use render=ub.Render.VECTOR, or evenly spaced edges."));
        }
        Ok(Self {
            x_edges: x,
            y_edges: y,
            colors,
            render: options.render,
            evenly_spaced: uneven.is_none(),
        })
    }
}

/// How far, in steps of the lattice, an edge may lie from the evenly spaced lattice from the
/// first edge to the last for the edges to count as evenly spaced.
const LATTICE_TOLERANCE: f64 = 1e-4;

/// The first edge that lies off the evenly spaced lattice from the first edge to the last, as
/// an error message describes it.
#[derive(Debug, Clone, PartialEq)]
struct UnevenEdge {
    /// The axis, `"x"` or `"y"`.
    axis: &'static str,
    /// What the lattice spaces evenly: `"edges"`, `"logarithms"` or `"times"`.
    spacing: &'static str,
    /// The index of the edge, and of the last edge.
    at: (usize, usize),
    /// The edge, the first edge and the last edge, written as in the data.
    values: [String; 3],
    /// Where the lattice puts the edge, written as in the data.
    expected: String,
}

impl std::fmt::Display for UnevenEdge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = format!("{}_edges", self.axis);
        let [edge, first, last] = &self.values;
        write!(
            f,
            "{name}[{}] = {edge} is off the evenly spaced {} from {name}[0] = {first} to \
             {name}[{}] = {last}, which put it at {}",
            self.at.0, self.spacing, self.at.1, self.expected,
        )
    }
}

/// The first of `edges`, on the `axis` axis of kind `kind`, that lies off the evenly spaced
/// lattice the image of the cells uses, from the first edge to the last; `None` when every edge
/// lies on it. Positions are compared in axis space: numbers on a linear axis, their
/// logarithms measured from the first edge ([`log_offset`]) on a log axis, whole microseconds
/// on a time axis. An edge lies on the lattice
/// when it is within [`LATTICE_TOLERANCE`] of a step of its place there; numbers may also be
/// off by two units of rounding error of the largest end, as far as a float can place them.
/// Times are compared exactly, in integers.
fn uneven_edge(axis: &'static str, kind: EdgeAxis, edges: &Edges) -> Option<UnevenEdge> {
    let n = edges.len().checked_sub(1).filter(|&n| n > 1)?;
    let (at, spacing, values, expected) = match (kind, edges) {
        (EdgeAxis::Numeric(transform), Edges::Numbers(v)) => {
            let log = matches!(transform, Transform::Log(_));
            // A log position is measured from the first edge, as the axis measures it, which
            // keeps narrow ranges far from 1 resolvable.
            let position = |x: f64| if log { log_offset(x, v[0]) } else { x };
            let (first, last) = (position(v[0]), position(v[n]));
            let step = (last - first) / n as f64;
            let slack = LATTICE_TOLERANCE * step + 2.0 * f64::EPSILON * first.abs().max(last.abs());
            let place = |i: usize| first + (last - first) * (i as f64 / n as f64);
            let at = (1..n).find(|&i| (position(v[i]) - place(i)).abs() > slack)?;
            let expected = if log {
                // Interpolated in logarithms, which stay within the supported range, and written
                // with 12 significant digits, the precision of logarithms of this size.
                let exact = (v[0].ln() + place(at)).exp();
                format!("{exact:.11e}").parse::<f64>().unwrap_or(exact)
            } else {
                place(at)
            };
            let spacing = if log { "logarithms" } else { "edges" };
            let values = [v[at], v[0], v[n]].map(|x| number(x).to_string());
            (at, spacing, values, number(expected).to_string())
        }
        (EdgeAxis::Time, Edges::Times(t)) => {
            let base = i128::from(t[0].micros());
            let span = i128::from(t[n].micros()) - base;
            let count = n as i128;
            // `n` times the distance of edge `i` from its place on the lattice, in microseconds,
            // exactly: within the tolerance when at most `LATTICE_TOLERANCE` of the span.
            let off = |i: usize| (i128::from(t[i].micros()) - base) * count - span * i as i128;
            let at =
                (1..n).find(|&i| off(i).unsigned_abs() as f64 > LATTICE_TOLERANCE * span as f64)?;
            // The place in whole microseconds, and the fraction of one beyond it.
            let exact = base * count + span * at as i128;
            let (whole, rest) = (exact.div_euclid(count), exact.rem_euclid(count));
            let label = Instant::clamped(i64::try_from(whole).unwrap_or(i64::MAX)).label();
            let expected = if rest == 0 {
                label
            } else {
                format!(
                    "{label} plus {} microseconds",
                    number(rest as f64 / count as f64)
                )
            };
            let values = [t[at], t[0], t[n]].map(Instant::label);
            (at, "times", values, expected)
        }
        // `check_edges` matches the edges to their axis.
        _ => return None,
    };
    Some(UnevenEdge {
        axis,
        spacing,
        at: (at, n),
        values,
        expected,
    })
}

/// The edges `given` along the axis `axis` of kind `kind`, whose argument is `name`: of the
/// kind the axis holds, at least two, finite or present, and strictly increasing.
fn check_edges(
    context: &'static str,
    (axis, name): (&'static str, &'static str),
    kind: EdgeAxis,
    given: GivenEdges<'_>,
) -> Result<Edges> {
    let mismatch = |holds: &str, expected: &str, hint: &'static str| {
        Error::data(
            context,
            format!("{name} holds {holds} but the {axis} axis {expected}."),
        )
        .with_parameter(name)
        .with_hint(hint)
    };
    let too_few = |length: usize| {
        (length < 2).then(|| {
            Error::data(
                context,
                format!("{name} has {length} values; at least two edges are required."),
            )
            .with_parameter(name)
            .with_hint("pass one more edge than the number of cells.")
        })
    };
    match (kind, given) {
        (EdgeAxis::Numeric(transform), GivenEdges::Numbers(edges)) => {
            if let Some(error) = too_few(edges.len()) {
                return Err(error);
            }
            check_numbers(context, name, edges, transform, NumericInput::Edges)?;
            for (i, &v) in edges.iter().enumerate() {
                if !v.is_finite() || (i > 0 && v <= edges[i - 1]) {
                    return Err(Error::data(
                        context,
                        format!(
                            "{name}[{i}] is {}; edges must be strictly increasing.",
                            number(v)
                        ),
                    )
                    .with_parameter(name)
                    .with_hint("pass finite edges in strictly increasing order."));
                }
            }
            Ok(Edges::Numbers(edges.into()))
        }
        (EdgeAxis::Time, GivenEdges::Times(edges)) => {
            if let Some(error) = too_few(edges.len()) {
                return Err(error);
            }
            let mut times: Vec<Instant> = Vec::with_capacity(edges.len());
            for (i, &edge) in edges.iter().enumerate() {
                let Some(t) = edge else {
                    return Err(Error::data(
                        context,
                        format!("{name}[{i}] is NaT; edges must be times."),
                    )
                    .with_parameter(name)
                    .with_hint("pass a time for every edge."));
                };
                if let Some(&previous) = times.last()
                    && t <= previous
                {
                    return Err(Error::data(
                        context,
                        format!(
                            "{name}[{i}] is {}, not after {name}[{}] = {}; edges must be strictly increasing.",
                            t.label(),
                            i - 1,
                            previous.label()
                        ),
                    )
                    .with_parameter(name)
                    .with_hint("pass edges in strictly increasing order."));
                }
                times.push(t);
            }
            Ok(Edges::Times(times.into()))
        }
        (EdgeAxis::Numeric(_), GivenEdges::Times(_)) => Err(mismatch(
            "times",
            "is not a time axis; pass numbers",
            "pass numbers, or use Scale.time() for times.",
        )),
        (EdgeAxis::Time, GivenEdges::Numbers(_)) => Err(mismatch(
            "numbers",
            "is a time axis; pass times",
            "pass times, or use a linear or log axis for numbers.",
        )),
        (_, GivenEdges::Categories) => Err(mismatch(
            "category names",
            "takes numbers or times; pass one of them",
            "pass numbers on a linear or log axis and times on a time axis.",
        )),
    }
}

/// Numeric edges must span a range the axis scale can hold.
fn check_edge_range(
    context: &'static str,
    name: &'static str,
    transform: Transform,
    edges: &[f64],
) -> Result<()> {
    let (first, last) = (edges[0], edges[edges.len() - 1]);
    crate::ContinuousScale::new(transform, first, last, context)
        .map(drop)
        .map_err(|_| {
            Error::data(
                context,
                format!(
                    "{name} spans {} to {}; {}.",
                    number(first),
                    number(last),
                    match transform {
                        Transform::Linear => {
                            "the range must be at least 1e-10 of their magnitude \
                             (and at least 1e-290)"
                        }
                        Transform::Log(_) => {
                            "the ratio of the last edge to the first \
                             must be at least 1 + 1e-9"
                        }
                    },
                ),
            )
            .with_parameter(name)
            .with_hint("rescale the coordinates or use a wider range of edges.")
        })
}

/// Sparse two-dimensional histogram counts. Empty bins are not stored.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Hist2dOptions, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// ax.hist2d(&[0.0, 1.0], &[0.0, 1.0], Hist2dOptions::default())?;
/// assert!(matches!(ax.marks(), [Mark::Hist2d(_)]));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Hist2dMark {
    pub(crate) bins: Arc<RectBins>,
    pub(crate) colors: ColorScaleSpec,
    /// How the bins are drawn, as given.
    pub(crate) render: Render,
}

impl Hist2dMark {
    /// Validates the coordinates and options, then bins paired finite points once.
    pub(super) fn new(
        figure: &Figure,
        index: usize,
        x: &[f64],
        y: &[f64],
        options: Hist2dOptions,
    ) -> Result<Self> {
        let context = "Axes.hist2d()";
        let transforms = numeric_axes(context, figure.panel(index))?;
        let bins = [
            bin_count(context, "bins", options.bins.0)?,
            bin_count(context, "bins", options.bins.1)?,
        ];
        check_points(context, x, y, transforms)?;
        let colors = ColorBy {
            cmap: options.cmap,
            scale: options.scale,
            ..ColorBy::new(Vec::new())
        };
        check_mapping(context, figure, index, &colors)?;
        Ok(Self {
            bins: Arc::new(RectBins::new(x, y, bins, transforms)),
            colors: ColorScaleSpec::of(&colors),
            render: options.render,
        })
    }
}

/// Coordinates counted in regular, pointy-top hexagons at the final plot aspect.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, HexbinOptions, Mark, ScaleKind, Theme};
/// let mut f = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// ax.hexbin(&[0.0, 1.0], &[0.0, 1.0], HexbinOptions::default())?;
/// assert!(matches!(ax.marks(), [Mark::Hexbin(_)]));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct HexbinMark {
    pub(crate) x: Arc<[f64]>,
    pub(crate) y: Arc<[f64]>,
    pub(crate) gridsize: BinCount,
    pub(crate) transforms: [Transform; 2],
    pub(crate) extents: [Vec<f64>; 2],
    pub(crate) colors: ColorScaleSpec,
}

impl HexbinMark {
    /// Validates and stores paired coordinates; layout determines the hexagon row spacing.
    pub(super) fn new(
        figure: &Figure,
        index: usize,
        x: &[f64],
        y: &[f64],
        options: HexbinOptions,
    ) -> Result<Self> {
        let context = "Axes.hexbin()";
        let transforms = numeric_axes(context, figure.panel(index))?;
        let gridsize = bin_count(context, "gridsize", options.gridsize)?;
        check_points(context, x, y, transforms)?;
        let colors = ColorBy {
            cmap: options.cmap,
            scale: options.scale,
            ..ColorBy::new(Vec::new())
        };
        check_mapping(context, figure, index, &colors)?;
        Ok(Self {
            x: x.into(),
            y: y.into(),
            gridsize,
            transforms,
            extents: crate::bin2d::ranges(x, y, transforms).map_or_else(
                || [Vec::new(), Vec::new()],
                |ranges| ranges.map(|(lo, hi)| vec![lo, hi]),
            ),
            colors: ColorScaleSpec::of(&colors),
        })
    }
}

/// Both axes must be numeric, before any coordinate is converted or stored.
pub(super) fn numeric_axes(context: &'static str, axes: &Axes) -> Result<[Transform; 2]> {
    let mut transforms = [Transform::Linear; 2];
    for (i, (name, kind)) in [("x", axes.x_scale()), ("y", axes.y_scale())]
        .into_iter()
        .enumerate()
    {
        transforms[i] = kind.transform().ok_or_else(|| {
            Error::usage(
                context,
                format!("the {name} axis is not numeric; this mark needs two numeric axes."),
            )
            .with_parameter(name)
            .with_hint("use Scale.linear() or Scale.log() for both axes.")
        })?;
    }
    Ok(transforms)
}

/// A bounded, nonzero cell count.
fn bin_count(context: &'static str, name: &'static str, n: usize) -> Result<BinCount> {
    i64::try_from(n)
        .ok()
        .and_then(BinCount::new)
        .ok_or_else(|| {
            Error::style(
                context,
                format!("{name} is {n}; it must be between 1 and {MAX_BINS}."),
            )
            .with_parameter(name)
            .with_hint("choose a positive count no greater than MAX_BINS.")
        })
}

/// Numeric inputs differ in their indices and in whether a NaN denotes a gap.
#[derive(Clone, Copy)]
enum NumericInput {
    Edges,
    Coordinates,
    Cells(usize),
}

/// Checks numbers in their own coordinate or matrix context, before storing a mark.
fn check_numbers(
    context: &'static str,
    name: &'static str,
    values: &[f64],
    scale: Transform,
    input: NumericInput,
) -> Result<()> {
    let (noun, hint) = match input {
        NumericInput::Edges => ("edges", "pass finite edges in strictly increasing order."),
        NumericInput::Coordinates => (
            "coordinates",
            "remove invalid coordinates, or use NaN to skip a point.",
        ),
        NumericInput::Cells(_) => ("values", "rescale the values, or use NaN for blank cells."),
    };
    for (i, &v) in values.iter().enumerate() {
        if v.is_nan() && !matches!(input, NumericInput::Edges) {
            continue;
        }
        let required = if !v.is_finite() {
            format!("{noun} must be finite")
        } else if v.abs() > crate::MAX_MAGNITUDE {
            format!(
                "{noun} must not exceed {:e} in magnitude",
                crate::MAX_MAGNITUDE
            )
        } else if matches!(scale, Transform::Log(_)) && v <= 0.0 {
            format!("{noun} must be positive on a log scale")
        } else if matches!(scale, Transform::Log(_)) && v < crate::MIN_LOG_VALUE {
            format!(
                "{noun} must be at least {:e} on a log scale",
                crate::MIN_LOG_VALUE
            )
        } else {
            continue;
        };
        let hint = if v.is_finite() && matches!(scale, Transform::Log(_)) && v <= 0.0 {
            match input {
                NumericInput::Edges => "use a linear axis, or pass positive edges.",
                NumericInput::Coordinates => {
                    "use a linear axis, or replace non-positive coordinates with NaN."
                }
                NumericInput::Cells(_) => "use a linear color scale, or NaN for blank cells.",
            }
        } else if v.is_finite() {
            match input {
                NumericInput::Edges => "rescale the edges to the supported numeric range.",
                NumericInput::Coordinates => {
                    "rescale the coordinates to the supported numeric range."
                }
                NumericInput::Cells(_) => "rescale the values to the supported numeric range.",
            }
        } else {
            hint
        };
        let index = match input {
            NumericInput::Cells(columns) => format!("{}, {}", i / columns, i % columns),
            _ => i.to_string(),
        };
        return Err(Error::data(
            context,
            format!("{name}[{index}] is {}; {required}.", number(v)),
        )
        .with_parameter(name)
        .with_hint(hint));
    }
    Ok(())
}

/// Carries color-option validation errors back to the mark call.
fn relocate(mut error: Error, context: &'static str) -> Error {
    if let Error::Data { context: c, .. } | Error::Style { context: c, .. } = &mut error {
        *c = context;
    }
    error
}

/// Equal-length numeric coordinates; only pairs with two finite coordinates are binned.
fn check_points(
    context: &'static str,
    x: &[f64],
    y: &[f64],
    transforms: [Transform; 2],
) -> Result<()> {
    if x.len() != y.len() {
        return Err(Error::data(
            context,
            format!(
                "x has {} values but y has {}; x and y must have the same length.",
                x.len(),
                y.len()
            ),
        )
        .with_parameter("y")
        .with_hint("pass one x and one y value per point."));
    }
    check_numbers(context, "x", x, transforms[0], NumericInput::Coordinates)?;
    check_numbers(context, "y", y, transforms[1], NumericInput::Coordinates)
}

/// A mark must use the axes' and shared group's colormap, transform, center and color limits.
pub(super) fn check_mapping(
    context: &'static str,
    figure: &Figure,
    index: usize,
    colors: &ColorBy,
) -> Result<()> {
    colors.check().map_err(|error| relocate(error, context))?;
    let axes = figure.panel(index);
    let new = ColorScaleSpec::of(colors);
    if let Some(existing) = axes.marks.iter().find_map(Mark::color_scale) {
        let parameter = if new.cmap != existing.cmap {
            "cmap"
        } else if new.scale != existing.scale {
            "scale"
        } else {
            "center"
        };
        new.check_same(&existing, context, parameter)?;
    }
    if let Some(limits) = axes.color_limits {
        new.check_range(
            limits,
            context,
            Some(if new.center.is_some() {
                "center"
            } else {
                "scale"
            }),
        )?;
    }
    figure.check_shared_color(index, &new, context, None)
}

#[cfg(test)]
mod tests {
    use crate::pipeline::plan_figure;
    use crate::{
        Figure, FontLibrary, HexbinOptions, Hist2dOptions, Item, MeshOptions, PathVerb, Point,
        ScaleKind, Theme,
    };

    fn figure(width: f64, height: f64) -> Figure {
        let mut f = Figure::new(width, height, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        f
    }

    fn cells(f: &Figure) -> Vec<Vec<Point>> {
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        crate::test_support::scene::assert_scene_bounded(&scene);
        scene
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Group {
                    clip: Some(_),
                    items,
                } => Some(items),
                _ => None,
            })
            .flatten()
            .filter_map(|item| match item {
                Item::Fill { path, .. } => Some(
                    path.verbs()
                        .iter()
                        .filter_map(|verb| match verb {
                            PathVerb::MoveTo(p) | PathVerb::LineTo(p) => Some(*p),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .collect()
    }

    fn dense_points(normal: bool, offset: f64) -> (Vec<f64>, Vec<f64>) {
        (0..2000)
            .map(|i| {
                let u = (i as f64 * 0.618_033_988_7).fract();
                let v = (i as f64 * 0.414_213_562_3).fract();
                if normal {
                    let radius = (-2.0 * (0.001 + 0.998 * u).ln()).sqrt();
                    let angle = std::f64::consts::TAU * v;
                    (radius * angle.cos() + offset, radius * angle.sin())
                } else {
                    let x = u * 4.0 - 2.0;
                    (x + offset, v * 4.0 - 2.0 + 0.5 * x + offset)
                }
            })
            .unzip()
    }

    use crate::test_support::polygon::{
        Pt, area, clip_to_rectangle, covers, edge_distance, inner_samples, inside, outline_samples,
        simple,
    };

    /// How far, in points, a checked point may lie off the outline it is checked against: the
    /// scene's coordinates are `f32`.
    const TOLERANCE: f64 = 2e-3;

    /// How much, in square points, a measured area may differ from its bound.
    const AREA_TOLERANCE: f64 = 1e-2;

    /// Where a cell's fill may reach under the later cells, `later` (their exact hexagons),
    /// across the edges of its exact `hexagon`: each such cell with the edge and the depth a
    /// heatmap cell would reach, 1 pt or half that cell when it is smaller.
    fn strips<'a>(hexagon: &[Pt; 6], later: &'a [[Pt; 6]]) -> Vec<(&'a [Pt; 6], Pt, Pt, f64)> {
        let center = (
            hexagon.iter().map(|p| p.0).sum::<f64>() / 6.0,
            hexagon.iter().map(|p| p.1).sum::<f64>() / 6.0,
        );
        let same = |p: Pt, q: Pt| (p.0 - q.0).hypot(p.1 - q.1) < 1e-3;
        let mut out = Vec::new();
        for i in 0..6 {
            let (a, b) = (hexagon[i], hexagon[(i + 1) % 6]);
            let depth = line_distance(center, a, b).min(1.0);
            out.extend(
                later
                    .iter()
                    .filter(|n| n.iter().any(|&q| same(q, a)) && n.iter().any(|&q| same(q, b)))
                    .map(|n| (n, a, b, depth)),
            );
        }
        out
    }

    /// The distance from `p` to the line through `a` and `b`.
    fn line_distance(p: Pt, a: Pt, b: Pt) -> f64 {
        ((b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1)).abs() / (b.0 - a.0).hypot(b.1 - a.1)
    }

    /// Checks the drawn `fill` of the cell whose exact hexagon is `hexagon`, inside the plot
    /// `plot` (left, top, right, bottom), given the exact hexagons of the cells whose fills
    /// come after it, `later`. Where the fill may lie is the hexagon and the [`strips`] under
    /// later cells, within the plot. A whole cell's fill must be a simple outline covering its
    /// whole hexagon and lying where it may, so its area is at least the hexagon's and at most
    /// that plus the strips'. A clipped cell's fill (clipping can leave edges of no area along
    /// the plot's edge) must cover the visible part of the hexagon, every point inside it must
    /// lie where it may, and its area must stay within those same bounds for the visible part.
    fn check_fill(
        fill: &[Pt],
        hexagon: &[Pt; 6],
        later: &[[Pt; 6]],
        plot: [f64; 4],
        whole: bool,
    ) -> Result<(), String> {
        let strips = strips(hexagon, later);
        let allowed = |p: Pt| {
            p.0 >= plot[0] - TOLERANCE
                && p.0 <= plot[2] + TOLERANCE
                && p.1 >= plot[1] - TOLERANCE
                && p.1 <= plot[3] + TOLERANCE
                && (covers(hexagon, p, TOLERANCE)
                    || strips.iter().any(|&(n, a, b, depth)| {
                        covers(n, p, TOLERANCE) && line_distance(p, a, b) <= depth + TOLERANCE
                    }))
        };
        let visible = clip_to_rectangle(hexagon, plot);
        let strip_area: f64 = strips
            .iter()
            .map(|&(_, a, b, depth)| depth * ((b.0 - a.0).hypot(b.1 - a.1) + 2.0 * depth))
            .sum();
        let filled = area(fill);
        if filled < area(&visible) - AREA_TOLERANCE
            || filled > area(&visible) + strip_area + AREA_TOLERANCE
        {
            return Err(format!(
                "{fill:?} fills {filled} pt², not at least the {} pt² of {hexagon:?} on the plot \
                 and at most {strip_area} pt² more",
                area(&visible)
            ));
        }
        if let Some(p) = outline_samples(&visible, 0.25)
            .into_iter()
            .find(|&p| !covers(fill, p, TOLERANCE))
        {
            return Err(format!("{fill:?} leaves {p:?} of {hexagon:?} uncovered"));
        }
        let stray = if whole {
            if !simple(fill, 1e-4) {
                return Err(format!("{fill:?} crosses or retraces itself"));
            }
            if let Some(p) = fill
                .iter()
                .find(|&&p| inside(hexagon, p) && edge_distance(hexagon, p) > TOLERANCE)
            {
                return Err(format!("{fill:?} has a corner {p:?} inside {hexagon:?}"));
            }
            // A simple outline lies where its edges lie.
            outline_samples(fill, 0.25)
                .into_iter()
                .find(|&p| !allowed(p))
        } else {
            inner_samples(fill, 0.25).into_iter().find(|&p| !allowed(p))
        };
        match stray {
            Some(p) => Err(format!(
                "{fill:?} reaches {p:?}, past {hexagon:?} and the reach under its later cells"
            )),
            None => Ok(()),
        }
    }

    /// [`check_fill`] for a whole cell, its fill drawn by itself on a plot that holds it all.
    fn check_outline(outline: &[Pt], hexagon: &[Pt; 6], later: &[[Pt; 6]]) -> Result<(), String> {
        let plot = [f64::MIN, f64::MIN, f64::MAX, f64::MAX];
        check_fill(outline, hexagon, later, plot, true)
    }

    /// One drawn hexbin cell: its exact hexagon, from the lattice mapped onto the plot as the
    /// axes map it, and whether it lies wholly inside the plot.
    struct HexCell {
        hexagon: [Pt; 6],
        whole: bool,
    }

    /// The hexbin cells of one panel, in drawing order, and the fills drawn for them.
    struct HexPanel {
        plot: crate::layout::Area,
        cells: Vec<HexCell>,
        fills: Vec<Vec<Pt>>,
    }

    /// The hexbin cells and fills of every panel of `f`.
    fn hex_panels(f: &Figure) -> Vec<HexPanel> {
        let mut out = Vec::new();
        for (resolved, layout, marks) in crate::pipeline::plan_panel_marks(f, FontLibrary::global())
        {
            let plot = layout.plot;
            let (xs, ys) = (resolved.x.continuous_scale(), resolved.y.continuous_scale());
            let mut panel = HexPanel {
                plot,
                cells: Vec::new(),
                fills: Vec::new(),
            };
            for mark in &marks {
                let crate::resolve::ResolvedMark::Hexbin {
                    bins: crate::bin2d::HexBins::Occupied(bins),
                    ..
                } = mark
                else {
                    continue;
                };
                let scale = resolved.color.as_ref().expect("a color scale");
                for (&position, &count) in bins.positions.iter().zip(&bins.counts) {
                    if scale.color(count).is_none() {
                        continue;
                    }
                    let hexagon = bins.vertices(position).map(|[x, y]| {
                        (
                            plot.x + xs.normalize(x) * plot.width,
                            plot.bottom() - ys.normalize(y) * plot.height,
                        )
                    });
                    let whole = hexagon.iter().all(|p| {
                        p.0 >= plot.x - 1e-3
                            && p.0 <= plot.right() + 1e-3
                            && p.1 >= plot.y - 1e-3
                            && p.1 <= plot.bottom() + 1e-3
                    });
                    panel.cells.push(HexCell { hexagon, whole });
                }
                if let Some(Item::Group { items, .. }) = crate::draw::marks(
                    f.theme(),
                    &crate::test_support::figure_fonts(f.theme()),
                    &resolved,
                    &layout,
                    std::slice::from_ref(mark),
                )
                .unwrap()
                {
                    panel.fills.extend(items.iter().map(|item| {
                        match item {
                            Item::Fill { path, .. } => path
                                .verbs()
                                .iter()
                                .filter_map(|verb| match verb {
                                    PathVerb::MoveTo(p) | PathVerb::LineTo(p) => {
                                        Some((f64::from(p.x), f64::from(p.y)))
                                    }
                                    _ => None,
                                })
                                .collect(),
                            other => panic!("expected a hexagon fill, got {other:?}"),
                        }
                    }));
                }
            }
            out.push(panel);
        }
        out
    }

    /// Checks every fill of `panel`. Each cell that shows on the plot owns the fill that holds
    /// the center of its visible part and covers its corners, the first such fill left when the
    /// cells are taken in order; each fill is checked by [`check_fill`] against the cells whose
    /// fills come after it. Every other fill must pass as the fill of a cell that does not show.
    fn check_panel(panel: &HexPanel) -> Result<(), String> {
        let p = panel.plot;
        let plot = [p.x, p.y, p.right(), p.bottom()];
        let mut owner: Vec<Option<usize>> = vec![None; panel.fills.len()];
        let mut fill_of = vec![None; panel.cells.len()];
        for (i, cell) in panel.cells.iter().enumerate() {
            let visible = clip_to_rectangle(&cell.hexagon, plot);
            if area(&visible) <= AREA_TOLERANCE {
                continue;
            }
            let center = (
                visible.iter().map(|p| p.0).sum::<f64>() / visible.len() as f64,
                visible.iter().map(|p| p.1).sum::<f64>() / visible.len() as f64,
            );
            let own = |(f, fill): &(usize, &Vec<Pt>)| {
                owner[*f].is_none()
                    && inside(fill, center)
                    && visible.iter().all(|&p| covers(fill, p, TOLERANCE))
            };
            let Some((f, _)) = panel.fills.iter().enumerate().find(own) else {
                return Err(format!("no fill covers the cell {:?}", cell.hexagon));
            };
            owner[f] = Some(i);
            fill_of[i] = Some(f);
        }
        let later = |f: usize| -> Vec<[Pt; 6]> {
            panel
                .cells
                .iter()
                .zip(&fill_of)
                .filter(|(_, other)| other.is_some_and(|other| other > f))
                .map(|(cell, _)| cell.hexagon)
                .collect()
        };
        for (i, cell) in panel.cells.iter().enumerate() {
            if let Some(f) = fill_of[i] {
                check_fill(&panel.fills[f], &cell.hexagon, &later(f), plot, cell.whole)?;
            }
        }
        // A fill left over is what shows of a cell off the plot: its reach under a later cell.
        for (f, _) in owner
            .iter()
            .enumerate()
            .filter(|(_, owner)| owner.is_none())
        {
            let fits = panel.cells.iter().zip(&fill_of).any(|(cell, own)| {
                own.is_none()
                    && check_fill(&panel.fills[f], &cell.hexagon, &later(f), plot, false).is_ok()
            });
            if !fits {
                return Err(format!("no cell owns the fill {:?}", panel.fills[f]));
            }
        }
        Ok(())
    }

    /// Whether the sides of `hexagon` are equal.
    fn regular(hexagon: &[Pt; 6]) -> bool {
        let sides: Vec<f64> = (0..6)
            .map(|i| {
                let (a, b) = (hexagon[i], hexagon[(i + 1) % 6]);
                (a.0 - b.0).hypot(a.1 - b.1)
            })
            .collect();
        sides[0] > 0.0 && sides.iter().all(|side| (side - sides[0]).abs() < 1e-4)
    }

    #[test]
    fn outline_check_rejects_notches_and_deep_reaches() {
        let s = 10.0 * 3.0_f64.sqrt() / 2.0;
        let hexagon = [
            (0.0, 10.0),
            (s, 5.0),
            (s, -5.0),
            (0.0, -10.0),
            (-s, -5.0),
            (-s, 5.0),
        ];
        assert_eq!(check_outline(&hexagon, &hexagon, &[]), Ok(()));
        // A corner halfway along the left edge, half a point in or out: the outline leaves part
        // of the hexagon uncovered, or reaches past it where no later cell lies.
        for offset in [0.5, -0.5] {
            let mut notched = hexagon.to_vec();
            notched.insert(5, (-s + offset, 0.0));
            assert!(check_outline(&notched, &hexagon, &[]).is_err(), "{offset}");
        }
        // Under the cell to the right, drawn later, the right edge may reach 1 pt, not more.
        let right = hexagon.map(|(x, y)| (x + 2.0 * s, y));
        for (reach, ok) in [(1.0, true), (1.5, false)] {
            let pushed = [
                hexagon[0],
                hexagon[1],
                (s + reach, 5.0 + reach / 3.0_f64.sqrt()),
                (s + reach, -5.0 - reach / 3.0_f64.sqrt()),
                hexagon[2],
                hexagon[3],
                hexagon[4],
                hexagon[5],
            ];
            assert_eq!(
                check_outline(&pushed, &hexagon, &[right]).is_ok(),
                ok,
                "{reach}"
            );
            // Without that cell drawn after it, no reach is allowed.
            assert!(check_outline(&pushed, &hexagon, &[]).is_err());
        }
    }

    #[test]
    fn outline_check_rejects_outlines_that_retrace_themselves() {
        let s = 10.0 * 3.0_f64.sqrt() / 2.0;
        let h = [
            (0.0, 10.0),
            (s, 5.0),
            (s, -5.0),
            (0.0, -10.0),
            (-s, -5.0),
            (-s, 5.0),
        ];
        // The hexagon traced forward and back covers its outline and fills nothing; with the
        // triangle of every other corner added, it fills only that triangle.
        let mut retraced = h.to_vec();
        retraced.push(h[0]);
        retraced.extend(h[1..].iter().rev());
        let mut triangle = retraced.clone();
        triangle.extend([h[0], h[2], h[4]]);
        let plot = crate::layout::Area {
            x: -30.0,
            y: -30.0,
            width: 80.0,
            height: 60.0,
        };
        for fill in [retraced, triangle] {
            assert!(check_outline(&fill, &h, &[]).is_err(), "{fill:?}");
            let panel = HexPanel {
                plot,
                cells: vec![HexCell {
                    hexagon: h,
                    whole: true,
                }],
                fills: vec![fill],
            };
            assert!(check_panel(&panel).is_err());
        }
    }

    #[test]
    fn panel_check_takes_later_cells_from_the_fill_order() {
        let s = 10.0 * 3.0_f64.sqrt() / 2.0;
        let h = [
            (0.0, 10.0),
            (s, 5.0),
            (s, -5.0),
            (0.0, -10.0),
            (-s, -5.0),
            (-s, 5.0),
        ];
        let right = h.map(|(x, y)| (x + 2.0 * s, y));
        let lift = 1.0 / 3.0_f64.sqrt();
        let pushed = vec![
            h[0],
            h[1],
            (s + 1.0, 5.0 + lift),
            (s + 1.0, -5.0 - lift),
            h[2],
            h[3],
            h[4],
            h[5],
        ];
        let panel = |fills| HexPanel {
            plot: crate::layout::Area {
                x: -30.0,
                y: -30.0,
                width: 80.0,
                height: 60.0,
            },
            cells: vec![
                HexCell {
                    hexagon: h,
                    whole: true,
                },
                HexCell {
                    hexagon: right,
                    whole: true,
                },
            ],
            fills,
        };
        // The left cell reaches under the right one only when the right one is drawn after it.
        assert_eq!(
            check_panel(&panel(vec![pushed.clone(), right.to_vec()])),
            Ok(())
        );
        assert!(check_panel(&panel(vec![right.to_vec(), pushed])).is_err());
    }

    #[test]
    fn panel_check_bounds_the_area_of_clipped_fills() {
        // A whole hexagon at (20, 20) and one cut in half by the left edge of a 40 pt plot.
        let s = 10.0 * 3.0_f64.sqrt() / 2.0;
        let h = [
            (0.0, -10.0),
            (s, -5.0),
            (s, 5.0),
            (0.0, 10.0),
            (-s, 5.0),
            (-s, -5.0),
        ];
        let whole = h.map(|(x, y)| (x + 20.0, y + 20.0));
        let clipped = h.map(|(x, y)| (x, y + 20.0));
        let panel = |fill| HexPanel {
            plot: crate::layout::Area {
                x: 0.0,
                y: 0.0,
                width: 40.0,
                height: 40.0,
            },
            cells: vec![
                HexCell {
                    hexagon: whole,
                    whole: true,
                },
                HexCell {
                    hexagon: clipped,
                    whole: false,
                },
            ],
            fills: vec![whole.to_vec(), fill],
        };
        let half = vec![(0.0, 10.0), (s, 15.0), (s, 25.0), (0.0, 30.0)];
        assert_eq!(check_panel(&panel(half.clone())), Ok(()));
        // An edge of no area along the plot's edge, as clipping can leave, is accepted.
        let mut bridge = half;
        bridge.push((0.0, 40.0));
        assert_eq!(check_panel(&panel(bridge)), Ok(()));
        // The whole plot is not what shows of the clipped cell.
        let plot = vec![(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)];
        assert!(check_panel(&panel(plot)).is_err());
    }

    fn assert_regular_whole(f: &Figure) {
        let panels = hex_panels(f);
        let cells: usize = panels.iter().map(|panel| panel.cells.len()).sum();
        assert!(cells >= 10, "{cells} cells");
        for panel in &panels {
            for cell in &panel.cells {
                assert!(cell.whole, "clipped hexagon: {:?}", cell.hexagon);
                assert!(regular(&cell.hexagon), "{:?}", cell.hexagon);
            }
            check_panel(panel).unwrap();
            assert_eq!(panel.fills.len(), panel.cells.len());
        }
    }

    fn assert_hexbin_points_covered(limits: [bool; 2], columns: i64) {
        let mut failures = Vec::new();
        for (normal, width, height, gridsize, ylim) in [
            (true, 6.0, 4.0, 30, (0.0, 1.0)),
            (true, 6.0, 4.0, 30, (-1.0, 1.0)),
            (true, 4.0, 7.0, 100, (0.0, 2.0)),
            (false, 6.0, 4.0, 10, (-1.0, 1.0)),
            (false, 6.0, 4.0, 30, (-1.0, 1.0)),
        ] {
            let grid = crate::Grid::new(1, columns)
                .unwrap()
                .with_share_x(crate::Share::All)
                .with_share_y(crate::Share::All);
            let mut f =
                Figure::with_grid(width * columns as f64, height, Theme::pigment(), grid).unwrap();
            let (mut x, mut y) = dense_points(normal, 0.0);
            x.extend([f64::NAN, 0.0]);
            y.extend([0.0, f64::NAN]);
            for col in 0..columns {
                let mut ax = f
                    .add_axes_at(
                        crate::Placement {
                            col: crate::GridIndex::At(col),
                            ..Default::default()
                        },
                        ScaleKind::Linear,
                        ScaleKind::Linear,
                    )
                    .unwrap();
                if limits[0] {
                    ax.xlim(-1.0, 1.0).unwrap();
                }
                if limits[1] {
                    ax.ylim(ylim.0, ylim.1).unwrap();
                }
                ax.hexbin(
                    &x,
                    &y,
                    HexbinOptions {
                        gridsize,
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            crate::bin2d::COUNT_CALLS.with(|calls| calls.set(0));
            cells(&f);
            let count_calls = crate::bin2d::COUNT_CALLS.with(|calls| calls.get());
            let panels = hex_panels(&f);
            for col in 0..columns {
                let (resolved, layout) = plan_figure(&f, (0, col as usize), FontLibrary::global());
                let plot = layout.plot;
                let panel = &panels[col as usize];
                let xs = resolved.x.continuous_scale();
                let ys = resolved.y.continuous_scale();
                let mut in_window = 0;
                let mut uncovered = 0;
                for (&x, &y) in x
                    .iter()
                    .zip(&y)
                    .filter(|(x, y)| x.is_finite() && y.is_finite())
                {
                    let x = xs.normalize(x);
                    let y = ys.normalize(y);
                    if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
                        continue;
                    }
                    in_window += 1;
                    let x = plot.x + x * plot.width;
                    let y = plot.bottom() - y * plot.height;
                    if !panel
                        .fills
                        .iter()
                        .any(|fill| fill.len() >= 3 && covers(fill, (x, y), 1e-4))
                    {
                        uncovered += 1;
                    }
                }
                let whole: Vec<_> = panel.cells.iter().filter(|cell| cell.whole).collect();
                assert!(in_window > 0);
                assert!(!whole.is_empty());
                let irregular = whole.iter().filter(|cell| !regular(&cell.hexagon)).count();
                let outlines = check_panel(panel).err().unwrap_or_default();
                let result = format!(
                    "limits={limits:?} columns={columns} col={col} normal={normal} \
                     {width}x{height} g{gridsize} ylim={ylim:?}: \
                     uncovered={uncovered}/{in_window}, irregular={irregular}/{}, \
                     count_calls={count_calls} {outlines}",
                    whole.len(),
                );
                eprintln!("{result}");
                if uncovered != 0 || irregular != 0 || !outlines.is_empty() {
                    failures.push(result);
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn hexbin_points_covered_without_limits() {
        assert_hexbin_points_covered([false, false], 1);
    }

    #[test]
    fn hexbin_points_covered_with_xlim() {
        assert_hexbin_points_covered([true, false], 1);
    }

    #[test]
    fn hexbin_points_covered_with_ylim() {
        assert_hexbin_points_covered([false, true], 1);
    }

    #[test]
    fn hexbin_points_covered_with_both_limits() {
        assert_hexbin_points_covered([true, true], 1);
    }

    #[test]
    fn hexbin_points_covered_without_limits_shared() {
        assert_hexbin_points_covered([false, false], 2);
    }

    #[test]
    fn hexbin_points_covered_with_xlim_shared() {
        assert_hexbin_points_covered([true, false], 2);
    }

    #[test]
    fn hexbin_points_covered_with_ylim_shared() {
        assert_hexbin_points_covered([false, true], 2);
    }

    #[test]
    fn hexbin_points_covered_with_both_limits_shared() {
        assert_hexbin_points_covered([true, true], 2);
    }

    #[test]
    fn dense_hexbin_grids_settle_with_whole_regular_cells() {
        for (rows, share_x, share_y, different, gridsize) in [
            (1, false, false, false, 10),
            (1, true, false, false, 10),
            (1, true, false, false, 30),
            (2, true, false, true, 10),
            (2, true, true, false, 10),
        ] {
            let share = |yes| {
                if yes {
                    crate::Share::All
                } else {
                    crate::Share::None
                }
            };
            let grid = crate::Grid::new(rows, 2)
                .unwrap()
                .with_share_x(share(share_x))
                .with_share_y(share(share_y));
            let mut f = Figure::with_grid(10.0, 6.0, Theme::pigment(), grid).unwrap();
            for row in 0..rows {
                for col in 0..2 {
                    let offset = if different {
                        (row * 2 + col) as f64 * 0.7
                    } else {
                        0.0
                    };
                    let (x, y) = dense_points(false, offset);
                    f.add_axes_at(
                        crate::Placement {
                            row: crate::GridIndex::At(row),
                            col: crate::GridIndex::At(col),
                            ..Default::default()
                        },
                        ScaleKind::Linear,
                        ScaleKind::Linear,
                    )
                    .unwrap()
                    .hexbin(
                        &x,
                        &y,
                        HexbinOptions {
                            gridsize,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                }
            }
            assert_regular_whole(&f);
        }
    }

    #[test]
    fn hexbin_counts_once_per_lattice_during_layout() {
        let (x, y) = dense_points(false, 0.0);
        for count in [1, 4] {
            let grid = crate::Grid::new(1, count)
                .unwrap()
                .with_share_x(crate::Share::All);
            let mut f = Figure::with_grid(6.0 * count as f64, 4.0, Theme::pigment(), grid).unwrap();
            crate::bin2d::COUNT_CALLS.with(|calls| calls.set(0));
            for col in 0..count {
                f.add_axes_at(
                    crate::Placement {
                        col: crate::GridIndex::At(col),
                        ..Default::default()
                    },
                    ScaleKind::Linear,
                    ScaleKind::Linear,
                )
                .unwrap()
                .hexbin(&x, &y, HexbinOptions::default())
                .unwrap();
            }
            assert_eq!(crate::bin2d::COUNT_CALLS.with(|calls| calls.get()), 0);
            f.to_scene(FontLibrary::global()).unwrap();
            assert_eq!(
                crate::bin2d::COUNT_CALLS.with(|calls| calls.get()),
                count as usize
            );
        }
    }

    #[test]
    fn dense_hexbin_with_scatter_settles_at_wide_aspect() {
        let (x, y) = dense_points(true, 0.0);
        let mut f = figure(12.0, 2.5);
        f.single_axes_mut()
            .hexbin(&x, &y, HexbinOptions::default())
            .unwrap()
            .scatter(
                crate::AxisData::Numbers(vec![-1.0, 1.0]),
                &[-1.0, 1.0],
                crate::ScatterOptions::default(),
            )
            .unwrap();
        assert_regular_whole(&f);
    }

    #[test]
    fn dense_hexbin_with_ylim_settles_at_extreme_aspects() {
        let (x, y) = dense_points(false, 0.0);
        for (width, height) in [(12.0, 2.5), (3.0, 10.0)] {
            let mut f = figure(width, height);
            f.single_axes_mut()
                .ylim(-10.0, 10.0)
                .unwrap()
                .hexbin(&x, &y, HexbinOptions::default())
                .unwrap();
            assert_regular_whole(&f);
        }
    }

    #[test]
    fn narrow_hexbin_window_names_the_constraint() {
        let (x, y) = dense_points(false, 0.0);
        let mut f = figure(6.0, 4.0);
        f.single_axes_mut()
            .xlim(0.0, 0.3)
            .unwrap()
            .hexbin(
                &x,
                &y,
                HexbinOptions {
                    gridsize: 10,
                    ..Default::default()
                },
            )
            .unwrap();
        let error = f.to_scene(FontLibrary::global()).unwrap_err();
        assert!(matches!(error, crate::Error::Data { .. }), "{error}");
        assert_eq!(error.parameter(), Some("gridsize"));
        assert!(
            error
                .to_string()
                .contains("hexagons of this size do not fit the window")
        );
        assert!(error.hint().unwrap().contains("ylim()"));
        assert!(error.hint().unwrap().contains("gridsize"));
        assert!(!error.to_string().contains("use explicit axis limits"));
    }

    #[test]
    fn single_log_coordinate_has_multiplicative_limits() {
        for hexbin in [false, true] {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let mut ax = f
                .add_axes(
                    ScaleKind::Log {
                        base: crate::LogBase::TEN,
                    },
                    ScaleKind::Linear,
                )
                .unwrap();
            if hexbin {
                ax.hexbin(&[0.1, 0.1], &[0.0, 1.0], HexbinOptions::default())
                    .unwrap();
            } else {
                ax.hist2d(&[0.1, 0.1], &[0.0, 1.0], Hist2dOptions::default())
                    .unwrap();
            }
            let (resolved, _) = plan_figure(&f, (0, 0), FontLibrary::global());
            let (lo, hi) = resolved.x.continuous_scale().domain();
            assert!(
                lo > 0.001 && lo < 0.1 && hi > 0.1 && hi < 10.0,
                "{hexbin}: {lo}..{hi}"
            );
        }
    }

    #[test]
    fn hist2d_log_edges_are_equal_in_axis_space() {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        f.add_axes(
            ScaleKind::Log {
                base: crate::LogBase::TEN,
            },
            ScaleKind::Log {
                base: crate::LogBase::TEN,
            },
        )
        .unwrap()
        .hist2d(
            &[1.0, 9.0, 10.0, 99.0, 100.0],
            &[1.0, 9.0, 10.0, 99.0, 100.0],
            Hist2dOptions {
                bins: (2, 2),
                ..Default::default()
            },
        )
        .unwrap();
        let crate::Mark::Hist2d(mark) = &f.cartesian_panels().next().unwrap().marks()[0] else {
            panic!()
        };
        for edges in [&mark.bins.x_edges, &mark.bins.y_edges] {
            assert_eq!(edges[0], 1.0);
            assert!((edges[1] - 10.0).abs() < 1e-12, "{edges:?}");
            assert_eq!(edges[2], 100.0);
        }
        assert_eq!(mark.bins.cells, vec![(0, 0), (1, 1)]);
        assert_eq!(mark.bins.counts, vec![2.0, 3.0]);
    }

    #[test]
    fn mesh_edges_have_coordinate_errors_without_gap_advice() {
        for bad in [f64::INFINITY, f64::NAN, -1.0] {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let error = f
                .add_axes(
                    ScaleKind::Log {
                        base: crate::LogBase::TEN,
                    },
                    ScaleKind::Linear,
                )
                .unwrap()
                .mesh(&[1.0, bad], &[0.0, 1.0], &[1.0], MeshOptions::default())
                .unwrap_err();
            assert_eq!(error.parameter(), Some("x_edges"));
            assert!(error.to_string().contains("x_edges[1]"));
            assert!(error.to_string().contains("edges must"), "{error}");
            assert!(!error.hint().unwrap().contains("NaN"));
        }
        let mut f = figure(6.0, 4.0);
        let error = f
            .single_axes_mut()
            .hexbin(&[0.0, f64::INFINITY], &[0.0, 1.0], HexbinOptions::default())
            .unwrap_err();
        assert!(
            error.to_string().contains("coordinates must be finite"),
            "{error}"
        );
    }

    #[test]
    fn mesh_narrow_range_error_includes_bounds_and_minimum() {
        let mut f = figure(6.0, 4.0);
        let error = f
            .single_axes_mut()
            .mesh(
                &[1.0, 1.000_000_000_001],
                &[0.0, 1.0],
                &[1.0],
                MeshOptions::default(),
            )
            .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("x_edges spans 1 to 1.000000000001"),
            "{message}"
        );
        assert!(
            message.contains("at least 1e-10 of their magnitude"),
            "{message}"
        );
    }

    fn assert_scatter_markers_inside_clip(f: &Figure) {
        let scene = f.to_scene(FontLibrary::global()).unwrap();
        let mut markers = 0;
        for item in &scene.items {
            let Item::Group {
                clip: Some(clip),
                items,
            } = item
            else {
                continue;
            };
            for item in items {
                let Item::Markers {
                    shape,
                    positions,
                    stroke,
                    ..
                } = item
                else {
                    continue;
                };
                let outline = stroke.as_ref().map_or(0.0, |s| s.width / 2.0);
                let vertices: Vec<_> = shape
                    .verbs()
                    .iter()
                    .flat_map(|v| match *v {
                        PathVerb::MoveTo(p) | PathVerb::LineTo(p) => vec![p],
                        PathVerb::QuadTo(c, p) => vec![c, p],
                        PathVerb::CubicTo(c1, c2, p) => vec![c1, c2, p],
                        PathVerb::Close => vec![],
                    })
                    .collect();
                for position in positions {
                    for vertex in &vertices {
                        let x = position.x + vertex.x;
                        let y = position.y + vertex.y;
                        assert!(x - outline >= clip.x && x + outline <= clip.x + clip.width);
                        assert!(y - outline >= clip.y && y + outline <= clip.y + clip.height);
                    }
                    markers += 1;
                }
            }
        }
        assert_eq!(markers, 2);
    }

    #[test]
    fn mesh_preserves_scatter_padding_and_exact_cell_extent() {
        for scatter in [[0.0, 1.0], [-1.0, 2.0]] {
            let mut f = figure(6.0, 4.0);
            f.single_axes_mut()
                .scatter(
                    crate::AxisData::Numbers(scatter.to_vec()),
                    &scatter,
                    crate::ScatterOptions::default(),
                )
                .unwrap();
            let (before, _) = plan_figure(&f, (0, 0), FontLibrary::global());
            f.single_axes_mut()
                .mesh(&[0.0, 1.0], &[0.0, 1.0], &[1.0], MeshOptions::default())
                .unwrap();
            assert_scatter_markers_inside_clip(&f);
            let (after, _) = plan_figure(&f, (0, 0), FontLibrary::global());
            assert_eq!(
                after.x.continuous_scale().domain(),
                before.x.continuous_scale().domain()
            );
            assert_eq!(
                after.y.continuous_scale().domain(),
                before.y.continuous_scale().domain()
            );
            crate::test_support::scene::assert_scene_bounded(
                &f.to_scene(FontLibrary::global()).unwrap(),
            );
        }
        let mut f = figure(6.0, 4.0);
        f.single_axes_mut()
            .scatter(
                crate::AxisData::Numbers(vec![0.0, 1.0]),
                &[0.0, 1.0],
                crate::ScatterOptions::default(),
            )
            .unwrap()
            .mesh(&[-2.0, 3.0], &[-2.0, 3.0], &[1.0], MeshOptions::default())
            .unwrap();
        let (r, _) = plan_figure(&f, (0, 0), FontLibrary::global());
        assert_eq!(r.x.continuous_scale().domain(), (-2.0, 3.0));
        assert_eq!(r.y.continuous_scale().domain(), (-2.0, 3.0));
    }

    #[test]
    fn mesh_value_errors_name_row_and_column() {
        for (value, scale) in [
            (f64::INFINITY, crate::Transform::Linear),
            (0.0, crate::Transform::Log(crate::LogBase::TEN)),
        ] {
            let mut f = figure(6.0, 4.0);
            let error = f
                .single_axes_mut()
                .mesh(
                    &[0.0, 1.0, 2.0],
                    &[0.0, 1.0, 2.0],
                    &[1.0, 2.0, 3.0, value],
                    MeshOptions {
                        scale,
                        ..Default::default()
                    },
                )
                .unwrap_err();
            assert!(error.to_string().contains("values[1, 1]"), "{error}");
            assert_eq!(error.parameter(), Some("values"));
        }
    }

    #[test]
    fn numeric_cells_contribute_numbers_not_times() {
        let mut f = figure(6.0, 4.0);
        f.single_axes_mut()
            .mesh(&[0.0, 1.0], &[0.0, 1.0], &[1.0], MeshOptions::default())
            .unwrap()
            .hist2d(&[0.0, 1.0], &[0.0, 1.0], Hist2dOptions::default())
            .unwrap()
            .hexbin(&[0.0, 1.0], &[0.0, 1.0], HexbinOptions::default())
            .unwrap();
        let marks =
            crate::resolve::resolve_axes_marks(f.cartesian_panels().next().unwrap(), f.theme());
        assert_eq!(marks.len(), 3);
        for mark in &marks {
            for axis in [crate::resolve::ValueAxis::X, crate::resolve::ValueAxis::Y] {
                assert!(mark.numbers(axis).next().is_some());
                assert!(mark.times(axis).next().is_none());
                assert!(mark.categories(axis).next().is_none());
            }
        }
    }

    #[test]
    fn numeric_cells_reject_time_axes_atomically() {
        for (x, y, parameter) in [
            (ScaleKind::Time, ScaleKind::Linear, "x"),
            (ScaleKind::Linear, ScaleKind::Time, "y"),
            (ScaleKind::Time, ScaleKind::Time, "x"),
        ] {
            let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            f.add_axes(x, y).unwrap();
            let before = f.clone();
            let mut ax = f.single_axes_mut();
            let errors = [
                ax.mesh(&[0.0, 1.0], &[0.0, 1.0], &[1.0], MeshOptions::default())
                    .unwrap_err(),
                ax.hist2d(&[0.0, 1.0], &[0.0, 1.0], Hist2dOptions::default())
                    .unwrap_err(),
                ax.hexbin(&[0.0, 1.0], &[0.0, 1.0], HexbinOptions::default())
                    .unwrap_err(),
            ];
            for (i, error) in errors.into_iter().enumerate() {
                assert!(matches!(error, crate::Error::Usage { .. }));
                assert_eq!(error.parameter(), Some(parameter));
                // Cells of numbers name the time method; counts have none.
                let hint = if i == 0 {
                    "use mesh_time() for times."
                } else {
                    "use Scale.linear() or Scale.log() for both axes."
                };
                assert_eq!(error.hint(), Some(hint));
            }
            assert_eq!(f, before);
        }
    }

    #[test]
    fn mesh_cells_span_edges() {
        let mut f = figure(6.0, 4.0);
        f.single_axes_mut()
            .mesh(
                &[0.0, 1.0, 4.0],
                &[2.0, 3.0, 7.0],
                &[1.0, f64::NAN, 3.0, 4.0],
                MeshOptions::default(),
            )
            .unwrap();
        let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        let cells = cells(&f);
        assert_eq!(cells.len(), 3);
        // The top row is drawn first; the bottom left cell, last, has no neighbour after it
        // to reach under and spans its edges exactly.
        let cell = &cells[2];
        assert_eq!(cell.len(), 4);
        let width = (cell[1].x - cell[0].x).abs() as f64;
        let height = (cell[2].y - cell[1].y).abs() as f64;
        assert!((width - layout.plot.width / 4.0).abs() < 1e-4);
        assert!((height - layout.plot.height / 5.0).abs() < 1e-4);
    }

    #[test]
    fn mesh_edges_set_domain() {
        let mut f = figure(6.0, 4.0);
        f.single_axes_mut()
            .mesh(
                &[-2.0, 1.0, 7.0],
                &[3.0, 4.0, 9.0],
                &[1.0; 4],
                MeshOptions::default(),
            )
            .unwrap();
        let (r, _) = plan_figure(&f, (0, 0), FontLibrary::global());
        assert_eq!(r.x.continuous_scale().domain(), (-2.0, 7.0));
        assert_eq!(r.y.continuous_scale().domain(), (3.0, 9.0));
    }

    #[test]
    fn hist2d_log_skips_empty_cells() {
        let mut f = figure(6.0, 4.0);
        f.single_axes_mut()
            .hist2d(
                &[0.0, 0.0, 2.0],
                &[0.0, 0.0, 2.0],
                Hist2dOptions {
                    bins: (2, 2),
                    scale: crate::Transform::Log(crate::LogBase::TEN),
                    ..Hist2dOptions::default()
                },
            )
            .unwrap();
        assert_eq!(cells(&f).len(), 2);
        let (r, _) = plan_figure(&f, (0, 0), FontLibrary::global());
        let color = r.color.unwrap();
        assert!(color.color(0.0).is_none());
        assert!(color.color(1.0).is_some());
        assert!(color.color(2.0).is_some());
    }

    #[test]
    fn hexbin_counts_sum_to_finite_points() {
        let mut f = figure(6.0, 4.0);
        f.single_axes_mut()
            .hexbin(
                &[0.0, 1.0, 1.0, 2.0, f64::NAN, 10.0],
                &[0.0, 1.0, 1.0, 2.0, 10.0, f64::NAN],
                HexbinOptions::default(),
            )
            .unwrap();
        let marks =
            crate::resolve::resolve_axes_marks(f.cartesian_panels().next().unwrap(), f.theme());
        let crate::resolve::ResolvedMark::Hexbin { mark, bins, .. } = &marks[0] else {
            panic!()
        };
        assert!(bins.counts().is_empty());
        let bins =
            crate::bin2d::HexBins::new(&mark.x, &mark.y, mark.gridsize, mark.transforms, 1.0)
                .unwrap();
        assert_eq!(bins.counts().iter().sum::<f64>(), 4.0);
        assert!(bins.counts().iter().all(|v| *v > 0.0));
    }

    fn hex_figure(width: f64, height: f64) -> Figure {
        let mut f = figure(width, height);
        f.single_axes_mut()
            .hexbin(
                &[0.0, 1.0, 2.0, 0.0, 2.0, f64::NAN],
                &[0.0, 3.0, 0.0, 3.0, 3.0, 1.0],
                HexbinOptions {
                    gridsize: 5,
                    ..HexbinOptions::default()
                },
            )
            .unwrap();
        f
    }

    #[test]
    fn hexagons_regular_on_screen() {
        for (width, height) in [(6.0, 4.0), (4.0, 7.0), (9.0, 3.0)] {
            let f = hex_figure(width, height);
            for points in cells(&f) {
                assert_eq!(points.len(), 6);
                let sides: Vec<_> = (0..6)
                    .map(|i| {
                        let a = points[i];
                        let b = points[(i + 1) % 6];
                        f64::from((a.x - b.x).hypot(a.y - b.y))
                    })
                    .collect();
                for side in &sides {
                    assert!((side - sides[0]).abs() < 1e-4, "{sides:?}");
                }
            }
        }
    }

    #[test]
    fn hexagons_regular_with_explicit_and_mixed_limits() {
        for mode in 0..4 {
            let mut f = hex_figure(6.0, 4.0);
            let mut ax = f.single_axes_mut();
            match mode {
                0 => {
                    ax.xlim(-1.0, 4.0).unwrap().ylim(-2.0, 5.0).unwrap();
                }
                1 => {
                    ax.xlim(-1.0, 4.0).unwrap();
                }
                2 => {
                    ax.ylim(-2.0, 5.0).unwrap();
                }
                _ => {
                    ax.scatter(
                        crate::AxisData::Numbers(vec![-10.0, 10.0]),
                        &[-20.0, 20.0],
                        crate::ScatterOptions::default(),
                    )
                    .unwrap();
                }
            }
            let [panel] = &hex_panels(&f)[..] else {
                panic!("expected one panel");
            };
            let whole: Vec<_> = panel.cells.iter().filter(|cell| cell.whole).collect();
            for cell in &whole {
                assert!(regular(&cell.hexagon), "mode={mode}: {:?}", cell.hexagon);
            }
            assert!(!whole.is_empty());
            check_panel(panel).unwrap_or_else(|error| panic!("mode={mode}: {error}"));
        }
    }

    #[test]
    fn hexagons_are_regular_on_log_and_shared_axes() {
        for log in [false, true] {
            let grid = crate::Grid::new(1, 2)
                .unwrap()
                .with_share_x(crate::Share::All)
                .with_share_y(crate::Share::All);
            let mut f = Figure::with_grid(12.0, 5.0, Theme::pigment(), grid).unwrap();
            let scale = if log {
                ScaleKind::Log {
                    base: crate::LogBase::TEN,
                }
            } else {
                ScaleKind::Linear
            };
            for col in 0..2 {
                f.add_axes_at(
                    crate::Placement {
                        col: crate::GridIndex::At(col),
                        ..Default::default()
                    },
                    scale,
                    scale,
                )
                .unwrap()
                .hexbin(
                    &[1.0, 10.0, 100.0],
                    &[2.0, 20.0, 200.0],
                    HexbinOptions {
                        gridsize: 5,
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            let panels = hex_panels(&f);
            assert!(panels.iter().all(|panel| !panel.cells.is_empty()));
            for panel in &panels {
                for cell in &panel.cells {
                    assert!(cell.whole, "log={log}: clipped hexagon {:?}", cell.hexagon);
                    assert!(regular(&cell.hexagon), "log={log}: {:?}", cell.hexagon);
                }
                check_panel(panel).unwrap_or_else(|error| panic!("log={log}: {error}"));
            }
        }
    }

    #[test]
    fn edge_hexagons_are_whole_under_automatic_limits() {
        for (width, height) in [(6.0, 4.0), (4.0, 7.0), (9.0, 3.0)] {
            let f = hex_figure(width, height);
            let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
            let p = layout.plot;
            let points: Vec<_> = cells(&f).into_iter().flatten().collect();
            assert!(!points.is_empty());
            let bounds = points.iter().fold(
                [
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                ],
                |mut b, v| {
                    b[0] = b[0].min(f64::from(v.x));
                    b[1] = b[1].max(f64::from(v.x));
                    b[2] = b[2].min(f64::from(v.y));
                    b[3] = b[3].max(f64::from(v.y));
                    b
                },
            );
            let gaps = [
                bounds[0] - p.x,
                p.right() - bounds[1],
                bounds[2] - p.y,
                p.bottom() - bounds[3],
            ];
            assert!(gaps.iter().all(|g| *g >= -1e-4), "{gaps:?}");
            assert!((gaps[0] - gaps[1]).abs() < 1e-4, "{gaps:?}");
            assert!((gaps[2] - gaps[3]).abs() < 1e-4, "{gaps:?}");
            assert!(gaps[0].abs() < 1e-4 || gaps[2].abs() < 1e-4, "{gaps:?}");
        }
    }

    /// One panel of a hexbin grid: hexbin coordinates and gridsize, and the x extent of an
    /// optional line from y = -1 to y = 1 drawn beside the hexbin.
    type HexPanelSpec = (Vec<f64>, Vec<f64>, usize, Option<f64>);

    /// A `width` by `height` figure with one column per panel of `specs`, axes not shared.
    fn hex_grid(width: f64, height: f64, specs: &[HexPanelSpec]) -> Figure {
        let grid = crate::Grid::new(1, specs.len() as i64).unwrap();
        let mut f = Figure::with_grid(width, height, Theme::pigment(), grid).unwrap();
        for (col, (x, y, gridsize, line)) in specs.iter().enumerate() {
            let mut axes = f
                .add_axes_at(
                    crate::Placement {
                        col: crate::GridIndex::At(col as i64),
                        ..Default::default()
                    },
                    ScaleKind::Linear,
                    ScaleKind::Linear,
                )
                .unwrap();
            axes.hexbin(
                x,
                y,
                HexbinOptions {
                    gridsize: *gridsize,
                    ..Default::default()
                },
            )
            .unwrap();
            if let Some(half) = line {
                axes.line(
                    crate::AxisData::Numbers(vec![-half, *half]),
                    &[-1.0, 1.0],
                    crate::LineOptions::default(),
                )
                .unwrap();
            }
        }
        f
    }

    /// The scene of `f` laid out keeping tight hexbin fits through every pass, as before
    /// cycles were handled, with the pass it settled at, and the scene laid out as it is now.
    fn scenes_keeping_and_leaving_tight_fits(
        f: &Figure,
    ) -> (
        crate::Result<(crate::Scene, usize)>,
        crate::Result<crate::Scene>,
    ) {
        let keep = |yes| crate::pipeline::KEEP_TIGHT_FITS.with(|k| k.set(yes));
        keep(true);
        let kept = f.to_scene(FontLibrary::global()).map(|scene| {
            (
                scene,
                crate::pipeline::SETTLED_PASS.with(std::cell::Cell::get),
            )
        });
        keep(false);
        (kept, f.to_scene(FontLibrary::global()))
    }

    #[test]
    fn repeated_aspects_with_moving_domains_keep_tight_fits() {
        // The plot aspects of passes 1 and 3 agree while the second panel's y domain still
        // moves; the tight fits settle after many passes and must stay as they were.
        let f = hex_grid(
            6.67,
            6.5,
            &[
                (vec![0.0], vec![3.417_795_483_538_547], 75, None),
                (vec![0.0], vec![2.2], 75, Some(0.002)),
            ],
        );
        let (kept, now) = scenes_keeping_and_leaving_tight_fits(&f);
        let (kept, pass) = kept.unwrap();
        assert!(
            pass < crate::pipeline::TIGHT_PASSES,
            "settled at pass {pass}"
        );
        assert_eq!(now.unwrap(), kept);
    }

    fn hex_panel_spec() -> impl proptest::strategy::Strategy<Value = HexPanelSpec> {
        use proptest::prelude::*;
        let coordinate = |span: f64| prop_oneof![Just(0.0), -span..span];
        (
            prop::collection::vec((coordinate(1.0), coordinate(5.0)), 1..4),
            1usize..=100,
            prop_oneof![Just(None), (1e-3f64..1.0).prop_map(Some)],
        )
            .prop_map(|(points, gridsize, line)| {
                let (x, y) = points.into_iter().unzip();
                (x, y, gridsize, line)
            })
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]
        #[test]
        fn layouts_that_settle_with_tight_fits_are_unchanged(
            specs in proptest::collection::vec(hex_panel_spec(), 1..=2),
            width in 3.0f64..10.0,
            height in 2.5f64..8.0,
        ) {
            let f = hex_grid(width, height, &specs);
            let (kept, now) = scenes_keeping_and_leaving_tight_fits(&f);
            if let Ok((kept, pass)) = kept
                && pass < crate::pipeline::TIGHT_PASSES
            {
                proptest::prop_assert_eq!(now.ok(), Some(kept));
            }
        }
    }

    #[test]
    fn several_hexbins_on_one_axes_settle_with_regular_whole_cells() {
        let x = [0.001_371_668_343_067_440_5, -0.005_193_980_487_487_872];
        let y = [-0.206_678_066_276_808_43, -0.315_107_578_680_739_35];
        let mut f = figure(5.390_892_456_943_188, 3.578_178_939_416_077_2);
        f.single_axes_mut()
            .hexbin(
                &x,
                &y,
                HexbinOptions {
                    gridsize: 52,
                    ..Default::default()
                },
            )
            .unwrap()
            .hexbin(
                &x.map(|x| 0.7 * x + 0.003_563_168_561_633_2),
                &y.map(|y| 1.5 * y - 1.0),
                HexbinOptions {
                    gridsize: 53,
                    ..Default::default()
                },
            )
            .unwrap();
        let panels = hex_panels(&f);
        assert_eq!(panels[0].cells.len(), 4);
        for cell in &panels[0].cells {
            assert!(cell.whole, "clipped hexagon: {:?}", cell.hexagon);
            assert!(regular(&cell.hexagon), "{:?}", cell.hexagon);
        }
    }

    #[test]
    fn hexbins_of_different_extents_on_one_axes_are_all_regular() {
        let x: Vec<f64> = (0..200)
            .map(|i| (i as f64 * 0.618_033_988_7).fract())
            .collect();
        let y: Vec<f64> = (0..200)
            .map(|i| (i as f64 * 0.414_213_562_3).fract())
            .collect();
        for (sx, sy, gridsize) in [
            (1.0, 1.0, 10),
            (1.5, 1.0, 10),
            (0.5, 3.0, 20),
            (2.0, 0.25, 7),
        ] {
            let mut f = figure(6.0, 4.0);
            f.single_axes_mut()
                .hexbin(
                    &x,
                    &y,
                    HexbinOptions {
                        gridsize: 10,
                        ..Default::default()
                    },
                )
                .unwrap()
                .hexbin(
                    &x.iter().map(|v| v * sx).collect::<Vec<_>>(),
                    &y.iter().map(|v| v * sy + 0.5).collect::<Vec<_>>(),
                    HexbinOptions {
                        gridsize,
                        ..Default::default()
                    },
                )
                .unwrap();
            let panels = hex_panels(&f);
            assert!(panels[0].cells.len() >= 20);
            for cell in &panels[0].cells {
                assert!(cell.whole, "clipped hexagon: {:?}", cell.hexagon);
                assert!(regular(&cell.hexagon), "{sx} {sy}: {:?}", cell.hexagon);
            }
        }
    }

    /// A figure of one lone hexbin beside a hexbin with a line and a legend at the right: the
    /// lone panel alternates between tight shapes while the other panel's limits keep growing.
    fn alternating_beside_growing(
        (x, y, span): (f64, f64, f64),
        gridsize: usize,
        (width, height): (f64, f64),
    ) -> Figure {
        let grid = crate::Grid::new(1, 2).unwrap();
        let mut f = Figure::with_grid(width, height, Theme::pigment(), grid).unwrap();
        let at = |col| crate::Placement {
            col: crate::GridIndex::At(col),
            ..Default::default()
        };
        let options = HexbinOptions {
            gridsize,
            ..Default::default()
        };
        f.add_axes_at(at(0), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .hexbin(&[x], &[y], options.clone())
            .unwrap();
        f.add_axes_at(at(1), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .hexbin(&[1.3 * x + span], &[0.7 * y - 0.2], options)
            .unwrap()
            .line(
                crate::AxisData::Numbers(vec![-span, span]),
                &[-1.0, 1.0],
                crate::LineOptions {
                    label: "reference".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .legend_at(
                crate::LegendStyle::Chips,
                crate::LegendPosition::Right,
                crate::SizeSpec::Auto,
            )
            .unwrap();
        f
    }

    #[test]
    fn alternating_panel_beside_growing_panel_settles() {
        let f = alternating_beside_growing(
            (
                -235.041_537_564_207_7,
                -0.546_357_062_448_124_9,
                212.648_455_513_756_86,
            ),
            58,
            (7.489_136_393_708_419, 7.976_442_061_018_854),
        );
        let laid_out = f.to_scene(FontLibrary::global());
        assert!(laid_out.is_ok(), "{:?}", laid_out.err());
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]
        #[test]
        fn alternating_panels_beside_growing_panels_settle(
            x in -300.0f64..300.0,
            y in -1.0f64..1.0,
            span in 1.0f64..300.0,
            gridsize in 1usize..=100,
            width in 4.0f64..10.0,
            height in 3.0f64..10.0,
        ) {
            let f = alternating_beside_growing((x, y, span), gridsize, (width, height));
            let laid_out = f.to_scene(FontLibrary::global());
            proptest::prop_assert!(laid_out.is_ok(), "{:?}", laid_out.err());
        }
    }

    #[test]
    fn hexbins_too_far_apart_for_one_lattice_are_refused() {
        for far in [1e20, 1e200] {
            let mut f = figure(6.0, 4.0);
            f.single_axes_mut()
                .hexbin(&[0.0, 1.0, 1.0], &[0.0, 0.5, 1.0], HexbinOptions::default())
                .unwrap()
                .hexbin(&[0.0, far], &[0.0, 1.0], HexbinOptions::default())
                .unwrap();
            let error = f.to_scene(FontLibrary::global()).unwrap_err();
            assert!(matches!(error, crate::Error::Data { .. }), "{error}");
            assert!(
                error.to_string().contains("too far apart for one lattice"),
                "{error}"
            );
            assert_eq!(
                error.hint(),
                Some("draw them on separate axes, or set explicit limits.")
            );
        }
        // Shared axes count every hexbin in the metric of the shared limits too; a grid of
        // more than one cell names the axes.
        let grid = crate::Grid::new(1, 2)
            .unwrap()
            .with_share_x(crate::Share::All);
        let mut f = Figure::with_grid(10.0, 4.0, Theme::pigment(), grid).unwrap();
        for (col, far) in [(0, 1.0), (1, 1e20)] {
            f.add_axes_at(
                crate::Placement {
                    col: crate::GridIndex::At(col),
                    ..Default::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap()
            .hexbin(&[0.0, 0.5, far], &[0.0, 0.5, 1.0], HexbinOptions::default())
            .unwrap();
        }
        let error = f.to_scene(FontLibrary::global()).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("Figure.to_scene(): row 0, col 0: "),
            "{error}"
        );
        assert!(error.to_string().contains("too far apart for one lattice"));
    }

    #[test]
    fn hexbins_far_apart_on_one_lattice_keep_their_counts() {
        let mut f = figure(6.0, 4.0);
        f.single_axes_mut()
            .hexbin(&[0.0, 1.0, 1.0], &[0.0, 0.5, 1.0], HexbinOptions::default())
            .unwrap()
            .hexbin(&[0.0, 1e6], &[0.0, 1.0], HexbinOptions::default())
            .unwrap();
        let (_, _, marks) =
            crate::pipeline::plan_panel_marks(&f, FontLibrary::global()).swap_remove(0);
        let counts: Vec<_> = marks
            .iter()
            .map(|mark| match mark {
                crate::resolve::ResolvedMark::Hexbin { bins, .. } => bins.counts().to_vec(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(counts, [vec![1.0; 3], vec![1.0; 2]]);
    }

    #[test]
    fn hexbins_without_paired_points_leave_their_neighbour_unchanged() {
        let x: Vec<f64> = (0..300)
            .map(|i| (i as f64 * 0.618_033_988_7).fract() * 3.0)
            .collect();
        let y: Vec<f64> = (0..300)
            .map(|i| (i as f64 * 0.414_213_562_3).fract())
            .collect();
        let svg = |extra: Option<(Vec<f64>, Vec<f64>)>| {
            let mut f = figure(6.0, 4.0);
            let mut axes = f.single_axes_mut();
            axes.hexbin(&x, &y, HexbinOptions::default()).unwrap();
            if let Some((ex, ey)) = extra {
                axes.hexbin(
                    &ex,
                    &ey,
                    HexbinOptions {
                        gridsize: 10,
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            f.to_svg(crate::SvgText::Editable).unwrap()
        };
        let alone = svg(None);
        assert_eq!(svg(Some((Vec::new(), Vec::new()))), alone);
        assert_eq!(svg(Some((vec![f64::NAN, 1.0], vec![2.0, f64::NAN]))), alone);
    }
}
