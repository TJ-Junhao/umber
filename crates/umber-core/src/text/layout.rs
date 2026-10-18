//! Placed glyphs and [`TextLayout`]: shaped runs set left to right, measured and outlined;
//! [`TextGlyphs`]: the glyphs of a layout as a backend embeds them in a font.

use std::collections::{BTreeSet, HashSet};

use read_fonts::FontRef;
use read_fonts::types::GlyphId;
use skrifa::MetadataProvider;
use skrifa::outline::OutlineGlyphCollection;

use super::font::{FontHandle, FontKey};
use super::guard::guarded;
use super::outline::append_glyph;
use super::runs::group_by_font;
use super::shaping::ShapedRun;
use crate::geometry::{Path, Point};

/// A glyph placed relative to the layout origin, in points, y-down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlacedGlyph {
    /// The glyph id in the font of its run.
    pub(crate) id: u16,
    /// The horizontal position of the glyph origin, from the left end of the baseline.
    pub(crate) x: f32,
    /// The vertical offset of the glyph origin from the baseline, downwards.
    pub(crate) y: f32,
    /// The byte offset in the run's text where the characters this glyph stands for start.
    cluster: u32,
}

/// Consecutive glyphs drawn with one font.
#[derive(Debug, Clone)]
pub(crate) struct GlyphRun {
    /// Identifies the font within the library that made the layout; used for grouping.
    font: FontKey,
    /// The font itself, so that the layout can be outlined on its own.
    handle: FontHandle,
    /// Points per font unit.
    scale: f32,
    glyphs: Vec<PlacedGlyph>,
    /// The text the glyphs were shaped from, which their clusters index.
    text: String,
}

impl PartialEq for GlyphRun {
    /// Runs are equal when they draw the same glyphs of the same font at the same places, for
    /// the same text. The handle follows from the font, so it is not compared.
    fn eq(&self, other: &Self) -> bool {
        self.font == other.font
            && self.scale == other.scale
            && self.glyphs == other.glyphs
            && self.text == other.text
    }
}

impl GlyphRun {
    /// The font the run is drawn with.
    pub(crate) fn run_font(&self) -> RunFont<'_> {
        RunFont {
            key: self.font,
            handle: &self.handle,
        }
    }

    /// The glyphs, in the order they were shaped.
    pub(crate) fn glyphs(&self) -> &[PlacedGlyph] {
        &self.glyphs
    }

    /// The text each glyph stands for, one entry per glyph of [`GlyphRun::glyphs`].
    ///
    /// The characters of a cluster go to its first glyph; the cluster's other glyphs stand for
    /// no text. A cluster runs from its offset to the next larger cluster offset of the run, or
    /// to the end of the text, so a ligature stands for all the characters it joins.
    pub(crate) fn glyph_texts(&self) -> Vec<&str> {
        let starts: BTreeSet<u32> = self.glyphs.iter().map(|g| g.cluster).collect();
        let mut seen: HashSet<u32> = HashSet::new();
        self.glyphs
            .iter()
            .map(|glyph| {
                if !seen.insert(glyph.cluster) {
                    return "";
                }
                let start = glyph.cluster as usize;
                let end = starts
                    .range(glyph.cluster + 1..)
                    .next()
                    .map_or(self.text.len(), |&next| next as usize);
                self.text.get(start..end).unwrap_or("")
            })
            .collect()
    }
}

#[cfg(test)]
impl GlyphRun {
    pub(super) fn font(&self) -> FontKey {
        self.font
    }
}

/// The font of a [`GlyphRun`], as a backend that embeds fonts sees it: an identity to group the
/// runs of one font by, and the font's bytes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunFont<'a> {
    key: FontKey,
    handle: &'a FontHandle,
}

impl PartialEq for RunFont<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for RunFont<'_> {}

impl std::hash::Hash for RunFont<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl RunFont<'_> {
    /// Runs `f` on the font bytes and face index, and returns what it returns; `None` when the
    /// bytes cannot be read. A font that did not ship with the crate is read under a panic
    /// guard, so a panic in `f` caused by malformed font data is also `None`.
    pub(crate) fn with_data<T>(&self, f: impl FnOnce(&[u8], u32) -> Option<T>) -> Option<T> {
        if self.handle.is_bundled() {
            self.handle.with_data(f).flatten()
        } else {
            self.handle
                .with_data(|data, index| guarded(|| f(data, index)).flatten())
                .flatten()
        }
    }
}

/// The glyphs a piece of text was drawn with, run by run: for each run the font, the glyph ids
/// and positions, and the text each glyph stands for. A backend that writes text as text, such
/// as PDF, embeds exactly these glyphs. The [default](Default) has no glyphs, as a text record
/// built by hand; such text can only be drawn from its outline.
///
/// # Examples
///
/// ```
/// use umber_core::{FaceId, Font, FontLibrary, TextGlyphs, TextStyle, Weight};
///
/// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
/// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
/// let glyphs = fonts.layout("Hi", &style)?.glyphs();
/// assert_eq!(glyphs.glyph_count(), 2);
/// assert!(TextGlyphs::default().is_empty());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextGlyphs {
    /// The font size in points.
    size: f32,
    runs: Vec<GlyphRun>,
}

impl TextGlyphs {
    /// Whether there are no glyphs.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::GEIST])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// assert!(fonts.layout("", &style)?.glyphs().is_empty());
    /// assert!(!fonts.layout("a", &style)?.glyphs().is_empty());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|run| run.glyphs.is_empty())
    }

    /// The number of glyphs, over all runs.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// assert_eq!(fonts.layout("a b", &style)?.glyphs().glyph_count(), 3);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.runs.iter().map(|run| run.glyphs.len()).sum()
    }

    /// The font size in points.
    pub(crate) fn size(&self) -> f32 {
        self.size
    }

    /// The runs, in order.
    pub(crate) fn runs(&self) -> &[GlyphRun] {
        &self.runs
    }
}

/// Shaped and measured text, ready to be outlined.
///
/// Produced by [`SceneFonts::layout`](crate::text::SceneFonts::layout). A layout holds shared
/// handles to the fonts it was shaped with, so it can be outlined with [`TextLayout::outline`]
/// on any thread, even after the library that made it is gone.
///
/// # Examples
///
/// ```
/// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
///
/// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
/// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 12.0, tabular_figures: false };
/// let layout = fonts.layout("Hg", &style)?;
/// assert!(layout.width() > 0.0 && layout.ascent() > 0.0 && layout.descent() > 0.0);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct TextLayout {
    runs: Vec<GlyphRun>,
    /// The font size in points.
    size: f32,
    width: f32,
    ascent: f32,
    descent: f32,
}

impl TextLayout {
    /// A layout without glyphs, with the primary font's ascent and descent in points.
    pub(super) fn empty(ascent: f32, descent: f32) -> TextLayout {
        TextLayout {
            runs: Vec::new(),
            size: 0.0,
            width: 0.0,
            ascent,
            descent,
        }
    }

    /// Places `shaped` runs left to right in order, at `size` points, each run with the font it
    /// was shaped with.
    pub(super) fn place(
        size: f32,
        ascent: f32,
        descent: f32,
        shaped: impl IntoIterator<Item = (FontKey, FontHandle, ShapedRun)>,
    ) -> TextLayout {
        let mut layout = TextLayout {
            size,
            ..TextLayout::empty(ascent, descent)
        };
        let mut pen = 0.0_f32;
        for (key, handle, run) in shaped {
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
                        cluster: g.cluster,
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
                text: run.text,
            });
        }
        layout.width = pen;
        layout
    }

    /// The total advance width in points.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// assert!(fonts.layout("wide text", &style)?.width() > fonts.layout("w", &style)?.width());
    /// # Ok::<(), umber_core::Error>(())
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
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::SOURCE_SERIF_4])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let ascent = fonts.layout("", &style)?.ascent();
    /// assert!(ascent > 5.0 && ascent < 15.0);
    /// # Ok::<(), umber_core::Error>(())
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
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::IBM_PLEX_SANS])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let descent = fonts.layout("g", &style)?.descent();
    /// assert!(descent > 0.0 && descent < 5.0);
    /// # Ok::<(), umber_core::Error>(())
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
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::GEIST])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// assert!(fonts.layout("", &style)?.is_empty());
    /// assert!(!fonts.layout("x", &style)?.is_empty());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|run| run.glyphs.is_empty())
    }

    /// The glyphs of this layout, run by run: the fonts, glyph ids and positions it was set
    /// with, and the text each glyph stands for.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{FaceId, Font, FontLibrary, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 12.0, tabular_figures: false };
    /// let layout = fonts.layout("Axis", &style)?;
    /// assert_eq!(layout.glyphs().glyph_count(), 4);
    /// assert_eq!(layout.glyphs(), layout.glyphs());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn glyphs(&self) -> TextGlyphs {
        TextGlyphs {
            size: self.size,
            runs: self.runs.clone(),
        }
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
    /// use umber_core::{FaceId, Font, FontLibrary, Point, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 12.0, tabular_figures: false };
    /// let layout = fonts.layout("H", &style)?;
    /// let (min, max) = layout.outline(Point { x: 10.0, y: 30.0 }).bounds().unwrap();
    /// assert!(min.x >= 10.0 && max.x <= 10.0 + layout.width());
    /// assert!(min.y < 30.0 && max.y <= 30.0 + 0.01);
    /// # Ok::<(), umber_core::Error>(())
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

    /// The runs of this layout, in order.
    #[cfg(test)]
    pub(super) fn runs(&self) -> &[GlyphRun] {
        &self.runs
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
        append_glyph(
            outlines,
            GlyphId::new(u32::from(glyph.id)),
            run.scale,
            origin.x + glyph.x,
            origin.y + glyph.y,
            path,
        );
    }
}
