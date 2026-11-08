//! Streamlines on whole figures: which lines a field draws, where they end, their heads and
//! colors, and how they hold the limits.

use crate::field::stream::{
    END_SPACING, HEAD_LENGTH_MIN, MAX_STEP, MIN_LENGTH, MIN_STEP, STEPS_PER_DENSITY,
};
use crate::geometry::{Path, PathVerb, Point};
use crate::layout::Layout;
use crate::legend::Swatch;
use crate::pipeline::plan_panel_marks;
use crate::resolve::Resolved;
use crate::scene::{Item, LineCap, Rect, Scene};
use crate::{
    AxesMut, ColorBy, ColorbarOptions, Error, Figure, FontLibrary, LogBase, Mark, PointColor,
    ScaleKind, StreamStarts, Streamline, StreamplotMark, StreamplotOptions, SvgText, Theme,
};

const LINEAR: [ScaleKind; 2] = [ScaleKind::Linear, ScaleKind::Linear];

fn figure_of(width: f64, height: f64, kinds: [ScaleKind; 2]) -> Figure {
    let mut figure = Figure::new(width, height, Theme::pigment()).unwrap();
    figure.add_axes(kinds[0], kinds[1]).unwrap();
    figure
}

fn linear() -> Figure {
    figure_of(6.0, 4.0, LINEAR)
}

fn axes(figure: &mut Figure) -> AxesMut<'_> {
    figure.single_axes_mut()
}

/// `count` values from `low` to `high`.
fn spread(low: f64, high: f64, count: u32) -> Vec<f64> {
    let step = (high - low) / f64::from(count - 1);
    (0..count).map(|i| low + step * f64::from(i)).collect()
}

/// The field `f(x, y) = (u, v)` on the grid of `x` and `y`, row by row.
fn sampled(x: &[f64], y: &[f64], f: impl Fn(f64, f64) -> (f64, f64)) -> (Vec<f64>, Vec<f64>) {
    y.iter()
        .flat_map(|&y| x.iter().map(move |&x| (x, y)))
        .map(|(x, y)| f(x, y))
        .unzip()
}

/// The field around a cylinder of radius 1 at the origin in a uniform flow along x, NaN inside.
fn cylinder(x: &[f64], y: &[f64]) -> (Vec<f64>, Vec<f64>) {
    sampled(x, y, |x, y| {
        let square = x * x + y * y;
        if square < 1.0 {
            return (f64::NAN, f64::NAN);
        }
        let fourth = square * square;
        (1.0 - (x * x - y * y) / fourth, -2.0 * x * y / fourth)
    })
}

fn options(density: f64) -> StreamplotOptions {
    StreamplotOptions {
        density,
        ..StreamplotOptions::default()
    }
}

fn starting(x: &[f64], y: &[f64]) -> StreamplotOptions {
    StreamplotOptions {
        starts: StreamStarts::At {
            x: x.to_vec(),
            y: y.to_vec(),
        },
        ..StreamplotOptions::default()
    }
}

/// Adds streamlines and returns the mark.
fn streamplot(
    figure: &mut Figure,
    (x, y): (&[f64], &[f64]),
    (u, v): (&[f64], &[f64]),
    options: StreamplotOptions,
) -> StreamplotMark {
    let mut axes = axes(figure);
    axes.streamplot(x, y, u, v, options).unwrap();
    match axes.marks().last() {
        Some(Mark::Streamplot(mark)) => mark.clone(),
        other => panic!("not streamlines: {other:?}"),
    }
}

/// The error of adding streamlines.
fn refused(
    figure: &mut Figure,
    (x, y): (&[f64], &[f64]),
    (u, v): (&[f64], &[f64]),
    options: StreamplotOptions,
) -> Error {
    let before = axes(figure).marks().len();
    let error = axes(figure).streamplot(x, y, u, v, options).unwrap_err();
    assert_eq!(
        axes(figure).marks().len(),
        before,
        "an error changes nothing"
    );
    error
}

/// The positions of the points of `line`, in steps, on a linear grid from `x0` and `y0`, `per`
/// data units per step along each axis.
fn in_steps(line: &Streamline, [x0, y0]: [f64; 2], [px, py]: [f64; 2]) -> Vec<[f64; 2]> {
    line.points()
        .iter()
        .map(|&[x, y]| [(x - x0) / px, (y - y0) / py])
        .collect()
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
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

/// The subpaths of `path`, each as its on-curve points.
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

/// A call that fails: `x`, `y`, `u`, `v`, the parameter named and the message.
type Case<'a> = (&'a [f64], &'a [f64], &'a [f64], &'a [f64], &'a str, &'a str);

#[test]
fn wrong_shapes_and_coordinates_are_data_errors_naming_the_parameter() {
    let mut f = linear();
    let (x, y) = ([0.0, 1.0, 2.0], [0.0, 1.0]);
    let cases: [Case<'_>; 7] = [
        (
            &x,
            &y,
            &[1.0; 5],
            &[1.0; 5],
            "u",
            "Axes.streamplot(): u has 5 values; it must have len(y) * len(x) = 2 * 3 = 6.",
        ),
        (
            &x,
            &y,
            &[1.0; 6],
            &[1.0; 5],
            "v",
            "Axes.streamplot(): u has 6 values but v has 5; u and v must have the same shape.",
        ),
        (
            &[0.0, 2.0, 1.0],
            &y,
            &[1.0; 6],
            &[1.0; 6],
            "x",
            "Axes.streamplot(): x[2] is 1, not above x[1] = 2; coordinates must be strictly \
             increasing.",
        ),
        (
            &[0.0, f64::NAN, 2.0],
            &y,
            &[1.0; 6],
            &[1.0; 6],
            "x",
            "Axes.streamplot(): x[1] is NaN; coordinates must be finite.",
        ),
        (
            &x,
            &[0.0, f64::INFINITY],
            &[1.0; 6],
            &[1.0; 6],
            "y",
            "Axes.streamplot(): y[1] is inf; values must be finite.",
        ),
        (
            &[0.0],
            &y,
            &[1.0; 2],
            &[1.0; 2],
            "x",
            "Axes.streamplot(): x has 1 coordinate; a streamplot grid needs at least two along \
             each axis.",
        ),
        (
            &x,
            &y,
            &[1.0, 1.0, 1.0, 1.0, f64::INFINITY, 1.0],
            &[1.0; 6],
            "u",
            "Axes.streamplot(): u[1][1] is inf; values must be finite.",
        ),
    ];
    for (x, y, u, v, parameter, message) in cases {
        let error = refused(&mut f, (x, y), (u, v), StreamplotOptions::default());
        assert!(matches!(error, Error::Data { .. }), "{error:?}");
        assert_eq!(error.parameter(), Some(parameter), "{error}");
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn time_and_category_axes_are_refused() {
    for (kinds, message, parameter) in [
        (
            [ScaleKind::Time, ScaleKind::Linear],
            "Axes.streamplot(): the x axis is a time axis; streamlines need linear or log axes.",
            "x",
        ),
        (
            [ScaleKind::Linear, ScaleKind::Category],
            "Axes.streamplot(): the y axis is a category axis; streamlines need linear or log \
             axes.",
            "y",
        ),
    ] {
        let mut f = figure_of(6.0, 4.0, kinds);
        let error = refused(
            &mut f,
            (&[0.0, 1.0], &[0.0, 1.0]),
            (&[1.0; 4], &[0.0; 4]),
            StreamplotOptions::default(),
        );
        assert!(matches!(error, Error::Usage { .. }), "{error:?}");
        assert_eq!(error.to_string(), message);
        assert_eq!(error.parameter(), Some(parameter));
    }
}

#[test]
fn a_density_outside_its_range_is_a_style_error() {
    let mut f = linear();
    for density in [0.0, 0.05, 10.5, f64::NAN, f64::INFINITY] {
        let error = refused(
            &mut f,
            (&[0.0, 1.0], &[0.0, 1.0]),
            (&[1.0; 4], &[0.0; 4]),
            options(density),
        );
        assert!(matches!(error, Error::Style { .. }), "{error:?}");
        assert_eq!(error.parameter(), Some("density"));
    }
    let error = refused(
        &mut f,
        (&[0.0, 1.0], &[0.0, 1.0]),
        (&[1.0; 4], &[0.0; 4]),
        options(0.0),
    );
    assert_eq!(
        error.to_string(),
        "Axes.streamplot(): density is 0; it must be from 0.1 to 10."
    );
    for density in [0.1, 10.0] {
        streamplot(
            &mut f,
            (&[0.0, 1.0], &[0.0, 1.0]),
            (&[1.0; 4], &[0.0; 4]),
            options(density),
        );
    }
}

#[test]
fn a_uniform_field_gives_evenly_spaced_straight_lines() {
    let mut f = linear();
    let (x, y) = (spread(0.0, 3.0, 4), spread(0.0, 1.0, 11));
    let (u, v) = sampled(&x, &y, |_, _| (2.0, 0.0));
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), options(1.0));
    let per = [3.0 / STEPS_PER_DENSITY, 1.0 / STEPS_PER_DENSITY];
    let mut rows: Vec<f64> = mark
        .lines()
        .iter()
        .map(|line| {
            let points = in_steps(line, [0.0, 0.0], per);
            let row = points[0][1];
            // Straight along x, from one edge to the other.
            assert!(points.iter().all(|p| p[1] == row), "{points:?}");
            assert!(points.windows(2).all(|pair| pair[1][0] > pair[0][0]));
            assert!(points[0][0] < 2.0 * MIN_STEP, "{:?}", points[0]);
            assert!(points[points.len() - 1][0] > STEPS_PER_DENSITY - 2.0 * MIN_STEP);
            row
        })
        .collect();
    rows.sort_by(f64::total_cmp);
    assert_eq!(rows.len(), 30);
    for (i, row) in rows.iter().enumerate() {
        assert!((row - (i as f64 + 0.5)).abs() < 1e-9, "{rows:?}");
    }
}

#[test]
fn uneven_spacing_keeps_a_uniform_field_straight() {
    let mut f = linear();
    let (x, y) = ([0.0, 0.1, 0.5, 2.0, 4.0], [0.0, 0.3, 1.0, 3.0]);
    let (u, v) = sampled(&x, &y, |_, _| (1.0, 0.5));
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), options(1.0));
    assert!(mark.lines().len() > 10);
    for line in mark.lines() {
        let [x0, y0] = line.points()[0];
        for &[x, y] in line.points() {
            assert!((y - y0 - 0.5 * (x - x0)).abs() < 1e-12, "{x} {y}");
        }
    }
}

#[test]
fn lines_end_at_a_hole_and_never_enter_it() {
    let mut f = linear();
    let (x, y) = (spread(0.0, 10.0, 11), spread(0.0, 10.0, 11));
    let (mut u, v) = sampled(&x, &y, |_, _| (1.0, 0.0));
    // A NaN at (5, 5) opens the hole 4 < x < 6, 4 < y < 6.
    u[5 * 11 + 5] = f64::NAN;
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), options(1.0));
    let inside = |&[x, y]: &[f64; 2]| x > 4.0 + 1e-9 && x < 6.0 - 1e-9 && y > 4.0 && y < 6.0;
    let mut ended = 0;
    for line in mark.lines() {
        assert!(!line.points().iter().any(inside), "{:?}", line.points());
        let last = line.points()[line.points().len() - 1];
        if (4.0..6.0).contains(&last[1]) && last[0] < 5.0 {
            // A line running into the hole ends at its edge.
            assert!((last[0] - 4.0).abs() < 1e-3, "{last:?}");
            ended += 1;
        }
    }
    assert!(ended >= 4, "{ended}");
    // A NaN in a mapped color value makes the same hole.
    let (u, v) = sampled(&x, &y, |_, _| (1.0, 0.0));
    let mut values = vec![1.0; 121];
    values[5 * 11 + 5] = f64::NAN;
    let mapped = streamplot(
        &mut linear(),
        (&x, &y),
        (&u, &v),
        StreamplotOptions {
            color: PointColor::By(ColorBy::new(values)),
            ..StreamplotOptions::default()
        },
    );
    let points = |mark: &StreamplotMark| -> Vec<Vec<[f64; 2]>> {
        mark.lines()
            .iter()
            .map(|line| line.points().to_vec())
            .collect()
    };
    assert_eq!(points(&mapped), points(&mark));
}

#[test]
fn a_rotation_gives_closed_loops_that_end() {
    let mut f = linear();
    let (x, y) = (spread(-1.0, 1.0, 21), spread(-1.0, 1.0, 21));
    let (u, v) = sampled(&x, &y, |x, y| (-y, x));
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), options(1.0));
    let per = 2.0 / STEPS_PER_DENSITY;
    let mut loops = 0;
    for line in mark.lines() {
        let radii: Vec<f64> = line.points().iter().map(|&[x, y]| x.hypot(y)).collect();
        let largest = radii.iter().copied().fold(0.0, f64::max);
        if largest > 0.95 {
            continue;
        }
        loops += 1;
        // On the circle it started on, within a thousandth of its radius.
        let first = radii[0];
        assert!(
            radii.iter().all(|r| (r - first).abs() < 1e-3 * first),
            "{radii:?}"
        );
        // Once round, and closed up to the spacing.
        let turn = 2.0 * std::f64::consts::PI * first / per;
        assert!(
            line.length() > turn - 1.0 && line.length() < turn,
            "{}",
            line.length()
        );
        let ends = distance(line.points()[0], line.points()[line.points().len() - 1]) / per;
        assert!(ends <= END_SPACING + MAX_STEP, "{ends}");
    }
    assert!(loops >= 10, "{loops}");
}

#[test]
fn a_source_and_a_sink_end_where_the_speed_is_zero() {
    let (x, y) = (spread(-1.0, 1.0, 21), spread(-1.0, 1.0, 21));
    let per = 2.0 / STEPS_PER_DENSITY;
    for sign in [1.0, -1.0] {
        let mut f = linear();
        let (u, v) = sampled(&x, &y, |x, y| (sign * x, sign * y));
        let mark = streamplot(&mut f, (&x, &y), (&u, &v), starting(&[0.6], &[0.3]));
        let points = mark.lines()[0].points();
        // Upstream of a source and downstream of a sink lies the origin.
        let at_origin = if sign > 0.0 {
            points[0]
        } else {
            points[points.len() - 1]
        };
        assert!(
            at_origin[0].hypot(at_origin[1]) / per < 0.01,
            "{at_origin:?}"
        );
        // The other end is on the edge, along the ray through the start.
        let edge = if sign > 0.0 {
            points[points.len() - 1]
        } else {
            points[0]
        };
        assert!(
            (edge[0] - 1.0).abs() < 1e-3 && (edge[1] - 0.5).abs() < 1e-3,
            "{edge:?}"
        );
        // A start on the point where the flow stops draws nothing, and says so.
        let error = refused(&mut f, (&x, &y), (&u, &v), starting(&[0.0], &[0.0]));
        assert_eq!(
            error.to_string(),
            "Axes.streamplot(): the speed is zero at starts[0] at (0, 0); no line passes there."
        );
    }
}

#[test]
fn no_two_automatic_lines_come_closer_than_half_a_step() {
    let mut f = linear();
    let (x, y) = (spread(-3.0, 3.0, 61), spread(-2.0, 2.0, 41));
    let (u, v) = cylinder(&x, &y);
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), options(1.0));
    let per = [6.0 / STEPS_PER_DENSITY, 4.0 / STEPS_PER_DENSITY];
    let lines: Vec<Vec<[f64; 2]>> = mark
        .lines()
        .iter()
        .map(|line| in_steps(line, [-3.0, -2.0], per))
        .collect();
    let mut closest = f64::INFINITY;
    for (i, a) in lines.iter().enumerate() {
        for b in &lines[i + 1..] {
            for &p in a {
                for &q in b {
                    closest = closest.min(distance(p, q));
                }
            }
        }
    }
    assert!(closest >= END_SPACING, "{closest}");
    assert!(
        closest < 0.6,
        "lines do come close where the flow converges: {closest}"
    );
    assert!(mark.lines().iter().all(|line| line.length() >= MIN_LENGTH));
}

#[test]
fn density_changes_the_count_and_figure_size_changes_nothing() {
    let (x, y) = (spread(-3.0, 3.0, 61), spread(-2.0, 2.0, 41));
    let (u, v) = cylinder(&x, &y);
    let counts: Vec<usize> = [0.5, 1.0, 2.0]
        .into_iter()
        .map(|density| {
            let mark = streamplot(&mut linear(), (&x, &y), (&u, &v), options(density));
            mark.lines().len()
        })
        .collect();
    assert!(counts[0] < counts[1] && counts[1] < counts[2], "{counts:?}");
    let small = streamplot(
        &mut figure_of(3.0, 2.0, LINEAR),
        (&x, &y),
        (&u, &v),
        options(1.0),
    );
    let mut wide = figure_of(12.0, 3.0, LINEAR);
    let large = streamplot(&mut wide, (&x, &y), (&u, &v), options(1.0));
    axes(&mut wide).equal_aspect().unwrap();
    assert_eq!(small.lines(), large.lines());
}

#[test]
fn the_same_input_draws_the_same_lines_and_pinned_points() {
    let draw = || {
        let mut f = linear();
        let (x, y) = (
            vec![-2.0, -1.5, -0.7, 0.0, 0.4, 1.1, 2.0],
            vec![-1.0, -0.2, 0.5, 1.0],
        );
        let (u, v) = sampled(&x, &y, |x, y| (1.0 - y * y + 0.3 * x, 0.6 * x - 0.2 * y));
        let mark = streamplot(&mut f, (&x, &y), (&u, &v), options(0.5));
        (f, mark)
    };
    let (first, mark) = draw();
    let (second, again) = draw();
    assert_eq!(mark.lines(), again.lines());
    assert_eq!(first.to_png(72).unwrap(), second.to_png(72).unwrap());
    assert_eq!(
        first.to_svg(SvgText::Outlines).unwrap(),
        second.to_svg(SvgText::Outlines).unwrap()
    );
    // Points pinned to their bits: a different platform or integrator shows here.
    let bits = |line: usize, point: usize| {
        let [x, y] = mark.lines()[line].points()[point];
        [x.to_bits(), y.to_bits()]
    };
    let pins = PINS;
    assert_eq!(mark.lines().len(), pins.0);
    let counts: Vec<usize> = [0, 2, 24]
        .map(|line| mark.lines()[line].points().len())
        .into();
    assert_eq!(counts, [55, 51, 99]);
    for &(line, point, expected) in pins.1 {
        assert_eq!(bits(line, point), expected, "line {line} point {point}");
    }
}

/// A point pinned to its bits: line, point, `[x, y]`.
type Pin = (usize, usize, [u64; 2]);

/// The number of lines of the field of `the_same_input_draws_the_same_lines_and_pinned_points`,
/// and some of their points.
const PINS: (usize, &[Pin]) = (
    25,
    &[
        (0, 0, [13835058039812354821, 4601676632307505571]),
        (0, 1, [13835058020891086694, 4601676047123623716]),
        (0, 27, [13834241892654899192, 13821943227258726190]),
        (0, 54, [13834625805468435243, 13830554428779167632]),
        (2, 0, [13833811292332938980, 4603035619859107686]),
        (2, 1, [13833785065355079593, 4602736548190851709]),
        (2, 25, [13832354520325793507, 13822015987285144708]),
        (2, 50, [13832233215512487752, 13830554399481583457]),
        (24, 0, [13829873068854694405, 4607182223735094508]),
        (24, 1, [13829873280744088485, 4607181646978686636]),
        (24, 49, [4582308498644150626, 4591829656596033160]),
        (24, 98, [4610848903535031185, 4607182298394125634]),
    ],
);

#[test]
fn a_start_draws_one_line_regardless_of_spacing() {
    let mut f = linear();
    let (x, y) = (spread(0.0, 1.0, 11), spread(0.0, 1.0, 11));
    let (u, v) = sampled(&x, &y, |_, _| (1.0, 0.0));
    let mark = streamplot(
        &mut f,
        (&x, &y),
        (&u, &v),
        starting(&[0.5, 0.5, 0.2], &[0.5, 0.501, 0.9]),
    );
    assert_eq!(mark.lines().len(), 3);
    for (line, y) in mark.lines().iter().zip([0.5, 0.501, 0.9]) {
        let points = line.points();
        assert!(points.iter().all(|p| p[1] == y));
        assert!(points[0][0] < 1e-3 && points[points.len() - 1][0] > 1.0 - 1e-3);
    }
}

#[test]
fn starts_outside_the_grid_in_a_hole_or_mismatched_are_data_errors() {
    let mut f = linear();
    let (x, y) = (spread(0.0, 1.0, 11), spread(0.0, 2.0, 11));
    let (mut u, v) = sampled(&x, &y, |_, _| (1.0, 0.0));
    u[5 * 11 + 5] = f64::NAN;
    let cases: [(&[f64], &[f64], &str); 5] = [
        (
            &[0.5, 1.5],
            &[0.5, 0.5],
            "Axes.streamplot(): starts[1] at (1.5, 0.5) lies outside the grid, which spans x 0 to \
             1 and y 0 to 2.",
        ),
        (
            &[0.5],
            &[1.0],
            "Axes.streamplot(): starts[0] at (0.5, 1) lies in a hole: a cell with a NaN corner in \
             u, v or color.",
        ),
        (
            &[0.5, 0.6],
            &[0.5],
            "Axes.streamplot(): starts has 2 x coordinates but 1 y coordinates; they must have \
             the same length.",
        ),
        (
            &[],
            &[],
            "Axes.streamplot(): starts is empty; pass at least one point.",
        ),
        (
            &[0.5],
            &[f64::NAN],
            "Axes.streamplot(): starts[0] is (0.5, NaN); a start must be finite.",
        ),
    ];
    for (xs, ys, message) in cases {
        let error = refused(&mut f, (&x, &y), (&u, &v), starting(xs, ys));
        assert!(matches!(error, Error::Data { .. }), "{error:?}");
        assert_eq!(error.parameter(), Some("starts"));
        assert_eq!(error.to_string(), message);
    }
    // A start on the edge of the grid is on the grid.
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), starting(&[1.0], &[0.2]));
    assert_eq!(mark.lines().len(), 1);
}

/// The head triangles of a one-color mark: the subpaths of its filled item.
fn heads(figure: &Figure) -> Vec<Vec<Point>> {
    let scene = scene(figure);
    let (_, items) = marks(&scene);
    items
        .iter()
        .filter_map(|item| match item {
            Item::Fill { path, .. } => Some(subpaths(path)),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn a_head_sits_at_the_middle_pointing_downstream() {
    for sign in [1.0, -1.0] {
        let mut f = linear();
        let (x, y) = (spread(0.0, 1.0, 11), spread(0.0, 1.0, 11));
        let (u, v) = sampled(&x, &y, |_, _| (sign, 0.0));
        streamplot(&mut f, (&x, &y), (&u, &v), starting(&[0.3], &[0.5]));
        let (resolved, layout) = planned(&f);
        let found = heads(&f);
        assert_eq!(found.len(), 1);
        let corners = &found[0];
        assert_eq!(corners.len(), 3);
        // The middle of a line from x = 0 to 1 is at 0.5; the tip is 2.5 pt downstream of it.
        let plot = layout.plot;
        let (x_low, x_high) = resolved.x.continuous().unwrap().domain();
        let (y_low, y_high) = resolved.y.continuous().unwrap().domain();
        let middle = [
            plot.x + (0.5 - x_low) / (x_high - x_low) * plot.width,
            plot.y + plot.height - (0.5 - y_low) / (y_high - y_low) * plot.height,
        ];
        let tip = corners[1];
        assert!(
            (f64::from(tip.x) - (middle[0] + sign * 2.5)).abs() < 0.01,
            "{tip:?}"
        );
        assert!((f64::from(tip.y) - middle[1]).abs() < 0.01);
        for base in [corners[0], corners[2]] {
            assert!((f64::from(base.x) - (middle[0] - sign * 2.5)).abs() < 0.01);
            assert!(((f64::from(base.y) - middle[1]).abs() - 2.0).abs() < 0.01);
        }
    }
}

#[test]
fn arrows_can_be_left_out_and_a_short_line_carries_none() {
    let (x, y) = (spread(0.0, 1.0, 11), spread(0.0, 1.0, 11));
    let (u, v) = sampled(&x, &y, |_, _| (1.0, 0.3));
    let mut f = linear();
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), options(1.0));
    assert_eq!(heads(&f).len(), mark.lines().len());
    let mut f = linear();
    streamplot(
        &mut f,
        (&x, &y),
        (&u, &v),
        StreamplotOptions {
            arrows: false,
            ..StreamplotOptions::default()
        },
    );
    let scene = scene(&f);
    let (_, items) = marks(&scene);
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], Item::Stroke { .. }));
    // An open cell 0.3 steps wide between the edge and a hole holds a line too short for a head.
    let (x, y) = ([0.0, 0.01, 0.02, 1.0], [0.0, 1.0]);
    let (u, v) = ([1.0, 1.0, f64::NAN, 1.0, 1.0, 1.0, f64::NAN, 1.0], [0.0; 8]);
    let mut f = linear();
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), starting(&[0.005], &[0.5]));
    let line = &mark.lines()[0];
    assert!(line.length() < HEAD_LENGTH_MIN, "{}", line.length());
    assert!(line.head().is_none());
    assert!(heads(&f).is_empty());
}

#[test]
fn one_color_is_one_stroke_and_one_fill() {
    let mut f = linear();
    let (x, y) = (spread(-3.0, 3.0, 61), spread(-2.0, 2.0, 41));
    let (u, v) = cylinder(&x, &y);
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), options(2.0));
    let scene = scene(&f);
    let (clip, items) = marks(&scene);
    assert_eq!(items.len(), 2);
    let Item::Stroke { path, style } = &items[0] else {
        panic!("the lines first");
    };
    assert_eq!(style.cap, LineCap::Round);
    assert_eq!(style.width, crate::contour::LINE_WIDTH);
    assert_eq!(subpaths(path).len(), mark.lines().len());
    // Every point lies within the clip, give or take the line margin.
    for point in subpaths(path).iter().flatten() {
        assert!(point.x >= clip.x - 3.0 && point.x <= clip.x + clip.width + 3.0);
        assert!(point.y >= clip.y - 3.0 && point.y <= clip.y + clip.height + 3.0);
    }
}

#[test]
fn a_color_mapping_draws_overlapping_pieces_and_a_colorbar() {
    let mut f = linear();
    let (x, y) = (spread(0.0, 1.0, 11), spread(0.0, 1.0, 11));
    let (u, v) = sampled(&x, &y, |_, _| (1.0, 0.0));
    let (values, _) = sampled(&x, &y, |x, _| (x, 0.0));
    let mark = streamplot(
        &mut f,
        (&x, &y),
        (&u, &v),
        StreamplotOptions {
            color: PointColor::By(ColorBy::new(values)),
            starts: StreamStarts::At {
                x: vec![0.5],
                y: vec![0.5],
            },
            ..StreamplotOptions::default()
        },
    );
    axes(&mut f).colorbar(ColorbarOptions::default()).unwrap();
    let line = &mark.lines()[0];
    assert_eq!(line.values().len(), line.points().len());
    assert!(
        line.values()
            .iter()
            .zip(line.points())
            .all(|(v, p)| (v - p[0]).abs() < 1e-12)
    );
    let scene = scene(&f);
    let (_, items) = marks(&scene);
    let strokes: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            Item::Stroke { path, style } => Some((subpaths(path), style)),
            _ => None,
        })
        .collect();
    assert!(strokes.len() > 10, "a color per piece: {}", strokes.len());
    assert!(strokes.iter().all(|(_, style)| style.cap == LineCap::Butt));
    // Pieces half a step long, each starting a quarter of a step after the one before: the line
    // is 30 steps long.
    let pieces: usize = strokes.iter().map(|(paths, _)| paths.len()).sum();
    assert_eq!(pieces, 120);
    // The head comes last, over the lines.
    assert!(matches!(items.last(), Some(Item::Fill { .. })));
    let (resolved, layout) = planned(&f);
    assert!(layout.colorbar.is_some());
    assert!(resolved.color.is_some());
    // A mapped mark has no legend entry.
    assert!(layout.legend.entries().is_empty());
}

#[test]
fn a_color_mapping_has_the_shape_of_u_and_no_label() {
    let mut f = linear();
    let (x, y) = ([0.0, 1.0, 2.0], [0.0, 1.0]);
    let error = refused(
        &mut f,
        (&x, &y),
        (&[1.0; 6], &[0.0; 6]),
        StreamplotOptions {
            color: PointColor::By(ColorBy::new(vec![1.0; 5])),
            ..StreamplotOptions::default()
        },
    );
    assert!(matches!(error, Error::Data { .. }));
    assert_eq!(error.parameter(), Some("color"));
    assert_eq!(
        error.to_string(),
        "Axes.streamplot(): color has 5 values but u has 6; color must have the shape of u."
    );
    let error = refused(
        &mut f,
        (&x, &y),
        (&[1.0; 6], &[0.0; 6]),
        StreamplotOptions {
            color: PointColor::By(ColorBy::new(vec![1.0; 6])),
            label: "Flow".into(),
            ..StreamplotOptions::default()
        },
    );
    assert!(matches!(error, Error::Usage { .. }));
    assert_eq!(error.parameter(), Some("label"));
    assert_eq!(
        error.to_string(),
        "Axes.streamplot(): label is \"Flow\"; color-mapped streamlines have no legend entry."
    );
}

#[test]
fn a_label_gives_a_line_swatch() {
    let mut f = linear();
    let (x, y) = ([0.0, 1.0, 2.0], [0.0, 1.0]);
    streamplot(
        &mut f,
        (&x, &y),
        (&[1.0; 6], &[0.2; 6]),
        StreamplotOptions {
            label: "Flow".into(),
            ..StreamplotOptions::default()
        },
    );
    let (_, layout) = planned(&f);
    let entries = layout.legend.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label.text, "Flow");
    assert!(matches!(
        entries[0].swatch.kind,
        Swatch::Line { width, .. } if width == crate::contour::LINE_WIDTH
    ));
}

#[test]
fn a_log_axis_measures_steps_in_logarithms() {
    let mut f = figure_of(
        6.0,
        4.0,
        [ScaleKind::Log { base: LogBase::TEN }, ScaleKind::Linear],
    );
    let (x, y) = ([1.0, 10.0, 100.0, 1000.0], [0.0, 1.0]);
    // Uniform in the logarithm of x: d(ln x)/dt = u / x = 1.
    let (u, v) = sampled(&x, &y, |x, _| (x, 0.0));
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), starting(&[2.0], &[0.5]));
    let line = &mark.lines()[0];
    let points = line.points();
    assert!(
        (points[0][0] - 1.0).abs() < 1e-3 && (points[points.len() - 1][0] - 1000.0).abs() < 1.0
    );
    assert!(
        (line.length() - STEPS_PER_DENSITY).abs() < 1e-3,
        "{}",
        line.length()
    );
    // The middle in steps is the middle in logarithms: 10^1.5.
    let head = line.head().unwrap();
    let [from, to] = [points[head.segment][0], points[head.segment + 1][0]];
    let middle = from + (to - from) * head.fraction;
    assert!((middle / 1000_f64.sqrt() - 1.0).abs() < 1e-2, "{middle}");
    // Automatic lines are evenly spaced on the log axis too.
    let mark = streamplot(&mut f, (&x, &y), (&u, &v), options(1.0));
    assert_eq!(mark.lines().len(), 30);
}

#[test]
fn limits_are_the_outer_coordinates_even_past_a_hole() {
    let mut f = linear();
    let (x, y) = ([-1.0, 0.0, 2.0], [0.5, 1.0, 3.0]);
    let mut u = vec![1.0; 9];
    let v = vec![0.5; 9];
    // The whole first column is NaN, so no line comes near x = -1.
    for row in 0..3 {
        u[row * 3] = f64::NAN;
    }
    streamplot(&mut f, (&x, &y), (&u, &v), StreamplotOptions::default());
    let (resolved, _) = planned(&f);
    assert_eq!(resolved.x.continuous().unwrap().domain(), (-1.0, 2.0));
    assert_eq!(resolved.y.continuous().unwrap().domain(), (0.5, 3.0));
    // On equal aspect the limits stay, and the plot takes their shape.
    axes(&mut f).equal_aspect().unwrap();
    let (resolved, layout) = planned(&f);
    assert_eq!(resolved.x.continuous().unwrap().domain(), (-1.0, 2.0));
    let ratio = (layout.plot.width / 3.0) / (layout.plot.height / 2.5);
    assert!((ratio - 1.0).abs() < 1e-9, "{ratio}");
}

/// The single line through `start` of the field `(u, v)` on the grid of `x` and `y`, on axes of
/// the scale kinds `kinds`.
fn through(
    kinds: [ScaleKind; 2],
    (x, y): (&[f64], &[f64]),
    (u, v): (&[f64], &[f64]),
    color: PointColor,
    start: [f64; 2],
) -> Streamline {
    let mut f = figure_of(6.0, 4.0, kinds);
    let options = StreamplotOptions {
        color,
        ..starting(&[start[0]], &[start[1]])
    };
    let mark = streamplot(&mut f, (x, y), (u, v), options);
    assert_eq!(mark.lines().len(), 1);
    mark.lines()[0].clone()
}

#[test]
fn a_step_never_jumps_a_narrow_hole() {
    let narrow = [0.0, 0.501, 0.502, 0.503, 1.0];
    let ones = [1.0; 10];
    // A NaN at 0.502 makes the cells from 0.501 to 0.503 a hole, across the flow along x.
    let mut across = ones;
    across[2] = f64::NAN;
    across[7] = f64::NAN;
    let line = through(
        LINEAR,
        (&narrow, &[0.0, 1.0]),
        (&across, &[0.0; 10]),
        PointColor::AUTO,
        [0.5, 0.5],
    );
    let last = line.points()[line.points().len() - 1];
    assert!(line.points().iter().all(|p| p[0] <= 0.501), "{last:?}");
    assert!(last[0] > 0.501 - 1e-4, "{last:?}");
    // The same along y.
    let along_y = [1.0, 1.0, 1.0, 1.0, f64::NAN, f64::NAN, 1.0, 1.0, 1.0, 1.0];
    let line = through(
        LINEAR,
        (&[0.0, 1.0], &narrow),
        (&[0.0; 10], &along_y),
        PointColor::AUTO,
        [0.5, 0.5],
    );
    assert!(line.points().iter().all(|p| p[1] <= 0.501));
    // Slanting across a hole on an uneven grid.
    let uneven = [0.0, 0.3, 0.3001, 0.3002, 1.0];
    let (u, v) = (vec![1.0; 10], vec![0.5; 10]);
    let mut slanting = u.clone();
    slanting[2] = f64::NAN;
    slanting[7] = f64::NAN;
    let line = through(
        LINEAR,
        (&uneven, &[0.0, 1.0]),
        (&slanting, &v),
        PointColor::AUTO,
        [0.1, 0.2],
    );
    assert!(
        line.points().iter().all(|p| p[0] <= 0.3),
        "{:?}",
        line.points()
    );
    // A NaN of the mapped colors is a hole too.
    let mut colors = vec![1.0; 10];
    colors[2] = f64::NAN;
    let line = through(
        LINEAR,
        (&narrow, &[0.0, 1.0]),
        (&u, &[0.0; 10]),
        PointColor::By(ColorBy::new(colors)),
        [0.5, 0.5],
    );
    assert!(line.points().iter().all(|p| p[0] <= 0.501));
}

#[test]
fn a_line_ends_where_the_flow_stops_without_turning_back() {
    let (x, y) = ([0.0, 0.5, 1.0], [0.0, 1.0]);
    // u = |2x - 1| is zero along x = 0.5 and positive on both sides.
    let line = through(
        LINEAR,
        (&x, &y),
        (&[1.0, 0.0, 1.0, 1.0, 0.0, 1.0], &[0.0; 6]),
        PointColor::AUTO,
        [0.501, 0.5],
    );
    let points = line.points();
    assert!(points.iter().all(|p| p[0] >= 0.5), "{points:?}");
    assert!(points[0][0] < 0.5 + 1e-4, "{:?}", points[0]);
    assert!(points[points.len() - 1][0] > 1.0 - 1e-4);
    // u = 1 - 2x changes sign there: the flow runs into the line from the left.
    let line = through(
        LINEAR,
        (&x, &y),
        (&[1.0, 0.0, -1.0, 1.0, 0.0, -1.0], &[0.0; 6]),
        PointColor::AUTO,
        [0.25, 0.5],
    );
    let points = line.points();
    assert!(points[0][0] < 1e-4 && points.iter().all(|p| p[0] <= 0.5));
    assert!(points[points.len() - 1][0] > 0.5 - 1e-4);
}

#[test]
fn directions_hold_at_any_range_of_the_data() {
    // Steps of 1e200 / 30 along x and 1e-200 / 30 along y: the flow runs diagonally in steps.
    let line = through(
        LINEAR,
        (&[0.0, 1e200], &[0.0, 1e-200]),
        (&[1e200; 4], &[1e-200; 4]),
        PointColor::AUTO,
        [5e199, 5e-201],
    );
    let (first, last) = (line.points()[0], line.points()[line.points().len() - 1]);
    assert!(
        first[0] / 1e200 < 1e-3 && first[1] / 1e-200 < 1e-3,
        "{first:?}"
    );
    assert!(
        last[0] / 1e200 > 1.0 - 1e-3 && last[1] / 1e-200 > 1.0 - 1e-3,
        "{last:?}"
    );
    assert!((line.length() - STEPS_PER_DENSITY * 2_f64.sqrt()).abs() < 1e-3);
    // A log axis over a ratio of 1 + 1e-8 has 3e309 steps per data unit at its start.
    let log = ScaleKind::Log { base: LogBase::TEN };
    let (low, high) = (1e-300, 1.000_000_01e-300);
    let line = through(
        [log, ScaleKind::Linear],
        (&[low, high], &[0.0, 1.0]),
        (&[1.0; 4], &[0.0; 4]),
        PointColor::AUTO,
        [1.000_000_005e-300, 0.5],
    );
    let (first, last) = (line.points()[0], line.points()[line.points().len() - 1]);
    assert!(line.points().len() > 100);
    assert!((first[0] / low - 1.0).abs() < 1e-11 && (last[0] / high - 1.0).abs() < 1e-11);
    assert!(
        (line.length() - STEPS_PER_DENSITY).abs() < 1e-3,
        "{}",
        line.length()
    );
}

#[test]
fn small_and_large_orbits_close_after_one_turn() {
    let (x, y) = ([-1.0, 0.0, 1.0], [-1.0, 0.0, 1.0]);
    let (u, v) = sampled(&x, &y, |x, y| (-y, x));
    let per = 2.0 / STEPS_PER_DENSITY;
    for radius in [0.001, 0.3 * per, 5.0 * per] {
        let line = through(LINEAR, (&x, &y), (&u, &v), PointColor::AUTO, [radius, 0.0]);
        let radii: Vec<f64> = line.points().iter().map(|&[x, y]| x.hypot(y)).collect();
        assert!(
            radii.iter().all(|r| (r / radius - 1.0).abs() < 1e-3),
            "{radius}: {radii:?}"
        );
        let turn = 2.0 * std::f64::consts::PI * radius / per;
        let length = line.length();
        if turn < 2.0 {
            // Shorter than the reach of its own points: closed when it has turned once round.
            assert!(
                (length / turn - 1.0).abs() < 0.05,
                "{radius}: {length} {turn}"
            );
        } else {
            assert!(
                length > turn - 1.0 && length <= turn,
                "{radius}: {length} {turn}"
            );
        }
    }
}
