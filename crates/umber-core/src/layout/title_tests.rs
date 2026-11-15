//! Titles must fit the same horizontal box in which they are aligned.

use super::*;
use crate::{Align, Figure, Grid, GridIndex, PieOptions, Placement, ScaleKind};

const TEXT: &str = "A very long title that certainly cannot fit in six inches of width at all, even as a subtitle: WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";
const SCALE_HINT: &str =
    "make the figure wider or lower the font scale with Theme.with_font_scale().";
const POINTS_HINT: &str = "make the figure wider or pass a smaller size=.";
const SHRUNK_TITLE_HINT: &str =
    "make the figure wider, shorten the title, or break it into lines with \"\\n\".";
const SHRUNK_SUBTITLE_HINT: &str =
    "make the figure wider, shorten the subtitle, or break it into lines with \"\\n\".";

#[derive(Clone, Copy)]
enum Kind {
    FigureTitle,
    FigureSubtitle,
    AxesTitle,
    AxesSubtitle,
    GridTitle,
    GridSubtitle,
    SpanningTitle,
    TwinTitle,
    TwinSubtitle,
    PieTitle,
}

impl Kind {
    fn is_figure(self) -> bool {
        matches!(self, Self::FigureTitle | Self::FigureSubtitle)
    }

    fn is_subtitle(self) -> bool {
        matches!(
            self,
            Self::FigureSubtitle | Self::AxesSubtitle | Self::GridSubtitle | Self::TwinSubtitle
        )
    }

    fn cols(self) -> i64 {
        match self {
            Self::GridTitle | Self::GridSubtitle => 2,
            Self::SpanningTitle => 3,
            _ => 1,
        }
    }

    fn span(self) -> i64 {
        if matches!(self, Self::SpanningTitle) {
            2
        } else {
            1
        }
    }

    /// The least size, in points, the text shrinks to in pigment: the panel title size for
    /// the figure title, the axis label size otherwise.
    fn floor(self) -> f32 {
        let theme = Theme::pigment();
        if matches!(self, Self::FigureTitle) {
            theme.title_size()
        } else {
            theme.body_size()
        }
    }

    fn room(self, width: f64) -> f64 {
        let gap = 24.0;
        let cell = (width - 2.0 * PADDING - gap * (self.cols() - 1) as f64) / self.cols() as f64;
        cell * self.span() as f64 + gap * (self.span() - 1) as f64
    }
}

fn figure(kind: Kind, width: f64, text: &str, size: SizeSpec, align: Align) -> Figure {
    let theme = Theme::pigment().with_title_align(align).unwrap();
    let mut f =
        Figure::with_grid(width / 72.0, 4.0, theme, Grid::new(1, kind.cols()).unwrap()).unwrap();
    if kind.is_figure() {
        if kind.is_subtitle() {
            f.subtitle_sized(text, size);
        } else {
            f.title_sized(text, size);
        }
    } else if matches!(kind, Kind::PieTitle) {
        f.add_pie_axes()
            .unwrap()
            .title_sized(text, size)
            .pie(vec!["A".into()], &[1.0], PieOptions::default())
            .unwrap();
    } else {
        let mut ax = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(0),
                    col_span: kind.span(),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        ax.xticks(&[]).unwrap().yticks(&[]).unwrap();
        if kind.is_subtitle() {
            ax.subtitle_sized(text, size);
        } else {
            ax.title_sized(text, size);
        }
        if matches!(kind, Kind::TwinTitle | Kind::TwinSubtitle) {
            ax.twin_y(ScaleKind::Linear).unwrap().yticks(&[]).unwrap();
        }
        if kind.cols() > 1 {
            f.add_axes_at(
                Placement {
                    col: GridIndex::At(kind.span()),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap()
            .xticks(&[])
            .unwrap()
            .yticks(&[])
            .unwrap();
        }
    }
    f
}

fn text_width(kind: Kind, text: &str, size: SizeSpec) -> f64 {
    let theme = Theme::pigment();
    let fonts = figure_fonts(FontLibrary::global(), &theme).unwrap();
    let styles = Styles::new(
        &theme,
        if kind.is_figure() {
            TextRole::FigureTitle
        } else {
            TextRole::Title
        },
        StyleSizes {
            title: size,
            body: size,
            ..Default::default()
        },
    );
    measure(
        &fonts,
        text,
        if kind.is_subtitle() {
            styles.body
        } else {
            styles.title
        },
    )
    .unwrap()
    .width()
}

fn refuses(kind: Kind) {
    for align in [Align::Left, Align::Center, Align::Right] {
        for size in [
            SizeSpec::Auto,
            SizeSpec::times(1.4).unwrap(),
            SizeSpec::pt(18.0).unwrap(),
        ] {
            let error = figure(kind, 432.0, TEXT, size, align)
                .to_scene(FontLibrary::global())
                .unwrap_err();
            assert!(matches!(error, Error::Style { .. }), "{error}");
            let prefix = if kind.cols() > 1 {
                "row 0, col 0: "
            } else {
                ""
            };
            let name = if kind.is_subtitle() {
                "subtitle"
            } else {
                "title"
            };
            let container = if kind.is_figure() { "figure" } else { "panel" };
            // Without a size, the text shrank to its floor and is still too wide there.
            let (size, at, hint) = match size {
                SizeSpec::Auto => {
                    let floor = kind.floor();
                    (
                        SizeSpec::pt(f64::from(floor)).unwrap(),
                        format!(" at {floor} pt, the smallest size it shrinks to"),
                        if kind.is_subtitle() {
                            SHRUNK_SUBTITLE_HINT
                        } else {
                            SHRUNK_TITLE_HINT
                        },
                    )
                }
                SizeSpec::Points(_) => (size, String::new(), POINTS_HINT),
                SizeSpec::Times(_) => (size, String::new(), SCALE_HINT),
            };
            assert_eq!(
                error.to_string(),
                format!(
                    "Figure.to_scene(): {prefix}the {name} {} is {} pt wide{at}; the {container} has {} pt.",
                    quoted(TEXT),
                    text_width(kind, TEXT, size).ceil(),
                    kind.room(432.0).floor(),
                )
            );
            assert_eq!(error.parameter(), None);
            assert_eq!(error.hint(), Some(hint));
        }
    }
}

#[test]
fn figure_title_must_fit() {
    refuses(Kind::FigureTitle);
}

#[test]
fn figure_subtitle_must_fit() {
    refuses(Kind::FigureSubtitle);
}

#[test]
fn axes_title_must_fit() {
    refuses(Kind::AxesTitle);
}

#[test]
fn axes_subtitle_must_fit() {
    refuses(Kind::AxesSubtitle);
}

#[test]
fn grid_title_cannot_reach_its_neighbour() {
    refuses(Kind::GridTitle);
}

#[test]
fn grid_subtitle_cannot_reach_its_neighbour() {
    refuses(Kind::GridSubtitle);
}

#[test]
fn spanning_title_must_fit_its_span() {
    refuses(Kind::SpanningTitle);
}

#[test]
fn twin_panel_title_must_fit() {
    refuses(Kind::TwinTitle);
}

#[test]
fn twin_panel_subtitle_must_fit() {
    refuses(Kind::TwinSubtitle);
}

#[test]
fn pie_title_must_fit() {
    refuses(Kind::PieTitle);
}

#[test]
fn titles_at_the_limit_and_within_tolerance_render() {
    for kind in [
        Kind::FigureTitle,
        Kind::FigureSubtitle,
        Kind::AxesTitle,
        Kind::AxesSubtitle,
        Kind::GridTitle,
        Kind::GridSubtitle,
        Kind::SpanningTitle,
        Kind::TwinTitle,
        Kind::TwinSubtitle,
        Kind::PieTitle,
    ] {
        for align in [Align::Left, Align::Center, Align::Right] {
            let size = SizeSpec::pt(18.0).unwrap();
            let text = "Station output";
            let needed = text_width(kind, text, size);
            let scale = kind.cols() as f64 / kind.span() as f64;
            let width = 432.0 + (needed - kind.room(432.0)) * scale;
            for deficit in [0.0, inset::CONTAINMENT_TOLERANCE / 2.0] {
                figure(kind, width - deficit * scale, text, size, align)
                    .to_scene(FontLibrary::global())
                    .unwrap();
            }
            let failing_width = width - 0.25 * scale;
            let error = figure(kind, failing_width, text, size, align)
                .to_scene(FontLibrary::global())
                .unwrap_err();
            let room = kind.room(failing_width).floor();
            assert!(
                error
                    .to_string()
                    .contains(&format!("is {} pt wide;", needed.ceil()))
            );
            assert!(error.to_string().ends_with(&format!("has {room} pt.")));
            figure(
                kind,
                failing_width + (needed.ceil() - room) * scale,
                text,
                size,
                align,
            )
            .to_scene(FontLibrary::global())
            .unwrap();
        }
    }
}

fn axis_figure(
    kind: Kind,
    right: bool,
    axis: &str,
    width: f64,
    text: &str,
    size: SizeSpec,
) -> Figure {
    let mut f = figure(kind, width, "", SizeSpec::Auto, Align::Left);
    if right && !matches!(kind, Kind::TwinTitle) {
        f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
            .unwrap()
            .twin_y(ScaleKind::Linear)
            .unwrap()
            .yticks(&[])
            .unwrap();
    }
    let mut axes = if right {
        f.twin_axes_at_mut(GridIndex::At(0), GridIndex::At(0))
    } else {
        f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
    }
    .unwrap();
    if axis == "x" {
        axes.xlabel_sized(text, size);
    } else {
        axes.ylabel_sized(text, size);
    }
    f
}

fn axis_refuses(kind: Kind, right: bool) {
    for axis in ["x", "y"] {
        for size in [
            SizeSpec::Auto,
            SizeSpec::times(1.4).unwrap(),
            SizeSpec::pt(18.0).unwrap(),
        ] {
            let error = axis_figure(kind, right, axis, 432.0, TEXT, size)
                .to_scene(FontLibrary::global())
                .unwrap_err();
            assert!(matches!(error, Error::Style { .. }), "{error}");
            let prefix = if kind.cols() > 1 {
                "row 0, col 0: "
            } else {
                ""
            };
            let side = if axis == "y" && matches!(kind, Kind::TwinTitle) {
                if right { "right " } else { "left " }
            } else {
                ""
            };
            assert_eq!(
                error.to_string(),
                format!(
                    "Figure.to_scene(): {prefix}the {side}{axis} axis label {} is {} pt wide; the panel has {} pt.",
                    quoted(TEXT),
                    text_width(Kind::AxesSubtitle, TEXT, size).ceil(),
                    kind.room(432.0).floor(),
                )
            );
            let remedy = if matches!(size, SizeSpec::Points(_)) {
                "pass a smaller size=."
            } else {
                "lower the font scale with Theme.with_font_scale()."
            };
            assert_eq!(error.parameter(), None);
            assert_eq!(
                error.hint(),
                Some(format!("make the figure wider or {remedy}").as_str())
            );
        }
    }
}

#[test]
fn single_panel_axis_titles_must_fit() {
    axis_refuses(Kind::AxesTitle, false);
}

#[test]
fn grid_axis_titles_must_fit() {
    axis_refuses(Kind::GridTitle, false);
    axis_refuses(Kind::SpanningTitle, false);
}

#[test]
fn twin_axis_titles_must_fit() {
    axis_refuses(Kind::TwinTitle, false);
    axis_refuses(Kind::TwinTitle, true);
}

#[test]
fn axis_titles_at_the_limit_and_within_tolerance_render() {
    for (kind, right) in [
        (Kind::AxesTitle, false),
        (Kind::GridTitle, false),
        (Kind::SpanningTitle, false),
        (Kind::TwinTitle, false),
        (Kind::TwinTitle, true),
    ] {
        for axis in ["x", "y"] {
            let size = SizeSpec::pt(18.0).unwrap();
            let text = "Station output";
            let needed = text_width(Kind::AxesSubtitle, text, size);
            let scale = kind.cols() as f64 / kind.span() as f64;
            let width = 432.0 + (needed - kind.room(432.0)) * scale;
            for deficit in [0.0, inset::CONTAINMENT_TOLERANCE / 2.0] {
                axis_figure(kind, right, axis, width - deficit * scale, text, size)
                    .to_scene(FontLibrary::global())
                    .unwrap();
            }
            let failing = width - 0.25 * scale;
            let error = axis_figure(kind, right, axis, failing, text, size)
                .to_scene(FontLibrary::global())
                .unwrap_err();
            let room = kind.room(failing).floor();
            assert!(error.to_string().ends_with(&format!(
                "is {} pt wide; the panel has {room} pt.",
                needed.ceil(),
            )));
            axis_figure(
                kind,
                right,
                axis,
                failing + (needed.ceil() - room) * scale,
                text,
                size,
            )
            .to_scene(FontLibrary::global())
            .unwrap();
        }
    }
}

#[test]
fn title_error_quotes_escaped_text() {
    let text = "\"quoted\" \\ title";
    let size = SizeSpec::pt(18.0).unwrap();
    let width = text_width(Kind::AxesTitle, text, size) + 2.0 * PADDING - 1.0;
    let error = figure(Kind::AxesTitle, width, text, size, Align::Left)
        .to_scene(FontLibrary::global())
        .unwrap_err();
    assert!(error.to_string().contains(&quoted(text)));
}

#[test]
fn one_cell_twin_keeps_title_alignment_and_room_with_ticks() {
    let text = "Station output across the whole figure";
    let size = SizeSpec::pt(18.0).unwrap();
    let width = text_width(Kind::AxesTitle, text, size) + 2.0 * PADDING;
    for align in [Align::Left, Align::Center, Align::Right] {
        for subtitle in [false, true] {
            let build = |width, twin| {
                let theme = Theme::pigment().with_title_align(align).unwrap();
                let mut f = Figure::new(width / 72.0, 4.0, theme).unwrap();
                let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
                ax.xticks(&[0.0, 1.0])
                    .unwrap()
                    .yticks_labeled(&[0.0, 1.0], vec!["Wide ticks".into(); 2])
                    .unwrap();
                if subtitle {
                    ax.subtitle_sized(text, size);
                } else {
                    ax.title_sized(text, size);
                }
                if twin {
                    ax.twin_y(ScaleKind::Linear)
                        .unwrap()
                        .yticks_labeled(&[0.0, 1.0], vec!["Right ticks".into(); 2])
                        .unwrap();
                }
                f
            };
            let needed = text_width(
                if subtitle {
                    Kind::AxesSubtitle
                } else {
                    Kind::AxesTitle
                },
                text,
                size,
            );
            let width = if subtitle {
                needed + 2.0 * PADDING
            } else {
                width
            };
            let single =
                crate::pipeline::try_plan_panels(&build(width, false), FontLibrary::global())
                    .unwrap();
            let twin = crate::pipeline::try_plan_panels(&build(width, true), FontLibrary::global())
                .unwrap();
            let label = |layout: &Layout| {
                let label = if subtitle {
                    &layout.subtitle
                } else {
                    &layout.title
                };
                let label = label.as_ref().unwrap().first();
                (label.x, label.y, label.width())
            };
            assert_eq!(label(&single[0].1), label(&twin[0].1));
            assert!(!twin[0].1.y_tick_labels.is_empty());
            assert!(!twin[1].1.y_tick_labels.is_empty());
            assert!(label(&twin[0].1).2 > twin[0].1.plot.width);
            for has_twin in [false, true] {
                let error = build(width - 0.25, has_twin)
                    .to_scene(FontLibrary::global())
                    .unwrap_err();
                assert!(error.to_string().contains(if subtitle {
                    "the subtitle"
                } else {
                    "the title"
                }));
                assert_eq!(error.hint(), Some(POINTS_HINT));
            }
        }
    }
}

#[test]
fn axis_title_limits_follow_placement_with_ticks() {
    for (kind, right) in [
        (Kind::AxesTitle, false),
        (Kind::GridTitle, false),
        (Kind::SpanningTitle, false),
        (Kind::TwinTitle, false),
        (Kind::TwinTitle, true),
        (Kind::GridTitle, true),
        (Kind::SpanningTitle, true),
    ] {
        for axis in ["x", "y"] {
            let text = "Station output";
            let size = SizeSpec::pt(18.0).unwrap();
            let twin = right || matches!(kind, Kind::TwinTitle);
            let build = |width| {
                let mut f = axis_figure(kind, right, axis, width, text, size);
                f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
                    .unwrap()
                    .xlim(0.0, 1.0)
                    .unwrap()
                    .xticks(&[0.5])
                    .unwrap()
                    .yticks_labeled(&[0.0, 1.0], vec!["Wide ticks".into(); 2])
                    .unwrap();
                if twin {
                    f.twin_axes_at_mut(GridIndex::At(0), GridIndex::At(0))
                        .unwrap()
                        .yticks_labeled(&[0.0, 1.0], vec!["Right ticks".into(); 2])
                        .unwrap();
                }
                f
            };
            let index = usize::from(right && axis == "y");
            let layouts =
                crate::pipeline::try_plan_panels(&build(864.0), FontLibrary::global()).unwrap();
            let layout = &layouts[index].1;
            let label = if axis == "x" {
                layout.xlabel.as_ref()
            } else {
                layout.ylabel.as_ref()
            }
            .unwrap();
            let room = if axis == "y" && !right {
                layout.plot.right() - label.x
            } else {
                layout.plot.width
            };
            let scale = kind.cols() as f64 / kind.span() as f64;
            let width = 864.0 - (room - label.width()) * scale;
            for deficit in [0.0, inset::CONTAINMENT_TOLERANCE / 2.0] {
                let f = build(width - deficit * scale);
                f.to_scene(FontLibrary::global()).unwrap();
                let layouts = crate::pipeline::try_plan_panels(&f, FontLibrary::global()).unwrap();
                let at = &layouts[index].1;
                let label = if axis == "x" { &at.xlabel } else { &at.ylabel }
                    .as_ref()
                    .unwrap();
                assert!(!at.y_tick_labels.is_empty());
                let left = if axis == "y" && !right {
                    at.y_tick_labels
                        .iter()
                        .map(|tick| tick.first().x)
                        .fold(f64::INFINITY, f64::min)
                } else {
                    at.plot.x
                };
                assert!(label.x >= left - inset::CONTAINMENT_TOLERANCE);
                assert!(label.x + label.width() <= at.plot.right() + inset::CONTAINMENT_TOLERANCE);
                if axis == "y" && !right {
                    assert!(label.width() > at.plot.width);
                }
            }
            let error = build(width - 0.25 * scale)
                .to_scene(FontLibrary::global())
                .unwrap_err();
            let side = if axis == "y" && twin {
                if right { "right " } else { "left " }
            } else {
                ""
            };
            assert!(
                error
                    .to_string()
                    .contains(&format!("the {side}{axis} axis label {}", quoted(text))),
                "{error}"
            );
            assert_eq!(error.hint(), Some(POINTS_HINT));
        }
    }
}

#[test]
fn figure_titles_are_checked_before_panel_errors() {
    let mut f = figure(Kind::AxesTitle, 432.0, TEXT, SizeSpec::Auto, Align::Left);
    f.title(TEXT).subtitle(TEXT);
    let error = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(error.to_string().contains("the title"));
    assert!(error.to_string().contains("the figure has"));
    f.title("");
    let error = f.to_scene(FontLibrary::global()).unwrap_err();
    assert!(error.to_string().contains("the subtitle"));
    assert!(error.to_string().contains("the figure has"));
}

#[test]
fn axis_title_error_quotes_escaped_text() {
    let text = "\"quoted\" \\ axis";
    let size = SizeSpec::pt(18.0).unwrap();
    let width = text_width(Kind::AxesSubtitle, text, size) + 2.0 * PADDING - 1.0;
    for axis in ["x", "y"] {
        let error = axis_figure(Kind::AxesTitle, false, axis, width, text, size)
            .to_scene(FontLibrary::global())
            .unwrap_err();
        assert!(error.to_string().contains(&quoted(text)));
    }
}

#[test]
fn reported_grid_width_keeps_room_for_axis_titles() {
    for kind in [Kind::GridTitle, Kind::SpanningTitle, Kind::TwinTitle] {
        for axis in ["x", "y", "right", "both"] {
            if matches!(axis, "right" | "both") && !matches!(kind, Kind::TwinTitle) {
                continue;
            }
            let build = |width| {
                let mut f = figure(kind, width, "", SizeSpec::Auto, Align::Left);
                let mut ax = f.axes_at_mut(GridIndex::At(0), GridIndex::At(0)).unwrap();
                if axis == "x" {
                    ax.xlabel("Distance travelled");
                } else if axis != "right" {
                    ax.ylabel("Height above ground");
                }
                if matches!(kind, Kind::TwinTitle) {
                    let mut right = f
                        .twin_axes_at_mut(GridIndex::At(0), GridIndex::At(0))
                        .unwrap();
                    right
                        .yticks_labeled(&[0.0, 1.0], vec!["Right ticks".into(); 2])
                        .unwrap();
                    if matches!(axis, "right" | "both") {
                        right.ylabel("Right height");
                    }
                }
                f
            };
            let error = build(1.0)
                .to_scene(FontLibrary::global())
                .unwrap_err()
                .to_string();
            let needed: f64 = error
                .split_once("at least ")
                .unwrap()
                .1
                .split_once(" pt")
                .unwrap()
                .0
                .parse()
                .unwrap();
            build(needed).to_scene(FontLibrary::global()).unwrap();
        }
    }
}

#[test]
fn reported_grid_x_tick_width_keeps_room_for_axis_titles() {
    for kind in [Kind::GridTitle, Kind::SpanningTitle, Kind::TwinTitle] {
        for axis in ["x", "y"] {
            let width = match kind {
                Kind::TwinTitle => 200.0,
                Kind::GridTitle => 260.0,
                _ => 280.0,
            };
            let build = |tick: &str| {
                let mut f = figure(kind, width, "", SizeSpec::Auto, Align::Left);
                let mut ax = f.axes_at_mut(GridIndex::At(0), GridIndex::At(0)).unwrap();
                ax.xlim(0.0, 1.0)
                    .unwrap()
                    .xticks_labeled(&[1.0], vec![tick.into()])
                    .unwrap()
                    .yticks_labeled(&[0.0, 1.0], vec!["W".repeat(19); 2])
                    .unwrap();
                if axis == "x" {
                    ax.xlabel("WWW");
                } else {
                    ax.ylabel(&"W".repeat(23));
                }
                f
            };
            let sample =
                crate::pipeline::try_plan_panels(&build("W"), FontLibrary::global()).unwrap();
            let char_width = sample[0].1.x_tick_labels[0].width();
            let error = build(&"W".repeat(50))
                .to_scene(FontLibrary::global())
                .unwrap_err()
                .to_string();
            let fits: f64 = error
                .split_once("at most ")
                .unwrap()
                .1
                .split_once(" pt fits")
                .unwrap()
                .0
                .parse()
                .unwrap();
            let count = (fits / char_width).floor() as usize;
            assert!(count > 0, "{error}");
            build(&"W".repeat(count))
                .to_scene(FontLibrary::global())
                .unwrap();
        }
    }
}

#[test]
fn reported_grid_y_tick_width_keeps_room_for_axis_titles() {
    let text = "W".repeat(12);
    let size = SizeSpec::pt(12.0).unwrap();
    let label_width = text_width(Kind::AxesSubtitle, &text, size);
    for kind in [Kind::TwinTitle, Kind::GridTitle] {
        let cols = kind.cols() as f64;
        let width = 2.0 * PADDING + 24.0 * (cols - 1.0) + cols * (label_width + 30.0);
        let build = |tick_size| {
            let mut f = figure(kind, width, "", SizeSpec::Auto, Align::Left);
            f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
                .unwrap()
                .ylabel_sized(&text, size)
                .yticks_labeled(&[0.0, 1.0], vec!["W".repeat(100); 2])
                .unwrap()
                .ytick_labels(SizeSpec::pt(tick_size).unwrap())
                .unwrap();
            f
        };
        let sample = crate::pipeline::try_plan_panels(&build(0.1), FontLibrary::global()).unwrap();
        let tick_width = sample[0].1.y_tick_labels[0].width();
        let error = build(12.0)
            .to_scene(FontLibrary::global())
            .unwrap_err()
            .to_string();
        let fits: f64 = error
            .split_once("at most ")
            .unwrap()
            .1
            .split_once(" pt leaves room")
            .unwrap()
            .0
            .parse()
            .unwrap();
        assert!(fits > 0.0, "{error}");
        // The suggestion must preserve the label's width even without the tick column.
        assert!(fits <= cols * 30.0 - Y_LABEL_GAP, "{error}");
        let fitted = build(0.1 * fits / tick_width);
        fitted.to_scene(FontLibrary::global()).unwrap();
        let layout = crate::pipeline::try_plan_panels(&fitted, FontLibrary::global()).unwrap();
        // Text sizes and advances are f32, so use their precision at the requested width.
        assert!((layout[0].1.y_tick_labels[0].width() - fits).abs() < 1e-4);
        assert!(layout[0].1.plot.width + 1e-4 >= label_width);
    }
}

#[test]
fn grid_y_tick_error_names_axis_title_when_shorter_ticks_cannot_help() {
    for kind in [Kind::TwinTitle, Kind::GridTitle] {
        for size in [SizeSpec::Auto, SizeSpec::pt(12.0).unwrap()] {
            let text = "W".repeat(30);
            let label_width = text_width(Kind::AxesSubtitle, &text, size);
            let cols = kind.cols() as f64;
            let width = 2.0 * PADDING + 24.0 * (cols - 1.0) + label_width - 10.0;
            let mut f = figure(kind, width, "", SizeSpec::Auto, Align::Left);
            f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
                .unwrap()
                .ylabel_sized(&text, size)
                .yticks_labeled(&[0.0, 1.0], vec!["W".repeat(100); 2])
                .unwrap();
            let error = f.to_scene(FontLibrary::global()).unwrap_err();
            let side = if matches!(kind, Kind::TwinTitle) {
                "left "
            } else {
                ""
            };
            let prefix = if cols > 1.0 { "row 0, col 0: " } else { "" };
            let room = label_width - 10.0 - (cols - 1.0) * MIN_PLOT;
            assert_eq!(
                error.to_string(),
                format!(
                    "Figure.to_scene(): {prefix}the {side}y axis label {} is {} pt wide; the panel has {} pt.",
                    quoted(&text),
                    label_width.ceil(),
                    room.floor(),
                )
            );
            assert_eq!(error.hint(), Some(panel::text_size_hint(size)));
        }
    }
}

#[test]
fn reported_x_tick_width_keeps_room_for_axis_titles() {
    for axis in ["x", "y"] {
        let build = |tick: &str| {
            let mut f = Figure::new(2.0, 4.0, Theme::midnight()).unwrap();
            let mut ax = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
            ax.xlim(0.0, 1.0)
                .unwrap()
                .xticks_labeled(&[1.0], vec![tick.into()])
                .unwrap()
                .yticks_labeled(&[0.0, 1.0], vec!["W".repeat(19); 2])
                .unwrap();
            if axis == "x" {
                ax.xlabel("WWW");
            } else {
                ax.ylabel(&"W".repeat(23));
            }
            f
        };
        let sample = crate::pipeline::try_plan_panels(&build("W"), FontLibrary::global()).unwrap();
        let char_width = sample[0].1.x_tick_labels[0].width();
        let error = build(&"W".repeat(20))
            .to_scene(FontLibrary::global())
            .unwrap_err();
        let message = error.to_string();
        let fits: f64 = message
            .split_once("at most ")
            .unwrap()
            .1
            .split_once(" pt fits")
            .unwrap()
            .0
            .parse()
            .unwrap();
        let count = (fits / char_width).floor() as usize;
        assert!(count > 0, "{message}");
        build(&"W".repeat(count))
            .to_scene(FontLibrary::global())
            .unwrap();
    }
}

/// The layouts of `figure`, in panel order.
fn planned(figure: &Figure) -> Vec<Layout> {
    crate::pipeline::try_plan_panels(figure, FontLibrary::global())
        .unwrap()
        .into_iter()
        .map(|(_, layout)| layout)
        .collect()
}

/// The figure header of `figure`, without a merged legend.
fn figure_header(figure: &Figure) -> FigureHeader {
    let fonts = figure_fonts(FontLibrary::global(), figure.theme()).unwrap();
    FigureHeader::measure(figure, &MeasuredLegend::default(), &fonts).unwrap()
}

/// The size, in points, `block` is set at.
fn size_of(block: Option<&TextBlock>) -> f32 {
    block.unwrap().first().style.size
}

#[test]
fn a_title_too_wide_shrinks_to_the_largest_half_point_that_fits() {
    let text = "Quarterly revenue by region and sales channel";
    let natural = text_width(Kind::AxesTitle, text, SizeSpec::Auto);
    for share in [0.95, 0.8, 0.62] {
        let room = (natural * share).floor();
        let f = figure(
            Kind::AxesTitle,
            room + 2.0 * PADDING,
            text,
            SizeSpec::Auto,
            Align::Left,
        );
        let layout = &planned(&f)[0];
        let title = layout.title.as_ref().unwrap();
        let size = title.first().style.size;
        assert!((8.5..15.0).contains(&size), "{size}");
        assert_eq!((size * 2.0).fract(), 0.0, "{size}");
        assert!(title.width() <= room + inset::CONTAINMENT_TOLERANCE);
        assert!(
            text_width(
                Kind::AxesTitle,
                text,
                SizeSpec::pt(f64::from(size) + 0.5).unwrap()
            ) > room
        );
        // Same face and weight; the header takes the height of the size used.
        assert_eq!(title.first().style.face, TITLE_FACE);
        assert_eq!(title.first().style.weight, Weight::SemiBold);
        let line = TITLE_LINE * f64::from(size);
        assert!((layout.header_bottom - (PADDING + line + HEADER_GAP)).abs() < 1e-9);
        // The plot starts that much higher than under a title at the theme's size.
        let mut fixed = f.clone();
        fixed.single_axes_mut().title_sized("Short", SizeSpec::Auto);
        let full = &planned(&fixed)[0];
        assert_eq!(size_of(full.title.as_ref()), 15.0);
        let saved = TITLE_LINE * (15.0 - f64::from(size));
        assert!((full.plot.y - layout.plot.y - saved).abs() < 1e-9);
    }
}

#[test]
fn a_title_that_fits_keeps_the_theme_size() {
    let text = "Station output";
    let width = text_width(Kind::AxesTitle, text, SizeSpec::Auto) + 2.0 * PADDING;
    for kind in [Kind::AxesTitle, Kind::GridTitle, Kind::PieTitle] {
        let f = figure(
            kind,
            width * kind.cols() as f64,
            text,
            SizeSpec::Auto,
            Align::Left,
        );
        assert_eq!(size_of(planned(&f)[0].title.as_ref()), 15.0);
    }
}

#[test]
fn a_title_with_an_explicit_size_keeps_it() {
    let text = "Quarterly revenue by region and sales channel";
    let size = SizeSpec::pt(15.0).unwrap();
    let room = (text_width(Kind::AxesTitle, text, size) * 0.8).floor();
    let error = figure(
        Kind::AxesTitle,
        room + 2.0 * PADDING,
        text,
        size,
        Align::Left,
    )
    .to_scene(FontLibrary::global())
    .unwrap_err();
    assert_eq!(error.hint(), Some(POINTS_HINT));
    assert!(
        error
            .to_string()
            .ends_with(&format!("the panel has {room} pt."))
    );
}

#[test]
fn a_title_that_fits_only_at_its_floor_is_set_there() {
    let text = "Quarterly revenue by region and sales channel";
    let floor = text_width(Kind::AxesTitle, text, SizeSpec::pt(8.5).unwrap());
    let f = figure(
        Kind::AxesTitle,
        floor.ceil() + 2.0 * PADDING,
        text,
        SizeSpec::Auto,
        Align::Left,
    );
    assert_eq!(size_of(planned(&f)[0].title.as_ref()), 8.5);
    let error = figure(
        Kind::AxesTitle,
        floor.ceil() - 1.0 + 2.0 * PADDING,
        text,
        SizeSpec::Auto,
        Align::Left,
    )
    .to_scene(FontLibrary::global())
    .unwrap_err();
    assert!(
        error.to_string().ends_with(&format!(
            "is {} pt wide at 8.5 pt, the smallest size it shrinks to; the panel has {} pt.",
            floor.ceil(),
            floor.ceil() - 1.0
        )),
        "{error}"
    );
    assert_eq!(error.hint(), Some(SHRUNK_TITLE_HINT));
}

#[test]
fn each_line_of_a_title_fits_at_one_size() {
    let long = "Quarterly revenue by region and sales channel";
    let natural = text_width(Kind::AxesTitle, long, SizeSpec::Auto);
    let room = (natural * 0.7).floor();
    for text in [format!("Revenue\n{long}"), format!("{long}\nRevenue")] {
        let f = figure(
            Kind::AxesTitle,
            room + 2.0 * PADDING,
            &text,
            SizeSpec::Auto,
            Align::Left,
        );
        let layout = &planned(&f)[0];
        let title = layout.title.as_ref().unwrap();
        let size = title.first().style.size;
        assert!(size < 15.0);
        assert!(title.lines().all(|line| line.style.size == size));
        assert!(title.width() <= room + inset::CONTAINMENT_TOLERANCE);
        assert!(
            text_width(
                Kind::AxesTitle,
                long,
                SizeSpec::pt(f64::from(size) + 0.5).unwrap()
            ) > room
        );
        assert!((title.pitch() - LINE_PITCH * f64::from(size)).abs() < 1e-9);
    }
}

#[test]
fn the_panel_titles_of_a_figure_share_the_smallest_size_any_needs() {
    let long = "Share of days by weather type, season, region and year";
    for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
        let natural = theme.title_size();
        let mut f = Figure::with_grid(10.0, 6.0, theme.clone(), Grid::new(2, 2).unwrap()).unwrap();
        f.title("Weather");
        let titles = [
            (0, 0, long, SizeSpec::Auto),
            (0, 1, "Rain", SizeSpec::Auto),
            (1, 0, "Wind", SizeSpec::pt(13.0).unwrap()),
        ];
        for (row, col, text, size) in titles {
            f.add_axes_at(
                Placement {
                    row: GridIndex::At(row),
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap()
            .title_sized(text, size)
            .subtitle("Daily");
        }
        f.add_pie_axes_at(Placement {
            row: GridIndex::At(1),
            col: GridIndex::At(1),
            ..Placement::default()
        })
        .unwrap()
        .title("Snow")
        .pie(vec!["A".into()], &[1.0], PieOptions::default())
        .unwrap();
        let layouts = planned(&f);
        let shrunk = size_of(layouts[0].title.as_ref());
        assert!((theme.body_size()..natural).contains(&shrunk), "{shrunk}");
        // Every panel title without a size takes it, the pie's too; an explicit size stays.
        assert_eq!(size_of(layouts[1].title.as_ref()), shrunk);
        assert_eq!(size_of(layouts[2].title.as_ref()), 13.0);
        assert_eq!(size_of(layouts[3].title.as_ref()), shrunk);
        // Subtitles stay at the axis label size; the panels of a row share their header band.
        for layout in &layouts[..3] {
            assert_eq!(size_of(layout.subtitle.as_ref()), theme.body_size());
        }
        assert_eq!(layouts[0].plot.y, layouts[1].plot.y);
        // The figure title fits and keeps its own size.
        let header = figure_header(&f);
        assert_eq!(
            size_of(header.title.as_ref()),
            natural * panel::FIGURE_TITLE_SCALE
        );
    }
}

#[test]
fn the_figure_title_shrinks_on_its_own_down_to_the_panel_title_size() {
    let text = "Daily rainfall and temperature";
    let theme = Theme::pigment();
    let figure_size = theme.title_size() * panel::FIGURE_TITLE_SCALE;
    let natural = text_width(Kind::FigureTitle, text, SizeSpec::Auto);
    let room = (natural * 0.8).floor();
    let mut f = Figure::new((room + 2.0 * PADDING) / 72.0, 4.0, theme.clone()).unwrap();
    f.title(text);
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Rain");
    let header = figure_header(&f);
    let title = header.title.as_ref().unwrap();
    let size = title.first().style.size;
    assert!((theme.title_size()..figure_size).contains(&size), "{size}");
    assert_eq!((size * 2.0).fract(), 0.0);
    assert!(title.width() <= room + inset::CONTAINMENT_TOLERANCE);
    assert!(
        text_width(
            Kind::FigureTitle,
            text,
            SizeSpec::pt(f64::from(size) + 0.5).unwrap()
        ) > room
    );
    // The grid starts below the shrunk header; the panel title keeps its size.
    let line = TITLE_LINE * f64::from(size);
    assert!((header.bottom - (PADDING + line + HEADER_GAP)).abs() < 1e-9);
    assert_eq!(size_of(planned(&f)[0].title.as_ref()), theme.title_size());
}

#[test]
fn shrunk_titles_over_plots_with_equal_aspect_keep_their_room() {
    let long = "A title wider than the plot below it and its cell";
    let mut f = Figure::with_grid(7.0, 3.0, Theme::pigment(), Grid::new(1, 2).unwrap()).unwrap();
    for col in [0, 1] {
        let mut axes = f
            .add_axes_at(
                Placement {
                    col: GridIndex::At(col),
                    ..Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        axes.xlim(0.0, 1.0)
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap()
            .title(long);
        if col == 1 {
            axes.equal_aspect().unwrap();
        }
    }
    f.to_scene(FontLibrary::global()).unwrap();
    let layouts = planned(&f);
    let size = size_of(layouts[0].title.as_ref());
    assert!(size < 15.0);
    assert_eq!(size_of(layouts[1].title.as_ref()), size);
    for layout in &layouts {
        let title = layout.title.as_ref().unwrap();
        assert!(title.first().y < layout.plot.y);
        assert!(layout.header_bottom <= layout.plot.y + 1e-9);
    }
}

const QUARTERLY: &str = "Quarterly revenue by region and sales channel";

/// A `width` in by 1 in figure of `cols` panels without ticks, each titled with three lines of
/// [`QUARTERLY`] at `size`.
fn short_titled(width: f64, cols: i64, size: SizeSpec) -> Figure {
    let title = [QUARTERLY; 3].join("\n");
    let mut f =
        Figure::with_grid(width, 1.0, Theme::pigment(), Grid::new(1, cols).unwrap()).unwrap();
    for col in 0..cols {
        f.add_axes_at(
            Placement {
                col: GridIndex::At(col),
                ..Placement::default()
            },
            ScaleKind::Linear,
            ScaleKind::Linear,
        )
        .unwrap()
        .xticks(&[])
        .unwrap()
        .yticks(&[])
        .unwrap()
        .title_sized(&title, size);
    }
    f
}

#[test]
fn titles_shrink_before_the_height_is_checked() {
    // At its own size the title of three lines is too tall for a 1 in figure, but the size it
    // shrinks to for the width leaves the plot room: the height is checked for that size.
    for (width, cols) in [(3.5, 1), (7.0, 2)] {
        let f = short_titled(width, cols, SizeSpec::Auto);
        f.to_scene(FontLibrary::global()).unwrap();
        let layouts = planned(&f);
        let size = size_of(layouts[0].title.as_ref());
        assert!(size < 15.0, "{size}");
        for layout in &layouts {
            assert_eq!(size_of(layout.title.as_ref()), size);
            assert!(layout.header_bottom <= layout.plot.y + 1e-9);
        }
        // The same size given explicitly lays out the same.
        let given = short_titled(width, cols, SizeSpec::pt(f64::from(size)).unwrap());
        let explicit = planned(&given);
        assert_eq!(layouts[0].plot, explicit[0].plot);
        // At the theme's size it does not fit.
        let error = short_titled(width, cols, SizeSpec::times(1.0).unwrap())
            .to_scene(FontLibrary::global())
            .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("tall") || message.contains("vertically"),
            "{message}"
        );
    }
}

/// A 3 in by 1.3 in figure whose wide first x tick label insets the plot, titled with three
/// lines of 18 "M", its y ticks automatic or at 0 and 1, the title at `size`.
fn inset_title(explicit_y: bool, size: SizeSpec) -> Figure {
    let mut f = Figure::new(3.0, 1.3, Theme::pigment()).unwrap();
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    axes.xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap()
        .xticks_labeled(&[0.0, 1.0], vec!["W".repeat(10), "1".into()])
        .unwrap()
        .title_sized(&vec!["M".repeat(18); 3].join("\n"), size);
    if explicit_y {
        axes.yticks(&[0.0, 1.0]).unwrap();
    }
    f
}

#[test]
fn guessed_ticks_do_not_refuse_a_height_the_fitted_title_leaves() {
    // The first ticks are guessed for a plot size; the title fits other widths than the final
    // ticks leave, so the height is not refused before the title is fitted to those.
    for explicit_y in [false, true] {
        let f = inset_title(explicit_y, SizeSpec::Auto);
        f.to_scene(FontLibrary::global()).unwrap();
        let size = size_of(planned(&f)[0].title.as_ref());
        assert!(size < 15.0, "{size}");
        inset_title(explicit_y, SizeSpec::pt(f64::from(size)).unwrap())
            .to_scene(FontLibrary::global())
            .unwrap();
    }
    inset_title(false, SizeSpec::pt(10.5).unwrap())
        .to_scene(FontLibrary::global())
        .unwrap();
}
