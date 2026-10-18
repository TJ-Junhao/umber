//! Fallback order and budgets: other bundled families first, then the explicit fallback fonts,
//! whose outline confirmations are budgeted per layout and cached.

use super::fixtures::{
    LayoutBundled, assert_no_bundled_font_has, fixture_font, omega_lib, style, without_outlines,
};
use crate::fonts::{BundledFont, Weight, font_bytes};
use crate::geometry::Point;
use crate::text::FontLibrary;
use crate::text::font::FontKey;
use crate::text::library::{EXTRA_CACHE_LIMIT, EXTRA_CONFIRMATIONS_PER_LAYOUT};
use crate::text::runs::attaches_to_previous;

fn extra_confirmations(lib: &FontLibrary) -> usize {
    lib.extra_confirmations()
}

#[test]
fn missing_glyph_uses_fallback_font() {
    let text = format!("A{}", crate::fonts::FALLBACK_TEST_CODEPOINT);
    let with = omega_lib().layout(&text, &style(BundledFont::Inter, 10.0));
    let a_only = omega_lib()
        .layout("A", &style(BundledFont::Inter, 10.0))
        .width();
    assert!(with.width() > a_only);
    // The fallback glyph is an outlined Ω, so the outline must extend past the "A".
    let path = with.outline(Point { x: 0.0, y: 0.0 });
    assert!(path.bounds().expect("outline").1.x > a_only);
    // Without the fallback the code point is drawn as .notdef: width still positive, no panic.
    let without =
        FontLibrary::with_fallbacks(vec![]).layout(&text, &style(BundledFont::Inter, 10.0));
    assert!(without.width() > a_only);
}

#[test]
fn corrupt_fallback_bytes_are_ignored() {
    let lib = FontLibrary::with_fallbacks(vec![vec![0, 1, 2, 3], Vec::new()]);
    assert!(lib.layout("AB", &style(BundledFont::Inter, 10.0)).width() > 0.0);
}

#[test]
fn missing_characters_fall_back_to_other_bundled_families_first() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let mut checked = 0;
    for font in BundledFont::ALL {
        let face = lib
            .bundled_parsed(font, Weight::Regular)
            .expect("bundled font parses");
        let missing = crate::fonts::EXTENDED_CODEPOINTS
            .iter()
            .flat_map(|&(lo, hi)| lo..=hi)
            .filter_map(char::from_u32)
            .find(|&c| face.glyph(c).is_none() && !attaches_to_previous(c));
        let Some(c) = missing else { continue };
        let layout = lib.layout(&format!("a{c}"), &style(font, 10.0));
        let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
        assert_eq!(fonts.len(), 2, "{font:?} U+{:04X}: {fonts:?}", u32::from(c));
        assert_eq!(fonts[0], FontKey::Bundled(font, Weight::Regular));
        assert!(
            matches!(fonts[1], FontKey::Bundled(other, Weight::Regular) if other != font),
            "{font:?} U+{:04X}: {fonts:?}",
            u32::from(c)
        );
        checked += 1;
    }
    assert!(checked > 0, "every family covers every extended code point");
}

#[test]
fn fallback_confirmations_per_layout_are_capped() {
    let codepoints: Vec<u32> = (0xF4000..0xF4000 + 3_000).collect();
    let text: String = codepoints
        .iter()
        .filter_map(|&cp| char::from_u32(cp))
        .collect();
    assert_no_bundled_font_has(&text);
    // Two fallback fonts that claim every character but have no outlines.
    let lib = FontLibrary::with_fallbacks(
        (0..2)
            .map(|k| without_outlines(fixture_font(&format!("Claim {k}"), 400, false, &codepoints)))
            .collect(),
    );
    let s = style(BundledFont::Inter, 10.0);
    let layout = lib.layout(&text, &s);
    // 3,000 characters would need 6,000 confirmations.
    assert_eq!(extra_confirmations(&lib), EXTRA_CONFIRMATIONS_PER_LAYOUT);
    let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
    assert_eq!(
        fonts,
        [FontKey::Bundled(BundledFont::Inter, Weight::Regular)]
    );
    // Results are cached across layouts; the next call confirms only the rest.
    let _ = lib.layout(&text, &s);
    assert_eq!(extra_confirmations(&lib), 6_000);
    let _ = lib.layout(&text, &s);
    assert_eq!(extra_confirmations(&lib), 6_000);
    assert_eq!(lib.extra_cache_len(), 6_000);
}

#[test]
fn fallback_confirmation_cache_is_bounded() {
    let total = EXTRA_CACHE_LIMIT + 4_000;
    let chars: Vec<char> = (0xF0000..).filter_map(char::from_u32).take(total).collect();
    let codepoints: Vec<u32> = chars.iter().map(|&c| u32::from(c)).collect();
    let lib = FontLibrary::with_fallbacks(vec![without_outlines(fixture_font(
        "Claim",
        400,
        false,
        &codepoints,
    ))]);
    let s = style(BundledFont::Inter, 10.0);
    for chunk in chars.chunks(EXTRA_CONFIRMATIONS_PER_LAYOUT) {
        let text: String = chunk.iter().collect();
        let _ = lib.layout(&text, &s);
    }
    assert_eq!(extra_confirmations(&lib), total);
    assert_eq!(lib.extra_cache_len(), EXTRA_CACHE_LIMIT);
    // Characters that did not fit are confirmed again; cached ones are not.
    let late: String = chars[total - 10..].iter().collect();
    let early: String = chars[..10].iter().collect();
    let _ = lib.layout(&format!("{early}{late}"), &s);
    assert_eq!(extra_confirmations(&lib), total + 10);
    assert_eq!(lib.extra_cache_len(), EXTRA_CACHE_LIMIT);
}

#[test]
fn fallback_without_outlines_is_skipped() {
    let fixture = include_bytes!("../../../tests/fixtures/fallback-only.ttf");
    let truncated = fixture[..516].to_vec();
    let text = crate::fonts::FALLBACK_TEST_CODEPOINT.to_string();
    let s = style(BundledFont::Inter, 10.0);
    let lib = FontLibrary::with_fallbacks(vec![truncated]);
    assert_eq!(lib.extra_count(), 1, "the truncated font still parses");
    let layout = lib.layout(&text, &s);
    let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
    assert_eq!(
        fonts,
        [FontKey::Bundled(BundledFont::Inter, Weight::Regular)]
    );
    let path = layout.outline(Point { x: 0.0, y: 0.0 });
    let notdef = FontLibrary::with_fallbacks(vec![])
        .layout(&text, &s)
        .outline(Point { x: 0.0, y: 0.0 });
    assert!(!path.is_empty());
    assert_eq!(path.verbs().len(), notdef.verbs().len());
}

#[test]
fn bundled_families_take_precedence_over_explicit_fallbacks() {
    let c = '\u{0114}';
    let geist = FontLibrary::with_fallbacks(vec![]);
    let geist_face = geist
        .bundled_parsed(BundledFont::Geist, Weight::Regular)
        .unwrap();
    let serif_face = geist
        .bundled_parsed(BundledFont::SourceSerif4, Weight::Regular)
        .unwrap();
    assert!(geist_face.glyph(c).is_none() && serif_face.glyph(c).is_some());
    let serif = font_bytes(BundledFont::SourceSerif4, Weight::Regular).to_vec();
    let lib = FontLibrary::with_fallbacks(vec![serif]);
    let layout = lib.layout(&c.to_string(), &style(BundledFont::Geist, 10.0));
    let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
    assert_eq!(
        fonts,
        [FontKey::Bundled(BundledFont::SourceSerif4, Weight::Regular)]
    );
}
