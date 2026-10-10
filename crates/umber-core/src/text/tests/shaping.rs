//! Shaping callbacks: mark positioning from glyph bounds, and the per-layout budget for bounds
//! read from system font files.

use std::cell::Cell;

use harfrust::{FontFuncs, GlyphExtents, ShaperFont};
use read_fonts::types::GlyphId;
use skrifa::outline::OutlineGlyphFormat;

use super::fixtures::{be16, cff_mark_font, fixture_font, style};
use crate::fonts::Font;
use crate::geometry::Point;
use crate::text::font::{Face, FontKey, ShapingFont};
use crate::text::shaping::{EXTENTS_PER_LAYOUT, Funcs};
use crate::text::{FontLibrary, TextLayout};

#[test]
fn system_fonts_position_marks_without_gpos() {
    // A base and a combining mark, without GDEF, GSUB or GPOS: the shaper positions the
    // mark from the glyph bounds.
    let mut font = fixture_font("Mark Font", 400, false, &[0xF0010, 0x0301]);
    for i in 0..be16(&font, 4) {
        let at = 12 + 16 * i;
        if matches!(&font[at..at + 4], b"GDEF" | b"GPOS" | b"GSUB") {
            font[at + 12..at + 16].copy_from_slice(&0u32.to_be_bytes());
        }
    }
    let text = "\u{F0010}\u{301}";
    let s = style(Font::Inter, 10.0);
    let extra = FontLibrary::with_fallbacks(vec![font.clone()]).layout(text, &s);
    let system = FontLibrary::with_test_system_fonts(vec![font]).layout(text, &s);
    assert_eq!(extra.runs().len(), 1);
    assert_eq!(system.runs().len(), 1);
    assert_eq!(extra.runs()[0].font(), FontKey::Extra(0));
    assert!(matches!(system.runs()[0].font(), FontKey::System(_)));
    let mark_y = |layout: &TextLayout| layout.runs()[0].glyphs()[1].y;
    assert!(
        (mark_y(&extra) + 7.998_047).abs() < 1e-4,
        "{}",
        mark_y(&extra)
    );
    assert!(
        (mark_y(&system) + 7.998_047).abs() < 1e-4,
        "{}",
        mark_y(&system)
    );
    assert_eq!(extra.width(), system.width());
    assert_eq!(
        extra.outline(Point { x: 0.0, y: 0.0 }),
        system.outline(Point { x: 0.0, y: 0.0 })
    );
}

#[test]
fn empty_cff_glyphs_have_empty_extents() {
    let font = cff_mark_font();
    let face = Face::parse(&font, 0).unwrap();
    assert_eq!(face.outline_format(), Some(OutlineGlyphFormat::Cff));
    let tuple = |e: Option<GlyphExtents>| e.map(|e| (e.x_bearing, e.y_bearing, e.width, e.height));
    // Draws, but with no path commands: empty extents, as HarfRust gives.
    assert_eq!(tuple(face.extents(GlyphId::new(1))), Some((0, 0, 0, 0)));
    assert_eq!(
        tuple(face.extents(GlyphId::new(2))),
        Some((100, 1300, 800, -1200))
    );
    // Missing: none.
    assert_eq!(tuple(face.extents(GlyphId::new(7))), None);
    let shaping = ShapingFont::new(font.clone(), 0).unwrap();
    let shaper = ShaperFont::new(&shaping);
    for glyph in [1, 2] {
        assert_eq!(
            tuple(shaper.default_glyph_extents(GlyphId::new(glyph))),
            tuple(face.extents(GlyphId::new(glyph))),
            "glyph {glyph}"
        );
    }

    // A base, an empty mark and a visible mark, stacked by the shaper from the bounds.
    let text = "\u{F0010}\u{300}\u{301}";
    let s = style(Font::Inter, 10.0);
    let extra = FontLibrary::with_fallbacks(vec![font.clone()]).layout(text, &s);
    let system = FontLibrary::with_test_system_fonts(vec![font]).layout(text, &s);
    assert_eq!(extra.runs()[0].font(), FontKey::Extra(0));
    assert!(matches!(system.runs()[0].font(), FontKey::System(_)));
    let visible_mark_y = |layout: &TextLayout| layout.runs()[0].glyphs()[2].y;
    assert_eq!(visible_mark_y(&extra), visible_mark_y(&system));
    assert!((visible_mark_y(&system) + 7.109_375).abs() < 1e-4);
    assert!(visible_mark_y(&extra) < -6.0, "{}", visible_mark_y(&extra));
    assert_eq!(
        extra.outline(Point { x: 0.0, y: 0.0 }),
        system.outline(Point { x: 0.0, y: 0.0 })
    );
}

#[test]
fn system_extents_per_layout_are_capped() {
    let bytes = include_bytes!("../../../tests/fixtures/fallback-only.ttf").to_vec();
    let face = Face::parse(&bytes, 0).unwrap();
    let shaping = ShapingFont::new(bytes.clone(), 0).unwrap();
    let shaper = ShaperFont::new(&shaping);
    assert!(face.extents(GlyphId::new(0)).is_some());
    assert!(face.extents(GlyphId::new(1)).is_some());
    let budget = Cell::new(EXTENTS_PER_LAYOUT);
    let funcs = Funcs::system(&face, &budget);
    // 4,095 computations for glyphs the font does not have, then glyph 1: 4,096.
    for glyph in 2..u32::try_from(EXTENTS_PER_LAYOUT).unwrap() + 1 {
        assert!(funcs.glyph_extents(&shaper, GlyphId::new(glyph)).is_none());
    }
    assert!(funcs.glyph_extents(&shaper, GlyphId::new(1)).is_some());
    assert_eq!(budget.get(), 0);
    // The 4,097th computation is refused, although glyph 0 has extents.
    assert!(funcs.glyph_extents(&shaper, GlyphId::new(0)).is_none());
    // Glyphs already computed are still answered from the cache.
    assert!(funcs.glyph_extents(&shaper, GlyphId::new(1)).is_some());
    assert!(funcs.glyph_extents(&shaper, GlyphId::new(2)).is_none());
}
