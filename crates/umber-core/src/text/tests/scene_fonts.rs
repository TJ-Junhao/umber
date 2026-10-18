//! The fonts of one scene: a face is the font at its index, laid out through the library.

use crate::Error;
use crate::fonts::{BundledFont, Font, Weight};
use crate::geometry::Point;
use crate::text::{FaceId, FontLibrary, TextStyle};

fn style(face: usize) -> TextStyle {
    TextStyle {
        face: FaceId::new(face),
        weight: Weight::Regular,
        size: 10.0,
        tabular_figures: false,
    }
}

#[test]
fn a_face_is_the_font_at_its_index() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let fonts = lib
        .with_figure_fonts(&[Font::GEIST, Font::SOURCE_SERIF_4])
        .unwrap();
    let alone = lib.with_figure_fonts(&[Font::SOURCE_SERIF_4]).unwrap();
    let text = "Quarterly revenue";
    let serif = fonts.layout(text, &style(1)).unwrap();
    assert_eq!(
        serif.width(),
        alone.layout(text, &style(0)).unwrap().width()
    );
    assert_eq!(
        serif.outline(Point::default()),
        alone
            .layout(text, &style(0))
            .unwrap()
            .outline(Point::default())
    );
    assert_ne!(
        serif.width(),
        fonts.layout(text, &style(0)).unwrap().width()
    );
    assert_eq!(fonts.fonts(), [Font::GEIST, Font::SOURCE_SERIF_4]);
    assert_eq!(fonts.font(FaceId::new(1)), Some(&Font::SOURCE_SERIF_4));
}

#[test]
fn every_bundled_font_lays_out_in_its_own_family() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let all: Vec<Font> = BundledFont::ALL.into_iter().map(Font::Bundled).collect();
    let fonts = lib.with_figure_fonts(&all).unwrap();
    let outlines: Vec<_> = (0..all.len())
        .map(|face| {
            fonts
                .layout("Ag", &style(face))
                .unwrap()
                .outline(Point::default())
        })
        .collect();
    for (i, outline) in outlines.iter().enumerate() {
        assert!(!outline.is_empty(), "{:?}", all[i]);
        assert_eq!(
            outlines.iter().filter(|other| *other == outline).count(),
            1,
            "{:?} draws like another family",
            all[i]
        );
    }
}

#[test]
fn a_font_listed_twice_lays_out_the_same_under_both_faces() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let fonts = lib.with_figure_fonts(&[Font::INTER, Font::INTER]).unwrap();
    let first = fonts.layout("Signal 2026", &style(0)).unwrap();
    let second = fonts.layout("Signal 2026", &style(1)).unwrap();
    assert_eq!(first.width(), second.width());
    assert_eq!(
        first.outline(Point::default()),
        second.outline(Point::default())
    );
}

#[test]
fn a_face_outside_the_fonts_is_a_usage_error() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let fonts = lib.with_figure_fonts(&[Font::INTER]).unwrap();
    let err = fonts.layout("Signal", &style(1)).unwrap_err();
    assert!(matches!(err, Error::Usage { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "SceneFonts.layout(): the text style names face 1, but the last face is 0; a face must \
         be the index of one of the fonts."
    );
    assert_eq!(
        err.hint(),
        Some("name a face by the index of a font given to with_figure_fonts.")
    );
    assert_eq!(fonts.font(FaceId::new(1)), None);
    let none = lib.with_figure_fonts(&[]).unwrap();
    assert!(none.fonts().is_empty());
    assert!(none.layout("Signal", &style(0)).is_err());
}

#[test]
fn a_font_without_a_face_for_the_weight_names_the_text_and_the_font() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let fonts = lib
        .with_figure_fonts(&[Font::INTER, Font::GEIST])
        .unwrap()
        .in_context("Figure.to_png()")
        .without_weight(FaceId::new(1), Weight::SemiBold);
    let bold = TextStyle {
        weight: Weight::SemiBold,
        ..style(1)
    };
    let err = fonts.layout("Quarterly revenue", &bold).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        "Figure.to_png(): the text \"Quarterly revenue\" needs the bold face of Geist, which the \
         font does not have; text can only be set in a weight its font has."
    );
    assert_eq!(
        err.hint(),
        Some(
            "pass bold= to Font.from_file, choose a family with a bold face, or pass bold_fallback=True."
        )
    );
    // The same font still sets regular text, and the other font bold text.
    assert!(fonts.layout("Quarterly revenue", &style(1)).is_ok());
    let inter_bold = TextStyle {
        face: FaceId::new(0),
        ..bold
    };
    assert!(fonts.layout("Quarterly revenue", &inter_bold).is_ok());
}

#[test]
fn custom_run_keys_distinguish_scene_faces_and_weights() {
    let font = |family| {
        Font::from_files(
            crate::fonts::font_bytes(family, Weight::Regular).to_vec(),
            Some(crate::fonts::font_bytes(family, Weight::SemiBold).to_vec()),
            "file".into(),
        )
        .unwrap()
    };
    let lib = FontLibrary::with_fallbacks(vec![]);
    let fonts = lib
        .with_figure_fonts(&[font(BundledFont::Inter), font(BundledFont::Geist)])
        .unwrap();
    let regular = fonts.layout("Ab", &style(0)).unwrap();
    let bold = fonts
        .layout(
            "Ab",
            &TextStyle {
                weight: Weight::SemiBold,
                ..style(0)
            },
        )
        .unwrap();
    let other = fonts.layout("Ab", &style(1)).unwrap();
    assert_ne!(regular.runs()[0].font(), bold.runs()[0].font());
    assert_ne!(regular.runs()[0].font(), other.runs()[0].font());
    assert_eq!(
        regular.runs()[0].font(),
        fonts.layout("Cd", &style(0)).unwrap().runs()[0].font()
    );
}

#[test]
fn file_regular_and_bold_share_identical_bytes() {
    let bytes = crate::fonts::font_bytes(BundledFont::Inter, Weight::Regular).to_vec();
    let font = Font::from_files(bytes.clone(), Some(bytes), "Inter".into()).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &font.owned_face(Weight::Regular).unwrap().bytes,
        &font.owned_face(Weight::SemiBold).unwrap().bytes,
    ));
}

#[test]
fn face_ids_are_plain_indices() {
    assert_eq!(FaceId::new(3).index(), 3);
    assert_eq!(FaceId::new(0), FaceId::new(0));
    assert_ne!(FaceId::new(0), FaceId::new(1));
}
