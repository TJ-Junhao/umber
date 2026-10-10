//! System font fallback.
//!
//! The system fonts are enumerated and indexed once per process, the first time a character is
//! missing from every bundled and explicit fallback font; every library that uses them shares
//! that index, while lookup and shaping caches stay per library. The index records each face's character coverage as
//! sorted code point ranges, so a lookup never opens a font file except to confirm the chosen
//! face. Candidates are tried in a fixed preference order: a ranked family list for the
//! character's script, then a generic list, then every other face sorted by name. Within a
//! family the face closest to the requested weight wins, upright before italic.
//!
//! Font files are never copied whole: fontdb memory-maps a font file each time it is read, and
//! layouts keep the database alive through an `Arc`. The shaper needs data it can own, so the
//! tables it reads (character map, metrics and layout tables, never outlines or bitmaps) are
//! copied for each chosen face, and kept for later layouts within a fixed budget. A kept face
//! is shaped from the tables of the file as it was when they were copied.
//!
//! Work and memory are bounded:
//! - the index holds at most [`Limits::max_faces`] faces and [`Limits::max_total_ranges`]
//!   coverage ranges in total; it skips faces larger than [`Limits::max_bytes`] and faces with
//!   more than [`Limits::max_ranges`] ranges;
//! - reading `cmap` tables visits each subtable once and at most [`Limits::max_face_visits`]
//!   segments or groups per face and [`Limits::max_build_visits`] per index build;
//! - one layout performs at most [`LOOKUPS_PER_LAYOUT`] uncached lookups and
//!   [`CONFIRMATIONS_PER_LAYOUT`] candidate confirmations, and one lookup at most
//!   [`CONFIRMATIONS_PER_LOOKUP`];
//! - the lookup cache holds at most [`CACHE_LIMIT`] entries;
//! - shaping tables are kept for at most [`SHAPING_CACHE_FACES`] faces and
//!   [`SHAPING_CACHE_BYTES`] bytes in total, counting tables copied after a face was kept.
//!
//! These caps apply after fontdb's own discovery: `load_system_fonts` enumerates every font in
//! the system font directories and keeps its metadata, bounded only by those directories.
//!
//! Concurrent misses on the same character are not merged; each one is bounded by the caps
//! above and cheap.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use read_fonts::model::Blob;
use read_fonts::tables::cmap::{Cmap, Cmap2, CmapSubtable};
use read_fonts::types::Tag;
use read_fonts::{FontData as RawData, FontRead, FontRef};

use super::{Face, FontData, FontHandle, ShapingFont, guarded, is_unicode, lock};
use crate::fonts::Weight;

/// The most uncached system font lookups one layout call performs. Characters beyond it are
/// drawn with the primary font's `.notdef`.
pub(super) const LOOKUPS_PER_LAYOUT: usize = 1024;

/// The most `(character, weight)` entries the lookup cache holds. When it is full, new results
/// are not cached; nothing is evicted.
pub(super) const CACHE_LIMIT: usize = 65_536;

/// The most candidate confirmations (open, parse, outline check) one uncached lookup performs.
/// When they are used up the character resolves to the primary font's `.notdef`.
pub(super) const CONFIRMATIONS_PER_LOOKUP: usize = 16;

/// The most candidate confirmations one layout call performs. When they are used up the
/// remaining characters resolve to the primary font's `.notdef`, and are not cached.
pub(super) const CONFIRMATIONS_PER_LAYOUT: usize = 4096;

/// The most system faces whose shaping tables are kept between layout calls.
pub(super) const SHAPING_CACHE_FACES: usize = 64;

/// The most bytes of shaping tables kept between layout calls, over all system faces. A face
/// whose tables do not fit is shaped from a fresh copy in every layout call instead.
pub(super) const SHAPING_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// The tables that shaping a system font may copy; see [`SystemFonts::shaping_font`].
///
/// The shaper's copy has no `glyf` or `CFF`; glyph bounds for fallback mark positioning are
/// read from the full font file instead (see `SystemExtents` in the parent module).
const SHAPING_TABLES: [Tag; 18] = [
    Tag::new(b"cmap"),
    Tag::new(b"head"),
    Tag::new(b"hhea"),
    Tag::new(b"hmtx"),
    Tag::new(b"maxp"),
    Tag::new(b"OS/2"),
    Tag::new(b"GDEF"),
    Tag::new(b"GSUB"),
    Tag::new(b"GPOS"),
    Tag::new(b"kern"),
    Tag::new(b"morx"),
    Tag::new(b"mort"),
    Tag::new(b"kerx"),
    Tag::new(b"ankr"),
    Tag::new(b"trak"),
    Tag::new(b"feat"),
    Tag::new(b"ltag"),
    Tag::new(b"vhea"),
];

/// A shaping font for a system face, with its table accounting.
pub(super) struct SystemShaping {
    pub(super) font: ShapingFont,
    tables: Arc<TableAccount>,
}

/// The table bytes one shaping font has copied, and whether the font is kept.
#[derive(Default)]
struct TableAccount {
    copied: AtomicUsize,
    /// Set once the font is kept: from then on every copy is charged to the cache's bytes.
    kept: AtomicBool,
}

/// Charges `bytes` to `total` if the result stays within `cap`.
fn reserve(total: &AtomicUsize, bytes: usize, cap: usize) -> bool {
    total
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
            used.checked_add(bytes).filter(|&next| next <= cap)
        })
        .is_ok()
}

/// The system font work one layout call may still do.
#[derive(Debug)]
pub(super) struct Budget {
    lookups: usize,
    confirmations: usize,
}

impl Budget {
    pub(super) fn per_layout() -> Budget {
        Budget {
            lookups: LOOKUPS_PER_LAYOUT,
            confirmations: CONFIRMATIONS_PER_LAYOUT,
        }
    }
}

/// Bounds on the system font index.
#[derive(Debug, Clone, Copy)]
pub(super) struct Limits {
    /// Faces with more merged coverage ranges than this are skipped.
    pub(super) max_ranges: usize,
    /// Faces whose font file is larger than this many bytes are skipped.
    pub(super) max_bytes: usize,
    /// At most this many faces are indexed, the first ones in sorted order.
    pub(super) max_faces: usize,
    /// The most merged ranges kept in the whole index. A face whose ranges do not fit is
    /// skipped, not truncated.
    pub(super) max_total_ranges: usize,
    /// Faces whose `cmap` needs more segment or group visits than this are skipped.
    pub(super) max_face_visits: usize,
    /// Once this many segment or group visits have been spent, no further faces are indexed.
    pub(super) max_build_visits: usize,
    /// The most shaping table bytes kept between layout calls; see [`SHAPING_CACHE_BYTES`].
    pub(super) shaping_cache_bytes: usize,
}

impl Limits {
    pub(super) const DEFAULT: Limits = Limits {
        max_ranges: 65_536,
        max_bytes: 256 * 1024 * 1024,
        max_faces: 10_000,
        max_total_ranges: 2_097_152,
        max_face_visits: 262_144,
        max_build_visits: 8_388_608,
        shaping_cache_bytes: SHAPING_CACHE_BYTES,
    };
}

/// Ranked families for Han ideographs.
const HAN: &[&str] = &[
    "Noto Sans CJK SC",
    "Noto Sans SC",
    "Source Han Sans SC",
    "PingFang SC",
    "Microsoft YaHei",
    "Hiragino Sans",
];
/// Ranked families for Hiragana and Katakana.
const KANA: &[&str] = &[
    "Noto Sans CJK JP",
    "Noto Sans JP",
    "Hiragino Sans",
    "Meiryo",
];
/// Ranked families for Hangul.
const HANGUL: &[&str] = &[
    "Noto Sans CJK KR",
    "Noto Sans KR",
    "Apple SD Gothic Neo",
    "Malgun Gothic",
];
/// Ranked families for Arabic.
const ARABIC: &[&str] = &[
    "Noto Naskh Arabic",
    "Noto Sans Arabic",
    "Amiri",
    "Geeza Pro",
    "DejaVu Sans",
];
/// Ranked families for Hebrew.
const HEBREW: &[&str] = &["Noto Sans Hebrew", "Arial Hebrew", "DejaVu Sans"];
/// Families tried for every character after the script-specific list.
const GENERIC: &[&str] = &["Noto Sans", "DejaVu Sans", "Arial Unicode MS"];

/// The ranked family list for the script of `c`, detected from its Unicode block.
fn script_families(c: char) -> &'static [&'static str] {
    match u32::from(c) {
        0x2E80..=0x2FDF // CJK and Kangxi radicals
        | 0x3000..=0x303F // CJK symbols and punctuation
        | 0x31C0..=0x31EF // CJK strokes
        | 0x3400..=0x4DBF // Extension A
        | 0x4E00..=0x9FFF // Unified ideographs
        | 0xF900..=0xFAFF // Compatibility ideographs
        | 0x20000..=0x323AF => HAN, // Extensions B and later, compatibility supplement
        0x3040..=0x30FF // Hiragana, Katakana
        | 0x31F0..=0x31FF // Katakana phonetic extensions
        | 0xFF65..=0xFF9F // Halfwidth Katakana
        | 0x1B000..=0x1B16F => KANA, // Kana supplement and extended
        0x1100..=0x11FF // Hangul Jamo
        | 0x3130..=0x318F // Compatibility Jamo
        | 0xA960..=0xA97F // Jamo extended A
        | 0xAC00..=0xD7FF // Syllables, Jamo extended B
        | 0xFFA0..=0xFFDC => HANGUL, // Halfwidth Hangul
        0x0600..=0x06FF | 0x0750..=0x077F | 0x0870..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => {
            ARABIC
        }
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => HEBREW,
        _ => &[],
    }
}

/// Lowercase ASCII letters and digits only, so that `"Noto Sans CJK SC"` matches `"NotoSansCJKsc"`.
fn normalize(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn target_weight(weight: Weight) -> u16 {
    match weight {
        Weight::Regular => 400,
        Weight::SemiBold => 600,
    }
}

fn weight_slot(weight: Weight) -> usize {
    match weight {
        Weight::Regular => 0,
        Weight::SemiBold => 1,
    }
}

/// One indexed system font face.
#[derive(Debug)]
pub(super) struct IndexedFace {
    id: fontdb::ID,
    /// Every family name of the face, normalized.
    families: Vec<String>,
    /// The first (English when available) family name, normalized; used for sorting.
    family: String,
    pub(super) post_script_name: String,
    pub(super) weight: u16,
    pub(super) italic: bool,
    path: String,
    pub(super) face_index: u32,
    /// Sorted, merged, inclusive code point ranges from the Unicode `cmap` subtables.
    ranges: Vec<(u32, u32)>,
}

impl IndexedFace {
    fn covers(&self, cp: u32) -> bool {
        let after = self.ranges.partition_point(|&(lo, _)| lo <= cp);
        after > 0 && self.ranges.get(after - 1).is_some_and(|&(_, hi)| cp <= hi)
    }

    /// Sort key within a family: closest weight, upright first, public before hidden faces
    /// (Apple's PostScript names starting with `.`), then stable identifiers.
    fn rank(&self, target: u16) -> (u16, bool, bool, &str, &str, u32) {
        (
            self.weight.abs_diff(target),
            self.italic,
            self.post_script_name.starts_with('.'),
            &self.post_script_name,
            &self.path,
            self.face_index,
        )
    }
}

/// Collects ranges while counting every segment or group visited, kept or not.
struct RangeScan {
    ranges: Vec<(u32, u32)>,
    visited: usize,
    max_visits: usize,
}

impl RangeScan {
    /// Records one visited segment or group; keeps `lo..=hi` if it is a valid range. Returns
    /// false once the visit budget is exceeded.
    fn visit(&mut self, lo: u32, hi: u32) -> bool {
        const MAX: u32 = 0x10_FFFF;
        self.visited += 1;
        if self.visited > self.max_visits {
            return false;
        }
        if lo <= hi && lo <= MAX {
            self.ranges.push((lo, hi.min(MAX)));
        }
        true
    }

    /// Counts a visit that yields no range.
    fn skip(&mut self) -> bool {
        self.visit(1, 0)
    }

    /// Reads one subtable from its segments or groups, never its code points. Returns false
    /// once the visit budget is exceeded.
    fn subtable(&mut self, sub: &CmapSubtable<'_>) -> bool {
        match sub {
            CmapSubtable::Format0(_) => self.visit(0, 255),
            CmapSubtable::Format2(sub) => self.format2(sub),
            CmapSubtable::Format4(sub) => {
                for (start, end) in sub.start_code().iter().zip(sub.end_code()) {
                    let (start, end) = (start.get(), end.get());
                    let ok = if start == 0xFFFF && end == 0xFFFF {
                        self.skip()
                    } else {
                        self.visit(u32::from(start), u32::from(end))
                    };
                    if !ok {
                        return false;
                    }
                }
                true
            }
            CmapSubtable::Format6(sub) => match (sub.first_code(), sub.entry_count()) {
                (first, count) if count > 0 => {
                    let first = u32::from(first);
                    self.visit(first, first + u32::from(count) - 1)
                }
                _ => self.skip(),
            },
            CmapSubtable::Format10(sub) => match (sub.start_char_code(), sub.num_chars()) {
                (first, count) if count > 0 => self.visit(first, first.saturating_add(count - 1)),
                _ => self.skip(),
            },
            CmapSubtable::Format12(sub) => {
                for group in sub.groups() {
                    if !self.visit(group.start_char_code(), group.end_char_code()) {
                        return false;
                    }
                }
                true
            }
            CmapSubtable::Format13(sub) => {
                for group in sub.groups() {
                    if !self.visit(group.start_char_code(), group.end_char_code()) {
                        return false;
                    }
                }
                true
            }
            CmapSubtable::Format8(_) | CmapSubtable::Format14(_) => true,
        }
    }

    /// Format 2 (high-byte mapping): 256 sub-header keys, one visit each.
    fn format2(&mut self, sub: &Cmap2<'_>) -> bool {
        /// Format, length, language and the 256 sub-header keys.
        const HEADERS: usize = 6 + 2 * 256;
        let data = sub.offset_data();
        for (high, key) in (0..=255u32).zip(sub.sub_header_keys()) {
            let key = key.get() / 8;
            let header = HEADERS + 8 * usize::from(key);
            let (Ok(first), Ok(count)) =
                (data.read_at::<u16>(header), data.read_at::<u16>(header + 2))
            else {
                if !self.skip() {
                    return false;
                }
                continue;
            };
            let (first, count) = (u32::from(first), u32::from(count));
            let ok = if key == 0 {
                // A single-byte code, valid if the sub-header's range contains it.
                if first <= high && high < first + count {
                    self.visit(high, high)
                } else {
                    self.skip()
                }
            } else {
                let base = (high << 8) + first;
                if count > 0 && base <= 0xFFFF {
                    self.visit(base, (base + count - 1).min(0xFFFF))
                } else {
                    self.skip()
                }
            };
            if !ok {
                return false;
            }
        }
        true
    }
}

/// Why reading a `cmap` table gave no ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CmapError {
    /// The table header cannot be read.
    Unreadable,
    /// More than the allowed number of segments or groups were visited.
    TooManyVisits,
}

/// Ranges read from a `cmap` table, unmerged, with the number of segments and groups visited.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CmapRanges {
    pub(super) ranges: Vec<(u32, u32)>,
    pub(super) visited: usize,
}

/// The code point ranges a `cmap` table declares in its Unicode subtables, unmerged.
///
/// Ranges are read from the subtable headers (segments and groups) without enumerating code
/// points. Each subtable is read once, however many encoding records point at it, and every
/// segment or group visited counts against `max_visits`, including invalid ones. Code points the
/// font maps to glyph 0 are included; a lookup confirms the glyph before using a face.
pub(super) fn cmap_ranges(cmap: &[u8], max_visits: usize) -> Result<CmapRanges, CmapError> {
    let cmap = Cmap::read(RawData::new(cmap)).map_err(|_| CmapError::Unreadable)?;
    let mut scan = RangeScan {
        ranges: Vec::new(),
        visited: 0,
        max_visits,
    };
    let mut seen: HashSet<u32> = HashSet::new();
    for record in cmap.encoding_records() {
        if !is_unicode(record) || !seen.insert(record.subtable_offset().to_u32()) {
            continue;
        }
        // A subtable that cannot be read declares nothing.
        let Ok(sub) = record.subtable(cmap.offset_data()) else {
            continue;
        };
        if !scan.subtable(&sub) {
            return Err(CmapError::TooManyVisits);
        }
    }
    Ok(CmapRanges {
        ranges: scan.ranges,
        visited: scan.visited,
    })
}

/// The result of indexing one face.
struct FaceCoverage {
    /// Segments and groups visited, kept or not.
    visited: usize,
    /// Sorted, merged ranges; `None` if the face is skipped.
    ranges: Option<Vec<(u32, u32)>>,
    /// Whether the visit budget was exceeded.
    too_many_visits: bool,
}

/// The coverage of a face as sorted, merged, inclusive ranges.
///
/// The face is skipped (`ranges` is `None`) when it does not parse, has no Unicode coverage,
/// needs more than `max_visits` visits, or has more than `max_ranges` merged ranges.
fn coverage(data: &[u8], index: u32, max_ranges: usize, max_visits: usize) -> FaceCoverage {
    let skipped = |visited, too_many_visits| FaceCoverage {
        visited,
        ranges: None,
        too_many_visits,
    };
    let Some(cmap) = FontRef::from_index(data, index)
        .ok()
        .and_then(|font| font.table_data(Tag::new(b"cmap")))
    else {
        return skipped(0, false);
    };
    let cmap = cmap.as_bytes();
    let CmapRanges {
        mut ranges,
        visited,
    } = match cmap_ranges(cmap, max_visits) {
        Ok(found) => found,
        Err(CmapError::TooManyVisits) => return skipped(max_visits.saturating_add(1), true),
        Err(CmapError::Unreadable) => return skipped(0, false),
    };
    ranges.sort_unstable();
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (lo, hi) in ranges {
        match merged.last_mut() {
            Some((_, last)) if lo <= last.saturating_add(1) => *last = (*last).max(hi),
            _ => merged.push((lo, hi)),
        }
    }
    if merged.is_empty() || merged.len() > max_ranges {
        return skipped(visited, false);
    }
    merged.shrink_to_fit();
    FaceCoverage {
        visited,
        ranges: Some(merged),
        too_many_visits: false,
    }
}

/// Where the system font database comes from.
pub(super) enum Source {
    /// The fonts installed on the machine.
    Os,
    /// In-memory fonts, so tests do not depend on the machine.
    #[cfg(test)]
    Memory(Vec<Vec<u8>>),
}

/// The indexed system fonts.
pub(super) struct FaceIndex {
    db: Arc<fontdb::Database>,
    pub(super) faces: Vec<IndexedFace>,
    /// Normalized family name to faces, for the ranked lists.
    by_family: HashMap<String, Vec<usize>>,
    /// Every face sorted by family, then by [`IndexedFace::rank`]; one order per weight.
    ordered: [Vec<usize>; 2],
    /// `cmap` segments and groups visited while building the index; read by tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) visited: usize,
}

fn source_path(source: &fontdb::Source) -> String {
    match source {
        fontdb::Source::Binary(_) => String::new(),
        fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => {
            path.to_string_lossy().into_owned()
        }
    }
}

impl FaceIndex {
    fn build(source: &Source, limits: Limits) -> FaceIndex {
        let mut db = fontdb::Database::new();
        match source {
            Source::Os => db.load_system_fonts(),
            #[cfg(test)]
            Source::Memory(fonts) => {
                for font in fonts {
                    db.load_font_data(font.clone());
                }
            }
        }
        // The face cap keeps the first faces in the deterministic order of the last preference
        // tier: family name, PostScript name, path, face index.
        let mut infos: Vec<(String, &str, String, u32, &fontdb::FaceInfo)> = db
            .faces()
            // macOS's LastResort font maps every code point to a placeholder symbol.
            .filter(|info| !info.post_script_name.starts_with("LastResort"))
            .map(|info| {
                let family = info
                    .families
                    .first()
                    .map(|(f, _)| normalize(f))
                    .unwrap_or_default();
                (
                    family,
                    info.post_script_name.as_str(),
                    source_path(&info.source),
                    info.index,
                    info,
                )
            })
            .collect();
        infos.sort_by(|a, b| (&a.0, a.1, &a.2, a.3).cmp(&(&b.0, b.1, &b.2, b.3)));
        infos.truncate(limits.max_faces);

        let mut faces = Vec::new();
        let mut visited = 0usize;
        let mut total_ranges = 0usize;
        for (family, post_script_name, path, face_index, info) in infos {
            let remaining = limits.max_build_visits.saturating_sub(visited);
            let max_visits = limits.max_face_visits.min(remaining);
            let found = db
                .with_face_data(info.id, |data, index| {
                    if data.len() > limits.max_bytes {
                        return None;
                    }
                    guarded(|| coverage(data, index, limits.max_ranges, max_visits))
                })
                .flatten();
            let Some(found) = found else {
                continue;
            };
            visited = visited.saturating_add(found.visited);
            if found.too_many_visits && remaining < limits.max_face_visits {
                // The build budget, not this face, ran out: index nothing more.
                break;
            }
            let Some(ranges) = found.ranges else {
                continue;
            };
            if total_ranges + ranges.len() > limits.max_total_ranges {
                continue;
            }
            total_ranges += ranges.len();
            faces.push(IndexedFace {
                id: info.id,
                families: info.families.iter().map(|(f, _)| normalize(f)).collect(),
                family,
                post_script_name: post_script_name.to_string(),
                weight: info.weight.0,
                italic: info.style != fontdb::Style::Normal,
                path,
                face_index,
                ranges,
            });
        }
        let mut by_family: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, face) in faces.iter().enumerate() {
            let mut seen = HashSet::new();
            for family in &face.families {
                if seen.insert(family) {
                    by_family.entry(family.clone()).or_default().push(i);
                }
            }
        }
        let order = |weight: Weight| {
            let target = target_weight(weight);
            let mut order: Vec<usize> = (0..faces.len()).collect();
            order.sort_by(|&a, &b| match (faces.get(a), faces.get(b)) {
                (Some(fa), Some(fb)) => {
                    (&fa.family, fa.rank(target)).cmp(&(&fb.family, fb.rank(target)))
                }
                _ => a.cmp(&b),
            });
            order
        };
        let ordered = [order(Weight::Regular), order(Weight::SemiBold)];
        FaceIndex {
            db: Arc::new(db),
            faces,
            by_family,
            ordered,
            visited,
        }
    }

    #[cfg(test)]
    pub(super) fn range_count(&self) -> usize {
        self.faces.iter().map(|f| f.ranges.len()).sum()
    }

    /// Approximate heap use of the index itself, without the font database.
    #[cfg(test)]
    pub(super) fn heap_bytes(&self) -> usize {
        let faces: usize = self
            .faces
            .iter()
            .map(|f| {
                std::mem::size_of::<IndexedFace>()
                    + f.ranges.capacity() * std::mem::size_of::<(u32, u32)>()
                    + f.families.iter().map(|s| s.capacity() + 24).sum::<usize>()
                    + f.family.capacity()
                    + f.post_script_name.capacity()
                    + f.path.capacity()
            })
            .sum();
        let families: usize = self
            .by_family
            .iter()
            .map(|(k, v)| k.capacity() + 24 + v.capacity() * 8 + 24)
            .sum();
        faces + families + self.ordered.iter().map(|o| o.capacity() * 8).sum::<usize>()
    }

    /// The first candidate for `c` in preference order for which `accept` returns true.
    fn find(
        &self,
        c: char,
        weight: Weight,
        mut accept: impl FnMut(usize) -> Verdict,
    ) -> Option<usize> {
        let cp = u32::from(c);
        let target = target_weight(weight);
        let covers = |i: &usize| self.faces.get(*i).is_some_and(|f| f.covers(cp));
        let mut tried: HashSet<usize> = HashSet::new();
        for name in script_families(c).iter().chain(GENERIC) {
            let Some(members) = self.by_family.get(&normalize(name)) else {
                continue;
            };
            let mut members: Vec<usize> = members.iter().copied().filter(covers).collect();
            members.sort_by(|&a, &b| match (self.faces.get(a), self.faces.get(b)) {
                (Some(fa), Some(fb)) => fa.rank(target).cmp(&fb.rank(target)),
                _ => a.cmp(&b),
            });
            for face in members {
                if tried.insert(face) {
                    match accept(face) {
                        Verdict::Accept => return Some(face),
                        Verdict::Reject => {}
                        Verdict::Stop => return None,
                    }
                }
            }
        }
        let ordered = self.ordered.get(weight_slot(weight))?;
        for face in ordered.iter().copied().filter(covers) {
            if !tried.contains(&face) {
                match accept(face) {
                    Verdict::Accept => return Some(face),
                    Verdict::Reject => {}
                    Verdict::Stop => return None,
                }
            }
        }
        None
    }
}

/// What [`FaceIndex::find`] does with a candidate.
enum Verdict {
    Accept,
    Reject,
    /// Give up on this character.
    Stop,
}

/// System font fallback state of one [`FontLibrary`](super::FontLibrary).
pub(super) struct SystemFonts {
    source: Source,
    limits: Limits,
    /// For [`Source::Os`], the process-wide index shared by every library.
    index: OnceLock<Arc<FaceIndex>>,
    /// `(character, weight)` to the chosen face, or `None` when no face qualifies.
    cache: Mutex<HashMap<(char, Weight), Option<usize>>>,
    /// Uncached lookups performed, for tests.
    lookups: AtomicUsize,
    /// Candidate confirmations performed, for tests.
    confirmations: AtomicUsize,
    /// Shaping fonts kept between layout calls, by face.
    shaping: Mutex<HashMap<usize, ShapingFont>>,
    /// Table bytes held by the kept shaping fonts, at most `limits.shaping_cache_bytes`.
    shaping_bytes: Arc<AtomicUsize>,
}

impl SystemFonts {
    pub(super) fn new(source: Source, limits: Limits) -> SystemFonts {
        SystemFonts {
            source,
            limits,
            index: OnceLock::new(),
            cache: Mutex::new(HashMap::new()),
            lookups: AtomicUsize::new(0),
            confirmations: AtomicUsize::new(0),
            shaping: Mutex::new(HashMap::new()),
            shaping_bytes: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(super) fn index(&self) -> &FaceIndex {
        self.shared_index()
    }

    /// The index, built on first use. The machine's fonts are indexed once per process and
    /// shared by every library that uses them; in-memory test fonts get an index of their own.
    pub(crate) fn shared_index(&self) -> &Arc<FaceIndex> {
        self.index.get_or_init(|| match &self.source {
            Source::Os => {
                static OS: OnceLock<Arc<FaceIndex>> = OnceLock::new();
                Arc::clone(OS.get_or_init(|| Arc::new(FaceIndex::build(&Source::Os, self.limits))))
            }
            #[cfg(test)]
            Source::Memory(_) => Arc::new(FaceIndex::build(&self.source, self.limits)),
        })
    }

    /// The face to draw `c` with, if any.
    ///
    /// An uncached lookup spends one lookup from `budget` and at most
    /// [`CONFIRMATIONS_PER_LOOKUP`] of its confirmations. A lookup that uses up its own
    /// confirmations answers `None`, which is cached like any other answer. When the layout's
    /// budget runs out the answer is `None` and nothing is cached.
    pub(super) fn face_for(&self, c: char, weight: Weight, budget: &mut Budget) -> Option<usize> {
        if let Some(found) = lock(&self.cache).get(&(c, weight)) {
            return *found;
        }
        if budget.lookups == 0 || budget.confirmations == 0 {
            return None;
        }
        budget.lookups -= 1;
        self.lookups.fetch_add(1, Ordering::Relaxed);
        let index = self.index();
        let mut left = CONFIRMATIONS_PER_LOOKUP;
        let mut layout_budget_ran_out = false;
        let found = index.find(c, weight, |face| {
            if left == 0 {
                return Verdict::Stop;
            }
            if budget.confirmations == 0 {
                layout_budget_ran_out = true;
                return Verdict::Stop;
            }
            left -= 1;
            budget.confirmations -= 1;
            self.confirmations.fetch_add(1, Ordering::Relaxed);
            if self.confirm(index, face, c) {
                Verdict::Accept
            } else {
                Verdict::Reject
            }
        });
        if !layout_budget_ran_out {
            let mut cache = lock(&self.cache);
            if cache.len() < CACHE_LIMIT {
                cache.insert((c, weight), found);
            }
        }
        found
    }

    /// Opens the face and checks that it really draws `c`.
    fn confirm(&self, index: &FaceIndex, face: usize, c: char) -> bool {
        let Some(info) = index.faces.get(face) else {
            return false;
        };
        index
            .db
            .with_face_data(info.id, |data, i| {
                data.len() <= self.limits.max_bytes
                    && guarded(|| Face::parse(data, i).is_some_and(|f| f.draws(c))).unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// A shared handle to a face chosen by [`SystemFonts::face_for`].
    pub(super) fn handle(&self, face: usize) -> Option<FontHandle> {
        let index = self.index.get()?;
        let info = index.faces.get(face)?;
        Some(FontHandle {
            data: FontData::System(Arc::clone(&index.db), info.id),
            index: info.face_index,
        })
    }

    /// A shaping font for a face chosen by [`SystemFonts::face_for`].
    ///
    /// HarfRust needs font data it can own, and fontdb only lends a system font's bytes for
    /// the duration of a call. So each table the shaper asks for is copied from the font file
    /// on first use, and only the tables in [`SHAPING_TABLES`]: never the outline or bitmap
    /// tables, which make up most of a large font file. A font kept with
    /// [`SystemFonts::keep_shaping_font`] is reused; otherwise a fresh one is made.
    ///
    /// A kept font charges every table it copies later to the cache's byte budget, and once
    /// that is used up it copies nothing more: the shaper then sees those tables as missing.
    pub(super) fn shaping_font(&self, face: usize) -> Option<SystemShaping> {
        if let Some(font) = lock(&self.shaping).get(&face) {
            return Some(SystemShaping {
                font: font.clone(),
                tables: Arc::new(TableAccount::default()),
            });
        }
        let index = self.index.get()?;
        let info = index.faces.get(face)?;
        let (db, id) = (Arc::clone(&index.db), info.id);
        let account = Arc::new(TableAccount::default());
        let (own, total) = (Arc::clone(&account), Arc::clone(&self.shaping_bytes));
        let cap = self.limits.shaping_cache_bytes;
        let tables = move |tag: Tag| -> Option<Blob> {
            if !SHAPING_TABLES.contains(&tag) {
                return None;
            }
            let table = db
                .with_face_data(id, |data, index| {
                    let table = FontRef::from_index(data, index).ok()?.table_data(tag)?;
                    let bytes = table.len();
                    if own.kept.load(Ordering::Acquire) && !reserve(&total, bytes, cap) {
                        return None;
                    }
                    own.copied.fetch_add(bytes, Ordering::Relaxed);
                    Some(table.as_bytes().to_vec())
                })
                .flatten()?;
            Some(Blob::from(table))
        };
        let tables: Arc<dyn Fn(Tag) -> Option<Blob> + Send + Sync> = Arc::new(tables);
        Some(SystemShaping {
            font: ShapingFont::new(tables, info.face_index)?,
            tables: account,
        })
    }

    /// Keeps a shaping font from [`SystemFonts::shaping_font`] for later layouts, within
    /// [`SHAPING_CACHE_FACES`] faces and the cache's byte budget. Nothing is evicted.
    ///
    /// The bytes the font has copied so far are charged now, and any table it copies later is
    /// charged when it is copied, so the kept fonts never hold more than the budget.
    pub(super) fn keep_shaping_font(&self, face: usize, shaping: SystemShaping) {
        let mut fonts = lock(&self.shaping);
        if fonts.contains_key(&face) || fonts.len() >= SHAPING_CACHE_FACES {
            return;
        }
        let bytes = shaping.tables.copied.load(Ordering::Relaxed);
        if !reserve(&self.shaping_bytes, bytes, self.limits.shaping_cache_bytes) {
            return;
        }
        shaping.tables.kept.store(true, Ordering::Release);
        fonts.insert(face, shaping.font);
    }

    /// An indexed face, for diagnostics.
    #[cfg(test)]
    pub(super) fn face(&self, face: usize) -> Option<&IndexedFace> {
        self.index.get()?.faces.get(face)
    }

    #[cfg(test)]
    pub(super) fn confirmations(&self) -> usize {
        self.confirmations.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(super) fn lookups(&self) -> usize {
        self.lookups.load(Ordering::Relaxed)
    }

    /// The number of faces whose shaping tables are kept, and the bytes they hold.
    #[cfg(test)]
    pub(super) fn shaping_cache(&self) -> (usize, usize) {
        let faces = lock(&self.shaping).len();
        (faces, self.shaping_bytes.load(Ordering::Acquire))
    }

    #[cfg(test)]
    pub(super) fn cache_len(&self) -> usize {
        lock(&self.cache).len()
    }
}
