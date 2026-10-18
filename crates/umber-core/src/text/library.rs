//! [`FontLibrary`]: the bundled, fallback and system fonts, and the coordination of one layout
//! call; [`SceneFonts`]: the fonts of one scene, which text names by [`FaceId`].
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

use super::font::{Face, FaceMetrics, FontHandle, FontKey, ShapingFont};
use super::guard::{guarded, lock};
use super::layout::TextLayout;
use super::runs::{self, TextRun, group_by_font};
use super::shaping::{EXTENTS_PER_LAYOUT, Funcs, ShapedRun, Shaper, features};
use super::style::{FaceId, TextStyle};
use super::system::{Budget, Limits, Source, SystemFonts};
use crate::error::quoted;
use crate::fonts::{BundledFont, Font, Weight, font_bytes};
use crate::{Error, Result};

/// The most `with_fallbacks` outline confirmations one layout call performs: checks that a
/// fallback font's glyph for a character has an outline. Beyond it, characters skip those
/// fonts and continue to the system fonts and the primary font's `.notdef`.
pub(super) const EXTRA_CONFIRMATIONS_PER_LAYOUT: usize = 4096;

/// The most entries in the cache of `with_fallbacks` confirmations. When it is full, new
/// results are not cached; nothing is evicted.
pub(super) const EXTRA_CACHE_LIMIT: usize = 65_536;

/// Number of bundled font files: every family in every weight.
const BUNDLED_COUNT: usize = BundledFont::ALL.len() * Weight::ALL.len();

/// Shapes and measures text with the bundled fonts and fallbacks.
///
/// Text is laid out through the [`SceneFonts`] of one scene, made by
/// [`FontLibrary::with_figure_fonts`]. `FontLibrary` is `Send + Sync`; one library can lay out
/// text from several threads at once, with deterministic results.
///
/// # System fonts and memory mapping
///
/// Libraries made by [`FontLibrary::bundled`] and [`FontLibrary::global`] fall back to
/// installed system fonts. fontdb memory-maps these font files when reading them. If a file is
/// truncated or modified in place while mapped, a read can crash the process with `SIGBUS` on
/// some platforms; the library's panic guards cannot catch this. Prefer
/// [`FontLibrary::global`], which shares lookup and shaping caches and reduces repeated reads.
/// [`FontLibrary::with_fallbacks`] performs no system font lookup.
///
/// System font files are read during discovery and coverage indexing, and later to confirm
/// candidate glyphs, copy shaping tables, obtain glyph bounds during shaping and outline glyphs.
/// The coverage index limits indexed faces and accepted file sizes, and layout calls limit
/// candidate confirmations. These limits do not cover fontdb's initial discovery.
///
/// If a file disappears before outlining, its glyphs are omitted. If it changes, glyphs may
/// come from the new file or be omitted. Parsing and outlining are guarded against unwinding
/// panics, subject to the memory-mapping caveat above.
///
/// # Examples
///
/// ```
/// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
///
/// let lib = FontLibrary::with_fallbacks(Vec::new());
/// let fonts = lib.with_figure_fonts(&[Font::JETBRAINS_MONO])?;
/// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 9.0, tabular_figures: true };
/// assert!(fonts.layout("1,234.5", &style)?.width() > 0.0);
/// # Ok::<(), umber_core::Error>(())
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
    metrics: FaceMetrics,
}

impl BundledFace {
    fn new(bytes: &'static [u8]) -> Option<BundledFace> {
        let font = FontRef::new(bytes).ok()?;
        let metrics = font.metrics(Size::unscaled(), LocationRef::default());
        Some(BundledFace {
            shaping: ShapingFont::new(bytes, 0)?,
            face: Face::parse(bytes, 0)?,
            metrics: FaceMetrics {
                units_per_em: metrics.units_per_em,
                ascender: metrics.ascent,
                descender: metrics.descent,
            },
        })
    }
}

/// The resolved primary face of one layout, independent of where its bytes came from.
struct PrimaryFace<'a> {
    key: FontKey,
    handle: FontHandle,
    shaping: &'a ShapingFont,
    face: &'a Face<'a>,
    metrics: FaceMetrics,
}

/// A font passed to [`FontLibrary::with_fallbacks`]: its bytes, shared with layouts, and the
/// shaping font built from the same bytes.
struct ExtraFont {
    bytes: Arc<Vec<u8>>,
    shaping: ShapingFont,
}

fn bundled_slot(font: BundledFont, weight: Weight) -> usize {
    let family = match font {
        BundledFont::SourceSerif4 => 0,
        BundledFont::IbmPlexSans => 1,
        BundledFont::Geist => 2,
        BundledFont::Inter => 3,
        BundledFont::JetBrainsMono => 4,
    };
    family * Weight::ALL.len() + weight.slot()
}

/// Fallback faces parsed for the duration of one layout call.
type ParsedExtras<'a> = Vec<Option<Face<'a>>>;

/// A shaped run waiting to be placed: its font and glyphs.
type ShapedSlot = Option<(FontKey, FontHandle, ShapedRun)>;

impl FontLibrary {
    fn new(extra: Vec<Vec<u8>>, system: Option<SystemFonts>) -> FontLibrary {
        let mut bundled: [Option<BundledFace>; BUNDLED_COUNT] = std::array::from_fn(|_| None);
        for font in BundledFont::ALL {
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
    /// System font files are memory-mapped when read; a file truncated or replaced while mapped
    /// can crash the process (`SIGBUS`) on some platforms. See
    /// [System fonts and memory mapping](FontLibrary#system-fonts-and-memory-mapping).
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let lib = FontLibrary::bundled();
    /// let fonts = lib.with_figure_fonts(&[Font::INTER])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// assert!(fonts.layout("Profit", &style)?.width() > 0.0);
    /// # Ok::<(), umber_core::Error>(())
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
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let lib = FontLibrary::with_fallbacks(vec![b"not a font".to_vec()]);
    /// let fonts = lib.with_figure_fonts(&[Font::INTER])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// assert!(fonts.layout("AB", &style)?.width() > 0.0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn with_fallbacks(fonts: Vec<Vec<u8>>) -> FontLibrary {
        FontLibrary::new(fonts, None)
    }

    /// A process-wide [`FontLibrary::bundled`] library, created on first use.
    ///
    /// This is the recommended way to get a library with system font fallback: one library, one
    /// set of lookup and shaping caches for the whole process. The memory-mapping caveat
    /// of [system fonts](FontLibrary#system-fonts-and-memory-mapping) applies.
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

    /// The fonts of one scene: `fonts`, in order, so that [`FaceId::new(i)`](FaceId::new) names
    /// `fonts[i]`. A font may be listed more than once.
    ///
    /// # Errors
    ///
    /// None for the bundled fonts, which are always available.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::SOURCE_SERIF_4, Font::INTER])?;
    /// assert_eq!(fonts.font(FaceId::new(1)), Some(&Font::INTER));
    /// let title = TextStyle { face: FaceId::new(0), weight: Weight::SemiBold, size: 15.0, tabular_figures: false };
    /// assert!(fonts.layout("Revenue", &title)?.width() > 0.0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_figure_fonts(&self, fonts: &[Font]) -> Result<SceneFonts<'_>> {
        Ok(SceneFonts {
            library: self,
            fonts: fonts.to_vec(),
            context: LAYOUT_CONTEXT,
            #[cfg(test)]
            missing_weights: Vec::new(),
        })
    }

    fn bundled_face(&self, font: BundledFont, weight: Weight) -> Option<&BundledFace> {
        self.bundled
            .get(bundled_slot(font, weight))
            .and_then(Option::as_ref)
    }

    /// A shared handle to a run's font. Scene keys only arise from this layout's primary face.
    fn handle(&self, key: FontKey, primary: &PrimaryFace<'_>) -> Option<FontHandle> {
        match key {
            FontKey::Bundled(font, weight) => Some(FontHandle::bundled(font, weight)),
            FontKey::Extra(index) => self
                .extra
                .get(index)
                .map(|extra| FontHandle::extra(Arc::clone(&extra.bytes))),
            FontKey::System(face) => self.system.as_ref()?.handle(face),
            FontKey::Scene(..) => Some(primary.handle.clone()),
        }
    }

    /// Resolves bundled or owned bytes to the same shaping path.
    fn layout_font(&self, text: &str, font: &Font, style: &TextStyle) -> TextLayout {
        match font {
            Font::Bundled(font) => {
                let Some(face) = self.bundled_face(*font, style.weight) else {
                    return TextLayout::empty(0.0, 0.0);
                };
                self.layout_primary(
                    text,
                    PrimaryFace {
                        key: FontKey::Bundled(*font, style.weight),
                        handle: FontHandle::bundled(*font, style.weight),
                        shaping: &face.shaping,
                        face: &face.face,
                        metrics: face.metrics,
                    },
                    style,
                )
            }
            Font::File(_) | Font::System(_) => guarded(|| {
                let owned = font.owned_face(style.weight)?;
                let face = Face::parse(&owned.bytes, owned.index)?;
                Some(self.layout_primary(
                    text,
                    PrimaryFace {
                        key: FontKey::Scene(style.face, style.weight),
                        handle: FontHandle::owned(owned),
                        shaping: &owned.shaping,
                        face: &face,
                        metrics: owned.metrics,
                    },
                    style,
                ))
            })
            .flatten()
            .unwrap_or_else(|| TextLayout::empty(0.0, 0.0)),
        }
    }

    /// Shapes and measures text with a resolved primary face and the usual fallback order.
    fn layout_primary(
        &self,
        text: &str,
        primary: PrimaryFace<'_>,
        style: &TextStyle,
    ) -> TextLayout {
        let size = if style.size.is_finite() && style.size > 0.0 {
            style.size
        } else {
            0.0
        };
        let scale = size / f32::from(primary.metrics.units_per_em.max(1));
        let (ascent, descent) = (
            (primary.metrics.ascender * scale).max(0.0),
            (-primary.metrics.descender * scale).max(0.0),
        );
        if size == 0.0 || text.is_empty() {
            return TextLayout::empty(ascent, descent);
        }

        let runs = self.split_runs(text, &primary, style);
        let features = features(style.tabular_figures);
        let primary_key = primary.key;
        let mut shaped: Vec<ShapedSlot> = runs.iter().map(|_| None).collect();
        let extents_budget = Cell::new(EXTENTS_PER_LAYOUT);
        for (key, members) in group_by_font(runs.iter().map(|run| run.font)) {
            let handle = self.handle(key, &primary);
            let Some(handle) = handle else { continue };
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
                FontKey::Scene(..) => Some(shape_all(primary.shaping, primary.face)),
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
        {
            let funcs = Funcs::new(primary.face);
            let mut shaper = Shaper::new(primary.shaping, &funcs, &features);
            let handle = primary.handle;
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
    fn split_runs(&self, text: &str, primary: &PrimaryFace<'_>, style: &TextStyle) -> Vec<TextRun> {
        let primary_key = primary.key;
        let mut extras: Option<ParsedExtras<'_>> = None;
        let mut budget = Budget::per_layout();
        let mut extra_budget = EXTRA_CONFIRMATIONS_PER_LAYOUT;
        runs::split_runs(
            text,
            primary_key,
            |c| primary.face.glyph(c).is_some(),
            |c| {
                self.fallback_for(
                    c,
                    primary_key,
                    style.weight,
                    &mut extras,
                    &mut extra_budget,
                    &mut budget,
                )
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
        primary: FontKey,
        weight: Weight,
        extras: &mut Option<ParsedExtras<'a>>,
        extra_budget: &mut usize,
        budget: &mut Budget,
    ) -> Option<FontKey> {
        for font in BundledFont::ALL {
            if FontKey::Bundled(font, weight) == primary {
                continue;
            }
            if self
                .bundled_face(font, weight)
                .is_some_and(|face| face.face.glyph(c).is_some())
            {
                return Some(FontKey::Bundled(font, weight));
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
            let key = (c, weight, index);
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
            .face_for(c, weight, budget)
            .map(FontKey::System)
    }
}

/// The fonts of one scene, laid out through a [`FontLibrary`].
///
/// Made by [`FontLibrary::with_figure_fonts`]. A [`TextStyle`] names its primary font by
/// [`FaceId`]: `FaceId::new(i)` is the `i`-th font of the list the fonts were made from.
///
/// # Examples
///
/// ```
/// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
///
/// let fonts = FontLibrary::global().with_figure_fonts(&[Font::GEIST, Font::JETBRAINS_MONO])?;
/// let mono = TextStyle { face: FaceId::new(1), weight: Weight::Regular, size: 9.0, tabular_figures: true };
/// assert_eq!(fonts.layout("1111", &mono)?.width(), fonts.layout("8888", &mono)?.width());
/// # Ok::<(), umber_core::Error>(())
/// ```
pub struct SceneFonts<'a> {
    library: &'a FontLibrary,
    fonts: Vec<Font>,
    /// The API call that errors of these fonts are reported in.
    context: &'static str,
    /// Faces and weights treated as missing, to exercise the error of a font without a face
    /// for a weight.
    #[cfg(test)]
    missing_weights: Vec<(FaceId, Weight)>,
}

impl std::fmt::Debug for SceneFonts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneFonts")
            .field("library", self.library)
            .field("fonts", &self.fonts)
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

/// The context of the errors of fonts made by [`FontLibrary::with_figure_fonts`].
const LAYOUT_CONTEXT: &str = "SceneFonts.layout()";

/// The hint of the error for a face that names no font.
const UNKNOWN_FACE_HINT: &str = "name a face by the index of a font given to with_figure_fonts.";

/// The hint of the error for a font without a face for the weight of the text.
const MISSING_WEIGHT_HINT: &str =
    "pass bold= to Font.from_file, choose a family with a bold face, or pass bold_fallback=True.";

/// Why a style cannot be laid out with a scene's fonts.
enum Unavailable<'a> {
    /// The style's face names no font.
    Face(FaceId),
    /// The font of the style's face has no face for the style's weight.
    Weight(&'a Font, Weight),
}

/// A text style checked against a scene's fonts by [`SceneFonts::check`], with the font of its
/// face: text in it has an available weight and lays out without error.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CheckedStyle {
    style: TextStyle,
    font: Font,
}

impl CheckedStyle {
    /// The style.
    pub(crate) fn style(&self) -> TextStyle {
        self.style
    }

    /// The font of the style's face.
    pub(crate) fn font(&self) -> &Font {
        &self.font
    }
}

/// The name of `weight` in error messages.
fn weight_name(weight: Weight) -> &'static str {
    match weight {
        Weight::Regular => "regular",
        Weight::SemiBold => "bold",
    }
}

impl<'a> SceneFonts<'a> {
    /// These fonts with their errors reported in `context`, the API call that lays text out
    /// with them, such as `"Figure.to_png()"`.
    pub(crate) fn in_context(self, context: &'static str) -> SceneFonts<'a> {
        SceneFonts { context, ..self }
    }

    /// These fonts with the font of `face` treated as having no face for `weight`.
    #[cfg(test)]
    pub(crate) fn without_weight(mut self, face: FaceId, weight: Weight) -> SceneFonts<'a> {
        self.missing_weights.push((face, weight));
        self
    }
}

impl SceneFonts<'_> {
    /// The fonts, in face order.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Font, FontLibrary};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
    /// assert_eq!(fonts.fonts(), [Font::INTER]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn fonts(&self) -> &[Font] {
        &self.fonts
    }

    /// The font of `face`, or `None` when these fonts have no such face.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{FaceId, Font, FontLibrary};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
    /// assert_eq!(fonts.font(FaceId::new(0)), Some(&Font::INTER));
    /// assert_eq!(fonts.font(FaceId::new(1)), None);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn font(&self, face: FaceId) -> Option<&Font> {
        self.fonts.get(face.index())
    }

    /// Shapes `text` with `style` and measures it.
    ///
    /// Characters missing from the font of `style.face` are drawn with the other bundled
    /// families in the same weight, then with the fallback fonts, and as `.notdef` if no font
    /// has them.
    ///
    /// One call checks at most 4,096 uncached glyph outlines in the
    /// [`with_fallbacks`](FontLibrary::with_fallbacks) fonts; beyond that, characters skip
    /// those fonts. It looks up at most 1,024 distinct characters in the system fonts that are
    /// not already cached, and opens at most 4,096 candidate system fonts to confirm a glyph
    /// (at most 16 per character). Characters beyond these limits are drawn as `.notdef`. This
    /// bounds the work a single string can cause.
    ///
    /// # Errors
    ///
    /// `Error::Usage` when `style.face` names none of these fonts, and `Error::Style`, naming
    /// `text` and the font, when the font has no face for `style.weight` and bold fallback is
    /// disabled. The bundled fonts have a face for every weight.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let small = fonts.layout("Signal", &style)?.width();
    /// let large = fonts.layout("Signal", &TextStyle { size: 20.0, ..style })?.width();
    /// assert!((large - 2.0 * small).abs() < 1e-3);
    /// assert!(fonts.layout("Signal", &TextStyle { face: FaceId::new(1), ..style }).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn layout(&self, text: &str, style: &TextStyle) -> Result<TextLayout> {
        let checked = self.check(text, style)?;
        Ok(self.layout_checked(text, &checked))
    }

    /// `style` checked against these fonts for setting `text`.
    ///
    /// # Errors
    ///
    /// As [`SceneFonts::layout`]: the error of a missing weight names `text`.
    pub(crate) fn check(&self, text: &str, style: &TextStyle) -> Result<CheckedStyle> {
        self.primary(style)
            .map_err(|unavailable| self.error(unavailable, Some(text)))
    }

    /// Shapes `text` with the checked style `style` and measures it, as
    /// [`SceneFonts::layout`] does.
    pub(crate) fn layout_checked(&self, text: &str, style: &CheckedStyle) -> TextLayout {
        self.library.layout_font(text, &style.font, &style.style)
    }

    /// The ascent and descent of the primary font of `style`, in points.
    ///
    /// # Errors
    ///
    /// As [`SceneFonts::layout`]; the error of a missing weight names the font and the weight
    /// but no text, so a caller with text to set checks that text first
    /// ([`SceneFonts::check`]).
    pub(crate) fn metrics(&self, style: &TextStyle) -> Result<(f32, f32)> {
        let primary = self
            .primary(style)
            .map_err(|unavailable| self.error(unavailable, None))?;
        let empty = self.layout_checked("", &primary);
        Ok((empty.ascent(), empty.descent()))
    }

    /// The primary font of `style`, checked for the requested weight.
    ///
    /// The font resolves the actual weight, including its bold fallback setting. A missing
    /// weight with no fallback is detected here as [`Unavailable::Weight`]. Unit tests also
    /// treat the faces and weights given to `without_weight` as missing.
    fn primary(&self, style: &TextStyle) -> std::result::Result<CheckedStyle, Unavailable<'_>> {
        let font = self.font(style.face).ok_or(Unavailable::Face(style.face))?;
        let weight = font.resolved_weight(style.weight);
        #[cfg(test)]
        let weight = weight.filter(|_| !self.missing_weights.contains(&(style.face, style.weight)));
        let weight = weight.ok_or(Unavailable::Weight(font, style.weight))?;
        Ok(CheckedStyle {
            style: TextStyle { weight, ..*style },
            font: font.clone(),
        })
    }

    /// The error for a style that cannot be laid out, naming `text` when there is text to set.
    fn error(&self, unavailable: Unavailable<'_>, text: Option<&str>) -> Error {
        match unavailable {
            Unavailable::Face(face) => {
                let fonts = match self.fonts.len().checked_sub(1) {
                    Some(last) => format!("the last face is {last}"),
                    None => "there are no fonts".to_owned(),
                };
                Error::usage(
                    self.context,
                    format!(
                        "the text style names face {}, but {fonts}; a face must be the index \
                         of one of the fonts.",
                        face.index()
                    ),
                )
                .with_hint(UNKNOWN_FACE_HINT)
            }
            Unavailable::Weight(font, weight) => {
                let subject = text.map_or_else(
                    || "text".to_owned(),
                    |text| format!("the text {}", quoted(text)),
                );
                let reason = if weight == Weight::SemiBold && font.is_variable_only_system() {
                    " The family's installed files are variable fonts; only the default instance is used."
                } else {
                    ""
                };
                Error::style(
                    self.context,
                    format!(
                        "{subject} needs the {} face of {}, which the font does not have; text \
                         can only be set in a weight its font has.{reason}",
                        weight_name(weight),
                        font.family_name()
                    ),
                )
                .with_hint(MISSING_WEIGHT_HINT)
            }
        }
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
    pub(super) fn bundled_parsed(
        &self,
        font: BundledFont,
        weight: Weight,
    ) -> Option<&Face<'static>> {
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
        let mut slots: Vec<usize> = BundledFont::ALL
            .iter()
            .flat_map(|&f| Weight::ALL.iter().map(move |&w| bundled_slot(f, w)))
            .collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), BUNDLED_COUNT);
        assert!(lib.bundled.iter().all(Option::is_some));
    }

    #[test]
    fn bundled_slots_are_family_major_then_weight() {
        let expected = [
            (BundledFont::SourceSerif4, Weight::Regular, 0),
            (BundledFont::SourceSerif4, Weight::SemiBold, 1),
            (BundledFont::IbmPlexSans, Weight::Regular, 2),
            (BundledFont::IbmPlexSans, Weight::SemiBold, 3),
            (BundledFont::Geist, Weight::Regular, 4),
            (BundledFont::Geist, Weight::SemiBold, 5),
            (BundledFont::Inter, Weight::Regular, 6),
            (BundledFont::Inter, Weight::SemiBold, 7),
            (BundledFont::JetBrainsMono, Weight::Regular, 8),
            (BundledFont::JetBrainsMono, Weight::SemiBold, 9),
        ];
        assert_eq!(expected.len(), BUNDLED_COUNT);
        for (font, weight, slot) in expected {
            assert_eq!(bundled_slot(font, weight), slot, "{font:?} {weight:?}");
        }
    }
}
