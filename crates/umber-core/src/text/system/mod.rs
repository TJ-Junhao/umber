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
//! Discovery, coverage reading and candidate confirmation read font files through fontdb, which
//! memory-maps a file each time it is read. The file of a face chosen to draw a character is
//! read into memory once, the first time one of its faces is chosen, and shared from then on
//! by every face of the file and every layout, outline and scene that uses them, so a finished
//! scene does not depend on the file. The shaper copies the tables it reads (character map,
//! metrics and layout tables, never outlines or bitmaps) from those bytes for each chosen face,
//! and keeps them for later layouts within a fixed budget.
//!
//! Work and memory are bounded:
//! - the index holds at most [`Limits::max_faces`] faces and [`Limits::max_total_ranges`]
//!   coverage ranges in total; it skips faces larger than [`Limits::max_bytes`] and faces with
//!   more than [`Limits::max_ranges`] ranges;
//! - reading `cmap` tables visits each subtable once and at most [`Limits::max_face_visits`]
//!   segments or groups per face and [`Limits::max_build_visits`] per index build;
//! - one layout performs at most [`LOOKUPS_PER_LAYOUT`](lookup::LOOKUPS_PER_LAYOUT) uncached
//!   lookups and [`CONFIRMATIONS_PER_LAYOUT`](lookup::CONFIRMATIONS_PER_LAYOUT) candidate
//!   confirmations, and one lookup at most
//!   [`CONFIRMATIONS_PER_LOOKUP`](lookup::CONFIRMATIONS_PER_LOOKUP);
//! - the lookup cache holds at most [`CACHE_LIMIT`](lookup::CACHE_LIMIT) entries;
//! - shaping tables are kept for at most [`SHAPING_CACHE_FACES`](shaping::SHAPING_CACHE_FACES)
//!   faces and [`SHAPING_CACHE_BYTES`] bytes in total, counting tables copied after a face was
//!   kept;
//! - the loaded font files take at most [`Limits::max_loaded_bytes`] in total.
//!
//! These caps apply after fontdb's own discovery: `load_system_fonts` enumerates every font in
//! the system font directories and keeps its metadata, bounded only by those directories.
//!
//! Concurrent misses on the same character are not merged; each one is bounded by the caps
//! above and cheap.

// Submodules, lowest first; each uses only the ones listed before it, plus `Limits` and
// `Source` from this module:
// - `index` owns discovery, coverage reading, the index and candidate ranking;
// - `lookup` owns budgeted confirmation of candidates and the lookup cache (uses `index`);
// - `shaping` owns table copying and the kept shaping fonts (uses `index`).
// This module owns the limits, the process-wide index and `SystemFonts`, which ties them
// together for one library.
mod index;
mod lookup;
mod shaping;

use std::sync::{Arc, OnceLock};

use self::index::FaceIndex;
use self::lookup::Lookups;
use self::shaping::ShapingCache;
use crate::fonts::Weight;
use crate::text::font::FontHandle;

pub(super) use self::lookup::Budget;
pub(crate) use self::lookup::find_family;
pub(super) use self::shaping::{SHAPING_CACHE_BYTES, SystemShaping};

#[cfg(test)]
pub(super) use self::index::{CmapError, FaceCoverage, SkipReason, cmap_ranges, coverage};
#[cfg(test)]
pub(super) use self::lookup::{
    CACHE_LIMIT, CONFIRMATIONS_PER_LAYOUT, CONFIRMATIONS_PER_LOOKUP, LOOKUPS_PER_LAYOUT,
};
#[cfg(test)]
pub(super) use self::shaping::SHAPING_CACHE_FACES;

/// Bounds on the system font index.
#[derive(Debug, Clone, Copy)]
pub(super) struct Limits {
    /// Faces with more merged coverage ranges than this are skipped.
    max_ranges: usize,
    /// Faces whose font file is larger than this many bytes are skipped.
    max_bytes: usize,
    /// At most this many faces are indexed, the first ones in sorted order.
    max_faces: usize,
    /// The most merged ranges kept in the whole index. A face whose ranges do not fit is
    /// skipped, not truncated.
    max_total_ranges: usize,
    /// Faces whose `cmap` needs more segment or group visits than this are skipped.
    max_face_visits: usize,
    /// Once this many segment or group visits have been spent, no further faces are indexed.
    max_build_visits: usize,
    /// The most shaping table bytes kept between layout calls; see [`SHAPING_CACHE_BYTES`].
    shaping_cache_bytes: usize,
    /// The most bytes of font files the index loads into memory in total; see
    /// [`Limits::max_loaded_bytes`].
    max_loaded_bytes: usize,
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
        max_loaded_bytes: 1024 * 1024 * 1024,
    };

    /// The most bytes of font files the index loads into memory in total: the files of the
    /// faces chosen to draw characters, each loaded once. A face whose file does not fit is
    /// not used.
    pub(super) const fn max_loaded_bytes(&self) -> usize {
        self.max_loaded_bytes
    }
}

/// Smaller limits for unit tests.
#[cfg(test)]
impl Limits {
    pub(super) const fn with_max_bytes(self, max_bytes: usize) -> Limits {
        Limits { max_bytes, ..self }
    }

    pub(super) const fn with_max_faces(self, max_faces: usize) -> Limits {
        Limits { max_faces, ..self }
    }

    pub(super) const fn with_max_total_ranges(self, max_total_ranges: usize) -> Limits {
        Limits {
            max_total_ranges,
            ..self
        }
    }

    pub(super) const fn with_max_face_visits(self, max_face_visits: usize) -> Limits {
        Limits {
            max_face_visits,
            ..self
        }
    }

    pub(super) const fn with_max_build_visits(self, max_build_visits: usize) -> Limits {
        Limits {
            max_build_visits,
            ..self
        }
    }

    pub(super) const fn with_shaping_cache_bytes(self, shaping_cache_bytes: usize) -> Limits {
        Limits {
            shaping_cache_bytes,
            ..self
        }
    }

    pub(super) const fn with_max_loaded_bytes(self, max_loaded_bytes: usize) -> Limits {
        Limits {
            max_loaded_bytes,
            ..self
        }
    }

    pub(super) const fn max_face_visits(&self) -> usize {
        self.max_face_visits
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

/// System font fallback state of one [`FontLibrary`](super::FontLibrary).
pub(super) struct SystemFonts {
    source: Source,
    limits: Limits,
    /// For [`Source::Os`], the process-wide index shared by every library.
    index: OnceLock<Arc<FaceIndex>>,
    /// Lookup results and work counters.
    lookups: Lookups,
    /// Shaping fonts kept between layout calls.
    shaping: ShapingCache,
}

impl SystemFonts {
    pub(super) fn new(source: Source, limits: Limits) -> SystemFonts {
        SystemFonts {
            source,
            limits,
            index: OnceLock::new(),
            lookups: Lookups::new(limits.max_bytes),
            shaping: ShapingCache::new(limits.shaping_cache_bytes),
        }
    }

    pub(super) fn index(&self) -> &FaceIndex {
        self.shared_index()
    }

    /// The index, built on first use. The machine's fonts are indexed once per process and
    /// shared by every library that uses them; in-memory test fonts get an index of their own.
    ///
    /// The process-wide index is built with the limits of the first library that needs it.
    /// Only [`FontLibrary::bundled`](super::FontLibrary::bundled) uses the machine's fonts,
    /// and it always passes [`Limits::DEFAULT`], so the shared index is built with those.
    pub(super) fn shared_index(&self) -> &Arc<FaceIndex> {
        self.index.get_or_init(|| match &self.source {
            Source::Os => {
                static OS: OnceLock<Arc<FaceIndex>> = OnceLock::new();
                Arc::clone(OS.get_or_init(|| Arc::new(FaceIndex::build(&Source::Os, self.limits))))
            }
            #[cfg(test)]
            Source::Memory(_) => Arc::new(FaceIndex::build(&self.source, self.limits)),
        })
    }

    /// The face to draw `c` with, if any, within `budget`; see [`Lookups::face_for`]. The index
    /// is built by the first uncached lookup within the budget.
    pub(super) fn face_for(&self, c: char, weight: Weight, budget: &mut Budget) -> Option<usize> {
        self.lookups.face_for(|| self.index(), c, weight, budget)
    }

    /// A shared handle to a face chosen by [`SystemFonts::face_for`].
    pub(super) fn handle(&self, face: usize) -> Option<FontHandle> {
        self.index.get()?.handle(face)
    }

    /// A shaping font for a face chosen by [`SystemFonts::face_for`]; see
    /// [`ShapingCache::font`].
    pub(super) fn shaping_font(&self, face: usize) -> Option<SystemShaping> {
        self.shaping.font(self.index.get().map(Arc::as_ref), face)
    }

    /// Keeps a shaping font from [`SystemFonts::shaping_font`] for later layouts; see
    /// [`ShapingCache::keep`].
    pub(super) fn keep_shaping_font(&self, face: usize, shaping: SystemShaping) {
        self.shaping.keep(face, shaping);
    }
}

/// Inspection for unit tests.
#[cfg(test)]
impl SystemFonts {
    /// An indexed face, for diagnostics.
    pub(super) fn face(&self, face: usize) -> Option<&index::IndexedFace> {
        self.index.get()?.face(face)
    }

    pub(super) fn confirmations(&self) -> usize {
        self.lookups.confirmations()
    }

    pub(super) fn lookups(&self) -> usize {
        self.lookups.lookups()
    }

    /// The number of faces whose shaping tables are kept, and the bytes they hold.
    pub(super) fn shaping_cache(&self) -> (usize, usize) {
        self.shaping.usage()
    }

    pub(super) fn cache_len(&self) -> usize {
        self.lookups.cache_len()
    }
}
