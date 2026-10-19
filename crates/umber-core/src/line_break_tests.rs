//! Titles and subtitles of several lines, and the characters text may hold.

use super::*;
use crate::layout::{LINE_PITCH, PADDING, TextBlock};
use crate::scene::{Item, TextItem};
use crate::{Align, AxisData, Grid, GridIndex, LineOptions, PieOptions, Placement, ScaleKind};

/// The library of these tests: the bundled fonts and no system fonts.
fn lib() -> FontLibrary {
    FontLibrary::with_fallbacks(Vec::new())
}

/// Lays out every panel and the figure header of `f`.
fn plan_of(f: &Figure) -> Result<FigurePlan> {
    let lib = lib();
    let groups = ShareGroups::new(f);
    let fonts = layout::figure_fonts(&lib, f.theme())?.in_context("Figure.to_scene()");
    let panels = panels(f, &groups, &fonts)?;
    plan(
        f,
        &panels,
        &groups,
        &fonts,
        "Figure.to_scene()",
        &layout::LabelCache::new(&fonts),
    )
}

/// A 6 by 4 in figure of one line chart in pigment with `align` for titles, its title and
/// subtitle `title` and `subtitle`.
fn single(align: Align, title: &str, subtitle: &str) -> Figure {
    let theme = Theme::pigment().with_title_align(align).unwrap();
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title(title)
        .subtitle(subtitle)
        .line(
            AxisData::Numbers(vec![0.0, 1.0, 2.0]),
            &[1.0, 3.0, 2.0],
            LineOptions::default(),
        )
        .unwrap();
    f
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// Checks that the lines of `block` are `LINE_PITCH` sizes apart and each aligned by `align`
/// within `left..right`, and that they spell `text`.
#[track_caller]
fn assert_lines(block: &TextBlock, text: &str, align: Align, (left, right): (f64, f64)) {
    let lines: Vec<_> = block.lines().collect();
    assert_eq!(
        lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
        text.split('\n').collect::<Vec<_>>()
    );
    let pitch = LINE_PITCH * f64::from(lines[0].style.size);
    assert!(close(block.pitch(), pitch));
    for (i, line) in lines.iter().enumerate() {
        assert!(
            close(line.y - lines[0].y, i as f64 * pitch),
            "{text:?} line {i}"
        );
        assert_eq!(
            line.x,
            align.position(line.width(), left, right),
            "{text:?} line {i}"
        );
    }
    let widest = lines.iter().map(|l| l.width()).fold(0.0, f64::max);
    assert_eq!(block.width(), widest);
}

#[test]
fn title_and_subtitle_lines_start_where_one_line_does() {
    const TITLE: &str = "Station output\nDaily mean across the network\nMW";
    const SUBTITLE: &str = "Measured hourly\n2026";
    for align in [Align::Left, Align::Center, Align::Right] {
        let one = plan_of(&single(align, "Station output", "Measured hourly")).unwrap();
        let many = plan_of(&single(align, TITLE, SUBTITLE)).unwrap();
        let (one, many) = (&one.panels[0].1, &many.panels[0].1);
        let room = (PADDING, 6.0 * 72.0 - PADDING);
        let (title, subtitle) = (
            many.title.as_ref().unwrap(),
            many.subtitle.as_ref().unwrap(),
        );
        assert_lines(title, TITLE, align, room);
        assert_lines(subtitle, SUBTITLE, align, room);
        // The first line is where the text of one line is; the subtitle moves down by the two
        // further title lines.
        let (one_title, one_subtitle) =
            (one.title.as_ref().unwrap(), one.subtitle.as_ref().unwrap());
        assert_eq!(
            (title.first().x, title.first().y),
            (one_title.first().x, one_title.first().y)
        );
        assert_eq!(subtitle.first().x, one_subtitle.first().x);
        assert!(close(
            subtitle.first().y - one_subtitle.first().y,
            2.0 * title.pitch()
        ));
        // The header grows by every further line, and the plot moves down with it.
        let grown = 2.0 * title.pitch() + subtitle.pitch();
        assert!(close(many.header_bottom - one.header_bottom, grown));
        assert!(close(many.plot.y - one.plot.y, grown));
        assert!(close(many.plot.bottom(), one.plot.bottom()));
    }
}

#[test]
fn blank_lines_are_kept_and_an_empty_text_removes_the_title() {
    let f = single(Align::Left, "\nMiddle\n\n", "");
    let plan = plan_of(&f).unwrap();
    let layout = &plan.panels[0].1;
    let title = layout.title.as_ref().unwrap();
    let texts: Vec<_> = title.lines().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["", "Middle", "", ""]);
    assert!(close(title.below_first(), 3.0 * title.pitch()));
    let none = plan_of(&single(Align::Left, "", "")).unwrap();
    assert!(none.panels[0].1.title.is_none());
    // Every line, blank ones too, is a text item of the scene.
    let scene = f.to_scene(&lib()).unwrap();
    let mut texts = Vec::new();
    collect_texts(&scene.items, &mut texts);
    assert_eq!(texts.iter().filter(|t| t.text.is_empty()).count(), 3);
    assert!(texts.iter().all(|t| !t.text.contains('\n')));
}

fn collect_texts<'a>(items: &'a [Item], out: &mut Vec<&'a TextItem>) {
    for item in items {
        match item {
            Item::Group { items, .. } => collect_texts(items, out),
            Item::Text(text) => out.push(text),
            _ => {}
        }
    }
}

/// A 9 by 4 in figure of two panels with the figure title `title`.
fn grid(title: &str) -> Figure {
    let mut f = Figure::with_grid(9.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    f.title(title).subtitle("Weekly totals");
    for col in 0..2 {
        f.add_axes_at(
            Placement {
                col: GridIndex::At(col),
                ..Placement::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap()
        .title("Panel")
        .line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[1.0, 2.0],
            LineOptions::default(),
        )
        .unwrap();
    }
    f
}

#[test]
fn a_figure_title_of_two_lines_pushes_the_grid_down() {
    let one = plan_of(&grid("Regional growth")).unwrap();
    let two = plan_of(&grid("Regional growth\nacross both regions")).unwrap();
    let title = two.header.title.as_ref().unwrap();
    assert_lines(
        title,
        "Regional growth\nacross both regions",
        Align::Left,
        (PADDING, 9.0 * 72.0 - PADDING),
    );
    let first = one.header.title.as_ref().unwrap().first();
    assert_eq!((title.first().x, title.first().y), (first.x, first.y));
    let pitch = title.pitch();
    assert!(close(two.header.bottom - one.header.bottom, pitch));
    let subtitle = |plan: &FigurePlan| plan.header.subtitle.as_ref().unwrap().first().y;
    assert!(close(subtitle(&two) - subtitle(&one), pitch));
    for ((_, a), (_, b)) in one.panels.iter().zip(&two.panels) {
        assert!(close(b.plot.y - a.plot.y, pitch));
        let (ta, tb) = (a.title.as_ref().unwrap(), b.title.as_ref().unwrap());
        assert!(close(tb.first().y - ta.first().y, pitch));
    }
}

#[test]
fn a_pie_title_breaks_into_lines() {
    let pie = |title: &str| {
        let mut f = Figure::new(5.0, 4.0, Theme::pigment()).unwrap();
        f.add_pie_axes()
            .unwrap()
            .title(title)
            .pie(
                vec!["A".into(), "B".into()],
                &[1.0, 2.0],
                PieOptions::default(),
            )
            .unwrap();
        plan_of(&f).unwrap()
    };
    let one = pie("Orders");
    let two = pie("Orders\nby region");
    let title = two.panels[0].1.title.as_ref().unwrap();
    assert_lines(
        title,
        "Orders\nby region",
        Align::Left,
        (PADDING, 5.0 * 72.0 - PADDING),
    );
    let first = one.panels[0].1.title.as_ref().unwrap().first();
    assert_eq!((title.first().x, title.first().y), (first.x, first.y));
    assert!(close(
        two.panels[0].1.header_bottom - one.panels[0].1.header_bottom,
        title.pitch()
    ));
}

#[test]
fn the_width_check_measures_the_widest_line() {
    let wide = "W".repeat(60);
    let width = plan_text_width(&wide);
    assert!(width > 408.0, "{width}");
    for title in [format!("Short\n{wide}"), format!("{wide}\nShort")] {
        let message = single(Align::Left, &title, "")
            .to_scene(&lib())
            .unwrap_err()
            .to_string();
        let prefix = format!(
            "Figure.to_scene(): the title {} is ",
            crate::error::quoted(&title)
        );
        let reported: f64 = message
            .strip_prefix(&prefix)
            .and_then(|rest| rest.split(' ').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("{message}"));
        assert_eq!(reported, width.ceil(), "{message}");
    }
    single(Align::Left, "Short\nlines\nonly", "")
        .to_scene(&lib())
        .unwrap();
}

/// The width of `text` in pigment's title style.
fn plan_text_width(text: &str) -> f64 {
    let f = single(Align::Left, text, "");
    let lib = lib();
    let fonts = layout::figure_fonts(&lib, f.theme()).unwrap();
    let style = crate::TextStyle {
        face: layout::TITLE_FACE,
        weight: crate::Weight::SemiBold,
        size: f.theme().title_size() * f.theme().font_scale(),
        tabular_figures: false,
    };
    f64::from(fonts.layout(text, &style).unwrap().width())
}

#[test]
fn further_lines_raise_the_height_room_error_with_a_height_that_suffices() {
    let title = vec!["Line"; 14].join("\n");
    let short = |height: f64| {
        let mut f = Figure::new(4.0, height, Theme::pigment()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .title(&title)
            .xticks(&[])
            .unwrap()
            .yticks(&[])
            .unwrap();
        f
    };
    let err = short(3.0).to_scene(&lib()).unwrap_err();
    let message = err.to_string();
    let needed: f64 = message
        .split("at least ")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{message}"));
    assert!(message.contains("pt tall"), "{message}");
    assert_eq!(
        err.hint(),
        Some("make the figure taller, or remove the title.")
    );
    // The title's 14 lines alone need more than 3 in.
    let title_height = 1.25 * 15.0 + 13.0 * LINE_PITCH * 15.0;
    assert!(needed > title_height, "{needed} vs {title_height}");
    short(needed / 72.0).to_scene(&lib()).unwrap();
    // Each further line takes room: without the last line the error needs one pitch less.
    let fewer = vec!["Line"; 13].join("\n");
    let mut f = Figure::new(4.0, 3.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title(&fewer)
        .xticks(&[])
        .unwrap()
        .yticks(&[])
        .unwrap();
    let less = f.to_scene(&lib()).unwrap_err().to_string();
    let needed_less: f64 = less
        .split("at least ")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{less}"));
    let pitch = LINE_PITCH * f64::from(Theme::pigment().title_size());
    assert!(
        (needed - needed_less - pitch).abs() <= 1.0,
        "{needed} {needed_less}"
    );
}

#[test]
fn a_figure_header_without_panels_needs_room_for_every_line() {
    let mut f = Figure::new(4.0, 0.6, Theme::pigment()).unwrap();
    f.title("One\nTwo\nThree");
    let err = f.to_scene(&lib()).unwrap_err();
    assert!(err.to_string().contains("pt tall; at least"), "{err}");
    let mut fits = Figure::new(4.0, 2.0, Theme::pigment()).unwrap();
    fits.title("One\nTwo\nThree");
    fits.to_scene(&lib()).unwrap();
}

/// The error of rendering `f`.
fn render_error(f: &Figure) -> Error {
    f.to_scene(&lib()).unwrap_err()
}

#[test]
fn one_line_texts_refuse_line_breaks_when_rendered() {
    let line_breaks = "line breaks are supported in titles, subtitles, text() and annotate().";
    let mut f = single(Align::Left, "Title", "");
    f.single_axes_mut().xlabel("Hours\nUTC");
    let err = render_error(&f);
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the x axis label \"Hours\\nUTC\" holds a line break at index 5; \
         axis labels are one line."
    );
    assert_eq!(err.hint(), Some(line_breaks));

    let mut f = single(Align::Left, "Title", "");
    f.single_axes_mut().ylabel("MW\r");
    assert_eq!(
        render_error(&f).to_string(),
        "Figure.to_scene(): the y axis label \"MW\\r\" holds U+000D (a carriage return) at \
         index 2; text cannot hold control characters."
    );

    // A series name the core does not check when the mark is added is refused when drawn.
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[1.0, 2.0],
            LineOptions {
                label: "North\nSouth".into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    let err = render_error(&f);
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the text \"North\\nSouth\" holds a line break at index 5; labels \
         and names are one line."
    );
    assert_eq!(err.hint(), Some(line_breaks));
}

#[test]
fn titles_refuse_other_control_characters_when_rendered() {
    for (text, shown, index, name) in [
        ("Tab\there", "\"Tab\\there\"", 3, "U+0009 (a tab)"),
        ("a\r\nb", "\"a\\r\\nb\"", 1, "U+000D (a carriage return)"),
        ("nul\0", "\"nul\\0\"", 3, "U+0000"),
        ("next\u{85}", "\"next\\u{85}\"", 4, "U+0085"),
    ] {
        let f = single(Align::Left, text, "");
        let err = render_error(&f);
        assert_eq!(
            err.to_string(),
            format!(
                "Figure.to_scene(): the title {shown} holds {name} at index {index}; text \
                 cannot hold control characters."
            )
        );
        let f = single(Align::Left, "Title", text);
        assert!(
            render_error(&f).to_string().contains("the subtitle"),
            "{text:?}"
        );
        let mut f = single(Align::Left, "", "");
        f.title(text);
        assert!(
            render_error(&f).to_string().contains("the title"),
            "{text:?}"
        );
    }
    let f = single(Align::Left, "a\u{2028}b", "");
    let err = render_error(&f);
    assert_eq!(
        err.to_string(),
        "Figure.to_scene(): the title \"a\\u{2028}b\" holds U+2028 (a line separator) at index \
         1; text cannot hold line or paragraph separators."
    );
    assert_eq!(err.hint(), Some("use \"\\n\" for a line break."));
    let mut f = Figure::new(5.0, 4.0, Theme::pigment()).unwrap();
    f.add_pie_axes()
        .unwrap()
        .title("Orders\u{2029}")
        .pie(vec!["A".into()], &[1.0], PieOptions::default())
        .unwrap();
    assert!(
        render_error(&f)
            .to_string()
            .contains("U+2029 (a paragraph separator) at index 6"),
    );
}

#[test]
fn checked_arguments_refuse_line_breaks_at_the_call() {
    let line_breaks = "line breaks are supported in titles, subtitles, text() and annotate().";
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
    let err = axes
        .bar(
            vec!["North".into(), "South\nEast".into()],
            &[1.0, 2.0],
            crate::BarOptions::default(),
        )
        .unwrap_err();
    assert!(matches!(err, Error::Data { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Axes.bar(): categories[1] holds a line break at index 5; category names are one line."
    );
    assert_eq!(
        (err.parameter(), err.hint()),
        (Some("categories"), Some(line_breaks))
    );
    let err = axes
        .line(
            AxisData::Categories(vec!["a\tb".into()]),
            &[1.0],
            LineOptions::default(),
        )
        .unwrap_err();
    assert!(matches!(err, Error::Data { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Axes.line(): x[0] holds U+0009 (a tab) at index 1; text cannot hold control characters."
    );
    assert_eq!(err.hint(), Some("use spaces instead of tabs."));
    assert!(axes.marks().is_empty());

    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    let err = axes
        .xticks_labeled(&[0.0, 1.0], vec!["low".into(), "high\n".into()])
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Axes.xticks_labeled(): labels[1] holds a line break at index 4; tick labels are one \
         line."
    );
    assert_eq!(err.parameter(), Some("labels"));
    let err = crate::TickFormat::AUTO.affix("\n", "").unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "TickFormat.affix(): prefix holds a line break at index 0; tick labels are one line."
    );

    let mut f = Figure::new(5.0, 4.0, Theme::pigment()).unwrap();
    let err = f
        .add_pie_axes()
        .unwrap()
        .pie(
            vec!["A".into(), "B\u{2028}".into()],
            &[1.0, 2.0],
            PieOptions::default(),
        )
        .unwrap_err();
    assert!(matches!(err, Error::Data { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "PieAxes.pie(): categories[1] holds U+2028 (a line separator) at index 1; text cannot \
         hold line or paragraph separators."
    );
}

#[test]
fn text_and_annotate_take_lines_and_refuse_other_control_characters() {
    use crate::{AnnotateOptions, AxisValue, TextOptions};
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    let at = (AxisValue::Number(0.5), AxisValue::Number(0.5));
    axes.text("Peak\nseason", at.clone(), TextOptions::default())
        .unwrap();
    axes.annotate("Dip\n\nlow", at.clone(), AnnotateOptions::default())
        .unwrap();
    let err = axes
        .text("Tab\tbed", at.clone(), TextOptions::default())
        .unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Axes.text(): text holds U+0009 (a tab) at index 3; text cannot hold control characters."
    );
    assert_eq!(
        (err.parameter(), err.hint()),
        (
            Some("text"),
            Some("use spaces instead of tabs, and \"\\n\" for a line break.")
        )
    );
    let err = axes
        .annotate("a\r\nb", at, AnnotateOptions::default())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Axes.annotate(): text holds U+000D (a carriage return) at index 1; text cannot hold \
         control characters."
    );
    assert_eq!(err.hint(), Some("use \"\\n\" alone for a line break."));
    assert_eq!(axes.annotations().len(), 2);
}

#[test]
fn laying_out_text_refuses_line_breaks() {
    let lib = lib();
    let fonts = lib.with_figure_fonts(&[crate::Font::INTER]).unwrap();
    let style = crate::TextStyle {
        face: crate::FaceId::new(0),
        weight: crate::Weight::Regular,
        size: 10.0,
        tabular_figures: false,
    };
    let err = fonts.layout("a\nb", &style).unwrap_err();
    assert_eq!(
        err.to_string(),
        "SceneFonts.layout(): the text \"a\\nb\" holds a line break at index 1; labels and names \
         are one line."
    );
    let mut scene = Scene::new(10.0, 10.0, crate::Rgba::RED);
    assert!(
        scene
            .push_text(
                &fonts,
                "\u{7f}",
                style,
                crate::Point::default(),
                crate::Rgba::RED
            )
            .is_err()
    );
    assert!(scene.items.is_empty());
}

/// A 6 by 4 in figure without ticks, with a title of three lines, a subtitle of two and an
/// annotation of two.
fn lines_figure() -> Figure {
    use crate::{AnnotateOptions, AxisValue};
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Station output\nDaily mean\nMW")
        .subtitle("Measured hourly\n2026")
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .xticks(&[])
        .unwrap()
        .yticks(&[])
        .unwrap()
        .annotate(
            "Peak\nof the season",
            (AxisValue::Number(0.5), AxisValue::Number(0.3)),
            AnnotateOptions {
                offset: (40.0, 30.0),
                ..AnnotateOptions::default()
            },
        )
        .unwrap();
    f
}

#[test]
fn editable_svg_writes_one_text_element_per_line() {
    use crate::render::{SvgText, render_svg};
    let scene = lines_figure().to_scene(&lib()).unwrap();
    let svg = render_svg(&scene, SvgText::Editable);
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let texts: Vec<(String, f64)> = doc
        .descendants()
        .filter(|n| n.has_tag_name("text"))
        .map(|n| {
            let y = n.attribute("y").unwrap().parse().unwrap();
            (n.text().unwrap_or_default().to_owned(), y)
        })
        .collect();
    let expected = [
        "Station output",
        "Daily mean",
        "MW",
        "Measured hourly",
        "2026",
        "Peak",
        "of the season",
    ];
    assert_eq!(
        texts.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
        expected
    );
    for pair in texts[..5].windows(2).chain(texts[5..].windows(2)) {
        assert!(pair[0].1 < pair[1].1, "{pair:?}");
    }
    assert!(!svg.contains('\t') && !svg.contains('\r') && !svg.contains("&#13;"));
    // As outlines, every line is a path and no text element is left.
    let outlines = render_svg(&scene, SvgText::Outlines);
    assert!(!outlines.contains("<text"));
    let paths = |svg: &str| svg.matches("<path").count();
    let mut one = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    one.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Station output")
        .subtitle("Measured hourly")
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .xticks(&[])
        .unwrap()
        .yticks(&[])
        .unwrap()
        .annotate(
            "Peak",
            (crate::AxisValue::Number(0.5), crate::AxisValue::Number(0.3)),
            crate::AnnotateOptions {
                offset: (40.0, 30.0),
                ..crate::AnnotateOptions::default()
            },
        )
        .unwrap();
    let one = render_svg(&one.to_scene(&lib()).unwrap(), SvgText::Outlines);
    assert_eq!(paths(&outlines), paths(&one) + 4);
}
