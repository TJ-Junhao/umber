//! Triangle meshes on whole figures: the Delaunay triangulation and its errors, given triangles
//! and their errors, the order and winding of what is drawn, the edges of `triplot`, the colors
//! of `tripcolor`, and how both hold the axes and their limits.

use crate::geometry::{Path, PathVerb, Point};
use crate::layout::Layout;
use crate::legend::Swatch;
use crate::pipeline::plan_panel_marks;
use crate::resolve::Resolved;
use crate::scene::{FillRule, Item, LineCap, LineJoin, Rect, Scene};
use crate::{
    AxesMut, CmapChoice, ColorSpec, ColorbarOptions, Error, Figure, FontLibrary, LogBase, Mark,
    MeshOptions, Rgba, ScaleKind, SizeSpec, Theme, Transform, TriValues, Triangles, TripcolorMark,
    TripcolorOptions, TriplotMark, TriplotOptions, delaunay,
};

const LINEAR: [ScaleKind; 2] = [ScaleKind::Linear, ScaleKind::Linear];
const LOG_X: [ScaleKind; 2] = [ScaleKind::Log { base: LogBase::TEN }, ScaleKind::Linear];

fn figure_of(kinds: [ScaleKind; 2]) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure.add_axes(kinds[0], kinds[1]).unwrap();
    figure
}

fn axes(figure: &mut Figure) -> AxesMut<'_> {
    figure.single_axes_mut()
}

/// The 30 points whose triangulation is pinned: a 4 by 4 grid of unit squares, each square's
/// corners on one circle; twelve points on one circle around it, at exact coordinates; and two
/// points inside.
fn pinned_points() -> (Vec<f64>, Vec<f64>) {
    let mut points: Vec<(f64, f64)> = Vec::new();
    for j in 0..4 {
        for i in 0..4 {
            points.push((f64::from(i), f64::from(j)));
        }
    }
    // Radius 2.5 around (1.5, 1.5): 2.5² = 1.5² + 2² = 2² + 1.5².
    points.extend([
        (4.0, 1.5),
        (1.5, 4.0),
        (-1.0, 1.5),
        (1.5, -1.0),
        (3.0, 3.5),
        (0.0, 3.5),
        (0.0, -0.5),
        (3.0, -0.5),
        (3.5, 3.0),
        (-0.5, 3.0),
        (-0.5, 0.0),
        (3.5, 0.0),
    ]);
    points.extend([(0.5, 2.25), (2.25, 0.75)]);
    points.into_iter().unzip()
}

/// The triangulation of [`pinned_points`], as `delaunay` returns it on every platform.
const PINNED: [[i64; 3]; 46] = [
    [0, 1, 5],
    [0, 4, 26],
    [0, 5, 4],
    [0, 22, 1],
    [0, 26, 22],
    [1, 2, 5],
    [1, 19, 2],
    [1, 22, 19],
    [2, 3, 29],
    [2, 6, 5],
    [2, 19, 23],
    [2, 23, 3],
    [2, 29, 6],
    [3, 7, 29],
    [3, 23, 27],
    [3, 27, 7],
    [4, 5, 8],
    [4, 8, 18],
    [4, 18, 26],
    [5, 6, 9],
    [5, 9, 8],
    [6, 7, 10],
    [6, 10, 9],
    [6, 29, 7],
    [7, 11, 10],
    [7, 16, 11],
    [7, 27, 16],
    [8, 9, 28],
    [8, 12, 25],
    [8, 25, 18],
    [8, 28, 12],
    [9, 10, 13],
    [9, 13, 28],
    [10, 11, 14],
    [10, 14, 13],
    [11, 15, 14],
    [11, 16, 24],
    [11, 24, 15],
    [12, 13, 21],
    [12, 21, 25],
    [12, 28, 13],
    [13, 14, 17],
    [13, 17, 21],
    [14, 15, 20],
    [14, 20, 17],
    [15, 24, 20],
];

#[test]
fn the_triangulation_of_a_fixed_point_set_is_pinned() {
    let (x, y) = pinned_points();
    let triangles = delaunay(&x, &y).unwrap();
    assert_eq!(triangles, PINNED);
}

#[test]
fn delaunay_rows_are_counter_clockwise_start_at_their_smallest_index_and_use_every_point() {
    let (x, y) = pinned_points();
    let triangles = delaunay(&x, &y).unwrap();
    for row in &triangles {
        let [a, b, c] = row.map(|i| usize::try_from(i).unwrap());
        let cross = (x[b] - x[a]) * (y[c] - y[a]) - (y[b] - y[a]) * (x[c] - x[a]);
        assert!(cross > 0.0, "{row:?} is not counter-clockwise");
        assert!(a < b && a < c, "{row:?} starts at its smallest index");
    }
    let mut sorted = triangles.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, triangles, "rows are sorted");
    let mut used = vec![false; x.len()];
    for &i in triangles.iter().flatten() {
        used[usize::try_from(i).unwrap()] = true;
    }
    assert!(used.iter().all(|&u| u));
    // A square, its diagonal of the library's choice.
    let square = delaunay(&[0.0, 1.0, 1.0, 0.0], &[0.0, 0.0, 1.0, 1.0]).unwrap();
    assert_eq!(square, [[0, 1, 2], [0, 2, 3]]);
}

#[test]
fn delaunay_does_not_depend_on_the_scale_of_the_coordinates() {
    let (x, y) = pinned_points();
    let unit = delaunay(&x, &y).unwrap();
    for factor in [1e-200, 1e-12, 4.0, 1e12, 1e290] {
        let (sx, sy): (Vec<f64>, Vec<f64>) = x
            .iter()
            .zip(&y)
            .map(|(x, y)| (x * factor, y * factor))
            .unzip();
        assert_eq!(delaunay(&sx, &sy).unwrap(), unit, "{factor}");
    }
}

/// The error of `delaunay(x, y)`: its message and parameter.
fn delaunay_error(x: &[f64], y: &[f64]) -> (String, Option<&'static str>) {
    let error = delaunay(x, y).unwrap_err();
    assert!(matches!(error, Error::Data { .. }), "{error:?}");
    (error.to_string(), error.parameter())
}

#[test]
fn delaunay_refuses_too_few_points_one_line_duplicates_and_missing_coordinates() {
    let cases: [(&[f64], &[f64], &str, &str); 9] = [
        (
            &[0.0, 1.0],
            &[0.0, 1.0],
            "delaunay(): x and y hold 2 points; a triangle needs at least three.",
            "x",
        ),
        (
            &[],
            &[],
            "delaunay(): x and y hold 0 points; a triangle needs at least three.",
            "x",
        ),
        (
            &[0.0, 1.0, 2.0],
            &[0.0, 1.0],
            "delaunay(): x has 3 values but y has 2; x and y must have the same length.",
            "y",
        ),
        (
            &[0.0, 1.0, 2.0, 3.0],
            &[1.0, 3.0, 5.0, 7.0],
            "delaunay(): all 4 points lie on one line; a triangle needs three points that do not.",
            "x",
        ),
        (
            &[0.0, 1.0, 1.0, 0.0, 0.5],
            &[0.0, 0.0, 0.0, 0.0, 1.0],
            // Point 2 repeats point 1 and point 3 repeats point 0: the lower repeat is named.
            "delaunay(): points 1 and 2 are both at (1, 0); every point must be different.",
            "x",
        ),
        (
            &[0.0, 1.0, -0.0],
            &[0.0, 1.0, 0.0],
            "delaunay(): points 0 and 2 are both at (0, 0); every point must be different.",
            "x",
        ),
        (
            &[0.0, 1.0, f64::NAN],
            &[0.0, 1.0, 0.0],
            "delaunay(): x[2] is NaN; coordinates must be finite.",
            "x",
        ),
        (
            &[0.0, 1.0, 0.0],
            &[0.0, f64::INFINITY, 1.0],
            "delaunay(): y[1] is inf; coordinates must be finite.",
            "y",
        ),
        (
            // A grid of 3 by 3 points, and a tenth point 2^-51 from its last.
            &[
                0.0,
                1.0,
                2.0,
                0.0,
                1.0,
                2.0,
                0.0,
                1.0,
                2.0,
                2.0 + 2.0 * f64::EPSILON,
            ],
            &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0],
            "delaunay(): points 8 and 9, at (2, 2) and (2.0000000000000004, 2), are too close to \
             tell apart; points must differ by more than about 1e-15 of the largest coordinate \
             along x or y.",
            "x",
        ),
    ];
    for (x, y, message, parameter) in cases {
        assert_eq!(delaunay_error(x, y), (message.to_owned(), Some(parameter)));
    }
}

#[test]
fn delaunay_tells_a_range_too_wide_for_double_precision_from_one_line() {
    // Not on one line, but the library's circumradius overflows.
    let (message, parameter) = delaunay_error(&[0.0, 1.0, 2.0], &[0.0, 0.0, 1e-200]);
    assert_eq!(
        message,
        "delaunay(): x spans 0 to 2 and y spans 0 to 1e-200; coordinates spread over so wide a \
         range of scales cannot be triangulated in double precision."
    );
    assert_eq!(parameter, Some("y"));
    // Scaled to a largest magnitude near 1, y would become subnormal and lose bits.
    let (message, parameter) =
        delaunay_error(&[0.0, 1e200, 0.0, 1e200], &[0.0, 0.0, 1e-200, 1e-200]);
    assert!(message.contains("so wide a range of scales"), "{message}");
    assert_eq!(parameter, Some("y"));
    // Points exactly on one line at the same scales are on one line.
    let (big, small) = (2_f64.powi(900), 2_f64.powi(-900));
    let (message, _) = delaunay_error(&[0.0, big, 2.0 * big], &[0.0, small, 2.0 * small]);
    assert_eq!(
        message,
        "delaunay(): all 3 points lie on one line; a triangle needs three points that do not."
    );
    // A grid, with points on the edges of its hull, triangulates.
    let grid: Vec<f64> = (0..9).map(|i| f64::from(i % 3)).collect();
    let rows: Vec<f64> = (0..9).map(|i| f64::from(i / 3)).collect();
    assert_eq!(delaunay(&grid, &rows).unwrap().len(), 8);
}

#[test]
fn the_zero_area_test_is_exact_whatever_the_order_of_the_corners() {
    // Points 0, 1 and 2 lie exactly on one line, at scales where products underflow and a
    // scaled copy loses bits.
    let (big, small) = (2_f64.powi(900), 2_f64.powi(-177));
    let x = [0.0, 3.0 * big / 8.0, big, big];
    let y = [0.0, 9.0 * small, 24.0 * small, 1.0];
    for row in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut f = figure_of(LINEAR);
        let error = triplot_error(&mut f, (&x, &y), Triangles::Given(&[row]));
        assert!(
            error.to_string().contains("so it has no area"),
            "{row:?}: {error}"
        );
    }
    let mut f = figure_of(LINEAR);
    triplot(&mut f, (&x, &y), Triangles::Given(&[[0, 1, 3]]));
}

/// Adds the edges of a mesh and returns the mark.
fn triplot(figure: &mut Figure, (x, y): (&[f64], &[f64]), triangles: Triangles<'_>) -> TriplotMark {
    let mut axes = axes(figure);
    axes.triplot(x, y, triangles, TriplotOptions::default())
        .unwrap();
    match axes.marks().last() {
        Some(Mark::Triplot(mark)) => mark.clone(),
        other => panic!("not a triplot: {other:?}"),
    }
}

/// Adds the colored triangles of a mesh and returns the mark.
fn tripcolor(
    figure: &mut Figure,
    (x, y): (&[f64], &[f64]),
    triangles: Triangles<'_>,
    values: &[f64],
    options: TripcolorOptions,
) -> TripcolorMark {
    let mut axes = axes(figure);
    axes.tripcolor(x, y, triangles, values, options).unwrap();
    match axes.marks().last() {
        Some(Mark::Tripcolor(mark)) => mark.clone(),
        other => panic!("not a tripcolor: {other:?}"),
    }
}

/// Options of `tripcolor` with one value per triangle.
fn per_triangle() -> TripcolorOptions {
    TripcolorOptions {
        on: TriValues::Triangles,
        ..TripcolorOptions::default()
    }
}

/// The error of adding the edges of a mesh to `figure`, which changes nothing.
fn triplot_error(figure: &mut Figure, (x, y): (&[f64], &[f64]), triangles: Triangles<'_>) -> Error {
    let before = axes(figure).marks().len();
    let error = axes(figure)
        .triplot(x, y, triangles, TriplotOptions::default())
        .unwrap_err();
    assert_eq!(
        axes(figure).marks().len(),
        before,
        "an error changes nothing"
    );
    error
}

/// A unit square of four points around a center point: four triangles meeting at point 4.
fn fan() -> (Vec<f64>, Vec<f64>) {
    (vec![0.0, 1.0, 1.0, 0.0, 0.5], vec![0.0, 0.0, 1.0, 1.0, 0.5])
}

#[test]
fn given_triangles_are_refused_with_their_rows_named() {
    let (x, y) = fan();
    let cases: [(&[[i64; 3]], &str); 7] = [
        (
            &[],
            "Axes.triplot(): triangles is empty; pass at least one triangle.",
        ),
        (
            &[[0, 1, 4], [1, 2, 5]],
            "Axes.triplot(): triangles[1][2] is 5; indices must be from 0 to 4, one per point.",
        ),
        (
            &[[0, -1, 4]],
            "Axes.triplot(): triangles[0][1] is -1; indices must be from 0 to 4, one per point.",
        ),
        (
            &[[0, 1, 4], [2, 4, 2]],
            "Axes.triplot(): triangles[1] is [2, 4, 2]; its three corners must be different \
             points.",
        ),
        (
            &[[0, 4, 2]],
            "Axes.triplot(): triangles[0] is [0, 4, 2]; its corners (0, 0), (0.5, 0.5) and (1, 1) \
             lie on one line, so it has no area.",
        ),
        (
            &[[0, 1, 4], [1, 2, 4], [4, 1, 0]],
            "Axes.triplot(): triangles[0] and triangles[2] join the same points, 0, 1 and 4; each \
             triangle must be given once.",
        ),
        (
            &[[0, 1, 4], [1, 2, 4], [0, 1, 2], [1, 3, 2]],
            "Axes.triplot(): the edge from point 1 to point 2 borders triangles[1], triangles[2] \
             and triangles[3]; an edge borders at most two triangles.",
        ),
    ];
    for (given, message) in cases {
        let mut f = figure_of(LINEAR);
        let error = triplot_error(&mut f, (&x, &y), Triangles::Given(given));
        assert!(matches!(error, Error::Data { .. }), "{error:?}");
        assert_eq!(error.to_string(), message);
        assert_eq!(error.parameter(), Some("triangles"));
    }
}

/// A call with points that fail: `x`, `y`, the scale kinds, the message and the parameter.
type PointsCase<'a> = (&'a [f64], &'a [f64], [ScaleKind; 2], &'a str, &'a str);

#[test]
fn points_are_refused_before_triangles() {
    let given = [[0, 1, 2]];
    let cases: [PointsCase<'_>; 6] = [
        (
            &[0.0, 1.0],
            &[0.0, 1.0, 2.0],
            LINEAR,
            "Axes.triplot(): x has 2 values but y has 3; x and y must have the same length.",
            "y",
        ),
        (
            &[0.0, 1.0, f64::NAN],
            &[0.0, 1.0, 2.0],
            LINEAR,
            "Axes.triplot(): x[2] is NaN; coordinates of a triangle mesh must be finite.",
            "x",
        ),
        (
            &[0.0, 1.0, 2.0],
            &[0.0, -f64::INFINITY, 2.0],
            LINEAR,
            "Axes.triplot(): y[1] is -inf; coordinates of a triangle mesh must be finite.",
            "y",
        ),
        (
            &[1.0, 0.0, 2.0],
            &[0.0, 1.0, 0.0],
            LOG_X,
            "Axes.triplot(): x[1] is 0; coordinates on a log axis must be positive.",
            "x",
        ),
        (
            &[0.0, 1.0],
            &[0.0, 1.0],
            LINEAR,
            "Axes.triplot(): x and y hold 2 points; a triangle needs at least three.",
            "x",
        ),
        (
            &[1.0, 1.0 + 1e-12, 1.0],
            &[0.0, 0.0, 1.0],
            LINEAR,
            "Axes.triplot(): x spans 1 to 1.000000000001; the range must be at least 1e-10 of \
             their magnitude (and at least 1e-290).",
            "x",
        ),
    ];
    for (x, y, kinds, message, parameter) in cases {
        let mut f = figure_of(kinds);
        let error = triplot_error(&mut f, (x, y), Triangles::Given(&given));
        assert!(matches!(error, Error::Data { .. }), "{error:?}");
        assert_eq!(error.to_string(), message);
        assert_eq!(error.parameter(), Some(parameter));
    }
    // Automatic triangles refuse what `delaunay` refuses, under the mark's name.
    let mut f = figure_of(LINEAR);
    let error = triplot_error(
        &mut f,
        (&[0.0, 1.0, 2.0], &[0.0, 1.0, 2.0]),
        Triangles::Auto,
    );
    assert_eq!(
        error.to_string(),
        "Axes.triplot(): all 3 points lie on one line; a triangle needs three points that do not."
    );
}

#[test]
fn time_and_category_axes_are_refused() {
    for (kinds, message, parameter) in [
        (
            [ScaleKind::Time, ScaleKind::Linear],
            "the x axis is a time axis; triangle meshes need linear or log axes.",
            "x",
        ),
        (
            [ScaleKind::Linear, ScaleKind::Category],
            "the y axis is a category axis; triangle meshes need linear or log axes.",
            "y",
        ),
    ] {
        let (x, y) = fan();
        let mut f = figure_of(kinds);
        let error = triplot_error(&mut f, (&x, &y), Triangles::Auto);
        assert!(matches!(error, Error::Usage { .. }), "{error:?}");
        assert_eq!(error.to_string(), format!("Axes.triplot(): {message}"));
        assert_eq!(error.parameter(), Some(parameter));
        let error = axes(&mut f)
            .tripcolor(
                &x,
                &y,
                Triangles::Auto,
                &[1.0; 5],
                TripcolorOptions::default(),
            )
            .unwrap_err();
        assert!(matches!(error, Error::Usage { .. }), "{error:?}");
        assert_eq!(error.to_string(), format!("Axes.tripcolor(): {message}"));
    }
}

fn scene(figure: &Figure) -> Scene {
    figure.to_scene(FontLibrary::global()).unwrap()
}

/// The clip and the items of the marks group of a one-panel figure.
fn marks(scene: &Scene) -> (Rect, Vec<Item>) {
    scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group {
                clip: Some(clip),
                items,
            } => Some((*clip, items.clone())),
            _ => None,
        })
        .expect("a marks group")
}

/// The points of each subpath of `path`.
fn subpaths(path: &Path) -> Vec<Vec<Point>> {
    let mut all: Vec<Vec<Point>> = Vec::new();
    for verb in path.verbs() {
        match *verb {
            PathVerb::MoveTo(p) => all.push(vec![p]),
            PathVerb::LineTo(p) | PathVerb::CubicTo(_, _, p) | PathVerb::QuadTo(_, p) => {
                all.last_mut().expect("a subpath").push(p);
            }
            PathVerb::Close => {}
        }
    }
    all
}

/// The layout of the only panel of `figure`.
fn planned(figure: &Figure) -> (Resolved, Layout) {
    let mut panels = plan_panel_marks(figure, FontLibrary::global());
    let (resolved, layout, _) = panels.remove(0);
    (resolved, layout)
}

#[test]
fn either_winding_and_any_row_order_make_the_same_mesh_and_the_same_drawing() {
    let (x, y) = fan();
    let given = [[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]];
    let flipped = [[4, 0, 3], [4, 3, 2], [1, 4, 2], [0, 4, 1]];
    let drawn = |triangles: &[[i64; 3]], values: &[f64]| {
        let mut f = figure_of(LINEAR);
        let plotted = triplot(&mut f, (&x, &y), Triangles::Given(triangles));
        let colored = tripcolor(
            &mut f,
            (&x, &y),
            Triangles::Given(triangles),
            values,
            per_triangle(),
        );
        assert_eq!(plotted.mesh(), colored.mesh());
        (colored, scene(&f))
    };
    let (mark, first) = drawn(&given, &[1.0, 2.0, 3.0, 4.0]);
    assert_eq!(
        mark.mesh().triangles(),
        [[0, 1, 4], [0, 3, 4], [1, 2, 4], [2, 3, 4]]
    );
    assert_eq!(mark.values(), [1.0, 4.0, 2.0, 3.0]);
    // The same triangles, each row's corners reversed or rotated and the rows in another order,
    // with their values in that order.
    let (mark, second) = drawn(&flipped, &[4.0, 3.0, 2.0, 1.0]);
    assert_eq!(mark.values(), [1.0, 4.0, 2.0, 3.0]);
    assert_eq!(first, second);
}

#[test]
fn triplot_draws_each_edge_once_as_one_path_at_half_the_line_width() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    let mark = triplot(&mut f, (&x, &y), Triangles::Auto);
    assert_eq!(mark.mesh().triangles().len(), 4);
    assert_eq!(
        mark.edges(),
        [
            [0, 1],
            [0, 3],
            [0, 4],
            [1, 2],
            [1, 4],
            [2, 3],
            [2, 4],
            [3, 4]
        ]
    );
    let (_, items) = marks(&scene(&f));
    let [Item::Stroke { path, style }] = items.as_slice() else {
        panic!("one stroked path: {items:?}");
    };
    let edges = subpaths(path);
    assert_eq!(edges.len(), 8);
    assert!(edges.iter().all(|edge| edge.len() == 2));
    let theme = Theme::pigment();
    assert_eq!(style.width, theme.drawn_line_width(SizeSpec::Auto) / 2.0);
    assert_eq!((style.cap, style.join), (LineCap::Round, LineJoin::Round));
    assert_eq!(style.color, theme.palette()[0]);
    // Each edge once: every pair of drawn ends is different.
    let mut ends: Vec<[(u32, u32); 2]> = edges
        .iter()
        .map(|edge| {
            let mut pair = [edge[0], edge[1]].map(|p| (p.x.to_bits(), p.y.to_bits()));
            pair.sort_unstable();
            pair
        })
        .collect();
    ends.sort_unstable();
    ends.dedup();
    assert_eq!(ends.len(), 8);
}

#[test]
fn triplot_takes_a_color_and_a_label_with_a_line_swatch() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    axes(&mut f)
        .line(
            crate::AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            crate::LineOptions {
                label: "Line".into(),
                ..crate::LineOptions::default()
            },
        )
        .unwrap()
        .triplot(
            &x,
            &y,
            Triangles::Auto,
            TriplotOptions {
                label: "Mesh".into(),
                ..TriplotOptions::default()
            },
        )
        .unwrap()
        .triplot(
            &x,
            &y,
            Triangles::Auto,
            TriplotOptions {
                label: "Red".into(),
                color: ColorSpec::Fixed(Rgba::RED),
            },
        )
        .unwrap();
    let (_, layout) = planned(&f);
    let entries = layout.legend.entries();
    assert_eq!(entries.len(), 3);
    let theme = Theme::pigment();
    let width = theme.drawn_line_width(SizeSpec::Auto) / 2.0;
    assert_eq!(entries[1].label.text, "Mesh");
    // The line took the first palette color, so the mesh takes the second.
    assert_eq!(entries[1].swatch.color, theme.palette()[1]);
    assert!(matches!(entries[1].swatch.kind, Swatch::Line { width: w, .. } if w == width));
    assert_eq!(entries[2].swatch.color, Rgba::RED);
    // A label is one line.
    let error = axes(&mut f)
        .triplot(
            &x,
            &y,
            Triangles::Auto,
            TriplotOptions {
                label: "two\nlines".into(),
                ..TriplotOptions::default()
            },
        )
        .unwrap_err();
    assert!(matches!(error, Error::Style { .. }), "{error:?}");
    assert_eq!(error.parameter(), Some("label"));
}

/// The fills of the marks group of `figure`, with the subpaths of each.
fn fills(figure: &Figure) -> Vec<(Rgba, Vec<Vec<Point>>)> {
    let (_, items) = marks(&scene(figure));
    items
        .iter()
        .filter_map(|item| match item {
            Item::Fill {
                path,
                paint: crate::Paint::Solid(color),
                rule,
            } => {
                assert_eq!(*rule, FillRule::NonZero);
                Some((*color, subpaths(path)))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn tripcolor_on_points_takes_the_mean_of_the_corners_and_a_nan_corner_blanks_its_triangles() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    let mark = tripcolor(
        &mut f,
        (&x, &y),
        Triangles::Auto,
        &[0.0, 3.0, 6.0, 9.0, 12.0],
        TripcolorOptions::default(),
    );
    // The triangles [0, 1, 4], [0, 3, 4], [1, 2, 4] and [2, 3, 4].
    assert_eq!(mark.values(), [5.0, 7.0, 7.0, 9.0]);
    // Three colors: three fills, in the order of their values, the second of two triangles.
    let drawn = fills(&f);
    assert_eq!(drawn.len(), 3);
    assert_eq!(drawn[1].1.len(), 2);
    // A stroke in its own color follows each fill.
    let (_, items) = marks(&scene(&f));
    assert_eq!(items.len(), 6);
    for pair in items.chunks(2) {
        let [
            Item::Fill { path, paint, .. },
            Item::Stroke {
                path: outline,
                style,
            },
        ] = pair
        else {
            panic!("a fill and its stroke: {pair:?}");
        };
        assert_eq!(path, outline);
        assert_eq!(*paint, crate::Paint::Solid(style.color));
        assert_eq!(style.width, crate::draw::triangles::HAIRLINE);
        assert_eq!(style.join, LineJoin::Round);
    }
    // A NaN at point 1 blanks the two triangles around it.
    let mut f = figure_of(LINEAR);
    let mark = tripcolor(
        &mut f,
        (&x, &y),
        Triangles::Auto,
        &[0.0, f64::NAN, 6.0, 9.0, 12.0],
        TripcolorOptions::default(),
    );
    assert!(mark.values()[0].is_nan() && mark.values()[2].is_nan());
    let drawn: usize = fills(&f).iter().map(|(_, paths)| paths.len()).sum();
    assert_eq!(drawn, 2);
}

#[test]
fn tripcolor_checks_the_number_of_values_against_on_even_when_both_counts_are_equal() {
    // Four corners and two points inside: six points and six triangles.
    let (x, y) = (
        vec![0.0, 3.0, 3.0, 0.0, 1.0, 2.0],
        vec![0.0, 0.0, 2.0, 2.0, 1.0, 1.1],
    );
    let rows = delaunay(&x, &y).unwrap();
    assert_eq!(rows.len(), x.len());
    let values = [0.0, 3.0, 6.0, 9.0, 12.0, 15.0];
    let mut f = figure_of(LINEAR);
    let on_points = tripcolor(
        &mut f,
        (&x, &y),
        Triangles::Auto,
        &values,
        TripcolorOptions::default(),
    );
    for (&[a, b, c], &mean) in on_points.mesh().triangles().iter().zip(on_points.values()) {
        assert_eq!(mean, (values[a] + values[b] + values[c]) / 3.0);
    }
    // One value per row of `delaunay(x, y)`, whatever order the mesh keeps its triangles in.
    let on_triangles = tripcolor(&mut f, (&x, &y), Triangles::Auto, &values, per_triangle());
    for (corners, &value) in on_triangles
        .mesh()
        .triangles()
        .iter()
        .zip(on_triangles.values())
    {
        let row = rows
            .iter()
            .position(|row| {
                let mut sorted = row.map(|i| usize::try_from(i).unwrap());
                sorted.sort_unstable();
                sorted == *corners
            })
            .unwrap();
        assert_eq!(value, values[row]);
    }
    // The fan has five points and four triangles.
    let (x, y) = fan();
    for (count, options, message) in [
        (
            5,
            per_triangle(),
            "values has 5 entries; on=TriValues.TRIANGLES needs one per triangle: 4.",
        ),
        (
            4,
            TripcolorOptions::default(),
            "values has 4 entries; on=TriValues.POINTS needs one per point: 5.",
        ),
    ] {
        let error = axes(&mut f)
            .tripcolor(&x, &y, Triangles::Auto, &vec![1.0; count], options)
            .unwrap_err();
        assert!(matches!(error, Error::Data { .. }), "{error:?}");
        assert_eq!(error.to_string(), format!("Axes.tripcolor(): {message}"));
        assert_eq!(error.parameter(), Some("values"));
    }
}

#[test]
fn tripcolor_refuses_bad_values_and_centers() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    let error = axes(&mut f)
        .tripcolor(
            &x,
            &y,
            Triangles::Auto,
            &[1.0, 2.0, f64::INFINITY, 4.0, 5.0],
            TripcolorOptions::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Axes.tripcolor(): values[2] is inf; values must be finite."
    );
    let log = TripcolorOptions {
        scale: Transform::Log(LogBase::TEN),
        ..TripcolorOptions::default()
    };
    let error = axes(&mut f)
        .tripcolor(
            &x,
            &y,
            Triangles::Auto,
            &[1.0, 2.0, 0.0, 4.0, 5.0],
            log.clone(),
        )
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Axes.tripcolor(): values[2] is 0; values on a log scale must be positive."
    );
    let error = axes(&mut f)
        .tripcolor(
            &x,
            &y,
            Triangles::Auto,
            &[1.0; 5],
            TripcolorOptions {
                center: Some(0.0),
                ..log
            },
        )
        .unwrap_err();
    assert!(matches!(error, Error::Style { .. }), "{error:?}");
    assert_eq!(error.parameter(), Some("center"));
    assert!(axes(&mut f).marks().is_empty());
}

#[test]
fn tripcolor_shares_the_color_scale_and_colorbar_with_other_marks() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    tripcolor(
        &mut f,
        (&x, &y),
        Triangles::Auto,
        &[0.0, 3.0, 6.0, 9.0, 12.0],
        TripcolorOptions::default(),
    );
    axes(&mut f)
        .mesh(&[2.0, 3.0], &[0.0, 1.0], &[20.0], MeshOptions::default())
        .unwrap()
        .colorbar(ColorbarOptions::default())
        .unwrap();
    let (resolved, layout) = planned(&f);
    assert!(layout.colorbar.is_some());
    // The triangles' values run from 5 to 9, the mesh's cell is 20.
    let scale = resolved.color.unwrap();
    assert_eq!(scale.norm.range(), (5.0, 20.0));
    // A different colormap is refused.
    let error = axes(&mut f)
        .tripcolor(
            &x,
            &y,
            Triangles::Auto,
            &[1.0; 5],
            TripcolorOptions {
                cmap: CmapChoice::Fixed(crate::Cmap::viridis()),
                ..TripcolorOptions::default()
            },
        )
        .unwrap_err();
    assert!(matches!(error, Error::Usage { .. }), "{error:?}");
    assert_eq!(error.parameter(), Some("cmap"));
}

#[test]
fn tripcolor_with_a_center_and_a_log_scale() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    tripcolor(
        &mut f,
        (&x, &y),
        Triangles::Auto,
        &[-6.0, 0.0, 3.0, 6.0, 0.0],
        TripcolorOptions {
            center: Some(0.0),
            ..TripcolorOptions::default()
        },
    );
    let (resolved, _) = planned(&f);
    // The triangles' values are -2, 0, 1 and 3: symmetric around 0.
    assert_eq!(resolved.color.unwrap().norm.range(), (-3.0, 3.0));
    let mut f = figure_of(LINEAR);
    tripcolor(
        &mut f,
        (&x, &y),
        Triangles::Auto,
        &[3.0, 30.0, 300.0, 3000.0, 3.0],
        TripcolorOptions {
            scale: Transform::Log(LogBase::TEN),
            ..TripcolorOptions::default()
        },
    );
    let (resolved, _) = planned(&f);
    // The means of the triangles [0, 1, 4], [0, 3, 4], [1, 2, 4] and [2, 3, 4].
    assert_eq!(resolved.color.unwrap().norm.range(), (12.0, 1101.0));
}

#[test]
fn limits_are_the_extent_of_every_point_without_padding_and_hold_equal_aspect() {
    let (mut x, mut y) = fan();
    // A point that no triangle uses still counts.
    x.push(3.0);
    y.push(-1.0);
    let given = [[0, 1, 4], [1, 2, 4]];
    for colored in [false, true] {
        let mut f = figure_of(LINEAR);
        if colored {
            tripcolor(
                &mut f,
                (&x, &y),
                Triangles::Given(&given),
                &[1.0, 2.0],
                per_triangle(),
            );
        } else {
            triplot(&mut f, (&x, &y), Triangles::Given(&given));
        }
        let (resolved, _) = planned(&f);
        assert_eq!(resolved.x.continuous().unwrap().domain(), (0.0, 3.0));
        assert_eq!(resolved.y.continuous().unwrap().domain(), (-1.0, 1.0));
        axes(&mut f).equal_aspect().unwrap();
        let (resolved, layout) = planned(&f);
        assert_eq!(resolved.x.continuous().unwrap().domain(), (0.0, 3.0));
        let ratio = (layout.plot.width / 3.0) / (layout.plot.height / 2.0);
        assert!((ratio - 1.0).abs() < 1e-9, "{ratio}");
    }
}

#[test]
fn on_a_log_axis_each_triangle_is_straight_between_its_corners_on_the_plot() {
    let (x, y) = (vec![1.0, 100.0, 10.0], vec![0.0, 0.0, 1.0]);
    let given = [[0, 1, 2]];
    let mut f = figure_of(LOG_X);
    tripcolor(
        &mut f,
        (&x, &y),
        Triangles::Given(&given),
        &[1.0],
        per_triangle(),
    );
    triplot(&mut f, (&x, &y), Triangles::Given(&given));
    let (clip, items) = marks(&scene(&f));
    let [
        Item::Fill { path, .. },
        Item::Stroke { .. },
        Item::Stroke { path: edges, .. },
    ] = items.as_slice()
    else {
        panic!("a filled triangle and its edges: {items:?}");
    };
    // The corners as drawn: the top one, at x = 10, halfway between the two bottom ones, at 1
    // and 100, on a log axis; the limits are the extent of the points, so the triangle spans the
    // plot, which the marks clip holds.
    let corners = &subpaths(path)[0];
    assert_eq!(corners.len(), 3);
    let mut sorted = corners.clone();
    sorted.sort_by(|a, b| a.x.total_cmp(&b.x));
    let [left, top, right] = [sorted[0], sorted[1], sorted[2]];
    assert_eq!(left.y, right.y);
    assert!(
        (top.x - f32::midpoint(left.x, right.x)).abs() < 1e-3,
        "{corners:?}"
    );
    assert!(top.y < left.y);
    assert!(clip.x <= left.x && right.x <= clip.x + clip.width);
    let close = |p: Point, (x, y): (f32, f32)| (p.x - x).abs() < 1e-3 && (p.y - y).abs() < 1e-3;
    // Each edge is one straight segment between two corners.
    for edge in subpaths(edges) {
        assert_eq!(edge.len(), 2);
        assert!(
            edge.iter()
                .all(|&p| corners.iter().any(|&c| close(p, (c.x, c.y))))
        );
    }
}

/// Twice the signed area of the polygon `points` on the page, positive clockwise as seen.
fn twice_area(points: &[Point]) -> f64 {
    (0..points.len())
        .map(|i| {
            let (p, q) = (points[i], points[(i + 1) % points.len()]);
            f64::from(p.x) * f64::from(q.y) - f64::from(q.x) * f64::from(p.y)
        })
        .sum()
}

#[test]
fn every_triangle_winds_one_way_on_the_page() {
    let (x, y) = pinned_points();
    for kinds in [LINEAR, LOG_X] {
        let x: Vec<f64> = x.iter().map(|x| x + 2.0).collect();
        let mut f = figure_of(kinds);
        let values: Vec<f64> = (0..x.len()).map(|i| f64::from(i as u32 % 3)).collect();
        tripcolor(
            &mut f,
            (&x, &y),
            Triangles::Auto,
            &values,
            TripcolorOptions::default(),
        );
        let drawn = fills(&f);
        assert_eq!(
            drawn.iter().map(|(_, paths)| paths.len()).sum::<usize>(),
            delaunay(&x, &y).unwrap().len()
        );
        for (_, triangles) in drawn {
            for triangle in triangles {
                assert!(twice_area(&triangle) > 0.0, "{triangle:?}");
            }
        }
    }
}

#[test]
fn overlapping_triangles_draw_the_higher_value_on_top_whatever_else_the_mesh_holds() {
    // Triangles 1 and 2 overlap; triangle 0, far from them, comes first in the mesh.
    let x = [10.0, 11.0, 10.0, 0.0, 2.0, 0.0, 0.5, 2.5, 0.5];
    let y = [0.0, 0.0, 1.0, 0.0, 0.0, 2.0, 0.5, 0.5, 2.5];
    let low = [[3, 4, 5]];
    let high = [[6, 7, 8]];
    for with_far in [false, true] {
        let mut given: Vec<[i64; 3]> = Vec::new();
        let mut values = Vec::new();
        if with_far {
            given.push([0, 1, 2]);
            values.push(2.0);
        }
        given.extend(low);
        values.push(1.0);
        given.extend(high);
        values.push(2.0);
        let mut f = figure_of(LINEAR);
        tripcolor(
            &mut f,
            (&x, &y),
            Triangles::Given(&given),
            &values,
            per_triangle(),
        );
        let drawn = fills(&f);
        assert_eq!(drawn.len(), 2, "{with_far}");
        // The low triangle first, then the high ones, with the far one if it is there.
        assert_eq!(drawn[0].1.len(), 1);
        assert_eq!(drawn[1].1.len(), 1 + usize::from(with_far));
    }
}

#[test]
fn a_triangle_beyond_explicit_limits_is_clipped() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    tripcolor(
        &mut f,
        (&x, &y),
        Triangles::Auto,
        &[0.0, 3.0, 6.0, 9.0, 12.0],
        TripcolorOptions::default(),
    );
    triplot(&mut f, (&x, &y), Triangles::Auto);
    axes(&mut f).xlim(0.6, 2.0).unwrap();
    let (clip, items) = marks(&scene(&f));
    let points: Vec<Point> = items
        .iter()
        .filter_map(|item| match item {
            Item::Fill { path, .. } | Item::Stroke { path, .. } => Some(subpaths(path)),
            _ => None,
        })
        .flatten()
        .flatten()
        .collect();
    assert!(!points.is_empty());
    // Clipped in axis space a few points past the plot's left edge.
    assert!(points.iter().all(|p| p.x >= clip.x - 4.0), "{points:?}");
}
