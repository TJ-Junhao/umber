//! Malformed fonts that can make the parser or shaper panic.

use super::fixtures::{LayoutBundled, bit_flipped_inter, style};
use crate::fonts::BundledFont;
use crate::geometry::Point;
use crate::text::FontLibrary;
use crate::text::font::{Face, ShapingFont};
use crate::text::guard::guarded;
use crate::text::shaping::{Funcs, Shaper, features};

#[test]
fn malformed_fonts_that_panic_while_parsing_are_skipped() {
    let bytes = bit_flipped_inter();
    // This seed made rustybuzz panic with an arithmetic overflow in debug builds. HarfRust
    // and skrifa parse it without panicking, so every stage the library guards is run here
    // under the same guard: whatever they do with these bytes, nothing may escape.
    let _ = guarded(|| {
        let face = Face::parse(&bytes, 0)?;
        let font = ShapingFont::new(bytes.clone(), 0)?;
        let funcs = Funcs::new(&face);
        let features = features(true);
        let shaped = Shaper::new(&font, &funcs, &features).shape("Ab 0123 fi AV", false);
        Some((face.draws('A'), shaped.map(|run| run.glyphs.len())))
    });
    // A panic inside a guarded call is turned into "unreadable".
    assert_eq!(guarded(|| -> bool { panic!("a malformed font") }), None);
    let s = style(BundledFont::Geist, 10.0);
    let fallback = crate::fonts::FALLBACK_TEST_CODEPOINT;
    let text = format!("Ab{fallback}\u{4E00}");

    let lib = FontLibrary::with_fallbacks(vec![bytes.clone()]);
    let layout = lib.layout(&text, &s);
    assert!(layout.width() > 0.0);
    assert!(!layout.outline(Point { x: 0.0, y: 0.0 }).is_empty());

    let lib = FontLibrary::with_test_system_fonts(vec![bytes]);
    let layout = lib.layout(&text, &s);
    assert!(layout.width() > 0.0);
    assert!(!layout.outline(Point { x: 0.0, y: 0.0 }).is_empty());
}

#[test]
fn building_a_library_while_unwinding_does_not_abort() {
    struct BuildOnDrop;
    impl Drop for BuildOnDrop {
        fn drop(&mut self) {
            let fixture = include_bytes!("../../../tests/fixtures/fallback-only.ttf").to_vec();
            let lib = FontLibrary::with_fallbacks(vec![fixture]);
            assert_eq!(lib.extra_count(), 1);
        }
    }
    let caught = std::panic::catch_unwind(|| {
        let _guard = BuildOnDrop;
        panic!("host panic, caught outside the Drop");
    });
    assert!(caught.is_err());
}
