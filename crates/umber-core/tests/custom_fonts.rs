//! File fonts keep their bytes, validate before use, and resolve through the scene font set.

#![allow(clippy::unwrap_used)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use umber_core::fonts::font_bytes;
use umber_core::{BundledFont, Error, FaceId, Font, FontLibrary, TextStyle, Weight};

fn file_font(bold: bool, name: &str) -> Font {
    Font::from_files(
        font_bytes(BundledFont::Inter, Weight::Regular).to_vec(),
        bold.then(|| font_bytes(BundledFont::Inter, Weight::SemiBold).to_vec()),
        name.to_owned(),
    )
    .unwrap()
}

#[test]
fn from_file_regular_and_bold() {
    let font = file_font(true, "one.ttf");
    let renamed = file_font(true, "two.ttf");
    assert_eq!(font, renamed);
    assert_eq!(font.family_name(), "Inter");
    assert!(font.has_face(Weight::Regular));
    assert!(font.has_face(Weight::SemiBold));
    assert!(!file_font(false, "regular.ttf").has_face(Weight::SemiBold));
    assert_ne!(font, Font::INTER);
    assert_ne!(font, file_font(false, "regular.ttf"));
    let hash = |font: &Font| {
        let mut hasher = DefaultHasher::new();
        font.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(&font), hash(&renamed));
}

#[test]
fn from_file_errors_exact() {
    for bad in [
        vec![],
        b"not a font".to_vec(),
        vec![0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ] {
        let err = Font::from_files(bad.clone(), None, "bad.ttf".into()).unwrap_err();
        assert!(matches!(err, Error::Data { .. }));
        assert_eq!(err.parameter(), Some("regular"));
        assert_eq!(err.hint(), Some("pass a .ttf or .otf file."));
        let err = Font::from_files(
            font_bytes(BundledFont::Inter, Weight::Regular).to_vec(),
            Some(bad),
            "bad.ttf".into(),
        )
        .unwrap_err();
        assert_eq!(err.parameter(), Some("bold"));
    }
}

#[test]
fn from_file_rejects_unsorted_table_directory() {
    let mut bytes = font_bytes(BundledFont::Inter, Weight::Regular).to_vec();
    for offset in 0..16 {
        bytes.swap(12 + offset, 28 + offset);
    }
    let err = Font::from_files(bytes, None, "unsorted.ttf".into()).unwrap_err();
    assert!(matches!(err, Error::Data { .. }));
    assert_eq!(err.parameter(), Some("regular"));
    assert_eq!(err.hint(), Some("pass a .ttf or .otf file."));
}

#[test]
fn ttc_rejected() {
    let err = Font::from_files(b"ttcf\0\0\0\0".to_vec(), None, "a.ttf".into()).unwrap_err();
    assert_eq!(
        err.hint(),
        Some("extract one face from the collection, or use Font.system().")
    );
}

#[test]
fn file_faces_match_bundled_metrics_and_outlines() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let fonts = lib
        .with_figure_fonts(&[file_font(true, "inter.ttf"), Font::INTER])
        .unwrap();
    for weight in Weight::ALL {
        let style = TextStyle {
            face: FaceId::new(0),
            weight,
            size: 12.0,
            tabular_figures: true,
        };
        let actual = fonts.layout("Revenue −1,234", &style).unwrap();
        let expected = fonts
            .layout(
                "Revenue −1,234",
                &TextStyle {
                    face: FaceId::new(1),
                    ..style
                },
            )
            .unwrap();
        assert_eq!(actual.width(), expected.width());
        assert_eq!(actual.ascent(), expected.ascent());
        assert_eq!(actual.descent(), expected.descent());
        assert_eq!(
            format!("{:?}", actual.outline(umber_core::Point { x: 0.0, y: 0.0 })),
            format!(
                "{:?}",
                expected.outline(umber_core::Point { x: 0.0, y: 0.0 })
            )
        );
    }
}

#[test]
fn missing_bold_fails_when_bold_text_is_drawn() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let fonts = lib
        .with_figure_fonts(&[file_font(false, "inter.ttf")])
        .unwrap();
    let style = TextStyle {
        face: FaceId::new(0),
        weight: Weight::SemiBold,
        size: 12.0,
        tabular_figures: false,
    };
    let err = fonts.layout("Revenue", &style).unwrap_err();
    assert!(err.to_string().contains("Revenue"));
    assert!(err.to_string().contains("Inter"));
    assert_eq!(
        err.hint(),
        Some("pass bold= to Font.from_file, or choose a family with a bold face.")
    );
    assert!(
        fonts
            .layout(
                "123",
                &TextStyle {
                    weight: Weight::Regular,
                    ..style
                }
            )
            .unwrap()
            .width()
            > 0.0
    );
}

#[test]
fn source_serif_uses_panose_when_family_class_is_zero() {
    for bytes in [
        include_bytes!("../../../tests/fonts/SourceSerif4-Regular.otf").as_slice(),
        include_bytes!("../../../tests/fonts/SourceSerif4Variable-Roman.ttf").as_slice(),
    ] {
        let font = Font::from_files(bytes.to_vec(), None, "Source Serif 4".into()).unwrap();
        assert_eq!(font.generic_family(), "serif");
    }
}

#[test]
fn variable_font_renders_its_default_instance() {
    let bytes = include_bytes!("../../../tests/fonts/SourceSerif4Variable-Roman.ttf");
    // Removing variation tables leaves the base outlines and metrics of the default instance.
    let mut static_bytes = bytes.to_vec();
    let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let records: Vec<_> = bytes[12..12 + 16 * count]
        .chunks_exact(16)
        .filter(|record| {
            !matches!(
                &record[..4],
                b"fvar" | b"gvar" | b"avar" | b"HVAR" | b"MVAR" | b"STAT"
            )
        })
        .collect();
    static_bytes[4..6].copy_from_slice(&(records.len() as u16).to_be_bytes());
    for (target, record) in static_bytes[12..].chunks_exact_mut(16).zip(records) {
        target.copy_from_slice(record);
    }
    let variable = Font::from_files(bytes.to_vec(), None, "variable".into()).unwrap();
    let default = Font::from_files(static_bytes, None, "default".into()).unwrap();
    let lib = FontLibrary::with_fallbacks(vec![]);
    let fonts = lib.with_figure_fonts(&[variable, default]).unwrap();
    let style = TextStyle {
        face: FaceId::new(0),
        weight: Weight::Regular,
        size: 24.0,
        tabular_figures: false,
    };
    let a = fonts.layout("Ag Revenue 123 −", &style).unwrap();
    let b = fonts
        .layout(
            "Ag Revenue 123 −",
            &TextStyle {
                face: FaceId::new(1),
                ..style
            },
        )
        .unwrap();
    assert_eq!(a.width(), b.width());
    assert_eq!(a.ascent(), b.ascent());
    assert_eq!(a.descent(), b.descent());
    assert_eq!(
        a.outline(umber_core::Point::default()),
        b.outline(umber_core::Point::default())
    );
}

#[test]
fn font_file_edge_cases_missing_digits_fall_back_and_layouts_own_their_data() {
    let sparse = include_bytes!("fixtures/fallback-only.ttf").to_vec();
    let font = Font::from_files(sparse.clone(), Some(sparse), "sparse.ttf".into()).unwrap();
    let layout = {
        let lib = FontLibrary::with_fallbacks(vec![]);
        let fonts = lib.with_figure_fonts(&[font]).unwrap();
        fonts
            .layout(
                "123 −",
                &TextStyle {
                    face: FaceId::new(0),
                    weight: Weight::Regular,
                    size: 12.0,
                    tabular_figures: false,
                },
            )
            .unwrap()
    };
    assert!(layout.width() > 0.0);
    assert!(
        !layout
            .outline(umber_core::Point { x: 0.0, y: 0.0 })
            .is_empty()
    );
}
