//! Text and annotations at data positions, and the values written on bars, through the public
//! API: where the text sits, the arrow, the limits, and the errors.

#![allow(clippy::unwrap_used, clippy::panic)]

use umber_core::scene::{Item, Scene, TextItem};
use umber_core::{
    Align, AnnotateOptions, AxesMut, AxisData, AxisValue, BarOptions, ColorSpec, Error, Figure,
    FontLibrary, Grid, GridIndex, HeatmapOptions, Instant, LogBase, Paint, PieOptions, Placement,
    Point, Rgba, ScaleKind, ScatterOptions, Series, SizeSpec, TextOptions, Theme, TickFormat,
    ValueLabels,
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

fn number(v: f64) -> AxisValue {
    AxisValue::Number(v)
}

fn render(figure: &Figure) -> Scene {
    figure.to_scene(FontLibrary::global()).unwrap()
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

fn text<'a>(scene: &'a Scene, content: &str) -> &'a TextItem {
    leaves(&scene.items)
        .into_iter()
        .find_map(|item| match item {
            Item::Text(t) if t.text == content => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text {content:?}"))
}

/// The marker centres of the scatter marks of `scene`, in drawing order.
fn markers(scene: &Scene) -> Vec<Point> {
    leaves(&scene.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Markers { positions, .. } => Some(positions.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// The ascent and descent of the font `item` was set in.
fn metrics(scene: &Scene, item: &TextItem) -> (f64, f64) {
    let fonts = FontLibrary::global()
        .with_figure_fonts(&scene.fonts)
        .unwrap();
    let layout = fonts.layout(&item.text, &item.style).unwrap();
    (f64::from(layout.ascent()), f64::from(layout.descent()))
}

/// The box of `item`: left, top, right, bottom, from its advance width and font metrics.
fn text_box(scene: &Scene, item: &TextItem) -> [f64; 4] {
    let (ascent, descent) = metrics(scene, item);
    let (x, y) = (f64::from(item.origin.x), f64::from(item.origin.y));
    [x, y - ascent, x + f64::from(item.width), y + descent]
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < TOLERANCE
}

/// A figure with one scatter point at each of `points`, limits 0 to 10 on both axes.
fn points(points: &[(f64, f64)]) -> Figure {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    let (xs, ys): (Vec<f64>, Vec<f64>) = points.iter().copied().unzip();
    axes(&mut f)
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .scatter(AxisData::Numbers(xs), &ys, ScatterOptions::default())
        .unwrap();
    f
}

#[test]
fn text_align_edges() {
    let mut f = points(&[(2.0, 7.5), (5.0, 5.0), (8.0, 2.5)]);
    for (content, at, align) in [
        ("left edge", (2.0, 7.5), Align::Left),
        ("centre", (5.0, 5.0), Align::Center),
        ("right edge", (8.0, 2.5), Align::Right),
    ] {
        let options = TextOptions {
            align,
            ..TextOptions::default()
        };
        axes(&mut f)
            .text(content, (number(at.0), number(at.1)), options)
            .unwrap();
    }
    let scene = render(&f);
    let centres = markers(&scene);
    for (content, centre, share) in [
        ("left edge", centres[0], 0.0),
        ("centre", centres[1], 0.5),
        ("right edge", centres[2], 1.0),
    ] {
        let item = text(&scene, content);
        let [left, top, right, bottom] = text_box(&scene, item);
        let at = left + share * (right - left);
        assert!(
            close(at, f64::from(centre.x)),
            "{content}: {at} vs {centre:?}"
        );
        assert!(
            close((top + bottom) / 2.0, f64::from(centre.y)),
            "{content}"
        );
        assert_eq!(item.color, Theme::pigment().text_color());
        assert_eq!(item.style.size, Theme::pigment().body_size());
    }
}

#[test]
fn text_takes_size_and_color() {
    let mut f = points(&[(5.0, 5.0)]);
    let options = TextOptions {
        size: SizeSpec::pt(15.0).unwrap(),
        color: ColorSpec::Fixed(Rgba::RED),
        ..TextOptions::default()
    };
    axes(&mut f)
        .text("big", (number(5.0), number(5.0)), options)
        .unwrap();
    let scene = render(&f);
    let item = text(&scene, "big");
    assert_eq!((item.style.size, item.color), (15.0, Rgba::RED));
}

#[test]
fn text_on_category_and_time_axes() {
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    let months: Vec<String> = ["Jan", "Feb", "Mar"].map(String::from).to_vec();
    axes(&mut f)
        .ylim(0.0, 60.0)
        .unwrap()
        .scatter(
            AxisData::Categories(months),
            &[24.0, 31.0, 42.0],
            ScatterOptions::default(),
        )
        .unwrap()
        .text(
            "March",
            (AxisValue::Category("Mar".into()), number(42.0)),
            TextOptions::default(),
        )
        .unwrap();
    let scene = render(&f);
    let item = text(&scene, "March");
    let march = markers(&scene)[2];
    assert!(close(f64::from(item.origin.x), f64::from(march.x)));

    let day = |d: i64| Instant::from_micros(d * 86_400_000_000).unwrap();
    let mut f = figure(ScaleKind::Time, ScaleKind::Linear);
    axes(&mut f)
        .scatter(
            AxisData::Times(vec![Some(day(0)), Some(day(3)), Some(day(9))]),
            &[1.0, 2.0, 3.0],
            ScatterOptions::default(),
        )
        .unwrap()
        .text(
            "launch",
            (AxisValue::Time(day(3)), number(2.0)),
            TextOptions::default(),
        )
        .unwrap();
    let scene = render(&f);
    let item = text(&scene, "launch");
    let launch = markers(&scene)[1];
    assert!(close(f64::from(item.origin.x), f64::from(launch.x)));
    let [_, top, _, bottom] = text_box(&scene, item);
    assert!(close((top + bottom) / 2.0, f64::from(launch.y)));
}

#[test]
fn annotate_side_follows_offset() {
    // The share of the box width and height, from its left and top, at the offset point.
    let sides = [
        ((0.0, 22.0), (0.5, 1.0)),
        ((18.0, 18.0), (0.0, 1.0)),
        ((22.0, 0.0), (0.0, 0.5)),
        ((18.0, -18.0), (0.0, 0.0)),
        ((0.0, -22.0), (0.5, 0.0)),
        ((-18.0, -18.0), (1.0, 0.0)),
        ((-22.0, 0.0), (1.0, 0.5)),
        ((-18.0, 18.0), (1.0, 1.0)),
    ];
    for (offset, (sx, sy)) in sides {
        for arrow in [false, true] {
            let mut f = points(&[(5.0, 5.0)]);
            let options = AnnotateOptions {
                offset,
                arrow,
                ..AnnotateOptions::default()
            };
            axes(&mut f)
                .annotate("note", (number(5.0), number(5.0)), options)
                .unwrap();
            let scene = render(&f);
            let point = markers(&scene)[0];
            let [left, top, right, bottom] = text_box(&scene, text(&scene, "note"));
            let anchor = (left + sx * (right - left), top + sy * (bottom - top));
            let expected = (f64::from(point.x) + offset.0, f64::from(point.y) - offset.1);
            assert!(
                close(anchor.0, expected.0) && close(anchor.1, expected.1),
                "{offset:?}: {anchor:?} vs {expected:?}"
            );
        }
    }
}

/// The line items of the text of several lines whose first line is `first`, top down: the
/// text items that follow it in drawing order, `count` in all.
fn lines<'a>(scene: &'a Scene, first: &str, count: usize) -> Vec<&'a TextItem> {
    let texts: Vec<&TextItem> = leaves(&scene.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Text(t) => Some(t),
            _ => None,
        })
        .collect();
    let start = texts
        .iter()
        .position(|t| t.text == first)
        .unwrap_or_else(|| panic!("no text {first:?}"));
    texts[start..start + count].to_vec()
}

/// The box of the lines `lines`: the widest line's extent, from the first line's top to the
/// last line's descent.
fn lines_box(scene: &Scene, lines: &[&TextItem]) -> [f64; 4] {
    let [_, top, _, _] = text_box(scene, lines[0]);
    let [_, _, _, bottom] = text_box(scene, lines[lines.len() - 1]);
    let left = lines
        .iter()
        .map(|t| f64::from(t.origin.x))
        .fold(f64::INFINITY, f64::min);
    let right = lines
        .iter()
        .map(|t| f64::from(t.origin.x + t.width))
        .fold(f64::NEG_INFINITY, f64::max);
    [left, top, right, bottom]
}

#[test]
fn text_lines_start_where_one_line_does() {
    for (align, share) in [
        (Align::Left, 0.0),
        (Align::Center, 0.5),
        (Align::Right, 1.0),
    ] {
        let options = TextOptions {
            align,
            ..TextOptions::default()
        };
        let mut one = points(&[(5.0, 5.0)]);
        axes(&mut one)
            .text("Peak", (number(5.0), number(5.0)), options)
            .unwrap();
        let mut many = points(&[(5.0, 5.0)]);
        axes(&mut many)
            .text("Peak\nof the season\n", (number(5.0), number(5.0)), options)
            .unwrap();
        let (one, many) = (render(&one), render(&many));
        let single = text(&one, "Peak");
        let lines = lines(&many, "Peak", 3);
        assert_eq!(
            lines.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(),
            ["Peak", "of the season", ""]
        );
        assert_eq!(lines[0].origin, single.origin, "{align:?}");
        let pitch = 1.2 * f64::from(single.style.size);
        let centre = f64::from(markers(&many)[0].x);
        for (i, line) in lines.iter().enumerate() {
            let dy = f64::from(line.origin.y - lines[0].origin.y);
            assert!(close(dy, i as f64 * pitch), "{align:?} line {i}: {dy}");
            let at = f64::from(line.origin.x) + share * f64::from(line.width);
            assert!(close(at, centre), "{align:?} line {i}: {at} vs {centre}");
        }
    }
}

#[test]
fn annotation_lines_face_the_point_and_the_arrow_leaves_their_box() {
    let sides = [
        ((0.0, 40.0), 0.5),
        ((36.0, 36.0), 0.0),
        ((40.0, 0.0), 0.0),
        ((36.0, -36.0), 0.0),
        ((0.0, -40.0), 0.5),
        ((-36.0, -36.0), 1.0),
        ((-40.0, 0.0), 1.0),
        ((-36.0, 36.0), 1.0),
    ];
    for (offset, share) in sides {
        let options = AnnotateOptions {
            offset,
            ..AnnotateOptions::default()
        };
        let mut one = points(&[(5.0, 5.0)]);
        axes(&mut one)
            .annotate("Peak", (number(5.0), number(5.0)), options)
            .unwrap();
        let mut many = points(&[(5.0, 5.0)]);
        axes(&mut many)
            .annotate(
                "Peak\nof the wet season",
                (number(5.0), number(5.0)),
                options,
            )
            .unwrap();
        let (one, many) = (render(&one), render(&many));
        let lines = lines(&many, "Peak", 2);
        assert_eq!(lines[1].text, "of the wet season");
        // The first line is where the text of one line is; the second is one pitch below it,
        // aligned on the side of the box that faces the point.
        assert_eq!(lines[0].origin, text(&one, "Peak").origin, "{offset:?}");
        let pitch = 1.2 * f64::from(lines[0].style.size);
        assert!(close(
            f64::from(lines[1].origin.y - lines[0].origin.y),
            pitch
        ));
        let edge = |t: &TextItem| f64::from(t.origin.x) + share * f64::from(t.width);
        assert!(close(edge(lines[0]), edge(lines[1])), "{offset:?}");
        // The shaft starts 2 pt beyond the edge of the box of both lines, on the line from its
        // centre.
        let [left, top, right, bottom] = lines_box(&many, &lines);
        let start = arrow(&many).0[0];
        let (cx, cy) = ((left + right) / 2.0, (top + bottom) / 2.0);
        let (dx, dy) = (f64::from(start.x) - cx, f64::from(start.y) - cy);
        let to_edge = ((right - left) / 2.0 / dx.abs()).min((bottom - top) / 2.0 / dy.abs());
        let beyond = dx.hypot(dy) * (1.0 - to_edge);
        assert!(close(beyond, 2.0), "{offset:?}: {beyond}");
        let point = markers(&many)[0];
        assert!(close(distance(arrow(&many).2[1], point), 3.0), "{offset:?}");
    }
}

#[test]
fn text_of_several_lines_must_stay_on_the_figure() {
    // The first line fits; the further lines reach past the bottom edge.
    let mut f = points(&[(5.0, 0.2)]);
    axes(&mut f)
        .ylim(0.0, 10.0)
        .unwrap()
        .text(
            "low\n1\n2\n3\n4\n5",
            (number(5.0), number(0.2)),
            TextOptions::default(),
        )
        .unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert!(
        err.to_string()
            .contains("the text \"low\\n1\\n2\\n3\\n4\\n5\" reaches"),
        "{err}"
    );
    assert!(err.to_string().contains("past the bottom edge"), "{err}");
}

/// The items of the annotations of the one axes of `scene`: its last group.
fn annotation_items(scene: &Scene) -> Vec<&Item> {
    leaves(std::slice::from_ref(scene.items.last().unwrap()))
}

/// The arrow of the one annotation in `scene`: its shaft and its head.
fn arrow(scene: &Scene) -> (Vec<Point>, f32, Vec<Point>, Rgba) {
    let leaves = annotation_items(scene);
    let shaft = leaves.iter().find_map(|item| match item {
        Item::Stroke { path, style } if style.width == 0.75 => Some((path.clone(), style.width)),
        _ => None,
    });
    let head = leaves.iter().find_map(|item| match item {
        Item::Fill {
            path,
            paint: Paint::Solid(color),
            ..
        } if *color == Theme::pigment().text_color() => Some((path.clone(), *color)),
        _ => None,
    });
    let ((shaft, width), (head, color)) = (shaft.unwrap(), head.unwrap());
    let ends = |path: &umber_core::Path| -> Vec<Point> {
        path.verbs()
            .iter()
            .filter_map(|verb| match *verb {
                umber_core::PathVerb::MoveTo(p) | umber_core::PathVerb::LineTo(p) => Some(p),
                _ => None,
            })
            .collect()
    };
    (ends(&shaft), width, ends(&head), color)
}

fn distance(a: Point, b: Point) -> f64 {
    f64::from(a.x - b.x).hypot(f64::from(a.y - b.y))
}

#[test]
fn arrow_stops_short_of_point() {
    for offset in [(24.0, 18.0), (0.0, 30.0), (-25.0, -25.0), (30.0, 0.0)] {
        let mut f = points(&[(5.0, 5.0)]);
        let options = AnnotateOptions {
            offset,
            ..AnnotateOptions::default()
        };
        axes(&mut f)
            .annotate("Peak", (number(5.0), number(5.0)), options)
            .unwrap();
        let scene = render(&f);
        let point = markers(&scene)[0];
        let (shaft, width, head, color) = arrow(&scene);
        assert_eq!((width, color), (0.75, Theme::pigment().text_color()));
        assert_eq!((shaft.len(), head.len()), (2, 3));
        let tip = head[1];
        assert!(close(distance(tip, point), 3.0), "{offset:?}");
        // The head is 5 pt long and 3.5 pt wide, its base where the shaft ends.
        let base = Point {
            x: (head[0].x + head[2].x) / 2.0,
            y: (head[0].y + head[2].y) / 2.0,
        };
        assert!(close(distance(base, tip), 5.0));
        assert!(close(distance(head[0], head[2]), 3.5));
        assert!(close(distance(shaft[1], base), 0.0));
        // The shaft starts 2 pt beyond the edge of the text box, on the line from its centre.
        let [left, top, right, bottom] = text_box(&scene, text(&scene, "Peak"));
        let start = shaft[0];
        let (cx, cy) = ((left + right) / 2.0, (top + bottom) / 2.0);
        let (dx, dy) = (f64::from(start.x) - cx, f64::from(start.y) - cy);
        let to_edge = ((right - left) / 2.0 / dx.abs()).min((bottom - top) / 2.0 / dy.abs());
        let beyond = dx.hypot(dy) * (1.0 - to_edge);
        assert!(close(beyond, 2.0), "{offset:?}: {beyond}");
    }
}

#[test]
fn annotate_without_arrow_draws_only_text() {
    let mut f = points(&[(5.0, 5.0)]);
    let options = AnnotateOptions {
        arrow: false,
        ..AnnotateOptions::default()
    };
    axes(&mut f)
        .annotate("Dip", (number(5.0), number(5.0)), options)
        .unwrap();
    let scene = render(&f);
    let items = annotation_items(&scene);
    assert!(matches!(items.as_slice(), [Item::Text(t)] if t.text == "Dip"));
}

#[test]
fn annotation_does_not_change_limits() {
    let plain = || {
        let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
        axes(&mut f)
            .scatter(
                AxisData::Numbers(vec![1.0, 2.0, 3.0]),
                &[4.0, 9.0, 2.0],
                ScatterOptions::default(),
            )
            .unwrap();
        f
    };
    let mut annotated = plain();
    axes(&mut annotated)
        .text("high", (number(2.0), number(9.0)), TextOptions::default())
        .unwrap()
        .annotate(
            "low",
            (number(3.0), number(2.0)),
            AnnotateOptions {
                offset: (-30.0, 20.0),
                ..AnnotateOptions::default()
            },
        )
        .unwrap();
    let (before, after) = (render(&plain()), render(&annotated));
    // The chrome and the marks are the same; the annotations come after them.
    assert_eq!(before.items[..2], after.items[..2]);
    assert_eq!(after.items.len(), before.items.len() + 1);
}

#[test]
fn point_outside_limits_rejected() {
    for (at, axis) in [((11.0, 5.0), "x"), ((5.0, -1.0), "y")] {
        let mut f = points(&[(5.0, 5.0)]);
        axes(&mut f)
            .text("far", (number(at.0), number(at.1)), TextOptions::default())
            .unwrap();
        let err = f.to_scene(FontLibrary::global()).unwrap_err();
        assert!(matches!(err, Error::Style { .. }), "{err}");
        assert_eq!(
            err.to_string(),
            format!("Figure.to_scene(): the text \"far\" lies outside the {axis} limits.")
        );
        assert_eq!(
            err.hint(),
            Some("set xlim/ylim to include it, or move the point.")
        );
    }
}

#[test]
fn text_outside_the_figure_rejected() {
    let mut f = points(&[(5.0, 5.0)]);
    axes(&mut f)
        .annotate(
            "pushed far to the right",
            (number(9.5), number(5.0)),
            AnnotateOptions {
                offset: (60.0, 0.0),
                ..AnnotateOptions::default()
            },
        )
        .unwrap();
    let err = f.to_png(72).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert!(
        err.to_string()
            .starts_with("Figure.to_png(): the annotation \"pushed far to the right\" reaches "),
        "{err}"
    );
    assert!(
        err.to_string()
            .ends_with("pt past the right edge of the figure.")
    );
    assert_eq!(err.hint(), Some("change align or offset."));
}

#[test]
fn unknown_category_is_refused_at_render() {
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    axes(&mut f)
        .bar(vec!["A".into()], &[3.0], BarOptions::default())
        .unwrap()
        .text(
            "nowhere",
            (AxisValue::Category("Z".into()), number(1.0)),
            TextOptions::default(),
        )
        .unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(err, Error::Data { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the text \"nowhere\" is at the x category \"Z\"; no mark on these \
         axes has that category."
    );
    assert_eq!(err.hint(), Some("draw a mark with that category first."));
}

#[test]
fn annotation_arguments_are_checked_atomically() {
    let mut f = points(&[(5.0, 5.0)]);
    type Call = Box<dyn Fn(&mut AxesMut<'_>) -> Result<(), Error>>;
    let cases: Vec<(Call, &str)> = vec![
        (
            Box::new(|a| {
                a.text("", (number(1.0), number(1.0)), TextOptions::default())
                    .map(drop)
            }),
            "text",
        ),
        (
            Box::new(|a| {
                a.text("x", (number(f64::NAN), number(1.0)), TextOptions::default())
                    .map(drop)
            }),
            "at",
        ),
        (
            Box::new(|a| {
                a.text(
                    "x",
                    (AxisValue::Category("A".into()), number(1.0)),
                    TextOptions::default(),
                )
                .map(drop)
            }),
            "at",
        ),
        (
            Box::new(|a| {
                a.annotate(
                    "x",
                    (number(1.0), number(1.0)),
                    AnnotateOptions {
                        offset: (0.0, 0.0),
                        ..AnnotateOptions::default()
                    },
                )
                .map(drop)
            }),
            "offset",
        ),
        (
            Box::new(|a| {
                a.annotate(
                    "x",
                    (number(1.0), number(1.0)),
                    AnnotateOptions {
                        offset: (f64::INFINITY, 1.0),
                        ..AnnotateOptions::default()
                    },
                )
                .map(drop)
            }),
            "offset",
        ),
    ];
    for (call, parameter) in cases {
        let err = call(&mut axes(&mut f)).unwrap_err();
        assert_eq!(err.parameter(), Some(parameter), "{err}");
        assert!(err.hint().is_some(), "{err}");
    }
    let axes = f.axes_at(GridIndex::Auto, GridIndex::Auto).unwrap();
    assert!(axes.annotations().is_empty());
}

#[test]
fn arrow_needs_room_between_text_and_point() {
    let mut f = points(&[(5.0, 5.0)]);
    axes(&mut f)
        .annotate(
            "close",
            (number(5.0), number(5.0)),
            AnnotateOptions {
                offset: (0.0, 6.0),
                ..AnnotateOptions::default()
            },
        )
        .unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the arrow of the annotation \"close\" has 6 pt between the text and \
         the point; an arrow needs at least 10 pt."
    );
    assert_eq!(err.hint(), Some("pass a larger offset, or arrow=False."));
}

// ---- Bar value labels ----------------------------------------------------------------------

fn annotated() -> ValueLabels {
    ValueLabels {
        annotate: true,
        ..ValueLabels::default()
    }
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

/// The bar fills of `scene` with their bounds: left, top, right, bottom.
fn bars(scene: &Scene) -> Vec<([f64; 4], Rgba)> {
    leaves(&scene.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Fill {
                path,
                paint: Paint::Solid(color),
                ..
            } => {
                let (lo, hi) = path.bounds()?;
                Some((
                    [
                        f64::from(lo.x),
                        f64::from(lo.y),
                        f64::from(hi.x),
                        f64::from(hi.y),
                    ],
                    *color,
                ))
            }
            _ => None,
        })
        .collect()
}

/// The plot area of the one axes of `scene`: the clip of its marks.
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

fn inside(inner: [f64; 4], outer: [f64; 4]) -> bool {
    inner[0] >= outer[0] - 1e-6
        && inner[1] >= outer[1] - 1e-6
        && inner[2] <= outer[2] + 1e-6
        && inner[3] <= outer[3] + 1e-6
}

#[test]
fn bar_labels_vertical_horizontal_grouped_stacked() {
    let theme = Theme::pigment();
    // Vertical: above a positive bar, below a negative one, 3 pt from the end.
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    let options = BarOptions {
        annotation: annotated(),
        ..BarOptions::default()
    };
    axes(&mut f)
        .bar(names(&["A", "B", "C"]), &[28.4, 42.0, -16.5], options)
        .unwrap();
    let scene = render(&f);
    let fills = bars(&scene);
    for (label, (bar, _)) in ["28.4", "42", "\u{2212}16.5"].iter().zip(&fills) {
        let item = text(&scene, label);
        let [left, top, right, bottom] = text_box(&scene, item);
        assert!(
            close((left + right) / 2.0, (bar[0] + bar[2]) / 2.0),
            "{label}"
        );
        if label.starts_with('\u{2212}') {
            assert!(close(top, bar[3] + 3.0), "{label}");
        } else {
            assert!(close(bottom, bar[1] - 3.0), "{label}");
        }
        assert_eq!(item.color, theme.text_color());
        assert_eq!(item.style.size, theme.body_size());
        assert!(item.style.tabular_figures);
        assert!(inside([left, top, right, bottom], plot(&scene)), "{label}");
    }

    // Horizontal: right of each bar, centred on it.
    let mut f = figure(ScaleKind::Linear, ScaleKind::Category);
    let options = BarOptions {
        annotation: annotated(),
        ..BarOptions::default()
    };
    axes(&mut f)
        .bar(names(&["A", "B"]), &[28.4, 52.8], options)
        .unwrap();
    let scene = render(&f);
    for (label, (bar, _)) in ["28.4", "52.8"].iter().zip(bars(&scene)) {
        let [left, top, right, bottom] = text_box(&scene, text(&scene, label));
        assert!(close(left, bar[2] + 3.0), "{label}");
        assert!(
            close((top + bottom) / 2.0, (bar[1] + bar[3]) / 2.0),
            "{label}"
        );
        assert!(inside([left, top, right, bottom], plot(&scene)), "{label}");
    }

    // Grouped: each bar of the group carries its own label.
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    let series = |label: &str, values: &[f64]| Series {
        label: label.into(),
        values: values.to_vec(),
        color: ColorSpec::AUTO,
    };
    axes(&mut f)
        .grouped_bar(
            names(&["Q1", "Q2"]),
            vec![
                series("Online", &[24.5, 31.2]),
                series("Retail", &[18.2, 26.3]),
            ],
            annotated(),
        )
        .unwrap();
    let scene = render(&f);
    let fills = bars(&scene);
    for (label, (bar, _)) in ["24.5", "18.2", "31.2", "26.3"].iter().zip(&fills) {
        let [left, _, right, bottom] = text_box(&scene, text(&scene, label));
        assert!(
            close((left + right) / 2.0, (bar[0] + bar[2]) / 2.0),
            "{label}"
        );
        assert!(close(bottom, bar[1] - 3.0), "{label}");
    }

    // Stacked: centred in each segment, in black or white by contrast.
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    axes(&mut f)
        .stacked_bar(
            names(&["Q1", "Q2"]),
            vec![
                series("Core", &[32.0, 38.0]),
                series("Other", &[18.0, 22.0]),
            ],
            annotated(),
        )
        .unwrap();
    let scene = render(&f);
    let fills = bars(&scene);
    for (label, (bar, color)) in ["32", "18", "38", "22"].iter().zip(&fills) {
        let item = text(&scene, label);
        let [left, top, right, bottom] = text_box(&scene, item);
        assert!(
            close((left + right) / 2.0, (bar[0] + bar[2]) / 2.0),
            "{label}"
        );
        assert!(
            close((top + bottom) / 2.0, (bar[1] + bar[3]) / 2.0),
            "{label}"
        );
        let expected = if contrast(*color, 0.0) >= contrast(*color, 1.0) {
            Rgba::opaque(0, 0, 0)
        } else {
            Rgba::opaque(255, 255, 255)
        };
        assert_eq!(item.color, expected, "{label} on {color:?}");
    }
}

/// The WCAG 2 contrast ratio of `fill` against a gray of relative luminance `other`.
fn contrast(fill: Rgba, other: f64) -> f64 {
    let linear = |channel: u8| {
        let c = f64::from(channel) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * linear(fill.r) + 0.7152 * linear(fill.g) + 0.0722 * linear(fill.b);
    let (hi, lo) = if luminance > other {
        (luminance, other)
    } else {
        (other, luminance)
    };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn bar_labels_take_format_and_size() {
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    let options = BarOptions {
        annotation: ValueLabels {
            annotate: true,
            format: Some(TickFormat::fixed(2).unwrap()),
            size: SizeSpec::pt(11.0).unwrap(),
        },
        ..BarOptions::default()
    };
    axes(&mut f).bar(names(&["A"]), &[3.0], options).unwrap();
    let scene = render(&f);
    assert_eq!(text(&scene, "3.00").style.size, 11.0);
    // AUTO follows the value axis tick format.
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    let options = BarOptions {
        annotation: annotated(),
        ..BarOptions::default()
    };
    axes(&mut f)
        .ytick_format(TickFormat::percent(0).unwrap())
        .unwrap()
        .bar(names(&["A"]), &[0.25], options)
        .unwrap();
    text(&render(&f), "25%");
}

#[test]
fn automatic_limits_grow_to_hold_bar_labels() {
    for height in [2.0, 2.55, 3.2] {
        let mut f = Figure::new(4.7, height, Theme::pigment()).unwrap();
        let options = BarOptions {
            annotation: annotated(),
            ..BarOptions::default()
        };
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .bar(
                names(&["A", "B", "C", "D", "E", "F"]),
                &[50.0, 60.0, 80.0, 70.0, 65.0, 97.0],
                options,
            )
            .unwrap();
        let scene = render(&f);
        let label = text(&scene, "97");
        assert!(
            inside(text_box(&scene, label), plot(&scene)),
            "height {height}"
        );
    }
}

#[test]
fn bar_label_beyond_explicit_limits_names_the_bar() {
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    let options = BarOptions {
        annotation: annotated(),
        ..BarOptions::default()
    };
    axes(&mut f)
        .ylim(0.0, 100.0)
        .unwrap()
        .bar(names(&["A", "B"]), &[50.0, 99.0], options)
        .unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the value label \"99\" of the bar \"B\" does not fit inside the \
         plot."
    );
    assert_eq!(
        err.hint(),
        Some("widen ylim (or xlim), or let the limits be automatic.")
    );
}

#[test]
fn stacked_label_that_does_not_fit_names_the_segment() {
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    let series = |label: &str, values: &[f64]| Series {
        label: label.into(),
        values: values.to_vec(),
        color: ColorSpec::AUTO,
    };
    axes(&mut f)
        .stacked_bar(
            names(&["Q1", "Q2"]),
            vec![
                series("Core", &[32.0, 38.0]),
                series("Service", &[21.0, 0.8]),
            ],
            annotated(),
        )
        .unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert!(
        err.to_string().starts_with(
            "Figure.to_scene(): the value label \"0.8\" of the series \"Service\" in the bar \
             \"Q2\" needs "
        ),
        "{err}"
    );
    assert_eq!(
        err.hint(),
        Some("enlarge the figure, write fewer decimals with format, or pass annotate=False.")
    );
}

#[test]
fn bar_label_options_are_checked() {
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    for (labels, parameter) in [
        (
            ValueLabels {
                format: Some(TickFormat::fixed(1).unwrap()),
                ..ValueLabels::default()
            },
            "format",
        ),
        (
            ValueLabels {
                size: SizeSpec::pt(9.0).unwrap(),
                ..ValueLabels::default()
            },
            "size",
        ),
        (
            ValueLabels {
                annotate: true,
                format: Some(TickFormat::iso()),
                ..ValueLabels::default()
            },
            "format",
        ),
    ] {
        let options = BarOptions {
            annotation: labels.clone(),
            ..BarOptions::default()
        };
        let err = axes(&mut f)
            .bar(names(&["A"]), &[1.0], options)
            .unwrap_err();
        assert_eq!(err.parameter(), Some(parameter), "{err}");
        let err = axes(&mut f)
            .stacked_bar(
                names(&["A"]),
                vec![Series {
                    values: vec![1.0],
                    ..Series::default()
                }],
                labels,
            )
            .unwrap_err();
        assert_eq!(err.parameter(), Some(parameter), "{err}");
    }
    assert!(
        f.axes_at(GridIndex::Auto, GridIndex::Auto)
            .unwrap()
            .marks()
            .is_empty()
    );
}

#[test]
fn automatic_labels_write_each_value_on_its_own() {
    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    let options = BarOptions {
        annotation: annotated(),
        ..BarOptions::default()
    };
    axes(&mut f)
        .bar(names(&["A", "B", "C"]), &[0.12345, 0.9, 1234567.0], options)
        .unwrap();
    let scene = render(&f);
    for label in ["0.12345", "0.9", "1.23457e6"] {
        text(&scene, label);
    }
}

#[test]
fn labels_that_fit_keep_the_automatic_range() {
    let log = ScaleKind::Log { base: LogBase::TEN };
    let tops = |annotate: bool| {
        let mut f = Figure::new(6.0, 8.0, Theme::pigment()).unwrap();
        let options = BarOptions {
            annotation: ValueLabels {
                annotate,
                ..ValueLabels::default()
            },
            ..BarOptions::default()
        };
        f.add_axes(ScaleKind::Category, log)
            .unwrap()
            .bar(names(&["A", "B"]), &[1.0, 10.0], options)
            .unwrap();
        bars(&render(&f))
            .into_iter()
            .map(|(bounds, _)| bounds)
            .collect::<Vec<_>>()
    };
    assert_eq!(tops(true), tops(false));
}

#[test]
fn annotations_are_painted_after_every_panel() {
    let mut f = Figure::with_grid(8.0, 3.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    f.title("Figure title");
    let at = |col| Placement {
        row: GridIndex::At(0),
        col: GridIndex::At(col),
        row_span: 1,
        col_span: 1,
    };
    f.add_axes_at(at(0), ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .annotate(
            "OVERLAY",
            (number(1.0), number(0.5)),
            AnnotateOptions {
                offset: (90.0, 0.0),
                arrow: false,
                size: SizeSpec::pt(24.0).unwrap(),
                ..AnnotateOptions::default()
            },
        )
        .unwrap();
    f.add_axes_at(at(1), ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &["X".to_owned()],
            &["Y".to_owned()],
            &[1.0],
            HeatmapOptions::default(),
        )
        .unwrap();
    let scene = render(&f);
    let last = annotation_items(&scene);
    assert!(
        matches!(last.as_slice(), [Item::Text(t)] if t.text == "OVERLAY"),
        "{last:?}"
    );
}

#[test]
fn annotations_and_bar_labels_render_with_a_pie_panel() {
    for pie_col in [0, 1] {
        let mut f =
            Figure::with_grid(10.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
        f.title("Mixed panels");
        let at = |col| Placement {
            row: GridIndex::At(0),
            col: GridIndex::At(col),
            row_span: 1,
            col_span: 1,
        };
        let mut axes = f
            .add_axes_at(at(1 - pie_col), ScaleKind::Category, ScaleKind::Linear)
            .unwrap();
        axes.bar(
            names(&["A", "B"]),
            &[1.25, -2.5],
            BarOptions {
                annotation: annotated(),
                ..BarOptions::default()
            },
        )
        .unwrap();
        axes.annotate(
            "OVERLAY",
            (AxisValue::Category("A".into()), number(0.0)),
            AnnotateOptions {
                arrow: false,
                ..AnnotateOptions::default()
            },
        )
        .unwrap();
        f.add_pie_axes_at(at(pie_col))
            .unwrap()
            .title("Pie panel")
            .pie(
                names(&["First", "Second"]),
                &[1.0, 3.0],
                PieOptions::default(),
            )
            .unwrap();

        let scene = render(&f);
        for label in ["Mixed panels", "Pie panel", "1.25", "\u{2212}2.5"] {
            text(&scene, label);
        }
        for label in ["1.25", "\u{2212}2.5"] {
            assert!(inside(text_box(&scene, text(&scene, label)), plot(&scene)));
        }
        assert!(leaves(&scene.items).iter().any(|item| matches!(
            item,
            Item::Fill { path, .. }
                if path.verbs().iter().any(|verb| matches!(verb, umber_core::PathVerb::CubicTo(..)))
        )));
        let last = annotation_items(&scene);
        assert!(
            matches!(last.as_slice(), [Item::Text(t)] if t.text == "OVERLAY"),
            "{last:?}"
        );
    }
}

#[test]
fn tiny_stacked_segment_reports_the_room_error() {
    for small in [0.01, 0.1] {
        let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
        let series = |label: &str, values: &[f64]| Series {
            label: label.into(),
            values: values.to_vec(),
            color: ColorSpec::AUTO,
        };
        axes(&mut f)
            .stacked_bar(
                names(&["A"]),
                vec![series("Large", &[100.0]), series("Small", &[small])],
                ValueLabels {
                    annotate: true,
                    format: Some(TickFormat::fixed(2).unwrap()),
                    ..ValueLabels::default()
                },
            )
            .unwrap();
        let err = f.to_scene(FontLibrary::global()).unwrap_err();
        assert!(matches!(err, Error::Style { .. }), "{err}");
        assert!(
            err.to_string().starts_with(&format!(
                "Figure.to_scene(): the value label \"{small:.2}\" of the series \"Small\" in \
                 the bar \"A\" needs "
            )),
            "{err}"
        );
    }
}

#[test]
fn values_lost_to_rounding_in_a_stack_report_the_room_error() {
    for (x, y) in [
        (ScaleKind::Category, ScaleKind::Linear),
        (ScaleKind::Linear, ScaleKind::Category),
    ] {
        for sign in [1.0, -1.0] {
            let mut f = figure(x, y);
            let series = |label: &str, value: f64| Series {
                label: label.into(),
                values: vec![value],
                color: ColorSpec::AUTO,
            };
            axes(&mut f)
                .stacked_bar(
                    names(&["A"]),
                    vec![
                        series("Large", sign * 100.0),
                        series("Small", sign * 1e-15),
                        series("Zero", 0.0),
                    ],
                    annotated(),
                )
                .unwrap();
            let err = f.to_scene(FontLibrary::global()).unwrap_err();
            assert!(matches!(err, Error::Style { .. }), "{err}");
            let minus = if sign < 0.0 { "\u{2212}" } else { "" };
            assert!(
                err.to_string().starts_with(&format!(
                    "Figure.to_scene(): the value label \"{minus}1e\u{2212}15\" of the series \
                     \"Small\" in the bar \"A\" needs "
                )),
                "{x:?} {sign}: {err}"
            );
        }
    }
}

#[test]
fn zero_and_nan_in_a_stack_have_no_label() {
    let zeros = |labels: ValueLabels| {
        let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
        let series = |label: &str, value: f64| Series {
            label: label.into(),
            values: vec![value],
            color: ColorSpec::AUTO,
        };
        axes(&mut f)
            .stacked_bar(
                names(&["A"]),
                vec![
                    series("Large", 100.0),
                    series("Zero", 0.0),
                    series("Gap", f64::NAN),
                ],
                labels,
            )
            .unwrap();
        let scene = render(&f);
        leaves(&scene.items)
            .iter()
            .filter(|item| matches!(item, Item::Text(t) if t.text == "0" || t.text == "NaN"))
            .count()
    };
    // The only "0" is the tick label, with or without value labels.
    assert_eq!(zeros(annotated()), zeros(ValueLabels::default()));
}
