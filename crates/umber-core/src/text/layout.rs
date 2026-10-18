//! Placed glyphs and [`TextLayout`]: shaped runs set left to right, measured and outlined.

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
#[derive(Debug, Clone, Copy)]
pub(super) struct PlacedGlyph {
    pub(super) id: u16,
    pub(super) x: f32,
    pub(super) y: f32,
}

/// Consecutive glyphs drawn with one font.
#[derive(Debug, Clone)]
pub(super) struct GlyphRun {
    /// Identifies the font within the library that made the layout; used for grouping.
    font: FontKey,
    /// The font itself, so that the layout can be outlined on its own.
    handle: FontHandle,
    /// Points per font unit.
    scale: f32,
    glyphs: Vec<PlacedGlyph>,
}

#[cfg(test)]
impl GlyphRun {
    pub(super) fn font(&self) -> FontKey {
        self.font
    }

    pub(super) fn glyphs(&self) -> &[PlacedGlyph] {
        &self.glyphs
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
    width: f32,
    ascent: f32,
    descent: f32,
}

impl TextLayout {
    /// A layout without glyphs, with the primary font's ascent and descent in points.
    pub(super) fn empty(ascent: f32, descent: f32) -> TextLayout {
        TextLayout {
            runs: Vec::new(),
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
        let mut layout = TextLayout::empty(ascent, descent);
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
