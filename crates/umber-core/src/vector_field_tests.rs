//! Vector fields on whole figures: what arrows and barbs draw, how they hold the limits, and
//! their legend entries.

use crate::field::arrow::ArrowFrame;
use crate::field::barb::{self, LINE_RISE};
use crate::geometry::{Path, PathVerb, Point};
use crate::layout::{Area, Layout};
use crate::legend::Swatch;
use crate::pipeline::plan_panel_marks;
use crate::resolve::{Resolved, ResolvedMark};
use crate::scene::{Item, Paint, Rect, Scene};
use crate::{
    ArrowKey, ArrowLength, AxesMut, AxisData, BarbOptions, ColorBy, ColorSpec, Direction, Error,
    FieldVectors, Figure, FontLibrary, Instant, LegendPosition, LegendStyle, LogBase, Pivot,
    PointColor, QuiverOptions, Rgba, ScaleKind, SizeSpec, Theme,
};

fn numbers(values: &[f64]) -> AxisData {
    AxisData::Numbers(values.to_vec())
}

fn figure_of(width: f64, height: f64, theme: Theme, kinds: [ScaleKind; 2]) -> Figure {
    let mut figure = Figure::new(width, height, theme).unwrap();
    figure.add_axes(kinds[0], kinds[1]).unwrap();
    figure
}

const LINEAR: [ScaleKind; 2] = [ScaleKind::Linear, ScaleKind::Linear];

/// A 6 × 4 in figure with linear axes in the default theme.
fn linear() -> Figure {
    figure_of(6.0, 4.0, Theme::pigment(), LINEAR)
}

fn axes(figure: &mut Figure) -> AxesMut<'_> {
    figure.single_axes_mut()
}

fn scene(figure: &Figure) -> Scene {
    figure.to_scene(FontLibrary::global()).unwrap()
}

/// The clip and the items of the marks group of a one-panel figure.
fn marks(scene: &Scene) -> (Rect, &[Item]) {
    scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group {
                clip: Some(clip),
                items,
            } => Some((*clip, items.as_slice())),
            _ => None,
        })
        .expect("a marks group")
}

/// The closed or open subpaths of `path`, each as its on-curve points.
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

/// The filled paths among `items`.
fn fills(items: &[Item]) -> Vec<&Path> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Fill { path, .. } => Some(path),
            _ => None,
        })
        .collect()
}

/// The strokes among `items`.
fn strokes(items: &[Item]) -> Vec<&Path> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Stroke { path, .. } => Some(path),
            _ => None,
        })
        .collect()
}

/// The arrows of a one-panel figure of one color: each arrow's corners.
fn arrows(figure: &Figure) -> Vec<Vec<Point>> {
    let scene = scene(figure);
    let (_, items) = marks(&scene);
    let paths = fills(items);
    assert_eq!(paths.len(), 1, "one item for one color");
    subpaths(paths[0])
}

/// The tail and the tip of the arrow with these corners, on the page.
fn ends(corners: &[Point]) -> ([f64; 2], [f64; 2]) {
    let f = |p: Point| [f64::from(p.x), f64::from(p.y)];
    match corners.len() {
        // A closing point is not stored: seven corners with a shaft, three without.
        7 => {
            let (a, b) = (f(corners[0]), f(corners[6]));
            ([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], f(corners[3]))
        }
        3 => {
            let (a, b) = (f(corners[0]), f(corners[2]));
            ([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], f(corners[1]))
        }
        n => panic!("an arrow has {n} corners"),
    }
}

fn length(corners: &[Point]) -> f64 {
    let (tail, tip) = ends(corners);
    (tip[0] - tail[0]).hypot(tip[1] - tail[1])
}

/// The arrow vector on the page, y up.
fn vector(corners: &[Point]) -> [f64; 2] {
    let (tail, tip) = ends(corners);
    [tip[0] - tail[0], tail[1] - tip[1]]
}

fn quiver(figure: &mut Figure, x: &[f64], y: &[f64], u: &[f64], v: &[f64], options: QuiverOptions) {
    axes(figure)
        .quiver(
            numbers(x),
            numbers(y),
            FieldVectors::points(u.to_vec(), v.to_vec()),
            options,
        )
        .unwrap();
}

fn grid_quiver(
    figure: &mut Figure,
    x: &[f64],
    y: &[f64],
    u: &[f64],
    v: &[f64],
    options: QuiverOptions,
) {
    axes(figure)
        .quiver(
            numbers(x),
            numbers(y),
            FieldVectors::grid(u.to_vec(), v.to_vec()),
            options,
        )
        .unwrap();
}

fn scaled(pt_per_unit: f64) -> QuiverOptions {
    QuiverOptions {
        length: ArrowLength::PtPerUnit(pt_per_unit),
        ..QuiverOptions::default()
    }
}

/// The layout of the only panel of `figure`, with its frames.
fn planned(figure: &Figure) -> (Resolved, Layout, Vec<ArrowFrame>) {
    let mut panels = plan_panel_marks(figure, FontLibrary::global());
    let (resolved, layout, marks) = panels.remove(0);
    let frames = marks
        .iter()
        .filter_map(|mark| match mark {
            ResolvedMark::Quiver { frame, .. } => Some(*frame),
            _ => None,
        })
        .collect();
    (resolved, layout, frames)
}

/// The points of one data unit along x and y on the final plot.
fn points_per_unit(resolved: &Resolved, plot: Area) -> [f64; 2] {
    let span = |scale: &crate::ContinuousScale| {
        let (lo, hi) = scale.domain();
        hi - lo
    };
    [
        plot.width / span(resolved.x.continuous().unwrap()),
        plot.height / span(resolved.y.continuous().unwrap()),
    ]
}

#[test]
fn points_draw_one_arrow_each_and_a_grid_one_per_crossing() {
    let mut points = linear();
    quiver(
        &mut points,
        &[0.0, 1.0, 2.0],
        &[0.0, 1.0, 0.0],
        &[1.0, 1.0, 1.0],
        &[0.0, 0.0, 1.0],
        scaled(10.0),
    );
    assert_eq!(arrows(&points).len(), 3);
    let mut grid = linear();
    grid_quiver(
        &mut grid,
        &[0.0, 1.0, 2.0],
        &[0.0, 1.0],
        &[1.0; 6],
        &[0.0; 6],
        scaled(10.0),
    );
    assert_eq!(arrows(&grid).len(), 6);
    // A grid runs row by row: the second row lies above the first on the page.
    let drawn = arrows(&grid);
    assert_eq!(ends(&drawn[0]).0[1], ends(&drawn[2]).0[1]);
    assert!(ends(&drawn[3]).0[1] < ends(&drawn[0]).0[1]);
}

/// The vectors, `x` and `y` of a call that is refused, with its message and parameter.
type ShapeCase<'a> = (FieldVectors, &'a [f64], &'a [f64], &'a str, &'a str);

#[test]
fn wrong_shapes_are_data_errors_naming_the_parameter() {
    let mut f = linear();
    let mut a = axes(&mut f);
    let cases: [ShapeCase<'_>; 5] = [
        (
            FieldVectors::points(vec![1.0], vec![1.0, 2.0]),
            &[0.0],
            &[0.0],
            "Axes.quiver(): u has 1 values but v has 2; u and v must have the same shape.",
            "v",
        ),
        (
            FieldVectors::points(vec![1.0], vec![1.0]),
            &[0.0],
            &[0.0, 1.0],
            "Axes.quiver(): x has 1 values but y has 2; x, y, u and v must have the same length.",
            "y",
        ),
        (
            FieldVectors::points(vec![1.0, 2.0], vec![1.0, 2.0]),
            &[0.0],
            &[0.0],
            "Axes.quiver(): x has 1 values but u has 2; x, y, u and v must have the same length.",
            "u",
        ),
        (
            FieldVectors::grid(vec![1.0; 5], vec![1.0; 5]),
            &[0.0, 1.0, 2.0],
            &[0.0, 1.0],
            "Axes.quiver(): u has 5 values; it must have len(y) * len(x) = 2 * 3 = 6.",
            "u",
        ),
        (
            FieldVectors::grid(vec![1.0; 2], vec![1.0; 2]),
            &[1.0, 0.0],
            &[0.0],
            "Axes.quiver(): x[1] is 0; coordinates of a grid must be strictly increasing.",
            "x",
        ),
    ];
    for (vectors, x, y, message, parameter) in cases {
        let error = a
            .quiver(numbers(x), numbers(y), vectors, QuiverOptions::default())
            .unwrap_err();
        assert!(matches!(error, Error::Data { .. }), "{error:?}");
        assert_eq!(error.to_string(), message);
        assert_eq!(error.parameter(), Some(parameter));
    }
    assert!(a.marks().is_empty());
}

#[test]
fn infinite_components_are_named_by_row_and_column_on_a_grid() {
    let mut f = linear();
    let error = axes(&mut f)
        .quiver(
            numbers(&[0.0, 1.0]),
            numbers(&[0.0, 1.0]),
            FieldVectors::grid(vec![1.0; 4], vec![1.0, 1.0, f64::INFINITY, 1.0]),
            QuiverOptions::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Axes.quiver(): v[1][0] is inf; values must be finite."
    );
    assert_eq!(error.parameter(), Some("v"));
}

#[test]
fn nan_leaves_an_arrow_out_of_the_drawing_the_limits_and_the_scale() {
    let mut with_nan = linear();
    quiver(
        &mut with_nan,
        &[0.0, 1.0, f64::NAN, 100.0, 2.0],
        &[0.0, 1.0, 5.0, 100.0, 0.0],
        &[1.0, 1.0, 50.0, f64::NAN, 2.0],
        &[0.0, 0.0, 0.0, 1.0, 0.0],
        QuiverOptions::default(),
    );
    let mut without = linear();
    quiver(
        &mut without,
        &[0.0, 1.0, 2.0],
        &[0.0, 1.0, 0.0],
        &[1.0, 1.0, 2.0],
        &[0.0, 0.0, 0.0],
        QuiverOptions::default(),
    );
    assert_eq!(scene(&with_nan), scene(&without));
}

#[test]
fn a_nan_grid_coordinate_leaves_its_column_out() {
    let mut f = linear();
    grid_quiver(
        &mut f,
        &[0.0, f64::NAN, 2.0],
        &[0.0, 1.0],
        &[1.0; 6],
        &[0.0; 6],
        scaled(10.0),
    );
    assert_eq!(arrows(&f).len(), 4);
}

#[test]
fn a_zero_vector_draws_a_dot_as_wide_as_the_shaft() {
    let mut f = linear();
    quiver(
        &mut f,
        &[0.0, 1.0],
        &[0.0, 1.0],
        &[0.0, 1.0],
        &[0.0, 0.0],
        scaled(20.0),
    );
    let scene = scene(&f);
    let (_, items) = marks(&scene);
    let paths = fills(items);
    let parts = subpaths(paths[0]);
    assert_eq!(parts.len(), 2);
    let across = |along: fn(&Point) -> f32| {
        let values = parts[0].iter().map(along);
        let high = values.clone().fold(f32::NEG_INFINITY, f32::max);
        high - values.fold(f32::INFINITY, f32::min)
    };
    assert!((f64::from(across(|p| p.x)) - 1.0).abs() < 1e-3);
    assert!((f64::from(across(|p| p.y)) - 1.0).abs() < 1e-3);
}

#[test]
fn a_length_is_exactly_proportional_and_pt_per_unit_gives_it() {
    let mut f = linear();
    quiver(
        &mut f,
        &[0.0, 1.0, 2.0],
        &[0.0, 1.0, 2.0],
        &[3.0, 6.0, 0.0],
        &[4.0, 8.0, 0.5],
        scaled(3.0),
    );
    let drawn = arrows(&f);
    assert!((length(&drawn[0]) - 15.0).abs() < 1e-3);
    assert!((length(&drawn[1]) - 30.0).abs() < 1e-3);
    // Shorter than its head, half a magnitude keeps its true 1.5 pt.
    assert!((length(&drawn[2]) - 1.5).abs() < 1e-3);
}

#[test]
fn the_longest_arrow_is_the_smaller_median_spacing_of_a_grid() {
    // Columns 1 unit apart and rows 2: on this wide plot the columns are not always the closer.
    let (x, y) = ([0.0, 1.0, 2.0, 3.0, 4.0], [0.0, 2.0, 4.0]);
    let mut f = linear();
    let u: Vec<f64> = (0..15).map(|i| 1.0 + f64::from(i % 3)).collect();
    grid_quiver(&mut f, &x, &y, &u, &[0.0; 15], QuiverOptions::default());
    let (resolved, layout, frames) = planned(&f);
    let [x_points, y_points] = points_per_unit(&resolved, layout.plot);
    let spacing = x_points.min(2.0 * y_points);
    assert!((frames[0].pt_per_unit * 3.0 - spacing).abs() < 1e-3 * spacing);
    let longest = arrows(&f).iter().map(|a| length(a)).fold(0.0, f64::max);
    assert!((longest - spacing).abs() < 1e-3, "{longest} {spacing}");
}

#[test]
fn the_median_ignores_one_wide_gap() {
    let mut f = linear();
    grid_quiver(
        &mut f,
        &[0.0, 1.0, 2.0, 3.0, 40.0],
        &[0.0],
        &[1.0; 5],
        &[0.0; 5],
        QuiverOptions::default(),
    );
    let (resolved, layout, frames) = planned(&f);
    let [x_points, _] = points_per_unit(&resolved, layout.plot);
    assert!((frames[0].pt_per_unit - x_points).abs() < 1e-3 * x_points);
}

#[test]
fn points_take_the_median_distance_to_the_nearest_neighbour() {
    // Pairs 3 units apart, each pair far from the others; all with one arrow length.
    let mut f = linear();
    let x = [0.0, 3.0, 30.0, 33.0, 60.0, 63.0];
    let y = [0.0, 0.0, 10.0, 10.0, 0.0, 0.0];
    quiver(
        &mut f,
        &x,
        &y,
        &[0.0; 6],
        &[1.0; 6],
        QuiverOptions::default(),
    );
    let (resolved, layout, frames) = planned(&f);
    let [x_points, _] = points_per_unit(&resolved, layout.plot);
    assert!((frames[0].pt_per_unit - 3.0 * x_points).abs() < 1e-3 * x_points);
}

#[test]
fn a_single_arrow_takes_the_length_of_a_barb_staff() {
    let mut f = linear();
    quiver(
        &mut f,
        &[1.0],
        &[1.0],
        &[3.0],
        &[4.0],
        QuiverOptions::default(),
    );
    assert!((length(&arrows(&f)[0]) - barb::STAFF).abs() < 1e-3);
    // So do points all at one place, with the theme's font scale.
    let theme = Theme::pigment().with_font_scale(1.5).unwrap();
    let mut f = figure_of(6.0, 4.0, theme, LINEAR);
    quiver(
        &mut f,
        &[1.0, 1.0],
        &[2.0, 2.0],
        &[1.0, 2.0],
        &[0.0, 0.0],
        QuiverOptions::default(),
    );
    let longest = arrows(&f).iter().map(|a| length(a)).fold(0.0, f64::max);
    assert!((longest - 1.5 * barb::STAFF).abs() < 1e-3);
}

#[test]
fn quivers_of_one_axes_share_the_automatic_scale() {
    let mut f = linear();
    let x = [0.0, 1.0, 2.0];
    quiver(
        &mut f,
        &x,
        &x,
        &[1.0; 3],
        &[0.0; 3],
        QuiverOptions::default(),
    );
    quiver(
        &mut f,
        &x,
        &x,
        &[4.0; 3],
        &[0.0; 3],
        QuiverOptions::default(),
    );
    quiver(&mut f, &x, &x, &[100.0; 3], &[0.0; 3], scaled(0.1));
    let (_, _, frames) = planned(&f);
    assert_eq!(frames[0].pt_per_unit, frames[1].pt_per_unit);
    assert_eq!(frames[2].pt_per_unit, 0.1);
}

#[test]
fn the_scale_settles_with_the_reach_of_its_own_arrows() {
    // Arrows that reach past the data at both ends change the limits they are measured on.
    let (x, y): (Vec<f64>, Vec<f64>) = (
        (0..8).map(f64::from).collect(),
        (0..5).map(f64::from).collect(),
    );
    let u: Vec<f64> = (0..40).map(|i| f64::from(i % 5) - 2.0).collect();
    let mut f = figure_of(6.0, 4.0, Theme::pigment(), LINEAR);
    grid_quiver(&mut f, &x, &y, &u, &[1.0; 40], QuiverOptions::default());
    let (resolved, layout, frames) = planned(&f);
    let [x_points, y_points] = points_per_unit(&resolved, layout.plot);
    let longest = 2.0_f64.hypot(1.0);
    let spacing = x_points.min(y_points);
    assert!((frames[0].pt_per_unit * longest - spacing).abs() < 1e-3);
    let again = scene(&f);
    assert_eq!(again, scene(&f), "the same figure draws the same scene");
}

#[test]
fn the_screen_direction_ignores_the_axes_and_the_data_direction_follows_them() {
    let (x, y) = ([0.0, 10.0], [0.0, 1.0]);
    let options = |direction| QuiverOptions {
        direction,
        ..scaled(10.0)
    };
    let mut screen = figure_of(8.0, 3.0, Theme::pigment(), LINEAR);
    quiver(
        &mut screen,
        &x,
        &y,
        &[1.0, 1.0],
        &[1.0, 1.0],
        options(Direction::Screen),
    );
    let [dx, dy] = vector(&arrows(&screen)[0]);
    assert!((dx - dy).abs() < 1e-3, "45° on any plot: {dx} {dy}");

    let mut data = figure_of(8.0, 3.0, Theme::pigment(), LINEAR);
    quiver(
        &mut data,
        &x,
        &y,
        &[1.0, 1.0],
        &[1.0, 1.0],
        options(Direction::Data),
    );
    let (resolved, layout, _) = planned(&data);
    let [x_points, y_points] = points_per_unit(&resolved, layout.plot);
    let drawn = arrows(&data);
    let [dx, dy] = vector(&drawn[0]);
    assert!(
        (dy / dx - y_points / x_points).abs() < 1e-3 * (y_points / x_points),
        "from (x, y) towards (x + u, y + v): {dx} {dy}"
    );
    // Only the angle changes: the length follows the magnitude in both modes.
    assert!((length(&drawn[0]) - 10.0 * 2.0_f64.sqrt()).abs() < 1e-3);
}

#[test]
fn both_directions_draw_the_same_arrows_on_equal_aspect() {
    let (x, y) = ([0.0, 1.0, 2.0], [0.0, 3.0]);
    let (u, v) = (
        [1.0, 2.0, 0.5, 1.0, 1.0, 3.0],
        [1.0, 0.0, 2.0, 1.0, 0.0, 3.0],
    );
    let draw = |direction| {
        let mut f = figure_of(8.0, 3.0, Theme::pigment(), LINEAR);
        grid_quiver(
            &mut f,
            &x,
            &y,
            &u,
            &v,
            QuiverOptions {
                direction,
                ..QuiverOptions::default()
            },
        );
        axes(&mut f).equal_aspect().unwrap();
        scene(&f)
    };
    assert_eq!(draw(Direction::Screen), draw(Direction::Data));
}

#[test]
fn data_direction_needs_two_linear_axes() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    for kinds in [
        [log, ScaleKind::Linear],
        [ScaleKind::Linear, ScaleKind::Time],
    ] {
        let mut f = figure_of(6.0, 4.0, Theme::pigment(), kinds);
        let y = match kinds[1] {
            ScaleKind::Time => AxisData::Times(vec![Some(Instant::from_micros(0).unwrap())]),
            _ => numbers(&[1.0]),
        };
        let error = axes(&mut f)
            .quiver(
                numbers(&[1.0]),
                y,
                FieldVectors::points(vec![1.0], vec![1.0]),
                QuiverOptions {
                    direction: Direction::Data,
                    ..QuiverOptions::default()
                },
            )
            .unwrap_err();
        assert!(matches!(error, Error::Usage { .. }), "{error:?}");
        assert_eq!(error.parameter(), Some("direction"));
        assert_eq!(error.hint(), Some("use Direction.SCREEN, or linear axes."));
    }
}

#[test]
fn a_pivot_is_the_part_of_the_arrow_on_the_sample() {
    let draw = |pivot| {
        let mut f = linear();
        axes(&mut f)
            .xlim(0.0, 10.0)
            .unwrap()
            .ylim(0.0, 10.0)
            .unwrap();
        quiver(
            &mut f,
            &[5.0],
            &[5.0],
            &[1.0],
            &[0.0],
            QuiverOptions {
                pivot,
                ..scaled(40.0)
            },
        );
        ends(&arrows(&f)[0])
    };
    let (tail, tip) = draw(Pivot::Tail);
    let (middle_tail, middle_tip) = draw(Pivot::Middle);
    let (tip_tail, tip_tip) = draw(Pivot::Tip);
    assert!((tip[0] - tail[0] - 40.0).abs() < 1e-3);
    assert!((middle_tail[0] - (tail[0] - 20.0)).abs() < 1e-3);
    assert!((middle_tip[0] - (tail[0] + 20.0)).abs() < 1e-3);
    assert!((tip_tail[0] - (tail[0] - 40.0)).abs() < 1e-3);
    assert!((tip_tip[0] - tail[0]).abs() < 1e-3);
}

#[test]
fn a_head_is_fixed_in_size_and_a_short_arrow_shrinks_whole() {
    let mut f = linear();
    quiver(
        &mut f,
        &[0.0, 1.0, 2.0],
        &[0.0, 1.0, 2.0],
        &[40.0, 10.0, 2.0],
        &[0.0, 0.0, 0.0],
        scaled(1.0),
    );
    let drawn = arrows(&f);
    let width = |corners: &[Point]| f64::from((corners[2].y - corners[4].y).abs());
    assert!((width(&drawn[0]) - 4.0).abs() < 1e-3);
    assert!((width(&drawn[1]) - 4.0).abs() < 1e-3);
    let head = |corners: &[Point]| f64::from((corners[3].x - corners[2].x).abs());
    assert!((head(&drawn[0]) - 5.0).abs() < 1e-3);
    assert!((head(&drawn[1]) - 5.0).abs() < 1e-3);
    // Two points long: a head scaled to it, and no shaft.
    assert_eq!(drawn[2].len(), 3);
    assert!((length(&drawn[2]) - 2.0).abs() < 1e-3);
    assert!((f64::from((drawn[2][0].y - drawn[2][2].y).abs()) - 1.6).abs() < 1e-3);
}

/// Every point of `paths`, on the page.
fn every_point(paths: &[&Path]) -> Vec<Point> {
    paths.iter().flat_map(|p| subpaths(p)).flatten().collect()
}

fn assert_inside(points: &[Point], clip: Rect) {
    for p in points {
        assert!(
            p.x >= clip.x - 1e-3
                && p.x <= clip.x + clip.width + 1e-3
                && p.y >= clip.y - 1e-3
                && p.y <= clip.y + clip.height + 1e-3,
            "{p:?} leaves {clip:?}"
        );
    }
}

#[test]
fn automatic_limits_hold_whole_arrows_with_each_pivot() {
    for pivot in [Pivot::Tail, Pivot::Middle, Pivot::Tip] {
        for options in [
            QuiverOptions {
                pivot,
                ..scaled(30.0)
            },
            QuiverOptions {
                pivot,
                ..QuiverOptions::default()
            },
        ] {
            let mut f = linear();
            let x = [0.0, 1.0, 2.0];
            quiver(
                &mut f,
                &x,
                &x,
                &[1.0, -1.0, 2.0],
                &[2.0, 1.0, -1.0],
                options,
            );
            let scene = scene(&f);
            let (clip, items) = marks(&scene);
            assert_inside(&every_point(&fills(items)), clip);
        }
    }
}

#[test]
fn explicit_limits_are_kept_and_clip_the_arrows() {
    let mut f = linear();
    axes(&mut f).xlim(0.0, 1.0).unwrap().ylim(0.0, 1.0).unwrap();
    quiver(&mut f, &[0.5], &[0.5], &[100.0], &[0.0], scaled(10.0));
    let (resolved, layout, _) = planned(&f);
    assert_eq!(resolved.x.continuous().unwrap().domain(), (0.0, 1.0));
    let drawn = arrows(&f);
    let (tail, tip) = ends(&drawn[0]);
    assert!(
        (tip[0] - tail[0] - 1000.0).abs() < 1e-2,
        "drawn at its length"
    );
    assert!(tip[0] > layout.plot.right(), "and cut by the marks clip");
}

#[test]
fn limits_hold_whole_arrows_on_equal_aspect() {
    let mut f = linear();
    let x = [0.0, 1.0, 2.0];
    grid_quiver(
        &mut f,
        &x,
        &x,
        &[2.0, 0.0, -1.0, 1.0, 1.0, 1.0, 2.0, 3.0, 1.0],
        &[1.0, 2.0, 1.0, 1.0, -1.0, -1.0, 0.0, 1.0, 2.0],
        QuiverOptions::default(),
    );
    axes(&mut f).equal_aspect().unwrap();
    let (resolved, layout, frames) = planned(&f);
    let [x_points, y_points] = points_per_unit(&resolved, layout.plot);
    assert!((x_points / y_points - 1.0).abs() < 1e-9);
    let longest = 10.0_f64.sqrt();
    assert!((frames[0].pt_per_unit * longest - x_points).abs() < 1e-3);
    let scene = scene(&f);
    let (clip, items) = marks(&scene);
    assert_inside(&every_point(&fills(items)), clip);
}

#[test]
fn a_mark_of_arrows_too_long_for_its_plot_names_a_figure_that_holds_them() {
    let mut f = linear();
    quiver(
        &mut f,
        &[0.0, 1.0],
        &[0.0, 1.0],
        &[0.0, 0.0],
        &[10.0, 10.0],
        scaled(40.0),
    );
    let error = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("the arrows need a figure at least"),
        "{error}"
    );
    assert!(
        error
            .hint()
            .is_some_and(|hint| hint.contains("pt_per_unit"))
    );
}

#[test]
fn a_pt_per_unit_is_checked_at_the_call() {
    for (scale, part) in [
        (0.0, "greater than 0"),
        (f64::NAN, "greater than 0"),
        (1e9, "at most 14400 pt long"),
    ] {
        let mut f = linear();
        let error = axes(&mut f)
            .quiver(
                numbers(&[0.0]),
                numbers(&[0.0]),
                FieldVectors::points(vec![1.0], vec![0.0]),
                scaled(scale),
            )
            .unwrap_err();
        assert!(matches!(error, Error::Style { .. }), "{error:?}");
        assert!(error.to_string().contains(part), "{error}");
        assert_eq!(error.parameter(), Some("pt_per_unit"));
    }
}

/// `options` with the key `value`, `text`.
fn keyed(value: f64, text: &str, options: QuiverOptions) -> QuiverOptions {
    QuiverOptions {
        key: Some(ArrowKey::new(value, text)),
        ..options
    }
}

/// The arrow swatches of the legend row of `layout`.
fn key_arrows(layout: &Layout) -> Vec<(f32, f64, String)> {
    layout
        .legend
        .entries()
        .iter()
        .filter_map(|entry| match entry.swatch.kind {
            Swatch::Arrow { length } => {
                Some((length, entry.swatch.width, entry.label.text.clone()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_key_is_drawn_at_the_true_length_of_its_value_with_its_text() {
    for options in [scaled(3.0), QuiverOptions::default()] {
        let mut f = linear();
        let x = [0.0, 1.0, 2.0];
        quiver(
            &mut f,
            &x,
            &x,
            &[10.0, 5.0, 2.0],
            &[0.0; 3],
            keyed(10.0, "10 m/s", options),
        );
        let (_, layout, frames) = planned(&f);
        let keys = key_arrows(&layout);
        assert_eq!(keys.len(), 1);
        let (length, slot, text) = &keys[0];
        assert_eq!(text, "10 m/s");
        assert!((f64::from(*length) - frames[0].key_length).abs() < 1e-3);
        assert!((slot - frames[0].pt_per_unit * 10.0).abs() < 1e-3);
        // Its arrow is the longest arrow of the plot, to the point.
        let scene = scene(&f);
        let drawn = arrows_in_legend(&scene);
        let longest = arrows(&f)
            .iter()
            .map(|a| self::length(a))
            .fold(0.0, f64::max);
        assert!((self::length(&drawn[0]) - longest).abs() < 1e-3);
    }
}

/// The arrows of the legend group of a one-panel figure.
fn arrows_in_legend(scene: &Scene) -> Vec<Vec<Point>> {
    scene
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Group { clip: None, items } => Some(items),
            _ => None,
        })
        .flatten()
        .filter_map(|item| match item {
            Item::Fill { path, .. } => {
                let parts = subpaths(path);
                let corners = parts.first()?;
                (parts.len() == 1 && matches!(corners.len(), 3 | 7)).then(|| corners.clone())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_key_works_in_every_position_and_style() {
    let positions = [
        LegendPosition::TopLeft,
        LegendPosition::TopRight,
        LegendPosition::Right,
        LegendPosition::Bottom,
        LegendPosition::Best,
    ];
    for style in [LegendStyle::Header, LegendStyle::Chips] {
        for position in positions {
            for options in [scaled(3.0), QuiverOptions::default()] {
                let mut f = linear();
                let x = [0.0, 1.0, 2.0];
                quiver(
                    &mut f,
                    &x,
                    &x,
                    &[10.0; 3],
                    &[1.0; 3],
                    keyed(10.0, "10 m/s", options),
                );
                axes(&mut f)
                    .legend_at(style, position, SizeSpec::Auto)
                    .unwrap();
                let (_, layout, frames) = planned(&f);
                let keys = key_arrows(&layout);
                assert_eq!(keys.len(), 1, "{style:?} {position:?}");
                assert!(
                    (f64::from(keys[0].0) - 10.0 * frames[0].pt_per_unit).abs() < 1e-3,
                    "{style:?} {position:?}"
                );
                let scene = scene(&f);
                let drawn = arrows_in_legend(&scene);
                assert_eq!(drawn.len(), 1, "{style:?} {position:?}");
            }
        }
    }
}

#[test]
fn a_key_stays_in_the_row_under_direct_labels() {
    let mut f = linear();
    let x = [0.0, 1.0, 2.0];
    quiver(
        &mut f,
        &x,
        &x,
        &[10.0; 3],
        &[1.0; 3],
        keyed(10.0, "10 m/s", QuiverOptions::default()),
    );
    axes(&mut f)
        .legend(LegendStyle::Direct, SizeSpec::Auto)
        .unwrap();
    let (_, layout, _) = planned(&f);
    assert_eq!(key_arrows(&layout).len(), 1);
    assert!(layout.direct.is_empty());
}

#[test]
fn a_legend_beside_the_plot_settles_with_the_key_it_makes_room_for() {
    // The wider the key, the narrower the plot, the shorter the arrows and the key.
    let mut f = linear();
    let x: Vec<f64> = (0..4).map(f64::from).collect();
    quiver(
        &mut f,
        &x,
        &[0.0, 0.0, 0.0, 0.0],
        &[1.0; 4],
        &[0.0; 4],
        keyed(1.0, "1 m/s", QuiverOptions::default()),
    );
    axes(&mut f)
        .legend_at(LegendStyle::Header, LegendPosition::Right, SizeSpec::Auto)
        .unwrap();
    let (resolved, layout, frames) = planned(&f);
    let [x_points, _] = points_per_unit(&resolved, layout.plot);
    assert!((frames[0].pt_per_unit - x_points).abs() < 1e-2);
    let keys = key_arrows(&layout);
    assert!((keys[0].1 - frames[0].key_length).abs() < 1e-4);
}

#[test]
fn a_key_beside_a_plot_of_equal_aspect_stays_on_the_canvas() {
    for position in [LegendPosition::Right, LegendPosition::Bottom] {
        let mut f = figure_of(7.0, 4.0, Theme::pigment(), LINEAR);
        let x: Vec<f64> = (0..9).map(f64::from).collect();
        let y: Vec<f64> = (0..5).map(f64::from).collect();
        grid_quiver(
            &mut f,
            &x,
            &y,
            &[1.0; 45],
            &[0.5; 45],
            keyed(1.0, "1 m/s", QuiverOptions::default()),
        );
        axes(&mut f)
            .equal_aspect()
            .unwrap()
            .legend_at(LegendStyle::Header, position, SizeSpec::Auto)
            .unwrap();
        let (resolved, layout, frames) = planned(&f);
        let [x_points, y_points] = points_per_unit(&resolved, layout.plot);
        assert!((x_points / y_points - 1.0).abs() < 1e-9);
        assert!((frames[0].pt_per_unit * 1.25_f64.sqrt() - x_points).abs() < 1e-3);
        let entry = &layout.legend.entries()[0];
        assert!(
            entry.label.x + entry.label.width() <= 7.0 * 72.0,
            "{position:?}"
        );
    }
}

#[test]
fn a_few_arrows_settle_on_the_limits_they_widen() {
    // Three columns: arrows along x take a large share of the plot, and each widens the limits.
    let mut f = linear();
    let x = [0.0, 1.0, 2.0];
    quiver(
        &mut f,
        &x,
        &[0.0; 3],
        &[1.0; 3],
        &[0.0; 3],
        QuiverOptions::default(),
    );
    let (resolved, layout, frames) = planned(&f);
    let [x_points, _] = points_per_unit(&resolved, layout.plot);
    assert!((frames[0].pt_per_unit - x_points).abs() < 1e-3 * x_points);
}

#[test]
fn arrows_across_a_row_stop_short_of_the_height_of_the_plot() {
    // The spacing along x is far longer than the plot is high.
    let mut f = figure_of(8.0, 2.0, Theme::pigment(), LINEAR);
    let x = [0.0, 1.0];
    quiver(
        &mut f,
        &x,
        &[0.0; 2],
        &[0.0; 2],
        &[1.0; 2],
        QuiverOptions::default(),
    );
    let (_, layout, frames) = planned(&f);
    assert!(frames[0].pt_per_unit <= 0.75 * layout.plot.height + 1e-6);
    let drawn = scene(&f);
    let (clip, items) = marks(&drawn);
    assert_inside(&every_point(&fills(items)), clip);
}

#[test]
fn arrows_pointing_both_ways_across_a_row_share_the_height_of_the_plot() {
    // At three quarters of the height no limits hold both: the search ends on a shorter scale.
    let mut f = figure_of(8.0, 2.0, Theme::pigment(), LINEAR);
    let x = [0.0, 1.0];
    quiver(
        &mut f,
        &x,
        &[0.0; 2],
        &[0.0; 2],
        &[1.0, -1.0],
        QuiverOptions::default(),
    );
    let (_, layout, frames) = planned(&f);
    assert!(frames[0].pt_per_unit < 0.5 * layout.plot.height);
    let drawn = scene(&f);
    let (clip, items) = marks(&drawn);
    assert_inside(&every_point(&fills(items)), clip);
}

#[test]
fn two_quivers_have_two_keys() {
    let mut f = linear();
    let x = [0.0, 1.0];
    quiver(
        &mut f,
        &x,
        &x,
        &[1.0; 2],
        &[0.0; 2],
        keyed(1.0, "1 m/s", scaled(10.0)),
    );
    quiver(
        &mut f,
        &x,
        &x,
        &[0.0; 2],
        &[1.0; 2],
        keyed(2.0, "2 km/h", scaled(5.0)),
    );
    let (_, layout, _) = planned(&f);
    let keys = key_arrows(&layout);
    assert_eq!(keys.len(), 2);
    assert_eq!((keys[0].0, keys[0].2.as_str()), (10.0, "1 m/s"));
    assert_eq!((keys[1].0, keys[1].2.as_str()), (10.0, "2 km/h"));
}

#[test]
fn a_mapped_key_takes_the_text_color_and_a_uniform_one_the_series_color() {
    let theme = Theme::pigment();
    let mut f = figure_of(6.0, 4.0, theme.clone(), LINEAR);
    let x = [0.0, 1.0];
    quiver(
        &mut f,
        &x,
        &x,
        &[1.0; 2],
        &[0.0; 2],
        QuiverOptions {
            color: PointColor::By(ColorBy::new(vec![0.0, 1.0])),
            ..keyed(1.0, "1 m/s", scaled(10.0))
        },
    );
    quiver(
        &mut f,
        &x,
        &x,
        &[0.0; 2],
        &[1.0; 2],
        keyed(1.0, "up", scaled(10.0)),
    );
    let (_, layout, _) = planned(&f);
    let entries = layout.legend.entries();
    assert_eq!(entries[0].swatch.color, theme.text_color());
    assert_eq!(entries[1].swatch.color, theme.palette()[0]);
}

#[test]
fn a_key_longer_than_the_shorter_side_of_the_plot_is_refused() {
    let positions = [
        LegendPosition::TopLeft,
        LegendPosition::Right,
        LegendPosition::Bottom,
        LegendPosition::Best,
    ];
    for position in positions {
        let mut f = linear();
        let x = [0.0, 1.0];
        quiver(
            &mut f,
            &x,
            &x,
            &[1.0; 2],
            &[0.0; 2],
            keyed(1000.0, "1000 m/s", scaled(3.0)),
        );
        axes(&mut f)
            .legend_at(LegendStyle::Header, position, SizeSpec::Auto)
            .unwrap();
        let error = f.to_scene(FontLibrary::global()).unwrap_err();
        assert!(matches!(error, Error::Style { .. }), "{error:?}");
        assert!(
            error
                .to_string()
                .contains("the arrow of the key \"1000 m/s\" is 3000 pt long"),
            "{position:?}: {error}"
        );
        assert_eq!(
            error.hint(),
            Some("pass a smaller value to key, or make the figure larger.")
        );
    }
}

#[test]
fn a_key_on_arrows_without_a_length_has_no_scale() {
    let mut f = linear();
    let x = [0.0, 1.0];
    quiver(
        &mut f,
        &x,
        &x,
        &[0.0; 2],
        &[0.0; 2],
        keyed(1.0, "1 m/s", QuiverOptions::default()),
    );
    let error = f.to_scene(FontLibrary::global()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Figure.to_scene(): the key \"1 m/s\" has no scale: none of the arrows has a length."
    );
    assert_eq!(error.hint(), Some("pass pt_per_unit, or remove the key."));
}

#[test]
fn a_bad_key_is_refused_at_the_call() {
    let cases = [
        (ArrowKey::new(0.0, "0"), "greater than 0"),
        (ArrowKey::new(f64::NAN, "none"), "greater than 0"),
        (ArrowKey::new(1.0, ""), "the key text is empty"),
        (ArrowKey::new(1.0, "two\nlines"), "one line"),
    ];
    for (key, part) in cases {
        let mut f = linear();
        let error = axes(&mut f)
            .quiver(
                numbers(&[0.0]),
                numbers(&[0.0]),
                FieldVectors::points(vec![1.0], vec![0.0]),
                QuiverOptions {
                    key: Some(key),
                    ..QuiverOptions::default()
                },
            )
            .unwrap_err();
        assert!(matches!(error, Error::Style { .. }), "{error:?}");
        assert!(error.to_string().contains(part), "{error}");
        assert_eq!(error.parameter(), Some("key"));
    }
}

#[test]
fn keys_of_different_lengths_stay_apart_in_a_merged_legend() {
    use crate::{Grid, GridIndex, LegendMode, Placement};
    let cells = Grid::new(1, 2).unwrap().with_legend(LegendMode::Merged);
    let mut f = Figure::with_grid(8.0, 4.0, Theme::pigment(), cells).unwrap();
    for (col, scale) in [(0, 10.0), (1, 20.0)] {
        let place = Placement {
            col: GridIndex::At(col),
            ..Placement::default()
        };
        f.add_axes_at(place, ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .quiver(
                numbers(&[0.0, 1.0]),
                numbers(&[0.0, 1.0]),
                FieldVectors::points(vec![1.0, 1.0], vec![0.0, 0.0]),
                keyed(1.0, "1 m/s", scaled(scale)),
            )
            .unwrap();
    }
    let scene = scene(&f);
    let lengths: Vec<f64> = arrows_in_legend_of_figure(&scene)
        .iter()
        .map(|arrow| length(arrow))
        .collect();
    assert_eq!(lengths.len(), 2, "{lengths:?}");
    assert!((lengths[0] - 10.0).abs() < 1e-3 && (lengths[1] - 20.0).abs() < 1e-3);
}

/// The arrows drawn anywhere outside a marks group.
fn arrows_in_legend_of_figure(scene: &Scene) -> Vec<Vec<Point>> {
    fn walk(items: &[Item], found: &mut Vec<Vec<Point>>) {
        for item in items {
            match item {
                Item::Group { clip: None, items } => walk(items, found),
                Item::Fill { path, .. } => {
                    let parts = subpaths(path);
                    if parts.len() == 1 && parts[0].len() == 7 {
                        found.extend(parts);
                    }
                }
                _ => {}
            }
        }
    }
    let mut found = Vec::new();
    walk(&scene.items, &mut found);
    found
}

#[test]
fn a_color_mapping_draws_one_item_per_color() {
    let mut f = linear();
    let x: Vec<f64> = (0..100).map(f64::from).collect();
    let values: Vec<f64> = (0..100).map(|i| f64::from(i % 2)).collect();
    quiver(
        &mut f,
        &x,
        &x,
        &[1.0; 100],
        &[0.0; 100],
        QuiverOptions {
            color: PointColor::By(ColorBy::new(values)),
            ..scaled(5.0)
        },
    );
    let scene = scene(&f);
    let (_, items) = marks(&scene);
    assert_eq!(items.len(), 2);
    let counts: Vec<_> = fills(items)
        .iter()
        .map(|path| subpaths(path).len())
        .collect();
    assert_eq!(counts, [50, 50]);
}

#[test]
fn a_color_mapping_has_the_shape_of_u() {
    let mut f = linear();
    let error = axes(&mut f)
        .quiver(
            numbers(&[0.0, 1.0]),
            numbers(&[0.0, 1.0]),
            FieldVectors::grid(vec![1.0; 4], vec![1.0; 4]),
            QuiverOptions {
                color: PointColor::By(ColorBy::new(vec![1.0, 2.0])),
                ..QuiverOptions::default()
            },
        )
        .unwrap_err();
    assert!(matches!(error, Error::Data { .. }), "{error:?}");
    assert_eq!(
        error.to_string(),
        "Axes.quiver(): color has 2 values but u has 4; color must have the shape of u."
    );
    assert_eq!(error.parameter(), Some("color"));
}

#[test]
fn a_mapped_nan_leaves_an_arrow_undrawn_and_keeps_the_scale() {
    let mut f = linear();
    let x = [0.0, 1.0, 2.0];
    quiver(
        &mut f,
        &x,
        &x,
        &[1.0, 2.0, 4.0],
        &[0.0; 3],
        QuiverOptions {
            color: PointColor::By(ColorBy::new(vec![1.0, 1.0, f64::NAN])),
            ..QuiverOptions::default()
        },
    );
    assert_eq!(arrows(&f).len(), 2);
}

#[test]
fn a_mapped_color_near_the_background_moves_away_from_it() {
    for theme in [Theme::pigment(), Theme::midnight(), Theme::graphite()] {
        let mut f = figure_of(6.0, 4.0, theme.clone(), LINEAR);
        let by = ColorBy {
            center: Some(0.0),
            ..ColorBy::new(vec![-1.0, 0.0, 1.0])
        };
        let x = [0.0, 1.0, 2.0];
        quiver(
            &mut f,
            &x,
            &x,
            &[1.0; 3],
            &[0.0; 3],
            QuiverOptions {
                color: PointColor::By(by),
                ..scaled(20.0)
            },
        );
        let scene = scene(&f);
        let (_, items) = marks(&scene);
        let middle = theme.diverging_cmap().at(0.5);
        let colors: Vec<Rgba> = items
            .iter()
            .filter_map(|item| match item {
                Item::Fill {
                    paint: Paint::Solid(color),
                    ..
                } => Some(*color),
                _ => None,
            })
            .collect();
        assert_eq!(colors.len(), 3);
        assert_ne!(colors[1], middle);
        let lightness = |color| crate::cmap::to_oklab(color)[0];
        let background = lightness(theme.background());
        assert!(
            (lightness(colors[1]) - background).abs() > (lightness(middle) - background).abs(),
            "the color moved away from the background"
        );
    }
}

#[test]
fn one_color_near_the_background_keeps_its_color() {
    let theme = Theme::pigment();
    let mut f = figure_of(6.0, 4.0, theme.clone(), LINEAR);
    quiver(
        &mut f,
        &[0.0, 1.0],
        &[0.0, 1.0],
        &[1.0; 2],
        &[0.0; 2],
        QuiverOptions {
            color: PointColor::Uniform(ColorSpec::Fixed(theme.background())),
            ..scaled(20.0)
        },
    );
    let scene = scene(&f);
    let (_, items) = marks(&scene);
    assert!(matches!(
        &items[0],
        Item::Fill { paint: Paint::Solid(color), .. } if *color == theme.background()
    ));
}

#[test]
fn arrows_hold_their_limits_on_log_and_time_axes() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let mut f = figure_of(6.0, 4.0, Theme::pigment(), [log, log]);
    grid_quiver(
        &mut f,
        &[1.0, 10.0, 100.0],
        &[1.0, 100.0],
        &[1.0; 6],
        &[1.0; 6],
        QuiverOptions::default(),
    );
    let (resolved, layout, frames) = planned(&f);
    let (lo, hi) = resolved.x.continuous().unwrap().domain();
    // A decade is the spacing along x, on the log axis.
    let decade = layout.plot.width / (hi / lo).log10();
    let (ylo, yhi) = resolved.y.continuous().unwrap().domain();
    let two_decades = 2.0 * layout.plot.height / (yhi / ylo).log10();
    let spacing = decade.min(two_decades);
    assert!((frames[0].pt_per_unit * 2.0_f64.sqrt() - spacing).abs() < 1e-2);
    let drawn = scene(&f);
    let (clip, items) = marks(&drawn);
    assert_inside(&every_point(&fills(items)), clip);

    let mut f = figure_of(
        6.0,
        4.0,
        Theme::pigment(),
        [ScaleKind::Time, ScaleKind::Linear],
    );
    let hour = 3_600_000_000;
    let times = (0..4)
        .map(|i| Some(Instant::from_micros(i * hour).unwrap()))
        .collect();
    axes(&mut f)
        .quiver(
            AxisData::Times(times),
            numbers(&[0.0, 1.0]),
            FieldVectors::grid(vec![1.0; 8], vec![1.0; 8]),
            QuiverOptions::default(),
        )
        .unwrap();
    let drawn = scene(&f);
    let (clip, items) = marks(&drawn);
    assert_eq!(subpaths(fills(items)[0]).len(), 8);
    assert_inside(&every_point(&fills(items)), clip);
}

#[test]
fn a_category_axis_is_refused() {
    for kinds in [
        [ScaleKind::Category, ScaleKind::Linear],
        [ScaleKind::Linear, ScaleKind::Category],
    ] {
        let mut f = figure_of(6.0, 4.0, Theme::pigment(), kinds);
        let error = axes(&mut f)
            .quiver(
                numbers(&[0.0]),
                numbers(&[0.0]),
                FieldVectors::points(vec![1.0], vec![0.0]),
                QuiverOptions::default(),
            )
            .unwrap_err();
        assert!(matches!(error, Error::Usage { .. }), "{error:?}");
        assert_eq!(
            error.hint(),
            Some("use Scale.linear(), Scale.log() or Scale.time() for both axes.")
        );
        let error = axes(&mut f)
            .barbs(
                numbers(&[0.0]),
                numbers(&[0.0]),
                FieldVectors::points(vec![1.0], vec![0.0]),
                BarbOptions::default(),
            )
            .unwrap_err();
        assert!(matches!(error, Error::Usage { .. }), "{error:?}");
    }
}

/// The scene of one barb of `(u, v)` on explicit limits.
fn barb(u: f64, v: f64, options: BarbOptions) -> Scene {
    let mut f = linear();
    axes(&mut f)
        .xlim(-1.0, 1.0)
        .unwrap()
        .ylim(-1.0, 1.0)
        .unwrap();
    axes(&mut f)
        .barbs(
            numbers(&[0.0]),
            numbers(&[0.0]),
            FieldVectors::points(vec![u], vec![v]),
            options,
        )
        .unwrap();
    scene(&f)
}

#[test]
fn each_speed_from_0_to_65_draws_its_flags_and_lines() {
    // The staff and lines are one stroked path, the flags one filled path.
    let expected: [(usize, usize); 14] = [
        (1, 0),
        (2, 0),
        (2, 0),
        (3, 0),
        (3, 0),
        (4, 0),
        (4, 0),
        (5, 0),
        (5, 0),
        (6, 0),
        (1, 1),
        (2, 1),
        (2, 1),
        (3, 1),
    ];
    for (i, (lines, flags)) in expected.into_iter().enumerate() {
        let speed = 5.0 * i as f64;
        let scene = barb(0.0, -speed, BarbOptions::default());
        let (_, items) = marks(&scene);
        let stroked = strokes(items);
        assert_eq!(stroked.len(), 1, "speed {speed}");
        assert_eq!(subpaths(stroked[0]).len(), lines, "speed {speed}");
        let filled = fills(items);
        assert_eq!(filled.len(), flags, "speed {speed}");
        if let Some(path) = filled.first() {
            assert_eq!(subpaths(path).len(), flags, "speed {speed}");
        }
    }
}

#[test]
fn a_speed_is_rounded_to_the_nearest_five_halves_up() {
    let drawn = |speed| barb(speed, 0.0, BarbOptions::default());
    assert_eq!(drawn(2.5), drawn(5.0));
    assert_ne!(drawn(2.4), drawn(5.0));
    assert_eq!(drawn(2.4), drawn(0.0));
    assert_eq!(drawn(7.5), drawn(10.0));
    assert_eq!(drawn(7.4), drawn(5.0));
    assert_eq!(drawn(47.5), drawn(50.0));
    assert_eq!(drawn(47.4), drawn(45.0));
}

#[test]
fn calm_is_an_open_circle_at_the_sample() {
    let scene = barb(0.0, 1.0, BarbOptions::default());
    let (_, items) = marks(&scene);
    assert!(fills(items).is_empty());
    let (min, max) = strokes(items)[0].bounds().unwrap();
    assert!((f64::from(max.x - min.x) - 6.0).abs() < 1e-3);
    assert!((f64::from(max.y - min.y) - 6.0).abs() < 1e-3);
}

/// The staff of the barb of `(u, v)` on the page: from the sample to the outer end.
fn staff(u: f64, v: f64, options: BarbOptions) -> (Point, Point) {
    let scene = barb(u, v, options);
    let (_, items) = marks(&scene);
    let parts = subpaths(strokes(items)[0]);
    (parts[0][0], parts[0][1])
}

#[test]
fn the_staff_runs_from_the_sample_against_the_wind() {
    // A wind blowing to the right comes from the left: the staff runs left.
    let (start, end) = staff(10.0, 0.0, BarbOptions::default());
    assert!((f64::from(start.x - end.x) - barb::STAFF).abs() < 1e-3);
    assert_eq!(start.y, end.y);
    // A wind blowing up comes from below: the staff runs down the page.
    let (start, end) = staff(0.0, 10.0, BarbOptions::default());
    assert!((f64::from(end.y - start.y) - barb::STAFF).abs() < 1e-3);
    // Barbs read the screen: u == v is 45° on any axes.
    let (start, end) = staff(10.0, 10.0, BarbOptions::default());
    assert!((f64::from(start.x - end.x) - f64::from(end.y - start.y)).abs() < 1e-3);
}

#[test]
fn lines_lie_on_the_right_of_the_staff_and_flip_moves_them() {
    let tip = |flip| {
        let scene = barb(
            0.0,
            -10.0,
            BarbOptions {
                flip,
                ..BarbOptions::default()
            },
        );
        let (_, items) = marks(&scene);
        let parts = subpaths(strokes(items)[0]);
        (parts[0][0], parts[1][1])
    };
    // The staff runs up the page: its right is the right of the page.
    let (sample, right) = tip(false);
    assert!((f64::from(right.x - sample.x) - LINE_RISE * barb::STAFF).abs() < 1e-3);
    let (sample, left) = tip(true);
    assert!((f64::from(sample.x - left.x) - LINE_RISE * barb::STAFF).abs() < 1e-3);
}

#[test]
fn a_size_is_the_length_of_the_staff_and_the_rest_scale_with_it() {
    let sized = |size| BarbOptions {
        size,
        ..BarbOptions::default()
    };
    let (start, end) = staff(10.0, 0.0, sized(SizeSpec::pt(48.0).unwrap()));
    assert!((f64::from(start.x - end.x) - 48.0).abs() < 1e-3);
    let (start, end) = staff(10.0, 0.0, sized(SizeSpec::times(0.5).unwrap()));
    assert!((f64::from(start.x - end.x) - 12.0).abs() < 1e-3);
    let scene = barb(10.0, 0.0, sized(SizeSpec::pt(48.0).unwrap()));
    let (_, items) = marks(&scene);
    let Item::Stroke { style, .. } = &items[0] else {
        panic!("a stroke");
    };
    assert!((f64::from(style.width) - 48.0 * barb::STROKE).abs() < 1e-4);
}

#[test]
fn an_automatic_staff_follows_the_font_scale() {
    let theme = Theme::pigment().with_font_scale(2.0).unwrap();
    let mut f = figure_of(6.0, 4.0, theme, LINEAR);
    axes(&mut f)
        .xlim(-1.0, 1.0)
        .unwrap()
        .ylim(-1.0, 1.0)
        .unwrap();
    axes(&mut f)
        .barbs(
            numbers(&[0.0]),
            numbers(&[0.0]),
            FieldVectors::points(vec![10.0], vec![0.0]),
            BarbOptions::default(),
        )
        .unwrap();
    let scene = scene(&f);
    let (_, items) = marks(&scene);
    let parts = subpaths(strokes(items)[0]);
    assert!((f64::from(parts[0][0].x - parts[0][1].x) - 48.0).abs() < 1e-3);
}

#[test]
fn a_time_height_section_holds_whole_barbs() {
    let hour = 3_600_000_000;
    let mut f = figure_of(
        6.0,
        4.0,
        Theme::pigment(),
        [ScaleKind::Time, ScaleKind::Linear],
    );
    let times = (0..6)
        .map(|i| Some(Instant::from_micros(i * 6 * hour).unwrap()))
        .collect();
    let heights = [500.0, 1000.0, 2000.0, 4000.0];
    let u: Vec<f64> = (0..24).map(|i| 3.0 * f64::from(i)).collect();
    axes(&mut f)
        .barbs(
            AxisData::Times(times),
            numbers(&heights),
            FieldVectors::grid(u, vec![-10.0; 24]),
            BarbOptions {
                label: "Wind (kn)".into(),
                ..BarbOptions::default()
            },
        )
        .unwrap();
    let scene = scene(&f);
    let (clip, items) = marks(&scene);
    assert_inside(&every_point(&strokes(items)), clip);
    assert_inside(&every_point(&fills(items)), clip);
    let lines = subpaths(strokes(items)[0]);
    assert!(lines.iter().filter(|line| line.len() == 2).count() > 24);
    // A label takes a line swatch.
    let (_, layout, _) = planned(&f);
    let entries = layout.legend.entries();
    assert_eq!(entries.len(), 1);
    assert!(matches!(entries[0].swatch.kind, Swatch::Line { .. }));
}

#[test]
fn a_mapped_barb_mark_draws_each_color_once() {
    let mut f = linear();
    let x = [0.0, 1.0, 2.0, 3.0];
    axes(&mut f)
        .barbs(
            numbers(&x),
            numbers(&x),
            FieldVectors::points(vec![60.0; 4], vec![0.0; 4]),
            BarbOptions {
                color: PointColor::By(ColorBy::new(vec![0.0, 1.0, 0.0, 1.0])),
                ..BarbOptions::default()
            },
        )
        .unwrap();
    let scene = scene(&f);
    let (_, items) = marks(&scene);
    assert_eq!(strokes(items).len(), 2);
    assert_eq!(fills(items).len(), 2);
}

#[test]
fn barbs_are_listed_in_the_row_under_direct_labels() {
    let mut f = linear();
    axes(&mut f)
        .barbs(
            numbers(&[0.0, 1.0]),
            numbers(&[0.0, 1.0]),
            FieldVectors::points(vec![10.0; 2], vec![0.0; 2]),
            BarbOptions {
                label: "Wind".into(),
                ..BarbOptions::default()
            },
        )
        .unwrap()
        .legend(LegendStyle::Direct, SizeSpec::Auto)
        .unwrap();
    let (_, layout, _) = planned(&f);
    assert_eq!(layout.legend.entries().len(), 1);
    assert!(layout.direct.is_empty());
}

#[test]
fn barbs_too_large_for_the_plot_are_named_as_barbs() {
    let mut f = figure_of(2.0, 1.5, Theme::pigment(), LINEAR);
    axes(&mut f)
        .barbs(
            numbers(&[0.0, 1.0]),
            numbers(&[0.0, 1.0]),
            FieldVectors::points(vec![0.0, 0.0], vec![10.0, 10.0]),
            BarbOptions {
                size: SizeSpec::pt(72.0).unwrap(),
                ..BarbOptions::default()
            },
        )
        .unwrap();
    let error = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("the barbs need a figure at least"),
        "{error}"
    );
}

#[test]
fn a_figure_without_samples_draws_nothing_for_them() {
    let mut f = linear();
    quiver(&mut f, &[], &[], &[], &[], QuiverOptions::default());
    axes(&mut f)
        .barbs(
            numbers(&[]),
            numbers(&[]),
            FieldVectors::points(vec![], vec![]),
            BarbOptions::default(),
        )
        .unwrap();
    assert!(marks_are_empty(&scene(&f)));
}

/// Whether a scene has no marks group, or an empty one.
fn marks_are_empty(scene: &Scene) -> bool {
    !scene
        .items
        .iter()
        .any(|item| matches!(item, Item::Group { clip: Some(_), items } if !items.is_empty()))
}
