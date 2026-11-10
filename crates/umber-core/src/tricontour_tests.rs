//! Contours on triangle meshes on whole figures: the checks of `tricontour` and their order,
//! the axes it takes, its levels, colors, colorbar and level labels shared with `contour`, the
//! color scale it shares with `tripcolor` and `mesh`, its limits, and how it is drawn: bands
//! and lines as items, crossings on the drawn edges on a log axis, and the same drawing for any
//! order and winding of the triangles.

use crate::geometry::{Path, PathVerb, Point};
use crate::layout::Layout;
use crate::pipeline::plan_panel_marks;
use crate::resolve::Resolved;
use crate::scene::{FillRule, Item, Scene};
use crate::{
    Cmap, CmapChoice, ColorSpec, ColorbarOptions, ContourLevels, ContourOptions, ContourStyle,
    Error, Figure, FontLibrary, LogBase, Mark, MeshOptions, Rgba, ScaleKind, Theme, TickFormat,
    Transform, Triangles, TripcolorOptions, TriplotOptions,
};

const LINEAR: [ScaleKind; 2] = [ScaleKind::Linear, ScaleKind::Linear];
const LOG_X: [ScaleKind; 2] = [ScaleKind::Log { base: LogBase::TEN }, ScaleKind::Linear];

fn figure_of(kinds: [ScaleKind; 2]) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure.add_axes(kinds[0], kinds[1]).unwrap();
    figure
}

/// A unit square around a centre point: four triangles meeting at point 4.
fn fan() -> (Vec<f64>, Vec<f64>) {
    (vec![0.0, 1.0, 1.0, 0.0, 0.5], vec![0.0, 0.0, 1.0, 1.0, 0.5])
}

/// The values of [`fan`]: low at the corners, high at the centre.
const PEAK: [f64; 5] = [0.0, 1.0, 2.0, 1.0, 4.0];

fn lines() -> ContourOptions {
    ContourOptions::default()
}

fn styled(style: ContourStyle) -> ContourOptions {
    ContourOptions {
        style,
        ..ContourOptions::default()
    }
}

/// The error of adding a contour of `z` on the mesh `(x, y)` with `options` to `f`, which must
/// leave the figure unchanged.
fn rejected(
    f: &mut Figure,
    (x, y, triangles): (&[f64], &[f64], Triangles<'_>),
    z: &[f64],
    options: ContourOptions,
) -> Error {
    let before = f.clone();
    let error = f
        .single_axes_mut()
        .tricontour(x, y, triangles, z, options)
        .unwrap_err();
    assert_eq!(*f, before);
    error
}

#[test]
fn values_levels_and_drawing_options_are_checked_in_order() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    let red = ColorSpec::Fixed(Rgba::RED);
    type Case = (Vec<f64>, ContourOptions, &'static str, &'static str);
    let cases: Vec<Case> = vec![
        (
            PEAK[..4].to_vec(),
            lines(),
            "Axes.tricontour(): z has 4 values; it must have one per point, 5.",
            "z",
        ),
        (
            vec![0.0, 1.0, f64::INFINITY, 1.0, 4.0],
            lines(),
            "Axes.tricontour(): z[2] is inf; values must be finite.",
            "z",
        ),
        (
            PEAK.to_vec(),
            ContourOptions {
                scale: Transform::Log(LogBase::TEN),
                ..lines()
            },
            "Axes.tricontour(): z[0] is 0; values on a log scale must be positive.",
            "z",
        ),
        (
            PEAK.to_vec(),
            ContourOptions {
                levels: ContourLevels::Count(0),
                ..lines()
            },
            "Axes.tricontour(): levels is 0; a count of levels must be between 1 and 64.",
            "levels",
        ),
        (
            PEAK.to_vec(),
            ContourOptions {
                color: red,
                ..styled(ContourStyle::Filled)
            },
            "Axes.tricontour(): color is given with style=Contour.FILLED; filled bands take \
             their colors from the colormap.",
            "color",
        ),
        (
            PEAK.to_vec(),
            ContourOptions {
                annotate: true,
                ..styled(ContourStyle::Filled)
            },
            "Axes.tricontour(): annotate is True with style=Contour.FILLED; level labels need \
             lines.",
            "annotate",
        ),
        (
            PEAK.to_vec(),
            ContourOptions {
                format: Some(TickFormat::fixed(1).unwrap()),
                ..lines()
            },
            "Axes.tricontour(): format is given but annotate is false; only annotated contours \
             show level labels.",
            "format",
        ),
    ];
    for (z, options, text, parameter) in cases {
        let error = rejected(&mut f, (&x, &y, Triangles::Auto), &z, options);
        assert_eq!(
            (error.to_string().as_str(), error.parameter()),
            (text, Some(parameter))
        );
    }
    let error = rejected(
        &mut f,
        (&x, &y, Triangles::Auto),
        &PEAK,
        ContourOptions {
            scale: Transform::Log(LogBase::TEN),
            center: Some(1.0),
            ..lines()
        },
    );
    // The values are checked first: 0 is not positive.
    assert_eq!(error.parameter(), Some("z"));
    let error = rejected(
        &mut f,
        (&x, &y, Triangles::Auto),
        &[1.0; 5],
        ContourOptions {
            scale: Transform::Log(LogBase::TEN),
            center: Some(1.0),
            ..lines()
        },
    );
    assert!(matches!(error, Error::Style { .. }), "{error:?}");
    assert_eq!(error.parameter(), Some("center"));
    // NaN is a hole, not an error.
    f.single_axes_mut()
        .tricontour(
            &x,
            &y,
            Triangles::Auto,
            &[0.0, f64::NAN, 2.0, 1.0, 4.0],
            lines(),
        )
        .unwrap();
}

#[test]
fn points_and_triangles_are_checked_as_for_triplot_and_before_the_values() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    let error = rejected(
        &mut f,
        (&x, &y, Triangles::Given(&[[0, 1, 5]])),
        &[f64::INFINITY; 5],
        lines(),
    );
    assert_eq!(
        error.to_string(),
        "Axes.tricontour(): triangles[0][2] is 5; indices must be from 0 to 4, one per point."
    );
    let error = rejected(&mut f, (&x[..4], &y, Triangles::Auto), &PEAK, lines());
    assert_eq!(error.parameter(), Some("y"));
    let error = rejected(
        &mut f,
        (&[0.0, 1.0, 2.0], &[0.0, 1.0, 2.0], Triangles::Auto),
        &[1.0; 3],
        lines(),
    );
    assert!(error.to_string().contains("lie on one line"), "{error}");
}

#[test]
fn numeric_axes_linear_or_log_and_no_others() {
    let (x, y) = fan();
    for (kinds, parameter) in [
        ([ScaleKind::Category, ScaleKind::Linear], "x"),
        ([ScaleKind::Linear, ScaleKind::Time], "y"),
    ] {
        let mut f = figure_of(kinds);
        let error = rejected(&mut f, (&x, &y, Triangles::Auto), &PEAK, lines());
        assert!(matches!(error, Error::Usage { .. }), "{error:?}");
        assert_eq!(error.parameter(), Some(parameter));
        assert!(error.to_string().starts_with("Axes.tricontour(): "));
    }
    let mut f = figure_of(LOG_X);
    let error = rejected(&mut f, (&x, &y, Triangles::Auto), &PEAK, lines());
    assert_eq!(
        error.to_string(),
        "Axes.tricontour(): x[0] is 0; coordinates on a log axis must be positive."
    );
    let shifted: Vec<f64> = x.iter().map(|x| 10.0_f64.powf(x + 1.0)).collect();
    f.single_axes_mut()
        .tricontour(&shifted, &y, Triangles::Auto, &PEAK, lines())
        .unwrap();
    let both = [ScaleKind::Log { base: LogBase::TEN }; 2];
    let mut f = figure_of(both);
    let lifted: Vec<f64> = y.iter().map(|y| y + 1.0).collect();
    f.single_axes_mut()
        .tricontour(&shifted, &lifted, Triangles::Auto, &PEAK, lines())
        .unwrap();
    scene(&f);
}

fn scene(figure: &Figure) -> Scene {
    figure.to_scene(FontLibrary::global()).unwrap()
}

/// The items of the marks group of a one-panel figure.
fn marks(scene: &Scene) -> Vec<Item> {
    scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group {
                clip: Some(_),
                items,
            } => Some(items.clone()),
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

/// The resolved axes and layout of the only panel of `figure`.
fn planned(figure: &Figure) -> (Resolved, Layout) {
    let mut panels = plan_panel_marks(figure, FontLibrary::global());
    let (resolved, layout, _) = panels.remove(0);
    (resolved, layout)
}

#[test]
fn levels_and_colors_come_from_the_values_drawn() {
    // A point that no triangle uses, with a value far from the others: it counts in the
    // limits, but not in the levels or the colors, which come from what is drawn.
    let (x, y) = ([0.0, 1.0, 0.0, 2.0], [0.0, 0.0, 1.0, 2.0]);
    let z = [0.0, 1.0, 2.0, 1e6];
    let mut f = figure_of(LINEAR);
    f.single_axes_mut()
        .tricontour(&x, &y, Triangles::Given(&[[0, 1, 2]]), &z, lines())
        .unwrap()
        .contour(&[0.0, 1.0], &[0.0, 1.0], &[0.0, 1.0, 2.0, 1.0], lines())
        .unwrap();
    let [Mark::Tricontour(mesh), Mark::Contour(grid)] = f.single_axes().marks() else {
        unreachable!()
    };
    assert_eq!(mesh.levels(), grid.levels());
    assert!(
        mesh.levels()
            .iter()
            .all(|&level| 0.0 < level && level < 2.0)
    );
    assert_eq!(mesh.z(), z.as_slice());
    assert_eq!(mesh.options(), &lines());
    let (resolved, _) = planned(&f);
    assert_eq!(resolved.x.continuous().unwrap().domain(), (0.0, 2.0));
    assert_eq!(resolved.color.unwrap().norm.range(), (0.0, 2.0));
    assert!(!marks(&scene(&f)).is_empty());
    // A value at a point whose triangles a NaN removes does not count either; every value is
    // still checked.
    let (x, y) = fan();
    let removed = [0.0, f64::NAN, 2.0, 1.0, 4.0];
    let mut f = figure_of(LINEAR);
    f.single_axes_mut()
        .tricontour(
            &x,
            &y,
            Triangles::Given(&[[0, 1, 4], [2, 3, 4]]),
            &removed,
            lines(),
        )
        .unwrap();
    let (resolved, _) = planned(&f);
    // Only [2, 3, 4] is drawn: values 2, 1 and 4.
    assert_eq!(resolved.color.unwrap().norm.range(), (1.0, 4.0));
    let mut f = figure_of(LINEAR);
    let error = rejected(
        &mut f,
        (&x, &y, Triangles::Given(&[[0, 1, 4]])),
        &[0.0, 1.0, f64::INFINITY, 1.0, 4.0],
        lines(),
    );
    assert_eq!(error.parameter(), Some("z"));
    // No triangle left: no level, no color, nothing drawn, as for a contour on a grid of NaN.
    for options in [lines(), styled(ContourStyle::Both)] {
        let mut f = figure_of(LINEAR);
        f.single_axes_mut()
            .tricontour(&x, &y, Triangles::Given(&[[0, 1, 4]]), &removed, options)
            .unwrap();
        let [Mark::Tricontour(mark)] = f.single_axes().marks() else {
            unreachable!()
        };
        assert!(mark.levels().is_empty());
        let (resolved, layout) = planned(&f);
        assert!(resolved.color.is_none() && layout.colorbar.is_none());
        let drawn = scene(&f)
            .items
            .iter()
            .any(|item| matches!(item, Item::Group { clip: Some(_), items } if !items.is_empty()));
        assert!(!drawn);
    }
    let (mut x, mut y) = fan();
    x.push(3.0);
    y.push(-1.0);
    let mut z = PEAK.to_vec();
    z.push(10.0);
    // Given levels are kept; a log scale takes log ticks.
    let mut f = figure_of(LINEAR);
    let given = ContourOptions {
        levels: ContourLevels::Values(vec![0.5, 3.0]),
        ..lines()
    };
    f.single_axes_mut()
        .tricontour(&x, &y, Triangles::Auto, &z, given)
        .unwrap();
    let positive: Vec<f64> = z.iter().map(|v| 10.0_f64.powf(*v)).collect();
    let log = ContourOptions {
        scale: Transform::Log(LogBase::TEN),
        ..styled(ContourStyle::Filled)
    };
    let mut g = figure_of(LINEAR);
    g.single_axes_mut()
        .tricontour(&x, &y, Triangles::Auto, &positive, log)
        .unwrap();
    let ([Mark::Tricontour(given)], [Mark::Tricontour(log)]) =
        (f.single_axes().marks(), g.single_axes().marks())
    else {
        unreachable!()
    };
    assert_eq!(given.levels(), [0.5, 3.0]);
    assert!(!log.levels().is_empty());
    assert!(
        log.levels()
            .iter()
            .all(|level| (level.log10() - level.log10().round()).abs() < 1e-9),
        "{:?}",
        log.levels()
    );
    scene(&g);
}

#[test]
fn the_colorbar_marks_the_levels_in_bands_when_filled_and_labels_read_as_its_ticks() {
    let (x, y) = fan();
    let options = |style| ContourOptions {
        levels: ContourLevels::Values(vec![0.5, 1.5, 2.5]),
        style,
        annotate: style != ContourStyle::Filled,
        ..ContourOptions::default()
    };
    for style in [
        ContourStyle::Lines,
        ContourStyle::Filled,
        ContourStyle::Both,
    ] {
        let mut f = figure_of(LINEAR);
        f.single_axes_mut()
            .tricontour(&x, &y, Triangles::Auto, &PEAK, options(style))
            .unwrap()
            .colorbar(ColorbarOptions::default())
            .unwrap();
        let (resolved, layout) = planned(&f);
        assert!(layout.colorbar.is_some());
        let levels = resolved.color.unwrap().levels.unwrap();
        let values: Vec<f64> = levels.ticks.iter().map(|tick| tick.value).collect();
        assert_eq!(values, [0.5, 1.5, 2.5]);
        assert_eq!(levels.labels.len(), 3);
        assert_eq!(levels.banded, style != ContourStyle::Lines);
    }
    // The labels on the lines: in the colorbar's format, upright along the lines.
    let mut f = figure_of(LINEAR);
    let (x, y, z) = smooth(400);
    f.single_axes_mut()
        .tricontour(
            &x,
            &y,
            Triangles::Auto,
            &z,
            ContourOptions {
                levels: ContourLevels::Values(vec![0.25, 0.5, 0.75]),
                annotate: true,
                ..ContourOptions::default()
            },
        )
        .unwrap();
    let texts: Vec<(String, f32)> = marks(&scene(&f))
        .iter()
        .filter_map(|item| match item {
            Item::RotatedText { text, angle } => Some((text.text.clone(), *angle)),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|(text, _)| text == "0.50"), "{texts:?}");
    assert!(
        texts
            .iter()
            .all(|(_, angle)| (-90.0..=90.0).contains(angle))
    );
}

/// Scattered points in the square from -2 to 2 with its corners, and a smooth bump on them.
fn smooth(count: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut state: u64 = 5;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1_u64 << 53) as f64 * 4.0 - 2.0
    };
    let (mut x, mut y) = (vec![-2.0, 2.0, 2.0, -2.0], vec![-2.0, -2.0, 2.0, 2.0]);
    for _ in 0..count {
        x.push(next());
        y.push(next());
    }
    let z = x
        .iter()
        .zip(&y)
        .map(|(x, y)| (-(x * x) - y * y).exp())
        .collect();
    (x, y, z)
}

#[test]
fn one_color_draws_every_line_in_it_without_a_color_scale() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    let red = ContourOptions {
        color: ColorSpec::Fixed(Rgba::RED),
        ..lines()
    };
    f.single_axes_mut()
        .tricontour(&x, &y, Triangles::Auto, &PEAK, red)
        .unwrap();
    let [Mark::Tricontour(mark)] = f.single_axes().marks() else {
        unreachable!()
    };
    assert!(mark.spec().color_scale().is_none());
    let (resolved, _) = planned(&f);
    assert!(resolved.color.is_none());
    let strokes: Vec<Rgba> = marks(&scene(&f))
        .iter()
        .filter_map(|item| match item {
            Item::Stroke { style, .. } => Some(style.color),
            _ => None,
        })
        .collect();
    assert!(!strokes.is_empty());
    assert!(strokes.iter().all(|&color| color == Rgba::RED));
}

#[test]
fn tricontours_share_the_color_scale_with_tripcolor_and_mesh() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    f.single_axes_mut()
        .tripcolor(&x, &y, Triangles::Auto, &PEAK, TripcolorOptions::default())
        .unwrap()
        .tricontour(&x, &y, Triangles::Auto, &PEAK, styled(ContourStyle::Both))
        .unwrap()
        .mesh(&[2.0, 3.0], &[0.0, 1.0], &[-6.0], MeshOptions::default())
        .unwrap();
    let (resolved, _) = planned(&f);
    // The points' values run from 0 to 4 and the mesh's cell is -6; the triangles of the
    // tripcolor take the means of their corners, inside that range.
    assert_eq!(resolved.color.unwrap().norm.range(), (-6.0, 4.0));
    let viridis = ContourOptions {
        cmap: CmapChoice::Fixed(Cmap::viridis()),
        ..lines()
    };
    let error = rejected(&mut f, (&x, &y, Triangles::Auto), &PEAK, viridis);
    assert!(matches!(error, Error::Usage { .. }), "{error:?}");
    assert_eq!(error.parameter(), Some("cmap"));
    // A center makes the range symmetric around it.
    let mut f = figure_of(LINEAR);
    let centered = ContourOptions {
        center: Some(1.0),
        ..styled(ContourStyle::Filled)
    };
    f.single_axes_mut()
        .tricontour(&x, &y, Triangles::Auto, &PEAK, centered)
        .unwrap();
    let (resolved, _) = planned(&f);
    assert_eq!(resolved.color.unwrap().norm.range(), (-2.0, 4.0));
}

#[test]
fn contours_on_grids_and_meshes_that_share_a_colorbar_need_the_same_levels() {
    let (x, y) = fan();
    let mut f = figure_of(LINEAR);
    f.single_axes_mut()
        .tricontour(&x, &y, Triangles::Auto, &PEAK, lines())
        .unwrap()
        .contour(&[0.0, 1.0], &[0.0, 1.0], &[0.0, 1.0, 2.0, 30.0], lines())
        .unwrap();
    let error = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(error, Error::Style { .. }), "{error:?}");
    assert!(
        error
            .to_string()
            .contains("two contours of these axes have different levels"),
        "{error}"
    );
}

#[test]
fn limits_are_the_extent_of_every_point_without_padding_and_hold_equal_aspect() {
    let (mut x, mut y) = fan();
    x.push(3.0);
    y.push(-1.0);
    let mut z = PEAK.to_vec();
    z.push(1.0);
    let mut f = figure_of(LINEAR);
    f.single_axes_mut()
        .tricontour(
            &x,
            &y,
            Triangles::Given(&[[0, 1, 4], [1, 2, 4]]),
            &z,
            lines(),
        )
        .unwrap();
    let (resolved, _) = planned(&f);
    assert_eq!(resolved.x.continuous().unwrap().domain(), (0.0, 3.0));
    assert_eq!(resolved.y.continuous().unwrap().domain(), (-1.0, 1.0));
    f.single_axes_mut().equal_aspect().unwrap();
    let (resolved, layout) = planned(&f);
    assert_eq!(resolved.x.continuous().unwrap().domain(), (0.0, 3.0));
    let ratio = (layout.plot.width / 3.0) / (layout.plot.height / 2.0);
    assert!((ratio - 1.0).abs() < 1e-9, "{ratio}");
}

#[test]
fn bands_are_one_even_odd_fill_each_and_lines_one_stroke_per_level() {
    let (x, y, z) = smooth(300);
    let levels = ContourLevels::Values(vec![0.2, 0.4, 0.6, 0.8]);
    for (style, fills, strokes) in [
        (ContourStyle::Filled, 5, 0),
        (ContourStyle::Lines, 0, 4),
        (ContourStyle::Both, 5, 4),
    ] {
        let mut f = figure_of(LINEAR);
        let options = ContourOptions {
            levels: levels.clone(),
            style,
            ..ContourOptions::default()
        };
        f.single_axes_mut()
            .tricontour(&x, &y, Triangles::Auto, &z, options)
            .unwrap();
        let items = marks(&scene(&f));
        let filled: Vec<&FillRule> = items
            .iter()
            .filter_map(|item| match item {
                Item::Fill { rule, .. } => Some(rule),
                _ => None,
            })
            .collect();
        assert_eq!(filled.len(), fills, "{style:?}");
        assert!(filled.iter().all(|rule| **rule == FillRule::EvenOdd));
        let stroked = items
            .iter()
            .filter(|item| matches!(item, Item::Stroke { .. }))
            .count();
        assert_eq!(stroked, strokes, "{style:?}");
    }
}

#[test]
fn on_a_log_axis_every_crossing_lies_on_the_edge_as_drawn() {
    // A structured mesh over a log x axis, its edges drawn in blue and its lines in red.
    let xs = [1.0, 3.0, 10.0, 30.0, 100.0, 300.0, 1000.0];
    let ys = [0.0, 1.0, 2.0, 3.0];
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for &row in &ys {
        for &column in &xs {
            x.push(column);
            y.push(row);
        }
    }
    let at = |i: usize, j: usize| i64::try_from(j * xs.len() + i).unwrap();
    let mut rows = Vec::new();
    for j in 0..ys.len() - 1 {
        for i in 0..xs.len() - 1 {
            rows.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            rows.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    // Linear in the data, so in log space the field bends: a crossing found in data space
    // would leave the drawn edge.
    let z: Vec<f64> = x.iter().zip(&y).map(|(x, y)| x / 100.0 + y).collect();
    let mut f = figure_of(LOG_X);
    let red = ContourOptions {
        levels: ContourLevels::Values(vec![0.5, 1.5, 2.5, 3.5]),
        color: ColorSpec::Fixed(Rgba::RED),
        ..ContourOptions::default()
    };
    let blue = TriplotOptions {
        color: ColorSpec::Fixed(Rgba::BLUE),
        ..TriplotOptions::default()
    };
    f.single_axes_mut()
        .tricontour(&x, &y, Triangles::Given(&rows), &z, red)
        .unwrap()
        .triplot(&x, &y, Triangles::Given(&rows), blue)
        .unwrap();
    let items = marks(&scene(&f));
    // The points of every subpath of the strokes in `color`.
    let drawn = |color: Rgba| -> Vec<Vec<Point>> {
        items
            .iter()
            .filter_map(|item| match item {
                Item::Stroke { path, style } if style.color == color => Some(subpaths(path)),
                _ => None,
            })
            .flatten()
            .collect()
    };
    let edges: Vec<(Point, Point)> = drawn(Rgba::BLUE)
        .into_iter()
        .map(|edge| (edge[0], edge[1]))
        .collect();
    let distance = |p: Point, (a, b): (Point, Point)| {
        let (dx, dy) = (f64::from(b.x - a.x), f64::from(b.y - a.y));
        let (px, py) = (f64::from(p.x - a.x), f64::from(p.y - a.y));
        let t = ((px * dx + py * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        (px - t * dx).hypot(py - t * dy)
    };
    let points: Vec<Point> = drawn(Rgba::RED).into_iter().flatten().collect();
    assert!(points.len() > 20, "{}", points.len());
    for p in points {
        let nearest = edges
            .iter()
            .map(|&edge| distance(p, edge))
            .fold(f64::INFINITY, f64::min);
        assert!(nearest < 1e-3, "{p:?} is {nearest} pt off the nearest edge");
    }
}

#[test]
fn either_winding_and_any_row_order_draw_the_same() {
    let (x, y) = fan();
    let given = [[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]];
    let flipped = [[4, 3, 2], [0, 4, 1], [4, 0, 3], [1, 4, 2]];
    let drawn = |triangles: &[[i64; 3]]| {
        let mut f = figure_of(LINEAR);
        let options = ContourOptions {
            levels: ContourLevels::Values(vec![0.5, 1.0, 2.0, 3.0]),
            annotate: true,
            ..styled(ContourStyle::Both)
        };
        f.single_axes_mut()
            .tricontour(&x, &y, Triangles::Given(triangles), &PEAK, options)
            .unwrap();
        scene(&f)
    };
    assert_eq!(drawn(&given), drawn(&flipped));
}

/// The colorbar levels of the only panel of `figure`, if it marks any.
fn bar_levels(figure: &Figure) -> Option<Vec<f64>> {
    let (resolved, _) = planned(figure);
    let levels = resolved.color?.levels?;
    Some(levels.ticks.iter().map(|tick| tick.value).collect())
}

/// A triangle whose one corner is NaN, so it draws nothing, with a point beyond it.
const EMPTY_X: [f64; 4] = [0.0, 1.0, 0.0, 2.0];
const EMPTY_Y: [f64; 4] = [0.0, 0.0, 1.0, 2.0];
const EMPTY_Z: [f64; 4] = [0.0, 1.0, f64::NAN, 2.0];

#[test]
fn a_mesh_contour_that_draws_nothing_takes_no_part_in_the_colorbar() {
    let empty = Triangles::Given(&[[0, 1, 2]]);
    // Beside a contour that draws: its automatic levels, none, are not compared.
    let mut f = figure_of(LINEAR);
    f.single_axes_mut()
        .tricontour(
            &[0.0, 1.0, 0.0],
            &[0.0, 0.0, 1.0],
            Triangles::Auto,
            &[0.0, 1.0, 2.0],
            lines(),
        )
        .unwrap()
        .tricontour(&EMPTY_X, &EMPTY_Y, empty, &EMPTY_Z, lines())
        .unwrap();
    let [Mark::Tricontour(drawn), Mark::Tricontour(nothing)] = f.single_axes().marks() else {
        unreachable!()
    };
    assert!(nothing.levels().is_empty() && !drawn.levels().is_empty());
    let expected = drawn.levels();
    scene(&f);
    assert_eq!(bar_levels(&f), Some(expected.clone()));
    // It still counts in the limits.
    let (resolved, _) = planned(&f);
    assert_eq!(resolved.x.continuous().unwrap().domain(), (0.0, 2.0));
    // With levels of its own, which differ, too.
    let mut f = figure_of(LINEAR);
    let given = ContourOptions {
        levels: ContourLevels::Values(vec![5.0]),
        ..styled(ContourStyle::Both)
    };
    f.single_axes_mut()
        .tricontour(
            &[0.0, 1.0, 0.0],
            &[0.0, 0.0, 1.0],
            Triangles::Auto,
            &[0.0, 1.0, 2.0],
            lines(),
        )
        .unwrap()
        .tricontour(&EMPTY_X, &EMPTY_Y, empty, &EMPTY_Z, given)
        .unwrap();
    scene(&f);
    assert_eq!(bar_levels(&f), Some(expected));
    // Filled over a mesh or a tripcolor: the bar stays the gradient of their values.
    for under in ["mesh", "tripcolor"] {
        let mut f = figure_of(LINEAR);
        let mut axes = f.single_axes_mut();
        if under == "mesh" {
            axes.mesh(
                &[0.0, 1.0, 2.0],
                &[0.0, 1.0],
                &[1.0, 3.0],
                MeshOptions::default(),
            )
            .unwrap();
        } else {
            axes.tripcolor(
                &EMPTY_X,
                &EMPTY_Y,
                Triangles::Auto,
                &[1.0, 3.0, 2.0, 2.0],
                TripcolorOptions::default(),
            )
            .unwrap();
        }
        axes.tricontour(
            &EMPTY_X,
            &EMPTY_Y,
            empty,
            &EMPTY_Z,
            styled(ContourStyle::Filled),
        )
        .unwrap()
        .colorbar(ColorbarOptions::default())
        .unwrap();
        let (resolved, layout) = planned(&f);
        assert!(layout.colorbar.is_some(), "{under}");
        let scale = resolved.color.unwrap();
        assert!(scale.levels.is_none(), "{under}: the bar marks levels");
        assert!(scale.ticks.len() >= 2, "{under}");
        scene(&f);
    }
}

/// A grid contour of `z` on the unit square.
fn grid(z: [f64; 4], options: ContourOptions) -> impl FnOnce(&mut Figure) {
    move |f| {
        f.single_axes_mut()
            .contour(&[0.0, 1.0], &[0.0, 1.0], &z, options)
            .unwrap();
    }
}

/// The values of a grid whose finite values never meet in a cell: nothing is drawn.
const NO_CELL: [f64; 4] = [0.0, 1.0, f64::NAN, 2.0];

#[test]
fn a_grid_contour_that_draws_nothing_takes_no_part_in_the_colorbar() {
    let beside = |empty: [f64; 4], options: ContourOptions| {
        let mut f = figure_of(LINEAR);
        grid([0.0, 1.0, 2.0, 3.0], lines())(&mut f);
        grid(empty, options)(&mut f);
        let [Mark::Contour(drawn), Mark::Contour(nothing)] = f.single_axes().marks() else {
            unreachable!()
        };
        assert!(!drawn.levels().is_empty());
        let expected = drawn.levels();
        let empty_levels = nothing.levels();
        scene(&f);
        assert_eq!(bar_levels(&f), Some(expected), "{empty:?}");
        // It still counts in the limits.
        let (resolved, _) = planned(&f);
        assert_eq!(resolved.x.continuous().unwrap().domain(), (0.0, 1.0));
        empty_levels
    };
    // All NaN: no automatic levels. One finite value, or finite values that share no cell:
    // levels from every finite value, still compared with nothing.
    assert!(beside([f64::NAN; 4], lines()).is_empty());
    assert!(beside([f64::NAN, f64::NAN, 5.0, f64::NAN], lines()).is_empty());
    assert!(!beside(NO_CELL, lines()).is_empty());
    // With levels of its own, which differ, too.
    let given = ContourOptions {
        levels: ContourLevels::Values(vec![5.0]),
        ..styled(ContourStyle::Both)
    };
    assert_eq!(beside([f64::NAN; 4], given), [5.0]);
}

#[test]
fn an_empty_filled_grid_contour_keeps_the_gradient_colorbar() {
    let mut f = figure_of(LINEAR);
    f.single_axes_mut()
        .mesh(
            &[0.0, 1.0, 2.0],
            &[0.0, 1.0],
            &[1.0, 3.0],
            MeshOptions::default(),
        )
        .unwrap();
    grid([f64::NAN; 4], styled(ContourStyle::Filled))(&mut f);
    f.single_axes_mut()
        .colorbar(ColorbarOptions::default())
        .unwrap();
    let (resolved, layout) = planned(&f);
    assert!(layout.colorbar.is_some());
    let scale = resolved.color.unwrap();
    assert!(scale.levels.is_none(), "the bar marks levels");
    assert!(scale.ticks.len() >= 2);
    scene(&f);
}

#[test]
fn a_grid_and_a_mesh_contour_on_one_bar_ignore_whichever_draws_nothing() {
    let (x, y) = fan();
    // An empty mesh contour beside a grid contour that draws, and the other way round.
    let mut f = figure_of(LINEAR);
    grid([0.0, 1.0, 2.0, 3.0], lines())(&mut f);
    f.single_axes_mut()
        .tricontour(
            &EMPTY_X,
            &EMPTY_Y,
            Triangles::Given(&[[0, 1, 2]]),
            &EMPTY_Z,
            lines(),
        )
        .unwrap();
    let Mark::Contour(drawn) = &f.single_axes().marks()[0] else {
        unreachable!()
    };
    let expected = drawn.levels();
    scene(&f);
    assert_eq!(bar_levels(&f), Some(expected));
    let mut f = figure_of(LINEAR);
    f.single_axes_mut()
        .tricontour(&x, &y, Triangles::Auto, &PEAK, lines())
        .unwrap();
    grid([f64::NAN; 4], lines())(&mut f);
    let Mark::Tricontour(drawn) = &f.single_axes().marks()[0] else {
        unreachable!()
    };
    let expected = drawn.levels();
    scene(&f);
    assert_eq!(bar_levels(&f), Some(expected));
    // Both empty: nothing on the bar.
    let mut f = figure_of(LINEAR);
    grid([f64::NAN; 4], lines())(&mut f);
    f.single_axes_mut()
        .tricontour(
            &EMPTY_X,
            &EMPTY_Y,
            Triangles::Given(&[[0, 1, 2]]),
            &EMPTY_Z,
            lines(),
        )
        .unwrap();
    scene(&f);
    assert_eq!(bar_levels(&f), None);
}

#[test]
fn a_grid_contour_that_draws_but_has_no_levels_is_still_compared() {
    // Equal values draw every cell but give no automatic levels: the contour takes part in
    // the check, so it differs from one with levels, as before.
    let mut f = figure_of(LINEAR);
    grid([0.0, 1.0, 2.0, 3.0], lines())(&mut f);
    grid([1.0; 4], lines())(&mut f);
    let error = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(error, Error::Style { .. }), "{error:?}");
    assert!(
        error
            .to_string()
            .contains("two contours of these axes have different levels"),
        "{error}"
    );
}
