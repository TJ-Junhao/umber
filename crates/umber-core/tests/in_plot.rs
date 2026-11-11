//! Text at positions in the plot, through the public API: where it sits for every alignment, on
//! every kind of axis, that it changes no limits, its containment errors, annotations whose text
//! sits at a plot position, and the same text in every output format.

#![allow(clippy::unwrap_used, clippy::panic)]

use umber_core::render::{SvgText, render_pdf, render_png, render_svg};
use umber_core::scene::{Item, Scene, TextItem};
use umber_core::{
    Align, AnnotateOptions, AnnotationPlacement, AxesMut, AxisData, AxisValue, Error, Figure,
    FontLibrary, GridIndex, InPlot, Instant, LineOptions, LogBase, ScaleKind, ScatterOptions,
    TextOptions, TextPosition, Theme,
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

fn at(x: f64, y: f64) -> InPlot {
    InPlot::new(x, y).unwrap()
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

/// The plot area of the first axes of `scene`, which must hold no lines or areas: the clip of its
/// marks, as left, top, right and bottom.
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

/// The box of `item`: left, top, right, bottom, from its advance width and font metrics.
fn text_box(scene: &Scene, item: &TextItem) -> [f64; 4] {
    let fonts = FontLibrary::global()
        .with_figure_fonts(&scene.fonts)
        .unwrap();
    let layout = fonts.layout(&item.text, &item.style).unwrap();
    let (x, y) = (f64::from(item.origin.x), f64::from(item.origin.y));
    [
        x,
        y - f64::from(layout.ascent()),
        x + f64::from(item.width),
        y + f64::from(layout.descent()),
    ]
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < TOLERANCE
}

/// The plot position of the fractions `(x, y)` in `plot`.
fn spot(plot: [f64; 4], (x, y): (f64, f64)) -> (f64, f64) {
    (
        plot[0] + x * (plot[2] - plot[0]),
        plot[3] - y * (plot[3] - plot[1]),
    )
}

/// A linear figure with three points and limits 0 to 10 on both axes. Points rather than a line:
/// the clip of a line's marks is wider than the plot area.
fn lined() -> Figure {
    let mut f = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut f)
        .xlim(0.0, 10.0)
        .unwrap()
        .ylim(0.0, 10.0)
        .unwrap()
        .scatter(
            AxisData::Numbers(vec![0.0, 5.0, 10.0]),
            &[2.0, 6.0, 4.0],
            ScatterOptions::default(),
        )
        .unwrap();
    f
}

#[test]
fn corners_and_centre_hold_the_aligned_edge_for_every_align() {
    let mut f = lined();
    let cases = [
        (0.0, 0.0),
        (1.0, 0.0),
        (0.0, 1.0),
        (1.0, 1.0),
        (0.5, 0.5),
        (0.25, 0.75),
    ];
    let aligns = [
        (Align::Left, 0.0),
        (Align::Center, 0.5),
        (Align::Right, 1.0),
    ];
    let mut names = Vec::new();
    for (i, (fx, fy)) in cases.into_iter().enumerate() {
        for (j, (align, _)) in aligns.into_iter().enumerate() {
            let name = char::from(b'a' + u8::try_from(i * 3 + j).unwrap()).to_string();
            let options = TextOptions {
                align,
                ..TextOptions::default()
            };
            axes(&mut f).text(&name, at(fx, fy), options).unwrap();
            names.push((name, (fx, fy), aligns[j].1));
        }
    }
    let scene = render(&f);
    let area = plot(&scene);
    for (name, fractions, share) in names {
        let item = text(&scene, &name);
        let [left, top, right, bottom] = text_box(&scene, item);
        let (x, y) = spot(area, fractions);
        assert!(close(left + share * (right - left), x), "{name}");
        assert!(close((top + bottom) / 2.0, y), "{name}");
    }
}

#[test]
fn text_in_the_plot_takes_size_and_color_and_keeps_line_breaks() {
    let mut f = lined();
    axes(&mut f)
        .text(
            "first\nsecond",
            at(0.5, 0.5),
            TextOptions {
                align: Align::Center,
                size: umber_core::SizeSpec::pt(15.0).unwrap(),
                color: umber_core::ColorSpec::Fixed(umber_core::Rgba::RED),
            },
        )
        .unwrap();
    let scene = render(&f);
    let (first, second) = (text(&scene, "first"), text(&scene, "second"));
    assert_eq!(
        (first.style.size, first.color),
        (15.0, umber_core::Rgba::RED)
    );
    let (x, y) = spot(plot(&scene), (0.5, 0.5));
    let [left, top, right, bottom] = text_box(&scene, first);
    assert!(close((left + right) / 2.0, x) && close((top + bottom) / 2.0, y));
    // The second line is centred on the same point, 1.2 sizes below the first.
    let [left, _, right, _] = text_box(&scene, second);
    assert!(close((left + right) / 2.0, x));
    assert!(close(
        f64::from(second.origin.y - first.origin.y),
        1.2 * 15.0
    ));
}

/// The centre of the text box of `content` in the one axes of `f`, and the centre of its plot.
fn centres(f: &Figure, content: &str) -> ((f64, f64), (f64, f64)) {
    let scene = render(f);
    let [left, top, right, bottom] = text_box(&scene, text(&scene, content));
    let area = plot(&scene);
    (
        ((left + right) / 2.0, (top + bottom) / 2.0),
        spot(area, (0.5, 0.5)),
    )
}

fn centred() -> TextOptions {
    TextOptions {
        align: Align::Center,
        ..TextOptions::default()
    }
}

#[test]
fn text_in_the_plot_ignores_log_category_and_time_scales() {
    let day = |d: i64| Instant::from_micros(d * 86_400_000_000).unwrap();
    let log = ScaleKind::Log { base: LogBase::TEN };
    let mut figures = Vec::new();

    let mut f = figure(log, log);
    axes(&mut f)
        .scatter(
            AxisData::Numbers(vec![1.0, 100.0, 10_000.0]),
            &[1.0, 50.0, 1000.0],
            ScatterOptions::default(),
        )
        .unwrap();
    figures.push(f);

    let mut f = figure(ScaleKind::Category, ScaleKind::Linear);
    axes(&mut f)
        .scatter(
            AxisData::Categories(["Jan", "Feb", "Mar"].map(String::from).to_vec()),
            &[24.0, 31.0, 42.0],
            ScatterOptions::default(),
        )
        .unwrap();
    figures.push(f);

    let mut f = figure(ScaleKind::Time, ScaleKind::Linear);
    axes(&mut f)
        .scatter(
            AxisData::Times(vec![Some(day(0)), Some(day(3)), Some(day(9))]),
            &[1.0, 2.0, 3.0],
            ScatterOptions::default(),
        )
        .unwrap();
    figures.push(f);

    for f in &mut figures {
        axes(f).text("middle", at(0.5, 0.5), centred()).unwrap();
        let (found, wanted) = centres(f, "middle");
        assert!(
            close(found.0, wanted.0) && close(found.1, wanted.1),
            "{found:?} vs {wanted:?}"
        );
    }
}

#[test]
fn text_in_the_plot_on_twin_axes_uses_the_shared_plot_area() {
    let mut f = lined();
    axes(&mut f)
        .twin_y(ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Numbers(vec![0.0, 10.0]),
            &[100.0, 900.0],
            LineOptions::default(),
        )
        .unwrap();
    axes(&mut f).text("left", at(0.5, 0.5), centred()).unwrap();
    f.twin_axes_at_mut(GridIndex::Auto, GridIndex::Auto)
        .unwrap()
        .text("right", at(0.5, 0.5), centred())
        .unwrap();
    let scene = render(&f);
    let area = plot(&scene);
    let (x, y) = spot(area, (0.5, 0.5));
    for content in ["left", "right"] {
        let [left, top, right, bottom] = text_box(&scene, text(&scene, content));
        assert!(close((left + right) / 2.0, x), "{content}");
        assert!(close((top + bottom) / 2.0, y), "{content}");
    }
}

/// The items of `scene` that are not drawn text of `contents`, with the groups holding them
/// removed.
fn without_notes(scene: &Scene, contents: &[&str]) -> Vec<Item> {
    scene
        .items
        .iter()
        .filter(|item| {
            !leaves(std::slice::from_ref(item))
                .iter()
                .any(|leaf| matches!(leaf, Item::Text(t) if contents.contains(&t.text.as_str())))
        })
        .cloned()
        .collect()
}

#[test]
fn text_in_the_plot_changes_no_limits() {
    let plain = lined();
    let mut f = lined();
    // No data point could hold these: they would be far outside the limits.
    axes(&mut f)
        .text(
            "corner",
            at(1.0, 1.0),
            TextOptions {
                align: Align::Right,
                ..TextOptions::default()
            },
        )
        .unwrap()
        .text("elsewhere", at(0.0, 0.0), TextOptions::default())
        .unwrap();
    let (before, after) = (render(&plain), render(&f));
    assert_eq!(
        without_notes(&after, &["corner", "elsewhere"]),
        before.items
    );
    // The limits are automatic here: they stay the data's.
    let mut auto = figure(ScaleKind::Linear, ScaleKind::Linear);
    axes(&mut auto)
        .line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions::default(),
        )
        .unwrap();
    let before = render(&auto);
    axes(&mut auto)
        .text("note", at(0.9, 0.9), TextOptions::default())
        .unwrap();
    assert_eq!(without_notes(&render(&auto), &["note"]), before.items);
}

#[test]
fn text_in_the_plot_reaching_past_the_figure_edge_is_refused() {
    let mut f = lined();
    axes(&mut f)
        .text(
            "a long note at the corner",
            at(1.0, 1.0),
            TextOptions::default(),
        )
        .unwrap();
    let err = f.to_png(72).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    let message = err.to_string();
    assert!(
        message.starts_with("Figure.to_png(): the text \"a long note at the corner\" reaches "),
        "{message}"
    );
    assert!(
        message.ends_with("pt past the right edge of the figure."),
        "{message}"
    );
    assert_eq!(err.hint(), Some("change align or at."));
}

#[test]
fn text_in_the_plot_reaching_into_the_caption_is_refused() {
    let build = |caption: bool, lines: usize| {
        let mut f = lined();
        axes(&mut f)
            .text(&vec!["line"; lines].join("\n"), at(0.5, 0.0), centred())
            .unwrap();
        if caption {
            f.caption("Caption caption caption");
        }
        f.to_scene(FontLibrary::global())
    };
    let mut found = false;
    for lines in 1..40 {
        let (plain, captioned) = (build(false, lines), build(true, lines));
        if let (Ok(_), Err(error)) = (&plain, &captioned) {
            found = true;
            assert!(matches!(error, Error::Style { .. }), "{error}");
            assert!(
                error.to_string().contains("pt into the caption."),
                "{error}"
            );
            assert_eq!(error.hint(), Some("change align or at."));
        }
        if let Err(error) = &plain {
            assert!(error.to_string().contains("edge of the figure."), "{error}");
        }
    }
    assert!(found, "no block of lines reaches only into the caption");
}

#[test]
fn text_in_the_plot_has_no_limits_to_lie_outside() {
    let mut f = lined();
    axes(&mut f)
        .text("anywhere", at(0.7, 0.3), TextOptions::default())
        .unwrap();
    assert!(f.to_scene(FontLibrary::global()).is_ok());
    assert_eq!(
        axes(&mut f).annotations()[0].at(),
        &TextPosition::InPlot(at(0.7, 0.3))
    );
}

fn pinned(text_at: Option<InPlot>, arrow: bool) -> AnnotateOptions {
    AnnotateOptions {
        text_at,
        arrow,
        ..AnnotateOptions::default()
    }
}

/// The number of strokes in `scene`: one per arrow shaft here.
fn strokes(scene: &Scene) -> usize {
    leaves(&scene.items)
        .into_iter()
        .filter(|item| matches!(item, Item::Stroke { .. }))
        .count()
}

#[test]
fn an_annotation_can_sit_at_a_plot_position_with_an_arrow_to_its_point() {
    let point = (number(2.0), number(2.0));
    let build = |arrow: bool| {
        let mut f = lined();
        axes(&mut f)
            .annotate("note", point.clone(), pinned(Some(at(0.8, 0.85)), arrow))
            .unwrap();
        render(&f)
    };
    let (with, without) = (build(true), build(false));
    assert_eq!(strokes(&with), strokes(&without) + 1);
    for scene in [&with, &without] {
        let [left, top, right, bottom] = text_box(scene, text(scene, "note"));
        let (x, y) = spot(plot(scene), (0.8, 0.85));
        assert!(close((left + right) / 2.0, x), "{left} {right} {x}");
        assert!(close((top + bottom) / 2.0, y));
    }
    // The arrow is routed as for an offset: the same pair of items, ending short of the point.
    let offset = {
        let mut f = lined();
        axes(&mut f)
            .annotate(
                "note",
                point.clone(),
                AnnotateOptions {
                    offset: (40.0, 40.0),
                    ..AnnotateOptions::default()
                },
            )
            .unwrap();
        render(&f)
    };
    assert_eq!(strokes(&offset), strokes(&with));
}

#[test]
fn an_annotation_at_a_plot_position_keeps_its_data_point_rules() {
    let mut f = lined();
    axes(&mut f)
        .annotate(
            "far",
            (number(11.0), number(5.0)),
            pinned(Some(at(0.8, 0.85)), true),
        )
        .unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the annotation \"far\" lies outside the x limits."
    );

    // The point is still checked when it is made: the wrong kind of value.
    let mut f = lined();
    let err = axes(&mut f)
        .annotate(
            "kind",
            (AxisValue::Category("a".into()), number(1.0)),
            pinned(Some(at(0.5, 0.5)), true),
        )
        .map(drop)
        .unwrap_err();
    assert!(matches!(err, Error::Data { .. }), "{err}");
}

#[test]
fn an_arrow_from_a_plot_position_needs_room_before_the_point() {
    let mut f = lined();
    // The plot's centre is the data point (5, 5).
    axes(&mut f)
        .annotate(
            "close",
            (number(5.0), number(5.0)),
            pinned(Some(at(0.5, 0.5)), true),
        )
        .unwrap();
    let err = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert!(
        err.to_string()
            .contains("the arrow of the annotation \"close\" has ")
    );
    assert_eq!(
        err.hint(),
        Some("move text_at away from the point, or pass arrow=False.")
    );
    // Without an arrow the same text is drawn.
    let mut f = lined();
    axes(&mut f)
        .annotate(
            "close",
            (number(5.0), number(5.0)),
            pinned(Some(at(0.5, 0.5)), false),
        )
        .unwrap();
    assert!(f.to_scene(FontLibrary::global()).is_ok());
}

#[test]
fn an_annotation_at_a_plot_position_refuses_an_offset() {
    let mut f = lined();
    let both = AnnotateOptions {
        offset: (24.0, 18.0),
        text_at: Some(at(0.5, 0.5)),
        ..AnnotateOptions::default()
    };
    let err = axes(&mut f)
        .annotate("note", (number(1.0), number(1.0)), both)
        .map(drop)
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }), "{err}");
    assert_eq!(err.parameter(), Some("text_at"));
    assert_eq!(
        err.to_string(),
        "Axes.annotate(): text_at and offset (24, 18) both place the text, in different ways."
    );
    assert!(err.hint().is_some());
    assert!(axes(&mut f).annotations().is_empty());
    // An offset of the default value is not a second placement.
    let same = AnnotateOptions {
        offset: (0.0, 12.0),
        ..both
    };
    axes(&mut f)
        .annotate("note", (number(1.0), number(1.0)), same)
        .unwrap();
    assert!(matches!(
        axes(&mut f).annotations()[0].placement(),
        AnnotationPlacement::InPlot { arrow: true, .. }
    ));
}

#[test]
fn positions_in_the_plot_are_fractions_from_zero_to_one() {
    for (x, y) in [(0.0, 0.0), (1.0, 1.0), (-0.0, 0.5), (0.02, 0.96)] {
        let position = InPlot::new(x, y).unwrap();
        assert_eq!((position.x(), position.y()), (x + 0.0, y + 0.0));
    }
    for (x, y, parameter, shown) in [
        (-0.1, 0.5, "x", "-0.1"),
        (1.5, 0.5, "x", "1.5"),
        (f64::NAN, 0.5, "x", "NaN"),
        (0.5, f64::INFINITY, "y", "inf"),
        (0.5, 1.000_001, "y", "1.000001"),
        // The first failure is x.
        (2.0, 2.0, "x", "2"),
    ] {
        let err = InPlot::new(x, y).unwrap_err();
        assert!(matches!(err, Error::Data { .. }), "{err}");
        assert_eq!(err.parameter(), Some(parameter));
        assert_eq!(
            err.to_string(),
            format!(
                "InPlot(): {parameter} is {shown}; a position in the plot is a fraction from 0 to 1."
            )
        );
        assert!(err.hint().is_some());
    }
}

#[test]
fn png_svg_and_pdf_draw_the_same_text_in_the_plot() {
    let mut f = lined();
    axes(&mut f)
        .text("n = 5 folds", at(0.02, 0.96), TextOptions::default())
        .unwrap()
        .annotate(
            "best run",
            (number(5.0), number(6.0)),
            pinned(Some(at(0.8, 0.2)), true),
        )
        .unwrap();
    let drawn = render(&f);
    let editable = render_svg(&drawn, SvgText::Editable, 300).unwrap();
    assert!(editable.contains(">n = 5 folds<") && editable.contains(">best run<"));
    let outlines = render_svg(&drawn, SvgText::Outlines, 300).unwrap();
    assert!(!outlines.contains("n = 5 folds"));
    let png = render_png(&drawn, 72).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    let pdf = render_pdf(&drawn, 300).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(f.to_svg(SvgText::Editable, 300).unwrap(), editable);
    assert_eq!(f.to_png(72).unwrap(), png);
    assert_eq!(f.to_pdf(300).unwrap(), pdf);
    // The text is in the picture: without it every format differs.
    let plain = lined();
    assert_ne!(plain.to_svg(SvgText::Editable, 300).unwrap(), editable);
    assert_ne!(plain.to_png(72).unwrap(), png);
    assert_ne!(plain.to_pdf(300).unwrap(), pdf);
}

#[test]
fn a_plot_position_matches_the_data_point_at_the_same_place() {
    // The scene of text at a data point has the same text and position as text placed at the
    // fractions of the same point: the new position type adds a way to place text, not a way
    // the old one is drawn.
    let mut by_data = lined();
    axes(&mut by_data)
        .text("note", (number(2.5), number(7.5)), TextOptions::default())
        .unwrap();
    let mut by_fraction = lined();
    axes(&mut by_fraction)
        .text("note", at(0.25, 0.75), TextOptions::default())
        .unwrap();
    assert_eq!(render(&by_data), render(&by_fraction));
}

fn aligned(align: Option<Align>, text_at: InPlot, arrow: bool) -> AnnotateOptions {
    AnnotateOptions {
        align,
        ..pinned(Some(text_at), arrow)
    }
}

#[test]
fn a_pinned_annotation_aligns_its_text_like_text_does() {
    let spot = at(0.8, 0.85);
    for align in [Align::Left, Align::Center, Align::Right] {
        for content in ["note", "two\nlines of text"] {
            let mut annotated = lined();
            axes(&mut annotated)
                .annotate(
                    content,
                    (number(2.0), number(2.0)),
                    aligned(Some(align), spot, true),
                )
                .unwrap();
            let mut written = lined();
            axes(&mut written)
                .text(
                    content,
                    spot,
                    TextOptions {
                        align,
                        ..TextOptions::default()
                    },
                )
                .unwrap();
            let (annotated, written) = (render(&annotated), render(&written));
            for line in content.split('\n') {
                let (a, w) = (text(&annotated, line), text(&written, line));
                assert!(
                    close(f64::from(a.origin.x), f64::from(w.origin.x))
                        && close(f64::from(a.origin.y), f64::from(w.origin.y)),
                    "{align:?} {line:?}: {:?} vs {:?}",
                    a.origin,
                    w.origin
                );
            }
        }
    }
}

#[test]
fn a_right_aligned_pinned_annotation_fits_near_the_right_edge() {
    let mut f = lined();
    axes(&mut f)
        .annotate(
            "a long note near the edge",
            (number(2.0), number(2.0)),
            aligned(Some(Align::Right), at(1.0, 0.9), true),
        )
        .unwrap();
    let scene = render(&f);
    let [_, _, right, _] = text_box(&scene, text(&scene, "a long note near the edge"));
    assert!(close(right, plot(&scene)[2]));
    // Centred or left-aligned at the same position, the text reaches past the figure.
    for align in [None, Some(Align::Left)] {
        let mut f = lined();
        axes(&mut f)
            .annotate(
                "a long note near the edge",
                (number(2.0), number(2.0)),
                aligned(align, at(1.0, 0.9), true),
            )
            .unwrap();
        assert!(f.to_scene(FontLibrary::global()).is_err());
    }
}

#[test]
fn an_automatic_align_is_the_centre_and_offset_annotations_are_unchanged() {
    let build = |align: Option<Align>| {
        let mut f = lined();
        axes(&mut f)
            .annotate(
                "note",
                (number(2.0), number(2.0)),
                aligned(align, at(0.8, 0.85), true),
            )
            .unwrap();
        render(&f)
    };
    assert_eq!(build(None), build(Some(Align::Center)));
    // Without text_at the options of an offset annotation are as before.
    let mut f = lined();
    axes(&mut f)
        .annotate("n", (number(5.0), number(5.0)), AnnotateOptions::default())
        .unwrap();
    assert!(matches!(
        axes(&mut f).annotations()[0].placement(),
        AnnotationPlacement::Offset { .. }
    ));
}

#[test]
fn an_align_without_text_at_is_refused() {
    let mut f = lined();
    let options = AnnotateOptions {
        align: Some(Align::Right),
        ..AnnotateOptions::default()
    };
    let err = axes(&mut f)
        .annotate("note", (number(1.0), number(1.0)), options)
        .map(drop)
        .unwrap_err();
    assert!(matches!(err, Error::Usage { .. }), "{err}");
    assert_eq!(err.parameter(), Some("align"));
    assert_eq!(
        err.hint(),
        Some("the direction of offset decides the alignment; pass text_at to use align.")
    );
    assert!(axes(&mut f).annotations().is_empty());
}
