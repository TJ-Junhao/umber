//! System font fallback.
//!
//! The system fonts are enumerated and indexed once, the first time a character is missing from
//! every bundled and explicit fallback font. The index records each face's character coverage as
//! sorted code point ranges, so a lookup never opens a font file except to confirm the chosen
//! face. Candidates are tried in a fixed preference order: a ranked family list for the
//! character's script, then a generic list, then every other face sorted by name. Within a
//! family the face closest to the requested weight wins, upright before italic.
//!
//! Work is bounded: one layout performs at most [`LOOKUPS_PER_LAYOUT`] uncached lookups, and the
//! process-wide lookup cache holds at most [`CACHE_LIMIT`] entries.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use rustybuzz::ttf_parser;

use super::{NoOutline, glyph_for, lock};
use crate::fonts::Weight;

/// The most uncached system font lookups one layout call performs. Characters beyond it are
/// drawn with the primary font's `.notdef`.
pub(super) const LOOKUPS_PER_LAYOUT: usize = 1024;

/// The most `(character, weight)` entries the lookup cache holds. When it is full, new results
/// are not cached; nothing is evicted.
pub(super) const CACHE_LIMIT: usize = 65_536;

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
    /// Identifies the font file, so that faces of one collection share their loaded bytes.
    file: usize,
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

/// The coverage of a face as sorted, merged, inclusive ranges. Empty if it does not parse.
fn coverage(data: &[u8], index: u32) -> Vec<(u32, u32)> {
    let Ok(face) = ttf_parser::Face::parse(data, index) else {
        return Vec::new();
    };
    let Some(cmap) = face.tables().cmap else {
        return Vec::new();
    };
    let mut points: Vec<u32> = Vec::new();
    for subtable in cmap.subtables {
        if subtable.is_unicode() {
            subtable.codepoints(|cp| points.push(cp));
        }
    }
    points.sort_unstable();
    points.dedup();
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for cp in points {
        match ranges.last_mut() {
            Some((_, hi)) if hi.checked_add(1) == Some(cp) => *hi = cp,
            _ => ranges.push((cp, cp)),
        }
    }
    ranges.shrink_to_fit();
    ranges
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
    db: fontdb::Database,
    pub(super) faces: Vec<IndexedFace>,
    /// Normalized family name to faces, for the ranked lists.
    by_family: HashMap<String, Vec<usize>>,
    /// Every face sorted by family, then by [`IndexedFace::rank`]; one order per weight.
    ordered: [Vec<usize>; 2],
}

impl FaceIndex {
    fn build(source: &Source) -> FaceIndex {
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
        let mut faces = Vec::new();
        let mut files: HashMap<String, usize> = HashMap::new();
        for info in db.faces() {
            // macOS's LastResort font maps every code point to a placeholder symbol.
            if info.post_script_name.starts_with("LastResort") {
                continue;
            }
            let ranges = db.with_face_data(info.id, coverage).unwrap_or_default();
            if ranges.is_empty() {
                continue;
            }
            let families: Vec<String> = info.families.iter().map(|(f, _)| normalize(f)).collect();
            let path = match &info.source {
                fontdb::Source::Binary(_) => String::new(),
                fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => {
                    path.to_string_lossy().into_owned()
                }
            };
            // In-memory fonts have no path; each is its own file.
            let next_file = files.len() + faces.len();
            let file = if path.is_empty() {
                next_file
            } else {
                *files.entry(path.clone()).or_insert(next_file)
            };
            faces.push(IndexedFace {
                id: info.id,
                family: families.first().cloned().unwrap_or_default(),
                families,
                post_script_name: info.post_script_name.clone(),
                weight: info.weight.0,
                italic: info.style != fontdb::Style::Normal,
                path,
                face_index: info.index,
                file,
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
            db,
            faces,
            by_family,
            ordered,
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
        mut accept: impl FnMut(usize) -> bool,
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
                if tried.insert(face) && accept(face) {
                    return Some(face);
                }
            }
        }
        let ordered = self.ordered.get(weight_slot(weight))?;
        ordered
            .iter()
            .copied()
            .filter(covers)
            .find(|face| !tried.contains(face) && accept(*face))
    }
}

/// Whether the face has a glyph with an outline for `c` (whitespace needs no outline).
fn has_outline(data: &[u8], index: u32, c: char) -> bool {
    let Ok(face) = ttf_parser::Face::parse(data, index) else {
        return false;
    };
    glyph_for(&face, c)
        .is_some_and(|g| c.is_whitespace() || face.outline_glyph(g, &mut NoOutline).is_some())
}

/// The bytes of a chosen system face, shared between layouts and outlines.
pub(super) struct LoadedFace {
    pub(super) data: Arc<Vec<u8>>,
    pub(super) index: u32,
}

/// System font fallback state of one [`FontLibrary`](super::FontLibrary).
pub(super) struct SystemFonts {
    source: Source,
    index: OnceLock<FaceIndex>,
    /// `(character, weight)` to the chosen face, or `None` when no face qualifies.
    cache: Mutex<HashMap<(char, Weight), Option<usize>>>,
    /// The bytes of every font file with a face chosen so far, by [`IndexedFace::file`]; bounded
    /// by the number of files.
    loaded: Mutex<HashMap<usize, Arc<Vec<u8>>>>,
    /// Uncached lookups performed, for tests.
    lookups: AtomicUsize,
}

impl SystemFonts {
    pub(super) fn new(source: Source) -> SystemFonts {
        SystemFonts {
            source,
            index: OnceLock::new(),
            cache: Mutex::new(HashMap::new()),
            loaded: Mutex::new(HashMap::new()),
            lookups: AtomicUsize::new(0),
        }
    }

    pub(super) fn index(&self) -> &FaceIndex {
        self.index.get_or_init(|| FaceIndex::build(&self.source))
    }

    /// The face to draw `c` with, if any. An uncached lookup spends one unit of `budget`; with
    /// no budget left the answer is `None` and nothing is cached.
    pub(super) fn face_for(&self, c: char, weight: Weight, budget: &mut usize) -> Option<usize> {
        if let Some(found) = lock(&self.cache).get(&(c, weight)) {
            return *found;
        }
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        self.lookups.fetch_add(1, Ordering::Relaxed);
        let index = self.index();
        let found = index.find(c, weight, |face| self.confirm(index, face, c));
        let mut cache = lock(&self.cache);
        if cache.len() < CACHE_LIMIT {
            cache.insert((c, weight), found);
        }
        found
    }

    /// Opens the face and checks that it really draws `c`; keeps its bytes when it does.
    fn confirm(&self, index: &FaceIndex, face: usize, c: char) -> bool {
        let Some(info) = index.faces.get(face) else {
            return false;
        };
        if let Some(data) = lock(&self.loaded).get(&info.file).cloned() {
            return has_outline(&data, info.face_index, c);
        }
        let data = index
            .db
            .with_face_data(info.id, |data, i| {
                has_outline(data, i, c).then(|| Arc::new(data.to_vec()))
            })
            .flatten();
        match data {
            Some(data) => {
                lock(&self.loaded).entry(info.file).or_insert(data);
                true
            }
            None => false,
        }
    }

    /// The bytes of a face chosen by [`SystemFonts::face_for`].
    pub(super) fn loaded(&self, face: usize) -> Option<LoadedFace> {
        let info = self.index.get()?.faces.get(face)?;
        let data = lock(&self.loaded).get(&info.file).cloned()?;
        Some(LoadedFace {
            data,
            index: info.face_index,
        })
    }

    /// The face's PostScript name, for diagnostics.
    #[cfg(test)]
    pub(super) fn face(&self, face: usize) -> Option<&IndexedFace> {
        self.index.get()?.faces.get(face)
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
