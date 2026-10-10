//! Character maps: secondary and format 2 subtables, and agreement between the shaper and
//! skrifa on the best subtable.

use read_fonts::types::GlyphId;

use super::fixtures::{assert_no_bundled_font_has, cmap_of, font_with_cmap, format12, style};
use crate::fonts::{Font, Weight};
use crate::text::FontLibrary;
use crate::text::cmap::SECONDARY_SUBTABLES;
use crate::text::font::{Face, FontKey};

/// 50,000 characters over 4,096 distinct code points from U+F0000.
fn many_secondary_characters() -> String {
    (0..50_000u32)
        .filter_map(|i| char::from_u32(0xF0000 + i % 4_096))
        .collect()
}

/// The groups of a subtable that maps the 4,096 code points from U+F0000 to glyph 1.
fn secondary_hits() -> Vec<(u32, u32, u32)> {
    (0..4_096).map(|k| (0xF0000 + k, 0xF0000 + k, 1)).collect()
}

fn assert_fast(elapsed: std::time::Duration) {
    let limit = if cfg!(debug_assertions) {
        std::time::Duration::from_secs(2)
    } else {
        std::time::Duration::from_millis(100)
    };
    println!("layout: {elapsed:?}");
    assert!(elapsed < limit, "{elapsed:?}");
}

/// Lays `text` out with `font` as a fallback font and as a system font, and returns the
/// glyph ids of each layout's single run.
fn glyphs_as_extra_and_system(font: &[u8], text: &str) -> [Vec<u16>; 2] {
    let s = style(Font::Inter, 10.0);
    let extra = FontLibrary::with_fallbacks(vec![font.to_vec()]).layout(text, &s);
    let system = FontLibrary::with_test_system_fonts(vec![font.to_vec()]).layout(text, &s);
    assert_eq!(extra.runs().len(), 1);
    assert_eq!(system.runs().len(), 1);
    assert_eq!(extra.runs()[0].font(), FontKey::Extra(0));
    assert!(matches!(system.runs()[0].font(), FontKey::System(_)));
    [extra, system].map(|layout| layout.runs()[0].glyphs().iter().map(|g| g.id).collect())
}

#[test]
fn secondary_subtables_are_read_once_each() {
    // 8,190 records share one large subtable that maps nothing useful; the record after
    // them maps the text; the best subtable (Windows, full repertoire) maps none of it.
    let miss: Vec<(u32, u32, u32)> = (0..16_384)
        .map(|k| (0x4000_0000 + 2 * k, 0x4000_0000 + 2 * k, 1))
        .collect();
    let subtables = [
        format12(&miss),
        format12(&secondary_hits()),
        format12(&[(0x8000_0000, 0x8000_0000, 1)]),
    ];
    let mut records = vec![(3, 1, 0); 8_190];
    records.extend([(3, 1, 1), (3, 10, 2)]);
    let font = font_with_cmap("Many Records", 400, false, cmap_of(&subtables, &records));
    let face = Face::parse(&font, 0).unwrap();
    assert_eq!(face.glyph('\u{F0000}'), Some(GlyphId::new(1)));
    assert_eq!(face.secondary_count(), 2);

    let text = many_secondary_characters();
    assert_no_bundled_font_has(&text[..4 * 4_096]);
    let lib = FontLibrary::with_fallbacks(vec![font]);
    let start = std::time::Instant::now();
    let layout = lib.layout(&text, &style(Font::Inter, 10.0));
    let elapsed = start.elapsed();
    let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
    assert_eq!(fonts, [FontKey::Extra(0)]);
    assert_eq!(layout.runs()[0].glyphs().len(), 50_000);
    assert!(layout.runs()[0].glyphs().iter().all(|g| g.id == 1));
    assert_fast(elapsed);
}

#[test]
fn secondary_subtables_are_capped() {
    // 8,190 distinct subtables that map nothing useful come before the one that maps the
    // text: it is past the cap, so the font does not have the characters.
    let mut subtables: Vec<Vec<u8>> = (0..8_190)
        .map(|k| format12(&[(0x4000_0000 + k, 0x4000_0000 + k, 1)]))
        .collect();
    subtables.push(format12(&secondary_hits()));
    subtables.push(format12(&[(0x8000_0000, 0x8000_0000, 1)]));
    let mut records: Vec<(u16, u16, usize)> = (0..8_191).map(|k| (3, 1, k)).collect();
    records.push((3, 10, 8_191));
    let font = font_with_cmap("Many Subtables", 400, false, cmap_of(&subtables, &records));
    let face = Face::parse(&font, 0).unwrap();
    assert_eq!(face.secondary_count(), SECONDARY_SUBTABLES);
    assert_eq!(face.glyph('\u{F0000}'), None);

    let text = many_secondary_characters();
    let lib = FontLibrary::with_fallbacks(vec![font]);
    let start = std::time::Instant::now();
    let layout = lib.layout(&text, &style(Font::Inter, 10.0));
    let elapsed = start.elapsed();
    let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
    assert_eq!(fonts, [FontKey::Bundled(Font::Inter, Weight::Regular)]);
    assert_fast(elapsed);
}

#[test]
fn format_2_deltas_wrap_around() {
    // A format 2 subtable whose sub-header for 0xE0xx maps U+E000 to `raw` plus `delta`.
    let cmap = |raw: u16, delta: i16| {
        let mut sub = Vec::new();
        for word in [2u16, 536, 0] {
            sub.extend(word.to_be_bytes());
        }
        for high in 0..256 {
            sub.extend((if high == 0xE0 { 8u16 } else { 0 }).to_be_bytes());
        }
        for word in [0u16, 0, 0, 0, 0, 1] {
            sub.extend(word.to_be_bytes());
        }
        sub.extend(delta.to_be_bytes());
        for word in [2u16, raw] {
            sub.extend(word.to_be_bytes());
        }
        let mut cmap = Vec::new();
        for word in [0u16, 1, 3, 1] {
            cmap.extend(word.to_be_bytes());
        }
        cmap.extend(12u32.to_be_bytes());
        cmap.extend(sub);
        cmap
    };
    for (raw, delta, expected) in [
        (1u16, 0i16, Some(1u32)),
        (2, -1, Some(1)),
        (65_535, 2, Some(1)),
        (1, -3, Some(65_534)),
        (3, -3, None),
    ] {
        let font = font_with_cmap("Cmap Two", 400, false, cmap(raw, delta));
        let face = Face::parse(&font, 0).unwrap();
        assert_eq!(
            face.glyph('\u{E000}').map(GlyphId::to_u32),
            expected,
            "{raw} {delta}"
        );
    }
}

#[test]
fn shaper_uses_skrifas_best_subtable_over_a_format_10_choice() {
    // Unicode full repertoire, format 12: U+F0010. Windows full repertoire, format 10:
    // only U+10000. skrifa prefers the first, HarfRust the second.
    let mut format10 = Vec::new();
    format10.extend(10u16.to_be_bytes());
    format10.extend(0u16.to_be_bytes());
    for field in [22u32, 0, 0x10000, 1] {
        format10.extend(field.to_be_bytes());
    }
    format10.extend(1u16.to_be_bytes());
    let cmap = cmap_of(
        &[format12(&[(0xF0010, 0xF0010, 1)]), format10],
        &[(0, 6, 0), (3, 10, 1)],
    );
    let font = font_with_cmap("Unique Records", 400, false, cmap);
    assert_eq!(
        Face::parse(&font, 0).unwrap().glyph('\u{F0010}'),
        Some(GlyphId::new(1))
    );
    assert_eq!(
        glyphs_as_extra_and_system(&font, "\u{F0010}"),
        [vec![1], vec![1]]
    );
}

#[test]
fn shaper_uses_skrifas_best_subtable_among_two_full_repertoire_records() {
    // Two full-repertoire subtables, each mapping one of the characters.
    let cmap = cmap_of(
        &[
            format12(&[(0xF0000, 0xF0000, 1)]),
            format12(&[(0xF0001, 0xF0001, 1)]),
        ],
        &[(3, 10, 0), (0, 4, 1)],
    );
    let font = font_with_cmap("Two Records", 400, false, cmap);
    assert_eq!(
        glyphs_as_extra_and_system(&font, "\u{F0000}\u{F0001}"),
        [vec![1, 1], vec![1, 1]]
    );
}
