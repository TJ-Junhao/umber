//! Text written in a series color (direct labels, point labels, the value column) reaches a
//! contrast of 4.5:1 with the background, while marks, swatches and end dots keep the series
//! color, and text with a color of its own keeps that color.

#![allow(clippy::unwrap_used, clippy::panic)]

use umber_core::scene::{Item, Paint, Scene, TextItem};
use umber_core::{
    Align, AxesMut, AxisData, ColorSpec, Figure, FontLibrary, GridIndex, LineOptions, Marker,
    Place, PointColor, PointLabels, Rgba, ScaleKind, ScatterGroup, ScatterOptions, SizeSpec,
    SvgText, TextOptions, Theme, TickFormat,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;

/// The least contrast ratio of text against the background.
const LEAST: f64 = 4.5;
/// A pale yellow: far below 4.5:1 on a light background.
const PALE: Rgba = Rgba::opaque(0xF2, 0xE0, 0x30);

/// The WCAG 2 contrast ratio of two colors, computed here from the definition.
fn ratio(a: Rgba, b: Rgba) -> f64 {
    let luminance = |c: Rgba| {
        let lin = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn axes(figure: &mut Figure) -> AxesMut<'_> {
    figure
        .axes_at_mut(GridIndex::Auto, GridIndex::Auto)
        .unwrap()
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

/// The last text item of `scene` that reads `content`.
fn text<'a>(scene: &'a Scene, content: &str) -> &'a TextItem {
    leaves(&scene.items)
        .into_iter()
        .rev()
        .find_map(|item| match item {
            Item::Text(t) if t.text == content => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text {content:?}"))
}

/// The fill colors of the markers (opaque), the solid fills and the stroke colors of `scene`.
fn mark_colors(scene: &Scene) -> Vec<Rgba> {
    leaves(&scene.items)
        .into_iter()
        .filter_map(|item| match item {
            // Markers are filled at the theme's marker alpha; compare the color alone.
            Item::Markers { fill, .. } => fill.map(|c| Rgba { a: 255, ..c }),
            Item::Fill {
                paint: Paint::Solid(c),
                ..
            } => Some(*c),
            Item::Stroke { style, .. } => Some(style.color),
            _ => None,
        })
        .collect()
}

/// The color text in series color `color` takes on `theme`: unchanged when it reads, else a
/// different color that does.
fn assert_readable(theme: &Theme, color: Rgba, text: Rgba, what: &str) {
    let background = theme.background();
    assert!(ratio(text, background) >= LEAST, "{what}: {text:?}");
    if ratio(color, background) >= LEAST {
        assert_eq!(text, color, "{what}");
    } else {
        assert_ne!(text, color, "{what}");
    }
}

fn labelled_line(label: &str) -> LineOptions {
    LineOptions {
        label: label.to_owned(),
        ..LineOptions::default()
    }
}

/// Four labelled lines whose ends are well apart, so each takes a direct label.
fn four_lines(theme: Theme) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    for (i, name) in ["One", "Two", "Three", "Four"].into_iter().enumerate() {
        let level = 10.0 * i as f64;
        axes(&mut f)
            .line(
                AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                &[level, level + 3.0, level + 6.0],
                labelled_line(name),
            )
            .unwrap();
    }
    f
}

#[test]
fn direct_labels_read_on_every_theme_and_their_marks_keep_the_series_color() {
    let mut changed = 0;
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let palette = theme.palette().to_vec();
        let scene = render(&four_lines(theme.clone()));
        let marks = mark_colors(&scene);
        for (i, name) in ["One", "Two", "Three", "Four"].into_iter().enumerate() {
            let item = text(&scene, name);
            let series = palette[i];
            // A theme with a legend row has no direct label to check; Graphite labels directly.
            if item.color == theme.text_color() {
                continue;
            }
            assert_readable(&theme, series, item.color, name);
            changed += usize::from(item.color != series);
            // The line, and any end dot, keep the series color.
            assert!(marks.contains(&series), "{name}: {series:?} in {marks:?}");
            assert!(
                !marks.contains(&item.color) || item.color == series,
                "{name}"
            );
        }
    }
    assert!(changed > 0, "some direct label needs a move");
}

#[test]
fn an_end_dot_and_a_line_keep_the_color_of_a_label_that_moved() {
    let theme = Theme::graphite();
    let palette = theme.palette().to_vec();
    let scene = render(&four_lines(theme.clone()));
    let moved: Vec<usize> = ["One", "Two", "Three", "Four"]
        .into_iter()
        .enumerate()
        .filter(|(i, name)| text(&scene, name).color != palette[*i])
        .map(|(i, _)| i)
        .collect();
    assert!(!moved.is_empty(), "a Graphite color is under 4.5:1");
    let marks = mark_colors(&scene);
    for i in moved {
        let dots = leaves(&scene.items)
            .into_iter()
            .filter(|item| matches!(item, Item::Fill { paint: Paint::Solid(c), .. } if *c == palette[i]))
            .count();
        assert_eq!(dots, 1, "series {i}: one end dot in the series color");
        assert!(marks.contains(&palette[i]));
    }
}

/// A scatter of one labelled point in `color`.
fn point(color: PointColor, x: f64, place: Place) -> ScatterOptions {
    ScatterOptions {
        color,
        marker: Marker::Circle,
        labels: PointLabels {
            annotate: true,
            format: Some(TickFormat::fixed(1).unwrap()),
            place,
            ..PointLabels::default()
        },
        label: format!("S{x}"),
        ..ScatterOptions::default()
    }
}

fn palette_color(index: usize) -> PointColor {
    PointColor::Uniform(ColorSpec::Palette {
        index,
        alpha: umber_core::Alpha::OPAQUE,
    })
}

/// A figure with one labelled point per palette color (and `extra`), in `theme`.
fn scattered(theme: Theme, extra: Option<Rgba>, place: Place) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    for i in 0..theme.palette().len() {
        let x = i as f64 + 1.0;
        axes(&mut f)
            .scatter(
                AxisData::Numbers(vec![x]),
                &[x + 0.25],
                point(palette_color(i), x, place),
            )
            .unwrap();
    }
    if let Some(color) = extra {
        let x = theme.palette().len() as f64 + 1.0;
        axes(&mut f)
            .scatter(
                AxisData::Numbers(vec![x]),
                &[x + 0.25],
                point(PointColor::Uniform(ColorSpec::Fixed(color)), x, place),
            )
            .unwrap();
    }
    f
}

fn check_point_labels(theme: &Theme, extra: Option<Rgba>, place: Place) {
    let scene = render(&scattered(theme.clone(), extra, place));
    let marks = mark_colors(&scene);
    let mut colors = theme.palette().to_vec();
    colors.extend(extra);
    for (i, color) in colors.into_iter().enumerate() {
        let content = format!("{:.1}", i as f64 + 1.25);
        let label = text(&scene, &content);
        assert_readable(theme, color, label.color, &content);
        assert!(
            marks.contains(&color),
            "{content}: the marker keeps {color:?}"
        );
    }
}

#[test]
fn point_labels_read_in_every_theme_and_their_markers_keep_the_series_color() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        check_point_labels(&theme, None, Place::Right);
    }
}

#[test]
fn value_columns_read_in_every_theme_and_their_markers_keep_the_series_color() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Category).unwrap();
        let names: Vec<String> = (0..theme.palette().len())
            .map(|i| format!("m{i}"))
            .collect();
        for (i, name) in names.iter().enumerate() {
            let options = ScatterOptions {
                color: palette_color(i),
                labels: PointLabels {
                    annotate: true,
                    format: Some(TickFormat::fixed(1).unwrap()),
                    place: Place::Column,
                    ..PointLabels::default()
                },
                ..ScatterOptions::default()
            };
            axes(&mut f)
                .scatter_shared(
                    AxisData::Numbers(vec![i as f64 + 1.25]),
                    AxisData::Categories(vec![name.clone()]),
                    options,
                )
                .unwrap();
        }
        let scene = render(&f);
        let marks = mark_colors(&scene);
        for (i, &color) in theme.palette().iter().enumerate() {
            let content = format!("{:.1}", i as f64 + 1.25);
            assert_readable(&theme, color, text(&scene, &content).color, &content);
            assert!(
                marks.contains(&color),
                "{content}: the marker keeps {color:?}"
            );
        }
    }
}

#[test]
fn a_color_given_to_the_series_is_a_mark_color_and_text_still_reads() {
    // Pale yellow on a light background and a pale blue-gray on a dark one.
    check_point_labels(&Theme::pigment(), Some(PALE), Place::Right);
    let dark = Theme::midnight();
    check_point_labels(&dark, Some(Rgba::opaque(0x20, 0x30, 0x50)), Place::Right);
    // A light and a dark background of a custom theme.
    let light = Theme::pigment()
        .with_background(Rgba::opaque(0xFF, 0xF8, 0xE0))
        .unwrap();
    check_point_labels(&light, Some(PALE), Place::Right);
}

#[test]
fn a_palette_set_with_with_palette_is_made_readable_too() {
    let pale = [
        PALE,
        Rgba::opaque(0xC8, 0xF0, 0xA0),
        Rgba::opaque(0x20, 0x20, 0x20),
    ];
    let light = Theme::pigment().with_palette(&pale).unwrap();
    check_point_labels(&light, None, Place::Right);
    let dark = Theme::midnight().with_palette(&pale).unwrap();
    check_point_labels(&dark, None, Place::Right);
    // The lines of a figure in the new palette are drawn in it as given.
    let scene = render(&four_lines(light));
    assert!(mark_colors(&scene).contains(&PALE));
}

#[test]
fn grouped_scatters_label_each_group_in_its_readable_color() {
    let theme = Theme::pigment();
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    let groups: Vec<ScatterGroup> = (0..4)
        .map(|i| {
            ScatterGroup::new(
                format!("g{i}"),
                AxisData::Numbers(vec![f64::from(i) + 1.0]),
                AxisData::Numbers(vec![f64::from(i) * 2.0 + 1.0]),
            )
        })
        .collect();
    axes(&mut f)
        .grouped_scatter(
            groups,
            Marker::Circle,
            SizeSpec::Auto,
            PointLabels {
                annotate: true,
                format: Some(TickFormat::fixed(1).unwrap()),
                ..PointLabels::default()
            },
            false,
        )
        .unwrap();
    let scene = render(&f);
    let marks = mark_colors(&scene);
    for (i, &color) in theme.palette()[..4].iter().enumerate() {
        let content = format!("{:.1}", i as f64 * 2.0 + 1.0);
        assert_readable(&theme, color, text(&scene, &content).color, &content);
        assert!(marks.contains(&color));
    }
}

#[test]
fn text_with_a_color_of_its_own_is_left_alone() {
    let theme = Theme::pigment();
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Category).unwrap();
    axes(&mut f)
        .scatter_shared(
            AxisData::Numbers(vec![2.0]),
            AxisData::Categories(vec!["north".to_owned()]),
            ScatterOptions::default(),
        )
        .unwrap()
        .ytick_highlight(vec!["north".to_owned()], ColorSpec::Fixed(PALE))
        .unwrap()
        .text(
            "note",
            umber_core::InPlot::new(0.5, 0.5).unwrap(),
            TextOptions {
                align: Align::Center,
                color: ColorSpec::Fixed(PALE),
                ..TextOptions::default()
            },
        )
        .unwrap();
    let scene = render(&f);
    assert_eq!(text(&scene, "note").color, PALE);
    assert_eq!(text(&scene, "north").color, PALE);
}

#[test]
fn png_svg_and_pdf_agree_on_the_moved_text_color() {
    let theme = Theme::pigment();
    let f = scattered(theme.clone(), None, Place::Right);
    let scene = render(&f);
    let label = text(&scene, "3.2");
    assert_ne!(label.color, theme.palette()[2], "the third color moves");
    for mode in [SvgText::Editable, SvgText::Outlines] {
        let svg = f.to_svg(mode).unwrap();
        assert_eq!(svg, umber_core::render::render_svg(&scene, mode));
        let hex = format!(
            "fill=\"#{:02X}{:02X}{:02X}\"",
            label.color.r, label.color.g, label.color.b
        );
        assert!(svg.contains(&hex), "{mode:?}: {hex}");
    }
    let png = f.to_png(144).unwrap();
    assert_eq!(png, umber_core::render::render_png(&scene, 144).unwrap());
    let pdf = f.to_pdf().unwrap();
    assert_eq!(pdf, umber_core::render::render_pdf(&scene).unwrap());
    if pdf_check::have("pdftoppm") {
        let dir = pdf_check::scratch("series-text");
        let path = pdf_check::write(&dir, "labels.pdf", &pdf);
        pdf_check::assert_matches_png(&path, &png, 144);
    } else {
        eprintln!("skipped the PDF raster comparison: pdftoppm is not installed");
    }
}

#[test]
fn a_translucent_series_gets_opaque_readable_label_text() {
    let theme = Theme::graphite();
    let line = Rgba {
        a: 0x50,
        ..Rgba::opaque(0xF2, 0xA9, 0x00)
    };
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    axes(&mut f)
        .line(
            AxisData::Numbers(vec![0.0, 1.0, 2.0]),
            &[0.0, 1.0, 2.0],
            LineOptions {
                label: "Faint".to_owned(),
                color: ColorSpec::Fixed(line),
                ..LineOptions::default()
            },
        )
        .unwrap();
    let scene = render(&f);
    let label = text(&scene, "Faint");
    assert_eq!(label.color.a, 255);
    assert!(
        ratio(label.color, theme.background()) >= LEAST,
        "{:?}",
        label.color
    );
    // The line and the end dot keep their color and alpha.
    let marks: Vec<Rgba> = leaves(&scene.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Stroke { style, .. } => Some(style.color),
            Item::Fill {
                paint: Paint::Solid(c),
                ..
            } => Some(*c),
            _ => None,
        })
        .collect();
    assert!(marks.contains(&line), "{marks:?}");
}

#[test]
fn a_mid_gray_page_gets_label_text_that_reads() {
    let gray = Rgba::opaque(0x66, 0x66, 0x66);
    let theme = Theme::graphite().with_background(gray).unwrap();
    let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    axes(&mut f)
        .line(
            AxisData::Numbers(vec![0.0, 1.0, 2.0]),
            &[0.0, 1.0, 2.0],
            LineOptions {
                label: "Gray".to_owned(),
                color: ColorSpec::Fixed(gray),
                ..LineOptions::default()
            },
        )
        .unwrap();
    let label = text(&render(&f), "Gray").color;
    assert!(ratio(label, gray) >= LEAST, "{label:?}");
}

#[test]
fn color_mapped_labels_keep_the_theme_text_color() {
    let theme = Theme::pigment().with_text_color(PALE).unwrap();
    for place in [Place::Right, Place::Column] {
        let mut f = Figure::new(6.0, 4.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes(&mut f)
            .scatter(
                AxisData::Numbers(vec![1.0, 2.0]),
                &[5.5, 6.5],
                ScatterOptions {
                    color: PointColor::By(umber_core::ColorBy::new(vec![0.0, 1.0])),
                    labels: PointLabels {
                        annotate: true,
                        place,
                        ..PointLabels::default()
                    },
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
        let scene = render(&f);
        assert_eq!(text(&scene, "5.5").color, PALE, "{place:?}");
        assert_eq!(text(&scene, "6.5").color, PALE, "{place:?}");
    }
}
