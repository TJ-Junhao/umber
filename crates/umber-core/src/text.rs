//! Text shaping, measurement and outlines.
//!
//! A [`FontLibrary`] shapes a string with one of the bundled fonts, applying kerning, standard
//! ligatures and, on request, tabular figures. Characters the requested font lacks are drawn with
//! another bundled family, then with fallback fonts: the system fonts for
//! [`FontLibrary::bundled`], or explicit font files for [`FontLibrary::with_fallbacks`].
//! Characters that no font covers are drawn as the primary font's `.notdef` glyph.
//!
//! All lengths are in points. Outlines are y-down with the baseline at `y = 0` of the origin
//! passed to [`TextLayout::outline`]. Runs are placed left to right in logical order; there is
//! no bidirectional reordering.
//!
//! Nothing here panics or fails: unreadable fonts are skipped, including fonts that make the
//! shaper panic, and missing glyphs degrade to `.notdef`. Such panics are caught, but the panic
//! hook is left alone: in debug builds the default or host hook may print a message for a
//! malformed fallback or system font.
//!
//! System fonts are memory-mapped by `fontdb` whenever they are read, including when a layout is
//! outlined. A system font file that changes or disappears between layout and outline yields the
//! new contents or nothing for those glyphs; the outline never panics. Shaping copies only the
//! tables the shaper reads from a system font, never its outlines.
//!
//! # Examples
//!
//! ```
//! use umber_core::text::{FontLibrary, TextStyle};
//! use umber_core::{Font, Point, Weight};
//!
//! let style = TextStyle { font: Font::Inter, weight: Weight::Regular, size: 10.0, tabular_figures: false };
//! let layout = FontLibrary::global().layout("Revenue", &style);
//! assert!(layout.width() > 0.0);
//! let path = layout.outline(Point { x: 0.0, y: 20.0 });
//! assert!(!path.is_empty());
//! ```

mod marks;
mod system;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use harfrust::{
    Buffer, BufferFlags, Direction, Feature, FontFuncs, GlyphExtents, Script, ShapeOptions,
    ShapePlan, ShaperFont, Tag,
};
use read_fonts::model::Blob;
use read_fonts::tables::cmap::{Cmap, Cmap2, CmapSubtable, EncodingRecord, PlatformId};
use read_fonts::types::GlyphId;
use read_fonts::{FontRef, TableProvider};
use skrifa::MetadataProvider;
use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::pen::ControlBoundsPen;
use skrifa::outline::{DrawSettings, OutlineGlyphCollection, OutlineGlyphFormat, OutlinePen};

use crate::fonts::{Font, Weight, font_bytes};
use crate::geometry::{Path, Point};
use system::{Budget, Limits, SystemFonts};

/// How a piece of text is set: font, weight, size and figure style.
///
/// # Examples
///
/// ```
/// use umber_core::{Font, TextStyle, Weight};
///
/// let style = TextStyle { font: Font::Geist, weight: Weight::SemiBold, size: 13.0, tabular_figures: true };
/// assert_eq!(style.size, 13.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// The primary font family.
    pub font: Font,
    /// The weight, used for the primary family and for bundled fallbacks.
    pub weight: Weight,
    /// The font size in points. NaN, infinite, zero and negative sizes are treated as 0.
    pub size: f32,
    /// Whether to request tabular (fixed-width) figures with the OpenType `tnum` feature.
    pub tabular_figures: bool,
}

/// Which font a run of text is drawn with, within one [`FontLibrary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FontKey {
    Bundled(Font, Weight),
    /// Index into [`FontLibrary::extra`].
    Extra(usize),
    /// Index into the system font index.
    System(usize),
}

/// Where the bytes of a font live.
#[derive(Clone)]
enum FontData {
    /// A bundled font, embedded in the binary.
    Bundled(&'static [u8]),
    /// A font passed to [`FontLibrary::with_fallbacks`].
    Extra(Arc<Vec<u8>>),
    /// A system font. fontdb memory-maps the file each time it is read, and outlining copies
    /// nothing. The `Arc` keeps the database alive for as long as a layout refers to it.
    System(Arc<fontdb::Database>, fontdb::ID),
}

/// A shared handle to the font a run was shaped with: its bytes and its face index.
#[derive(Clone)]
struct FontHandle {
    data: FontData,
    index: u32,
}

impl FontHandle {
    fn bundled(font: Font, weight: Weight) -> FontHandle {
        FontHandle {
            data: FontData::Bundled(font_bytes(font, weight)),
            index: 0,
        }
    }

    /// Runs `f` on the font bytes and face index; `None` if the bytes cannot be read.
    fn with_data<T>(&self, f: impl FnOnce(&[u8], u32) -> T) -> Option<T> {
        match &self.data {
            FontData::Bundled(bytes) => Some(f(bytes, self.index)),
            FontData::Extra(bytes) => Some(f(bytes, self.index)),
            FontData::System(db, id) => db.with_face_data(*id, f),
        }
    }

    /// Whether the bytes are the crate's own fonts, which need no panic guard.
    fn is_bundled(&self) -> bool {
        matches!(self.data, FontData::Bundled(_))
    }
}

/// A font prepared for shaping.
pub(crate) type ShapingFont = harfrust::Font;

/// The most secondary Unicode subtables a face consults; see [`Face::secondary_glyph`].
const SECONDARY_SUBTABLES: usize = 16;

/// Slots in a face's cache of secondary lookups.
const SECONDARY_CACHE_SLOTS: usize = 256;

/// A font parsed for character lookup and outlines.
pub(crate) struct Face<'a> {
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
    fn parse(data: &'a [u8], index: u32) -> Option<Face<'a>> {
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
    fn glyph(&self, c: char) -> Option<GlyphId> {
        self.charmap
            .map(c)
            .or_else(|| self.secondary_glyph(u32::from(c)))
            .filter(|g| *g != GlyphId::NOTDEF)
    }

    /// The glyph for `cp` from the first secondary Unicode subtable, in table order, that
    /// maps it.
    ///
    /// The best subtable is tried first, by [`Face::glyph`] and by the shaper. This also reads
    /// format 2, which neither of them does, and formats 0, 6 and 10, which skrifa's lookup
    /// does not, so that every subtable the system font index reads can produce the glyph.
    ///
    /// The secondary subtables are resolved once per face: distinct subtables only, without
    /// the best one, and at most [`SECONDARY_SUBTABLES`] of them; later ones are ignored.
    /// Results are cached per code point, so a lookup costs at most that many subtable
    /// lookups however many encoding records the font has.
    fn secondary_glyph(&self, cp: u32) -> Option<GlyphId> {
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
        let found = self.secondary.iter().find_map(|sub| {
            let glyph = match sub {
                CmapSubtable::Format2(sub) => format2_glyph(sub, cp),
                sub => sub.map_codepoint(cp),
            };
            glyph.filter(|g| *g != GlyphId::NOTDEF)
        });
        let value = found.map_or(u32::MAX, GlyphId::to_u32);
        slot.store(key << 32 | u64::from(value), Ordering::Relaxed);
        found.filter(|g| g.to_u32() != u32::MAX)
    }

    /// Whether `glyph` has an outline with at least one segment.
    fn has_outline(&self, glyph: GlyphId) -> bool {
        self.outlines.get(glyph).is_some_and(|outline| {
            let mut pen = SegmentCount(0);
            outline.draw(unhinted(), &mut pen).is_ok() && pen.0 > 0
        })
    }

    /// The ink extents of `glyph` in font units, the way HarfRust reads them from a font's own
    /// outline tables.
    ///
    /// - A glyph that draws without any path commands, such as an empty mark, has empty
    ///   extents.
    /// - A glyph that is missing or cannot be drawn has none.
    /// - Otherwise: for `glyf` outlines the bounding box the glyph states, with the horizontal
    ///   bearing from `hmtx`; for other outlines the box of the drawn points.
    fn extents(&self, glyph: GlyphId) -> Option<GlyphExtents> {
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
    pub(crate) fn draws(&self, c: char) -> bool {
        self.glyph(c)
            .is_some_and(|g| c.is_whitespace() || self.has_outline(g))
    }
}

/// The distinct Unicode subtables of `cmap` other than the one skrifa's lookup uses, in table
/// order, at most [`SECONDARY_SUBTABLES`] of them.
fn secondary_subtables<'a>(cmap: &Cmap<'a>) -> Vec<CmapSubtable<'a>> {
    let best = best_subtable_offset(cmap);
    let mut seen: HashSet<u32> = HashSet::new();
    let mut found = Vec::new();
    for record in cmap.encoding_records() {
        if found.len() == SECONDARY_SUBTABLES {
            break;
        }
        let offset = record.subtable_offset().to_u32();
        if !is_unicode(record) || Some(offset) == best || !seen.insert(offset) {
            continue;
        }
        if let Ok(sub) = record.subtable(cmap.offset_data()) {
            found.push(sub);
        }
    }
    found
}

/// The offset of the subtable skrifa's `Charmap` selects, found the same way: records in
/// reverse order, symbol before full-repertoire before BMP subtables, formats 4, 12 and 13.
fn best_subtable_offset(cmap: &Cmap<'_>) -> Option<u32> {
    let mut best: Option<(u8, u32)> = None;
    for record in cmap.encoding_records().iter().rev() {
        let rank = match (record.platform_id(), record.encoding_id()) {
            (PlatformId::Unicode, 5) => continue,
            (PlatformId::Windows, 0) => 3,
            (PlatformId::Windows, 10) | (PlatformId::Unicode, 4 | 6) => 2,
            (PlatformId::ISO | PlatformId::Unicode, _) | (PlatformId::Windows, 1) => 1,
            _ => continue,
        };
        if best.is_some_and(|(current, _)| rank <= current) {
            continue;
        }
        let supported = matches!(
            record.subtable(cmap.offset_data()),
            Ok(CmapSubtable::Format4(_) | CmapSubtable::Format12(_) | CmapSubtable::Format13(_))
        );
        if supported {
            best = Some((rank, record.subtable_offset().to_u32()));
        }
    }
    best.map(|(_, offset)| offset)
}

/// Whether a `cmap` encoding record holds a Unicode subtable: any Unicode platform encoding,
/// or Windows Unicode BMP (1) and full repertoire (10).
pub(crate) fn is_unicode(record: &EncodingRecord) -> bool {
    match record.platform_id() {
        PlatformId::Unicode => true,
        PlatformId::Windows => matches!(record.encoding_id(), 1 | 10),
        _ => false,
    }
}

/// Looks `cp` up in a format 2 (high-byte mapping) subtable.
///
/// Codes below 0xFF use sub-header 0, as single bytes; others use the sub-header of their high
/// byte. Glyph 0 means no glyph.
fn format2_glyph(sub: &Cmap2<'_>, cp: u32) -> Option<GlyphId> {
    /// Format, length, language and the 256 sub-header keys.
    const HEADERS: usize = 6 + 2 * 256;
    let code = u16::try_from(cp).ok()?;
    let (high, low) = (code >> 8, code & 0xFF);
    let key = if code < 0xFF {
        0
    } else {
        sub.sub_header_keys().get(usize::from(high))?.get() / 8
    };
    let data = sub.offset_data();
    let header = HEADERS + 8 * usize::from(key);
    let first = data.read_at::<u16>(header).ok()?;
    let count = data.read_at::<u16>(header + 2).ok()?;
    let delta = data.read_at::<i16>(header + 4).ok()?;
    let range_offset = data.read_at::<u16>(header + 6).ok()?;
    if low < first || low >= first.checked_add(count)? {
        return None;
    }
    // The range offset counts bytes from the range offset field itself.
    let at = header + 6 + usize::from(range_offset) + 2 * usize::from(low - first);
    let glyph = data.read_at::<u16>(at).ok()?;
    if glyph == 0 {
        return None;
    }
    // The delta is added modulo 65,536.
    let id = glyph.wrapping_add_signed(delta);
    Some(GlyphId::new(u32::from(id)))
}

/// The most glyph extents computed from system font files in one layout call; see
/// [`SystemExtents`].
const EXTENTS_PER_LAYOUT: usize = 4096;

/// Glyph bounds for a system font, whose shaping copy has no outline tables.
///
/// The shaper needs bounds to position marks in fonts without GPOS mark attachment. They are
/// read from the full font file while it is mapped for shaping, cached per glyph for the
/// font's runs in one layout call, and at most [`EXTENTS_PER_LAYOUT`] are computed per layout
/// call over all system fonts; beyond that, glyphs have no bounds.
struct SystemExtents<'b> {
    budget: &'b Cell<usize>,
    cache: RefCell<HashMap<GlyphId, Option<GlyphExtents>>>,
}

/// The shaper's font callbacks: the same subtables as [`Face::glyph`] when HarfRust's own
/// choice has no glyph, and for system fonts, glyph bounds from the full font file.
struct Funcs<'f, 'a> {
    face: &'f Face<'a>,
    extents: Option<SystemExtents<'f>>,
}

impl<'f, 'a> Funcs<'f, 'a> {
    fn new(face: &'f Face<'a>) -> Self {
        Funcs {
            face,
            extents: None,
        }
    }

    fn system(face: &'f Face<'a>, budget: &'f Cell<usize>) -> Self {
        Funcs {
            face,
            extents: Some(SystemExtents {
                budget,
                cache: RefCell::new(HashMap::new()),
            }),
        }
    }
}

impl FontFuncs for Funcs<'_, '_> {
    /// HarfRust's own lookup, then the lookup of [`Face::glyph`]: skrifa's best subtable and
    /// the secondary subtables. HarfRust may choose a different best subtable than skrifa, so
    /// skrifa's is consulted before the secondaries, which leave it out.
    fn nominal_glyph(&self, font: &ShaperFont<'_, '_>, c: u32) -> Option<GlyphId> {
        font.default_nominal_glyph(c)
            .or_else(|| self.face.charmap.map(c).filter(|g| *g != GlyphId::NOTDEF))
            .or_else(|| self.face.secondary_glyph(c))
    }

    fn glyph_extents(&self, font: &ShaperFont<'_, '_>, glyph: GlyphId) -> Option<GlyphExtents> {
        let Some(system) = &self.extents else {
            // Bundled and fallback fonts own their outline tables; the shaper reads them.
            return font.default_glyph_extents(glyph);
        };
        if let Some(found) = system.cache.borrow().get(&glyph) {
            return *found;
        }
        let left = system.budget.get();
        if left == 0 {
            return None;
        }
        system.budget.set(left - 1);
        let found = self.face.extents(glyph);
        system.cache.borrow_mut().insert(glyph, found);
        found
    }
}

/// Unhinted outlines in font units, at the default location.
fn unhinted() -> DrawSettings<'static> {
    DrawSettings::unhinted(Size::unscaled(), LocationRef::default())
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

/// A piece of the input text drawn with one font.
#[derive(Debug, Clone)]
struct TextRun {
    font: FontKey,
    /// Draw default-ignorable characters as glyphs (`.notdef` if the font lacks them) instead of
    /// hiding them. Set only for marks and invisible characters at the very start of the text,
    /// which have no base character to attach to.
    preserve_ignorables: bool,
    range: Range<usize>,
}

/// A glyph placed relative to the layout origin, in points, y-down.
#[derive(Debug, Clone, Copy)]
struct PlacedGlyph {
    id: u16,
    x: f32,
    y: f32,
}

/// Consecutive glyphs drawn with one font.
#[derive(Debug, Clone)]
struct GlyphRun {
    /// Identifies the font within the library that made the layout; used for grouping.
    font: FontKey,
    /// The font itself, so that the layout can be outlined on its own.
    handle: FontHandle,
    /// Points per font unit.
    scale: f32,
    glyphs: Vec<PlacedGlyph>,
}

/// Shaped and measured text, ready to be outlined.
///
/// Produced by [`FontLibrary::layout`]. A layout holds shared handles to the fonts it was shaped
/// with, so it can be outlined with [`TextLayout::outline`] on any thread, even after the library
/// that made it is gone.
///
/// # Examples
///
/// ```
/// use umber_core::{Font, FontLibrary, TextStyle, Weight};
///
/// let style = TextStyle { font: Font::Inter, weight: Weight::Regular, size: 12.0, tabular_figures: false };
/// let layout = FontLibrary::global().layout("Hg", &style);
/// assert!(layout.width() > 0.0 && layout.ascent() > 0.0 && layout.descent() > 0.0);
/// ```
#[derive(Debug, Clone)]
pub struct TextLayout {
    runs: Vec<GlyphRun>,
    width: f32,
    ascent: f32,
    descent: f32,
}

impl TextLayout {
    /// The total advance width in points.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Font, FontLibrary, TextStyle, Weight};
    ///
    /// let style = TextStyle { font: Font::Inter, weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let lib = FontLibrary::global();
    /// assert!(lib.layout("wide text", &style).width() > lib.layout("w", &style).width());
    /// ```
    #[must_use]
    pub fn width(&self) -> f32 {
        self.width
    }

    /// The primary font's ascender above the baseline in points, positive.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Font, FontLibrary, TextStyle, Weight};
    ///
    /// let style = TextStyle { font: Font::SourceSerif4, weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let ascent = FontLibrary::global().layout("", &style).ascent();
    /// assert!(ascent > 5.0 && ascent < 15.0);
    /// ```
    #[must_use]
    pub fn ascent(&self) -> f32 {
        self.ascent
    }

    /// The primary font's descender below the baseline in points, positive.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Font, FontLibrary, TextStyle, Weight};
    ///
    /// let style = TextStyle { font: Font::IbmPlexSans, weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let descent = FontLibrary::global().layout("g", &style).descent();
    /// assert!(descent > 0.0 && descent < 5.0);
    /// ```
    #[must_use]
    pub fn descent(&self) -> f32 {
        self.descent
    }

    /// Whether the layout has no glyphs: the text was empty or the size was not positive.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Font, FontLibrary, TextStyle, Weight};
    ///
    /// let style = TextStyle { font: Font::Geist, weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// assert!(FontLibrary::global().layout("", &style).is_empty());
    /// assert!(!FontLibrary::global().layout("x", &style).is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|run| run.glyphs.is_empty())
    }

    /// Converts the layout into glyph outlines, in points, y-down.
    ///
    /// The baseline is at `origin.y` and the text starts at `origin.x`. Glyphs without outlines,
    /// such as spaces, contribute nothing. System fonts are read again here; if such a font file
    /// changed or disappeared since the layout, its glyphs come from the new file or are left
    /// out.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Font, FontLibrary, Point, TextStyle, Weight};
    ///
    /// let style = TextStyle { font: Font::Inter, weight: Weight::Regular, size: 12.0, tabular_figures: false };
    /// let layout = FontLibrary::global().layout("H", &style);
    /// let (min, max) = layout.outline(Point { x: 10.0, y: 30.0 }).bounds().unwrap();
    /// assert!(min.x >= 10.0 && max.x <= 10.0 + layout.width());
    /// assert!(min.y < 30.0 && max.y <= 30.0 + 0.01);
    /// ```
    #[must_use]
    pub fn outline(&self, origin: Point) -> Path {
        let mut run_paths: Vec<Path> = self.runs.iter().map(|_| Path::new()).collect();
        for (_, members) in group_by_font(self.runs.iter().map(|run| run.font)) {
            let Some(handle) = members.first().and_then(|&i| self.runs.get(i)) else {
                continue;
            };
            let handle = &handle.handle;
            let draw = |data: &[u8], index: u32| {
                let outlines = FontRef::from_index(data, index).ok()?.outline_glyphs();
                let paths: Vec<(usize, Path)> = members
                    .iter()
                    .filter_map(|&i| {
                        let run = self.runs.get(i)?;
                        let mut path = Path::new();
                        outline_run(&outlines, run, origin, &mut path);
                        Some((i, path))
                    })
                    .collect();
                Some(paths)
            };
            let drawn = if handle.is_bundled() {
                handle.with_data(draw).flatten()
            } else {
                handle
                    .with_data(|data, index| guarded(|| draw(data, index)).flatten())
                    .flatten()
            };
            for (i, path) in drawn.unwrap_or_default() {
                if let Some(slot) = run_paths.get_mut(i) {
                    *slot = path;
                }
            }
        }
        let mut path = Path::new();
        for run_path in &run_paths {
            path.extend(run_path);
        }
        path
    }
}

/// The most `with_fallbacks` outline confirmations one layout call performs: checks that a
/// fallback font's glyph for a character has an outline. Beyond it, characters skip those
/// fonts and continue to the system fonts and the primary font's `.notdef`.
const EXTRA_CONFIRMATIONS_PER_LAYOUT: usize = 4096;

/// The most entries in the cache of `with_fallbacks` confirmations. When it is full, new
/// results are not cached; nothing is evicted.
const EXTRA_CACHE_LIMIT: usize = 65_536;

/// Number of bundled font files: every family in every weight.
const BUNDLED_COUNT: usize = Font::ALL.len() * Weight::ALL.len();

/// Shapes and measures text with the bundled fonts and fallbacks.
///
/// `FontLibrary` is `Send + Sync`; one library can lay out text from several threads at once,
/// with deterministic results.
///
/// # Examples
///
/// ```
/// use umber_core::{Font, FontLibrary, TextStyle, Weight};
///
/// let lib = FontLibrary::with_fallbacks(Vec::new());
/// let style = TextStyle { font: Font::JetBrainsMono, weight: Weight::Regular, size: 9.0, tabular_figures: true };
/// assert!(lib.layout("1,234.5", &style).width() > 0.0);
/// ```
pub struct FontLibrary {
    /// Indexed by [`bundled_slot`]. `None` if the font failed to parse.
    bundled: [Option<BundledFace>; BUNDLED_COUNT],
    /// Fallback font files that parsed, in the caller's order.
    extra: Vec<ExtraFont>,
    /// `(character, weight, extra font)` to whether that font's glyph for the character has an
    /// outline; at most [`EXTRA_CACHE_LIMIT`] entries.
    extra_cache: Mutex<HashMap<(char, Weight, usize), bool>>,
    /// Outline confirmations performed on the fallback fonts, for tests.
    extra_confirmations: AtomicUsize,
    /// `Some` for [`FontLibrary::bundled`]; `None` disables system lookup.
    system: Option<SystemFonts>,
}

impl std::fmt::Debug for FontLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontLibrary")
            .field("extra_fonts", &self.extra.len())
            .field("system_fallback", &self.system.is_some())
            .finish_non_exhaustive()
    }
}

/// A bundled font, parsed once per library.
struct BundledFace {
    shaping: ShapingFont,
    face: Face<'static>,
    units_per_em: u16,
    /// Ascender and descender in font units, from the font's metrics; descender is negative.
    ascender: f32,
    descender: f32,
}

impl BundledFace {
    fn new(bytes: &'static [u8]) -> Option<BundledFace> {
        let font = FontRef::new(bytes).ok()?;
        let metrics = font.metrics(Size::unscaled(), LocationRef::default());
        Some(BundledFace {
            shaping: ShapingFont::new(bytes, 0)?,
            face: Face::parse(bytes, 0)?,
            units_per_em: metrics.units_per_em,
            ascender: metrics.ascent,
            descender: metrics.descent,
        })
    }
}

/// A font passed to [`FontLibrary::with_fallbacks`]: its bytes, shared with layouts, and the
/// shaping font built from the same bytes.
struct ExtraFont {
    bytes: Arc<Vec<u8>>,
    shaping: ShapingFont,
}

fn bundled_slot(font: Font, weight: Weight) -> usize {
    let family = match font {
        Font::SourceSerif4 => 0,
        Font::IbmPlexSans => 1,
        Font::Geist => 2,
        Font::Inter => 3,
        Font::JetBrainsMono => 4,
    };
    let weight = match weight {
        Weight::Regular => 0,
        Weight::SemiBold => 1,
    };
    family * Weight::ALL.len() + weight
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `f`, which parses or shapes font bytes that did not ship with the crate, and turns a
/// panic into `None`.
///
/// Malformed fonts could trigger panics inside the font parser or the shaper, for example
/// arithmetic overflow in debug builds. Such a font is treated as unreadable. The panic hook is
/// left alone, so the default or host hook may still print a message for the caught panic. With
/// `panic = "abort"` nothing can be caught.
fn guarded<T>(f: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(f)).ok()
}

/// Whether `c` is a nonspacing or enclosing mark (General Category Mn or Me).
fn is_mark(c: char) -> bool {
    let cp = u32::from(c);
    let after = marks::MARKS.partition_point(|&(lo, _)| lo <= cp);
    after > 0 && marks::MARKS.get(after - 1).is_some_and(|&(_, hi)| cp <= hi)
}

/// Characters that stay in the run of the character before them instead of choosing a font of
/// their own: marks (Mn, Me) that are not alphanumeric, emoji modifiers and tags, joiners,
/// invisible format characters and controls. Variation selectors are marks.
///
/// This keeps a base character and its marks in one font, and keeps invisible characters from
/// triggering a fallback search.
fn attaches_to_previous(c: char) -> bool {
    if c.is_control() {
        return true;
    }
    if c.is_alphanumeric() {
        return false;
    }
    is_mark(c)
        || matches!(
            u32::from(c),
            0x1F3FB..=0x1F3FF // Emoji skin tone modifiers
                | 0xE0001 | 0xE0020..=0xE007F // Tags
                | 0x00AD // Soft hyphen
                | 0x061C // Arabic letter mark
                | 0x180E // Mongolian vowel separator
                | 0x200B..=0x200F // Zero-width space, non-joiner, joiner, direction marks
                | 0x202A..=0x202E // Bidi embeddings and overrides
                | 0x2060..=0x206F // Word joiner, invisible operators, bidi isolates
                | 0xFEFF // Zero-width no-break space
        )
}

/// An [`OutlinePen`] that counts path commands, used to test whether a glyph has an outline.
struct SegmentCount(usize);

impl OutlinePen for SegmentCount {
    fn move_to(&mut self, _: f32, _: f32) {
        self.0 += 1;
    }
    fn line_to(&mut self, _: f32, _: f32) {
        self.0 += 1;
    }
    fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {
        self.0 += 1;
    }
    fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
        self.0 += 1;
    }
    fn close(&mut self) {
        self.0 += 1;
    }
}

/// Maps font units to points: `x * scale + dx`, `-y * scale + dy` (y-down).
struct PathBuilder {
    path: Path,
    scale: f32,
    dx: f32,
    dy: f32,
}

impl PathBuilder {
    fn point(&self, x: f32, y: f32) -> Point {
        Point {
            x: x * self.scale + self.dx,
            y: -y * self.scale + self.dy,
        }
    }
}

impl OutlinePen for PathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.path.move_to(p);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.path.line_to(p);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (c, p) = (self.point(x1, y1), self.point(x, y));
        self.path.quad_to(c, p);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1, c2, p) = (self.point(x1, y1), self.point(x2, y2), self.point(x, y));
        self.path.cubic_to(c1, c2, p);
    }

    fn close(&mut self) {
        self.path.close();
    }
}

/// One shaped glyph, in font units.
#[derive(Debug, Clone, Copy)]
struct ShapedGlyph {
    id: u16,
    advance: i32,
    x_offset: i32,
    y_offset: i32,
}

/// The shaped glyphs of one run, with the font's units per em.
struct ShapedRun {
    units_per_em: i32,
    glyphs: Vec<ShapedGlyph>,
}

/// Shapes several runs of text with one font, reusing shape plans across runs.
struct Shaper<'f> {
    font: &'f ShapingFont,
    shaper: ShaperFont<'f, 'f>,
    features: &'f [Feature],
    plans: Vec<(Direction, Option<Script>, ShapePlan)>,
}

impl<'f> Shaper<'f> {
    fn new(font: &'f ShapingFont, funcs: &'f dyn FontFuncs, features: &'f [Feature]) -> Self {
        Shaper {
            font,
            shaper: ShaperFont::new(font).with_font_funcs(Some(funcs)),
            features,
            plans: Vec::new(),
        }
    }

    /// Shapes `text`; `None` if the shaper refuses the buffer.
    fn shape(&mut self, text: &str, preserve_ignorables: bool) -> Option<ShapedRun> {
        let mut buffer = Buffer::new();
        buffer.push_str(text);
        if preserve_ignorables {
            buffer.set_flags(BufferFlags::PRESERVE_DEFAULT_IGNORABLES);
        }
        // Always sets a direction, left to right when the text has none of its own.
        buffer.guess_segment_properties();
        let direction = buffer.direction();
        let script = buffer.script();
        let index = match self
            .plans
            .iter()
            .position(|(d, s, _)| *d == direction && *s == script)
        {
            Some(index) => index,
            None => {
                let plan = ShapePlan::new(self.font, direction, script, None, self.features);
                self.plans.push((direction, script, plan));
                self.plans.len() - 1
            }
        };
        let (_, _, plan) = self.plans.get(index)?;
        let options = ShapeOptions::new().plan(Some(plan)).features(self.features);
        harfrust::shape(&self.shaper, &mut buffer, options).ok()?;
        let glyphs = buffer
            .glyph_infos()
            .iter()
            .zip(buffer.glyph_positions())
            .map(|(info, pos)| ShapedGlyph {
                id: u16::try_from(info.glyph_id).unwrap_or(0),
                advance: pos.x_advance,
                x_offset: pos.x_offset,
                y_offset: pos.y_offset,
            })
            .collect();
        Some(ShapedRun {
            units_per_em: i32::from(self.font.units_per_em()),
            glyphs,
        })
    }
}

/// Fallback faces parsed for the duration of one layout call.
type ParsedExtras<'a> = Vec<Option<Face<'a>>>;

/// A shaped run waiting to be placed: its font and glyphs.
type ShapedSlot = Option<(FontKey, FontHandle, ShapedRun)>;

impl FontLibrary {
    fn new(extra: Vec<Vec<u8>>, system: Option<SystemFonts>) -> FontLibrary {
        let mut bundled: [Option<BundledFace>; BUNDLED_COUNT] = std::array::from_fn(|_| None);
        for font in Font::ALL {
            for weight in Weight::ALL {
                if let Some(slot) = bundled.get_mut(bundled_slot(font, weight)) {
                    *slot = BundledFace::new(font_bytes(font, weight));
                }
            }
        }
        let extra = extra
            .into_iter()
            .filter_map(|bytes| {
                let bytes = Arc::new(bytes);
                let shared: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::clone(&bytes) as _;
                let shaping = guarded(|| {
                    FontRef::new(&bytes).ok()?;
                    ShapingFont::new(Blob::from(shared), 0)
                })
                .flatten()?;
                Some(ExtraFont { bytes, shaping })
            })
            .collect();
        FontLibrary {
            bundled,
            extra,
            extra_cache: Mutex::new(HashMap::new()),
            extra_confirmations: AtomicUsize::new(0),
            system,
        }
    }

    /// The bundled fonts, falling back to the system fonts for characters none of them has.
    ///
    /// The system fonts are only enumerated and indexed the first time a character is missing
    /// from every bundled font, once per process: every `bundled()` library shares the index. A missing character then tries, in order: a ranked family list
    /// for its script (Han, Kana, Hangul, Arabic, Hebrew), the generic families Noto Sans,
    /// DejaVu Sans and Arial Unicode MS, and every other face sorted by family name. Within a
    /// family the face closest to the requested weight wins, upright before italic. Results are
    /// cached per character and weight, up to 65,536 entries; beyond that new results are not
    /// cached. The index holds at most 10,000 faces and skips font files over 256 MiB and faces
    /// with more than 65,536 coverage ranges; the shared index is always built with these
    /// default limits. Prefer [`FontLibrary::global`], which shares one
    /// library and its caches.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Font, FontLibrary, TextStyle, Weight};
    ///
    /// let lib = FontLibrary::bundled();
    /// let style = TextStyle { font: Font::Inter, weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// assert!(lib.layout("Profit", &style).width() > 0.0);
    /// ```
    #[must_use]
    pub fn bundled() -> FontLibrary {
        FontLibrary::new(
            Vec::new(),
            Some(SystemFonts::new(system::Source::Os, Limits::DEFAULT)),
        )
    }

    /// Like [`FontLibrary::bundled`], with in-memory fonts standing in for the system fonts.
    #[cfg(test)]
    fn with_test_system_fonts(fonts: Vec<Vec<u8>>) -> FontLibrary {
        FontLibrary::with_test_system_fonts_and_limits(fonts, Limits::DEFAULT)
    }

    /// Like [`FontLibrary::with_test_system_fonts`], with custom index limits.
    #[cfg(test)]
    fn with_test_system_fonts_and_limits(fonts: Vec<Vec<u8>>, limits: Limits) -> FontLibrary {
        FontLibrary::new(
            Vec::new(),
            Some(SystemFonts::new(system::Source::Memory(fonts), limits)),
        )
    }

    /// The bundled fonts plus the given fallback font files, without any system font lookup.
    ///
    /// Fallbacks are tried in order. Bytes that do not parse as a font are ignored, and a
    /// fallback is only used for a character it can draw with an outline. This gives
    /// reproducible output regardless of the fonts installed on the machine. Whether a
    /// fallback's glyph has an outline is cached per character and weight, up to 65,536
    /// entries; beyond that new results are not cached. A font's character map is read from
    /// its best Unicode subtable and at most 16 other distinct Unicode subtables; further
    /// subtables are ignored.
    ///
    /// Fonts that make the parser or shaper panic (possible in debug builds) are skipped. The
    /// panic is caught, but the process's panic hook still runs, so a message may be printed.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Font, FontLibrary, TextStyle, Weight};
    ///
    /// let lib = FontLibrary::with_fallbacks(vec![b"not a font".to_vec()]);
    /// let style = TextStyle { font: Font::Inter, weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// assert!(lib.layout("AB", &style).width() > 0.0);
    /// ```
    #[must_use]
    pub fn with_fallbacks(fonts: Vec<Vec<u8>>) -> FontLibrary {
        FontLibrary::new(fonts, None)
    }

    /// A process-wide [`FontLibrary::bundled`] library, created on first use.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::FontLibrary;
    ///
    /// assert!(std::ptr::eq(FontLibrary::global(), FontLibrary::global()));
    /// ```
    #[must_use]
    pub fn global() -> &'static FontLibrary {
        static GLOBAL: OnceLock<FontLibrary> = OnceLock::new();
        GLOBAL.get_or_init(FontLibrary::bundled)
    }

    fn bundled_face(&self, font: Font, weight: Weight) -> Option<&BundledFace> {
        self.bundled
            .get(bundled_slot(font, weight))
            .and_then(Option::as_ref)
    }

    /// A shared handle to the font behind `key`.
    fn handle(&self, key: FontKey) -> Option<FontHandle> {
        match key {
            FontKey::Bundled(font, weight) => Some(FontHandle::bundled(font, weight)),
            FontKey::Extra(index) => self.extra.get(index).map(|extra| FontHandle {
                data: FontData::Extra(Arc::clone(&extra.bytes)),
                index: 0,
            }),
            FontKey::System(face) => self.system.as_ref()?.handle(face),
        }
    }

    /// Shapes `text` with `style` and measures it.
    ///
    /// Characters missing from `style.font` are drawn with the other bundled families in the
    /// same weight, then with the fallback fonts, and as `.notdef` if no font has them.
    ///
    /// One call checks at most 4,096 uncached glyph outlines in the
    /// [`with_fallbacks`](FontLibrary::with_fallbacks) fonts; beyond that, characters skip
    /// those fonts. It looks up at most 1,024 distinct characters in the system fonts that are
    /// not already cached, and opens at most 4,096 candidate system fonts to confirm a glyph
    /// (at most 16 per character). Characters beyond these limits are drawn as `.notdef`. This
    /// bounds the work a single string can cause.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Font, FontLibrary, TextStyle, Weight};
    ///
    /// let lib = FontLibrary::global();
    /// let style = TextStyle { font: Font::Inter, weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let small = lib.layout("Signal", &style).width();
    /// let large = lib.layout("Signal", &TextStyle { size: 20.0, ..style }).width();
    /// assert!((large - 2.0 * small).abs() < 1e-3);
    /// ```
    #[must_use]
    pub fn layout(&self, text: &str, style: &TextStyle) -> TextLayout {
        let size = if style.size.is_finite() && style.size > 0.0 {
            style.size
        } else {
            0.0
        };
        let primary = self.bundled_face(style.font, style.weight);
        let (ascent, descent) = primary.map_or((0.0, 0.0), |face| {
            let scale = size / f32::from(face.units_per_em.max(1));
            (
                (face.ascender * scale).max(0.0),
                (-face.descender * scale).max(0.0),
            )
        });
        let mut layout = TextLayout {
            runs: Vec::new(),
            width: 0.0,
            ascent,
            descent,
        };
        if size == 0.0 || text.is_empty() {
            return layout;
        }

        let runs = self.split_runs(text, style);
        let features = features(style.tabular_figures);
        let primary_key = FontKey::Bundled(style.font, style.weight);
        let mut shaped: Vec<ShapedSlot> = runs.iter().map(|_| None).collect();
        let extents_budget = Cell::new(EXTENTS_PER_LAYOUT);
        for (key, members) in group_by_font(runs.iter().map(|run| run.font)) {
            let Some(handle) = self.handle(key) else {
                continue;
            };
            let shape_all = |font: &ShapingFont, face: &Face<'_>| {
                let funcs = if matches!(key, FontKey::System(_)) {
                    Funcs::system(face, &extents_budget)
                } else {
                    Funcs::new(face)
                };
                let mut shaper = Shaper::new(font, &funcs, &features);
                members
                    .iter()
                    .filter_map(|&i| {
                        let run = runs.get(i)?;
                        let run_text = text.get(run.range.clone())?;
                        Some((i, shaper.shape(run_text, run.preserve_ignorables)?))
                    })
                    .collect::<Vec<_>>()
            };
            let results = match key {
                FontKey::Bundled(font, weight) => self
                    .bundled_face(font, weight)
                    .map(|bundled| shape_all(&bundled.shaping, &bundled.face)),
                // Fonts that did not ship with the crate are parsed and shaped under a guard.
                FontKey::Extra(index) => self.extra.get(index).and_then(|extra| {
                    guarded(|| Face::parse(&extra.bytes, 0).map(|f| shape_all(&extra.shaping, &f)))
                        .flatten()
                }),
                FontKey::System(face) => self.system.as_ref().and_then(|system| {
                    let shaping = system.shaping_font(face)?;
                    let results = handle
                        .with_data(|data, index| {
                            guarded(|| {
                                Face::parse(data, index).map(|f| shape_all(&shaping.font, &f))
                            })
                            .flatten()
                        })
                        .flatten()?;
                    // Only a font that shaped without panicking is kept for later layouts.
                    system.keep_shaping_font(face, shaping);
                    Some(results)
                }),
            };
            for (i, run) in results.unwrap_or_default() {
                if let Some(slot) = shaped.get_mut(i) {
                    *slot = Some((key, handle.clone(), run));
                }
            }
        }

        // A fallback font that could not be read again (for example a system font file that
        // disappeared, or a font that panicked while shaping) is replaced by the primary font,
        // so the text keeps a width.
        if let Some(face) = primary {
            let funcs = Funcs::new(&face.face);
            let mut shaper = Shaper::new(&face.shaping, &funcs, &features);
            let handle = FontHandle::bundled(style.font, style.weight);
            for (slot, run) in shaped.iter_mut().zip(&runs) {
                if slot.is_none()
                    && let Some(glyphs) = text
                        .get(run.range.clone())
                        .and_then(|run_text| shaper.shape(run_text, run.preserve_ignorables))
                {
                    *slot = Some((primary_key, handle.clone(), glyphs));
                }
            }
        }

        let mut pen = 0.0_f32;
        for (key, handle, run) in shaped.into_iter().flatten() {
            let scale = size / run.units_per_em.max(1) as f32;
            let mut units: i64 = 0;
            let glyphs = run
                .glyphs
                .iter()
                .map(|g| {
                    let placed = PlacedGlyph {
                        id: g.id,
                        x: pen + (units + i64::from(g.x_offset)) as f32 * scale,
                        y: -(g.y_offset as f32) * scale,
                    };
                    units += i64::from(g.advance);
                    placed
                })
                .collect();
            pen += units as f32 * scale;
            layout.runs.push(GlyphRun {
                font: key,
                handle,
                scale,
                glyphs,
            });
        }
        layout.width = pen;
        layout
    }

    /// Splits `text` into byte ranges, each drawn with one font.
    ///
    /// Characters that [attach to the previous one](attaches_to_previous) join its run. At the
    /// very start of the text they have nothing to attach to, so they form a primary-font run
    /// that keeps them visible (as `.notdef` when the font lacks them) instead of shaping to
    /// nothing.
    fn split_runs(&self, text: &str, style: &TextStyle) -> Vec<TextRun> {
        let primary_key = FontKey::Bundled(style.font, style.weight);
        let primary = self.bundled_face(style.font, style.weight);
        let mut extras: Option<ParsedExtras<'_>> = None;
        let mut chosen: HashMap<char, FontKey> = HashMap::new();
        let mut budget = Budget::per_layout();
        let mut extra_budget = EXTRA_CONFIRMATIONS_PER_LAYOUT;
        let mut runs: Vec<TextRun> = Vec::new();
        for (start, c) in text.char_indices() {
            let end = start + c.len_utf8();
            let (key, preserve) = if attaches_to_previous(c) {
                runs.last().map_or((primary_key, true), |run| {
                    (run.font, run.preserve_ignorables)
                })
            } else if primary.is_some_and(|face| face.face.glyph(c).is_some()) {
                (primary_key, false)
            } else if let Some(key) = chosen.get(&c) {
                (*key, false)
            } else {
                let key = self
                    .fallback_for(c, style, &mut extras, &mut extra_budget, &mut budget)
                    .unwrap_or(primary_key);
                chosen.insert(c, key);
                (key, false)
            };
            match runs.last_mut() {
                Some(last) if last.font == key && last.preserve_ignorables == preserve => {
                    last.range.end = end;
                }
                _ => runs.push(TextRun {
                    font: key,
                    preserve_ignorables: preserve,
                    range: start..end,
                }),
            }
        }
        runs
    }

    /// The first font after the primary one that can draw `c`: (a) the other bundled families,
    /// (b) the `with_fallbacks` fonts, then the system fonts. Fallback and system fonts must
    /// have an outline for the glyph. Uncached fallback font confirmations spend
    /// `extra_budget`, and system font lookups spend `budget`.
    fn fallback_for<'a>(
        &'a self,
        c: char,
        style: &TextStyle,
        extras: &mut Option<ParsedExtras<'a>>,
        extra_budget: &mut usize,
        budget: &mut Budget,
    ) -> Option<FontKey> {
        for font in Font::ALL {
            if font == style.font {
                continue;
            }
            if self
                .bundled_face(font, style.weight)
                .is_some_and(|face| face.face.glyph(c).is_some())
            {
                return Some(FontKey::Bundled(font, style.weight));
            }
        }
        let extras = extras.get_or_insert_with(|| {
            self.extra
                .iter()
                .map(|extra| guarded(|| Face::parse(&extra.bytes, 0)).flatten())
                .collect()
        });
        for (index, face) in extras.iter().enumerate() {
            // The glyph lookup is cheap and bounded; only the outline check is budgeted.
            let Some(glyph) = face.as_ref().and_then(|f| guarded(|| f.glyph(c)).flatten()) else {
                continue;
            };
            let key = (c, style.weight, index);
            let cached = lock(&self.extra_cache).get(&key).copied();
            let draws = match cached {
                Some(draws) => draws,
                // Out of confirmations: skip the remaining fallback fonts.
                None if *extra_budget == 0 => break,
                None => {
                    *extra_budget -= 1;
                    self.extra_confirmations.fetch_add(1, Ordering::Relaxed);
                    let draws = face.as_ref().is_some_and(|f| {
                        guarded(|| c.is_whitespace() || f.has_outline(glyph)).unwrap_or(false)
                    });
                    let mut cache = lock(&self.extra_cache);
                    if cache.len() < EXTRA_CACHE_LIMIT {
                        cache.insert(key, draws);
                    }
                    draws
                }
            };
            if draws {
                return Some(FontKey::Extra(index));
            }
        }
        self.system
            .as_ref()?
            .face_for(c, style.weight, budget)
            .map(FontKey::System)
    }
}

/// Appends the outlines of every glyph of `run` to `path`.
fn outline_run(
    outlines: &OutlineGlyphCollection<'_>,
    run: &GlyphRun,
    origin: Point,
    path: &mut Path,
) {
    for glyph in &run.glyphs {
        let mut builder = PathBuilder {
            path: Path::new(),
            scale: run.scale,
            dx: origin.x + glyph.x,
            dy: origin.y + glyph.y,
        };
        // A glyph whose outline fails part-way contributes nothing rather than a broken shape.
        let drawn = outlines
            .get(GlyphId::new(u32::from(glyph.id)))
            .is_some_and(|outline| outline.draw(unhinted(), &mut builder).is_ok());
        if drawn {
            path.extend(&builder.path);
        }
    }
}

/// The indices of the items of `keys`, grouped by key, groups in order of first appearance.
///
/// One pass with a hash map, so the cost is linear in the number of runs whatever the number of
/// distinct fonts.
fn group_by_font(keys: impl Iterator<Item = FontKey>) -> Vec<(FontKey, Vec<usize>)> {
    let mut slots: HashMap<FontKey, usize> = HashMap::new();
    let mut groups: Vec<(FontKey, Vec<usize>)> = Vec::new();
    for (i, key) in keys.enumerate() {
        let slot = *slots.entry(key).or_insert_with(|| {
            groups.push((key, Vec::new()));
            groups.len() - 1
        });
        if let Some((_, members)) = groups.get_mut(slot) {
            members.push(i);
        }
    }
    groups
}

/// The OpenType features applied to every run.
fn features(tabular_figures: bool) -> Vec<Feature> {
    let mut features = vec![
        Feature::new(Tag::new(b"kern"), 1, ..),
        Feature::new(Tag::new(b"liga"), 1, ..),
    ];
    if tabular_figures {
        features.push(Feature::new(Tag::new(b"tnum"), 1, ..));
    }
    features
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{Font, Weight};

    fn style(font: Font, size: f32) -> TextStyle {
        TextStyle {
            font,
            weight: Weight::Regular,
            size,
            tabular_figures: false,
        }
    }
    fn omega_lib() -> FontLibrary {
        FontLibrary::with_fallbacks(vec![
            include_bytes!("../tests/fixtures/fallback-only.ttf").to_vec(),
        ])
    }

    #[test]
    fn width_scales_linearly_with_size() {
        let lib = FontLibrary::with_fallbacks(vec![]);
        let a = lib.layout("Signal 2026", &style(Font::Inter, 10.0)).width();
        let b = lib.layout("Signal 2026", &style(Font::Inter, 20.0)).width();
        assert!(a > 0.0 && (b - 2.0 * a).abs() < 1e-3 * b, "{a} {b}");
    }

    #[test]
    fn kerning_is_applied() {
        let lib = FontLibrary::with_fallbacks(vec![]);
        let s = style(Font::Inter, 20.0);
        let av = lib.layout("AV", &s).width();
        let sep = lib.layout("A", &s).width() + lib.layout("V", &s).width();
        assert!(av < sep, "AV {av} vs A+V {sep}");
    }

    #[test]
    fn tabular_figures_have_equal_widths() {
        let lib = FontLibrary::with_fallbacks(vec![]);
        for font in Font::ALL {
            let s = TextStyle {
                font,
                weight: Weight::Regular,
                size: 10.0,
                tabular_figures: true,
            };
            let one = lib.layout("1111", &s).width();
            let eight = lib.layout("8888", &s).width();
            assert!((one - eight).abs() < 1e-3, "{font:?}: {one} vs {eight}");
        }
    }

    #[test]
    fn outline_sits_on_the_baseline_within_the_advance() {
        let lib = FontLibrary::with_fallbacks(vec![]);
        let layout = lib.layout("Hg", &style(Font::Inter, 12.0));
        let path = layout.outline(Point { x: 5.0, y: 50.0 });
        let (min, max) = path.bounds().expect("non-empty");
        assert!(min.x >= 5.0 - 1.0 && max.x <= 5.0 + layout.width() + 1.0);
        assert!(
            min.y < 50.0 && max.y > 50.0,
            "H above and g below the baseline"
        );
        assert!(min.y >= 50.0 - layout.ascent() - 1.0 && max.y <= 50.0 + layout.descent() + 1.0);
    }

    #[test]
    fn empty_and_degenerate_sizes() {
        let lib = FontLibrary::with_fallbacks(vec![]);
        let empty = lib.layout("", &style(Font::Geist, 10.0));
        assert_eq!(empty.width(), 0.0);
        assert!(empty.ascent() > 0.0);
        assert!(empty.outline(Point { x: 0.0, y: 0.0 }).is_empty());
        for size in [0.0, -3.0, f32::NAN] {
            let l = lib.layout("abc", &style(Font::Geist, size));
            assert_eq!(l.width(), 0.0);
            assert!(l.outline(Point { x: 0.0, y: 0.0 }).is_empty());
        }
    }

    #[test]
    fn missing_glyph_uses_fallback_font() {
        let text = format!("A{}", crate::fonts::FALLBACK_TEST_CODEPOINT);
        let with = omega_lib().layout(&text, &style(Font::Inter, 10.0));
        let a_only = omega_lib().layout("A", &style(Font::Inter, 10.0)).width();
        assert!(with.width() > a_only);
        // The fallback glyph is an outlined Ω, so the outline must extend past the "A".
        let path = with.outline(Point { x: 0.0, y: 0.0 });
        assert!(path.bounds().expect("outline").1.x > a_only);
        // Without the fallback the code point is drawn as .notdef: width still positive, no panic.
        let without = FontLibrary::with_fallbacks(vec![]).layout(&text, &style(Font::Inter, 10.0));
        assert!(without.width() > a_only);
    }

    #[test]
    fn corrupt_fallback_bytes_are_ignored() {
        let lib = FontLibrary::with_fallbacks(vec![vec![0, 1, 2, 3], Vec::new()]);
        assert!(lib.layout("AB", &style(Font::Inter, 10.0)).width() > 0.0);
    }

    #[test]
    fn unsupported_text_never_panics() {
        let lib = FontLibrary::with_fallbacks(vec![]);
        for text in [
            "数据可视化",
            "👍🏽",
            "مرحبا",
            "e\u{301}",
            "\u{FE0F}",
            "a\u{200D}b",
        ] {
            let l = lib.layout(text, &style(Font::IbmPlexSans, 10.0));
            assert!(l.width() > 0.0, "{text:?}");
            let _ = l.outline(Point { x: 0.0, y: 0.0 });
        }
    }

    #[test]
    fn concurrent_layout_is_deterministic() {
        let lib = FontLibrary::with_fallbacks(vec![]);
        let s = style(Font::SourceSerif4, 11.0);
        let reference = lib
            .layout("Revenue −12.5%", &s)
            .outline(Point { x: 0.0, y: 0.0 });
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..50 {
                        let p = lib
                            .layout("Revenue −12.5%", &s)
                            .outline(Point { x: 0.0, y: 0.0 });
                        assert_eq!(p.verbs(), reference.verbs());
                    }
                });
            }
        });
    }

    #[test]
    fn long_text_layout_is_fast_enough() {
        let lib = omega_lib();
        let text: String = format!("{}数a", crate::fonts::FALLBACK_TEST_CODEPOINT).repeat(3_334);
        let start = std::time::Instant::now();
        let l = lib.layout(&text, &style(Font::Inter, 10.0));
        assert!(l.width() > 0.0);
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn global_library_is_shared() {
        assert!(std::ptr::eq(FontLibrary::global(), FontLibrary::global()));
    }

    #[test]
    fn font_library_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FontLibrary>();
        assert_send_sync::<TextLayout>();
    }

    #[test]
    fn every_bundled_font_is_loaded_in_its_own_slot() {
        let lib = FontLibrary::with_fallbacks(vec![]);
        let mut slots: Vec<usize> = Font::ALL
            .iter()
            .flat_map(|&f| Weight::ALL.iter().map(move |&w| bundled_slot(f, w)))
            .collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), BUNDLED_COUNT);
        assert!(lib.bundled.iter().all(Option::is_some));
    }

    #[test]
    fn missing_characters_fall_back_to_other_bundled_families_first() {
        let lib = FontLibrary::with_fallbacks(vec![]);
        let mut checked = 0;
        for font in Font::ALL {
            let face = lib
                .bundled_face(font, Weight::Regular)
                .expect("bundled font parses");
            let missing = crate::fonts::EXTENDED_CODEPOINTS
                .iter()
                .flat_map(|&(lo, hi)| lo..=hi)
                .filter_map(char::from_u32)
                .find(|&c| face.face.glyph(c).is_none() && !attaches_to_previous(c));
            let Some(c) = missing else { continue };
            let layout = lib.layout(&format!("a{c}"), &style(font, 10.0));
            let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
            assert_eq!(fonts.len(), 2, "{font:?} U+{:04X}: {fonts:?}", u32::from(c));
            assert_eq!(fonts[0], FontKey::Bundled(font, Weight::Regular));
            assert!(
                matches!(fonts[1], FontKey::Bundled(other, Weight::Regular) if other != font),
                "{font:?} U+{:04X}: {fonts:?}",
                u32::from(c)
            );
            checked += 1;
        }
        assert!(checked > 0, "every family covers every extended code point");
    }

    #[test]
    fn marks_and_invisible_characters_stay_in_the_previous_run() {
        let lib = omega_lib();
        let fallback = crate::fonts::FALLBACK_TEST_CODEPOINT;
        let s = style(Font::Inter, 10.0);
        for text in [
            format!("{fallback}\u{301}"),
            format!("{fallback}\u{FE0F}"),
            format!("{fallback}\u{200D}"),
        ] {
            let layout = lib.layout(&text, &s);
            assert_eq!(layout.runs.len(), 1, "{text:?}");
            assert_eq!(layout.runs[0].font, FontKey::Extra(0), "{text:?}");
        }
        // A hidden joiner adds no width.
        let joined = lib.layout("a\u{200D}b", &s).width();
        assert!((joined - lib.layout("ab", &s).width()).abs() < 1e-4);
    }

    #[test]
    fn repeated_fallback_characters_form_one_run() {
        let fallback = crate::fonts::FALLBACK_TEST_CODEPOINT;
        let text = format!("ab{fallback}{fallback}{fallback}cd");
        let layout = omega_lib().layout(&text, &style(Font::Geist, 10.0));
        let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
        let primary = FontKey::Bundled(Font::Geist, Weight::Regular);
        assert_eq!(fonts, [primary, FontKey::Extra(0), primary]);
    }

    #[test]
    fn layout_is_empty_only_without_glyphs() {
        let lib = FontLibrary::with_fallbacks(vec![]);
        assert!(lib.layout("", &style(Font::Inter, 10.0)).is_empty());
        assert!(lib.layout("abc", &style(Font::Inter, 0.0)).is_empty());
        assert!(!lib.layout("abc", &style(Font::Inter, 10.0)).is_empty());
        assert!(lib.layout("", &style(Font::Inter, f32::INFINITY)).ascent() == 0.0);
    }

    // --- System fallback, with in-memory fonts standing in for the system fonts. ---

    fn be16(data: &[u8], at: usize) -> usize {
        usize::from(u16::from_be_bytes([data[at], data[at + 1]]))
    }

    fn be32(data: &[u8], at: usize) -> usize {
        u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize
    }

    /// A `cmap` table whose `records` encoding records (Windows, Unicode full repertoire) all
    /// point at one format 12 subtable that maps every code point in `codepoints` to glyph 1.
    fn format12_cmap(codepoints: &[u32], records: u16) -> Vec<u8> {
        let mut points = codepoints.to_vec();
        points.sort_unstable();
        points.dedup();
        // One group per code point: a format 12 group maps consecutive code points to
        // consecutive glyphs, and every code point here must map to glyph 1.
        let groups: Vec<(u32, u32)> = points.iter().map(|&cp| (cp, cp)).collect();
        let mut cmap = Vec::new();
        cmap.extend(0u16.to_be_bytes()); // version
        cmap.extend(records.to_be_bytes());
        let offset = 4 + 8 * u32::from(records);
        for _ in 0..records {
            cmap.extend(3u16.to_be_bytes()); // Windows
            cmap.extend(10u16.to_be_bytes()); // Unicode full repertoire
            cmap.extend(offset.to_be_bytes());
        }
        cmap.extend(12u16.to_be_bytes()); // format 12
        cmap.extend(0u16.to_be_bytes());
        cmap.extend((16 + 12 * groups.len() as u32).to_be_bytes());
        cmap.extend(0u32.to_be_bytes()); // language
        cmap.extend((groups.len() as u32).to_be_bytes());
        for (lo, hi) in groups {
            cmap.extend(lo.to_be_bytes());
            cmap.extend(hi.to_be_bytes());
            cmap.extend(1u32.to_be_bytes()); // glyph 1, the outlined glyph
        }
        cmap
    }

    /// `fallback-only.ttf` rebuilt with a new name, weight and style, and its one outlined glyph
    /// mapped to every code point in `codepoints`.
    fn fixture_font(family: &str, weight: u16, italic: bool, codepoints: &[u32]) -> Vec<u8> {
        font_with_cmap(family, weight, italic, format12_cmap(codepoints, 1))
    }

    /// The tables of a font, in directory order.
    fn tables_of(font: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
        (0..be16(font, 4))
            .map(|i| {
                let record = 12 + 16 * i;
                let tag: [u8; 4] = font[record..record + 4].try_into().unwrap();
                let (offset, length) = (be32(font, record + 8), be32(font, record + 12));
                (tag, font[offset..offset + length].to_vec())
            })
            .collect()
    }

    /// A TrueType font file holding `tables`.
    fn assemble(mut tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
        tables.sort_by_key(|(tag, _)| *tag);
        let count = tables.len();
        let selector = usize::BITS - 1 - count.leading_zeros();
        let search_range = (1usize << selector) * 16;
        let mut font = Vec::new();
        font.extend(0x0001_0000u32.to_be_bytes());
        for field in [
            count,
            search_range,
            selector as usize,
            count * 16 - search_range,
        ] {
            font.extend((field as u16).to_be_bytes());
        }
        let mut offset = 12 + 16 * count;
        let mut body = Vec::new();
        for (tag, data) in &tables {
            let mut padded = data.clone();
            padded.resize(data.len().div_ceil(4) * 4, 0);
            let checksum = padded
                .chunks(4)
                .map(|w| u32::from_be_bytes(w.try_into().unwrap()))
                .fold(0u32, u32::wrapping_add);
            font.extend(tag);
            font.extend(checksum.to_be_bytes());
            font.extend((offset as u32).to_be_bytes());
            font.extend((data.len() as u32).to_be_bytes());
            offset += padded.len();
            body.extend(padded);
        }
        font.extend(body);
        font
    }

    /// `font` with the table `tag` added or replaced.
    fn with_table(font: &[u8], tag: [u8; 4], data: Vec<u8>) -> Vec<u8> {
        let mut tables = tables_of(font);
        tables.retain(|(t, _)| *t != tag);
        tables.push((tag, data));
        assemble(tables)
    }

    /// `fallback-only.ttf` rebuilt with a new name, weight and style, and the given `cmap`.
    fn font_with_cmap(family: &str, weight: u16, italic: bool, cmap: Vec<u8>) -> Vec<u8> {
        let base: &[u8] = include_bytes!("../tests/fixtures/fallback-only.ttf");
        assert_eq!(be32(base, 0), 0x0001_0000);
        let mut tables = tables_of(base);

        let post_script = format!(
            "{}-{weight}{}",
            family.replace(' ', ""),
            if italic { "Italic" } else { "" }
        );
        let names = [
            (1u16, family.to_string()),
            (2, "Regular".into()),
            (4, family.into()),
            (6, post_script),
        ];
        let mut strings = Vec::new();
        let mut name = Vec::new();
        name.extend(0u16.to_be_bytes());
        name.extend((names.len() as u16).to_be_bytes());
        name.extend((6 + 12 * names.len() as u16).to_be_bytes());
        for (id, text) in &names {
            let encoded: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
            for field in [
                3u16,
                1,
                0x409,
                *id,
                encoded.len() as u16,
                strings.len() as u16,
            ] {
                name.extend(field.to_be_bytes());
            }
            strings.extend(encoded);
        }
        name.extend(strings);

        for (tag, data) in &mut tables {
            match &*tag {
                b"cmap" => *data = cmap.clone(),
                b"name" => *data = name.clone(),
                b"OS/2" => {
                    data[4..6].copy_from_slice(&weight.to_be_bytes());
                    let mut selection = u16::from_be_bytes([data[62], data[63]]);
                    selection = if italic {
                        (selection | 1) & !(1 << 6)
                    } else {
                        selection & !1
                    };
                    data[62..64].copy_from_slice(&selection.to_be_bytes());
                }
                b"post" => {
                    let angle: i32 = if italic { -12 << 16 } else { 0 };
                    data[4..8].copy_from_slice(&angle.to_be_bytes());
                }
                _ => {}
            }
        }
        assemble(tables)
    }

    fn assert_no_bundled_font_has(text: &str) {
        let lib = FontLibrary::with_fallbacks(vec![]);
        for c in text.chars() {
            for face in lib.bundled.iter().flatten() {
                assert!(face.face.glyph(c).is_none(), "a bundled font has {c:?}");
            }
        }
    }

    /// The PostScript names of the system faces the runs of `text` were drawn with.
    fn system_picks(lib: &FontLibrary, text: &str, weight: Weight) -> Vec<String> {
        let s = TextStyle {
            weight,
            ..style(Font::Inter, 10.0)
        };
        let system = lib.system.as_ref().unwrap();
        lib.layout(text, &s)
            .runs
            .iter()
            .map(|run| match run.font {
                FontKey::System(face) => system.face(face).unwrap().post_script_name.clone(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    fn lookups(lib: &FontLibrary) -> usize {
        lib.system.as_ref().unwrap().lookups()
    }

    #[test]
    fn fixture_fonts_parse_with_the_requested_properties() {
        let bytes = fixture_font("Test Family", 600, true, &[0xF0000, 0xF0001, 0xF0005]);
        let face = Face::parse(&bytes, 0).unwrap();
        assert!(face.glyph('\u{F0005}').is_some());
        assert!(face.glyph('\u{F0002}').is_none());
        let mut db = fontdb::Database::new();
        db.load_font_data(bytes);
        let info = db.faces().next().unwrap();
        assert_eq!(info.families[0].0, "Test Family");
        assert_eq!(info.post_script_name, "TestFamily-600Italic");
        assert_eq!(info.weight.0, 600);
        assert_ne!(info.style, fontdb::Style::Normal);
    }

    #[test]
    fn system_fallback_prefers_ranked_families_then_sorted_names() {
        let han = '\u{4E00}';
        let (generic, unranked) = ('\u{F0010}', '\u{F0020}');
        assert_no_bundled_font_has(&format!("{han}{generic}{unranked}"));
        // Database order puts the losing face first every time.
        let lib = FontLibrary::with_test_system_fonts(vec![
            fixture_font("Aardvark Sans", 400, false, &[0x4E00, 0xF0010]),
            fixture_font("Zulu Sans", 400, false, &[0xF0020]),
            fixture_font("Hiragino Sans", 400, false, &[0x4E00]),
            fixture_font("PingFang SC", 400, false, &[0x4E00]),
            fixture_font("DejaVu Sans", 400, false, &[0xF0010]),
            fixture_font("Noto Sans", 400, false, &[0xF0010]),
            fixture_font("Bravo Sans", 400, false, &[0xF0020]),
        ]);
        // Han: PingFang SC is ranked above Hiragino Sans; Aardvark is unranked.
        assert_eq!(
            system_picks(&lib, &han.to_string(), Weight::Regular),
            ["PingFangSC-400"]
        );
        // Generic list: Noto Sans before DejaVu Sans, both before unranked families.
        assert_eq!(
            system_picks(&lib, &generic.to_string(), Weight::Regular),
            ["NotoSans-400"]
        );
        // Unranked: by family name, not database order.
        assert_eq!(
            system_picks(&lib, &unranked.to_string(), Weight::Regular),
            ["BravoSans-600".replace("600", "400")]
        );
    }

    #[test]
    fn system_fallback_matches_weight_then_upright_style() {
        let c = '\u{4E01}';
        assert_no_bundled_font_has(&c.to_string());
        let lib = FontLibrary::with_test_system_fonts(vec![
            fixture_font("PingFang SC", 700, false, &[0x4E01]),
            fixture_font("PingFang SC", 600, true, &[0x4E01]),
            fixture_font("PingFang SC", 300, false, &[0x4E01]),
            fixture_font("PingFang SC", 600, false, &[0x4E01]),
            fixture_font("PingFang SC", 400, true, &[0x4E01]),
            fixture_font("PingFang SC", 400, false, &[0x4E01]),
        ]);
        let text = c.to_string();
        assert_eq!(
            system_picks(&lib, &text, Weight::Regular),
            ["PingFangSC-400"]
        );
        assert_eq!(
            system_picks(&lib, &text, Weight::SemiBold),
            ["PingFangSC-600"]
        );
        // Unranked families follow the same rule.
        let lib = FontLibrary::with_test_system_fonts(vec![
            fixture_font("Other", 400, true, &[0xF0030]),
            fixture_font("Other", 800, false, &[0xF0030]),
            fixture_font("Other", 500, false, &[0xF0030]),
        ]);
        assert_eq!(
            system_picks(&lib, "\u{F0030}", Weight::SemiBold),
            ["Other-500"]
        );
        // Weight distance comes before style: 400 italic beats 500 upright for Regular.
        assert_eq!(
            system_picks(&lib, "\u{F0030}", Weight::Regular),
            ["Other-400Italic"]
        );
    }

    #[test]
    fn system_lookups_per_layout_are_capped() {
        let codepoints: Vec<u32> = (0xF0100..0xF0100 + 1_100).collect();
        let text: String = codepoints
            .iter()
            .filter_map(|&cp| char::from_u32(cp))
            .collect();
        assert_no_bundled_font_has(&text);
        let lib = FontLibrary::with_test_system_fonts(vec![fixture_font(
            "Cover",
            400,
            false,
            &codepoints,
        )]);
        let layout = lib.layout(&text, &style(Font::Inter, 10.0));
        assert_eq!(lookups(&lib), system::LOOKUPS_PER_LAYOUT);
        // The first 1,024 characters use the system font, the rest the primary font's .notdef.
        let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
        assert!(
            matches!(
                fonts.as_slice(),
                [
                    FontKey::System(_),
                    FontKey::Bundled(Font::Inter, Weight::Regular)
                ]
            ),
            "{fonts:?}"
        );
        assert_eq!(layout.runs[0].glyphs.len(), 1_024);
        assert_eq!(layout.runs[1].glyphs.len(), 76);
        // The next call looks up only the 76 characters that were not cached.
        let again = lib.layout(&text, &style(Font::Inter, 10.0));
        assert_eq!(lookups(&lib), 1_100);
        assert_eq!(again.runs.len(), 1);
    }

    #[test]
    fn system_lookups_are_cached_across_layouts() {
        let hit: String = (0xF0200..0xF0200 + 500)
            .filter_map(char::from_u32)
            .collect();
        let miss: String = (0xF0400..0xF0400 + 300)
            .filter_map(char::from_u32)
            .collect();
        let text = format!("{hit}{miss}");
        assert_no_bundled_font_has(&text);
        let covered: Vec<u32> = (0xF0200..0xF0200 + 500).collect();
        let lib =
            FontLibrary::with_test_system_fonts(vec![fixture_font("Cover", 400, false, &covered)]);
        let s = style(Font::Inter, 10.0);
        let layout = lib.layout(&text, &s);
        assert_eq!(lookups(&lib), 800);
        let fonts: Vec<(FontKey, usize)> = layout
            .runs
            .iter()
            .map(|r| (r.font, r.glyphs.len()))
            .collect();
        assert!(
            matches!(
                fonts.as_slice(),
                [(FontKey::System(_), 500), (FontKey::Bundled(..), 300)]
            ),
            "{fonts:?}"
        );
        let first = layout.outline(Point { x: 0.0, y: 0.0 });
        // Found and not-found results are both cached.
        let second = lib.layout(&text, &s).outline(Point { x: 0.0, y: 0.0 });
        assert_eq!(lookups(&lib), 800);
        assert_eq!(first, second);
        // The weight is part of the key.
        let _ = lib.layout(
            &hit,
            &TextStyle {
                weight: Weight::SemiBold,
                ..s
            },
        );
        assert_eq!(lookups(&lib), 1_300);
    }

    #[test]
    fn system_lookup_cache_is_bounded() {
        let lib = FontLibrary::with_test_system_fonts(vec![]);
        let system = lib.system.as_ref().unwrap();
        let s = style(Font::Inter, 10.0);
        let total = system::CACHE_LIMIT + 4_000;
        let chars: Vec<char> = (0xF0000..).filter_map(char::from_u32).take(total).collect();
        for chunk in chars.chunks(system::LOOKUPS_PER_LAYOUT) {
            let text: String = chunk.iter().collect();
            let _ = lib.layout(&text, &s);
        }
        assert_eq!(system.lookups(), total);
        assert_eq!(system.cache_len(), system::CACHE_LIMIT);
        // Characters that did not fit are looked up again; cached ones are not.
        let late: String = chars[total - 10..].iter().collect();
        let early: String = chars[..10].iter().collect();
        let _ = lib.layout(&format!("{early}{late}"), &s);
        assert_eq!(system.lookups(), total + 10);
        assert_eq!(system.cache_len(), system::CACHE_LIMIT);
    }

    #[test]
    fn system_shaping_tables_are_kept_within_bounds() {
        let n = u32::try_from(system::SHAPING_CACHE_FACES).unwrap() + 1;
        let codepoints: Vec<u32> = (0..n).map(|k| 0xF9000 + k).collect();
        let text: String = codepoints
            .iter()
            .filter_map(|&cp| char::from_u32(cp))
            .collect();
        assert_no_bundled_font_has(&text);
        let lib = FontLibrary::with_test_system_fonts(
            codepoints
                .iter()
                .enumerate()
                .map(|(k, &cp)| fixture_font(&format!("Face {k:03}"), 400, false, &[cp]))
                .collect(),
        );
        let system = lib.system.as_ref().unwrap();
        let s = style(Font::Inter, 10.0);
        let first = lib.layout(&text, &s);
        // Every character is drawn with its own system face.
        assert_eq!(first.runs.len(), codepoints.len());
        assert!(
            first
                .runs
                .iter()
                .all(|run| matches!(run.font, FontKey::System(_)))
        );
        let (faces, bytes) = system.shaping_cache();
        assert_eq!(faces, system::SHAPING_CACHE_FACES);
        assert!(bytes > 0 && bytes <= system::SHAPING_CACHE_BYTES, "{bytes}");
        // Kept faces shape the same as fresh ones, and nothing more is kept.
        let again = lib.layout(&text, &s);
        assert_eq!(first.width(), again.width());
        assert_eq!(
            first.outline(Point { x: 0.0, y: 0.0 }),
            again.outline(Point { x: 0.0, y: 0.0 })
        );
        assert_eq!(system.shaping_cache(), (faces, bytes));
    }

    #[test]
    fn kept_shaping_fonts_charge_later_table_copies() {
        let text = "\u{F9100}";
        assert_no_bundled_font_has(text);
        let font = fixture_font("Lazy Tables", 400, false, &[0xF9100]);
        let font = with_table(&font, *b"kern", vec![0; 4_000]);
        let s = style(Font::Inter, 10.0);
        let probe = FontLibrary::with_test_system_fonts(vec![font.clone()]);
        let _ = probe.layout(text, &s);
        let (_, all) = probe.system.as_ref().unwrap().shaping_cache();
        assert!(all > 4_000, "{all}");

        for budget in [all, all - 1, 1_000] {
            let limits = Limits {
                shaping_cache_bytes: budget,
                ..Limits::DEFAULT
            };
            let lib = FontLibrary::with_test_system_fonts_and_limits(vec![font.clone()], limits);
            let system = lib.system.as_ref().unwrap();
            assert_eq!(system.index().faces.len(), 1);
            // Kept before it has copied anything, so every table is copied afterwards.
            let fresh = system.shaping_font(0).unwrap();
            system.keep_shaping_font(0, fresh);
            assert_eq!(system.shaping_cache(), (1, 0));
            let layout = lib.layout(text, &s);
            assert_eq!(layout.runs[0].font, FontKey::System(0));
            assert!(layout.width() > 0.0);
            let (faces, bytes) = system.shaping_cache();
            assert_eq!(faces, 1);
            assert!(bytes <= budget, "{bytes} > {budget}");
            let kern = system.shaping_font(0).unwrap().font.tables().kern().is_ok();
            if budget == all {
                assert_eq!((bytes, kern), (all, true));
            } else {
                assert!(bytes > 0 && bytes < budget.min(all), "{bytes}");
            }
            if budget < 4_000 {
                assert!(!kern, "`kern` does not fit in {budget} bytes");
            }
        }
    }

    // --- Fallback font budgets. ---

    /// A format 12 subtable with the given `(first, last, glyph)` groups.
    fn format12(groups: &[(u32, u32, u32)]) -> Vec<u8> {
        let mut sub = Vec::new();
        sub.extend(12u16.to_be_bytes());
        sub.extend(0u16.to_be_bytes());
        sub.extend((16 + 12 * groups.len() as u32).to_be_bytes());
        sub.extend(0u32.to_be_bytes());
        sub.extend((groups.len() as u32).to_be_bytes());
        for &(first, last, glyph) in groups {
            for field in [first, last, glyph] {
                sub.extend(field.to_be_bytes());
            }
        }
        sub
    }

    /// A `cmap` table with the given `(platform, encoding, subtable)` records; each subtable
    /// is stored once, however many records point at it.
    fn cmap_of(subtables: &[Vec<u8>], records: &[(u16, u16, usize)]) -> Vec<u8> {
        let mut offsets = Vec::new();
        let mut offset = 4 + 8 * records.len();
        for sub in subtables {
            offsets.push(offset as u32);
            offset += sub.len();
        }
        let mut cmap = Vec::new();
        cmap.extend(0u16.to_be_bytes());
        cmap.extend((records.len() as u16).to_be_bytes());
        for &(platform, encoding, sub) in records {
            cmap.extend(platform.to_be_bytes());
            cmap.extend(encoding.to_be_bytes());
            cmap.extend(offsets[sub].to_be_bytes());
        }
        for sub in subtables {
            cmap.extend(sub);
        }
        cmap
    }

    /// 50,000 characters over 4,096 distinct code points from U+F0000.
    fn many_secondary_characters() -> String {
        (0..50_000u32)
            .filter_map(|i| char::from_u32(0xF0000 + i % 4_096))
            .collect()
    }

    /// The groups of a subtable that maps the 4,096 code points from U+F0000 to glyph 1.
    fn secondary_hits() -> Vec<(u32, u32, u32)> {
        (0..4_096).map(|k| (0xF0000 + k, 0xF0000 + k, 1)).collect()
    }

    fn assert_fast(elapsed: std::time::Duration) {
        let limit = if cfg!(debug_assertions) {
            std::time::Duration::from_secs(2)
        } else {
            std::time::Duration::from_millis(100)
        };
        println!("layout: {elapsed:?}");
        assert!(elapsed < limit, "{elapsed:?}");
    }

    #[test]
    fn secondary_subtables_are_read_once_each() {
        // 8,190 records share one large subtable that maps nothing useful; the record after
        // them maps the text; the best subtable (Windows, full repertoire) maps none of it.
        let miss: Vec<(u32, u32, u32)> = (0..16_384)
            .map(|k| (0x4000_0000 + 2 * k, 0x4000_0000 + 2 * k, 1))
            .collect();
        let subtables = [
            format12(&miss),
            format12(&secondary_hits()),
            format12(&[(0x8000_0000, 0x8000_0000, 1)]),
        ];
        let mut records = vec![(3, 1, 0); 8_190];
        records.extend([(3, 1, 1), (3, 10, 2)]);
        let font = font_with_cmap("Many Records", 400, false, cmap_of(&subtables, &records));
        let face = Face::parse(&font, 0).unwrap();
        assert_eq!(face.glyph('\u{F0000}'), Some(GlyphId::new(1)));
        assert_eq!(face.secondary.len(), 2);

        let text = many_secondary_characters();
        assert_no_bundled_font_has(&text[..4 * 4_096]);
        let lib = FontLibrary::with_fallbacks(vec![font]);
        let start = std::time::Instant::now();
        let layout = lib.layout(&text, &style(Font::Inter, 10.0));
        let elapsed = start.elapsed();
        let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
        assert_eq!(fonts, [FontKey::Extra(0)]);
        assert_eq!(layout.runs[0].glyphs.len(), 50_000);
        assert!(layout.runs[0].glyphs.iter().all(|g| g.id == 1));
        assert_fast(elapsed);
    }

    #[test]
    fn secondary_subtables_are_capped() {
        // 8,190 distinct subtables that map nothing useful come before the one that maps the
        // text: it is past the cap, so the font does not have the characters.
        let mut subtables: Vec<Vec<u8>> = (0..8_190)
            .map(|k| format12(&[(0x4000_0000 + k, 0x4000_0000 + k, 1)]))
            .collect();
        subtables.push(format12(&secondary_hits()));
        subtables.push(format12(&[(0x8000_0000, 0x8000_0000, 1)]));
        let mut records: Vec<(u16, u16, usize)> = (0..8_191).map(|k| (3, 1, k)).collect();
        records.push((3, 10, 8_191));
        let font = font_with_cmap("Many Subtables", 400, false, cmap_of(&subtables, &records));
        let face = Face::parse(&font, 0).unwrap();
        assert_eq!(face.secondary.len(), SECONDARY_SUBTABLES);
        assert_eq!(face.glyph('\u{F0000}'), None);

        let text = many_secondary_characters();
        let lib = FontLibrary::with_fallbacks(vec![font]);
        let start = std::time::Instant::now();
        let layout = lib.layout(&text, &style(Font::Inter, 10.0));
        let elapsed = start.elapsed();
        let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
        assert_eq!(fonts, [FontKey::Bundled(Font::Inter, Weight::Regular)]);
        assert_fast(elapsed);
    }

    fn extra_confirmations(lib: &FontLibrary) -> usize {
        lib.extra_confirmations.load(Ordering::Relaxed)
    }

    #[test]
    fn fallback_confirmations_per_layout_are_capped() {
        let codepoints: Vec<u32> = (0xF4000..0xF4000 + 3_000).collect();
        let text: String = codepoints
            .iter()
            .filter_map(|&cp| char::from_u32(cp))
            .collect();
        assert_no_bundled_font_has(&text);
        // Two fallback fonts that claim every character but have no outlines.
        let lib = FontLibrary::with_fallbacks(
            (0..2)
                .map(|k| {
                    without_outlines(fixture_font(&format!("Claim {k}"), 400, false, &codepoints))
                })
                .collect(),
        );
        let s = style(Font::Inter, 10.0);
        let layout = lib.layout(&text, &s);
        // 3,000 characters would need 6,000 confirmations.
        assert_eq!(extra_confirmations(&lib), EXTRA_CONFIRMATIONS_PER_LAYOUT);
        let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
        assert_eq!(fonts, [FontKey::Bundled(Font::Inter, Weight::Regular)]);
        // Results are cached across layouts; the next call confirms only the rest.
        let _ = lib.layout(&text, &s);
        assert_eq!(extra_confirmations(&lib), 6_000);
        let _ = lib.layout(&text, &s);
        assert_eq!(extra_confirmations(&lib), 6_000);
        assert_eq!(lock(&lib.extra_cache).len(), 6_000);
    }

    #[test]
    fn fallback_confirmation_cache_is_bounded() {
        let total = EXTRA_CACHE_LIMIT + 4_000;
        let chars: Vec<char> = (0xF0000..).filter_map(char::from_u32).take(total).collect();
        let codepoints: Vec<u32> = chars.iter().map(|&c| u32::from(c)).collect();
        let lib = FontLibrary::with_fallbacks(vec![without_outlines(fixture_font(
            "Claim",
            400,
            false,
            &codepoints,
        ))]);
        let s = style(Font::Inter, 10.0);
        for chunk in chars.chunks(EXTRA_CONFIRMATIONS_PER_LAYOUT) {
            let text: String = chunk.iter().collect();
            let _ = lib.layout(&text, &s);
        }
        assert_eq!(extra_confirmations(&lib), total);
        assert_eq!(lock(&lib.extra_cache).len(), EXTRA_CACHE_LIMIT);
        // Characters that did not fit are confirmed again; cached ones are not.
        let late: String = chars[total - 10..].iter().collect();
        let early: String = chars[..10].iter().collect();
        let _ = lib.layout(&format!("{early}{late}"), &s);
        assert_eq!(extra_confirmations(&lib), total + 10);
        assert_eq!(lock(&lib.extra_cache).len(), EXTRA_CACHE_LIMIT);
    }

    #[test]
    fn bundled_libraries_share_one_system_index() {
        let a = FontLibrary::bundled();
        let b = FontLibrary::bundled();
        let index = |lib: &FontLibrary| Arc::clone(lib.system.as_ref().unwrap().shared_index());
        assert!(Arc::ptr_eq(&index(&a), &index(&b)));
        assert!(Arc::ptr_eq(&index(&a), &index(FontLibrary::global())));
        // Libraries with in-memory system fonts keep their own.
        let c = FontLibrary::with_test_system_fonts(vec![]);
        let d = FontLibrary::with_test_system_fonts(vec![]);
        assert!(!Arc::ptr_eq(&index(&c), &index(&d)));
        assert!(!Arc::ptr_eq(&index(&a), &index(&c)));
    }

    #[test]
    fn system_fonts_position_marks_without_gpos() {
        // A base and a combining mark, without GDEF, GSUB or GPOS: the shaper positions the
        // mark from the glyph bounds.
        let mut font = fixture_font("Mark Font", 400, false, &[0xF0010, 0x0301]);
        for i in 0..be16(&font, 4) {
            let at = 12 + 16 * i;
            if matches!(&font[at..at + 4], b"GDEF" | b"GPOS" | b"GSUB") {
                font[at + 12..at + 16].copy_from_slice(&0u32.to_be_bytes());
            }
        }
        let text = "\u{F0010}\u{301}";
        let s = style(Font::Inter, 10.0);
        let extra = FontLibrary::with_fallbacks(vec![font.clone()]).layout(text, &s);
        let system = FontLibrary::with_test_system_fonts(vec![font]).layout(text, &s);
        assert_eq!(extra.runs.len(), 1);
        assert_eq!(system.runs.len(), 1);
        assert_eq!(extra.runs[0].font, FontKey::Extra(0));
        assert!(matches!(system.runs[0].font, FontKey::System(_)));
        let mark_y = |layout: &TextLayout| layout.runs[0].glyphs[1].y;
        assert!(
            (mark_y(&extra) + 7.998_047).abs() < 1e-4,
            "{}",
            mark_y(&extra)
        );
        assert!(
            (mark_y(&system) + 7.998_047).abs() < 1e-4,
            "{}",
            mark_y(&system)
        );
        assert_eq!(extra.width(), system.width());
        assert_eq!(
            extra.outline(Point { x: 0.0, y: 0.0 }),
            system.outline(Point { x: 0.0, y: 0.0 })
        );
    }

    #[test]
    fn format_2_deltas_wrap_around() {
        // A format 2 subtable whose sub-header for 0xE0xx maps U+E000 to `raw` plus `delta`.
        let cmap = |raw: u16, delta: i16| {
            let mut sub = Vec::new();
            for word in [2u16, 536, 0] {
                sub.extend(word.to_be_bytes());
            }
            for high in 0..256 {
                sub.extend((if high == 0xE0 { 8u16 } else { 0 }).to_be_bytes());
            }
            for word in [0u16, 0, 0, 0, 0, 1] {
                sub.extend(word.to_be_bytes());
            }
            sub.extend(delta.to_be_bytes());
            for word in [2u16, raw] {
                sub.extend(word.to_be_bytes());
            }
            let mut cmap = Vec::new();
            for word in [0u16, 1, 3, 1] {
                cmap.extend(word.to_be_bytes());
            }
            cmap.extend(12u32.to_be_bytes());
            cmap.extend(sub);
            cmap
        };
        for (raw, delta, expected) in [
            (1u16, 0i16, Some(1u32)),
            (2, -1, Some(1)),
            (65_535, 2, Some(1)),
            (1, -3, Some(65_534)),
            (3, -3, None),
        ] {
            let font = font_with_cmap("Cmap Two", 400, false, cmap(raw, delta));
            let face = Face::parse(&font, 0).unwrap();
            assert_eq!(
                face.glyph('\u{E000}').map(GlyphId::to_u32),
                expected,
                "{raw} {delta}"
            );
        }
    }

    /// Lays `text` out with `font` as a fallback font and as a system font, and returns the
    /// glyph ids of each layout's single run.
    fn glyphs_as_extra_and_system(font: &[u8], text: &str) -> [Vec<u16>; 2] {
        let s = style(Font::Inter, 10.0);
        let extra = FontLibrary::with_fallbacks(vec![font.to_vec()]).layout(text, &s);
        let system = FontLibrary::with_test_system_fonts(vec![font.to_vec()]).layout(text, &s);
        assert_eq!(extra.runs.len(), 1);
        assert_eq!(system.runs.len(), 1);
        assert_eq!(extra.runs[0].font, FontKey::Extra(0));
        assert!(matches!(system.runs[0].font, FontKey::System(_)));
        [extra, system].map(|layout| layout.runs[0].glyphs.iter().map(|g| g.id).collect())
    }

    #[test]
    fn shaper_uses_skrifas_best_subtable_over_a_format_10_choice() {
        // Unicode full repertoire, format 12: U+F0010. Windows full repertoire, format 10:
        // only U+10000. skrifa prefers the first, HarfRust the second.
        let mut format10 = Vec::new();
        format10.extend(10u16.to_be_bytes());
        format10.extend(0u16.to_be_bytes());
        for field in [22u32, 0, 0x10000, 1] {
            format10.extend(field.to_be_bytes());
        }
        format10.extend(1u16.to_be_bytes());
        let cmap = cmap_of(
            &[format12(&[(0xF0010, 0xF0010, 1)]), format10],
            &[(0, 6, 0), (3, 10, 1)],
        );
        let font = font_with_cmap("Unique Records", 400, false, cmap);
        assert_eq!(
            Face::parse(&font, 0).unwrap().glyph('\u{F0010}'),
            Some(GlyphId::new(1))
        );
        assert_eq!(
            glyphs_as_extra_and_system(&font, "\u{F0010}"),
            [vec![1], vec![1]]
        );
    }

    #[test]
    fn shaper_uses_skrifas_best_subtable_among_two_full_repertoire_records() {
        // Two full-repertoire subtables, each mapping one of the characters.
        let cmap = cmap_of(
            &[
                format12(&[(0xF0000, 0xF0000, 1)]),
                format12(&[(0xF0001, 0xF0001, 1)]),
            ],
            &[(3, 10, 0), (0, 4, 1)],
        );
        let font = font_with_cmap("Two Records", 400, false, cmap);
        assert_eq!(
            glyphs_as_extra_and_system(&font, "\u{F0000}\u{F0001}"),
            [vec![1, 1], vec![1, 1]]
        );
    }

    /// A CFF font with three glyphs: an empty `.notdef`, an empty glyph for U+0300 and a
    /// square for U+F0010 and U+0301; no GDEF, GSUB or GPOS.
    fn cff_mark_font() -> Vec<u8> {
        fn index(items: &[Vec<u8>]) -> Vec<u8> {
            let mut out = Vec::new();
            out.extend((items.len() as u16).to_be_bytes());
            if items.is_empty() {
                return out;
            }
            out.push(4);
            let mut offset = 1u32;
            out.extend(offset.to_be_bytes());
            for item in items {
                offset += item.len() as u32;
                out.extend(offset.to_be_bytes());
            }
            for item in items {
                out.extend(item);
            }
            out
        }
        fn dict_int(value: usize) -> Vec<u8> {
            let mut out = vec![29];
            out.extend((value as i32).to_be_bytes());
            out
        }
        fn number(value: i16) -> Vec<u8> {
            let mut out = vec![28];
            out.extend(value.to_be_bytes());
            out
        }
        const ENDCHAR: u8 = 14;
        let mut square = Vec::new();
        for (args, op) in [
            (&[100i16, 100][..], 21u8),
            (&[800], 6),
            (&[1200], 7),
            (&[-800], 6),
        ] {
            for &arg in args {
                square.extend(number(arg));
            }
            square.push(op);
        }
        square.push(ENDCHAR);
        let charstrings = index(&[vec![ENDCHAR], vec![ENDCHAR], square]);
        // defaultWidthX 0.
        let private = vec![139, 20];
        let header = vec![1, 0, 4, 4];
        let name = index(&[b"Test".to_vec()]);
        let top = |charstrings_at: usize, private_at: usize| {
            let mut dict = dict_int(charstrings_at);
            dict.push(17);
            dict.extend(dict_int(private.len()));
            dict.extend(dict_int(private_at));
            dict.push(18);
            index(&[dict])
        };
        let empty = index(&[]);
        let charstrings_at = header.len() + name.len() + top(0, 0).len() + 2 * empty.len();
        let private_at = charstrings_at + charstrings.len();
        let mut cff = header;
        cff.extend(name);
        cff.extend(top(charstrings_at, private_at));
        cff.extend(&empty);
        cff.extend(&empty);
        cff.extend(charstrings);
        cff.extend(private);

        let base: &[u8] = include_bytes!("../tests/fixtures/fallback-only.ttf");
        let mut tables = tables_of(base);
        tables.retain(|(tag, _)| !matches!(tag, b"glyf" | b"loca" | b"GDEF" | b"GSUB" | b"GPOS"));
        for (tag, data) in &mut tables {
            match &*tag {
                b"maxp" => {
                    *data = [
                        0x0000_5000u32.to_be_bytes().to_vec(),
                        3u16.to_be_bytes().to_vec(),
                    ]
                    .concat()
                }
                b"hhea" => data[34..36].copy_from_slice(&3u16.to_be_bytes()),
                b"hmtx" => {
                    *data = [(1_344u16, 0i16), (0, 0), (1_000, 100)]
                        .iter()
                        .flat_map(|(advance, bearing)| {
                            [advance.to_be_bytes(), bearing.to_be_bytes()]
                        })
                        .flatten()
                        .collect();
                }
                b"cmap" => {
                    *data = cmap_of(
                        &[format12(&[
                            (0x300, 0x300, 1),
                            (0x301, 0x301, 2),
                            (0xF0010, 0xF0010, 2),
                        ])],
                        &[(3, 10, 0)],
                    );
                }
                _ => {}
            }
        }
        tables.push((*b"CFF ", cff));
        let mut font = assemble(tables);
        font[0..4].copy_from_slice(b"OTTO");
        font
    }

    #[test]
    fn empty_cff_glyphs_have_empty_extents() {
        let font = cff_mark_font();
        let face = Face::parse(&font, 0).unwrap();
        assert_eq!(face.outlines.format(), Some(OutlineGlyphFormat::Cff));
        let tuple =
            |e: Option<GlyphExtents>| e.map(|e| (e.x_bearing, e.y_bearing, e.width, e.height));
        // Draws, but with no path commands: empty extents, as HarfRust gives.
        assert_eq!(tuple(face.extents(GlyphId::new(1))), Some((0, 0, 0, 0)));
        assert_eq!(
            tuple(face.extents(GlyphId::new(2))),
            Some((100, 1300, 800, -1200))
        );
        // Missing: none.
        assert_eq!(tuple(face.extents(GlyphId::new(7))), None);
        let shaping = ShapingFont::new(font.clone(), 0).unwrap();
        let shaper = ShaperFont::new(&shaping);
        for glyph in [1, 2] {
            assert_eq!(
                tuple(shaper.default_glyph_extents(GlyphId::new(glyph))),
                tuple(face.extents(GlyphId::new(glyph))),
                "glyph {glyph}"
            );
        }

        // A base, an empty mark and a visible mark, stacked by the shaper from the bounds.
        let text = "\u{F0010}\u{300}\u{301}";
        let s = style(Font::Inter, 10.0);
        let extra = FontLibrary::with_fallbacks(vec![font.clone()]).layout(text, &s);
        let system = FontLibrary::with_test_system_fonts(vec![font]).layout(text, &s);
        assert_eq!(extra.runs[0].font, FontKey::Extra(0));
        assert!(matches!(system.runs[0].font, FontKey::System(_)));
        let visible_mark_y = |layout: &TextLayout| layout.runs[0].glyphs[2].y;
        assert_eq!(visible_mark_y(&extra), visible_mark_y(&system));
        assert!((visible_mark_y(&system) + 7.109_375).abs() < 1e-4);
        assert!(visible_mark_y(&extra) < -6.0, "{}", visible_mark_y(&extra));
        assert_eq!(
            extra.outline(Point { x: 0.0, y: 0.0 }),
            system.outline(Point { x: 0.0, y: 0.0 })
        );
    }

    #[test]
    fn system_extents_per_layout_are_capped() {
        let bytes = include_bytes!("../tests/fixtures/fallback-only.ttf").to_vec();
        let face = Face::parse(&bytes, 0).unwrap();
        let shaping = ShapingFont::new(bytes.clone(), 0).unwrap();
        let shaper = ShaperFont::new(&shaping);
        assert!(face.extents(GlyphId::new(0)).is_some());
        assert!(face.extents(GlyphId::new(1)).is_some());
        let budget = Cell::new(EXTENTS_PER_LAYOUT);
        let funcs = Funcs::system(&face, &budget);
        // 4,095 computations for glyphs the font does not have, then glyph 1: 4,096.
        for glyph in 2..u32::try_from(EXTENTS_PER_LAYOUT).unwrap() + 1 {
            assert!(funcs.glyph_extents(&shaper, GlyphId::new(glyph)).is_none());
        }
        assert!(funcs.glyph_extents(&shaper, GlyphId::new(1)).is_some());
        assert_eq!(budget.get(), 0);
        // The 4,097th computation is refused, although glyph 0 has extents.
        assert!(funcs.glyph_extents(&shaper, GlyphId::new(0)).is_none());
        // Glyphs already computed are still answered from the cache.
        assert!(funcs.glyph_extents(&shaper, GlyphId::new(1)).is_some());
        assert!(funcs.glyph_extents(&shaper, GlyphId::new(2)).is_none());
    }

    // --- Mark attachment. ---

    #[test]
    fn mark_table_is_sorted_and_disjoint() {
        let table = marks::MARKS;
        assert!(table.windows(2).all(|w| w[0].1 + 1 < w[1].0));
        assert!(table.iter().all(|&(lo, hi)| lo <= hi));
        for c in [
            '\u{0300}', '\u{0591}', '\u{0F18}', '\u{A67C}', '\u{20DD}', '\u{FE0F}',
        ] {
            assert!(is_mark(c), "{c:?}");
        }
        for c in ['a', ' ', '\u{05D0}', '\u{E000}', '\u{200D}'] {
            assert!(!is_mark(c), "{c:?}");
        }
    }

    #[test]
    fn marks_outside_the_latin_blocks_stay_with_their_base() {
        let fallback = crate::fonts::FALLBACK_TEST_CODEPOINT;
        let lib = omega_lib();
        for mark in ['\u{0591}', '\u{A67C}', '\u{0F18}'] {
            let layout = lib.layout(&format!("{fallback}{mark}"), &style(Font::Inter, 10.0));
            let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
            assert_eq!(fonts, [FontKey::Extra(0)], "U+{:04X}", u32::from(mark));
        }
    }

    // --- Fallback fonts that cannot draw a glyph. ---

    #[test]
    fn fallback_without_outlines_is_skipped() {
        let fixture = include_bytes!("../tests/fixtures/fallback-only.ttf");
        let truncated = fixture[..516].to_vec();
        let text = crate::fonts::FALLBACK_TEST_CODEPOINT.to_string();
        let s = style(Font::Inter, 10.0);
        let lib = FontLibrary::with_fallbacks(vec![truncated]);
        assert_eq!(lib.extra.len(), 1, "the truncated font still parses");
        let layout = lib.layout(&text, &s);
        let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
        assert_eq!(fonts, [FontKey::Bundled(Font::Inter, Weight::Regular)]);
        let path = layout.outline(Point { x: 0.0, y: 0.0 });
        let notdef = FontLibrary::with_fallbacks(vec![])
            .layout(&text, &s)
            .outline(Point { x: 0.0, y: 0.0 });
        assert!(!path.is_empty());
        assert_eq!(path.verbs().len(), notdef.verbs().len());
    }

    // --- Self-contained layouts. ---

    #[test]
    fn layout_outlines_after_its_library_is_dropped() {
        let text = format!("A{}", crate::fonts::FALLBACK_TEST_CODEPOINT);
        let lib = omega_lib();
        let layout = lib.layout(&text, &style(Font::Inter, 10.0));
        assert_eq!(layout.runs.len(), 2);
        let before = layout.outline(Point { x: 3.0, y: 7.0 });
        drop(lib);
        let after = layout.outline(Point { x: 3.0, y: 7.0 });
        assert_eq!(before, after);
        let a_only = FontLibrary::with_fallbacks(vec![]).layout("A", &style(Font::Inter, 10.0));
        assert!(after.bounds().expect("outline").1.x > 3.0 + a_only.width());
        // Clones share the font handles.
        assert_eq!(layout.clone().outline(Point { x: 3.0, y: 7.0 }), before);
    }

    // --- Fallback precedence. ---

    #[test]
    fn bundled_families_take_precedence_over_explicit_fallbacks() {
        let c = '\u{0114}';
        let geist = FontLibrary::with_fallbacks(vec![]);
        let geist_face = geist.bundled_face(Font::Geist, Weight::Regular).unwrap();
        let serif_face = geist
            .bundled_face(Font::SourceSerif4, Weight::Regular)
            .unwrap();
        assert!(geist_face.face.glyph(c).is_none() && serif_face.face.glyph(c).is_some());
        let serif = font_bytes(Font::SourceSerif4, Weight::Regular).to_vec();
        let lib = FontLibrary::with_fallbacks(vec![serif]);
        let layout = lib.layout(&c.to_string(), &style(Font::Geist, 10.0));
        let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
        assert_eq!(
            fonts,
            [FontKey::Bundled(Font::SourceSerif4, Weight::Regular)]
        );
    }

    // --- Malformed fonts that can make the parser or shaper panic. ---

    /// Inter Regular with the seven bytes changed that a fuzzing seed found: in debug builds
    /// the previous shaper, rustybuzz, panicked on it with an arithmetic overflow.
    fn bit_flipped_inter() -> Vec<u8> {
        const CHANGES: [(usize, u8); 7] = [
            (959, 11),
            (4201, 148),
            (11280, 109),
            (14203, 16),
            (24820, 97),
            (25908, 26),
            (28908, 225),
        ];
        let mut bytes = font_bytes(Font::Inter, Weight::Regular).to_vec();
        for (offset, value) in CHANGES {
            bytes[offset] = value;
        }
        bytes
    }

    #[test]
    fn malformed_fonts_that_panic_while_parsing_are_skipped() {
        let bytes = bit_flipped_inter();
        // This seed made rustybuzz panic with an arithmetic overflow in debug builds. HarfRust
        // and skrifa parse it without panicking, so every stage the library guards is run here
        // under the same guard: whatever they do with these bytes, nothing may escape.
        let _ = guarded(|| {
            let face = Face::parse(&bytes, 0)?;
            let font = ShapingFont::new(bytes.clone(), 0)?;
            let funcs = Funcs::new(&face);
            let features = features(true);
            let shaped = Shaper::new(&font, &funcs, &features).shape("Ab 0123 fi AV", false);
            Some((face.draws('A'), shaped.map(|run| run.glyphs.len())))
        });
        // A panic inside a guarded call is turned into "unreadable".
        assert_eq!(guarded(|| -> bool { panic!("a malformed font") }), None);
        let s = style(Font::Geist, 10.0);
        let fallback = crate::fonts::FALLBACK_TEST_CODEPOINT;
        let text = format!("Ab{fallback}\u{4E00}");

        let lib = FontLibrary::with_fallbacks(vec![bytes.clone()]);
        let layout = lib.layout(&text, &s);
        assert!(layout.width() > 0.0);
        assert!(!layout.outline(Point { x: 0.0, y: 0.0 }).is_empty());

        let lib = FontLibrary::with_test_system_fonts(vec![bytes]);
        let layout = lib.layout(&text, &s);
        assert!(layout.width() > 0.0);
        assert!(!layout.outline(Point { x: 0.0, y: 0.0 }).is_empty());
    }

    #[test]
    fn building_a_library_while_unwinding_does_not_abort() {
        struct BuildOnDrop;
        impl Drop for BuildOnDrop {
            fn drop(&mut self) {
                let fixture = include_bytes!("../tests/fixtures/fallback-only.ttf").to_vec();
                let lib = FontLibrary::with_fallbacks(vec![fixture]);
                assert_eq!(lib.extra.len(), 1);
            }
        }
        let caught = std::panic::catch_unwind(|| {
            let _guard = BuildOnDrop;
            panic!("host panic, caught outside the Drop");
        });
        assert!(caught.is_err());
    }

    // --- Index limits. ---

    fn indexed_names(lib: &FontLibrary) -> Vec<String> {
        let index = lib.system.as_ref().unwrap().index();
        let mut names: Vec<String> = index
            .faces
            .iter()
            .map(|f| f.post_script_name.clone())
            .collect();
        names.sort();
        names
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
        let limits = Limits {
            max_bytes: small.len(),
            ..Limits::DEFAULT
        };
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
        let limits = Limits {
            max_faces: 2,
            ..Limits::DEFAULT
        };
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

    /// Empties the `glyf` and `loca` tables, so the font still claims its code points but
    /// has no outlines.
    fn without_outlines(mut bytes: Vec<u8>) -> Vec<u8> {
        for i in 0..be16(&bytes, 4) {
            let at = 12 + 16 * i;
            if &bytes[at..at + 4] == b"glyf" || &bytes[at..at + 4] == b"loca" {
                bytes[at + 12..at + 16].copy_from_slice(&0u32.to_be_bytes());
            }
        }
        bytes
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
        let read = system::cmap_ranges(&cmap, Limits::DEFAULT.max_face_visits).unwrap();
        assert_eq!(read.ranges, [(0xE000, 0xE000)]);
        assert_eq!(read.visited, 256);

        let font = font_with_cmap("Format Two", 400, false, cmap);
        let face = Face::parse(&font, 0).unwrap();
        assert!(face.draws('\u{E000}'));
        let s = style(Font::Inter, 10.0);
        let extra = FontLibrary::with_fallbacks(vec![font.clone()]).layout("\u{E000}", &s);
        assert_eq!(extra.runs[0].font, FontKey::Extra(0));
        let lib = FontLibrary::with_test_system_fonts(vec![font]);
        assert_eq!(
            system_picks(&lib, "\u{E000}", Weight::Regular),
            ["FormatTwo-400"]
        );
    }

    /// 32 system fonts that claim `codepoints` but have no outlines.
    fn outline_free_candidates(codepoints: &[u32]) -> FontLibrary {
        FontLibrary::with_test_system_fonts(
            (0..32)
                .map(|k| {
                    without_outlines(fixture_font(
                        &format!("Claim {k:02}"),
                        400,
                        false,
                        codepoints,
                    ))
                })
                .collect(),
        )
    }

    #[test]
    fn confirmations_per_lookup_are_capped() {
        let codepoints: Vec<u32> = (0xF3000..0xF3000 + 64).collect();
        let text: String = codepoints
            .iter()
            .filter_map(|&cp| char::from_u32(cp))
            .collect();
        assert_no_bundled_font_has(&text);
        let lib = outline_free_candidates(&codepoints);
        let system = lib.system.as_ref().unwrap();
        let layout = lib.layout(&text, &style(Font::Inter, 10.0));
        assert_eq!(system.lookups(), 64);
        assert_eq!(
            system.confirmations(),
            64 * system::CONFIRMATIONS_PER_LOOKUP
        );
        assert!(system.confirmations() <= system::CONFIRMATIONS_PER_LAYOUT);
        let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
        assert_eq!(fonts, [FontKey::Bundled(Font::Inter, Weight::Regular)]);
        // A lookup that used up its own confirmations is cached.
        let _ = lib.layout(&text, &style(Font::Inter, 10.0));
        assert_eq!(system.lookups(), 64);
    }

    #[test]
    fn confirmations_per_layout_are_capped() {
        let codepoints: Vec<u32> = (0xF4000..0xF4000 + 300).collect();
        let text: String = codepoints
            .iter()
            .filter_map(|&cp| char::from_u32(cp))
            .collect();
        assert_no_bundled_font_has(&text);
        let lib = outline_free_candidates(&codepoints);
        let system = lib.system.as_ref().unwrap();
        let layout = lib.layout(&text, &style(Font::Inter, 10.0));
        // 300 characters would need 300 * 16 = 4,800 confirmations.
        assert_eq!(system.confirmations(), system::CONFIRMATIONS_PER_LAYOUT);
        assert_eq!(system.lookups(), system::CONFIRMATIONS_PER_LAYOUT / 16);
        assert!(layout.width() > 0.0);
        // Characters cut off by the layout budget are not cached; the next call continues.
        let _ = lib.layout(&text, &style(Font::Inter, 10.0));
        assert_eq!(system.lookups(), 300);
        assert_eq!(system.confirmations(), 300 * 16);
    }

    #[test]
    fn index_keeps_a_bounded_number_of_ranges() {
        let spaced = |base: u32, n: u32| -> Vec<u32> { (0..n).map(|k| base + 2 * k).collect() };
        let limits = Limits {
            max_total_ranges: 15,
            ..Limits::DEFAULT
        };
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
        assert_eq!(lib.system.as_ref().unwrap().index().range_count(), 13);
    }

    #[test]
    fn cmap_subtables_are_read_once_and_every_visit_counts() {
        let codepoints: Vec<u32> = (0..1_000).map(|k| 0xF8000 + 2 * k).collect();
        // 64 encoding records point at one 1,000-group subtable.
        let cmap = format12_cmap(&codepoints, 64);
        let read = system::cmap_ranges(&cmap, Limits::DEFAULT.max_face_visits).unwrap();
        assert_eq!(read.visited, 1_000);
        assert_eq!(read.ranges.len(), 1_000);
        // Invalid groups yield nothing but are still counted.
        let invalid = format12_cmap(&[0xFFFF_FFFF], 64);
        let read = system::cmap_ranges(&invalid, Limits::DEFAULT.max_face_visits).unwrap();
        assert_eq!((read.ranges.len(), read.visited), (0, 1));
        assert_eq!(
            system::cmap_ranges(&cmap, 999),
            Err(system::CmapError::TooManyVisits)
        );

        let font = |name: &str| font_with_cmap(name, 400, false, format12_cmap(&codepoints, 64));
        // Per face: 1,000 visits are needed.
        let limits = Limits {
            max_face_visits: 999,
            ..Limits::DEFAULT
        };
        let lib = FontLibrary::with_test_system_fonts_and_limits(vec![font("Alpha")], limits);
        assert!(indexed_names(&lib).is_empty());
        assert_eq!(lib.system.as_ref().unwrap().index().visited, 1_000);
        let limits = Limits {
            max_face_visits: 1_000,
            ..Limits::DEFAULT
        };
        let lib = FontLibrary::with_test_system_fonts_and_limits(vec![font("Alpha")], limits);
        assert_eq!(indexed_names(&lib), ["Alpha-400"]);
        // Per build: the third face no longer fits and indexing stops.
        let limits = Limits {
            max_build_visits: 2_500,
            ..Limits::DEFAULT
        };
        let lib = FontLibrary::with_test_system_fonts_and_limits(
            vec![font("Alpha"), font("Bravo"), font("Charlie"), font("Delta")],
            limits,
        );
        assert_eq!(indexed_names(&lib), ["Alpha-400", "Bravo-400"]);
        assert_eq!(lib.system.as_ref().unwrap().index().visited, 2_501);
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
        let max = Limits::DEFAULT.max_face_visits;
        let read = system::cmap_ranges(&cmap, max).unwrap();
        assert_eq!(read.ranges, [(0, 0x10_FFFF)]);
        assert_eq!(read.visited, 1);
        // Truncated and empty tables give no ranges and no panic.
        assert_eq!(system::cmap_ranges(&cmap[..30], max).unwrap().ranges, []);
        assert_eq!(
            system::cmap_ranges(&[], max),
            Err(system::CmapError::Unreadable)
        );
    }

    #[test]
    fn grouping_runs_by_font_is_linear() {
        // 200,000 runs over 100,000 distinct fonts: a per-font rescan of all runs would take
        // 2 * 10^10 steps.
        let keys: Vec<FontKey> = (0..200_000).map(|i| FontKey::Extra(i % 100_000)).collect();
        let start = std::time::Instant::now();
        let groups = group_by_font(keys.iter().copied());
        let elapsed = start.elapsed();
        assert_eq!(groups.len(), 100_000);
        for (n, (key, members)) in groups.iter().enumerate() {
            assert_eq!(
                *key,
                FontKey::Extra(n),
                "groups keep first-appearance order"
            );
            assert_eq!(members, &[n, n + 100_000]);
        }
        // The quadratic version takes about 28 s even in release builds. Unoptimized builds
        // running the whole suite in parallel get headroom.
        let limit = std::time::Duration::from_secs(if cfg!(debug_assertions) { 10 } else { 1 });
        assert!(elapsed < limit, "{elapsed:?}");
    }

    #[test]
    fn many_runs_and_fallback_fonts_lay_out_in_linear_time() {
        const FONTS: u32 = 1_000;
        let fonts: Vec<Vec<u8>> = (0..FONTS)
            .map(|k| fixture_font(&format!("Fallback {k}"), 400, false, &[0xF1000 + k]))
            .collect();
        let lib = FontLibrary::with_fallbacks(fonts);
        let text: String = (0..100_000u32)
            .flat_map(|i| ['a', char::from_u32(0xF1000 + i % FONTS).unwrap()])
            .collect();
        assert_no_bundled_font_has(&text.replace('a', ""));
        let s = style(Font::Inter, 10.0);
        let start = std::time::Instant::now();
        let layout = lib.layout(&text, &s);
        let laid_out = start.elapsed();
        let path = layout.outline(Point { x: 0.0, y: 0.0 });
        let total = start.elapsed();
        assert_eq!(layout.runs.len(), 200_000);
        let distinct: std::collections::HashSet<FontKey> =
            layout.runs.iter().map(|r| r.font).collect();
        assert_eq!(distinct.len(), FONTS as usize + 1);
        assert!(!path.is_empty());
        // Release builds must lay out in under a second; unoptimized test builds get headroom.
        let limit = std::time::Duration::from_secs(if cfg!(debug_assertions) { 30 } else { 1 });
        assert!(laid_out < limit, "layout {laid_out:?}");
        println!(
            "200,000 runs, {} fonts: layout {laid_out:?}, layout + outline {total:?}",
            FONTS + 1
        );
    }

    /// Depends on the fonts installed on the machine, so it is not run by default:
    /// `cargo test -p umber-core system_fallback -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on the system fonts"]
    fn system_fallback_draws_cjk() {
        let rss_kib = || {
            std::process::Command::new("ps")
                .args(["-o", "rss=", "-p", &std::process::id().to_string()])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|o| o.trim().parse::<u64>().ok())
                .unwrap_or(0)
        };
        let lib = FontLibrary::bundled();
        let system = lib.system.as_ref().expect("system fallback");
        let before = rss_kib();
        let start = std::time::Instant::now();
        let index = system.index();
        println!(
            "index: {} faces, {} ranges, ~{} KiB heap, built in {:?}, RSS +{} KiB",
            index.faces.len(),
            index.range_count(),
            index.heap_bytes() / 1024,
            start.elapsed(),
            rss_kib().saturating_sub(before)
        );
        let s = style(Font::Inter, 10.0);
        let name = |key: FontKey| match key {
            FontKey::System(face) => system.face(face).map_or_else(String::new, |f| {
                format!("{} (weight {})", f.post_script_name, f.weight)
            }),
            other => format!("{other:?}"),
        };
        let layout = lib.layout("A数据可视化", &s);
        let fonts: Vec<FontKey> = layout.runs.iter().map(|run| run.font).collect();
        println!(
            "runs: {:?}, width {}",
            fonts.iter().map(|&k| name(k)).collect::<Vec<_>>(),
            layout.width()
        );
        assert!(matches!(
            fonts.as_slice(),
            [FontKey::Bundled(..), FontKey::System(_)]
        ));
        let path = layout.outline(Point { x: 0.0, y: 0.0 });
        let a_only = lib.layout("A", &s).width();
        assert!(path.bounds().expect("outline").1.x > a_only + 30.0);
        let semibold = lib.layout(
            "数",
            &TextStyle {
                weight: Weight::SemiBold,
                ..s
            },
        );
        println!(
            "SemiBold: {:?}",
            semibold
                .runs
                .iter()
                .map(|r| name(r.font))
                .collect::<Vec<_>>()
        );

        let distinct: String = (0x4E00..0x4E00 + 1024).filter_map(char::from_u32).collect();
        let lookups = system.lookups();
        let start = std::time::Instant::now();
        let cold = lib.layout(&distinct, &s);
        println!(
            "1,024 distinct CJK characters, cold: {:?} ({} lookups)",
            start.elapsed(),
            system.lookups() - lookups
        );
        let start = std::time::Instant::now();
        let warm = lib.layout(&distinct, &s);
        println!("same, warm: {:?}", start.elapsed());
        assert!(cold.width() > 0.0 && (cold.width() - warm.width()).abs() < 1e-3);
        println!(
            "RSS after layouts: +{} KiB",
            rss_kib().saturating_sub(before)
        );

        // Emoji and Arabic must not panic, whatever the system offers.
        for text in [
            "👍🏽",
            "مرحبا",
            "שלום",
            "한국어",
            "かな",
            "e\u{301}",
            "\u{FE0F}",
            "a\u{200D}b",
        ] {
            let l = lib.layout(text, &s);
            println!(
                "{text:?}: width {}, runs {:?}",
                l.width(),
                l.runs.iter().map(|r| name(r.font)).collect::<Vec<_>>()
            );
            assert!(l.width() > 0.0, "{text:?}");
            let path = l.outline(Point { x: 0.0, y: 0.0 });
            println!(
                "  outline: {} verbs, bounds {:?}",
                path.verbs().len(),
                path.bounds()
            );
        }
    }
}
