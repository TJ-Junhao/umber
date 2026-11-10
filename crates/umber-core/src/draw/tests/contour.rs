//! Contours as drawn: bands, lines and level labels, the colorbar of their levels, and the
//! degenerate grids that draw little or nothing.

use crate::pipeline::{plan_figure, plan_panel_marks};
use crate::scene::{FillRule, Item, Paint, Scene};
use crate::{
    ColorbarOptions, ContourLevels, ContourOptions, ContourStyle, Error, Figure, FontLibrary, Mark,
    MeshOptions, Rgba, ScaleKind, SvgText, Theme, TickFormat, Transform,
};

/// A figure with one contour of `z` on the grid `x` by `y`.
fn figure(x: &[f64], y: &[f64], z: &[f64], options: ContourOptions) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .contour(x, y, z, options)
        .unwrap();
    f
}

/// Options of `style`.
fn styled(style: ContourStyle) -> ContourOptions {
    ContourOptions {
        style,
        ..ContourOptions::default()
    }
}

/// A smooth field on a 9 × 7 grid: two bumps of opposite sign and a slope.
fn field() -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let x: Vec<f64> = (0..9).map(|i| f64::from(i) / 2.0 - 2.0).collect();
    let y: Vec<f64> = (0..7).map(|j| f64::from(j) / 2.0 - 1.5).collect();
    let z = y
        .iter()
        .flat_map(|&y| {
            x.iter().map(move |&x| {
                (-((x + 0.8).powi(2) + y * y)).exp() * 2.0
                    - (-((x - 1.0).powi(2) + (y - 0.4).powi(2))).exp()
                    + 0.1 * x
            })
        })
        .collect();
    (x, y, z)
}

fn scene(f: &Figure) -> Scene {
    f.to_scene(FontLibrary::global()).unwrap()
}

/// The items of the clipped marks group; empty when nothing is drawn.
fn mark_items(scene: &Scene) -> Vec<&Item> {
    scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group {
                clip: Some(_),
                items,
            } => Some(items.iter().collect()),
            _ => None,
        })
        .unwrap_or_default()
}

/// The colors of the fills among `items`, each checked to be an even-odd fill of one color.
fn fill_colors(items: &[&Item]) -> Vec<Rgba> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Fill {
                paint: Paint::Solid(color),
                rule: FillRule::EvenOdd,
                ..
            } => Some(*color),
            Item::Fill { .. } => panic!("a contour band is a solid even-odd fill: {item:?}"),
            _ => None,
        })
        .collect()
}

fn stroke_count(items: &[&Item]) -> usize {
    items
        .iter()
        .filter(|item| matches!(item, Item::Stroke { .. }))
        .count()
}

/// The items of the colorbar group, which a contour figure without a legend draws last.
fn colorbar_items(scene: &Scene) -> &[Item] {
    match scene.items.last() {
        Some(Item::Group { clip: None, items }) => items,
        other => panic!("expected the colorbar group last, got {other:?}"),
    }
}

/// The levels of the one contour of `f`.
fn levels(f: &Figure) -> Vec<f64> {
    match f.single_axes().marks() {
        [Mark::Contour(contour)] => contour.levels(),
        other => panic!("expected one contour, got {other:?}"),
    }
}

#[test]
fn contour_degenerate_grids() {
    let lib = FontLibrary::global();
    let (x, y) = ([0.0, 1.0, 2.0], [0.0, 1.0]);
    let middle = Theme::pigment().cmap().at(0.5);
    // A constant field: no lines; every finite cell filled in the middle color; one tick.
    for style in [
        ContourStyle::Lines,
        ContourStyle::Filled,
        ContourStyle::Both,
    ] {
        let f = figure(&x, &y, &[3.0; 6], styled(style));
        assert!(levels(&f).is_empty());
        let drawn = scene(&f);
        let items = mark_items(&drawn);
        assert_eq!(stroke_count(&items), 0, "{style:?}");
        let fills = fill_colors(&items);
        if style == ContourStyle::Lines {
            assert!(fills.is_empty());
        } else {
            assert_eq!(fills, [middle], "{style:?}");
        }
        let (resolved, layout) = plan_figure(&f, (0, 0), lib);
        let ticks = resolved.color.unwrap().ticks;
        assert_eq!(ticks.len(), 1, "{style:?}");
        assert_eq!(ticks[0].label, "3");
        assert!(layout.colorbar.is_some());
    }
    // Every value NaN: nothing drawn, no colorbar.
    for style in [
        ContourStyle::Lines,
        ContourStyle::Filled,
        ContourStyle::Both,
    ] {
        let f = figure(&x, &y, &[f64::NAN; 6], styled(style));
        let drawn = scene(&f);
        assert!(mark_items(&drawn).is_empty(), "{style:?}");
        let (resolved, layout) = plan_figure(&f, (0, 0), lib);
        assert!(resolved.color.is_none() && layout.colorbar.is_none());
    }
    // The smallest grid, 2 × 2: one cell, its levels and lines.
    let f = figure(
        &[0.0, 1.0],
        &[0.0, 1.0],
        &[0.0, 1.0, 1.0, 2.0],
        styled(ContourStyle::Both),
    );
    let count = levels(&f).len();
    assert!(count >= 2 && levels(&f).iter().all(|&v| 0.0 < v && v < 2.0));
    let drawn = scene(&f);
    let items = mark_items(&drawn);
    assert_eq!(fill_colors(&items).len(), count + 1);
    assert_eq!(stroke_count(&items), count);
    // An island of NaN: drawn around, never over.
    let (x9, y7, mut z) = field();
    z[3 * 9 + 4] = f64::NAN;
    z[3 * 9 + 5] = f64::NAN;
    for style in [
        ContourStyle::Lines,
        ContourStyle::Filled,
        ContourStyle::Both,
    ] {
        let options = ContourOptions {
            annotate: style != ContourStyle::Filled,
            ..styled(style)
        };
        let f = figure(&x9, &y7, &z, options);
        crate::test_support::scene::assert_scene_bounded(&scene(&f));
    }
    // Levels outside the range of the values: no line and no error; the fill takes the color
    // of the low end of the range.
    let outside = ContourLevels::Values(vec![10.0, 20.0]);
    let f = figure(
        &x,
        &y,
        &[0.0, 1.0, 2.0, 1.0, 2.0, 3.0],
        ContourOptions {
            levels: outside.clone(),
            ..ContourOptions::default()
        },
    );
    assert_eq!(stroke_count(&mark_items(&scene(&f))), 0);
    let f = figure(
        &x,
        &y,
        &[0.0, 1.0, 2.0, 1.0, 2.0, 3.0],
        ContourOptions {
            levels: outside,
            style: ContourStyle::Filled,
            ..ContourOptions::default()
        },
    );
    let drawn = scene(&f);
    assert_eq!(
        fill_colors(&mark_items(&drawn)),
        [Theme::pigment().cmap().at(0.0)]
    );
    // A saddle cell: the same drawing every time.
    let saddle = figure(
        &[0.0, 1.0],
        &[0.0, 1.0],
        &[1.0, 0.0, 0.0, 1.0],
        ContourOptions {
            levels: ContourLevels::Values(vec![0.25, 0.5, 0.75]),
            style: ContourStyle::Both,
            ..ContourOptions::default()
        },
    );
    let first = saddle.to_svg(SvgText::Editable).unwrap();
    assert_eq!(saddle.to_svg(SvgText::Editable).unwrap(), first);
    assert_eq!(stroke_count(&mark_items(&scene(&saddle))), 3);
}

#[test]
fn colorbar_banded_for_filled() {
    let lib = FontLibrary::global();
    let (x, y, z) = field();
    for style in [ContourStyle::Filled, ContourStyle::Both] {
        let f = figure(&x, &y, &z, styled(style));
        let levels = levels(&f);
        assert!(levels.len() >= 4, "{levels:?}");
        let (resolved, _) = plan_figure(&f, (0, 0), lib);
        let color = resolved.color.unwrap();
        // Ticks at the levels, labelled as the levels are.
        let (low, high) = color.norm.range();
        let inside: Vec<f64> = levels
            .iter()
            .copied()
            .filter(|&v| low <= v && v <= high)
            .collect();
        assert_eq!(color.ticks.len(), inside.len());
        for (tick, level) in color.ticks.iter().zip(&inside) {
            assert_eq!(tick.pos.get(), color.norm.unit(*level).unwrap());
        }
        // A band per level and one below the first, in the colors of the filled bands, and no
        // gradient.
        let drawn = scene(&f);
        let bar = colorbar_items(&drawn);
        assert!(!bar.iter().any(|item| matches!(
            item,
            Item::Fill {
                paint: Paint::Sampled { .. },
                ..
            }
        )));
        let bands: Vec<Rgba> = bar
            .iter()
            .filter_map(|item| match item {
                Item::Fill {
                    paint: Paint::Solid(color),
                    ..
                } => Some(*color),
                _ => None,
            })
            .collect();
        assert_eq!(bands.len(), levels.len() + 1);
        assert_eq!(bands, fill_colors(&mark_items(&drawn)), "{style:?}");
        assert_eq!(bands[0], color.cmap.at(0.0));
        assert_eq!(bands[levels.len()], color.cmap.at(1.0));
    }
    // Lines: the smooth bar, with ticks at the levels.
    let f = figure(&x, &y, &z, ContourOptions::default());
    let drawn = scene(&f);
    let bar = colorbar_items(&drawn);
    assert!(bar.iter().any(|item| matches!(
        item,
        Item::Fill {
            paint: Paint::Sampled { .. },
            ..
        }
    )));
    let (resolved, _) = plan_figure(&f, (0, 0), lib);
    let labels: Vec<String> = resolved
        .color
        .unwrap()
        .ticks
        .iter()
        .map(|tick| tick.label.clone())
        .collect();
    let Mark::Contour(contour) = &f.single_axes().marks()[0] else {
        unreachable!()
    };
    assert_eq!(labels, contour.spec().labels_in(&TickFormat::default()));
}

#[test]
fn lines_take_their_level_colors_or_one_color() {
    let (x, y, z) = field();
    let f = figure(&x, &y, &z, ContourOptions::default());
    let (resolved, _) = plan_figure(&f, (0, 0), FontLibrary::global());
    let color = resolved.color.unwrap();
    let drawn = scene(&f);
    let strokes: Vec<(Rgba, f32)> = mark_items(&drawn)
        .iter()
        .filter_map(|item| match item {
            Item::Stroke { style, .. } => Some((style.color, style.width)),
            _ => None,
        })
        .collect();
    let expected: Vec<(Rgba, f32)> = levels(&f)
        .iter()
        .map(|&level| {
            // Levels close to the background are drawn darker (`contour_contrast`).
            let level_color = color.color(level).unwrap();
            let drawn = super::contour_contrast::expected_line_color(
                Theme::pigment().background(),
                level_color,
            );
            (drawn, 1.25)
        })
        .collect();
    assert_eq!(strokes, expected);
    // One color: every line in it, no color scale and no colorbar.
    let one = ContourOptions {
        color: crate::ColorSpec::Fixed(Rgba::RED),
        ..ContourOptions::default()
    };
    let f = figure(&x, &y, &z, one);
    let (resolved, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
    assert!(resolved.color.is_none() && layout.colorbar.is_none());
    let drawn = scene(&f);
    let items = mark_items(&drawn);
    assert!(
        items
            .iter()
            .all(|item| matches!(item, Item::Stroke { style, .. } if style.color == Rgba::RED))
    );
    assert_eq!(items.len(), levels(&f).len());
    // Both: thin lines in the theme's text color at 0.7 opacity, over the bands.
    let f = figure(&x, &y, &z, styled(ContourStyle::Both));
    let drawn = scene(&f);
    let items = mark_items(&drawn);
    let text = Theme::pigment().text_color();
    let first_stroke = items
        .iter()
        .position(|item| matches!(item, Item::Stroke { .. }))
        .unwrap();
    assert!(
        items[..first_stroke]
            .iter()
            .all(|item| matches!(item, Item::Fill { .. }))
    );
    for item in &items[first_stroke..] {
        let Item::Stroke { style, .. } = item else {
            panic!("lines come after the bands: {item:?}")
        };
        assert_eq!(style.width, 0.6);
        assert_eq!(style.color, text.scale_alpha(0.7));
    }
}

#[test]
fn level_labels_sit_on_long_lines_and_break_them() {
    let (x, y, z) = field();
    let annotated = ContourOptions {
        annotate: true,
        ..ContourOptions::default()
    };
    let f = figure(&x, &y, &z, annotated);
    let drawn = scene(&f);
    let items = mark_items(&drawn);
    let Mark::Contour(contour) = &f.single_axes().marks()[0] else {
        unreachable!()
    };
    let labels: Vec<(&str, f32)> = items
        .iter()
        .filter_map(|item| match item {
            Item::RotatedText { text, angle } => Some((text.text.as_str(), *angle)),
            _ => None,
        })
        .collect();
    assert!(!labels.is_empty());
    for (text, angle) in &labels {
        assert!(
            contour
                .spec()
                .labels_in(&TickFormat::default())
                .iter()
                .any(|label| label == text),
            "{text}"
        );
        assert!(*angle > -90.0 && *angle <= 90.0, "{angle}");
    }
    // The labels come after the lines, which have more pieces than without labels.
    let plain = scene(&figure(&x, &y, &z, ContourOptions::default()));
    let pieces = |items: &[&Item]| -> usize {
        items
            .iter()
            .map(|item| match item {
                Item::Stroke { path, .. } => path
                    .verbs()
                    .iter()
                    .filter(|verb| matches!(verb, crate::PathVerb::MoveTo(_)))
                    .count(),
                _ => 0,
            })
            .sum()
    };
    assert!(pieces(&items) > pieces(&mark_items(&plain)));
    // In SVG as editable text, each label is turned about its origin.
    let svg = f.to_svg(SvgText::Editable).unwrap();
    assert_eq!(
        svg.matches("transform=\"rotate(").count(),
        labels.iter().filter(|(_, a)| *a != 0.0).count()
    );
}

/// The colorbar levels of the only panel of `f`, if it marks any.
fn bar_levels(f: &Figure) -> Option<Vec<f64>> {
    let (resolved, _, _) = plan_panel_marks(f, FontLibrary::global()).remove(0);
    let levels = resolved.color?.levels?;
    Some(levels.ticks.iter().map(|tick| tick.value).collect())
}

/// A figure with one grid contour of `z` on the unit square for each of `contours`.
fn grids(contours: &[([f64; 4], ContourOptions)]) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    for (z, options) in contours {
        axes.contour(&[0.0, 1.0], &[0.0, 1.0], z, options.clone())
            .unwrap();
    }
    f
}

/// Values whose finite ones never meet in a cell: nothing is drawn.
const NO_CELL: [f64; 4] = [0.0, 1.0, f64::NAN, 2.0];

const RISING: [f64; 4] = [0.0, 1.0, 2.0, 3.0];

#[test]
fn a_grid_contour_that_draws_nothing_takes_no_part_in_the_colorbar() {
    let beside = |empty: [f64; 4], options: ContourOptions| {
        let f = grids(&[(RISING, ContourOptions::default()), (empty, options)]);
        let [Mark::Contour(drawn), Mark::Contour(nothing)] = f.single_axes().marks() else {
            unreachable!()
        };
        assert!(!drawn.levels().is_empty());
        let expected = drawn.levels();
        let empty_levels = nothing.levels();
        scene(&f);
        assert_eq!(bar_levels(&f), Some(expected), "{empty:?}");
        // It still counts in the limits.
        let (resolved, _, _) = plan_panel_marks(&f, FontLibrary::global()).remove(0);
        assert_eq!(resolved.x.continuous().unwrap().domain(), (0.0, 1.0));
        empty_levels
    };
    // All NaN: no automatic levels. One finite value, or finite values that share no cell:
    // levels from every finite value, still compared with nothing.
    assert!(beside([f64::NAN; 4], ContourOptions::default()).is_empty());
    assert!(
        beside(
            [f64::NAN, f64::NAN, 5.0, f64::NAN],
            ContourOptions::default()
        )
        .is_empty()
    );
    assert!(!beside(NO_CELL, ContourOptions::default()).is_empty());
    // With levels of its own, which differ, too.
    let given = ContourOptions {
        levels: ContourLevels::Values(vec![5.0]),
        ..styled(ContourStyle::Both)
    };
    assert_eq!(beside([f64::NAN; 4], given), [5.0]);
}

#[test]
fn an_empty_filled_grid_contour_keeps_the_gradient_colorbar() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    axes.mesh(
        &[0.0, 1.0, 2.0],
        &[0.0, 1.0],
        &[1.0, 3.0],
        MeshOptions::default(),
    )
    .unwrap()
    .contour(
        &[0.0, 1.0],
        &[0.0, 1.0],
        &[f64::NAN; 4],
        styled(ContourStyle::Filled),
    )
    .unwrap()
    .colorbar(ColorbarOptions::default())
    .unwrap();
    let (resolved, layout, _) = plan_panel_marks(&f, FontLibrary::global()).remove(0);
    assert!(layout.colorbar.is_some());
    let scale = resolved.color.unwrap();
    assert!(scale.levels.is_none(), "the bar marks levels");
    assert!(scale.ticks.len() >= 2);
    scene(&f);
}

#[test]
fn an_empty_grid_contour_adds_nothing_to_the_color_range() {
    // Over a mesh of 1 to 3, an empty contour of values up to 300 leaves the range alone, on
    // a linear, a centered and a log scale. One that draws something still widens it.
    let values = [100.0, 200.0, f64::NAN, 300.0];
    let range = |mesh: MeshOptions, contour: ContourOptions, z: [f64; 4]| {
        let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.mesh(&[0.0, 1.0, 2.0], &[0.0, 1.0], &[1.0, 3.0], mesh)
            .unwrap()
            .contour(&[0.0, 1.0], &[0.0, 1.0], &z, contour)
            .unwrap();
        let (resolved, _, _) = plan_panel_marks(&f, FontLibrary::global()).remove(0);
        resolved.color.unwrap().norm.range()
    };
    let filled = styled(ContourStyle::Filled);
    assert_eq!(
        range(MeshOptions::default(), filled.clone(), values),
        (1.0, 3.0)
    );
    let centered = |center| {
        (
            MeshOptions {
                center,
                ..MeshOptions::default()
            },
            ContourOptions {
                center,
                ..filled.clone()
            },
        )
    };
    let (mesh, contour) = centered(Some(2.0));
    assert_eq!(range(mesh, contour, values), (1.0, 3.0));
    let log = Transform::Log(crate::LogBase::TEN);
    let mesh = MeshOptions {
        scale: log,
        ..MeshOptions::default()
    };
    let contour = ContourOptions {
        scale: log,
        ..filled.clone()
    };
    assert_eq!(range(mesh, contour, values), (1.0, 3.0));
    // Every finite value of a contour that draws something counts, as before.
    assert_eq!(
        range(MeshOptions::default(), filled, [100.0, 200.0, 250.0, 300.0]),
        (1.0, 300.0)
    );
}

#[test]
fn a_grid_contour_that_draws_but_has_no_levels_is_still_compared() {
    // Equal values draw every cell but give no automatic levels: the contour takes part in
    // the check, so it differs from one with levels, as before.
    let f = grids(&[
        (RISING, ContourOptions::default()),
        ([1.0; 4], ContourOptions::default()),
    ]);
    let error = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(error, Error::Style { .. }), "{error:?}");
    assert!(
        error
            .to_string()
            .contains("two contours of these axes have different levels"),
        "{error}"
    );
}
