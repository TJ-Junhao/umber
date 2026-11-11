//! Figure captions: placement under everything else, alignment, line breaks, containment and
//! the same text in every output format.

#![allow(clippy::unwrap_used)]

use umber_core::render::{SvgText, render_pdf, render_png, render_svg};
use umber_core::scene::{Item, Scene, TextItem};
use umber_core::{
    Align, AnnotateOptions, AxisData, AxisValue, ColorBy, ColorbarOptions, ColorbarPosition, Error,
    Figure, FontLibrary, Grid, GridIndex, LegendMode, LegendPosition, LegendStyle, LineOptions,
    PieOptions, Placement, PointColor, ScaleKind, ScatterOptions, SizeSpec, Theme,
};

const PADDING: f64 = 12.0;
const CAPTION: &str = "Long: up to 1,000 epochs; patience 100.";

fn texts(scene: &Scene) -> Vec<&TextItem> {
    fn collect<'a>(items: &'a [Item], out: &mut Vec<&'a TextItem>) {
        for item in items {
            match item {
                Item::Text(text) => out.push(text),
                Item::Group { items, .. } => collect(items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    collect(&scene.items, &mut out);
    out
}

fn scene(figure: &Figure) -> Scene {
    figure.to_scene(FontLibrary::global()).unwrap()
}

fn line(figure: &mut Figure, label: &str) {
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xlabel("Epoch")
        .ylabel("Loss")
        .line(
            AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0]),
            &[1.0, 3.0, 2.0, 4.0],
            LineOptions {
                label: label.into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
}

/// The single text item written as `text`.
fn only<'a>(scene: &'a Scene, text: &str) -> &'a TextItem {
    let found: Vec<_> = texts(scene)
        .into_iter()
        .filter(|item| item.text == text)
        .collect();
    assert_eq!(found.len(), 1, "{text:?} is drawn {} times", found.len());
    found[0]
}

/// The room a one-line caption takes: the gap above it and its line box.
fn room(caption: &TextItem) -> f64 {
    8.0 + 1.35 * f64::from(caption.style.size)
}

/// Asserts that `scene` draws `CAPTION` below every other text and inside the padding.
fn assert_caption_below_everything(scene: &Scene, height: f64) {
    let caption = only(scene, CAPTION);
    let baseline = f64::from(caption.origin.y);
    assert!(baseline <= height - PADDING, "{baseline} past the padding");
    assert!(baseline > height - PADDING - 1.35 * f64::from(caption.style.size));
    for other in texts(scene) {
        if other.text != CAPTION {
            assert!(
                other.origin.y < caption.origin.y,
                "{:?} at {} is not above the caption at {baseline}",
                other.text,
                other.origin.y,
            );
        }
    }
}

#[test]
fn the_caption_sits_under_the_x_label_and_the_panels_make_room() {
    let mut plain = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    line(&mut plain, "");
    let mut captioned = plain.clone();
    captioned.caption(CAPTION);
    let (before, after) = (scene(&plain), scene(&captioned));
    assert_caption_below_everything(&after, 4.0 * 72.0);
    let caption = only(&after, CAPTION);
    let moved = f64::from(only(&before, "Epoch").origin.y - only(&after, "Epoch").origin.y);
    assert!((moved - room(caption)).abs() < 1e-3, "{moved}");
    // The x label stays above the caption by at least the gap.
    assert!(f64::from(caption.origin.y - only(&after, "Epoch").origin.y) > 8.0);
}

#[test]
fn the_caption_sits_under_a_bottom_legend() {
    let theme = Theme::pigment()
        .with_legend_style(LegendStyle::Header)
        .unwrap()
        .with_legend_position(LegendPosition::Bottom)
        .unwrap();
    let mut plain = Figure::new(6.0, 4.0, theme).unwrap();
    line(&mut plain, "North");
    let mut captioned = plain.clone();
    captioned.caption(CAPTION);
    let after = scene(&captioned);
    assert_caption_below_everything(&after, 4.0 * 72.0);
    let legend = only(&after, "North");
    let caption = only(&after, CAPTION);
    assert!(legend.origin.y > only(&after, "Epoch").origin.y);
    assert!(caption.origin.y - legend.origin.y > 8.0);
    assert!(legend.origin.y < only(&scene(&plain), "North").origin.y);
}

#[test]
fn the_caption_sits_under_a_bottom_colorbar() {
    let mut figure = Figure::new(6.4, 4.4, Theme::pigment()).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.xlabel("Distance")
        .scatter(
            AxisData::Numbers(vec![1.0, 2.0, 3.0, 4.0]),
            &[4.0, 2.0, 3.0, 1.0],
            ScatterOptions {
                color: PointColor::By(ColorBy::new(vec![-2.0, 0.0, 3.0, 8.0])),
                ..ScatterOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions {
            label: "Temperature".into(),
            position: Some(ColorbarPosition::Bottom),
            ..ColorbarOptions::default()
        })
        .unwrap();
    figure.caption(CAPTION);
    let after = scene(&figure);
    assert_caption_below_everything(&after, 4.4 * 72.0);
    let caption = only(&after, CAPTION);
    assert!(only(&after, "Temperature").origin.y < caption.origin.y);
}

#[test]
fn the_three_alignments_follow_the_title_edges() {
    for align in [Align::Left, Align::Center, Align::Right] {
        let theme = Theme::pigment().with_title_align(align).unwrap();
        let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
        figure.title("Training").caption("Footnote");
        line(&mut figure, "");
        let scene = scene(&figure);
        let (title, caption) = (only(&scene, "Training"), only(&scene, "Footnote"));
        let edges = |item: &TextItem| {
            let (left, width) = (f64::from(item.origin.x), f64::from(item.width));
            (left, left + width / 2.0, left + width)
        };
        let (caption, title) = (edges(caption), edges(title));
        match align {
            Align::Left => {
                assert!((caption.0 - PADDING).abs() < 1e-3 && (title.0 - PADDING).abs() < 1e-3);
            }
            Align::Center => {
                assert!((caption.1 - 216.0).abs() < 1e-3 && (title.1 - 216.0).abs() < 1e-3)
            }
            Align::Right => {
                let right = 432.0 - PADDING;
                assert!((caption.2 - right).abs() < 1e-3 && (title.2 - right).abs() < 1e-3);
            }
        }
    }
}

#[test]
fn line_breaks_stack_the_lines_and_grow_the_room() {
    let mut one = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
    line(&mut one, "");
    let mut two = one.clone();
    one.caption("First");
    two.caption("First\nSecond");
    let (one, two) = (scene(&one), scene(&two));
    let (first, second) = (only(&two, "First"), only(&two, "Second"));
    let size = f64::from(first.style.size);
    assert!((f64::from(second.origin.y - first.origin.y) - 1.2 * size).abs() < 1e-3);
    // The last line ends where the single line does.
    assert!((second.origin.y - only(&one, "First").origin.y).abs() < 1e-3);
    // One more line moves the panels up by one more pitch.
    let moved = f64::from(only(&one, "Epoch").origin.y - only(&two, "Epoch").origin.y);
    assert!((moved - 1.2 * size).abs() < 1e-3, "{moved}");
}

#[test]
fn a_line_too_wide_for_the_figure_is_refused_with_both_widths() {
    for (size, hint) in [
        (
            SizeSpec::Auto,
            "break the line with \"\\n\", make the figure wider or lower the font scale with \
             Theme.with_font_scale().",
        ),
        (
            SizeSpec::pt(10.0).unwrap(),
            "break the line with \"\\n\", make the figure wider or pass a smaller size=.",
        ),
    ] {
        let mut figure = Figure::new(2.0, 3.0, Theme::pigment()).unwrap();
        line(&mut figure, "");
        figure.caption_sized(CAPTION, size);
        let error = figure.to_scene(FontLibrary::global()).unwrap_err();
        assert!(matches!(error, Error::Style { .. }), "{error}");
        let message = error.to_string();
        // Quoted texts are cut to forty characters.
        let prefix = "Figure.to_scene(): the caption \"Long: up to 1,000 epochs; patience 100";
        assert!(message.starts_with(prefix), "{message}");
        assert!(
            message.ends_with(" pt wide; the figure has 120 pt."),
            "{message}"
        );
        assert_eq!(error.hint(), Some(hint));
        // Breaking the line makes it fit.
        figure.caption_sized("Long: up to 1,000\nepochs; patience 100.", size);
        figure.to_scene(FontLibrary::global()).unwrap();
    }
}

#[test]
fn other_control_characters_are_refused_as_in_a_subtitle() {
    let mut caption = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    caption.caption("ab\tc");
    let mut subtitle = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    subtitle.subtitle("ab\tc");
    let caption = caption.to_scene(FontLibrary::global()).unwrap_err();
    let subtitle = subtitle.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(caption, Error::Style { .. }));
    assert_eq!(
        caption.to_string(),
        subtitle.to_string().replace("subtitle", "caption")
    );
    assert_eq!(caption.hint(), subtitle.hint());
}

#[test]
fn an_empty_caption_removes_it_and_a_new_one_replaces_it() {
    let mut plain = Figure::new(6.0, 4.0, Theme::midnight()).unwrap();
    line(&mut plain, "");
    let mut figure = plain.clone();
    figure.caption("First").caption(CAPTION);
    let replaced = scene(&figure);
    assert!(texts(&replaced).iter().all(|text| text.text != "First"));
    only(&replaced, CAPTION);
    figure.caption("");
    assert_eq!(figure.caption_text(), "");
    assert_eq!(scene(&figure), scene(&plain));
}

#[test]
fn a_figure_without_a_caption_lays_out_as_before() {
    // The grid is laid out on the whole canvas when there is no caption: the same scene as
    // a figure whose caption was set and cleared, and the caption's room is the only change.
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    line(&mut figure, "North");
    let plain = scene(&figure);
    figure.caption("Note").caption("");
    assert_eq!(scene(&figure), plain);
}

#[test]
fn the_caption_is_muted_and_sized_like_tick_labels() {
    let theme = Theme::graphite();
    let mut figure = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    line(&mut figure, "");
    figure.caption("Note");
    let drawn = scene(&figure);
    let caption = only(&drawn, "Note");
    assert_eq!(caption.color, theme.muted_color());
    let tick = texts(&drawn)
        .into_iter()
        .find(|text| text.text == "0")
        .unwrap();
    assert_eq!(caption.style.size, tick.style.size);
    for (size, expected) in [
        (SizeSpec::times(2.0).unwrap(), 2.0 * tick.style.size),
        (SizeSpec::pt(9.0).unwrap(), 9.0),
    ] {
        figure.caption_sized("Note", size);
        let drawn = scene(&figure);
        assert_eq!(only(&drawn, "Note").style.size, expected);
    }
}

#[test]
fn a_grid_and_a_pie_figure_keep_the_caption_under_the_panels() {
    let mut grid = Figure::with_grid(9.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    for col in [0, 1] {
        grid.add_axes_at(
            Placement {
                col: GridIndex::At(col),
                ..Placement::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap()
        .xlabel("Epoch")
        .line(
            AxisData::Numbers(vec![0.0, 1.0, 2.0]),
            &[1.0, 3.0, 2.0],
            LineOptions::default(),
        )
        .unwrap();
    }
    grid.caption(CAPTION);
    assert_caption_below_everything(&scene(&grid), 4.0 * 72.0);

    let mut pie = Figure::new(5.0, 4.0, Theme::pigment()).unwrap();
    pie.add_pie_axes()
        .unwrap()
        .title("Share")
        .pie(
            vec!["A".into(), "B".into()],
            &[1.0, 3.0],
            PieOptions::default(),
        )
        .unwrap();
    pie.caption(CAPTION);
    assert_caption_below_everything(&scene(&pie), 4.0 * 72.0);
}

#[test]
fn twin_axes_keep_the_caption_under_both_y_axes() {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.xlabel("Epoch")
        .line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[3.0, 1.0],
            LineOptions::default(),
        )
        .unwrap();
    axes.twin_y(ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[1.0, 2.0],
            LineOptions::default(),
        )
        .unwrap();
    figure.caption(CAPTION);
    assert_caption_below_everything(&scene(&figure), 4.0 * 72.0);
}

#[test]
fn a_figure_without_axes_holds_the_caption_or_says_how_tall_it_must_be() {
    let mut figure = Figure::new(4.0, 1.5, Theme::pigment()).unwrap();
    figure.title("Only a header").caption("Note");
    assert_caption_below_everything_text(&scene(&figure), "Note", 108.0);
    let mut short = Figure::new(4.0, 0.5, Theme::pigment()).unwrap();
    short.title("Only a header").caption("Note");
    let error = short.to_scene(FontLibrary::global()).unwrap_err();
    assert!(matches!(error, Error::Style { .. }), "{error}");
    assert!(error.to_string().contains("pt tall"), "{error}");
}

fn assert_caption_below_everything_text(scene: &Scene, caption: &str, height: f64) {
    let note = only(scene, caption);
    assert!(f64::from(note.origin.y) <= height - PADDING);
    for other in texts(scene) {
        if other.text != caption {
            assert!(other.origin.y < note.origin.y);
        }
    }
}

#[test]
fn a_figure_too_short_for_the_caption_names_the_height_with_it() {
    let build = |height: f64| {
        let mut figure = Figure::new(6.0, height / 72.0, Theme::pigment()).unwrap();
        line(&mut figure, "");
        figure.caption("Note\nand more");
        figure
    };
    let error = build(60.0).to_scene(FontLibrary::global()).unwrap_err();
    let message = error.to_string();
    let shown: f64 = message
        .split("the figure is ")
        .nth(1)
        .and_then(|rest| rest.split(" pt tall").next())
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("{message}"));
    let needed: f64 = message
        .split("at least ")
        .nth(1)
        .and_then(|rest| rest.split(" pt").next())
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("{message}"));
    assert_eq!(shown, 60.0, "{message}");
    build(needed).to_scene(FontLibrary::global()).unwrap();
    assert!(
        build(needed - 2.0).to_scene(FontLibrary::global()).is_err(),
        "{message}"
    );
}

#[test]
fn png_svg_and_pdf_draw_the_same_caption() {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    line(&mut figure, "");
    figure.caption("Footnote under the chart");
    let drawn = scene(&figure);
    let editable = render_svg(&drawn, SvgText::Editable, 300).unwrap();
    assert!(editable.contains(">Footnote under the chart<"));
    let outlines = render_svg(&drawn, SvgText::Outlines, 300).unwrap();
    assert!(!outlines.contains("Footnote under the chart"));
    assert!(!outlines.contains("<text"));
    let png = render_png(&drawn, 72).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    let pdf = render_pdf(&drawn, 300).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    // The Figure methods render the same scene.
    assert_eq!(figure.to_svg(SvgText::Editable, 300).unwrap(), editable);
    assert_eq!(figure.to_png(72).unwrap(), png);
    assert_eq!(figure.to_pdf(300).unwrap(), pdf);
    // A figure without the caption renders differently in every format.
    figure.caption("");
    assert_ne!(figure.to_png(72).unwrap(), png);
    assert_ne!(figure.to_pdf(300).unwrap(), pdf);
}

/// The bounds of the glyphs of `text`: top and bottom of its ink.
fn ink(item: &TextItem) -> (f64, f64) {
    let (low, high) = item.outline.bounds().unwrap();
    (f64::from(low.y), f64::from(high.y))
}

#[test]
fn per_panel_bottom_legends_sit_between_their_x_label_and_the_caption() {
    let theme = Theme::pigment()
        .with_legend_style(LegendStyle::Header)
        .unwrap()
        .with_legend_position(LegendPosition::Bottom)
        .unwrap();
    let grid = Grid::new(1, 2).unwrap().with_legend(LegendMode::PerPanel);
    let mut figure = Figure::with_grid(9.0, 4.5, theme, grid).unwrap();
    for col in [0, 1] {
        let mut axes = figure
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        axes.xlabel("Epoch");
        for label in ["Short", "Long"] {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                &[1.0, 3.0, 2.0],
                LineOptions {
                    label: label.into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
    }
    figure.caption(CAPTION);
    let drawn = scene(&figure);
    let caption = only(&drawn, CAPTION);
    let at = |name: &str| {
        let mut found: Vec<_> = texts(&drawn)
            .into_iter()
            .filter(|item| item.text == name)
            .collect();
        assert_eq!(found.len(), 2, "{name}");
        found.sort_by(|a, b| a.origin.x.total_cmp(&b.origin.x));
        found
    };
    let (labels, legends) = (at("Epoch"), at("Short"));
    for (label, legend) in labels.iter().zip(&legends) {
        assert!(label.origin.y < legend.origin.y, "legend above its x label");
        assert!(
            legend.origin.y < caption.origin.y,
            "legend below the caption"
        );
        // Each legend belongs to its own panel: it starts in the panel's half.
        assert_eq!(legend.origin.x < 324.0, label.origin.x < 324.0);
    }
}

#[test]
fn annotations_and_text_cannot_reach_into_the_caption() {
    let build = |caption: bool, dy: f64, text: bool| {
        let theme = Theme::pigment().with_title_align(Align::Center).unwrap();
        let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
        let mut axes = figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        axes.line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions::default(),
        )
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap();
        let at = (AxisValue::Number(0.5), AxisValue::Number(0.0));
        if text {
            let options = umber_core::TextOptions {
                align: Align::Center,
                ..umber_core::TextOptions::default()
            };
            let lines = vec!["line"; dy as usize].join("\n");
            axes.text(&lines, at, options).unwrap();
        } else {
            let options = AnnotateOptions {
                offset: (0.0, -dy),
                arrow: false,
                ..AnnotateOptions::default()
            };
            axes.annotate("Annotation annotation", at, options).unwrap();
        }
        if caption {
            figure.caption("Caption caption caption");
        }
        figure.to_scene(FontLibrary::global())
    };
    let (mut fits, mut refused) = (0, 0);
    for dy in (5..70).step_by(5) {
        match build(true, f64::from(dy), false) {
            Ok(drawn) => {
                fits += 1;
                let note = only(&drawn, "Annotation annotation");
                let caption = only(&drawn, "Caption caption caption");
                assert!(ink(note).1 <= ink(caption).0, "dy {dy}: ink overlaps");
            }
            Err(error) => {
                refused += 1;
                let message = error.to_string();
                assert!(message.contains("into the caption."), "{message}");
                assert!(message.contains("the annotation \"Annotation annotation\""));
                assert_eq!(error.hint(), Some("change align or offset."));
                // Without a caption the same annotation is unchanged: drawn, or refused only
                // for the figure edge.
                if let Err(plain) = build(false, f64::from(dy), false) {
                    assert!(!plain.to_string().contains("caption"), "{plain}");
                }
            }
        }
    }
    assert!(fits > 0 && refused > 0, "{fits} fit, {refused} refused");
    // Text written with `text()` goes through the same check: a block of lines centered on the
    // plot's bottom reaches past the bottom edge without a caption and into it with one.
    let mut found = false;
    for lines in 1..40 {
        let plain = build(false, f64::from(lines), true);
        let captioned = build(true, f64::from(lines), true);
        if let (Ok(_), Err(error)) = (&plain, &captioned) {
            found = true;
            assert!(
                error.to_string().contains("pt into the caption."),
                "{error}"
            );
        }
        if let Err(error) = &plain {
            assert!(error.to_string().contains("edge of the figure."), "{error}");
        }
    }
    assert!(found, "no block of lines reaches only into the caption");
}
