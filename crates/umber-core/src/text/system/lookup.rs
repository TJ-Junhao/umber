//! Budgeted lookup of the system face that draws a character, and the lookup cache.
//!
//! A lookup walks the candidates of [`FaceIndex::find`] and confirms each one by opening the
//! face, within a per-lookup and a per-layout budget of confirmations. Answers are cached per
//! library, `(character, weight)` to the chosen face or to no face.

use std::collections::HashMap;
use std::sync::Mutex;
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
