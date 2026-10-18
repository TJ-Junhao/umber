//! Budgeted lookup of the system face that draws a character, and the lookup cache.
//!
//! A lookup walks the candidates of [`FaceIndex::find`] and confirms each one by opening the
//! face, within a per-lookup and a per-layout budget of confirmations. Answers are cached per
//! library, `(character, weight)` to the chosen face or to no face.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use read_fonts::{FontRef, TableProvider};
use skrifa::MetadataProvider;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::index::{FaceIndex, Verdict};
use crate::fonts::Weight;
use crate::text::font::Face;
use crate::text::guard::{guarded, lock};

/// The most uncached system font lookups one layout call performs. Characters beyond it are
/// drawn with the primary font's `.notdef`.
pub(in crate::text) const LOOKUPS_PER_LAYOUT: usize = 1024;

/// The most `(character, weight)` entries the lookup cache holds. When it is full, new results
/// are not cached; nothing is evicted.
pub(in crate::text) const CACHE_LIMIT: usize = 65_536;

/// The most candidate confirmations (open, parse, outline check) one uncached lookup performs.
/// When they are used up the character resolves to the primary font's `.notdef`.
pub(in crate::text) const CONFIRMATIONS_PER_LOOKUP: usize = 16;

/// The most candidate confirmations one layout call performs. When they are used up the
/// remaining characters resolve to the primary font's `.notdef`, and are not cached.
pub(in crate::text) const CONFIRMATIONS_PER_LAYOUT: usize = 4096;

/// The system font work one layout call may still do.
#[derive(Debug)]
pub(in crate::text) struct Budget {
    lookups: usize,
    confirmations: usize,
}

impl Budget {
    pub(in crate::text) fn per_layout() -> Budget {
        Budget {
            lookups: LOOKUPS_PER_LAYOUT,
            confirmations: CONFIRMATIONS_PER_LAYOUT,
        }
    }
}

/// The lookup cache of one library, with counters of the work done.
pub(super) struct Lookups {
    /// Faces whose font file is larger than this many bytes are never confirmed.
    max_bytes: usize,
    /// `(character, weight)` to the chosen face, or `None` when no face qualifies.
    cache: Mutex<HashMap<(char, Weight), Option<usize>>>,
    /// Uncached lookups performed, for tests.
    lookups: AtomicUsize,
    /// Candidate confirmations performed, for tests.
    confirmations: AtomicUsize,
}

impl Lookups {
    pub(super) fn new(max_bytes: usize) -> Lookups {
        Lookups {
            max_bytes,
            cache: Mutex::new(HashMap::new()),
            lookups: AtomicUsize::new(0),
            confirmations: AtomicUsize::new(0),
        }
    }

    /// The face of `index` to draw `c` with, if any. `index` is called only for an uncached
    /// lookup within the budget.
    ///
    /// An uncached lookup spends one lookup from `budget` and at most
    /// [`CONFIRMATIONS_PER_LOOKUP`] of its confirmations. A lookup that uses up its own
    /// confirmations answers `None`, which is cached like any other answer. When the layout's
    /// budget runs out the answer is `None` and nothing is cached.
    pub(super) fn face_for<'i>(
        &self,
        index: impl FnOnce() -> &'i FaceIndex,
        c: char,
        weight: Weight,
        budget: &mut Budget,
    ) -> Option<usize> {
        if let Some(found) = lock(&self.cache).get(&(c, weight)) {
            return *found;
        }
        if budget.lookups == 0 || budget.confirmations == 0 {
            return None;
        }
        budget.lookups -= 1;
        self.lookups.fetch_add(1, Ordering::Relaxed);
        let index = index();
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
        index
            .with_face_data(face, |data, i| {
                data.len() <= self.max_bytes
                    && guarded(|| Face::parse(data, i).is_some_and(|f| f.draws(c))).unwrap_or(false)
            })
            .unwrap_or(false)
    }
}

/// Finds a named installed family. Every call scans the installed fonts; there is no cache.
pub(crate) fn find_family(family: &str) -> crate::Result<crate::fonts::SystemFamily> {
    check_family(family)?;
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    family_in(&db, family)
}

fn check_family(family: &str) -> crate::Result<()> {
    if family.is_empty() {
        return Err(crate::Error::usage(
            "Font.system()",
            "the family name is empty; it must name an installed font family.",
        )
        .with_parameter("family")
        .with_hint("check the installed family names."));
    }
    Ok(())
}

/// Faces from one source are inspected together, so a collection is mapped only once.
#[derive(Hash, PartialEq, Eq)]
enum SourceKey<'a> {
    File(&'a Path),
    Binary(*const ()),
}

fn source_key(source: &fontdb::Source) -> SourceKey<'_> {
    match source {
        fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => SourceKey::File(path),
        fontdb::Source::Binary(bytes) => SourceKey::Binary(Arc::as_ptr(bytes).cast::<()>()),
    }
}

fn source_groups(db: &fontdb::Database) -> Vec<Vec<&fontdb::FaceInfo>> {
    let mut slots = HashMap::new();
    let mut groups: Vec<Vec<&fontdb::FaceInfo>> = Vec::new();
    for face in db.faces() {
        let index = *slots.entry(source_key(&face.source)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        if let Some(group) = groups.get_mut(index) {
            group.push(face);
        }
    }
    groups
}

/// A matching face with the name used to find it and a shared copy of its source.
struct FamilyFace<'a> {
    info: &'a fontdb::FaceInfo,
    name: String,
    bytes: Option<Arc<Vec<u8>>>,
    variable: bool,
}

impl<'a> FamilyFace<'a> {
    fn new(info: &'a fontdb::FaceInfo, name: String) -> FamilyFace<'a> {
        FamilyFace {
            info,
            name,
            bytes: None,
            variable: false,
        }
    }

    fn parse(&self, family: &str) -> crate::Result<crate::text::OwnedFace> {
        let parsed = self.bytes.as_ref().and_then(|bytes| {
            crate::text::OwnedFace::parse(Arc::clone(bytes), self.info.index, &self.name)
        });
        parsed.ok_or_else(|| {
            crate::Error::usage(
                "Font.system()",
                format!(
                    "the selected face '{}' of '{}' cannot be read; it must be a readable \
                     TrueType or OpenType font.",
                    self.info.post_script_name.escape_debug(),
                    family.escape_debug(),
                ),
            )
            .with_parameter("family")
            .with_hint("check the installed font files.")
        })
    }
}

fn family_faces<'a>(db: &'a fontdb::Database, family: &str) -> (Vec<FamilyFace<'a>>, Vec<String>) {
    let normalized = family.to_lowercase();
    let mut matches = Vec::new();
    let mut installed_names = Vec::new();
    for info in db.faces() {
        if let Some((name, _)) = info
            .families
            .iter()
            .find(|(name, _)| name.to_lowercase() == normalized)
        {
            matches.push(FamilyFace::new(info, name.clone()));
        }
        installed_names.extend(info.families.iter().map(|(name, _)| name.clone()));
    }
    // fontdb prefers name ID 16 and may omit the legacy family (name ID 1).
    // Consult those aliases only when the database's family names did not match.
    if matches.is_empty() {
        matches = legacy_family_faces(db, &normalized, &mut installed_names);
    }
    let mut sources = HashMap::new();
    for face in &mut matches {
        db.with_face_data(face.info.id, |data, index| {
            face.variable = FontRef::from_index(data, index).is_ok_and(|font| font.fvar().is_ok());
            // Only matched sources are copied, once even when a collection has several faces.
            let bytes = sources
                .entry(source_key(&face.info.source))
                .or_insert_with(|| Arc::new(data.to_vec()));
            face.bytes = Some(Arc::clone(bytes));
        });
    }
    (matches, installed_names)
}

fn legacy_family_faces<'a>(
    db: &'a fontdb::Database,
    normalized: &str,
    installed_names: &mut Vec<String>,
) -> Vec<FamilyFace<'a>> {
    let mut matches = Vec::new();
    for group in source_groups(db) {
        let Some(first) = group.first() else { continue };
        db.with_face_data(first.id, |data, _| {
            for info in group {
                let Ok(font) = FontRef::from_index(data, info.index) else {
                    continue;
                };
                let names: Vec<_> = font
                    .localized_strings(read_fonts::types::NameId::new(1))
                    .map(|name| name.to_string())
                    .filter(|name| !name.is_empty())
                    .collect();
                if let Some(name) = names.iter().find(|name| name.to_lowercase() == normalized) {
                    matches.push(FamilyFace::new(info, name.clone()));
                }
                installed_names.extend(names);
            }
        });
    }
    matches
}

fn family_in(db: &fontdb::Database, family: &str) -> crate::Result<crate::fonts::SystemFamily> {
    check_family(family)?;
    let (faces, names) = family_faces(db, family);
    let regular = faces
        .iter()
        .min_by_key(|face| {
            let info = face.info;
            (
                info.style != fontdb::Style::Normal,
                info.stretch != fontdb::Stretch::Normal,
                info.weight.0.abs_diff(400),
                info.weight.0,
                &info.post_script_name,
                info.index,
            )
        })
        .ok_or_else(|| unknown_family(names, family))?;
    let bold = faces
        .iter()
        .filter(|face| {
            !face.variable && face.info.style == fontdb::Style::Normal && face.info.weight.0 >= 600
        })
        .min_by_key(|face| {
            let info = face.info;
            (
                info.stretch != fontdb::Stretch::Normal,
                info.weight.0 - 600,
                &info.post_script_name,
                info.index,
            )
        });
    let variable_only = faces.iter().all(|face| face.variable);
    let name = regular.name.clone();
    let regular = regular.parse(family)?;
    let bold = bold.map(|face| face.parse(family)).transpose()?;
    Ok(crate::fonts::SystemFamily::new(
        name,
        regular,
        bold,
        variable_only,
    ))
}

fn unknown_family(mut names: Vec<String>, family: &str) -> crate::Error {
    names.sort_unstable_by_key(|name| (name.to_lowercase(), name.clone()));
    names.dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());
    let query = family.to_lowercase();
    names.sort_by_cached_key(|name| {
        let lower = name.to_lowercase();
        let shared_words = query
            .split_whitespace()
            .filter(|word| lower.split_whitespace().any(|other| *word == other))
            .count();
        let prefix = query
            .chars()
            .zip(lower.chars())
            .take_while(|(a, b)| a == b)
            .count();
        (
            Reverse(shared_words),
            Reverse(prefix),
            edit_distance(&query, &lower),
            lower,
        )
    });
    let mut message = format!(
        "no installed family is named '{}'; it must name an installed font family.",
        family.escape_debug()
    );
    if !names.is_empty() {
        message.push_str(&format!(
            " Closest: {}.",
            names.into_iter().take(3).collect::<Vec<_>>().join(", ")
        ));
    }
    crate::Error::usage("Font.system()", message)
        .with_parameter("family")
        .with_hint("check the installed family names.")
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<_> = right.chars().collect();
    let mut row: Vec<usize> = (0..=right.len()).collect();
    for (i, a) in left.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, &b) in right.iter().enumerate() {
            let previous = row[j + 1];
            row[j + 1] = (row[j] + 1)
                .min(previous + 1)
                .min(diagonal + usize::from(a != b));
            diagonal = previous;
        }
    }
    row[right.len()]
}

#[cfg(test)]
#[path = "family_tests.rs"]
mod family_tests;

/// Inspection for unit tests.
#[cfg(test)]
impl Lookups {
    pub(super) fn lookups(&self) -> usize {
        self.lookups.load(Ordering::Relaxed)
    }

    pub(super) fn confirmations(&self) -> usize {
        self.confirmations.load(Ordering::Relaxed)
    }

    pub(super) fn cache_len(&self) -> usize {
        lock(&self.cache).len()
    }
}
