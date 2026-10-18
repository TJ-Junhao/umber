//! The system font index, with in-memory fonts standing in for the system fonts: coverage
//! read from `cmap` tables, the reasons a face is left out, and the bounds on one index build.

use super::fixtures::{
    LayoutBundled, assert_no_bundled_font_has, fixture_font, font_with_cmap, format12_cmap, style,
};
use super::system_fallback::{indexed_names, system_picks};
use crate::fonts::{BundledFont, Weight};
use crate::text::FontLibrary;
use crate::text::font::{Face, FontKey};
use crate::text::system::{self, FaceCoverage, Limits, SkipReason};

/// A face mapping three code points that merge into two ranges, read with three visits.
fn three_points() -> Vec<u8> {
    fixture_font("Three Points", 400, false, &[0xF0070, 0xF0071, 0xF0073])
}

#[test]
fn coverage_of_an_indexed_face_is_sorted_and_merged() {
    let max = Limits::DEFAULT.max_face_visits();
    assert_eq!(
        system::coverage(&three_points(), 0, 2, max),
        FaceCoverage::Indexed {
            ranges: vec![(0xF0070, 0xF0071), (0xF0073, 0xF0073)],
            visited: 3,
        }
    );
}

#[test]
fn coverage_skips_faces_over_the_visit_budget() {
    let font = three_points();
    // The visit count is one past the budget, however many visits the table would need.
    assert_eq!(
        system::coverage(&font, 0, 2, 2),
        FaceCoverage::Skipped {
            reason: SkipReason::TooManyVisits,
            visited: 3,
        }
    );
    assert_eq!(
        system::coverage(&font, 0, 2, 1),
        FaceCoverage::Skipped {
            reason: SkipReason::TooManyVisits,
            visited: 2,
        }
    );
    assert!(matches!(
        system::coverage(&font, 0, 2, 3),
        FaceCoverage::Indexed { visited: 3, .. }
    ));
}

#[test]
fn coverage_skips_faces_without_coverage() {
    let max = Limits::DEFAULT.max_face_visits();
    // A group beyond U+10FFFF declares nothing but is still visited.
    let invalid = font_with_cmap("Invalid", 400, false, format12_cmap(&[0xFFFF_FFFF], 1));
    assert_eq!(
        system::coverage(&invalid, 0, 2, max),
        FaceCoverage::Skipped {
            reason: SkipReason::NoCoverage,
            visited: 1,
        }
    );
    // No Unicode subtable at all: nothing is visited.
    let none = font_with_cmap("None", 400, false, format12_cmap(&[0xF0070], 0));
    assert_eq!(
        system::coverage(&none, 0, 2, max),
        FaceCoverage::Skipped {
            reason: SkipReason::NoCoverage,
            visited: 0,
        }
    );
}

#[test]
fn coverage_skips_unreadable_faces() {
    let max = Limits::DEFAULT.max_face_visits();
    let unreadable = FaceCoverage::Skipped {
        reason: SkipReason::Unreadable,
        visited: 0,
    };
    // Not a font file.
    assert_eq!(system::coverage(b"not a font", 0, 2, max), unreadable);
    // A face index the file does not have.
    assert_eq!(system::coverage(&three_points(), 1, 2, max), unreadable);
    // A `cmap` table whose header cannot be read.
    let empty = font_with_cmap("Empty", 400, false, Vec::new());
    assert_eq!(system::coverage(&empty, 0, 2, max), unreadable);
}

#[test]
fn coverage_skips_faces_over_the_range_limit() {
    let max = Limits::DEFAULT.max_face_visits();
    // The limit applies to merged ranges: three code points, two ranges.
    assert_eq!(
        system::coverage(&three_points(), 0, 1, max),
        FaceCoverage::Skipped {
            reason: SkipReason::TooManyRanges,
            visited: 3,
        }
    );
}

#[test]
fn faces_with_too_many_ranges_are_not_indexed() {
    let target = 0x30000;
    let many: Vec<u32> = (0..65_537).map(|k| target + 2 * k).collect();
    let lib = FontLibrary::with_test_system_fonts(vec![
        fixture_font("Aaa Many", 400, false, &many),
        fixture_font("Aaa Limit", 400, false, &many[..65_536]),
        fixture_font("Zzz Few", 400, false, &[target]),
    ]);
    let c = char::from_u32(target).unwrap();
    assert_no_bundled_font_has(&c.to_string());
    // "Aaa Many" sorts first and covers the character, but has 65,537 ranges.
    assert_eq!(
        system_picks(&lib, &c.to_string(), Weight::Regular),
        ["AaaLimit-400"]
    );
    assert_eq!(indexed_names(&lib), ["AaaLimit-400", "ZzzFew-400"]);
}

#[test]
fn oversized_font_files_are_not_indexed() {
    let target = 0xF0050;
    let big = fixture_font(
        "Aaa Big",
        400,
        false,
        &(target..target + 1_000).collect::<Vec<_>>(),
    );
    let small = fixture_font("Zzz Small", 400, false, &[target]);
    let limits = Limits::DEFAULT.with_max_bytes(small.len());
    assert!(big.len() > small.len());
    let lib = FontLibrary::with_test_system_fonts_and_limits(vec![big, small], limits);
    let c = char::from_u32(target).unwrap();
    assert_eq!(
        system_picks(&lib, &c.to_string(), Weight::Regular),
        ["ZzzSmall-400"]
    );
    assert_eq!(indexed_names(&lib), ["ZzzSmall-400"]);
}

#[test]
fn index_keeps_the_first_faces_in_sorted_order() {
    let target = 0xF0060;
    let limits = Limits::DEFAULT.with_max_faces(2);
    let lib = FontLibrary::with_test_system_fonts_and_limits(
        vec![
            fixture_font("Charlie", 400, false, &[target]),
            fixture_font("Alpha", 400, false, &[target]),
            fixture_font("Bravo", 400, false, &[target]),
        ],
        limits,
    );
    assert_eq!(indexed_names(&lib), ["Alpha-400", "Bravo-400"]);
}

#[test]
fn format_2_cmaps_are_indexed() {
    // A Windows Unicode format 2 subtable mapping U+E000 to the outlined glyph 1.
    let mut sub = Vec::new();
    for word in [2u16, 536, 0] {
        sub.extend(word.to_be_bytes());
    }
    for high in 0..256 {
        sub.extend((if high == 0xE0 { 8u16 } else { 0 }).to_be_bytes());
    }
    // Sub-header 0 (empty), sub-header 1 for 0xE0xx: first code 0, one entry, delta 0,
    // range offset 2 (the glyph array right after it), then the glyph array [1].
    for word in [0u16, 0, 0, 0, 0, 1, 0, 2, 1] {
        sub.extend(word.to_be_bytes());
    }
    assert_eq!(sub.len(), 536);
    let mut cmap = Vec::new();
    for word in [0u16, 1, 3, 1] {
        cmap.extend(word.to_be_bytes());
    }
    cmap.extend(12u32.to_be_bytes());
    cmap.extend(sub);
    let read = system::cmap_ranges(&cmap, Limits::DEFAULT.max_face_visits()).unwrap();
    assert_eq!(read.ranges(), [(0xE000, 0xE000)]);
    assert_eq!(read.visited(), 256);

    let font = font_with_cmap("Format Two", 400, false, cmap);
    let face = Face::parse(&font, 0).unwrap();
    assert!(face.draws('\u{E000}'));
    let s = style(BundledFont::Inter, 10.0);
    let extra = FontLibrary::with_fallbacks(vec![font.clone()]).layout("\u{E000}", &s);
    assert_eq!(extra.runs()[0].font(), FontKey::Extra(0));
    let lib = FontLibrary::with_test_system_fonts(vec![font]);
    assert_eq!(
        system_picks(&lib, "\u{E000}", Weight::Regular),
        ["FormatTwo-400"]
    );
}

/// A `cmap` table with one Windows Unicode BMP record pointing at a format 2 subtable: the
/// 518-byte header with every sub-header key 0 except `keys` (high byte, sub-header number),
/// followed by `rest`, the sub-header bytes.
fn format2_cmap(keys: &[(usize, u16)], rest: &[u8]) -> Vec<u8> {
    let mut cmap = Vec::new();
    for word in [0u16, 1, 3, 1] {
        cmap.extend(word.to_be_bytes());
    }
    cmap.extend(12u32.to_be_bytes());
    let length = u16::try_from(518 + rest.len()).unwrap();
    for word in [2u16, length, 0] {
        cmap.extend(word.to_be_bytes());
    }
    let mut table = [0u16; 256];
    for &(high, header) in keys {
        table[high] = header * 8;
    }
    for key in table {
        cmap.extend(key.to_be_bytes());
    }
    cmap.extend(rest);
    cmap
}

#[test]
fn format_2_unreadable_sub_headers_cost_one_visit_each() {
    let max = Limits::DEFAULT.max_face_visits();
    // Sub-header 0 maps the single bytes 0x00..=0xFF.
    let all_bytes: Vec<u8> = [0u16, 256, 0, 0]
        .iter()
        .flat_map(|w| w.to_be_bytes())
        .collect();
    // (case, table, unreadable sub-headers): every high byte is one visit, whether its
    // sub-header is read or not.
    let cases = [
        ("first field missing", format2_cmap(&[], &[]), 256),
        ("count field missing", format2_cmap(&[], &[0, 0]), 256),
        (
            "key out of range",
            format2_cmap(&[(0x41, 100)], &all_bytes),
            1,
        ),
    ];
    for (case, cmap, unreadable) in cases {
        let read = system::cmap_ranges(&cmap, max).unwrap();
        assert_eq!(read.visited(), 256, "{case}");
        assert_eq!(read.ranges().len(), 256 - unreadable, "{case}");
        // The high byte whose sub-header is unreadable maps nothing.
        assert!(read.ranges().iter().all(|&(lo, _)| lo != 0x41), "{case}");
    }
    // A header with no sub-headers: 256 unreadable sub-headers fit a budget of 256, not 255.
    let empty = format2_cmap(&[], &[]);
    assert_eq!(empty.len(), 12 + 518);
    let read = system::cmap_ranges(&empty, 256).unwrap();
    assert_eq!((read.ranges().len(), read.visited()), (0, 256));
    assert_eq!(
        system::cmap_ranges(&empty, 255),
        Err(system::CmapError::TooManyVisits)
    );
}

#[test]
fn index_keeps_a_bounded_number_of_ranges() {
    let spaced = |base: u32, n: u32| -> Vec<u32> { (0..n).map(|k| base + 2 * k).collect() };
    let limits = Limits::DEFAULT.with_max_total_ranges(15);
    let lib = FontLibrary::with_test_system_fonts_and_limits(
        vec![
            fixture_font("Alpha", 400, false, &spaced(0xF5000, 10)),
            fixture_font("Bravo", 400, false, &spaced(0xF6000, 10)),
            fixture_font("Charlie", 400, false, &spaced(0xF7000, 3)),
        ],
        limits,
    );
    // Bravo does not fit after Alpha and is skipped whole; Charlie still fits.
    assert_eq!(indexed_names(&lib), ["Alpha-400", "Charlie-400"]);
    assert_eq!(lib.system().unwrap().index().range_count(), 13);
}

#[test]
fn cmap_subtables_are_read_once_and_every_visit_counts() {
    let codepoints: Vec<u32> = (0..1_000).map(|k| 0xF8000 + 2 * k).collect();
    // 64 encoding records point at one 1,000-group subtable.
    let cmap = format12_cmap(&codepoints, 64);
    let read = system::cmap_ranges(&cmap, Limits::DEFAULT.max_face_visits()).unwrap();
    assert_eq!(read.visited(), 1_000);
    assert_eq!(read.ranges().len(), 1_000);
    // Invalid groups yield nothing but are still counted.
    let invalid = format12_cmap(&[0xFFFF_FFFF], 64);
    let read = system::cmap_ranges(&invalid, Limits::DEFAULT.max_face_visits()).unwrap();
    assert_eq!((read.ranges().len(), read.visited()), (0, 1));
    assert_eq!(
        system::cmap_ranges(&cmap, 999),
        Err(system::CmapError::TooManyVisits)
    );

    let font = |name: &str| font_with_cmap(name, 400, false, format12_cmap(&codepoints, 64));
    // Per face: 1,000 visits are needed.
    let limits = Limits::DEFAULT.with_max_face_visits(999);
    let lib = FontLibrary::with_test_system_fonts_and_limits(vec![font("Alpha")], limits);
    assert!(indexed_names(&lib).is_empty());
    assert_eq!(lib.system().unwrap().index().visited(), 1_000);
    let limits = Limits::DEFAULT.with_max_face_visits(1_000);
    let lib = FontLibrary::with_test_system_fonts_and_limits(vec![font("Alpha")], limits);
    assert_eq!(indexed_names(&lib), ["Alpha-400"]);
    // Per build: the third face no longer fits and indexing stops.
    let limits = Limits::DEFAULT.with_max_build_visits(2_500);
    let lib = FontLibrary::with_test_system_fonts_and_limits(
        vec![font("Alpha"), font("Bravo"), font("Charlie"), font("Delta")],
        limits,
    );
    assert_eq!(indexed_names(&lib), ["Alpha-400", "Bravo-400"]);
    assert_eq!(lib.system().unwrap().index().visited(), 2_501);
}

#[test]
fn cmap_ranges_are_read_without_enumerating_code_points() {
    // A format 12 group spanning every code point is one range, not 1.1 million.
    let mut cmap = Vec::new();
    for field in [0u16, 1, 3, 10] {
        cmap.extend(field.to_be_bytes());
    }
    cmap.extend(12u32.to_be_bytes());
    cmap.extend(12u16.to_be_bytes());
    cmap.extend(0u16.to_be_bytes());
    cmap.extend(28u32.to_be_bytes());
    cmap.extend(0u32.to_be_bytes());
    cmap.extend(1u32.to_be_bytes());
    for field in [0u32, u32::MAX, 1] {
        cmap.extend(field.to_be_bytes());
    }
    let max = Limits::DEFAULT.max_face_visits();
    let read = system::cmap_ranges(&cmap, max).unwrap();
    assert_eq!(read.ranges(), [(0, 0x10_FFFF)]);
    assert_eq!(read.visited(), 1);
    // Truncated and empty tables give no ranges and no panic.
    assert_eq!(system::cmap_ranges(&cmap[..30], max).unwrap().ranges(), []);
    assert_eq!(
        system::cmap_ranges(&[], max),
        Err(system::CmapError::Unreadable)
    );
}

#[test]
fn font_files_past_the_loaded_bytes_limit_are_not_used() {
    let target = 0xF0060;
    let font = fixture_font("Aaa Loaded", 400, false, &[target]);
    let c = char::from_u32(target).unwrap().to_string();
    let library = |max: usize| {
        FontLibrary::with_test_system_fonts_and_limits(
            vec![font.clone()],
            Limits::DEFAULT.with_max_loaded_bytes(max),
        )
    };
    assert_eq!(
        system_picks(&library(font.len()), &c, Weight::Regular),
        ["AaaLoaded-400"]
    );
    // The face is chosen, but its file cannot be loaded: the primary font draws `.notdef`.
    let picks = system_picks(&library(font.len() - 1), &c, Weight::Regular);
    assert!(!picks.contains(&"AaaLoaded-400".to_owned()), "{picks:?}");
}
