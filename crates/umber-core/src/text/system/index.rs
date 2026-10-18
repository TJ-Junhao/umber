//! Discovery, coverage indexing and ranking of the system fonts.
//!
//! [`FaceIndex::build`] enumerates the fonts of a [`Source`], reads each face's character
//! coverage from its `cmap` table as sorted code point ranges within the [`Limits`], and orders
//! the faces for [`FaceIndex::find`], which yields the candidates for a character in preference
//! order.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use read_fonts::tables::cmap::{Cmap, Cmap2, CmapSubtable};
use read_fonts::types::Tag;
use read_fonts::{FontData as RawData, FontRead, FontRef};

use super::{Limits, Source};
use crate::fonts::Weight;
use crate::text::cmap::{Format2Byte, format2_byte, format2_range, is_unicode};
use crate::text::font::FontHandle;
use crate::text::guard::guarded;

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

/// One indexed system font face.
#[derive(Debug)]
pub(in crate::text) struct IndexedFace {
    id: fontdb::ID,
    /// Every family name of the face, normalized.
    families: Vec<String>,
    /// The first (English when available) family name, normalized; used for sorting.
    family: String,
    post_script_name: String,
    weight: u16,
    italic: bool,
    path: String,
    face_index: u32,
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

    /// The index of the face within its font file.
    pub(super) fn face_index(&self) -> u32 {
        self.face_index
    }
}

/// Inspection for unit tests.
#[cfg(test)]
impl IndexedFace {
    pub(in crate::text) fn post_script_name(&self) -> &str {
        &self.post_script_name
    }

    pub(in crate::text) fn weight(&self) -> u16 {
        self.weight
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
        for high in 0..=255u16 {
            let Some(byte) = format2_byte(sub, high) else {
                if !self.skip() {
                    return false;
                }
                continue;
            };
            let Some((first, count)) = format2_range(sub, byte.key()) else {
                if !self.skip() {
                    return false;
                }
                continue;
            };
            let (high, first, count) = (u32::from(high), u32::from(first), u32::from(count));
            let ok = match byte {
                Format2Byte::Single => {
                    // A single-byte code, valid if the sub-header's range contains it.
                    if first <= high && high < first + count {
                        self.visit(high, high)
                    } else {
                        self.skip()
                    }
                }
                Format2Byte::Lead(_) => {
                    let base = (high << 8) + first;
                    let end = (high << 8) + 0xFF;
                    if count > 0 && base <= end {
                        self.visit(base, (base + count - 1).min(end))
                    } else {
                        self.skip()
                    }
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
pub(in crate::text) enum CmapError {
    /// The table header cannot be read.
    Unreadable,
    /// More than the allowed number of segments or groups were visited.
    TooManyVisits,
}

/// Ranges read from a `cmap` table, unmerged, with the number of segments and groups visited.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::text) struct CmapRanges {
    ranges: Vec<(u32, u32)>,
    visited: usize,
}

/// Inspection for unit tests.
#[cfg(test)]
impl CmapRanges {
    pub(in crate::text) fn ranges(&self) -> &[(u32, u32)] {
        &self.ranges
    }

    pub(in crate::text) fn visited(&self) -> usize {
        self.visited
    }
}

/// The code point ranges a `cmap` table declares in its Unicode subtables, unmerged.
///
/// Ranges are read from the subtable headers (segments and groups) without enumerating code
/// points. Each subtable is read once, however many encoding records point at it, and every
/// segment or group visited counts against `max_visits`, including invalid ones. Code points the
/// font maps to glyph 0 are included; a lookup confirms the glyph before using a face.
pub(in crate::text) fn cmap_ranges(
    cmap: &[u8],
    max_visits: usize,
) -> Result<CmapRanges, CmapError> {
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

/// Why a face is left out of the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::text) enum SkipReason {
    /// Reading the `cmap` table needs more segment or group visits than allowed.
    TooManyVisits,
    /// The Unicode subtables declare no code points.
    NoCoverage,
    /// The face, its `cmap` table or the table header cannot be read.
    Unreadable,
    /// The face has more merged ranges than allowed.
    TooManyRanges,
}

/// The result of reading one face's coverage.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::text) enum FaceCoverage {
    /// The face covers these sorted, merged, inclusive ranges.
    Indexed {
        ranges: Vec<(u32, u32)>,
        /// Segments and groups visited, kept or not.
        visited: usize,
    },
    /// The face is left out of the index.
    Skipped {
        reason: SkipReason,
        /// Segments and groups visited, kept or not; one more than allowed when the visit
        /// budget was exceeded.
        visited: usize,
    },
}

impl FaceCoverage {
    /// Segments and groups visited, kept or not.
    fn visited(&self) -> usize {
        match self {
            FaceCoverage::Indexed { visited, .. } | FaceCoverage::Skipped { visited, .. } => {
                *visited
            }
        }
    }
}

/// The coverage of face `index` of the font file `data` as sorted, merged, inclusive ranges.
///
/// The face is skipped when it does not parse, has no Unicode coverage, needs more than
/// `max_visits` visits, or has more than `max_ranges` merged ranges.
pub(in crate::text) fn coverage(
    data: &[u8],
    index: u32,
    max_ranges: usize,
    max_visits: usize,
) -> FaceCoverage {
    let skipped = |reason, visited| FaceCoverage::Skipped { reason, visited };
    let Some(cmap) = FontRef::from_index(data, index)
        .ok()
        .and_then(|font| font.table_data(Tag::new(b"cmap")))
    else {
        return skipped(SkipReason::Unreadable, 0);
    };
    let cmap = cmap.as_bytes();
    let CmapRanges {
        mut ranges,
        visited,
    } = match cmap_ranges(cmap, max_visits) {
        Ok(found) => found,
        Err(CmapError::TooManyVisits) => {
            return skipped(SkipReason::TooManyVisits, max_visits.saturating_add(1));
        }
        Err(CmapError::Unreadable) => return skipped(SkipReason::Unreadable, 0),
    };
    ranges.sort_unstable();
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (lo, hi) in ranges {
        match merged.last_mut() {
            Some((_, last)) if lo <= last.saturating_add(1) => *last = (*last).max(hi),
            _ => merged.push((lo, hi)),
        }
    }
    if merged.is_empty() {
        return skipped(SkipReason::NoCoverage, visited);
    }
    if merged.len() > max_ranges {
        return skipped(SkipReason::TooManyRanges, visited);
    }
    merged.shrink_to_fit();
    FaceCoverage::Indexed {
        ranges: merged,
        visited,
    }
}

/// The indexed system fonts.
pub(in crate::text) struct FaceIndex {
    db: Arc<fontdb::Database>,
    faces: Vec<IndexedFace>,
    /// Normalized family name to faces, for the ranked lists.
    by_family: HashMap<String, Vec<usize>>,
    /// Every face sorted by family, then by [`IndexedFace::rank`]; one order per weight, at
    /// [`Weight::slot`].
    ordered: [Vec<usize>; Weight::ALL.len()],
    /// `cmap` segments and groups visited while building the index.
    #[cfg(test)]
    visited: usize,
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
    pub(super) fn build(source: &Source, limits: Limits) -> FaceIndex {
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
            visited = visited.saturating_add(found.visited());
            let ranges = match found {
                FaceCoverage::Indexed { ranges, .. } => ranges,
                FaceCoverage::Skipped {
                    reason: SkipReason::TooManyVisits,
                    ..
                } if remaining < limits.max_face_visits => {
                    // The build budget, not this face, ran out: index nothing more.
                    break;
                }
                FaceCoverage::Skipped { .. } => continue,
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
            let target = weight.numeric();
            let mut order: Vec<usize> = (0..faces.len()).collect();
            order.sort_by(|&a, &b| match (faces.get(a), faces.get(b)) {
                (Some(fa), Some(fb)) => {
                    (&fa.family, fa.rank(target)).cmp(&(&fb.family, fb.rank(target)))
                }
                _ => a.cmp(&b),
            });
            order
        };
        let ordered = Weight::ALL.map(order);
        FaceIndex {
            db: Arc::new(db),
            faces,
            by_family,
            ordered,
            #[cfg(test)]
            visited,
        }
    }

    /// The indexed face `face`.
    pub(super) fn face(&self, face: usize) -> Option<&IndexedFace> {
        self.faces.get(face)
    }

    /// Runs `f` on the bytes and face index of the font file of face `face`; `None` if the face
    /// is not indexed or its file cannot be read.
    pub(super) fn with_face_data<T>(
        &self,
        face: usize,
        f: impl FnOnce(&[u8], u32) -> T,
    ) -> Option<T> {
        let info = self.faces.get(face)?;
        self.db.with_face_data(info.id, f)
    }

    /// A shared handle to face `face`, which keeps the font database alive.
    pub(super) fn handle(&self, face: usize) -> Option<FontHandle> {
        let info = self.faces.get(face)?;
        Some(FontHandle::system(
            Arc::clone(&self.db),
            info.id,
            info.face_index,
        ))
    }

    /// The first candidate for `c` in preference order for which `accept` returns
    /// [`Verdict::Accept`].
    pub(super) fn find(
        &self,
        c: char,
        weight: Weight,
        mut accept: impl FnMut(usize) -> Verdict,
    ) -> Option<usize> {
        let cp = u32::from(c);
        let target = weight.numeric();
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
        let ordered = self.ordered.get(weight.slot())?;
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

/// Inspection for unit tests.
#[cfg(test)]
impl FaceIndex {
    /// The indexed faces, in index order.
    pub(in crate::text) fn faces(&self) -> &[IndexedFace] {
        &self.faces
    }

    /// `cmap` segments and groups visited while building the index.
    pub(in crate::text) fn visited(&self) -> usize {
        self.visited
    }

    pub(in crate::text) fn range_count(&self) -> usize {
        self.faces.iter().map(|f| f.ranges.len()).sum()
    }

    /// Approximate heap use of the index itself, without the font database.
    pub(in crate::text) fn heap_bytes(&self) -> usize {
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
}

/// What [`FaceIndex::find`] does with a candidate.
pub(super) enum Verdict {
    Accept,
    Reject,
    /// Give up on this character.
    Stop,
}
