//! [`FontLibrary`]: the bundled, fallback and system fonts, and the coordination of one layout
//! call.
//!
//! A layout call splits the text into runs with a font chosen per character within the call's
//! budgets, shapes the runs of each font together, shapes runs whose font could not be read
//! with the primary font, and places the result.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use read_fonts::FontRef;
use read_fonts::model::Blob;
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};

use super::font::{Face, FontHandle, FontKey, ShapingFont};
use super::guard::{guarded, lock};
use super::layout::TextLayout;
use super::runs::{self, TextRun, group_by_font};
use super::shaping::{EXTENTS_PER_LAYOUT, Funcs, ShapedRun, Shaper, features};
use super::style::TextStyle;
use super::system::{Budget, Limits, Source, SystemFonts};
use crate::fonts::{Font, Weight, font_bytes};

/// The most `with_fallbacks` outline confirmations one layout call performs: checks that a
/// fallback font's glyph for a character has an outline. Beyond it, characters skip those
/// fonts and continue to the system fonts and the primary font's `.notdef`.
pub(super) const EXTRA_CONFIRMATIONS_PER_LAYOUT: usize = 4096;

/// The most entries in the cache of `with_fallbacks` confirmations. When it is full, new
/// results are not cached; nothing is evicted.
pub(super) const EXTRA_CACHE_LIMIT: usize = 65_536;

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
            Some(SystemFonts::new(Source::Os, Limits::DEFAULT)),
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
            FontKey::Extra(index) => self
                .extra
                .get(index)
                .map(|extra| FontHandle::extra(Arc::clone(&extra.bytes))),
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
        if size == 0.0 || text.is_empty() {
            return TextLayout::empty(ascent, descent);
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

        TextLayout::place(size, ascent, descent, shaped.into_iter().flatten())
    }

    /// Splits `text` into runs, choosing a font for each character the primary font lacks with
    /// [`FontLibrary::fallback_for`], within this layout call's budgets.
    fn split_runs(&self, text: &str, style: &TextStyle) -> Vec<TextRun> {
        let primary_key = FontKey::Bundled(style.font, style.weight);
        let primary = self.bundled_face(style.font, style.weight);
        let mut extras: Option<ParsedExtras<'_>> = None;
        let mut budget = Budget::per_layout();
        let mut extra_budget = EXTRA_CONFIRMATIONS_PER_LAYOUT;
        runs::split_runs(
            text,
            primary_key,
            |c| primary.is_some_and(|face| face.face.glyph(c).is_some()),
            |c| {
                self.fallback_for(c, style, &mut extras, &mut extra_budget, &mut budget)
                    .unwrap_or(primary_key)
            },
        )
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

/// Construction and inspection for unit tests.
#[cfg(test)]
impl FontLibrary {
    /// Like [`FontLibrary::bundled`], with in-memory fonts standing in for the system fonts.
    pub(super) fn with_test_system_fonts(fonts: Vec<Vec<u8>>) -> FontLibrary {
        FontLibrary::with_test_system_fonts_and_limits(fonts, Limits::DEFAULT)
    }

    /// Like [`FontLibrary::with_test_system_fonts`], with custom index limits.
    pub(super) fn with_test_system_fonts_and_limits(
        fonts: Vec<Vec<u8>>,
        limits: Limits,
    ) -> FontLibrary {
        FontLibrary::new(
            Vec::new(),
            Some(SystemFonts::new(Source::Memory(fonts), limits)),
        )
    }

    /// The parsed face of the bundled font `font` in `weight`, if it parsed.
    pub(super) fn bundled_parsed(&self, font: Font, weight: Weight) -> Option<&Face<'static>> {
        self.bundled_face(font, weight).map(|bundled| &bundled.face)
    }

    /// The parsed faces of every bundled font that parsed.
    pub(super) fn all_bundled_parsed(&self) -> impl Iterator<Item = &Face<'static>> {
        self.bundled.iter().flatten().map(|bundled| &bundled.face)
    }

    /// The number of fallback fonts that parsed.
    pub(super) fn extra_count(&self) -> usize {
        self.extra.len()
    }

    /// The number of cached fallback font confirmations.
    pub(super) fn extra_cache_len(&self) -> usize {
        lock(&self.extra_cache).len()
    }

    /// Outline confirmations performed on the fallback fonts so far.
    pub(super) fn extra_confirmations(&self) -> usize {
        self.extra_confirmations.load(Ordering::Relaxed)
    }

    /// The system font fallback state, `None` when system lookup is disabled.
    pub(super) fn system(&self) -> Option<&SystemFonts> {
        self.system.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
