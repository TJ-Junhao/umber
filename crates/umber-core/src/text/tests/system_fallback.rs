//! System font fallback, with in-memory fonts standing in for the system fonts: preference
//! order, lookup and confirmation budgets, caches, shaping tables and index sharing. The index
//! itself is tested in [`system_index`](super::system_index).

use std::sync::Arc;

use read_fonts::TableProvider;

use super::fixtures::{
    assert_no_bundled_font_has, fixture_font, style, with_table, without_outlines,
};
use crate::fonts::{Font, Weight};
use crate::geometry::Point;
use crate::text::font::FontKey;
use crate::text::system::{self, Limits};
use crate::text::{FontLibrary, TextStyle};

/// The PostScript names of the system faces the runs of `text` were drawn with.
pub(super) fn system_picks(lib: &FontLibrary, text: &str, weight: Weight) -> Vec<String> {
    let s = TextStyle {
        weight,
        ..style(Font::Inter, 10.0)
    };
    let system = lib.system().unwrap();
    lib.layout(text, &s)
        .runs()
        .iter()
        .map(|run| match run.font() {
            FontKey::System(face) => system.face(face).unwrap().post_script_name().to_string(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn lookups(lib: &FontLibrary) -> usize {
    lib.system().unwrap().lookups()
}

/// The sorted PostScript names of the indexed system faces.
pub(super) fn indexed_names(lib: &FontLibrary) -> Vec<String> {
    let index = lib.system().unwrap().index();
    let mut names: Vec<String> = index
        .faces()
        .iter()
        .map(|f| f.post_script_name().to_string())
        .collect();
    names.sort();
    names
}

/// 32 system fonts that claim `codepoints` but have no outlines.
fn outline_free_candidates(codepoints: &[u32]) -> FontLibrary {
    FontLibrary::with_test_system_fonts(
        (0..32)
            .map(|k| {
                without_outlines(fixture_font(
                    &format!("Claim {k:02}"),
                    400,
                    false,
                    codepoints,
                ))
            })
            .collect(),
    )
}

#[test]
fn system_fallback_prefers_ranked_families_then_sorted_names() {
    let han = '\u{4E00}';
    let (generic, unranked) = ('\u{F0010}', '\u{F0020}');
    assert_no_bundled_font_has(&format!("{han}{generic}{unranked}"));
    // Database order puts the losing face first every time.
    let lib = FontLibrary::with_test_system_fonts(vec![
        fixture_font("Aardvark Sans", 400, false, &[0x4E00, 0xF0010]),
        fixture_font("Zulu Sans", 400, false, &[0xF0020]),
        fixture_font("Hiragino Sans", 400, false, &[0x4E00]),
        fixture_font("PingFang SC", 400, false, &[0x4E00]),
        fixture_font("DejaVu Sans", 400, false, &[0xF0010]),
        fixture_font("Noto Sans", 400, false, &[0xF0010]),
        fixture_font("Bravo Sans", 400, false, &[0xF0020]),
    ]);
    // Han: PingFang SC is ranked above Hiragino Sans; Aardvark is unranked.
    assert_eq!(
        system_picks(&lib, &han.to_string(), Weight::Regular),
        ["PingFangSC-400"]
    );
    // Generic list: Noto Sans before DejaVu Sans, both before unranked families.
    assert_eq!(
        system_picks(&lib, &generic.to_string(), Weight::Regular),
        ["NotoSans-400"]
    );
    // Unranked: by family name, not database order.
    assert_eq!(
        system_picks(&lib, &unranked.to_string(), Weight::Regular),
        ["BravoSans-600".replace("600", "400")]
    );
}

#[test]
fn system_fallback_matches_weight_then_upright_style() {
    let c = '\u{4E01}';
    assert_no_bundled_font_has(&c.to_string());
    let lib = FontLibrary::with_test_system_fonts(vec![
        fixture_font("PingFang SC", 700, false, &[0x4E01]),
        fixture_font("PingFang SC", 600, true, &[0x4E01]),
        fixture_font("PingFang SC", 300, false, &[0x4E01]),
        fixture_font("PingFang SC", 600, false, &[0x4E01]),
        fixture_font("PingFang SC", 400, true, &[0x4E01]),
        fixture_font("PingFang SC", 400, false, &[0x4E01]),
    ]);
    let text = c.to_string();
    assert_eq!(
        system_picks(&lib, &text, Weight::Regular),
        ["PingFangSC-400"]
    );
    assert_eq!(
        system_picks(&lib, &text, Weight::SemiBold),
        ["PingFangSC-600"]
    );
    // Unranked families follow the same rule.
    let lib = FontLibrary::with_test_system_fonts(vec![
        fixture_font("Other", 400, true, &[0xF0030]),
        fixture_font("Other", 800, false, &[0xF0030]),
        fixture_font("Other", 500, false, &[0xF0030]),
    ]);
    assert_eq!(
        system_picks(&lib, "\u{F0030}", Weight::SemiBold),
        ["Other-500"]
    );
    // Weight distance comes before style: 400 italic beats 500 upright for Regular.
    assert_eq!(
        system_picks(&lib, "\u{F0030}", Weight::Regular),
        ["Other-400Italic"]
    );
}

#[test]
fn system_lookups_per_layout_are_capped() {
    let codepoints: Vec<u32> = (0xF0100..0xF0100 + 1_100).collect();
    let text: String = codepoints
        .iter()
        .filter_map(|&cp| char::from_u32(cp))
        .collect();
    assert_no_bundled_font_has(&text);
    let lib =
        FontLibrary::with_test_system_fonts(vec![fixture_font("Cover", 400, false, &codepoints)]);
    let layout = lib.layout(&text, &style(Font::Inter, 10.0));
    assert_eq!(lookups(&lib), system::LOOKUPS_PER_LAYOUT);
    // The first 1,024 characters use the system font, the rest the primary font's .notdef.
    let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
    assert!(
        matches!(
            fonts.as_slice(),
            [
                FontKey::System(_),
                FontKey::Bundled(Font::Inter, Weight::Regular)
            ]
        ),
        "{fonts:?}"
    );
    assert_eq!(layout.runs()[0].glyphs().len(), 1_024);
    assert_eq!(layout.runs()[1].glyphs().len(), 76);
    // The next call looks up only the 76 characters that were not cached.
    let again = lib.layout(&text, &style(Font::Inter, 10.0));
    assert_eq!(lookups(&lib), 1_100);
    assert_eq!(again.runs().len(), 1);
}

#[test]
fn system_lookups_are_cached_across_layouts() {
    let hit: String = (0xF0200..0xF0200 + 500)
        .filter_map(char::from_u32)
        .collect();
    let miss: String = (0xF0400..0xF0400 + 300)
        .filter_map(char::from_u32)
        .collect();
    let text = format!("{hit}{miss}");
    assert_no_bundled_font_has(&text);
    let covered: Vec<u32> = (0xF0200..0xF0200 + 500).collect();
    let lib =
        FontLibrary::with_test_system_fonts(vec![fixture_font("Cover", 400, false, &covered)]);
    let s = style(Font::Inter, 10.0);
    let layout = lib.layout(&text, &s);
    assert_eq!(lookups(&lib), 800);
    let fonts: Vec<(FontKey, usize)> = layout
        .runs()
        .iter()
        .map(|r| (r.font(), r.glyphs().len()))
        .collect();
    assert!(
        matches!(
            fonts.as_slice(),
            [(FontKey::System(_), 500), (FontKey::Bundled(..), 300)]
        ),
        "{fonts:?}"
    );
    let first = layout.outline(Point { x: 0.0, y: 0.0 });
    // Found and not-found results are both cached.
    let second = lib.layout(&text, &s).outline(Point { x: 0.0, y: 0.0 });
    assert_eq!(lookups(&lib), 800);
    assert_eq!(first, second);
    // The weight is part of the key.
    let _ = lib.layout(
        &hit,
        &TextStyle {
            weight: Weight::SemiBold,
            ..s
        },
    );
    assert_eq!(lookups(&lib), 1_300);
}

#[test]
fn system_lookup_cache_is_bounded() {
    let lib = FontLibrary::with_test_system_fonts(vec![]);
    let system = lib.system().unwrap();
    let s = style(Font::Inter, 10.0);
    let total = system::CACHE_LIMIT + 4_000;
    let chars: Vec<char> = (0xF0000..).filter_map(char::from_u32).take(total).collect();
    for chunk in chars.chunks(system::LOOKUPS_PER_LAYOUT) {
        let text: String = chunk.iter().collect();
        let _ = lib.layout(&text, &s);
    }
    assert_eq!(system.lookups(), total);
    assert_eq!(system.cache_len(), system::CACHE_LIMIT);
    // Characters that did not fit are looked up again; cached ones are not.
    let late: String = chars[total - 10..].iter().collect();
    let early: String = chars[..10].iter().collect();
    let _ = lib.layout(&format!("{early}{late}"), &s);
    assert_eq!(system.lookups(), total + 10);
    assert_eq!(system.cache_len(), system::CACHE_LIMIT);
}

#[test]
fn system_shaping_tables_are_kept_within_bounds() {
    let n = u32::try_from(system::SHAPING_CACHE_FACES).unwrap() + 1;
    let codepoints: Vec<u32> = (0..n).map(|k| 0xF9000 + k).collect();
    let text: String = codepoints
        .iter()
        .filter_map(|&cp| char::from_u32(cp))
        .collect();
    assert_no_bundled_font_has(&text);
    let lib = FontLibrary::with_test_system_fonts(
        codepoints
            .iter()
            .enumerate()
            .map(|(k, &cp)| fixture_font(&format!("Face {k:03}"), 400, false, &[cp]))
            .collect(),
    );
    let system = lib.system().unwrap();
    let s = style(Font::Inter, 10.0);
    let first = lib.layout(&text, &s);
    // Every character is drawn with its own system face.
    assert_eq!(first.runs().len(), codepoints.len());
    assert!(
        first
            .runs()
            .iter()
            .all(|run| matches!(run.font(), FontKey::System(_)))
    );
    let (faces, bytes) = system.shaping_cache();
    assert_eq!(faces, system::SHAPING_CACHE_FACES);
    assert!(bytes > 0 && bytes <= system::SHAPING_CACHE_BYTES, "{bytes}");
    // Kept faces shape the same as fresh ones, and nothing more is kept.
    let again = lib.layout(&text, &s);
    assert_eq!(first.width(), again.width());
    assert_eq!(
        first.outline(Point { x: 0.0, y: 0.0 }),
        again.outline(Point { x: 0.0, y: 0.0 })
    );
    assert_eq!(system.shaping_cache(), (faces, bytes));
}

#[test]
fn kept_shaping_fonts_charge_later_table_copies() {
    let text = "\u{F9100}";
    assert_no_bundled_font_has(text);
    let font = fixture_font("Lazy Tables", 400, false, &[0xF9100]);
    let font = with_table(&font, *b"kern", vec![0; 4_000]);
    let s = style(Font::Inter, 10.0);
    let probe = FontLibrary::with_test_system_fonts(vec![font.clone()]);
    let _ = probe.layout(text, &s);
    let (_, all) = probe.system().unwrap().shaping_cache();
    assert!(all > 4_000, "{all}");

    for budget in [all, all - 1, 1_000] {
        let limits = Limits::DEFAULT.with_shaping_cache_bytes(budget);
        let lib = FontLibrary::with_test_system_fonts_and_limits(vec![font.clone()], limits);
        let system = lib.system().unwrap();
        assert_eq!(system.index().faces().len(), 1);
        // Kept before it has copied anything, so every table is copied afterwards.
        let fresh = system.shaping_font(0).unwrap();
        system.keep_shaping_font(0, fresh);
        assert_eq!(system.shaping_cache(), (1, 0));
        let layout = lib.layout(text, &s);
        assert_eq!(layout.runs()[0].font(), FontKey::System(0));
        assert!(layout.width() > 0.0);
        let (faces, bytes) = system.shaping_cache();
        assert_eq!(faces, 1);
        assert!(bytes <= budget, "{bytes} > {budget}");
        let kern = system.shaping_font(0).unwrap().font.tables().kern().is_ok();
        if budget == all {
            assert_eq!((bytes, kern), (all, true));
        } else {
            assert!(bytes > 0 && bytes < budget.min(all), "{bytes}");
        }
        if budget < 4_000 {
            assert!(!kern, "`kern` does not fit in {budget} bytes");
        }
    }
}

#[test]
fn bundled_libraries_share_one_system_index() {
    let a = FontLibrary::bundled();
    let b = FontLibrary::bundled();
    let index = |lib: &FontLibrary| Arc::clone(lib.system().unwrap().shared_index());
    assert!(Arc::ptr_eq(&index(&a), &index(&b)));
    assert!(Arc::ptr_eq(&index(&a), &index(FontLibrary::global())));
    // Libraries with in-memory system fonts keep their own.
    let c = FontLibrary::with_test_system_fonts(vec![]);
    let d = FontLibrary::with_test_system_fonts(vec![]);
    assert!(!Arc::ptr_eq(&index(&c), &index(&d)));
    assert!(!Arc::ptr_eq(&index(&a), &index(&c)));
}

#[test]
fn confirmations_per_lookup_are_capped() {
    let codepoints: Vec<u32> = (0xF3000..0xF3000 + 64).collect();
    let text: String = codepoints
        .iter()
        .filter_map(|&cp| char::from_u32(cp))
        .collect();
    assert_no_bundled_font_has(&text);
    let lib = outline_free_candidates(&codepoints);
    let system = lib.system().unwrap();
    let layout = lib.layout(&text, &style(Font::Inter, 10.0));
    assert_eq!(system.lookups(), 64);
    assert_eq!(
        system.confirmations(),
        64 * system::CONFIRMATIONS_PER_LOOKUP
    );
    assert!(system.confirmations() <= system::CONFIRMATIONS_PER_LAYOUT);
    let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
    assert_eq!(fonts, [FontKey::Bundled(Font::Inter, Weight::Regular)]);
    // A lookup that used up its own confirmations is cached.
    let _ = lib.layout(&text, &style(Font::Inter, 10.0));
    assert_eq!(system.lookups(), 64);
}

#[test]
fn confirmations_per_layout_are_capped() {
    let codepoints: Vec<u32> = (0xF4000..0xF4000 + 300).collect();
    let text: String = codepoints
        .iter()
        .filter_map(|&cp| char::from_u32(cp))
        .collect();
    assert_no_bundled_font_has(&text);
    let lib = outline_free_candidates(&codepoints);
    let system = lib.system().unwrap();
    let layout = lib.layout(&text, &style(Font::Inter, 10.0));
    // 300 characters would need 300 * 16 = 4,800 confirmations.
    assert_eq!(system.confirmations(), system::CONFIRMATIONS_PER_LAYOUT);
    assert_eq!(system.lookups(), system::CONFIRMATIONS_PER_LAYOUT / 16);
    assert!(layout.width() > 0.0);
    // Characters cut off by the layout budget are not cached; the next call continues.
    let _ = lib.layout(&text, &style(Font::Inter, 10.0));
    assert_eq!(system.lookups(), 300);
    assert_eq!(system.confirmations(), 300 * 16);
}

/// Depends on the fonts installed on the machine, so it is not run by default:
/// `cargo test -p umber-core system_fallback -- --ignored --nocapture`.
#[test]
#[ignore = "depends on the system fonts"]
fn system_fallback_draws_cjk() {
    let rss_kib = || {
        std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|o| o.trim().parse::<u64>().ok())
            .unwrap_or(0)
    };
    let lib = FontLibrary::bundled();
    let system = lib.system().expect("system fallback");
    let before = rss_kib();
    let start = std::time::Instant::now();
    let index = system.index();
    println!(
        "index: {} faces, {} ranges, ~{} KiB heap, built in {:?}, RSS +{} KiB",
        index.faces().len(),
        index.range_count(),
        index.heap_bytes() / 1024,
        start.elapsed(),
        rss_kib().saturating_sub(before)
    );
    let s = style(Font::Inter, 10.0);
    let name = |key: FontKey| match key {
        FontKey::System(face) => system.face(face).map_or_else(String::new, |f| {
            format!("{} (weight {})", f.post_script_name(), f.weight())
        }),
        other => format!("{other:?}"),
    };
    let layout = lib.layout("A数据可视化", &s);
    let fonts: Vec<FontKey> = layout.runs().iter().map(|run| run.font()).collect();
    println!(
        "runs: {:?}, width {}",
        fonts.iter().map(|&k| name(k)).collect::<Vec<_>>(),
        layout.width()
    );
    assert!(matches!(
        fonts.as_slice(),
        [FontKey::Bundled(..), FontKey::System(_)]
    ));
    let path = layout.outline(Point { x: 0.0, y: 0.0 });
    let a_only = lib.layout("A", &s).width();
    assert!(path.bounds().expect("outline").1.x > a_only + 30.0);
    let semibold = lib.layout(
        "数",
        &TextStyle {
            weight: Weight::SemiBold,
            ..s
        },
    );
    println!(
        "SemiBold: {:?}",
        semibold
            .runs()
            .iter()
            .map(|r| name(r.font()))
            .collect::<Vec<_>>()
    );

    let distinct: String = (0x4E00..0x4E00 + 1024).filter_map(char::from_u32).collect();
    let lookups = system.lookups();
    let start = std::time::Instant::now();
    let cold = lib.layout(&distinct, &s);
    println!(
        "1,024 distinct CJK characters, cold: {:?} ({} lookups)",
        start.elapsed(),
        system.lookups() - lookups
    );
    let start = std::time::Instant::now();
    let warm = lib.layout(&distinct, &s);
    println!("same, warm: {:?}", start.elapsed());
    assert!(cold.width() > 0.0 && (cold.width() - warm.width()).abs() < 1e-3);
    println!(
        "RSS after layouts: +{} KiB",
        rss_kib().saturating_sub(before)
    );

    // Emoji and Arabic must not panic, whatever the system offers.
    for text in [
        "👍🏽",
        "مرحبا",
        "שלום",
        "한국어",
        "かな",
        "e\u{301}",
        "\u{FE0F}",
        "a\u{200D}b",
    ] {
        let l = lib.layout(text, &s);
        println!(
            "{text:?}: width {}, runs {:?}",
            l.width(),
            l.runs().iter().map(|r| name(r.font())).collect::<Vec<_>>()
        );
        assert!(l.width() > 0.0, "{text:?}");
        let path = l.outline(Point { x: 0.0, y: 0.0 });
        println!(
            "  outline: {} verbs, bounds {:?}",
            path.verbs().len(),
            path.bounds()
        );
    }
}
