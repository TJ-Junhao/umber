//! Named-family selection with an in-memory font database.

use super::*;
use crate::fonts::{BundledFont, Font, font_bytes};
use crate::test_support::font_fixture;
use std::sync::Arc;

fn insert(
    db: &mut fontdb::Database,
    family: &str,
    font: BundledFont,
    weight: u16,
    style: fontdb::Style,
) {
    let mut source = fontdb::Database::new();
    source.load_font_data(font_bytes(font, Weight::Regular).to_vec());
    let mut face = source.faces().next().unwrap().clone();
    face.families = vec![(family.into(), fontdb::Language::English_UnitedStates)];
    face.weight = fontdb::Weight(weight);
    face.style = style;
    db.push_face_info(face);
}

fn resolve(db: &fontdb::Database, family: &str) -> Font {
    Font::System(Arc::new(family_in(db, family).unwrap()))
}

fn chosen(font: &Font, weight: Weight) -> &[u8] {
    &font.owned_face(weight).unwrap().bytes
}

#[test]
fn system_family_selects_upright_weights_and_canonical_name() {
    let mut db = fontdb::Database::new();
    insert(
        &mut db,
        "Example",
        BundledFont::Geist,
        400,
        fontdb::Style::Italic,
    );
    insert(
        &mut db,
        "Example",
        BundledFont::Inter,
        400,
        fontdb::Style::Normal,
    );
    insert(
        &mut db,
        "Example",
        BundledFont::Geist,
        600,
        fontdb::Style::Italic,
    );
    insert(
        &mut db,
        "Example",
        BundledFont::IbmPlexSans,
        700,
        fontdb::Style::Normal,
    );
    insert(
        &mut db,
        "Example",
        BundledFont::SourceSerif4,
        600,
        fontdb::Style::Normal,
    );
    let font = resolve(&db, "eXaMpLe");
    assert_eq!(font.family_name(), "Example");
    assert_eq!(font, resolve(&db, "Example"));
    assert_eq!(
        chosen(&font, Weight::Regular),
        font_bytes(BundledFont::Inter, Weight::Regular)
    );
    assert_eq!(
        chosen(&font, Weight::SemiBold),
        font_bytes(BundledFont::SourceSerif4, Weight::Regular)
    );
}

#[test]
fn system_family_uses_closest_regular_and_does_not_synthesize_bold() {
    let mut db = fontdb::Database::new();
    insert(
        &mut db,
        "Example",
        BundledFont::Geist,
        300,
        fontdb::Style::Normal,
    );
    insert(
        &mut db,
        "Example",
        BundledFont::Inter,
        450,
        fontdb::Style::Normal,
    );
    insert(
        &mut db,
        "Example",
        BundledFont::SourceSerif4,
        900,
        fontdb::Style::Italic,
    );
    let font = resolve(&db, "Example");
    assert_eq!(
        chosen(&font, Weight::Regular),
        font_bytes(BundledFont::Inter, Weight::Regular)
    );
    assert!(!font.has_face(Weight::SemiBold));
    insert(
        &mut db,
        "Example",
        BundledFont::IbmPlexSans,
        700,
        fontdb::Style::Normal,
    );
    let font = resolve(&db, "Example");
    assert_eq!(
        chosen(&font, Weight::SemiBold),
        font_bytes(BundledFont::IbmPlexSans, Weight::Regular)
    );
}

#[test]
fn system_unknown_lists_closest_with_static_hint() {
    let mut db = fontdb::Database::new();
    for family in ["Times New Roman", "Times"] {
        insert(
            &mut db,
            family,
            BundledFont::Inter,
            400,
            fontdb::Style::Normal,
        );
    }
    let error = family_in(&db, "Tmes").unwrap_err();
    assert_eq!(
        error.to_string(),
        "Font.system(): no installed family is named 'Tmes'; it must name an installed font family. Closest: Times, Times New Roman, Inter."
    );
    assert_eq!(error.parameter(), Some("family"));
    assert_eq!(error.hint(), Some("check the installed family names."));
    let empty = family_in(&fontdb::Database::new(), "Tmes").unwrap_err();
    assert!(!empty.to_string().contains("Closest:"));
    assert_eq!(edit_distance("字体", "字体a"), 1);
}

#[test]
fn system_family_missing_file_is_reported_at_construction() {
    let mut source = fontdb::Database::new();
    source.load_font_data(font_bytes(BundledFont::Inter, Weight::Regular).to_vec());
    let mut face = source.faces().next().unwrap().clone();
    face.source = fontdb::Source::File(std::path::PathBuf::from("/umber-absent-font-394856.ttf"));
    let mut db = fontdb::Database::new();
    db.push_face_info(face);
    let error = family_in(&db, "Inter").unwrap_err();
    assert_eq!(error.parameter(), Some("family"));
    assert_eq!(error.hint(), Some("check the installed font files."));
}

#[test]
fn system_family_prefers_normal_width_before_weight_distance() {
    let mut db = fontdb::Database::new();
    insert(
        &mut db,
        "Example",
        BundledFont::Inter,
        450,
        fontdb::Style::Normal,
    );
    insert(
        &mut db,
        "Example",
        BundledFont::IbmPlexSans,
        700,
        fontdb::Style::Normal,
    );
    let mut condensed = fontdb::Database::new();
    insert(
        &mut condensed,
        "Example",
        BundledFont::Geist,
        400,
        fontdb::Style::Normal,
    );
    insert(
        &mut condensed,
        "Example",
        BundledFont::SourceSerif4,
        600,
        fontdb::Style::Normal,
    );
    for face in condensed.faces() {
        let mut face = face.clone();
        face.stretch = fontdb::Stretch::Condensed;
        db.push_face_info(face);
    }
    let font = resolve(&db, "Example");
    assert_eq!(
        chosen(&font, Weight::Regular),
        font_bytes(BundledFont::Inter, Weight::Regular)
    );
    assert_eq!(
        chosen(&font, Weight::SemiBold),
        font_bytes(BundledFont::IbmPlexSans, Weight::Regular)
    );
}

#[test]
fn system_family_matches_legacy_names_not_kept_by_fontdb() {
    let mut db = fontdb::Database::new();
    // The source file's name ID 1 is Inter; the database exposes its typographic family.
    insert(
        &mut db,
        "Typographic Family",
        BundledFont::Inter,
        400,
        fontdb::Style::Normal,
    );
    let font = resolve(&db, "iNtEr");
    assert_eq!(font.family_name(), "Inter");
    assert_eq!(
        chosen(&font, Weight::Regular),
        font_bytes(BundledFont::Inter, Weight::Regular)
    );
}

#[test]
fn system_family_prefers_typographic_matches_over_legacy_aliases() {
    let mut db = fontdb::Database::new();
    insert(
        &mut db,
        "Typographic Family",
        BundledFont::Inter,
        400,
        fontdb::Style::Normal,
    );
    insert(
        &mut db,
        "Inter",
        BundledFont::Geist,
        450,
        fontdb::Style::Normal,
    );
    let font = resolve(&db, "iNtEr");
    assert_eq!(
        chosen(&font, Weight::Regular),
        font_bytes(BundledFont::Geist, Weight::Regular)
    );
}

#[test]
fn system_closest_names_favour_shared_words_and_prefixes_case_insensitively() {
    let mut db = fontdb::Database::new();
    for family in ["Ubuntu", "Noto Serif", "Noto Sans", "Inter"] {
        insert(
            &mut db,
            family,
            BundledFont::Inter,
            400,
            fontdb::Style::Normal,
        );
    }
    for query in ["noto", "NOTO"] {
        let error = family_in(&db, query).unwrap_err().to_string();
        assert!(
            error.ends_with("Closest: Noto Sans, Noto Serif, Inter."),
            "{error}"
        );
    }
}

/// A database holding `bytes` as the only face of its family.
fn database_of(bytes: &[u8]) -> (fontdb::Database, String) {
    let mut db = fontdb::Database::new();
    db.load_font_data(bytes.to_vec());
    let family = db.faces().next().unwrap().families[0].0.clone();
    (db, family)
}

/// Adds the faces of `bytes` to `db` as faces of `family`.
fn insert_file(db: &mut fontdb::Database, family: &str, bytes: &[u8]) {
    let mut source = fontdb::Database::new();
    source.load_font_data(bytes.to_vec());
    for face in source.faces() {
        let mut face = face.clone();
        face.families = vec![(family.into(), fontdb::Language::English_UnitedStates)];
        db.push_face_info(face);
    }
}

/// The `wght` of the named instance drawing `weight`, or `None` at the default location.
fn instance_weight(font: &Font, weight: Weight) -> Option<f32> {
    match font.owned_face(weight).unwrap().instance() {
        crate::text::Instance::Named(instance) => Some(instance.weight()),
        crate::text::Instance::Default => None,
    }
}

#[test]
fn system_variable_only_family_draws_bold_in_its_bold_instance() {
    let Some(bytes) = font_fixture::read("UmberFixtureSerif-Variable.ttf") else {
        return;
    };
    let (db, family) = database_of(&bytes);
    let font = resolve(&db, &family);
    assert!(font.has_face(Weight::SemiBold));
    assert_eq!(instance_weight(&font, Weight::Regular), Some(400.0));
    assert_eq!(instance_weight(&font, Weight::SemiBold), Some(700.0));
    assert!(Arc::ptr_eq(
        &font.owned_face(Weight::Regular).unwrap().bytes,
        &font.owned_face(Weight::SemiBold).unwrap().bytes
    ));
    assert_eq!(font.missing_bold_note(), None);
    // The faces are the file font's faces.
    let file = Font::from_files(bytes.to_vec(), None, family).unwrap();
    let lib = crate::FontLibrary::with_fallbacks(vec![]);
    let fonts = lib.with_figure_fonts(&[font, file]).unwrap();
    for weight in Weight::ALL {
        let style = |face| crate::TextStyle {
            face: crate::FaceId::new(face),
            weight,
            size: 12.0,
            tabular_figures: false,
        };
        let system = fonts.layout("Heading 123", &style(0)).unwrap();
        let file = fonts.layout("Heading 123", &style(1)).unwrap();
        assert_eq!(system.width(), file.width());
        assert_eq!(
            system.outline(crate::Point::default()),
            file.outline(crate::Point::default())
        );
    }
}

#[test]
fn system_mixed_family_keeps_its_static_faces_and_fills_only_a_missing_bold() {
    let Some(bytes) = font_fixture::read("UmberFixtureSerif-Variable.ttf") else {
        return;
    };
    let mut db = fontdb::Database::new();
    insert(
        &mut db,
        "Example",
        BundledFont::Inter,
        400,
        fontdb::Style::Normal,
    );
    insert_file(&mut db, "Example", &bytes);
    // The static regular face is closer to 400 than the variable file's default, Light 300.
    let font = resolve(&db, "Example");
    assert_eq!(
        chosen(&font, Weight::Regular),
        font_bytes(BundledFont::Inter, Weight::Regular)
    );
    assert_eq!(chosen(&font, Weight::SemiBold), bytes.as_slice());
    assert_eq!(instance_weight(&font, Weight::SemiBold), Some(700.0));
    // A static bold face keeps priority over the variable file's bold instance.
    insert(
        &mut db,
        "Example",
        BundledFont::Geist,
        800,
        fontdb::Style::Normal,
    );
    let font = resolve(&db, "Example");
    assert_eq!(
        chosen(&font, Weight::SemiBold),
        font_bytes(BundledFont::Geist, Weight::Regular)
    );
    assert_eq!(instance_weight(&font, Weight::SemiBold), None);
    assert_eq!(instance_weight(&font, Weight::Regular), None);
}

#[test]
fn system_static_regular_face_wins_over_a_variable_file_that_sorts_first() {
    // Both are upright, normal width and weight 400, and the variable file's PostScript name
    // sorts first: the static face is still the regular face.
    let mut db = fontdb::Database::new();
    insert(
        &mut db,
        "Example",
        BundledFont::Inter,
        400,
        fontdb::Style::Normal,
    );
    let mut source = fontdb::Database::new();
    source.load_font_data(
        include_bytes!("../../../tests/fixtures/outline-variable-light.ttf").to_vec(),
    );
    let mut variable = source.faces().next().unwrap().clone();
    variable.families = vec![("Example".into(), fontdb::Language::English_UnitedStates)];
    variable.weight = fontdb::Weight(400);
    variable.post_script_name = "AAVariable".into();
    db.push_face_info(variable);
    let names: Vec<String> = db.faces().map(|f| f.post_script_name.clone()).collect();
    assert!(
        names.iter().any(|name| name.as_str() > "AAVariable"),
        "{names:?}"
    );
    let font = resolve(&db, "Example");
    assert_eq!(
        chosen(&font, Weight::Regular),
        font_bytes(BundledFont::Inter, Weight::Regular)
    );
    assert_eq!(instance_weight(&font, Weight::Regular), None);
    assert_eq!(font.drawn_weight(Weight::Regular), 400);
    // Without the static face, the variable file is the regular face, in its instance 350.
    let mut only = fontdb::Database::new();
    for face in db
        .faces()
        .filter(|face| face.post_script_name == "AAVariable")
    {
        only.push_face_info(face.clone());
    }
    let font = resolve(&only, "Example");
    assert_eq!(instance_weight(&font, Weight::Regular), Some(350.0));
}

#[test]
fn system_variable_only_family_explains_missing_bold() {
    for bytes in [
        &include_bytes!("../../../tests/fixtures/outline-variable-light.ttf")[..],
        &include_bytes!("../../../tests/fixtures/outline-variable.ttf")[..],
    ] {
        let (db, family) = database_of(bytes);
        let font = resolve(&db, &family);
        assert!(!font.has_face(Weight::SemiBold));
        let lib = crate::FontLibrary::with_fallbacks(vec![]);
        let fonts = lib.with_figure_fonts(&[font]).unwrap();
        let error = fonts
            .layout(
                "Heading",
                &crate::TextStyle {
                    face: crate::FaceId::new(0),
                    weight: Weight::SemiBold,
                    size: 12.0,
                    tabular_figures: false,
                },
            )
            .unwrap_err();
        assert!(
            error.to_string().ends_with(
                "The family's installed files are variable fonts without a named instance of \
                 weight 600 or more."
            ),
            "{error}"
        );
        assert_eq!(
            error.hint(),
            Some(
                "pass bold= to Font.from_file, choose a family with a bold face, or pass bold_fallback=True."
            )
        );
    }
}

#[test]
fn system_bold_fallback_uses_regular_only_without_a_bold_face() {
    use crate::{FaceId, FontLibrary, Point, TextStyle};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut db = fontdb::Database::new();
    insert(
        &mut db,
        "Example",
        BundledFont::Inter,
        400,
        fontdb::Style::Normal,
    );
    let lib = FontLibrary::with_fallbacks(vec![]);
    let style = TextStyle {
        face: FaceId::new(0),
        weight: Weight::SemiBold,
        size: 15.0,
        tabular_figures: false,
    };
    for has_bold in [false, true] {
        if has_bold {
            insert(
                &mut db,
                "Example",
                BundledFont::Geist,
                600,
                fontdb::Style::Normal,
            );
        }
        let plain = resolve(&db, "Example");
        let fallback = plain.clone().with_bold_fallback(true).unwrap();
        assert_ne!(plain, fallback);
        assert_eq!(fallback.has_face(Weight::SemiBold), has_bold);
        let hash = |font: &Font| {
            let mut hasher = DefaultHasher::new();
            font.hash(&mut hasher);
            hasher.finish()
        };
        assert_ne!(hash(&plain), hash(&fallback));
        assert_eq!(fallback.clone().with_bold_fallback(false).unwrap(), plain);
        let fonts = lib.with_figure_fonts(&[fallback, plain]).unwrap();
        let actual = fonts.layout("Heading 123", &style).unwrap();
        let expected = fonts
            .layout(
                "Heading 123",
                &TextStyle {
                    face: FaceId::new(1),
                    weight: if has_bold {
                        Weight::SemiBold
                    } else {
                        Weight::Regular
                    },
                    ..style
                },
            )
            .unwrap();
        assert_eq!(actual.width(), expected.width());
        assert_eq!(
            actual.outline(Point::default()),
            expected.outline(Point::default())
        );
    }
}

#[test]
fn system_variable_only_bold_fallback_renders_every_bold_role() {
    use crate::{AxisData, Figure, FontLibrary, LegendStyle, LineOptions, ScaleKind, Theme};
    // Characters of the fixture, so that the variable font itself draws the bold text.
    const HEADING: &str = "\u{E00A}\u{E00B}\u{E00C}";
    // Weight instances at 350, 450 and 500 only: no bold face.
    let bytes = include_bytes!("../../../tests/fixtures/outline-variable-light.ttf");
    let (db, family) = database_of(bytes);
    let plain = resolve(&db, &family);
    let fallback = plain.clone().with_bold_fallback(true).unwrap();
    assert!(!fallback.has_face(Weight::SemiBold));
    // The file font of the same file draws bold text in its regular instance the same way.
    let reference = Font::from_files(bytes.to_vec(), None, family)
        .unwrap()
        .with_bold_fallback(true)
        .unwrap();
    let lib = FontLibrary::with_fallbacks(vec![]);
    let figure = |font: Font, role| {
        let theme = Theme::pigment()
            .with_title_font(font.clone())
            .unwrap()
            .with_body_font(font)
            .unwrap();
        let mut fig = Figure::new(6.0, 4.0, theme).unwrap();
        if role == 1 {
            fig.title(HEADING);
        }
        let mut axes = fig.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        if role == 0 {
            axes.title(HEADING);
        }
        axes.legend(
            if role == 2 {
                LegendStyle::Direct
            } else {
                LegendStyle::None
            },
            crate::SizeSpec::Auto,
        )
        .unwrap();
        axes.line(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            LineOptions {
                label: HEADING.into(),
                ..LineOptions::default()
            },
        )
        .unwrap();
        fig
    };
    for role in 0..3 {
        let err = figure(plain.clone(), role).to_scene(&lib).unwrap_err();
        assert!(
            err.to_string()
                .contains("The family's installed files are variable fonts"),
            "{err}"
        );
        let actual = figure(fallback.clone(), role).to_scene(&lib).unwrap();
        let expected = figure(reference.clone(), role).to_scene(&lib).unwrap();
        assert_eq!(
            crate::render::render_png(&actual, 96).unwrap(),
            crate::render::render_png(&expected, 96).unwrap()
        );
        assert_eq!(
            crate::render::render_svg(&actual, crate::render::SvgText::Outlines, 300).unwrap(),
            crate::render::render_svg(&expected, crate::render::SvgText::Outlines, 300).unwrap()
        );
        assert!(
            !crate::render::render_svg(&actual, crate::render::SvgText::Editable, 300)
                .unwrap()
                .contains("font-weight=\"600\"")
        );
    }
}

#[test]
fn system_collection_faces_share_one_byte_buffer() {
    let mut bytes = b"ttcf\0\x01\0\0\0\0\0\x02\0\0\0\0\0\0\0\0".to_vec();
    for (i, weight) in Weight::ALL.into_iter().enumerate() {
        let original = font_bytes(BundledFont::Inter, weight);
        let start = bytes.len();
        bytes[12 + 4 * i..16 + 4 * i].copy_from_slice(&(start as u32).to_be_bytes());
        bytes.extend(original);
        let count = u16::from_be_bytes([original[4], original[5]]) as usize;
        for record in 0..count {
            let offset = start + 12 + 16 * record + 8;
            let value =
                u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) + start as u32;
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }
    }
    let mut db = fontdb::Database::new();
    db.load_font_data(bytes);
    let font = resolve(&db, "Inter");
    let regular = font.owned_face(Weight::Regular).unwrap();
    let bold = font.owned_face(Weight::SemiBold).unwrap();
    assert_ne!(regular.index, bold.index);
    assert!(Arc::ptr_eq(&regular.bytes, &bold.bytes));
}

#[test]
fn system_family_accepts_an_installed_collection() {
    let original = font_bytes(BundledFont::Inter, Weight::Regular);
    let mut bytes = b"ttcf\0\x01\0\0\0\0\0\x01\0\0\0\x10".to_vec();
    bytes.extend(original);
    let count = u16::from_be_bytes([original[4], original[5]]) as usize;
    for record in 0..count {
        let offset = 16 + 12 + 16 * record + 8;
        let value = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) + 16;
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    assert!(Font::from_files(bytes.clone(), None, "collection".into()).is_err());
    let mut db = fontdb::Database::new();
    db.load_font_data(bytes);
    let font = resolve(&db, "Inter");
    let lib = crate::FontLibrary::with_fallbacks(vec![]);
    let fonts = lib.with_figure_fonts(&[font]).unwrap();
    let layout = fonts
        .layout(
            "Collection 123",
            &crate::TextStyle {
                face: crate::FaceId::new(0),
                weight: Weight::Regular,
                size: 12.0,
                tabular_figures: false,
            },
        )
        .unwrap();
    assert!(layout.width() > 0.0);
    assert!(!layout.outline(crate::Point { x: 0.0, y: 0.0 }).is_empty());
}
