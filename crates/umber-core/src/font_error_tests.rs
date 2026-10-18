//! A font without a face for the weight of some text fails the render that sets that text,
//! naming the text and the font; renders that set no text in that weight succeed.

use super::*;
use crate::layout::{BODY_FACE, TITLE_FACE};
use crate::{AxisData, LegendStyle, LineOptions, ScaleKind, Weight};

/// A pigment figure with one line labelled `label`, the panel title `title` and the figure
/// title `figure_title`; empty texts are left out.
fn figure(theme: Theme, title: &str, figure_title: &str, label: &str) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    if !figure_title.is_empty() {
        f.title(figure_title);
    }
    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
    if !title.is_empty() {
        axes.title(title);
    }
    axes.line(
        AxisData::Numbers(vec![0.0, 1.0, 2.0]),
        &[1.0, 3.0, 2.0],
        LineOptions {
            label: label.into(),
            ..LineOptions::default()
        },
    )
    .unwrap();
    f
}

/// The scene of `figure` with the bold face of `face` treated as missing, rendered as
/// `Figure.to_png()` renders.
fn render_without_bold(figure: &Figure, face: crate::FaceId) -> Result<Scene> {
    let fonts = crate::test_support::figure_fonts(figure.theme())
        .in_context("Figure.to_png()")
        .without_weight(face, Weight::SemiBold);
    scene_with_labels(
        figure,
        &fonts,
        "Figure.to_png()",
        &layout::LabelCache::new(&fonts),
    )
}

#[test]
fn a_panel_title_without_a_bold_face_fails_the_render_naming_the_title_and_font() {
    let f = figure(Theme::pigment(), "Quarterly revenue", "", "");
    let err = render_without_bold(&f, TITLE_FACE).unwrap_err();
    assert!(matches!(err, crate::Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_png(): the text \"Quarterly revenue\" needs the bold face of Source Serif 4, \
         which the font does not have; text can only be set in a weight its font has."
    );
    assert_eq!(
        err.hint(),
        Some(
            "pass bold= to Font.from_file, choose a family with a bold face, or pass bold_fallback=True."
        )
    );
}

#[test]
fn a_figure_title_without_a_bold_face_fails_the_render_naming_the_title() {
    let f = figure(Theme::graphite(), "", "Annual report", "");
    let err = render_without_bold(&f, TITLE_FACE).unwrap_err();
    assert!(
        err.to_string().starts_with(
            "Figure.to_png(): the text \"Annual report\" needs the bold face of Geist"
        ),
        "{err}"
    );
}

#[test]
fn a_direct_label_without_a_bold_body_face_fails_the_render_naming_the_label() {
    let theme = Theme::pigment()
        .with_legend_style(LegendStyle::Direct)
        .unwrap();
    let f = figure(theme, "", "", "North");
    let err = render_without_bold(&f, BODY_FACE).unwrap_err();
    assert!(
        err.to_string().starts_with(
            "Figure.to_png(): the text \"North\" needs the bold face of IBM Plex Sans"
        ),
        "{err}"
    );
}

#[test]
fn a_font_without_a_bold_face_renders_text_that_is_not_bold() {
    // Subtitle, axis labels, tick labels and a header legend are regular text.
    let mut f = figure(Theme::pigment(), "", "", "North");
    f.subtitle("Regular text only");
    f.single_axes_mut()
        .subtitle("Panel subtitle")
        .xlabel("Day")
        .ylabel("Count");
    for face in [TITLE_FACE, BODY_FACE] {
        let scene = render_without_bold(&f, face).unwrap();
        assert_eq!(scene, scene_of(&f), "{face:?}");
    }
}

/// The scene of `figure` as `Figure::to_scene` gives it.
fn scene_of(figure: &Figure) -> Scene {
    figure.to_scene(crate::FontLibrary::global()).unwrap()
}
