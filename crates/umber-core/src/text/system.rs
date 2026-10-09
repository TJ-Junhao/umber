//! System font fallback.
//!
//! The system fonts are enumerated and indexed once, the first time a character is missing from
//! every bundled and explicit fallback font. The index records each face's character coverage as
//! sorted code point ranges, so a lookup never opens a font file except to confirm the chosen
//! face. Candidates are tried in a fixed preference order: a ranked family list for the
//! character's script, then a generic list, then every other face sorted by name. Within a
//! family the face closest to the requested weight wins, upright before italic.
//!
//! Font data is never copied: fontdb memory-maps a font file each time it is read, and layouts
//! keep the database alive through an `Arc`.
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
//! - the lookup cache holds at most [`CACHE_LIMIT`] entries.
//!
//! These caps apply after fontdb's own discovery: `load_system_fonts` enumerates every font in
//! the system font directories and keeps its metadata, bounded only by those directories.
//!
//! Concurrent misses on the same character are not merged; each one is bounded by the caps
//! above and cheap.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use rustybuzz::ttf_parser;

use super::{FontData, FontHandle, draws, guarded, lock};
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
}

impl Limits {
    pub(super) const DEFAULT: Limits = Limits {
        max_ranges: 65_536,
        max_bytes: 256 * 1024 * 1024,
        max_faces: 10_000,
        max_total_ranges: 2_097_152,
        max_face_visits: 262_144,
        max_build_visits: 8_388_608,
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

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes([*bytes.first()?, *bytes.get(1)?]))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at.checked_add(4)?)?;
    let mut word = [0; 4];
    word.copy_from_slice(bytes);
    Some(u32::from_be_bytes(word))
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

    /// Reads one subtable. Returns false once the visit budget is exceeded.
    fn subtable(&mut self, sub: &[u8]) -> bool {
        match u16_at(sub, 0) {
            Some(0) => self.visit(0, 255),
            Some(2) => self.format2(sub),
            Some(4) => {
                let segments2 = usize::from(u16_at(sub, 6).unwrap_or(0));
                for s in 0..segments2 / 2 {
                    let end = u16_at(sub, 14 + 2 * s);
                    let start = u16_at(sub, 16 + segments2 + 2 * s);
                    let (Some(start), Some(end)) = (start, end) else {
                        break;
                    };
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
            Some(6) => match (u16_at(sub, 6), u16_at(sub, 8)) {
                (Some(first), Some(count)) if count > 0 => {
                    let first = u32::from(first);
                    self.visit(first, first + u32::from(count) - 1)
                }
                _ => self.skip(),
            },
            Some(10) => match (u32_at(sub, 12), u32_at(sub, 16)) {
                (Some(first), Some(count)) if count > 0 => {
                    self.visit(first, first.saturating_add(count - 1))
                }
                _ => self.skip(),
            },
            Some(12 | 13) => {
                let groups = u32_at(sub, 12).unwrap_or(0);
                for g in 0..usize::try_from(groups).unwrap_or(0) {
                    let at = 16 + 12 * g;
                    let (Some(start), Some(end)) = (u32_at(sub, at), u32_at(sub, at + 4)) else {
                        break;
                    };
                    if !self.visit(start, end) {
                        return false;
                    }
                }
                true
            }
            _ => true,
        }
    }

    /// Format 2 (high-byte mapping): 256 sub-header keys, one visit each.
    fn format2(&mut self, sub: &[u8]) -> bool {
        const HEADERS: usize = 6 + 2 * 256;
        for high in 0..256u16 {
            let Some(key) = u16_at(sub, 6 + 2 * usize::from(high)) else {
                return self.skip();
            };
            let header = HEADERS + 8 * usize::from(key / 8);
            let (Some(first), Some(count)) = (u16_at(sub, header), u16_at(sub, header + 2)) else {
                if !self.skip() {
                    return false;
                }
                continue;
            };
            let (first, count) = (u32::from(first), u32::from(count));
            let ok = if key / 8 == 0 {
                // A single-byte code, valid if the sub-header's range contains it.
                let code = u32::from(high);
                if first <= code && code < first + count {
                    self.visit(code, code)
                } else {
                    self.skip()
                }
            } else {
                let base = (u32::from(high) << 8) + first;
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
    let records = u16_at(cmap, 2).ok_or(CmapError::Unreadable)?;
    let mut scan = RangeScan {
        ranges: Vec::new(),
        visited: 0,
        max_visits,
    };
    let mut seen: HashSet<u32> = HashSet::new();
    for record in 0..usize::from(records) {
        let at = 4 + 8 * record;
        let (Some(platform), Some(encoding), Some(offset)) =
            (u16_at(cmap, at), u16_at(cmap, at + 2), u32_at(cmap, at + 4))
        else {
            break;
        };
        let unicode = platform == 0 || (platform == 3 && (encoding == 1 || encoding == 10));
        if !unicode || !seen.insert(offset) {
            continue;
        }
        let Some(sub) = usize::try_from(offset).ok().and_then(|o| cmap.get(o..)) else {
            continue;
        };
        if !scan.subtable(sub) {
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
    let Some(cmap) = ttf_parser::RawFace::parse(data, index)
        .ok()
        .and_then(|raw| raw.table(ttf_parser::Tag::from_bytes(b"cmap")))
    else {
        return skipped(0, false);
    };
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
    index: OnceLock<FaceIndex>,
    /// `(character, weight)` to the chosen face, or `None` when no face qualifies.
    cache: Mutex<HashMap<(char, Weight), Option<usize>>>,
    /// Uncached lookups performed, for tests.
    lookups: AtomicUsize,
    /// Candidate confirmations performed, for tests.
    confirmations: AtomicUsize,
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
        }
    }

    pub(super) fn index(&self) -> &FaceIndex {
        self.index
            .get_or_init(|| FaceIndex::build(&self.source, self.limits))
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
                    && guarded(|| ttf_parser::Face::parse(data, i).is_ok_and(|f| draws(&f, c)))
                        .unwrap_or(false)
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

    #[cfg(test)]
    pub(super) fn cache_len(&self) -> usize {
        lock(&self.cache).len()
    }
}
