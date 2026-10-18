//! Horizontal marks on a category y axis: bars, boxes and violins mirror their vertical form,
//! categories run top to bottom, grouped slots follow the legend, scatter is a dot plot, and the
//! y tick column holds the category labels.

use super::{chrome_items, mark_group, strokes};
use crate::geometry::{Path, PathVerb, Point};
use crate::layout::{Area, PlacedBaseline};
use crate::model::{
    AxesMut, BarOptions, BoxOptions, Distribution, Figure, ScatterOptions, Series, ViolinOptions,
};
use crate::resolve::resolve_figure_marks;
use crate::scene::Item;
use crate::test_support::scene::assert_scene_bounded;
use crate::{AxisData, Error, FontLibrary, ScaleKind, Theme};

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

/// The square plot both forms of a mirrored pair are drawn on, so that the horizontal form is
/// the vertical one with its axes swapped.
const SQUARE: Area = Area {
    x: 60.0,
    y: 40.0,
    width: 240.0,
    height: 240.0,
};

/// A figure in `theme` whose one axes has a category x axis beside a linear y axis limited to
/// `limits` (vertical), or a linear x axis limited to `limits` beside a category y axis
/// (horizontal), with the marks `add` adds.
fn oriented(
    theme: &Theme,
    horizontal: bool,
    limits: (f64, f64),
    add: impl FnOnce(&mut AxesMut<'_>),
) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    let (x, y) = if horizontal {
        (ScaleKind::Linear, ScaleKind::Category)
    } else {
        (ScaleKind::Category, ScaleKind::Linear)
    };
    let mut axes = f.add_axes(x, y).unwrap();
    if horizontal {
        axes.xlim(limits.0, limits.1).unwrap();
    } else {
        axes.ylim(limits.0, limits.1).unwrap();
    }
    add(&mut axes);
    f
}

/// The marks of the one axes of `figure` as drawn on [`SQUARE`] instead of the plot the layout
/// chose, the baseline moved with it.
fn marks_on_square(figure: &Figure) -> Vec<Item> {
    let (resolved, mut layout) =
        crate::pipeline::plan_figure(figure, (0, 0), FontLibrary::global());
    layout.plot = SQUARE;
    layout.baseline = PlacedBaseline::on(&SQUARE, resolved.baseline);
    let marks = resolve_figure_marks(figure).swap_remove(0);
    match crate::draw::marks(figure.theme(), &resolved, &layout, &marks) {
        Some(Item::Group { items, .. }) => items,
        other => panic!("expected a marks group, got {other:?}"),
    }
}

/// Where the point `p` of a vertical form on [`SQUARE`] lies in the horizontal form: its
/// distance from the left edge becomes its distance from the top, and its height above the
/// bottom edge its distance from the left edge.
fn mirrored(p: Point) -> (f64, f64) {
    let (x, y) = (f64::from(p.x), f64::from(p.y));
    (SQUARE.x + (SQUARE.bottom() - y), SQUARE.y + (x - SQUARE.x))
}

#[track_caller]
fn assert_point_mirrors(vertical: Point, horizontal: Point, what: &str) {
    let (x, y) = mirrored(vertical);
    assert!(
        (f64::from(horizontal.x) - x).abs() < 1e-3 && (f64::from(horizontal.y) - y).abs() < 1e-3,
        "{what}: {horizontal:?} is not the mirror ({x}, {y}) of {vertical:?}"
    );
}

#[track_caller]
fn assert_path_mirrors(vertical: &Path, horizontal: &Path, what: &str) {
    assert_eq!(vertical.verbs().len(), horizontal.verbs().len(), "{what}");
    for (a, b) in vertical.verbs().iter().zip(horizontal.verbs()) {
        match (*a, *b) {
            (PathVerb::MoveTo(p), PathVerb::MoveTo(q))
            | (PathVerb::LineTo(p), PathVerb::LineTo(q)) => assert_point_mirrors(p, q, what),
            (PathVerb::CubicTo(p1, p2, p3), PathVerb::CubicTo(q1, q2, q3)) => {
                for (p, q) in [(p1, q1), (p2, q2), (p3, q3)] {
                    assert_point_mirrors(p, q, what);
                }
            }
            (PathVerb::Close, PathVerb::Close) => {}
            other => panic!("{what}: verbs differ: {other:?}"),
        }
    }
}

/// Asserts that the `horizontal` items are the `vertical` ones mirrored: the same items in the
/// same order, with the same paints and styles, every point at its mirror image.
#[track_caller]
fn assert_items_mirror(vertical: &[Item], horizontal: &[Item]) {
    assert_eq!(vertical.len(), horizontal.len());
    assert!(!vertical.is_empty());
    for (i, (a, b)) in vertical.iter().zip(horizontal).enumerate() {
        let what = format!("item {i}");
        match (a, b) {
            (
                Item::Fill { path, paint, rule },
                Item::Fill {
                    path: h_path,
                    paint: h_paint,
                    rule: h_rule,
                },
            ) => {
                assert_eq!((paint, rule), (h_paint, h_rule), "{what}");
                assert_path_mirrors(path, h_path, &what);
            }
            (
                Item::Stroke { path, style },
                Item::Stroke {
                    path: h_path,
                    style: h_style,
                },
            ) => {
                assert_eq!(style, h_style, "{what}");
                assert_path_mirrors(path, h_path, &what);
            }
            (
                Item::Markers {
                    shape,
                    fill,
                    stroke,
                    positions,
                },
                Item::Markers {
                    shape: h_shape,
                    fill: h_fill,
                    stroke: h_stroke,
                    positions: h_positions,
                },
            ) => {
                assert_eq!((shape, fill, stroke), (h_shape, h_fill, h_stroke), "{what}");
                assert_eq!(positions.len(), h_positions.len(), "{what}");
                for (&p, &q) in positions.iter().zip(h_positions) {
                    assert_point_mirrors(p, q, &what);
                }
            }
            other => panic!("{what}: items differ in kind: {other:?}"),
        }
    }
}

fn series(label: &str, values: &[f64]) -> Series {
    Series {
        label: label.into(),
        values: values.to_vec(),
        ..Series::default()
    }
}

#[test]
fn horizontal_bar_mirrors_vertical() {
    let cats = names(&["North", "East", "South", "West"]);
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let draw = |horizontal: bool| {
            let f = oriented(&theme, horizontal, (-4.0, 12.0), |a| {
                // Rounded ends on both sides of 0, a NaN gap and a bar cut by the limit.
                a.bar(
                    cats.clone(),
                    &[3.0, -2.5, f64::NAN, 15.0],
                    BarOptions::default(),
                )
                .unwrap()
                .grouped_bar(
                    cats.clone(),
                    vec![
                        series("2025", &[1.0, 2.0, -3.0, 4.0]),
                        series("2026", &[2.0, f64::NAN, 3.0, 5.0]),
                    ],
                    crate::ValueLabels::default(),
                )
                .unwrap()
                .stacked_bar(
                    cats.clone(),
                    vec![
                        series("Hydro", &[2.0, 3.0, -1.0, 1.0]),
                        series("Wind", &[1.0, -2.0, 4.0, 0.0]),
                        series("Solar", &[3.0, 1.0, 2.0, 2.0]),
                    ],
                    crate::ValueLabels::default(),
                )
                .unwrap();
            });
            marks_on_square(&f)
        };
        assert_items_mirror(&draw(false), &draw(true));
    }
}

#[test]
fn horizontal_box_and_violin_mirror_vertical() {
    let cats = names(&["North", "East", "South"]);
    let groups = vec![
        vec![1.0, 2.0, 3.0, 4.0, 5.0, 12.0],
        vec![2.0, 4.0, 5.0, 6.0, 8.0],
        vec![-4.0, 1.0, 2.0, 3.0, 4.0, 5.0, 30.0],
    ];
    let shifted: Vec<Vec<f64>> = groups
        .iter()
        .map(|g| g.iter().map(|v| v + 2.0).collect())
        .collect();
    let distributions = || {
        vec![
            Distribution {
                label: "Car".into(),
                groups: groups.clone(),
                ..Distribution::default()
            },
            Distribution {
                label: "Transit".into(),
                groups: shifted.clone(),
                ..Distribution::default()
            },
        ]
    };
    let theme = Theme::pigment();
    let draw = |horizontal: bool| {
        // The limits cut the long whisker and the outlier of the last group.
        let f = oriented(&theme, horizontal, (-6.0, 14.0), |a| {
            a.box_plot(cats.clone(), groups.clone(), BoxOptions::default())
                .unwrap()
                .grouped_box_plot(cats.clone(), distributions())
                .unwrap()
                .violin(cats.clone(), groups.clone(), ViolinOptions::default())
                .unwrap()
                .grouped_violin(cats.clone(), distributions())
                .unwrap();
        });
        marks_on_square(&f)
    };
    assert_items_mirror(&draw(false), &draw(true));
}

/// The centre of the bounds of `path`.
fn center(path: &Path) -> (f64, f64) {
    let (lo, hi) = path.bounds().unwrap();
    (
        (f64::from(lo.x) + f64::from(hi.x)) / 2.0,
        (f64::from(lo.y) + f64::from(hi.y)) / 2.0,
    )
}

/// The items of the marks group of `scene`, the one group with a clip, whatever else the scene
/// holds (a header with a legend row, for instance).
fn marks_of(scene: &crate::scene::Scene) -> &[Item] {
    scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group {
                clip: Some(_),
                items,
            } => Some(items.as_slice()),
            _ => None,
        })
        .expect("a clipped marks group")
}

/// The fill paths of the marks group of `figure`, in order.
fn fills(figure: &Figure) -> Vec<Path> {
    let scene = figure.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    marks_of(&scene)
        .iter()
        .filter_map(|item| match item {
            Item::Fill { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect()
}

fn horizontal_figure(theme: Theme) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category).unwrap();
    f
}

#[test]
fn categories_run_top_to_bottom() {
    let mut f = horizontal_figure(Theme::graphite());
    f.single_axes_mut()
        .bar(
            names(&["Gamma", "Alpha", "Beta"]),
            &[3.0, 5.0, 2.0],
            BarOptions::default(),
        )
        .unwrap()
        // A later mark adds its new category below the others.
        .bar(
            names(&["Alpha", "Delta"]),
            &[1.0, 4.0],
            BarOptions::default(),
        )
        .unwrap();
    let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
    let plot = layout.plot;
    let bars = fills(&f);
    assert_eq!(bars.len(), 5);
    // Bars grow rightwards from x = 0, where the baseline is.
    let zero = layout.baseline.at;
    assert_eq!(resolved.baseline.axis, crate::resolve::ValueAxis::X);
    // The first mark's bars, in axis order: Gamma, Alpha, Beta, at band centres top down.
    let n = 4.0;
    for (i, bar) in bars[..3].iter().enumerate() {
        let (_, y) = center(bar);
        let expected = plot.y + (i as f64 + 0.5) / n * plot.height;
        assert!((y - expected).abs() < 1e-3, "bar {i}: {y} vs {expected}");
        let (lo, hi) = bar.bounds().unwrap();
        assert!(
            (f64::from(lo.x) - zero).abs() < 1e-3,
            "bar {i} starts at {lo:?}"
        );
        assert!(f64::from(hi.x) > zero + 1.0);
    }
    // Delta, added last, is the bottom band.
    let (_, y) = center(&bars[4]);
    assert!((y - (plot.y + 3.5 / n * plot.height)).abs() < 1e-3);

    // The labels sit in the y tick column in the same order, right-aligned to the same edge.
    let labels: Vec<&str> = layout
        .y_tick_labels
        .iter()
        .map(|l| l.text.as_str())
        .collect();
    let order: Vec<f64> = layout.y_tick_labels.iter().map(|l| l.y).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    let mut sorted = layout.y_tick_labels.clone();
    sorted.sort_by(|a, b| a.y.total_cmp(&b.y));
    assert_eq!(
        sorted.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
        ["Gamma", "Alpha", "Beta", "Delta"]
    );
    assert_eq!(labels.len(), 4);
    let ends: Vec<f64> = layout
        .y_tick_labels
        .iter()
        .map(|l| l.x + l.width())
        .collect();
    assert!(
        ends.iter()
            .all(|&e| (e - ends[0]).abs() < 1e-9 && e < plot.x)
    );
}

#[test]
fn grouped_slots_follow_legend_order() {
    let mut f = horizontal_figure(Theme::pigment());
    f.single_axes_mut()
        .grouped_bar(
            names(&["A", "B"]),
            vec![
                series("North", &[3.0, 4.0]),
                series("South", &[2.0, 5.0]),
                series("East", &[1.0, 2.0]),
            ],
            crate::ValueLabels::default(),
        )
        .unwrap();
    let marks = resolve_figure_marks(&f).swap_remove(0);
    let legend: Vec<String> = marks[0]
        .legend_swatch()
        .into_iter()
        .map(|entry| entry.label().to_owned())
        .collect();
    assert_eq!(legend, ["North", "South", "East"]);
    let bars = fills(&f);
    assert_eq!(bars.len(), 6);
    // Within each category the slots run top to bottom in legend order.
    for category in bars.chunks(3) {
        let ys: Vec<f64> = category.iter().map(|bar| center(bar).1).collect();
        assert!(ys.windows(2).all(|w| w[0] < w[1]), "{ys:?}");
    }
    // The first category's group lies above the second's.
    assert!(center(&bars[2]).1 < center(&bars[3]).1);
}

#[test]
fn horizontal_violin_quantiles_meet_outline() {
    let mut values: Vec<f64> = (0..40)
        .map(|i| f64::from(i % 7) + f64::from(i) / 10.0)
        .collect();
    values.push(25.0);
    let mut f = horizontal_figure(Theme::pigment());
    f.single_axes_mut()
        .violin(names(&["A"]), vec![values], ViolinOptions::default())
        .unwrap();
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let items = mark_group(&scene).1;
    let [
        Item::Fill { path: outline, .. },
        Item::Stroke { .. },
        quantiles @ ..,
    ] = items
    else {
        panic!("violin outline and quantile lines, got {items:?}")
    };
    assert_eq!(quantiles.len(), 3);
    let (lo, hi) = outline.bounds().unwrap();
    // The violin is centred on its band across y.
    let cy = (f64::from(lo.y) + f64::from(hi.y)) / 2.0;
    let polygon: Vec<(f64, f64)> = outline
        .verbs()
        .iter()
        .filter_map(|verb| match verb {
            PathVerb::MoveTo(p) | PathVerb::LineTo(p) => Some((f64::from(p.x), f64::from(p.y))),
            PathVerb::Close => None,
            other => panic!("polygon expected, got {other:?}"),
        })
        .collect();
    for (i, (item, fraction)) in quantiles.iter().zip([0.5, 0.5, 1.0]).enumerate() {
        let Item::Stroke { path, .. } = item else {
            panic!("quantile stroke")
        };
        let [PathVerb::MoveTo(top), PathVerb::LineTo(bottom)] = path.verbs() else {
            panic!("two-point quantile line")
        };
        // Quantile lines run across the violin, vertically.
        assert_eq!(top.x, bottom.x);
        let x = f64::from(top.x);
        let outline_y = polygon
            .iter()
            .zip(polygon.iter().cycle().skip(1))
            .filter(|(a, b)| a.1 <= cy && b.1 <= cy && x >= a.0.min(b.0) && x <= a.0.max(b.0))
            .map(|(a, b)| {
                if a.0 == b.0 {
                    a.1.min(b.1)
                } else {
                    a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0)
                }
            })
            .reduce(f64::min)
            .expect("outline crosses the quantile");
        let half = fraction * (cy - outline_y);
        for (end, actual, expected) in [
            ("top", f64::from(top.y), cy - half),
            ("bottom", f64::from(bottom.y), cy + half),
        ] {
            let error = (actual - expected).abs();
            assert!(
                error <= 1e-3,
                "quantile {i} {end}: endpoint error {error} pt"
            );
        }
    }
}

#[test]
fn dot_plot_points_at_band_centres() {
    let mut f = horizontal_figure(Theme::midnight());
    f.single_axes_mut()
        .xlim(50.0, 90.0)
        .unwrap()
        .scatter_shared(
            AxisData::Numbers(vec![78.7, 81.0, 54.5, 60.0]),
            AxisData::Categories(names(&["United States", "Germany", "Nigeria", "Germany"])),
            ScatterOptions::default(),
        )
        .unwrap();
    let (_, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
    let plot = layout.plot;
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    assert_scene_bounded(&scene);
    let [Item::Markers { positions, .. }] = mark_group(&scene).1 else {
        panic!("one batch of markers")
    };
    let band = |i: f64| plot.y + (i + 0.5) / 3.0 * plot.height;
    let at = |v: f64| plot.x + (v - 50.0) / 40.0 * plot.width;
    let expected = [
        (at(78.7), band(0.0)),
        (at(81.0), band(1.0)),
        (at(54.5), band(2.0)),
        (at(60.0), band(1.0)),
    ];
    assert_eq!(positions.len(), expected.len());
    for (p, (x, y)) in positions.iter().zip(expected) {
        assert!(
            (f64::from(p.x) - x).abs() < 1e-3 && (f64::from(p.y) - y).abs() < 1e-3,
            "{p:?} vs ({x}, {y})"
        );
    }
}

#[test]
fn horizontal_charts_draw_vertical_gridlines_and_baseline() {
    let mut f = horizontal_figure(Theme::pigment());
    f.single_axes_mut()
        .bar(
            names(&["A", "B", "C"]),
            &[3.0, 7.0, 5.0],
            BarOptions::default(),
        )
        .unwrap();
    let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let lines = strokes(&scene);
    // One gridline per x tick but the one the baseline replaces, then the baseline; all of them
    // vertical, and none at the categories.
    assert_eq!(lines.len(), resolved.x.ticks.len());
    assert!(layout.y_grid.is_empty());
    for (path, _) in &lines {
        let [PathVerb::MoveTo(a), PathVerb::LineTo(b)] = path.verbs() else {
            panic!("a straight line")
        };
        assert_eq!(a.x, b.x, "{path:?}");
        assert!((f64::from(a.y) - layout.plot.y).abs() < 1e-3, "{path:?}");
        assert!(
            (f64::from(b.y) - layout.plot.bottom()).abs() < 1e-3,
            "{path:?}"
        );
    }
    let (baseline, _) = lines.last().unwrap();
    let PathVerb::MoveTo(at) = baseline.verbs()[0] else {
        panic!("a straight line")
    };
    assert!((f64::from(at.x) - layout.baseline.at).abs() < 1e-3);
    // The category labels are the y tick labels, drawn in the chrome.
    let texts = chrome_items(&scene)
        .iter()
        .filter(|item| matches!(item, Item::Text(_)))
        .count();
    assert_eq!(
        texts,
        layout.y_tick_labels.len() + layout.x_tick_labels.len()
    );
}

/// 100 category names of 60 characters, in Latin, which the bundled fonts draw.
fn long_names() -> Vec<String> {
    (0..100)
        .map(|i| {
            format!("{i:03} {}", "category name ".repeat(4))
                .chars()
                .take(60)
                .collect()
        })
        .collect()
}

#[test]
fn many_long_y_categories_fit_or_name_the_label() {
    // The bundled fonts alone, without system fonts, so the widths are the same everywhere.
    let lib = FontLibrary::with_fallbacks(Vec::new());
    let labels = long_names();
    let heights: Vec<f64> = (0..100).map(|i| f64::from(i % 9) + 1.0).collect();
    let figure = |width: f64| {
        let mut f = Figure::new(width, 30.0, Theme::graphite()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Category)
            .unwrap()
            .bar(labels.clone(), &heights, BarOptions::default())
            .unwrap();
        f
    };
    // A wide figure reserves the label column: the plot starts right of the widest label.
    let wide = figure(16.0);
    let (_, layout) = crate::pipeline::plan_figure(&wide, (0, 0), &lib);
    assert_eq!(layout.y_tick_labels.len(), 100);
    let widest = layout
        .y_tick_labels
        .iter()
        .map(|l| l.width())
        .fold(0.0, f64::max);
    assert!(layout.plot.x > widest, "{} vs {widest}", layout.plot.x);
    assert!(layout.y_tick_labels.iter().all(|l| l.x >= 0.0));
    assert_eq!(layout.y_tick_labels[0].text, labels[0]);
    assert!(wide.to_scene(&lib).is_ok());
    // A narrow one has no room for the plot, and the error names the widest label.
    let err = figure(3.0).to_scene(&lib).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err:?}");
    let message = err.to_string();
    // The message quotes the label, shortened with an ellipsis when it is long.
    let quoted = message
        .strip_prefix("Figure.to_scene(): y tick label \"")
        .and_then(|rest| rest.split(['"', '…']).next())
        .unwrap_or_else(|| panic!("the error names a label: {message}"));
    let widest_label = layout
        .y_tick_labels
        .iter()
        .find(|l| l.width() == widest)
        .unwrap();
    assert!(
        quoted.len() > 20 && widest_label.text.starts_with(quoted),
        "{message} names {quoted:?}, not the widest label {:?}",
        widest_label.text
    );
    assert_eq!(
        err.hint(),
        Some("shorten the y tick labels or widen the figure.")
    );
}

#[test]
fn cjk_y_categories_render() {
    // Their widths depend on the system fonts that draw them, so only rendering is checked.
    let names: Vec<String> = (0..40)
        .map(|i| format!("{i:02} {}", "類別名稱".repeat(6)))
        .collect();
    let mut f = Figure::new(16.0, 12.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category)
        .unwrap()
        .bar(names, &[1.0; 40], BarOptions::default())
        .unwrap();
    assert_eq!(fills(&f).len(), 40);
}

/// The items of `items`, the items of groups in their place.
fn flattened(items: &[Item]) -> Vec<&Item> {
    items
        .iter()
        .flat_map(|item| match item {
            Item::Group { items, .. } => flattened(items),
            other => vec![other],
        })
        .collect()
}

#[test]
fn vertical_reference_lines_run_the_full_height_of_a_horizontal_chart() {
    use crate::model::{AxisValue, RuleOptions, SpanOptions};
    let mut f = horizontal_figure(Theme::pigment());
    f.single_axes_mut()
        .scatter_shared(
            AxisData::Numbers(vec![54.5, 83.7]),
            AxisData::Categories(names(&["a", "b"])),
            ScatterOptions::default(),
        )
        .unwrap()
        .vline(AxisValue::Number(70.0), RuleOptions::default())
        .unwrap()
        .vspan(60.0, 65.0, SpanOptions::default())
        .unwrap();
    let (resolved, layout) = crate::pipeline::plan_figure(&f, (0, 0), FontLibrary::global());
    // The baseline sits at the x axis's lower limit, on the left.
    assert!(!resolved.x.holds_zero());
    assert_eq!(layout.baseline.at, layout.plot.x);
    let scene = f.to_scene(FontLibrary::global()).unwrap();
    let items = flattened(mark_group(&scene).1);
    let [span, Item::Markers { .. }, line] = items.as_slice() else {
        panic!("a span, the dots and a line, got {items:?}")
    };
    let (Item::Fill { path: span, .. }, Item::Stroke { path: line, .. }) = (span, line) else {
        panic!("a span fill and a line stroke")
    };
    let (plot_top, plot_bottom) = (layout.plot.y, layout.plot.bottom());
    for path in [span, line] {
        let (lo, hi) = path.bounds().unwrap();
        assert!((f64::from(lo.y) - plot_top).abs() < 1e-3, "{lo:?}");
        assert!((f64::from(hi.y) - plot_bottom).abs() < 1e-3, "{hi:?}");
    }
}
