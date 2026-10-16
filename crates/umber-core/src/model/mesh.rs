//! Numeric cells and two-dimensional count marks.

use std::sync::Arc;

use super::{Axes, CmapChoice, ColorBy, Mark};
use crate::bin2d::RectBins;
use crate::binning::BinCount;
use crate::color_scale::ColorScaleSpec;
use crate::error::number;
use crate::{Error, MAX_BINS, Result, Transform};

/// Colormap and normalization of numeric cells.
///
/// # Examples
///
/// ```
/// use umber_core::{MeshOptions, Transform};
/// assert_eq!(MeshOptions::default().scale, Transform::Linear);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MeshOptions {
    /// The colormap; the theme chooses sequential or diverging according to `center`.
    pub cmap: CmapChoice,
    /// Linear or log color normalization.
    pub scale: Transform,
    /// The middle value of a symmetric, linear diverging mapping.
    pub center: Option<f64>,
}

impl Default for MeshOptions {
    fn default() -> Self {
        Self {
            cmap: CmapChoice::Theme,
            scale: Transform::Linear,
            center: None,
        }
    }
}

/// Equal-width bin counts and their color mapping.
///
/// # Examples
///
/// ```
/// use umber_core::Hist2dOptions;
/// assert_eq!(Hist2dOptions::default().bins, (30, 30));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Hist2dOptions {
    /// Number of bins along x and y, each in `1..=MAX_BINS`.
    pub bins: (usize, usize),
    /// The colormap.
    pub cmap: CmapChoice,
    /// Linear or log color normalization.
    pub scale: Transform,
}

impl Default for Hist2dOptions {
    fn default() -> Self {
        Self {
            bins: (30, 30),
            cmap: CmapChoice::Theme,
            scale: Transform::Linear,
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
    pub(crate) x_edges: Arc<[f64]>,
    pub(crate) y_edges: Arc<[f64]>,
    pub(crate) colors: ColorBy,
}

impl MeshMark {
    /// Validates every edge and cell before storing anything.
    pub(super) fn new(
        axes: &Axes,
        x: &[f64],
        y: &[f64],
        values: &[f64],
        options: MeshOptions,
    ) -> Result<Self> {
        let context = "Axes.mesh()";
        let transforms = numeric_axes(context, axes)?;
        for (name, edges, transform) in
            [("x_edges", x, transforms[0]), ("y_edges", y, transforms[1])]
        {
            if edges.len() < 2 {
                return Err(Error::data(
                    context,
                    format!(
                        "{name} has {} values; at least two edges are required.",
                        edges.len()
                    ),
                )
                .with_parameter(name)
                .with_hint("pass one more edge than the number of cells."));
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
        }
        for (name, edges, transform) in
            [("x_edges", x, transforms[0]), ("y_edges", y, transforms[1])]
        {
            crate::ContinuousScale::new(transform, edges[0], edges[edges.len() - 1], context)
                .map_err(|_| {
                    Error::data(
                        context,
                        format!(
                            "{name} spans {} to {}; {}.",
                            number(edges[0]),
                            number(edges[edges.len() - 1]),
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
                })?;
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
        check_mapping(context, axes, &colors)?;
        Ok(Self {
            x_edges: x.into(),
            y_edges: y.into(),
            colors,
        })
    }
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
}

impl Hist2dMark {
    /// Validates the coordinates and options, then bins paired finite points once.
    pub(super) fn new(axes: &Axes, x: &[f64], y: &[f64], options: Hist2dOptions) -> Result<Self> {
        let context = "Axes.hist2d()";
        let transforms = numeric_axes(context, axes)?;
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
        check_mapping(context, axes, &colors)?;
        Ok(Self {
            bins: Arc::new(RectBins::new(x, y, bins, transforms)),
            colors: ColorScaleSpec::of(&colors),
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
    pub(super) fn new(axes: &Axes, x: &[f64], y: &[f64], options: HexbinOptions) -> Result<Self> {
        let context = "Axes.hexbin()";
        let transforms = numeric_axes(context, axes)?;
        let gridsize = bin_count(context, "gridsize", options.gridsize)?;
        check_points(context, x, y, transforms)?;
        let colors = ColorBy {
            cmap: options.cmap,
            scale: options.scale,
            ..ColorBy::new(Vec::new())
        };
        check_mapping(context, axes, &colors)?;
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
fn numeric_axes(context: &'static str, axes: &Axes) -> Result<[Transform; 2]> {
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

/// A mark must use the existing axes-wide colormap, transform, center and color limits.
fn check_mapping(context: &'static str, axes: &Axes, colors: &ColorBy) -> Result<()> {
    colors.check().map_err(|error| relocate(error, context))?;
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
    Ok(())
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

    fn assert_regular_whole(f: &Figure) {
        let polygons = cells(f);
        assert!(polygons.len() >= 10, "{} cells", polygons.len());
        for points in polygons.into_iter().filter(|p| p.len() > 1) {
            assert_eq!(points.len(), 6, "clipped hexagon: {points:?}");
            let sides: Vec<_> = (0..6)
                .map(|i| {
                    let a = points[i];
                    let b = points[(i + 1) % 6];
                    f64::from((a.x - b.x).hypot(a.y - b.y))
                })
                .collect();
            assert!(sides[0] > 0.0);
            for side in &sides {
                assert!((side - sides[0]).abs() < 1e-4, "{sides:?}");
            }
        }
    }

    fn contains_point(polygon: &[Point], x: f64, y: f64) -> bool {
        let mut positive = false;
        let mut negative = false;
        for (a, b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
            let dx = f64::from(b.x) - f64::from(a.x);
            let dy = f64::from(b.y) - f64::from(a.y);
            let cross = dx * (y - f64::from(a.y)) - dy * (x - f64::from(a.x));
            let tolerance = 1e-4 * dx.hypot(dy);
            positive |= cross > tolerance;
            negative |= cross < -tolerance;
        }
        polygon.len() >= 3 && !(positive && negative)
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
            let polygons = cells(&f);
            let count_calls = crate::bin2d::COUNT_CALLS.with(|calls| calls.get());
            for col in 0..columns {
                let (resolved, layout) = plan_figure(&f, (0, col as usize), FontLibrary::global());
                let plot = layout.plot;
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
                    if !polygons.iter().any(|polygon| contains_point(polygon, x, y)) {
                        uncovered += 1;
                    }
                }
                let whole: Vec<_> = polygons
                    .iter()
                    .filter(|polygon| {
                        !polygon.is_empty()
                            && polygon.iter().all(|p| {
                                f64::from(p.x) > plot.x + 1e-4
                                    && f64::from(p.x) < plot.right() - 1e-4
                                    && f64::from(p.y) > plot.y + 1e-4
                                    && f64::from(p.y) < plot.bottom() - 1e-4
                            })
                    })
                    .collect();
                assert!(in_window > 0);
                assert!(!whole.is_empty());
                let irregular = whole
                    .iter()
                    .filter(|polygon| {
                        let sides: Vec<_> = polygon
                            .iter()
                            .zip(polygon.iter().cycle().skip(1))
                            .map(|(a, b)| f64::from((a.x - b.x).hypot(a.y - b.y)))
                            .collect();
                        polygon.len() != 6
                            || sides[0] <= 0.0
                            || sides.iter().any(|side| (side - sides[0]).abs() >= 1e-4)
                    })
                    .count();
                let result = format!(
                    "limits={limits:?} columns={columns} col={col} normal={normal} \
                     {width}x{height} g{gridsize} ylim={ylim:?}: \
                     uncovered={uncovered}/{in_window}, irregular={irregular}/{}, \
                     count_calls={count_calls}",
                    whole.len(),
                );
                eprintln!("{result}");
                if uncovered != 0 || irregular != 0 {
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
        let crate::Mark::Hist2d(mark) = &f.panels().next().unwrap().marks()[0] else {
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
        let marks = crate::resolve::resolve_axes_marks(f.panels().next().unwrap(), f.theme());
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
            for error in errors {
                assert!(matches!(error, crate::Error::Usage { .. }));
                assert_eq!(error.parameter(), Some(parameter));
                assert_eq!(
                    error.hint(),
                    Some("use Scale.linear() or Scale.log() for both axes.")
                );
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
        assert_eq!(cells[0].len(), 4);
        let width = (cells[0][1].x - cells[0][0].x).abs() as f64;
        let height = (cells[0][2].y - cells[0][1].y).abs() as f64;
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
        let marks = crate::resolve::resolve_axes_marks(f.panels().next().unwrap(), f.theme());
        let crate::resolve::ResolvedMark::Hexbin { mark, bins, .. } = &marks[0] else {
            panic!()
        };
        assert!(bins.counts().is_empty());
        let bins =
            crate::bin2d::HexBins::new(&mark.x, &mark.y, mark.gridsize, mark.transforms, 1.0);
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
            let mut whole = 0;
            for points in cells(&f).into_iter().filter(|p| p.len() == 6) {
                whole += 1;
                let sides: Vec<_> = (0..6)
                    .map(|i| {
                        let a = points[i];
                        let b = points[(i + 1) % 6];
                        f64::from((a.x - b.x).hypot(a.y - b.y))
                    })
                    .collect();
                for side in &sides {
                    assert!((side - sides[0]).abs() < 1e-4, "mode={mode}: {sides:?}");
                }
            }
            assert!(whole > 0);
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
            let polygons = cells(&f);
            assert!(!polygons.is_empty());
            for points in polygons {
                assert_eq!(points.len(), 6);
                let sides: Vec<_> = (0..6)
                    .map(|i| {
                        let a = points[i];
                        let b = points[(i + 1) % 6];
                        f64::from((a.x - b.x).hypot(a.y - b.y))
                    })
                    .collect();
                for side in &sides {
                    assert!((side - sides[0]).abs() < 1e-4, "log={log}: {sides:?}");
                }
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
}
