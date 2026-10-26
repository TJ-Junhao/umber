//! Text size overrides reach scene text and the layout that reserves its room.

#![allow(clippy::unwrap_used)]

use umber_core::scene::{Item, Scene, TextItem};
use umber_core::{
    AxisData, ColorBy, ColorbarOptions, ColorbarPosition, Figure, FontLibrary, Grid, GridIndex,
    LegendMode, LegendPosition, LegendStyle, LineOptions, Paint, Placement, PointColor, ScaleKind,
    ScatterOptions, SceneFonts, Share, SizeSpec, Theme, TickFormat,
};

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

#[test]
fn pt_size_ignores_font_scale() {
    for scale in [0.5, 1.0, 2.0] {
        let mut figure =
            Figure::new(12.0, 10.0, Theme::pigment().with_font_scale(scale).unwrap()).unwrap();
        figure.title_sized("Figure", SizeSpec::pt(12.0).unwrap());
        let mut axes = figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        axes.title_sized("Panel", SizeSpec::pt(12.0).unwrap())
            .subtitle_sized("Subtitle", SizeSpec::pt(12.0).unwrap())
            .xlabel_sized("Horizontal", SizeSpec::pt(12.0).unwrap())
            .ylabel_sized("Vertical", SizeSpec::pt(12.0).unwrap());
        axes.xtick_labels(SizeSpec::pt(12.0).unwrap()).unwrap();
        axes.ytick_labels(SizeSpec::pt(12.0).unwrap()).unwrap();
        let scene = figure.to_scene(FontLibrary::global()).unwrap();
        assert!(texts(&scene).iter().all(|text| text.style.size == 12.0));
    }
}

#[test]
fn automatic_tick_spacing_uses_each_axis_size() {
    for axis in [0, 1] {
        let mut figure = Figure::new(8.0, 6.0, Theme::pigment()).unwrap();
        let mut axes = figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        axes.xlim(0.0, 1.0).unwrap().ylim(0.0, 1.0).unwrap();
        if axis == 0 {
            axes.xtick_labels(SizeSpec::pt(36.0).unwrap()).unwrap();
        } else {
            axes.ytick_labels(SizeSpec::pt(36.0).unwrap()).unwrap();
        }
        let scene = figure.to_scene(FontLibrary::global()).unwrap();
        let mut labels: Vec<_> = texts(&scene)
            .into_iter()
            .filter(|text| text.style.size == 36.0)
            .collect();
        assert!(labels.len() >= 2);
        labels.sort_by(|a, b| {
            if axis == 0 {
                a.origin.x.total_cmp(&b.origin.x)
            } else {
                a.origin.y.total_cmp(&b.origin.y)
            }
        });
        for pair in labels.windows(2) {
            if axis == 0 {
                assert!(pair[1].origin.x >= pair[0].origin.x + pair[0].width + 18.0 - 1e-3);
            } else {
                let fonts = FontLibrary::global()
                    .with_figure_fonts(&scene.fonts)
                    .unwrap();
                let metrics = fonts.layout("", &pair[0].style).unwrap();
                let height = metrics.ascent() + metrics.descent();
                assert!(pair[1].origin.y >= pair[0].origin.y + height + 18.0 - 1e-3);
            }
        }
    }
}

#[test]
fn shared_tick_size_conflict_rejected_without_mutating_figure() {
    let grid = Grid::new(1, 2).unwrap().with_share_x(Share::All);
    let mut figure = Figure::with_grid(12.0, 4.0, Theme::pigment(), grid).unwrap();
    let left = Placement {
        col: GridIndex::At(0),
        ..Placement::default()
    };
    let right = Placement {
        col: GridIndex::At(1),
        ..Placement::default()
    };
    figure
        .add_axes_at(left, ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .xtick_labels(SizeSpec::pt(8.0).unwrap())
        .unwrap();
    figure
        .add_axes_at(right, ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    let before = figure.clone();
    let error = figure
        .axes_at_mut(GridIndex::Auto, GridIndex::At(1))
        .unwrap()
        .xtick_labels(SizeSpec::pt(9.0).unwrap())
        .unwrap_err();
    assert_eq!(error.parameter(), Some("size"));
    assert_eq!(error.hint(), Some("set the same size on every panel."));
    assert_eq!(figure, before);
}

#[test]
fn merged_legend_size_conflict_rejected_without_mutating_figure() {
    let mut figure =
        Figure::with_grid(12.0, 4.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    for col in [0, 1] {
        figure
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
    }
    figure
        .axes_at_mut(GridIndex::Auto, GridIndex::At(0))
        .unwrap()
        .legend(LegendStyle::Header, SizeSpec::pt(8.0).unwrap())
        .unwrap();
    let before = figure.clone();
    let error = figure
        .axes_at_mut(GridIndex::Auto, GridIndex::At(1))
        .unwrap()
        .legend(LegendStyle::Header, SizeSpec::pt(9.0).unwrap())
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Axes.legend(): size is Size.pt(9); row 0, col 0 uses Size.pt(8), and the figure shows one legend."
    );
    assert_eq!(figure, before);
}

#[derive(Debug)]
struct TextBox {
    name: String,
    size: Option<f32>,
    bounds: (f32, f32, f32, f32),
}

fn scene_boxes(scene: &Scene) -> Vec<TextBox> {
    fn collect(items: &[Item], fonts: &SceneFonts<'_>, boxes: &mut Vec<TextBox>) {
        for item in items {
            match item {
                Item::Text(text) => {
                    let metrics = fonts.layout(&text.text, &text.style).unwrap();
                    boxes.push(TextBox {
                        name: text.text.clone(),
                        size: Some(text.style.size),
                        bounds: (
                            text.origin.x,
                            text.origin.y - metrics.ascent(),
                            text.origin.x + text.width,
                            text.origin.y + metrics.descent(),
                        ),
                    });
                }
                Item::Fill {
                    path,
                    paint: Paint::Sampled { .. },
                    ..
                } => {
                    let (lo, hi) = path.bounds().unwrap();
                    boxes.push(TextBox {
                        name: "colorbar rectangle".into(),
                        size: None,
                        bounds: (lo.x, lo.y, hi.x, hi.y),
                    });
                }
                Item::Group { items, .. } => collect(items, fonts, boxes),
                _ => {}
            }
        }
    }
    let mut boxes = Vec::new();
    let fonts = FontLibrary::global()
        .with_figure_fonts(&scene.fonts)
        .unwrap();
    collect(&scene.items, &fonts, &mut boxes);
    boxes
}

fn overlap(a: &TextBox, b: &TextBox) -> bool {
    let (a, b) = (a.bounds, b.bounds);
    a.2.min(b.2) - a.0.max(b.0) > 0.01 && a.3.min(b.3) - a.1.max(b.1) > 0.01
}

fn assert_text_separation(figure: &Figure, auto: &Figure) {
    let boxes = scene_boxes(&figure.to_scene(FontLibrary::global()).unwrap());
    let baseline = scene_boxes(&auto.to_scene(FontLibrary::global()).unwrap());
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            if !overlap(a, b) {
                continue;
            }
            // AUTO at the equivalent font scale already has contact between a y label and
            // its top tick when their sizes are equal. Large title and subtitle fonts can
            // exceed their line boxes and touch across the fixed gap. Only these pairs are
            // exempt, and only when that same pair actually overlaps.
            let exempt = (a.size == b.size
                && ((a.name.starts_with("Vertical") && b.name.starts_with("Y tick"))
                    || (b.name.starts_with("Vertical") && a.name.starts_with("Y tick"))))
                || (a.name.starts_with("Panel") && b.name.starts_with("Subtitle"))
                || (b.name.starts_with("Panel") && a.name.starts_with("Subtitle"))
                || (a.name == "Figure" && b.name == "Figure subtitle")
                || (b.name == "Figure" && a.name == "Figure subtitle");
            assert!(
                exempt
                    && baseline.iter().filter(|t| t.name == a.name).any(|x| {
                        baseline
                            .iter()
                            .filter(|t| t.name == b.name)
                            .any(|y| overlap(x, y))
                    }),
                "overlapping boxes: {a:?} and {b:?}",
            );
        }
    }
}

fn large_text_figure(
    theme: Theme,
    size: SizeSpec,
    grid: bool,
    style: LegendStyle,
    position: ColorbarPosition,
    sized_ticks: bool,
) -> Figure {
    let count = if grid { 2 } else { 1 };
    let layout = Grid::new(count, count)
        .unwrap()
        .with_legend(LegendMode::PerPanel)
        .with_share_color(if grid { Share::All } else { Share::None });
    let mut figure = Figure::with_grid(80.0, 80.0, theme, layout).unwrap();
    figure
        .title_sized("Figure", size)
        .subtitle_sized("Figure subtitle", size);
    for row in 0..count {
        for col in 0..count {
            let mut axes = figure
                .add_axes_at(
                    Placement {
                        row: GridIndex::At(row),
                        col: GridIndex::At(col),
                        ..Placement::default()
                    },
                    ScaleKind::Linear,
                    ScaleKind::Linear,
                )
                .unwrap();
            axes.scatter(
                AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                &[1.0, 3.0, 2.0],
                ScatterOptions {
                    color: PointColor::By(ColorBy::new(vec![0.0, 5.0, 10.0])),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
            for (name, y) in [("First", [1.0, 2.0, 3.0]), ("Second", [3.0, 2.0, 1.0])] {
                axes.line(
                    AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                    &y,
                    LineOptions {
                        label: format!("{name} {row}{col}"),
                        ..LineOptions::default()
                    },
                )
                .unwrap();
            }
            axes.title_sized(&format!("Panel {row}{col}"), size)
                .subtitle_sized(&format!("Subtitle {row}{col}"), size)
                .xlabel_sized(&format!("Horizontal {row}{col}"), size)
                .ylabel_sized(&format!("Vertical gypsy {row}{col}"), size);
            axes.xticks_labeled(
                &[0.0, 2.0],
                vec![format!("X tick {row}{col}a"), format!("X tick {row}{col}b")],
            )
            .unwrap();
            axes.yticks_labeled(
                &[1.0, 3.0],
                vec![format!("Y tick {row}{col}a"), format!("Y tick {row}{col}b")],
            )
            .unwrap();
            // Keep the top tick on the plot edge to exercise its gap to the y label.
            axes.ylim(0.0, 3.0).unwrap();
            if sized_ticks {
                axes.xtick_labels(size).unwrap().ytick_labels(size).unwrap();
            }
            if style == LegendStyle::Direct {
                axes.legend(style, size).unwrap();
            } else {
                axes.legend_at(style, LegendPosition::Bottom, size).unwrap();
            }
            if !grid {
                axes.colorbar(ColorbarOptions {
                    label: "gypsy".into(),
                    position: Some(position),
                    size,
                    ..ColorbarOptions::default()
                })
                .unwrap();
            }
        }
    }
    if grid {
        figure
            .colorbar(ColorbarOptions {
                label: "gypsy".into(),
                position: Some(position),
                size,
                ..ColorbarOptions::default()
            })
            .unwrap();
    }
    figure
}

/// A `rows` × `cols` grid on a `width` × `height` in figure whose panels share their color
/// scale as `share`, with the figure's colorbar at `position` (the side `share` implies when
/// `None`), its ticks labelled " K", and x tick labels at `size` on ticks at both ends of every
/// x axis.
fn shared_colorbar_figure(
    theme: Theme,
    (rows, cols): (i64, i64),
    (width, height): (f64, f64),
    share: Share,
    position: Option<ColorbarPosition>,
    size: SizeSpec,
) -> Figure {
    let grid = Grid::new(rows, cols).unwrap().with_share_color(share);
    let mut figure = Figure::with_grid(width, height, theme, grid).unwrap();
    figure
        .colorbar(ColorbarOptions {
            label: "Temperature".into(),
            position,
            format: Some(TickFormat::fixed(0).unwrap().affix("", " K").unwrap()),
            ..ColorbarOptions::default()
        })
        .unwrap();
    for row in 0..rows {
        for col in 0..cols {
            let mut axes = figure
                .add_axes_at(
                    Placement {
                        row: GridIndex::At(row),
                        col: GridIndex::At(col),
                        ..Placement::default()
                    },
                    ScaleKind::Linear,
                    ScaleKind::Linear,
                )
                .unwrap();
            axes.scatter(
                AxisData::Numbers(vec![0.0, 1.0, 2.0]),
                &[1.0, 3.0, 2.0],
                ScatterOptions {
                    color: PointColor::By(ColorBy::new(vec![0.0, 5.0, (10 + row + col) as f64])),
                    ..ScatterOptions::default()
                },
            )
            .unwrap();
            axes.xlim(0.0, 2.0).unwrap().xtick_labels(size).unwrap();
        }
    }
    figure
}

/// The figure side, in inches, that an error naming the figure's width or height as too small
/// says suffices: `(true, w)` for a width, `(false, h)` for a height.
fn sufficient_side(message: &str) -> Option<(bool, f64)> {
    let (_, rest) = message.split_once("the figure is ")?;
    let wide = rest.split_once(" pt ")?.1.starts_with("wide");
    let (_, rest) = rest.split_once("at least ")?;
    let points: f64 = rest.split_once(" pt")?.0.parse().ok()?;
    Some((wide, points / 72.0))
}

#[test]
fn x_tick_labels_keep_clear_of_shared_colorbars() {
    let theme = Theme::pigment();
    let scale = 48.0 / f64::from(theme.body_size());
    let mut failures = Vec::new();
    for (rows, cols) in [(2, 2), (1, 3)] {
        // A row's bar sits right of it and a column's below it; a figure's on either side.
        for (share, position) in [
            (Share::Row, None),
            (Share::Col, None),
            (Share::All, Some(ColorbarPosition::Right)),
            (Share::All, Some(ColorbarPosition::Bottom)),
        ] {
            for (theme, size) in [
                (theme.clone(), SizeSpec::pt(48.0).unwrap()),
                (theme.with_font_scale(scale).unwrap(), SizeSpec::Auto),
            ] {
                let case = format!("{rows} x {cols} {share:?} {position:?} {size:?}");
                let mut side = (4.0 * cols as f64 + 2.0, 3.5 * rows as f64);
                // A figure too small for its texts names a size that suffices; that renders.
                let scene = loop {
                    let figure = shared_colorbar_figure(
                        theme.clone(),
                        (rows, cols),
                        side,
                        share,
                        position,
                        size,
                    );
                    match figure.to_scene(FontLibrary::global()) {
                        Ok(scene) => break Some(scene),
                        Err(error) => match sufficient_side(&error.to_string()) {
                            Some((true, width)) if width > side.0 => side.0 = width,
                            Some((false, height)) if height > side.1 => side.1 = height,
                            _ => {
                                failures.push(format!("{case} at {side:?}: {error}"));
                                break None;
                            }
                        },
                    }
                };
                let Some(scene) = scene else {
                    continue;
                };
                let boxes = scene_boxes(&scene);
                let colorbar: Vec<_> = boxes
                    .iter()
                    .filter(|b| {
                        b.name == "colorbar rectangle"
                            || b.name == "Temperature"
                            || b.name.ends_with(" K")
                    })
                    .collect();
                let x_ticks: Vec<_> = boxes
                    .iter()
                    .filter(|b| b.name.parse::<f64>().is_ok())
                    .collect();
                assert!(colorbar.len() > 2 && x_ticks.len() >= 2, "{case}");
                for tick in &x_ticks {
                    for bar in colorbar.iter().filter(|bar| overlap(tick, bar)) {
                        failures.push(format!("{case}: {tick:?} overlaps {bar:?}"));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn large_text_boxes_do_not_overlap() {
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        for size in [
            SizeSpec::pt(30.0).unwrap(),
            SizeSpec::pt(48.0).unwrap(),
            SizeSpec::pt(72.0).unwrap(),
            SizeSpec::times(4.0).unwrap(),
        ] {
            let scale = match size {
                SizeSpec::Points(points) => points.get() / f64::from(theme.body_size()),
                SizeSpec::Times(factor) => factor.get(),
                SizeSpec::Auto => 1.0,
            };
            for grid in [false, true] {
                for style in [
                    LegendStyle::Header,
                    LegendStyle::Chips,
                    LegendStyle::Direct,
                    LegendStyle::None,
                ] {
                    for position in [ColorbarPosition::Right, ColorbarPosition::Bottom] {
                        for sized_ticks in [false, true] {
                            let figure = large_text_figure(
                                theme.clone(),
                                size,
                                grid,
                                style,
                                position,
                                sized_ticks,
                            );
                            let auto = large_text_figure(
                                theme.clone().with_font_scale(scale).unwrap(),
                                SizeSpec::Auto,
                                grid,
                                style,
                                position,
                                sized_ticks,
                            );
                            assert_text_separation(&figure, &auto);
                        }
                    }
                }
            }
        }
    }
}
