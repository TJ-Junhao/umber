//! Shaping fonts for system faces: the tables they copy, and the bounded cache that keeps them
//! between layout calls.
//!
//! HarfRust needs font data it can own, and fontdb only lends a system font's bytes for the
//! duration of a call. So each table the shaper asks for is copied from the font file on first
//! use, and only the tables in [`SHAPING_TABLES`]: never the outline or bitmap tables, which make
//! up most of a large font file. Every copied byte is accounted for, so the kept fonts never
//! hold more than the cache's byte budget.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use read_fonts::FontRef;
use read_fonts::model::Blob;
use read_fonts::types::Tag;

use super::index::FaceIndex;
use crate::text::font::ShapingFont;
use crate::text::guard::lock;

/// The most system faces whose shaping tables are kept between layout calls.
pub(in crate::text) const SHAPING_CACHE_FACES: usize = 64;

/// The most bytes of shaping tables kept between layout calls, over all system faces. A face
/// whose tables do not fit is shaped from a fresh copy in every layout call instead.
pub(in crate::text) const SHAPING_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// The tables that shaping a system font may copy; see [`ShapingCache::font`].
///
/// The shaper's copy has no `glyf` or `CFF`; glyph bounds for fallback mark positioning are
/// read from the full font file instead (see `SystemExtents` in `text::shaping`).
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
pub(in crate::text) struct SystemShaping {
    /// The font the shaper reads; the layout call shapes its runs with it.
    pub(in crate::text) font: ShapingFont,
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

/// The shaping fonts one library keeps between layout calls, by face.
pub(super) struct ShapingCache {
    fonts: Mutex<HashMap<usize, ShapingFont>>,
    /// Table bytes held by the kept shaping fonts, at most `cap`.
    bytes: Arc<AtomicUsize>,
    /// The most table bytes the kept fonts may hold.
    cap: usize,
}

impl ShapingCache {
    pub(super) fn new(cap: usize) -> ShapingCache {
        ShapingCache {
            fonts: Mutex::new(HashMap::new()),
            bytes: Arc::new(AtomicUsize::new(0)),
            cap,
        }
    }

    /// A shaping font for face `face` of `index`: the kept one if there is one, otherwise a
    /// fresh one that copies the tables in [`SHAPING_TABLES`] on first use.
    ///
    /// A kept font charges every table it copies later to the cache's byte budget, and once
    /// that is used up it copies nothing more: the shaper then sees those tables as missing.
    pub(super) fn font(&self, index: Option<&FaceIndex>, face: usize) -> Option<SystemShaping> {
        if let Some(font) = lock(&self.fonts).get(&face) {
            return Some(SystemShaping {
                font: font.clone(),
                tables: Arc::new(TableAccount::default()),
            });
        }
        let index = index?;
        let face_index = index.face(face)?.face_index();
        let handle = index.handle(face)?;
        let account = Arc::new(TableAccount::default());
        let (own, total) = (Arc::clone(&account), Arc::clone(&self.bytes));
        let cap = self.cap;
        let tables = move |tag: Tag| -> Option<Blob> {
            if !SHAPING_TABLES.contains(&tag) {
                return None;
            }
            let table = handle
                .with_data(|data, index| {
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
            font: ShapingFont::new(tables, face_index)?,
            tables: account,
        })
    }

    /// Keeps a shaping font from [`ShapingCache::font`] for later layouts, within
    /// [`SHAPING_CACHE_FACES`] faces and the cache's byte budget. Nothing is evicted.
    ///
    /// The bytes the font has copied so far are charged now, and any table it copies later is
    /// charged when it is copied, so the kept fonts never hold more than the budget.
    pub(super) fn keep(&self, face: usize, shaping: SystemShaping) {
        let mut fonts = lock(&self.fonts);
        if fonts.contains_key(&face) || fonts.len() >= SHAPING_CACHE_FACES {
            return;
        }
        let bytes = shaping.tables.copied.load(Ordering::Relaxed);
        if !reserve(&self.bytes, bytes, self.cap) {
            return;
        }
        shaping.tables.kept.store(true, Ordering::Release);
        fonts.insert(face, shaping.font);
    }
}

/// Inspection for unit tests.
#[cfg(test)]
impl ShapingCache {
    /// The number of faces whose shaping tables are kept, and the bytes they hold.
    pub(super) fn usage(&self) -> (usize, usize) {
        let faces = lock(&self.fonts).len();
        (faces, self.bytes.load(Ordering::Acquire))
    }
}
