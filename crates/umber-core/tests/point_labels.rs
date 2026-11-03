//! The values written next to scatter points, through the public API: which value, its format
//! and color, the fixed sides and the limits they grow, the automatic place, what it avoids
//! and the automatic limits it grows, and the errors.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use umber_core::scene::{Item, Scene, TextItem};
use umber_core::{
    AxesMut, AxisData, AxisValue, BarOptions, ColorBy, ColorSpec, Error, ErrorBarOptions, Figure,
    FontLibrary, GridIndex, Instant, LineOptions, Marker, Place, PointColor, PointLabels, Rgba,
    ScaleKind, ScatterOptions, SizeSpec, SvgText, TextOptions, Theme, TickFormat, ValueLabels,
};

const TOLERANCE: f64 = 2e-3;

fn figure(x: ScaleKind, y: ScaleKind) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure.add_axes(x, y).unwrap();
    figure
}

fn axes(figure: &mut Figure) -> AxesMut<'_> {
    figure
        .axes_at_mut(GridIndex::Auto, GridIndex::Auto)
        .unwrap()
}

fn render(figure: &Figure) -> Scene {
    figure.to_scene(FontLibrary::global()).unwrap()
}

fn render_err(figure: &Figure) -> Error {
    figure.to_scene(FontLibrary::global()).unwrap_err()
}

fn nums(v: &[f64]) -> AxisData {
    AxisData::Numbers(v.to_vec())
}

fn names(v: &[&str]) -> AxisData {
    AxisData::Categories(v.iter().map(|s| (*s).to_owned()).collect())
}

fn labels(place: Place) -> PointLabels {
    PointLabels {
        annotate: true,
        place,
        ..PointLabels::default()
    }
}

fn options(place: Place) -> ScatterOptions {
    ScatterOptions {
        labels: labels(place),
        ..ScatterOptions::default()
    }
}

/// Every leaf item of `items`, groups flattened, in drawing order.
fn leaves(items: &[Item]) -> Vec<&Item> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Item::Group { items, .. } => out.extend(leaves(items)),
            other => out.push(other),
        }
    }
    out
}

/// The text items of `scene`, in drawing order.
fn texts(scene: &Scene) -> Vec<&TextItem> {
    leaves(&scene.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Text(t) => Some(t),
            _ => None,
        })
        .collect()
}

/// The contents of the text items of `scene`.
fn contents(scene: &Scene) -> Vec<&str> {
    texts(scene).into_iter().map(|t| t.text.as_str()).collect()
}

/// The last text item of `scene` that reads `content`: value labels are drawn after the tick
/// labels, which can read the same.
fn text<'a>(scene: &'a Scene, content: &str) -> &'a TextItem {
    texts(scene)
        .into_iter()
        .rfind(|t| t.text == content)
        .unwrap_or_else(|| panic!("no text {content:?} in {:?}", contents(scene)))
}

/// The box of `item`: left, top, right, bottom, from its advance width and font metrics.
fn text_box(scene: &Scene, item: &TextItem) -> [f64; 4] {
    let fonts = FontLibrary::global()
        .with_figure_fonts(&scene.fonts)
        .unwrap();
    let layout = fonts.layout(&item.text, &item.style).unwrap();
    let (ascent, descent) = (f64::from(layout.ascent()), f64::from(layout.descent()));
    let (x, y) = (f64::from(item.origin.x), f64::from(item.origin.y));
    [x, y - ascent, x + f64::from(item.width), y + descent]
}

/// The centres of every marker of `scene` and the reach of each from its centre, its shape's
/// half extent plus its whole outline width (the reach of a square, whose miter corner reaches
/// that far), along x and y.
fn markers(scene: &Scene) -> Vec<((f64, f64), (f64, f64))> {
    leaves(&scene.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Markers {
                shape,
                stroke,
                positions,
                ..
            } => {
                let (lo, hi) = shape.bounds().unwrap();
                let width = stroke.as_ref().map_or(0.0, |s| f64::from(s.width));
                let reach = (
                    f64::from(hi.x - lo.x) / 2.0 + width,
                    f64::from(hi.y - lo.y) / 2.0 + width,
                );
                Some(
                    positions
                        .iter()
                        .map(|p| ((f64::from(p.x), f64::from(p.y)), reach))
                        .collect::<Vec<_>>(),
                )
            }
            _ => None,
        })
        .flatten()
        .collect()
}

/// The plot area of the one axes of `scene`: the clip of its marks (with no line, the plot).
fn plot(scene: &Scene) -> [f64; 4] {
    scene
        .items
        .iter()
        .find_map(|item| match item {
            Item::Group { clip: Some(r), .. } => Some([
                f64::from(r.x),
                f64::from(r.y),
                f64::from(r.x + r.width),
                f64::from(r.y + r.height),
            ]),
            _ => None,
        })
        .unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < TOLERANCE
}

fn inside(inner: [f64; 4], outer: [f64; 4]) -> bool {
    inner[0] >= outer[0] - 1e-6
        && inner[1] >= outer[1] - 1e-6
        && inner[2] <= outer[2] + 1e-6
        && inner[3] <= outer[3] + 1e-6
}

/// The side of its point that the box `label` lies on, for a point at `centre`.
fn side(label: [f64; 4], (x, y): (f64, f64)) -> Place {
    if label[3] <= y && label[0] < x && x < label[2] {
        Place::Above
    } else if label[1] >= y && label[0] < x && x < label[2] {
        Place::Below
    } else if label[0] >= x {
        Place::Right
    } else {
        Place::Left
    }
}

/// A figure on linear axes from 0 to 10 holding one scatter of `points` labelled at `place`.
fn points(points: &[(f64, f64)], place: Place) -> Figure {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    let (xs, ys): (Vec<f64>, Vec<f64>) = points.iter().copied().unzip();
    axes(&mut f)
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .scatter(AxisData::Numbers(xs), &ys, options(place))
        .unwrap();
    f
}

/// The side the label `content` of the point drawn at index `point` takes in `scene`.
fn side_of(scene: &Scene, content: &str, point: usize) -> Place {
    side(
        text_box(scene, text(scene, content)),
        markers(scene)[point].0,
    )
}

// ---- Which value, and how it is written ----------------------------------------------------

#[test]
fn the_value_is_the_coordinate_on_the_numeric_axis() {
    // A category y axis shows x.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Category);
    axes(&mut f)
        .scatter_shared(
            nums(&[71.25, 74.5]),
            names(&["Base", "Gate"]),
            options(Place::Auto),
        )
        .unwrap();
    let scene = render(&f);
    text(&scene, "71.25");
    text(&scene, "74.5");
    // A category x axis shows y.
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    axes(&mut f)
        .scatter_shared(names(&["A", "B"]), nums(&[3.5, 6.0]), options(Place::Auto))
        .unwrap();
    let scene = render(&f);
    text(&scene, "3.5");
    text(&scene, "6");
    // Two numeric axes show y.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .scatter(nums(&[1.5, 2.5]), &[10.25, 20.75], options(Place::Auto))
        .unwrap();
    let scene = render(&f);
    text(&scene, "10.25");
    text(&scene, "20.75");
    assert!(!contents(&scene).contains(&"1.5"));
}

#[test]
fn two_category_axes_are_refused_when_the_call_is_made() {
    let mut f = figure(ScaleKind::Category, ScaleKind::Category);
    let err = axes(&mut f)
        .scatter_shared(names(&["A"]), names(&["B"]), options(Place::Auto))
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }), "{err}");
    let err = axes(&mut f)
        .grouped_scatter(
            vec![("North".into(), names(&["A"]), names(&["B"]))],
            Marker::Circle,
            SizeSpec::Auto,
            labels(Place::Auto),
        )
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }), "{err}");
}

#[test]
fn times_on_the_value_axis_are_written_in_its_format() {
    let day = |d: i64| Instant::from_micros((20_515 + d) * 86_400_000_000).unwrap();
    let times = AxisData::Times(vec![Some(day(0)), Some(day(14))]);
    let mut f = figure(ScaleKind::Category, ScaleKind::Time);
    axes(&mut f)
        .scatter_shared(names(&["A", "B"]), times.clone(), options(Place::Auto))
        .unwrap();
    let scene = render(&f);
    text(&scene, "Mar 3");
    text(&scene, "Mar 17");
    let mut f = figure(ScaleKind::Linear, ScaleKind::Time);
    let iso = ScatterOptions {
        labels: PointLabels {
            format: Some(TickFormat::iso()),
            ..labels(Place::Right)
        },
        ..ScatterOptions::default()
    };
    axes(&mut f)
        .scatter_shared(nums(&[1.0, 2.0]), times, iso)
        .unwrap();
    let scene = render(&f);
    text(&scene, "2026-03-03");
    let label = text_box(&scene, text(&scene, "2026-03-17"));
    assert!(
        inside(label, plot(&scene)),
        "the fixed side fits on a time axis"
    );
}

#[test]
fn the_automatic_format_follows_the_value_axis_and_an_explicit_one_wins() {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .ytick_format(TickFormat::percent(0).unwrap())
        .unwrap()
        .scatter(nums(&[1.0, 2.0]), &[0.25, 0.5], options(Place::Auto))
        .unwrap();
    let scene = render(&f);
    text(&scene, "25%");
    text(&scene, "50%");
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    let fixed = ScatterOptions {
        labels: PointLabels {
            format: Some(TickFormat::fixed(2).unwrap().affix("", " s").unwrap()),
            ..labels(Place::Auto)
        },
        ..ScatterOptions::default()
    };
    axes(&mut f)
        .ytick_format(TickFormat::percent(0).unwrap())
        .unwrap()
        .scatter(nums(&[1.0, 2.0]), &[0.25, 0.5], fixed)
        .unwrap();
    let scene = render(&f);
    text(&scene, "0.25 s");
    text(&scene, "0.50 s");
    // The automatic format writes each value on its own.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .scatter(nums(&[1.0, 2.0]), &[71.25, 2.5e6], options(Place::Auto))
        .unwrap();
    let scene = render(&f);
    text(&scene, "71.25");
    text(&scene, "2.5e6");
}

#[test]
fn the_label_size_is_the_tick_label_size_unless_given() {
    let f = points(&[(5.0, 5.0)], Place::Auto);
    let scene = render(&f);
    let tick = texts(&scene)[0].style.size;
    assert_eq!(text(&scene, "5").style.size, tick);
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    let sized = ScatterOptions {
        labels: PointLabels {
            size: SizeSpec::pt(13.0).unwrap(),
            ..labels(Place::Auto)
        },
        ..ScatterOptions::default()
    };
    axes(&mut f).scatter(nums(&[1.0]), &[4.0], sized).unwrap();
    assert!((text(&render(&f), "4").style.size - 13.0).abs() < 1e-6);
}

// ---- Color -----------------------------------------------------------------------------------

#[test]
fn labels_take_the_series_color_at_full_opacity() {
    let theme = Theme::pigment();
    let palette = theme.palette();
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    let red = Rgba {
        r: 200,
        g: 30,
        b: 40,
        a: 128,
    };
    axes(&mut f)
        .scatter(nums(&[1.0]), &[1.5], options(Place::Auto))
        .unwrap()
        .scatter(
            nums(&[2.0]),
            &[2.5],
            ScatterOptions {
                color: PointColor::Uniform(ColorSpec::Fixed(red)),
                ..options(Place::Auto)
            },
        )
        .unwrap();
    let scene = render(&f);
    assert_eq!(
        text(&scene, "1.5").color,
        Rgba {
            a: 255,
            ..palette[0]
        }
    );
    assert_eq!(text(&scene, "2.5").color, Rgba { a: 255, ..red });
    // With groups, each group's labels take the group's color.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .grouped_scatter(
            vec![
                ("North".into(), nums(&[1.0]), nums(&[3.5])),
                ("South".into(), nums(&[2.0]), nums(&[4.5])),
            ],
            Marker::Circle,
            SizeSpec::Auto,
            labels(Place::Auto),
        )
        .unwrap();
    let scene = render(&f);
    assert_eq!(
        text(&scene, "3.5").color,
        Rgba {
            a: 255,
            ..palette[0]
        }
    );
    assert_eq!(
        text(&scene, "4.5").color,
        Rgba {
            a: 255,
            ..palette[1]
        }
    );
    // A color-mapped scatter's labels take the theme's text color.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    let by = ColorBy::new(vec![0.0, 1.0]);
    axes(&mut f)
        .scatter(
            nums(&[1.0, 2.0]),
            &[5.5, 6.5],
            ScatterOptions {
                color: PointColor::By(by),
                ..options(Place::Auto)
            },
        )
        .unwrap();
    let scene = render(&f);
    assert_eq!(text(&scene, "5.5").color, theme.text_color());
    assert_eq!(text(&scene, "6.5").color, theme.text_color());
}

// ---- Fixed sides ---------------------------------------------------------------------------

#[test]
fn each_fixed_side_sits_3_pt_beyond_the_marker_edge() {
    for place in [Place::Above, Place::Below, Place::Left, Place::Right] {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        axes(&mut f)
            .xlim(0.0, 10.0)
            .unwrap()
            .ylim(0.0, 10.0)
            .unwrap()
            .scatter(
                nums(&[5.0]),
                &[5.0],
                ScatterOptions {
                    marker: Marker::Square,
                    ..options(place)
                },
            )
            .unwrap();
        let scene = render(&f);
        let [((x, y), (reach_x, reach_y))] = markers(&scene)[..] else {
            panic!("one marker expected");
        };
        let [left, top, right, bottom] = text_box(&scene, text(&scene, "5"));
        match place {
            Place::Above => {
                assert!(close(bottom, y - reach_y - 3.0), "{place:?}");
                assert!(close((left + right) / 2.0, x), "{place:?}");
            }
            Place::Below => {
                assert!(close(top, y + reach_y + 3.0), "{place:?}");
                assert!(close((left + right) / 2.0, x), "{place:?}");
            }
            Place::Left => {
                assert!(close(right, x - reach_x - 3.0), "{place:?}");
                assert!(close((top + bottom) / 2.0, y), "{place:?}");
            }
            Place::Right => {
                assert!(close(left, x + reach_x + 3.0), "{place:?}");
                assert!(close((top + bottom) / 2.0, y), "{place:?}");
            }
            Place::Auto => unreachable!(),
        }
    }
}

#[test]
fn a_fixed_side_never_moves_even_over_marks() {
    // A line through the point: a fixed side still sits on it.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .line(nums(&[0.0, 10.0]), &[5.0, 5.0], LineOptions::default())
        .unwrap()
        .scatter(nums(&[5.0]), &[5.0], options(Place::Right))
        .unwrap();
    let scene = render(&f);
    assert_eq!(side_of(&scene, "5", 0), Place::Right);
}

#[test]
fn automatic_limits_grow_so_that_fixed_sides_fit() {
    let data = [(0.0, 0.0), (5.0, 3.0), (10.0, 10.0)];
    for place in [Place::Above, Place::Below, Place::Left, Place::Right] {
        for height in [1.6, 2.4, 4.0] {
            let mut f = Figure::new(3.0, height, Theme::pigment()).unwrap();
            let (xs, ys): (Vec<f64>, Vec<f64>) = data.iter().copied().unzip();
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .scatter(AxisData::Numbers(xs), &ys, options(place))
                .unwrap();
            let scene = render(&f);
            for label in ["0", "3", "10"] {
                let label_box = text_box(&scene, text(&scene, label));
                assert!(
                    inside(label_box, plot(&scene)),
                    "{place:?} at height {height}: {label} at {label_box:?} leaves {:?}",
                    plot(&scene)
                );
            }
        }
    }
    // Automatic labels that all fit inside the plot leave the limits as they are without
    // labels: the points sit where they sit without labels.
    let plain = |labels: PointLabels| {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        let (xs, ys): (Vec<f64>, Vec<f64>) = data.iter().copied().unzip();
        axes(&mut f)
            .scatter(
                AxisData::Numbers(xs),
                &ys,
                ScatterOptions {
                    labels,
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        markers(&render(&f))
    };
    assert_eq!(plain(labels(Place::Auto)), plain(PointLabels::default()));
    assert_ne!(plain(labels(Place::Above)), plain(PointLabels::default()));
}

#[test]
fn a_fixed_side_beyond_explicit_limits_names_the_series() {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .ylim(0.0, 10.0)
        .unwrap()
        .scatter(
            nums(&[1.0, 2.0]),
            &[5.0, 9.9],
            ScatterOptions {
                label: "Long training".into(),
                ..options(Place::Above)
            },
        )
        .unwrap();
    let err = render_err(&f);
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the value label \"9.9\" of the series \"Long training\" does not \
         fit inside the plot."
    );
    assert_eq!(
        err.hint(),
        Some("widen xlim or ylim, or let the limits be automatic.")
    );
}

// ---- The automatic place -------------------------------------------------------------------

#[test]
fn the_automatic_place_tries_above_right_below_left() {
    // Room everywhere: above.
    let scene = render(&points(&[(5.0, 5.0)], Place::Auto));
    assert_eq!(side_of(&scene, "5", 0), Place::Above);
    // Near the top edge: right.
    let scene = render(&points(&[(5.0, 9.7)], Place::Auto));
    assert_eq!(side_of(&scene, "9.7", 0), Place::Right);
    // Near the top right corner: below.
    let scene = render(&points(&[(9.7, 9.7)], Place::Auto));
    assert_eq!(side_of(&scene, "9.7", 0), Place::Below);
    // Near the top right corner, with a mark below: left.
    let mut f = points(&[(9.7, 9.7)], Place::Auto);
    axes(&mut f)
        .y_error_bars(nums(&[9.7]), &[8.0], &[9.7], ErrorBarOptions::default())
        .unwrap();
    let scene = render(&f);
    assert_eq!(side_of(&scene, "9.7", 0), Place::Left);
}

#[test]
fn automatic_labels_avoid_error_bars_lines_markers_and_labels() {
    // A vertical error bar through the point blocks above and below.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .y_error_bars(nums(&[5.0]), &[3.0], &[7.0], ErrorBarOptions::default())
        .unwrap()
        .scatter(nums(&[5.0]), &[5.0], options(Place::Auto))
        .unwrap();
    assert_eq!(side_of(&render(&f), "5", 0), Place::Right);
    // A horizontal error bar blocks right and left but not above.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .x_error_bars(&[10.0], &[3.0], &[7.0], ErrorBarOptions::default())
        .unwrap()
        .scatter(nums(&[5.0]), &[10.0], options(Place::Auto))
        .unwrap();
    assert_eq!(side_of(&render(&f), "10", 0), Place::Below);
    // A line rising steeply through the point blocks above and below as a stroke; it would
    // block every side as a bounding box.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .line(nums(&[4.5, 5.5]), &[0.0, 10.0], LineOptions::default())
        .unwrap()
        .scatter(nums(&[5.0]), &[5.0], options(Place::Auto))
        .unwrap();
    assert_eq!(side_of(&render(&f), "5", 0), Place::Right);
    // Another scatter's marker just above blocks above.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .scatter(nums(&[5.0]), &[5.4], ScatterOptions::default())
        .unwrap()
        .scatter(nums(&[5.0]), &[5.0], options(Place::Auto))
        .unwrap();
    let scene = render(&f);
    assert_eq!(side_of(&scene, "5", 1), Place::Right);
    // A label placed before blocks a later one: the first point's label sits above it, over
    // the second point's above.
    let scene = render(&points(&[(5.0, 5.0), (5.1, 5.03)], Place::Auto));
    let first = text_box(&scene, texts(&scene).iter().rev().nth(1).unwrap());
    let second = text_box(&scene, texts(&scene).last().unwrap());
    let centres: Vec<_> = markers(&scene).into_iter().map(|(c, _)| c).collect();
    assert_eq!(side(first, centres[0]), Place::Above);
    assert_eq!(side(second, centres[1]), Place::Right);
    // Bar labels are placed first and block point labels: with the line through the point
    // and the marker over it, only below is clear, until the bar's label takes that place.
    let with_bar_label = |annotate: bool| {
        let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
        let category = |name: &str| vec![name.to_owned()];
        // The line comes first, so the categories run Z, A, B and it passes both sides of A.
        axes(&mut f)
            .ylim(0.0, 10.0)
            .unwrap()
            .line(
                names(&["Z", "A", "B"]),
                &[4.75, 4.75, 4.75],
                LineOptions::default(),
            )
            .unwrap()
            .bar(
                category("A"),
                &[3.9],
                BarOptions {
                    annotation: ValueLabels {
                        annotate,
                        ..ValueLabels::default()
                    },
                    ..BarOptions::default()
                },
            )
            .unwrap()
            .scatter_shared(names(&["A"]), nums(&[5.1]), ScatterOptions::default())
            .unwrap()
            .scatter_shared(names(&["A"]), nums(&[4.75]), options(Place::Auto))
            .unwrap();
        f
    };
    let scene = render(&with_bar_label(false));
    assert_eq!(side_of(&scene, "4.75", 1), Place::Below);
    let err = render_err(&with_bar_label(true));
    assert!(
        err.to_string()
            .contains("\"4.75\" of scatter 2 has no clear position"),
        "{err}"
    );
}

#[test]
fn text_and_annotations_are_not_obstacles() {
    let mut f = points(&[(5.0, 5.0)], Place::Auto);
    axes(&mut f)
        .text(
            "note",
            (AxisValue::Number(5.0), AxisValue::Number(5.6)),
            TextOptions::default(),
        )
        .unwrap();
    assert_eq!(side_of(&render(&f), "5", 0), Place::Above);
}

#[test]
fn placement_is_deterministic() {
    let build = || {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        let xs: Vec<f64> = (0..40).map(|i| f64::from(i % 8) * 1.2).collect();
        let ys: Vec<f64> = (0..40).map(|i| f64::from(i / 8) * 0.9 + 0.1).collect();
        axes(&mut f)
            .line(nums(&xs[..8]), &ys[..8], LineOptions::default())
            .unwrap()
            .scatter(AxisData::Numbers(xs), &ys, options(Place::Auto))
            .unwrap();
        f
    };
    let (a, b) = (build(), build());
    assert_eq!(render(&a), render(&b));
    assert_eq!(
        a.to_svg(SvgText::Editable).unwrap(),
        b.to_svg(SvgText::Editable).unwrap()
    );
}

#[test]
fn no_clear_position_names_the_series_and_the_value() {
    let crowded = |label: &str| {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        let mut ax = axes(&mut f);
        ax.xlim(0.0, 10.0).unwrap().ylim(0.0, 10.0).unwrap();
        ax.scatter(nums(&[2.0]), &[2.0], ScatterOptions::default())
            .unwrap();
        // A grid of markers around the point leaves no side clear.
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for i in -6..=6 {
            for j in -6..=6 {
                xs.push(5.0 + f64::from(i) * 0.12);
                ys.push(5.0 + f64::from(j) * 0.12);
            }
        }
        ax.scatter(AxisData::Numbers(xs), &ys, ScatterOptions::default())
            .unwrap()
            .scatter(
                nums(&[5.0]),
                &[5.0],
                ScatterOptions {
                    label: label.into(),
                    ..options(Place::Auto)
                },
            )
            .unwrap();
        render_err(&f)
    };
    let err = crowded("Long training");
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the value label \"5\" of the series \"Long training\" has no clear \
         position: above, right, below and left of its point each leave the plot or overlap a \
         mark or a label."
    );
    assert_eq!(
        err.hint(),
        Some(
            "use place=Place.ABOVE (or another side) to fix the side, a larger figure, or a \
             smaller label size."
        )
    );
    let err = crowded("");
    assert!(
        err.to_string()
            .contains("the value label \"5\" of scatter 3 has"),
        "{err}"
    );
}

// ---- Automatic limits that make room for automatic labels -----------------------------------

/// Fold scores of two series on a category x axis, each joined by a thin line, with error bars
/// on the mean: the highest point has lines on both sides, so only above is clear. `ylim` sets
/// explicit limits; `annotate` labels the points automatically.
fn matched_folds(ylim: Option<(f64, f64)>, annotate: bool) -> Figure {
    let folds = ["Fold 1", "Fold 2", "Fold 3", "Fold 4", "Fold 5", "Mean"];
    let series = [
        ("80 epochs", [66.8, 68.1, 65.4, 69.0, 67.3, 67.32], 1.24),
        ("Long training", [78.4, 80.2, 76.9, 81.7, 79.5, 79.34], 1.6),
    ];
    let mut f = Figure::new(6.4, 3.8, Theme::pigment()).unwrap();
    let mut ax = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
    if let Some((low, high)) = ylim {
        ax.ylim(low, high).unwrap();
    }
    for (label, values, sd) in series {
        let mean = values[5];
        ax.line(
            names(&folds),
            &values,
            LineOptions {
                width: SizeSpec::pt(0.8).unwrap(),
                ..LineOptions::default()
            },
        )
        .unwrap()
        .y_error_bars(
            names(&["Mean"]),
            &[mean - sd],
            &[mean + sd],
            ErrorBarOptions::default(),
        )
        .unwrap()
        .scatter_shared(
            names(&folds),
            nums(&values),
            ScatterOptions {
                label: label.into(),
                labels: PointLabels {
                    annotate,
                    format: annotate.then(|| TickFormat::fixed(1).unwrap()),
                    ..PointLabels::default()
                },
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    }
    f
}

/// The y value at the scene height `at` in a matched-folds scene: its data markers come first,
/// six per series, and map y linearly.
fn folds_value(scene: &Scene, at: f64) -> f64 {
    let centres: Vec<_> = markers(scene).into_iter().map(|(c, _)| c).collect();
    // Fold 3 of the first series and Fold 4 of the second.
    let ((_, low), (_, high)) = (centres[2], centres[9]);
    65.4 + (at - low) * (81.7 - 65.4) / (high - low)
}

#[test]
fn automatic_limits_grow_to_hold_the_label_above_the_highest_point() {
    let plain = render(&matched_folds(None, false));
    let old_top = folds_value(&plain, plot(&plain)[1]);
    let scene = render(&matched_folds(None, true));
    assert_eq!(side_of(&scene, "81.7", 9), Place::Above);
    let label = text_box(&scene, text(&scene, "81.7"));
    assert!(
        inside(label, plot(&scene)),
        "{label:?} leaves {:?}",
        plot(&scene)
    );
    // The label reaches past the upper limit the plot has without labels, and the limits grew
    // to hold it.
    assert!(
        folds_value(&scene, label[1]) > old_top,
        "{} <= {old_top}",
        folds_value(&scene, label[1])
    );
    // Every other label still fits inside the plot.
    for value in [
        "66.8", "65.4", "69.0", "67.3", "78.4", "80.2", "76.9", "79.5",
    ] {
        let other = text_box(&scene, text(&scene, value));
        assert!(inside(other, plot(&scene)), "{value} at {other:?}");
    }
}

#[test]
fn explicit_limits_without_room_still_fail() {
    let err = render_err(&matched_folds(Some((62.0, 82.0)), true));
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the value label \"81.7\" of the series \"Long training\" has no \
         clear position: above, right, below and left of its point each leave the plot or \
         overlap a mark or a label."
    );
    assert_eq!(
        err.hint(),
        Some(
            "use place=Place.ABOVE (or another side) to fix the side, a larger figure, or a \
             smaller label size."
        )
    );
}

/// A point at the right end of the data, at x = 10, with a horizontal line into it from the
/// left and a vertical error bar through it, so that only right is clear, labelled
/// automatically when `annotate`; `xlim` sets explicit x limits.
fn last_point(xlim: Option<(f64, f64)>, annotate: bool) -> Figure {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    let mut ax = axes(&mut f);
    if let Some((low, high)) = xlim {
        ax.xlim(low, high).unwrap();
    }
    ax.line(
        nums(&[0.0, 5.0, 10.0]),
        &[1234.5, 1234.5, 1234.5],
        LineOptions::default(),
    )
    .unwrap()
    .y_error_bars(
        nums(&[10.0]),
        &[1000.0],
        &[1500.0],
        ErrorBarOptions::default(),
    )
    .unwrap()
    .scatter(
        nums(&[10.0]),
        &[1234.5],
        if annotate {
            options(Place::Auto)
        } else {
            ScatterOptions::default()
        },
    )
    .unwrap();
    f
}

#[test]
fn automatic_limits_grow_along_x_for_a_label_right_of_the_last_point() {
    let plain = render(&last_point(None, false));
    let old_room = plot(&plain)[2] - markers(&plain)[0].0.0;
    let scene = render(&last_point(None, true));
    assert_eq!(side_of(&scene, "1234.5", 0), Place::Right);
    let label = text_box(&scene, text(&scene, "1234.5"));
    assert!(
        inside(label, plot(&scene)),
        "{label:?} leaves {:?}",
        plot(&scene)
    );
    // Without the growth the label would leave the plot: it needs more room than the point
    // had, and the limits now leave it that room.
    let centre = markers(&scene)[0].0;
    assert!(label[2] - centre.0 > old_room, "{label:?} from {centre:?}");
    assert!(plot(&scene)[2] - centre.0 > old_room);
}

#[test]
fn only_automatic_limits_grow_when_the_other_axis_is_explicit() {
    // Explicit x limits: the label cannot leave across them, and no other side is clear.
    let err = render_err(&last_point(Some((0.0, 10.0)), true));
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert!(
        err.to_string()
            .contains("the value label \"1234.5\" of scatter 1 has no clear position"),
        "{err}"
    );
    // Explicit x limits and automatic y limits: above the highest point of a flat line, with an
    // error bar below it, is the only clear side; the label grows y, and x stays as it is.
    let peak = |annotate: bool| {
        let (xs, ys) = ([1.0, 2.0, 3.0], [4.9, 5.0, 4.9]);
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        axes(&mut f)
            .xlim(0.0, 4.0)
            .unwrap()
            .line(nums(&xs), &ys, LineOptions::default())
            .unwrap()
            .y_error_bars(nums(&[2.0]), &[0.0], &[5.0], ErrorBarOptions::default())
            .unwrap()
            .scatter(
                nums(&xs),
                &ys,
                if annotate {
                    options(Place::Auto)
                } else {
                    ScatterOptions::default()
                },
            )
            .unwrap();
        f
    };
    let (plain, scene) = (render(&peak(false)), render(&peak(true)));
    assert_eq!(side_of(&scene, "5", 1), Place::Above);
    let label = text_box(&scene, text(&scene, "5"));
    assert!(
        inside(label, plot(&scene)),
        "{label:?} leaves {:?}",
        plot(&scene)
    );
    // Where the points sit across the plot, and how far the peak sits below its top.
    let across = |scene: &Scene| -> Vec<f64> {
        let [left, _, right, _] = plot(scene);
        markers(scene)[..3]
            .iter()
            .map(|((x, _), _)| (x - left) / (right - left))
            .collect()
    };
    for (a, b) in across(&scene).into_iter().zip(across(&plain)) {
        assert!(close(a, b), "{a} != {b}");
    }
    let below_top = |scene: &Scene| markers(scene)[1].0.1 - plot(scene)[1];
    assert!(below_top(&scene) > below_top(&plain) + 1.0);
}

#[test]
fn growing_limits_is_deterministic() {
    let (a, b) = (matched_folds(None, true), matched_folds(None, true));
    assert_eq!(render(&a), render(&b));
    assert_eq!(
        a.to_svg(SvgText::Editable).unwrap(),
        b.to_svg(SvgText::Editable).unwrap()
    );
    assert_eq!(a.to_png(72).unwrap(), b.to_png(72).unwrap());
}

#[test]
fn png_svg_and_pdf_render_labels_that_grew_the_limits() {
    let f = matched_folds(None, true);
    let svg = f.to_svg(SvgText::Editable).unwrap();
    assert!(svg.contains(">81.7</text>"), "{svg}");
    let png = f.to_png(72).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    let pdf = f.to_pdf().unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    // All three come from the scene of the settled pass, which holds each label once.
    let scene = render(&f);
    assert_eq!(contents(&scene).iter().filter(|t| **t == "81.7").count(), 1);
}

// ---- Options and missing points ------------------------------------------------------------

#[test]
fn label_options_without_annotate_are_refused() {
    for (labels, parameter) in [
        (
            PointLabels {
                format: Some(TickFormat::fixed(1).unwrap()),
                ..PointLabels::default()
            },
            "format",
        ),
        (
            PointLabels {
                size: SizeSpec::pt(9.0).unwrap(),
                ..PointLabels::default()
            },
            "label_size",
        ),
        (
            PointLabels {
                place: Place::Below,
                ..PointLabels::default()
            },
            "place",
        ),
    ] {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        let err = axes(&mut f)
            .scatter(
                nums(&[1.0]),
                &[2.0],
                ScatterOptions {
                    labels: labels.clone(),
                    ..ScatterOptions::default()
                },
            )
            .unwrap_err();
        assert!(matches!(err, Error::Usage { .. }), "{err}");
        assert_eq!(err.parameter(), Some(parameter));
        assert_eq!(
            err.hint(),
            Some("pass annotate=True, or leave the label options out.")
        );
        let err = axes(&mut f)
            .grouped_scatter(
                vec![("North".into(), nums(&[1.0]), nums(&[2.0]))],
                Marker::Circle,
                SizeSpec::Auto,
                labels,
            )
            .unwrap_err();
        assert_eq!(err.parameter(), Some(parameter));
        assert!(axes(&mut f).marks().is_empty());
    }
    // A number format does not write times, nor ISO 8601 numbers.
    let mut f = figure(ScaleKind::Category, ScaleKind::Time);
    let fixed = ScatterOptions {
        labels: PointLabels {
            format: Some(TickFormat::fixed(1).unwrap()),
            ..labels(Place::Auto)
        },
        ..ScatterOptions::default()
    };
    let at = AxisData::Times(vec![Some(Instant::from_micros(0).unwrap())]);
    let err = axes(&mut f)
        .scatter_shared(names(&["A"]), at, fixed)
        .unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    let iso = ScatterOptions {
        labels: PointLabels {
            format: Some(TickFormat::iso()),
            ..labels(Place::Auto)
        },
        ..ScatterOptions::default()
    };
    let err = axes(&mut f).scatter(nums(&[1.0]), &[2.0], iso).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
}

#[test]
fn points_that_are_not_drawn_have_no_label() {
    // NaN coordinates.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .scatter(
            nums(&[1.0, f64::NAN, 3.0, 4.0]),
            &[1.3, 2.3, f64::NAN, 4.3],
            options(Place::Auto),
        )
        .unwrap();
    let scene = render(&f);
    text(&scene, "1.3");
    text(&scene, "4.3");
    assert!(!contents(&scene).contains(&"2.3"));
    // A color-mapped value that maps to no color.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    let by = ColorBy::new(vec![0.0, f64::NAN, 1.0]);
    axes(&mut f)
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .scatter(
            nums(&[1.0, 2.0, 3.0]),
            &[1.3, 2.3, 3.3],
            ScatterOptions {
                color: PointColor::By(by),
                ..options(Place::Auto)
            },
        )
        .unwrap();
    let scene = render(&f);
    assert!(!contents(&scene).contains(&"2.3"));
    text(&scene, "3.3");
    // A point outside explicit limits.
    let mut f = points(&[(5.0, 5.0)], Place::Auto);
    axes(&mut f)
        .scatter(nums(&[5.0]), &[12.5], options(Place::Above))
        .unwrap();
    assert!(!contents(&render(&f)).contains(&"12.5"));
}

// ---- Output --------------------------------------------------------------------------------

#[test]
fn annotate_false_changes_nothing() {
    let build = |labels: PointLabels| {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        axes(&mut f)
            .scatter(
                nums(&[1.0, 2.0]),
                &[3.0, 4.0],
                ScatterOptions {
                    labels,
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        f
    };
    let (default, off) = (build(PointLabels::default()), build(PointLabels::default()));
    assert_eq!(render(&default), render(&off));
    assert_eq!(default.to_png(72).unwrap(), off.to_png(72).unwrap());
    // Without labels, the scene holds no value label group.
    assert!(!contents(&render(&default)).contains(&"3"));
}

#[test]
fn png_svg_and_pdf_render_the_labels() {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Category);
    axes(&mut f)
        .x_error_bars_shared(
            names(&["Base", "Gate"]),
            Arc::from(&[69.0, 72.0][..]),
            Arc::from(&[73.5, 77.0][..]),
            ErrorBarOptions::default(),
        )
        .unwrap()
        .scatter_shared(
            nums(&[71.25, 74.5]),
            names(&["Base", "Gate"]),
            options(Place::Auto),
        )
        .unwrap();
    let svg = f.to_svg(SvgText::Editable).unwrap();
    assert!(
        svg.contains(">71.25</text>") && svg.contains(">74.5</text>"),
        "{svg}"
    );
    let png = f.to_png(72).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    let pdf = f.to_pdf().unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    // All three come from one scene, which holds each label once.
    let scene = render(&f);
    assert_eq!(
        contents(&scene).iter().filter(|t| **t == "71.25").count(),
        1
    );
}
