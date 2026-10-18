//! Font identity, font bytes and parsed faces.
//!
//! A [`FontKey`] names a library font or a primary face in one scene. A [`FontHandle`] shares
//! the font's bytes, so a layout can be outlined after its library is gone. A [`Face`] is a font parsed for character
//! lookup, outlines and glyph bounds.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use harfrust::GlyphExtents;
use read_fonts::tables::cmap::CmapSubtable;
use read_fonts::types::GlyphId;
use read_fonts::{FontRef, TableProvider};
use skrifa::MetadataProvider;
use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::pen::ControlBoundsPen;
use skrifa::outline::{OutlineGlyphCollection, OutlineGlyphFormat};

use super::cmap::{secondary_subtables, subtable_glyph};
use super::outline::{draws_segments, unhinted};
use super::style::FaceId;
use crate::fonts::{BundledFont, Weight, font_bytes};

/// Which font a run is drawn with, within one scene and its [`FontLibrary`](super::FontLibrary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum FontKey {
    Bundled(BundledFont, Weight),
    /// Index into the library's fallback fonts, in the caller's order.
    Extra(usize),
    /// Index into the system font index.
    System(usize),
    /// A file or installed-family face in one SceneFonts, with its requested weight.
    /// Stable across layouts in that scene, not across different scene font sets.
    Scene(FaceId, Weight),
}

/// Where the bytes of a font live.
#[derive(Clone)]
enum FontData {
    /// A bundled font, embedded in the binary.
    Bundled(&'static [u8]),
    /// A font passed to [`FontLibrary::with_fallbacks`](super::FontLibrary::with_fallbacks).
    Extra(Arc<Vec<u8>>),
    /// A system font. fontdb memory-maps the file each time it is read, and outlining copies
    /// nothing. The `Arc` keeps the database alive for as long as a layout refers to it.
    System(Arc<fontdb::Database>, fontdb::ID),
}

/// A shared handle to the font a run was shaped with: its bytes and its face index.
#[derive(Clone)]
pub(super) struct FontHandle {
    data: FontData,
    index: u32,
}

impl FontHandle {
    /// A bundled font, face 0 of its file.
    pub(super) fn bundled(font: BundledFont, weight: Weight) -> FontHandle {
        FontHandle {
            data: FontData::Bundled(font_bytes(font, weight)),
            index: 0,
        }
    }

    /// A font file passed by the caller, face 0 of it.
    pub(super) fn extra(bytes: Arc<Vec<u8>>) -> FontHandle {
        FontHandle {
            data: FontData::Extra(bytes),
            index: 0,
        }
    }

    /// An owned face, including a face selected from an installed collection.
    pub(super) fn owned(font: &OwnedFace) -> FontHandle {
        FontHandle {
            data: FontData::Extra(Arc::clone(&font.bytes)),
            index: font.index,
        }
    }

    /// Face `index` of the system font `id` in `db`.
    pub(super) fn system(db: Arc<fontdb::Database>, id: fontdb::ID, index: u32) -> FontHandle {
        FontHandle {
            data: FontData::System(db, id),
            index,
        }
    }

    /// Runs `f` on the font bytes and face index; `None` if the bytes cannot be read.
    pub(super) fn with_data<T>(&self, f: impl FnOnce(&[u8], u32) -> T) -> Option<T> {
        match &self.data {
            FontData::Bundled(bytes) => Some(f(bytes, self.index)),
            FontData::Extra(bytes) => Some(f(bytes, self.index)),
            FontData::System(db, id) => db.with_face_data(*id, f),
        }
    }

    /// Whether the bytes are the crate's own fonts, which need no panic guard.
    pub(super) fn is_bundled(&self) -> bool {
        matches!(self.data, FontData::Bundled(_))
    }
}

impl std::fmt::Debug for FontHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match &self.data {
            FontData::Bundled(_) => "bundled",
            FontData::Extra(_) => "fallback",
            FontData::System(..) => "system",
        };
        f.debug_struct("FontHandle")
            .field("kind", &kind)
            .field("index", &self.index)
            .finish()
    }
}

/// A font prepared for shaping.
pub(super) type ShapingFont = harfrust::Font;

/// A validated, owned primary face. Its bytes and default-instance shaper are immutable.
/// Unlike a fallback discovered during layout, it cannot disappear before outlining.
#[derive(Clone)]
pub(crate) struct OwnedFace {
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(super) index: u32,
    pub(super) shaping: ShapingFont,
    pub(super) metrics: FaceMetrics,
    pub(crate) family: String,
    pub(crate) generic: &'static str,
}

/// Default-instance vertical metrics, in font units.
#[derive(Clone, Copy)]
pub(super) struct FaceMetrics {
    pub(super) units_per_em: u16,
    pub(super) ascender: f32,
    pub(super) descender: f32,
}

impl OwnedFace {
    /// Validates the table ranges and required outline and metric tables at construction.
    pub(crate) fn parse(bytes: Arc<Vec<u8>>, index: u32, name: &str) -> Option<OwnedFace> {
        super::guard::guarded(|| {
            let font = FontRef::from_index(&bytes, index).ok()?;
            let records = font.table_directory().table_records();
            if records
                .windows(2)
                .any(|pair| pair[0].tag() >= pair[1].tag())
            {
                return None;
            }
            for record in records {
                font.table_data(record.tag())?;
            }
            let head = font.head().ok()?;
            let maxp = font.maxp().ok()?;
            if head.units_per_em() == 0 || maxp.num_glyphs() == 0 {
                return None;
            }
            font.hhea().ok()?;
            font.hmtx().ok()?;
            font.cmap().ok()?;
            match font.outline_glyphs().format()? {
                OutlineGlyphFormat::Glyf => {
                    font.glyf().ok()?;
                    font.loca(None).ok()?;
                }
                OutlineGlyphFormat::Cff => {
                    font.cff().ok()?;
                }
                OutlineGlyphFormat::Cff2 => {
                    font.cff2().ok()?;
                }
                OutlineGlyphFormat::Varc => {
                    font.varc().ok()?;
                }
            }
            let metrics = font.metrics(Size::unscaled(), LocationRef::default());
            let family = [16, 1]
                .into_iter()
                .find_map(|id| {
                    font.localized_strings(read_fonts::types::NameId::new(id))
                        .english_or_first()
                        .map(|s| s.to_string())
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or_else(|| name.to_owned());
            let generic = if font.post().is_ok_and(|post| post.is_fixed_pitch() != 0) {
                "monospace"
            } else if font.os2().is_ok_and(|os2| {
                let class = os2.s_family_class() >> 8;
                let panose = os2.panose_10();
                (1..=7).contains(&class)
                    || (class == 0
                        && panose.first() == Some(&2)
                        && panose.get(1).is_some_and(|serif| (2..=10).contains(serif)))
            }) {
                "serif"
            } else {
                "sans-serif"
            };
            let shared: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::clone(&bytes) as _;
            let shaping = ShapingFont::new(read_fonts::model::Blob::from(shared), index)?;
            Some(OwnedFace {
                bytes,
                index,
                shaping,
                metrics: FaceMetrics {
                    units_per_em: metrics.units_per_em,
                    ascender: metrics.ascent,
                    descender: metrics.descent,
                },
                family,
                generic,
            })
        })
        .flatten()
    }
}

impl std::fmt::Debug for OwnedFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedFace")
            .field("family", &self.family)
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

/// Slots in a face's cache of secondary lookups.
const SECONDARY_CACHE_SLOTS: usize = 256;

/// A font parsed for character lookup and outlines.
pub(super) struct Face<'a> {
    font: FontRef<'a>,
    /// The best Unicode subtable.
    charmap: Charmap<'a>,
    /// The other distinct Unicode subtables, in table order; see [`Face::secondary_glyph`].
    secondary: Vec<CmapSubtable<'a>>,
    /// A direct-mapped cache of secondary lookups, indexed by code point. Each slot holds
    /// `(cp + 1) << 32 | glyph`, with glyph `u32::MAX` for "no glyph", or 0 when empty.
    secondary_cache: Box<[AtomicU64]>,
    outlines: OutlineGlyphCollection<'a>,
}

impl<'a> Face<'a> {
    /// Parses face `index` of `data`; `None` if it is not a font.
    pub(super) fn parse(data: &'a [u8], index: u32) -> Option<Face<'a>> {
        let font = FontRef::from_index(data, index).ok()?;
        let secondary = font
            .cmap()
            .ok()
            .map(|cmap| secondary_subtables(&cmap))
            .unwrap_or_default();
        let slots = if secondary.is_empty() {
            0
        } else {
            SECONDARY_CACHE_SLOTS
        };
        Some(Face {
            font: font.clone(),
            charmap: font.charmap(),
            secondary,
            secondary_cache: (0..slots).map(|_| AtomicU64::new(0)).collect(),
            outlines: font.outline_glyphs(),
        })
    }

    /// The glyph for `c`, treating a mapping to glyph 0 (`.notdef`) as missing.
    pub(super) fn glyph(&self, c: char) -> Option<GlyphId> {
        self.charmap
            .map(c)
            .or_else(|| self.secondary_glyph(u32::from(c)))
            .filter(|g| *g != GlyphId::NOTDEF)
    }

    /// The glyph for `cp` from the best Unicode subtable alone, treating `.notdef` as missing.
    pub(super) fn best_subtable_glyph(&self, cp: u32) -> Option<GlyphId> {
        self.charmap.map(cp).filter(|g| *g != GlyphId::NOTDEF)
    }

    /// The glyph for `cp` from the first secondary Unicode subtable, in table order, that
    /// maps it.
    ///
    /// The best subtable is tried first, by [`Face::glyph`] and by the shaper. This also reads
    /// format 2, which neither of them does, and formats 0, 6 and 10, which skrifa's lookup
    /// does not, so that every subtable the system font index reads can produce the glyph.
    ///
    /// The secondary subtables are resolved once per face: distinct subtables only, without
    /// the best one, and at most [`SECONDARY_SUBTABLES`](super::cmap::SECONDARY_SUBTABLES) of
    /// them; later ones are ignored. Results are cached per code point, so a lookup costs at
    /// most that many subtable lookups however many encoding records the font has.
    pub(super) fn secondary_glyph(&self, cp: u32) -> Option<GlyphId> {
        if self.secondary.is_empty() {
            return None;
        }
        let key = u64::from(cp) + 1;
        let slot = self
            .secondary_cache
            .get(cp as usize % SECONDARY_CACHE_SLOTS)?;
        let cached = slot.load(Ordering::Relaxed);
        if cached >> 32 == key {
            let glyph = (cached & 0xFFFF_FFFF) as u32;
            return (glyph != u32::MAX).then(|| GlyphId::new(glyph));
        }
        let found = self
            .secondary
            .iter()
            .find_map(|sub| subtable_glyph(sub, cp).filter(|g| *g != GlyphId::NOTDEF));
        let value = found.map_or(u32::MAX, GlyphId::to_u32);
        slot.store(key << 32 | u64::from(value), Ordering::Relaxed);
        found.filter(|g| g.to_u32() != u32::MAX)
    }

    /// Whether `glyph` has an outline with at least one segment.
    pub(super) fn has_outline(&self, glyph: GlyphId) -> bool {
        self.outlines
            .get(glyph)
            .is_some_and(|outline| draws_segments(&outline))
    }

    /// The ink extents of `glyph` in font units, the way HarfRust reads them from a font's own
    /// outline tables.
    ///
    /// - A glyph that draws without any path commands, such as an empty mark, has empty
    ///   extents.
    /// - A glyph that is missing or cannot be drawn has none.
    /// - Otherwise: for `glyf` outlines the bounding box the glyph states, with the horizontal
    ///   bearing from `hmtx`; for other outlines the box of the drawn points.
    pub(super) fn extents(&self, glyph: GlyphId) -> Option<GlyphExtents> {
        let outline = self.outlines.get(glyph)?;
        let mut pen = ControlBoundsPen::new();
        outline.draw(unhinted(), &mut pen).ok()?;
        let Some(drawn) = pen.bounding_box() else {
            return Some(GlyphExtents::default());
        };
        let (bounds, x_bearing) = match self.outlines.format() {
            Some(OutlineGlyphFormat::Glyf) => {
                let metrics = self
                    .font
                    .glyph_metrics(Size::unscaled(), LocationRef::default());
                let stated = metrics.bounds(glyph)?;
                let bearing = metrics.left_side_bearing(glyph).unwrap_or(0.0);
                let left = stated.x_min.min(stated.x_max);
                (
                    (
                        left,
                        stated.x_min.max(stated.x_max),
                        stated.y_min,
                        stated.y_max,
                    ),
                    bearing,
                )
            }
            _ => (
                (drawn.x_min, drawn.x_max, drawn.y_min, drawn.y_max),
                drawn.x_min,
            ),
        };
        let (left, right, y_min, y_max) = bounds;
        let (bottom, top) = (y_min.min(y_max), y_min.max(y_max));
        Some(GlyphExtents {
            x_bearing: x_bearing.round() as i32,
            y_bearing: top.round() as i32,
            width: (right - left).round() as i32,
            height: (bottom - top).round() as i32,
        })
    }

    /// Whether this face can draw `c`: it has a glyph, and the glyph has an outline unless `c`
    /// is whitespace. Bitmap-only glyphs and fonts without outline tables do not count.
    pub(super) fn draws(&self, c: char) -> bool {
        self.glyph(c)
            .is_some_and(|g| c.is_whitespace() || self.has_outline(g))
    }

    /// The number of secondary subtables this face consults.
    #[cfg(test)]
    pub(super) fn secondary_count(&self) -> usize {
        self.secondary.len()
    }

    /// The format of this face's outlines, if it has any.
    #[cfg(test)]
    pub(super) fn outline_format(&self) -> Option<OutlineGlyphFormat> {
        self.outlines.format()
    }
}
