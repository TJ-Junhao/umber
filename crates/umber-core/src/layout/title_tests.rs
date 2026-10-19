//! Titles must fit the same horizontal box in which they are aligned.

use super::*;
use crate::{Align, Figure, Grid, GridIndex, PieOptions, Placement, ScaleKind};

const TEXT: &str = "A very long title that certainly cannot fit in six inches of width at all, even as a subtitle: WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";
const SCALE_HINT: &str =
    "make the figure wider or lower the font scale with Theme.with_font_scale().";
const POINTS_HINT: &str = "make the figure wider or pass a smaller size=.";

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
            assert_eq!(
                error.to_string(),
                format!(
                    "Figure.to_scene(): {prefix}the {name} {} is {} pt wide; the {container} has {} pt.",
                    quoted(TEXT),
                    text_width(kind, TEXT, size).ceil(),
                    kind.room(432.0).floor(),
                )
            );
            assert_eq!(error.parameter(), None);
            assert_eq!(
                error.hint(),
                Some(if matches!(size, SizeSpec::Points(_)) {
                    POINTS_HINT
                } else {
                    SCALE_HINT
                })
            );
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
            assert_eq!(
                error.to_string(),
                format!(
                    "Figure.to_scene(): {prefix}the {axis} axis label is {} pt wide; the panel has {} pt.",
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
