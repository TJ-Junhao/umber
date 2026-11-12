//! Variable fonts are drawn in the named instances chosen for their regular and bold faces.

use read_fonts::types::GlyphId;
use read_fonts::{FontRef, TableProvider};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};

use crate::Error;
use crate::fonts::{Font, Weight};
use crate::geometry::Point;
use crate::test_support::font_fixture;
use crate::text::{FaceId, FontLibrary, Instance, TextStyle};

/// A variable font whose default is Light 300, with instances at 200, 300, 400, 700 and 900.
const FIXTURE: &str = "UmberFixtureSerif-Variable.ttf";

const LIGHT: &[u8] = include_bytes!("../../../tests/fixtures/outline-variable-light.ttf");
const WIDTH: &[u8] = include_bytes!("../../../tests/fixtures/outline-width.ttf");
const UNNAMED: &[u8] = include_bytes!("../../../tests/fixtures/outline-variable.ttf");

fn style(face: usize, weight: Weight) -> TextStyle {
    TextStyle {
        face: FaceId::new(face),
        weight,
        size: 20.0,
        tabular_figures: false,
    }
}

fn file(regular: &[u8], bold: Option<&[u8]>) -> Font {
    Font::from_files(regular.to_vec(), bold.map(<[u8]>::to_vec), "test".into()).unwrap()
}

/// The `wght` of the named instance drawing `weight`, or `None` at the default location.
fn instance_weight(font: &Font, weight: Weight) -> Option<f32> {
    match font.owned_face(weight).unwrap().instance() {
        Instance::Named(instance) => Some(instance.weight()),
        Instance::Default => None,
    }
}

/// The user coordinates of the named instance drawing `weight`.
fn instance_user(font: &Font, weight: Weight) -> Vec<(read_fonts::types::Tag, f32)> {
    match font.owned_face(weight).unwrap().instance() {
        Instance::Named(instance) => instance.location().user().to_vec(),
        Instance::Default => Vec::new(),
    }
}

#[test]
fn a_variable_file_draws_regular_at_400_and_bold_at_the_first_weight_from_600() {
    let Some(bytes) = font_fixture::read(FIXTURE) else {
        return;
    };
    let font = file(&bytes, None);
    assert!(font.has_face(Weight::SemiBold));
    assert_eq!(instance_weight(&font, Weight::Regular), Some(400.0));
    // The fixture has no instance at 600; Bold 700 is the closest from above.
    assert_eq!(instance_weight(&font, Weight::SemiBold), Some(700.0));
    assert_eq!(font.drawn_weight(Weight::Regular), 400);
    assert_eq!(font.drawn_weight(Weight::SemiBold), 700);
}

#[test]
fn a_tie_on_weight_goes_to_the_instance_nearest_the_default_on_the_other_axes() {
    let Some(bytes) = font_fixture::read("SourceSerif4Variable-Roman.ttf") else {
        return;
    };
    // Five instances have weight 400 and five 600, at optical sizes 8 to 60; the default
    // optical size is 20.
    let font = file(&bytes, None);
    let opsz = read_fonts::types::Tag::new(b"opsz");
    for (weight, wght) in [(Weight::Regular, 400.0), (Weight::SemiBold, 600.0)] {
        let user = instance_user(&font, weight);
        assert!(user.contains(&(opsz, 20.0)), "{weight:?} {user:?}");
        assert_eq!(instance_weight(&font, weight), Some(wght));
    }
    // The regular instance is the default location, which needs no coordinates.
    let regular = font.owned_face(Weight::Regular).unwrap();
    assert!(regular.shaping.normalized_coords().is_empty());
}

#[test]
fn a_tie_between_350_and_450_goes_to_the_lighter_and_no_instance_from_600_means_no_bold() {
    let font = file(LIGHT, None);
    assert_eq!(instance_weight(&font, Weight::Regular), Some(350.0));
    assert!(!font.has_face(Weight::SemiBold));
    assert_eq!(
        font.missing_bold_note(),
        Some("The file is a variable font without a named instance of weight 600 or more.")
    );
}

#[test]
fn a_bold_file_without_an_instance_from_600_uses_its_instance_closest_to_600() {
    let font = file(UNNAMED, Some(LIGHT));
    assert_eq!(instance_weight(&font, Weight::SemiBold), Some(500.0));
    assert_eq!(font.drawn_weight(Weight::SemiBold), 500);
    assert_eq!(font.missing_bold_note(), None);
    // The same file as regular and bold file: the bold file's rule applies.
    let both = file(LIGHT, Some(LIGHT));
    assert_eq!(instance_weight(&both, Weight::Regular), Some(350.0));
    assert_eq!(instance_weight(&both, Weight::SemiBold), Some(500.0));
}

#[test]
fn files_without_a_wght_axis_or_named_instances_stay_at_their_default() {
    for bytes in [WIDTH, UNNAMED] {
        let font = file(bytes, None);
        assert_eq!(instance_weight(&font, Weight::Regular), None);
        assert!(!font.has_face(Weight::SemiBold));
        assert_eq!(font.drawn_weight(Weight::Regular), 400);
        let with_bold = file(bytes, Some(bytes));
        assert_eq!(instance_weight(&with_bold, Weight::SemiBold), None);
        assert_eq!(with_bold.drawn_weight(Weight::SemiBold), 600);
    }
}

#[test]
fn shaped_advances_and_metrics_are_those_of_the_instance() {
    let Some(bytes) = font_fixture::read(FIXTURE) else {
        return;
    };
    let font = file(&bytes, None);
    let lib = FontLibrary::with_fallbacks(vec![]);
    let fonts = lib.with_figure_fonts(std::slice::from_ref(&font)).unwrap();
    let parsed = FontRef::new(&bytes).unwrap();
    let units_per_em = f32::from(parsed.head().map_or(1000, |head| head.units_per_em()));
    for weight in Weight::ALL {
        let face = font.owned_face(weight).unwrap();
        let Instance::Named(instance) = face.instance() else {
            panic!("{weight:?} is not a named instance");
        };
        let at = LocationRef::new(instance.location().coords());
        let advances = parsed.glyph_metrics(Size::unscaled(), at);
        let metrics = parsed.metrics(Size::unscaled(), at);
        // One glyph per layout, so no kerning pair moves an advance.
        for c in ['H', 'a', 'm', 'g', '1', 'W'] {
            let layout = fonts.layout(&c.to_string(), &style(0, weight)).unwrap();
            let [run] = layout.runs() else {
                panic!("one run");
            };
            let [glyph] = run.glyphs() else {
                panic!("one glyph");
            };
            let expected = advances
                .advance_width(GlyphId::new(u32::from(glyph.id)))
                .unwrap()
                .round();
            let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
            assert!(
                close(glyph.advance * units_per_em / 20.0, expected),
                "{c} {weight:?}: {} against {expected}",
                glyph.advance * units_per_em / 20.0
            );
            assert!(close(layout.ascent(), metrics.ascent * 20.0 / units_per_em));
            assert!(close(
                layout.descent(),
                -metrics.descent * 20.0 / units_per_em
            ));
        }
    }
}

#[test]
fn regular_and_bold_from_one_file_differ_and_differ_from_the_default() {
    let Some(bytes) = font_fixture::read(FIXTURE) else {
        return;
    };
    let lib = FontLibrary::with_fallbacks(vec![]);
    let font = file(&bytes, None);
    // The light font has no named instances: it stays at the default location, Light 300.
    let default = file(&strip_named_instances(&bytes), None);
    assert_eq!(instance_weight(&default, Weight::Regular), None);
    let fonts = lib.with_figure_fonts(&[font, default]).unwrap();
    let text = "Revenue 2024";
    let light = fonts.layout(text, &style(1, Weight::Regular)).unwrap();
    let regular = fonts.layout(text, &style(0, Weight::Regular)).unwrap();
    let bold = fonts.layout(text, &style(0, Weight::SemiBold)).unwrap();
    assert!(light.width() < regular.width() && regular.width() < bold.width());
    let outlines = [&light, &regular, &bold].map(|layout| layout.outline(Point::default()));
    assert_ne!(outlines[0], outlines[1]);
    assert_ne!(outlines[1], outlines[2]);
    let program = |layout: &crate::text::TextLayout| layout.runs()[0].run_font().program();
    assert_ne!(program(&regular), program(&bold));
    assert_ne!(program(&light), program(&regular));
}

/// `bytes` with an `fvar` that lists no named instances.
fn strip_named_instances(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let count = usize::from(u16::from_be_bytes([bytes[4], bytes[5]]));
    let fvar = (0..count)
        .map(|i| 12 + 16 * i)
        .find(|&record| &bytes[record..record + 4] == b"fvar")
        .map(|record| {
            u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize
        })
        .unwrap();
    // The instance count is the eighth u16 of the table header; the checksum is not checked.
    out[fvar + 12..fvar + 14].copy_from_slice(&0_u16.to_be_bytes());
    out
}

#[test]
fn bold_fallback_is_allowed_and_inert_with_a_bold_instance_and_refused_with_a_bold_file() {
    let Some(bytes) = font_fixture::read(FIXTURE) else {
        return;
    };
    let lib = FontLibrary::with_fallbacks(vec![]);
    let plain = file(&bytes, None);
    let fallback = plain.clone().with_bold_fallback(true).unwrap();
    assert_ne!(plain, fallback);
    assert!(fallback.has_face(Weight::SemiBold));
    let fonts = lib.with_figure_fonts(&[plain, fallback]).unwrap();
    let a = fonts
        .layout("Heading", &style(0, Weight::SemiBold))
        .unwrap();
    let b = fonts
        .layout("Heading", &style(1, Weight::SemiBold))
        .unwrap();
    assert_eq!(a.width(), b.width());
    assert_eq!(a.outline(Point::default()), b.outline(Point::default()));

    let error = file(&bytes, Some(&bytes))
        .with_bold_fallback(true)
        .unwrap_err();
    assert!(matches!(error, Error::Usage { .. }), "{error}");

    // Without a bold instance, the fallback draws bold text in the regular instance.
    let light = file(LIGHT, None);
    let fonts = lib
        .with_figure_fonts(&[light.clone(), light.with_bold_fallback(true).unwrap()])
        .unwrap();
    let text = "\u{E00A}\u{E00B}\u{E00C}";
    let error = fonts.layout(text, &style(0, Weight::SemiBold)).unwrap_err();
    assert!(
        error.to_string().ends_with(
            "The file is a variable font without a named instance of weight 600 or more."
        ),
        "{error}"
    );
    let regular = fonts.layout(text, &style(1, Weight::Regular)).unwrap();
    let bold = fonts.layout(text, &style(1, Weight::SemiBold)).unwrap();
    assert_eq!(
        regular.outline(Point::default()),
        bold.outline(Point::default())
    );
}

#[test]
fn equality_follows_the_files_not_the_instances() {
    let plain = file(LIGHT, None);
    assert_eq!(plain, file(LIGHT, None));
    assert_ne!(plain, file(LIGHT, Some(LIGHT)));
    let Font::File(files) = &plain else {
        unreachable!()
    };
    assert_eq!(files.regular_file(), LIGHT);
    assert_eq!(files.bold_file(), None);
    let Font::File(files) = file(UNNAMED, Some(LIGHT)) else {
        unreachable!()
    };
    assert_eq!(files.bold_file(), Some(LIGHT));
}
