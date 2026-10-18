//! Contours at the edges of their rules: levels equal to values, clipped rings with labels, log
//! axes, extreme coordinates, colorbars shared by several contours, centered bars, the stroke
//! clip, level counts and level labels in the colorbar's format.

use crate::pipeline::plan_figure;
use crate::scene::{Item, Paint, Scene};
use crate::{
    ColorSpec, ContourLevels, ContourOptions, ContourStyle, Error, Figure, FontLibrary, Grid,
    GridIndex, LogBase, Mark, PathVerb, Placement, Rgba, RuleOptions, ScaleKind, Share, Theme,
    TickFormat,
};

fn figure(x: ScaleKind) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(x, ScaleKind::Linear).unwrap();
    f
}

fn scene(f: &Figure) -> Scene {
    f.to_scene(FontLibrary::global()).unwrap()
}

/// The items of the clipped marks group, and its clip.
fn marks(scene: &Scene) -> (&[Item], crate::scene::Rect) {
    scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group {
                clip: Some(clip),
                items,
            } => Some((items.as_slice(), *clip)),
            _ => None,
        })
        .expect("a marks group")
}

/// The subpaths, as points, of the strokes of `color` among `items`.
fn strokes(items: &[Item], color: Rgba) -> Vec<Vec<(f64, f64)>> {
    let mut out = Vec::new();
    for item in items {
        let Item::Stroke { path, style } = item else {
            continue;
        };
        if style.color != color {
            continue;
        }
        for verb in path.verbs() {
            match *verb {
                PathVerb::MoveTo(p) => out.push(vec![(f64::from(p.x), f64::from(p.y))]),
                PathVerb::LineTo(p) => {
                    if let Some(last) = out.last_mut() {
                        last.push((f64::from(p.x), f64::from(p.y)));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

fn red() -> ContourOptions {
    ContourOptions {
        color: ColorSpec::Fixed(Rgba::RED),
        ..ContourOptions::default()
    }
}

#[test]
fn a_level_equal_to_values_fills_its_region() {
    let mut f = figure(ScaleKind::Linear);
    f.single_axes_mut()
        .contour(
            &[0.0, 1.0, 2.0],
            &[0.0, 1.0],
            &[0.0, 1.0, 2.0, 0.0, 1.0, 2.0],
            ContourOptions {
                levels: ContourLevels::Values(vec![1.0]),
                style: ContourStyle::Filled,
                ..ContourOptions::default()
            },
        )
        .unwrap();
    let drawn = scene(&f);
    let (items, _) = marks(&drawn);
    let fills = items
        .iter()
        .filter(|item| matches!(item, Item::Fill { .. }))
        .count();
    // The finite cells, then the right half at or above 1.
    assert_eq!(fills, 2, "{items:?}");
}

#[test]
fn annotating_a_clipped_ring_keeps_its_shape() {
    let drawn = |annotate: bool| {
        let mut f = figure(ScaleKind::Linear);
        f.single_axes_mut()
            .contour(
                &[0.0, 1.0, 2.0],
                &[0.0, 1.0, 2.0],
                &[0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0],
                ContourOptions {
                    levels: ContourLevels::Values(vec![2.0]),
                    annotate,
                    ..red()
                },
            )
            .unwrap()
            .xlim(0.75, 2.0)
            .unwrap()
            .ylim(0.0, 2.0)
            .unwrap();
        let drawn = scene(&f);
        strokes(marks(&drawn).0, Rgba::RED)
    };
    let plain = drawn(false);
    let labelled = drawn(true);
    assert!(!labelled.is_empty());
    let distance = |p: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = dx * dx + dy * dy;
        let t = if length > 0.0 {
            (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
    };
    // Every drawn segment of the labelled line lies on the plain line: its points do, and so
    // does the middle of each of its segments.
    for piece in &labelled {
        let mut points = piece.clone();
        points.extend(
            piece
                .windows(2)
                .map(|w| ((w[0].0 + w[1].0) / 2.0, (w[0].1 + w[1].1) / 2.0)),
        );
        for p in points {
            let nearest = plain
                .iter()
                .flat_map(|line| line.windows(2).map(|w| distance(p, w[0], w[1])))
                .fold(f64::INFINITY, f64::min);
            assert!(nearest < 1e-3, "{p:?} is {nearest} pt off the line");
        }
    }
}

#[test]
fn log_axes_interpolate_in_the_data() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let mut f = figure(log);
    f.single_axes_mut()
        .contour(
            &[1.0, 100.0],
            &[0.0, 1.0],
            &[0.0, 1.0, 0.0, 1.0],
            ContourOptions {
                levels: ContourLevels::Values(vec![0.5]),
                ..red()
            },
        )
        .unwrap()
        .vline(
            crate::AxisValue::Number(50.5),
            RuleOptions {
                color: ColorSpec::Fixed(Rgba::BLUE),
                ..RuleOptions::default()
            },
        )
        .unwrap()
        .xlim(1.0, 100.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap();
    let drawn = scene(&f);
    let (items, _) = marks(&drawn);
    let contour = strokes(items, Rgba::RED);
    let rule = strokes(items, Rgba::BLUE);
    let x = |lines: &[Vec<(f64, f64)>]| lines[0][0].0;
    assert!(
        (x(&contour) - x(&rule)).abs() < 1e-3,
        "{contour:?} {rule:?}"
    );
}

#[test]
fn extreme_coordinates_keep_exact_ends() {
    let mut f = figure(ScaleKind::Linear);
    f.single_axes_mut()
        .contour(
            &[-1e300, 1.0],
            &[0.0, 1.0],
            &[3.0; 4],
            ContourOptions {
                style: ContourStyle::Filled,
                ..ContourOptions::default()
            },
        )
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap();
    let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    let drawn = scene(&f);
    let (items, _) = marks(&drawn);
    let Some(Item::Fill { path, .. }) = items.first() else {
        panic!("a fill: {items:?}")
    };
    let (low, high) = path.bounds().unwrap();
    let plot = layout.plot;
    assert!(f64::from(low.x) <= plot.x + 1e-3, "{low:?} {plot:?}");
    assert!(
        f64::from(high.x) >= plot.right() - 1e-3,
        "{high:?} {plot:?}"
    );
}

/// A 1 × 2 grid sharing one colorbar, each panel with a filled contour of the levels given.
fn shared(levels: [&[f64]; 2]) -> Figure {
    let grid = Grid::new(1, 2).unwrap().with_share_color(Share::All);
    let mut f = Figure::with_grid(8.0, 4.0, Theme::pigment(), grid).unwrap();
    for (col, levels) in levels.into_iter().enumerate() {
        f.add_axes_at(
            Placement {
                row: GridIndex::At(0),
                col: GridIndex::At(col as i64),
                ..Placement::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap()
        .contour(
            &[0.0, 1.0],
            &[0.0, 1.0],
            &[0.0, 1.0, 0.0, 1.0],
            ContourOptions {
                levels: ContourLevels::Values(levels.to_vec()),
                style: ContourStyle::Filled,
                ..ContourOptions::default()
            },
        )
        .unwrap();
    }
    f
}

#[test]
fn contours_on_one_colorbar_need_the_same_levels() {
    let error = shared([&[0.25, 0.75], &[0.4, 0.6]])
        .to_scene(FontLibrary::global())
        .unwrap_err();
    assert!(matches!(error, Error::Style { .. }), "{error:?}");
    assert_eq!(
        error.to_string(),
        "Figure.to_scene(): the contours of row 0, col 0 and row 0, col 1 have different levels; \
         contours that share a colorbar need the same levels."
    );
    assert_eq!(
        error.hint(),
        Some("pass the same levels to every contour that shares a colorbar.")
    );
    shared([&[0.25, 0.75], &[0.25, 0.75]])
        .to_scene(FontLibrary::global())
        .unwrap();
    // Two contours of one axes.
    let mut f = figure(ScaleKind::Linear);
    for levels in [vec![0.25], vec![0.5]] {
        f.single_axes_mut()
            .contour(
                &[0.0, 1.0],
                &[0.0, 1.0],
                &[0.0, 1.0, 0.0, 1.0],
                ContourOptions {
                    levels: ContourLevels::Values(levels),
                    ..ContourOptions::default()
                },
            )
            .unwrap();
    }
    let error = f.to_scene(FontLibrary::global()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Figure.to_scene(): two contours of these axes have different levels; contours that \
         share a colorbar need the same levels."
    );
    // Lines and bands of the same levels: one bar, in bands.
    let mut f = figure(ScaleKind::Linear);
    for style in [ContourStyle::Lines, ContourStyle::Filled] {
        f.single_axes_mut()
            .contour(
                &[0.0, 1.0],
                &[0.0, 1.0],
                &[0.0, 1.0, 0.0, 1.0],
                ContourOptions {
                    levels: ContourLevels::Values(vec![0.5]),
                    style,
                    ..ContourOptions::default()
                },
            )
            .unwrap();
    }
    let drawn = scene(&f);
    let Some(Item::Group { clip: None, items }) = drawn.items.last() else {
        panic!("the colorbar last")
    };
    assert!(!items.iter().any(|item| matches!(
        item,
        Item::Fill {
            paint: Paint::Sampled { .. },
            ..
        }
    )));
}

#[test]
fn a_centered_contour_bar_keeps_its_center_tick() {
    let mut f = figure(ScaleKind::Linear);
    f.single_axes_mut()
        .contour(
            &[0.0, 1.0],
            &[0.0, 1.0],
            &[-1.0, 1.0, -1.0, 1.0],
            ContourOptions {
                levels: ContourLevels::Values(vec![-0.8, -0.2, 0.4, 0.8]),
                center: Some(0.0),
                style: ContourStyle::Filled,
                ..ContourOptions::default()
            },
        )
        .unwrap();
    let (resolved, _) = plan_figure(&f, (0, 0), FontLibrary::global());
    let ticks = resolved.color.unwrap().ticks;
    assert!(
        ticks
            .iter()
            .any(|tick| tick.label == "0" && (tick.pos.get() - 0.5).abs() < 1e-12),
        "{ticks:?}"
    );
}

#[test]
fn contour_lines_widen_the_marks_clip() {
    let clip_of = |f: &Figure| marks(&scene(f)).1;
    let mut lines = figure(ScaleKind::Linear);
    lines
        .single_axes_mut()
        .contour(
            &[0.0, 1.0],
            &[0.0, 1.0],
            &[0.0, 1.0, 0.0, 1.0],
            ContourOptions {
                levels: ContourLevels::Values(vec![1.0]),
                ..red()
            },
        )
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap();
    let (_, layout) = plan_figure(&lines, (0, 0), FontLibrary::global());
    let clip = clip_of(&lines);
    // Half of the 1.25 pt stroke on the right of the plot, which has room for it.
    let right = f64::from(clip.x + clip.width);
    assert!(
        (right - (layout.plot.right() + 0.625)).abs() < 1e-3,
        "{clip:?} {:?}",
        layout.plot
    );
}

#[test]
fn a_count_asks_for_about_that_many_levels() {
    for (count, least, most) in [(1, 1, 1), (2, 1, 3), (3, 2, 5), (5, 3, 8), (9, 5, 14)] {
        let mut f = figure(ScaleKind::Linear);
        f.single_axes_mut()
            .contour(
                &[0.0, 1.0],
                &[0.0, 1.0],
                &[0.0, 1.0, 0.0, 1.0],
                ContourOptions {
                    levels: ContourLevels::Count(count),
                    ..ContourOptions::default()
                },
            )
            .unwrap();
        let [Mark::Contour(contour)] = f.single_axes().marks() else {
            unreachable!()
        };
        let levels = contour.levels();
        assert!(
            (least..=most).contains(&levels.len()),
            "levels={count}: {levels:?}"
        );
        assert!(levels.iter().all(|&v| 0.0 < v && v < 1.0), "{levels:?}");
    }
}

#[test]
fn level_labels_follow_the_colorbar_format() {
    let mut f = figure(ScaleKind::Linear);
    let x: Vec<f64> = (0..21).map(|i| f64::from(i) / 20.0).collect();
    let z: Vec<f64> = (0..21 * 21).map(|k| f64::from(k % 21) / 20.0).collect();
    f.single_axes_mut()
        .contour(
            &x,
            &x,
            &z,
            ContourOptions {
                levels: ContourLevels::Values(vec![0.25, 0.5, 0.75]),
                annotate: true,
                ..ContourOptions::default()
            },
        )
        .unwrap()
        .ytick_format(TickFormat::percent(0).unwrap())
        .unwrap();
    let (resolved, _) = plan_figure(&f, (0, 0), FontLibrary::global());
    let bar: Vec<String> = resolved
        .color
        .unwrap()
        .ticks
        .iter()
        .map(|tick| tick.label.clone())
        .collect();
    assert_eq!(bar, ["25%", "50%", "75%"]);
    let drawn = scene(&f);
    let labels: Vec<&str> = marks(&drawn)
        .0
        .iter()
        .filter_map(|item| match item {
            Item::RotatedText { text, .. } => Some(text.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(labels.len(), 3, "{labels:?}");
    assert!(
        labels.iter().all(|label| bar.iter().any(|b| b == label)),
        "{labels:?}"
    );
}
