//! Measurement of laid-out text: widths, features, degenerate input and thread safety.

use super::fixtures::{LayoutBundled, omega_lib, style};
use crate::fonts::BundledFont;
use crate::geometry::Point;
use crate::text::{FontLibrary, TextLayout, TextStyle};

#[test]
fn width_scales_linearly_with_size() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let a = lib
        .layout("Signal 2026", &style(BundledFont::Inter, 10.0))
        .width();
    let b = lib
        .layout("Signal 2026", &style(BundledFont::Inter, 20.0))
        .width();
    assert!(a > 0.0 && (b - 2.0 * a).abs() < 1e-3 * b, "{a} {b}");
}

#[test]
fn kerning_is_applied() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let s = style(BundledFont::Inter, 20.0);
    let av = lib.layout("AV", &s).width();
    let sep = lib.layout("A", &s).width() + lib.layout("V", &s).width();
    assert!(av < sep, "AV {av} vs A+V {sep}");
}

#[test]
fn tabular_figures_have_equal_widths() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    for font in BundledFont::ALL {
        let s = TextStyle {
            tabular_figures: true,
            ..style(font, 10.0)
        };
        let one = lib.layout("1111", &s).width();
        let eight = lib.layout("8888", &s).width();
        assert!((one - eight).abs() < 1e-3, "{font:?}: {one} vs {eight}");
    }
}

#[test]
fn empty_and_degenerate_sizes() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let empty = lib.layout("", &style(BundledFont::Geist, 10.0));
    assert_eq!(empty.width(), 0.0);
    assert!(empty.ascent() > 0.0);
    assert!(empty.outline(Point { x: 0.0, y: 0.0 }).is_empty());
    for size in [0.0, -3.0, f32::NAN] {
        let l = lib.layout("abc", &style(BundledFont::Geist, size));
        assert_eq!(l.width(), 0.0);
        assert!(l.outline(Point { x: 0.0, y: 0.0 }).is_empty());
    }
}

#[test]
fn unsupported_text_never_panics() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    for text in [
        "数据可视化",
        "👍🏽",
        "مرحبا",
        "e\u{301}",
        "\u{FE0F}",
        "a\u{200D}b",
    ] {
        let l = lib.layout(text, &style(BundledFont::IbmPlexSans, 10.0));
        assert!(l.width() > 0.0, "{text:?}");
        let _ = l.outline(Point { x: 0.0, y: 0.0 });
    }
}

#[test]
fn concurrent_layout_is_deterministic() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let s = style(BundledFont::SourceSerif4, 11.0);
    let reference = lib
        .layout("Revenue −12.5%", &s)
        .outline(Point { x: 0.0, y: 0.0 });
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..50 {
                    let p = lib
                        .layout("Revenue −12.5%", &s)
                        .outline(Point { x: 0.0, y: 0.0 });
                    assert_eq!(p.verbs(), reference.verbs());
                }
            });
        }
    });
}

#[test]
fn long_text_layout_is_fast_enough() {
    let lib = omega_lib();
    let text: String = format!("{}数a", crate::fonts::FALLBACK_TEST_CODEPOINT).repeat(3_334);
    let start = std::time::Instant::now();
    let l = lib.layout(&text, &style(BundledFont::Inter, 10.0));
    assert!(l.width() > 0.0);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn global_library_is_shared() {
    assert!(std::ptr::eq(FontLibrary::global(), FontLibrary::global()));
}

#[test]
fn font_library_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<FontLibrary>();
    assert_send_sync::<TextLayout>();
}

#[test]
fn layout_is_empty_only_without_glyphs() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    assert!(lib.layout("", &style(BundledFont::Inter, 10.0)).is_empty());
    assert!(
        lib.layout("abc", &style(BundledFont::Inter, 0.0))
            .is_empty()
    );
    assert!(
        !lib.layout("abc", &style(BundledFont::Inter, 10.0))
            .is_empty()
    );
    assert!(
        lib.layout("", &style(BundledFont::Inter, f32::INFINITY))
            .ascent()
            == 0.0
    );
}
