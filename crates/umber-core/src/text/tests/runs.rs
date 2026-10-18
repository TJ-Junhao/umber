//! Splitting text into runs: marks and invisible characters, repeated fallbacks, and grouping
//! by font in linear time.

use super::fixtures::{LayoutBundled, assert_no_bundled_font_has, fixture_font, omega_lib, style};
use crate::fonts::{BundledFont, Weight};
use crate::geometry::Point;
use crate::text::FontLibrary;
use crate::text::font::FontKey;
use crate::text::marks;
use crate::text::runs::{group_by_font, is_mark};

#[test]
fn marks_and_invisible_characters_stay_in_the_previous_run() {
    let lib = omega_lib();
    let fallback = crate::fonts::FALLBACK_TEST_CODEPOINT;
    let s = style(BundledFont::Inter, 10.0);
    for text in [
        format!("{fallback}\u{301}"),
        format!("{fallback}\u{FE0F}"),
        format!("{fallback}\u{200D}"),
    ] {
        let layout = lib.layout(&text, &s);
        assert_eq!(layout.runs().len(), 1, "{text:?}");
        assert_eq!(layout.runs()[0].font(), FontKey::Extra(0), "{text:?}");
    }
    // A hidden joiner adds no width.
    let joined = lib.layout("a\u{200D}b", &s).width();
    assert!((joined - lib.layout("ab", &s).width()).abs() < 1e-4);
}

#[test]
fn repeated_fallback_characters_form_one_run() {
    let fallback = crate::fonts::FALLBACK_TEST_CODEPOINT;
    let text = format!("ab{fallback}{fallback}{fallback}cd");
    let layout = omega_lib().layout(&text, &style(BundledFont::Geist, 10.0));
    let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
    let primary = FontKey::Bundled(BundledFont::Geist, Weight::Regular);
    assert_eq!(fonts, [primary, FontKey::Extra(0), primary]);
}

#[test]
fn mark_table_is_sorted_and_disjoint() {
    let table = marks::MARKS;
    assert!(table.windows(2).all(|w| w[0].1 + 1 < w[1].0));
    assert!(table.iter().all(|&(lo, hi)| lo <= hi));
    for c in [
        '\u{0300}', '\u{0591}', '\u{0F18}', '\u{A67C}', '\u{20DD}', '\u{FE0F}',
    ] {
        assert!(is_mark(c), "{c:?}");
    }
    for c in ['a', ' ', '\u{05D0}', '\u{E000}', '\u{200D}'] {
        assert!(!is_mark(c), "{c:?}");
    }
}

#[test]
fn marks_outside_the_latin_blocks_stay_with_their_base() {
    let fallback = crate::fonts::FALLBACK_TEST_CODEPOINT;
    let lib = omega_lib();
    for mark in ['\u{0591}', '\u{A67C}', '\u{0F18}'] {
        let layout = lib.layout(
            &format!("{fallback}{mark}"),
            &style(BundledFont::Inter, 10.0),
        );
        let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
        assert_eq!(fonts, [FontKey::Extra(0)], "U+{:04X}", u32::from(mark));
    }
}

#[test]
fn grouping_runs_by_font_is_linear() {
    // 200,000 runs over 100,000 distinct fonts: a per-font rescan of all runs would take
    // 2 * 10^10 steps.
    let keys: Vec<FontKey> = (0..200_000).map(|i| FontKey::Extra(i % 100_000)).collect();
    let start = std::time::Instant::now();
    let groups = group_by_font(keys.iter().copied());
    let elapsed = start.elapsed();
    assert_eq!(groups.len(), 100_000);
    for (n, (key, members)) in groups.iter().enumerate() {
        assert_eq!(
            *key,
            FontKey::Extra(n),
            "groups keep first-appearance order"
        );
        assert_eq!(members, &[n, n + 100_000]);
    }
    // The quadratic version takes about 28 s even in release builds. Unoptimized builds
    // running the whole suite in parallel get headroom.
    let limit = std::time::Duration::from_secs(if cfg!(debug_assertions) { 10 } else { 1 });
    assert!(elapsed < limit, "{elapsed:?}");
}

#[test]
fn many_runs_and_fallback_fonts_lay_out_in_linear_time() {
    const FONTS: u32 = 1_000;
    let fonts: Vec<Vec<u8>> = (0..FONTS)
        .map(|k| fixture_font(&format!("Fallback {k}"), 400, false, &[0xF1000 + k]))
        .collect();
    let lib = FontLibrary::with_fallbacks(fonts);
    let text: String = (0..100_000u32)
        .flat_map(|i| ['a', char::from_u32(0xF1000 + i % FONTS).unwrap()])
        .collect();
    assert_no_bundled_font_has(&text.replace('a', ""));
    let s = style(BundledFont::Inter, 10.0);
    let start = std::time::Instant::now();
    let layout = lib.layout(&text, &s);
    let laid_out = start.elapsed();
    let path = layout.outline(Point { x: 0.0, y: 0.0 });
    let total = start.elapsed();
    assert_eq!(layout.runs().len(), 200_000);
    let distinct: std::collections::HashSet<FontKey> =
        layout.runs().iter().map(|r| r.font()).collect();
    assert_eq!(distinct.len(), FONTS as usize + 1);
    assert!(!path.is_empty());
    // Release builds must lay out in under a second; unoptimized test builds get headroom.
    let limit = std::time::Duration::from_secs(if cfg!(debug_assertions) { 30 } else { 1 });
    assert!(laid_out < limit, "layout {laid_out:?}");
    println!(
        "200,000 runs, {} fonts: layout {laid_out:?}, layout + outline {total:?}",
        FONTS + 1
    );
}
