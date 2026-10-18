//! Placed glyphs and [`TextLayout`]: shaped runs set left to right, measured and outlined;
//! [`TextGlyphs`]: the glyphs of a layout as a backend embeds them in a font.

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

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
    source: RunSource,
    /// Points per font unit.
    scale: f32,
    glyphs: Vec<PlacedGlyph>,
    /// The text the glyphs were shaped from, which their clusters index.
    text: String,
}

/// Where the program of a run's font is read from.
#[derive(Debug, Clone)]
enum RunSource {
    /// Through the handle the run was shaped with. The bytes of a bundled font and of a
    /// fallback font file never change; a system font is read from its file again.
    Handle(FontHandle),
    /// A copy of a system font's program, taken when the text was captured into a scene and
    /// shared by every run of that font there, so later changes to the file do not reach it.
    Captured(Arc<FontProgram>),
}

/// The bytes of a font file and the index of the face in it.
#[derive(Debug)]
struct FontProgram {
    bytes: Vec<u8>,
    index: u32,
}

impl RunSource {
    /// Runs `f` on the font bytes and face index, and returns what it returns; `None` when the
    /// bytes cannot be read. A font that did not ship with the crate is read under a panic
    /// guard, so a panic in `f` caused by malformed font data is also `None`.
    fn read<T>(&self, f: impl FnOnce(&[u8], u32) -> Option<T>) -> Option<T> {
        match self {
            RunSource::Handle(handle) if handle.is_bundled() => handle.with_data(f).flatten(),
            RunSource::Handle(handle) => handle
                .with_data(|data, index| guarded(|| f(data, index)).flatten())
                .flatten(),
            RunSource::Captured(program) => guarded(|| f(&program.bytes, program.index)).flatten(),
        }
    }
}

impl PartialEq for GlyphRun {
    /// Runs are equal when they draw the same glyphs of the same font at the same places, for
    /// the same text. The source follows from the font, so it is not compared.
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
            source: &self.source,
        }
    }

    /// The outlines of the run's glyphs, placed for text whose baseline starts at `origin`, as
    /// [`TextLayout::outline`] draws them. Empty when the font cannot be read.
    pub(crate) fn outline(&self, origin: Point) -> Path {
        self.source
            .read(|data, index| {
                let outlines = FontRef::from_index(data, index).ok()?.outline_glyphs();
                let mut path = Path::new();
                outline_run(&outlines, self, origin, &mut path);
                Some(path)
            })
            .unwrap_or_default()
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

/// The font of a [`GlyphRun`], as a backend that embeds fonts sees it: the identity of its
/// font program, to group the runs of one font by, and the program's bytes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunFont<'a> {
    key: FontKey,
    source: &'a RunSource,
}

/// The identity of a font program: the same font file and face, whichever library or layout
/// the run came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ProgramId(Identity);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Identity {
    /// Bytes that do not move or change while a run refers to them: a bundled font, a fallback
    /// font file or a captured system font, by the address and length of the bytes.
    Bytes {
        address: usize,
        length: usize,
        index: u32,
    },
    /// A system font that was not captured, read from its file: by its place in the system
    /// font index, which is shared by the whole process, and the length of the file.
    System {
        key: FontKey,
        length: usize,
        index: u32,
    },
}

impl RunFont<'_> {
    /// The identity of the font program; `None` when the bytes cannot be read.
    pub(crate) fn program(&self) -> Option<ProgramId> {
        let system = matches!(self.source, RunSource::Handle(_)) && reads_system_file(self.key);
        self.with_data(|data, index| {
            Some(ProgramId(if system {
                Identity::System {
                    key: self.key,
                    length: data.len(),
                    index,
                }
            } else {
                Identity::Bytes {
                    address: data.as_ptr() as usize,
                    length: data.len(),
                    index,
                }
            }))
        })
    }

    /// Runs `f` on the font bytes and face index, and returns what it returns; `None` when the
    /// bytes cannot be read. A font that did not ship with the crate is read under a panic
    /// guard, so a panic in `f` caused by malformed font data is also `None`.
    pub(crate) fn with_data<T>(&self, f: impl FnOnce(&[u8], u32) -> Option<T>) -> Option<T> {
        self.source.read(f)
    }
}

/// Whether the font of `key` is a system font, read again from its file whenever it is read:
/// the bytes of the other fonts never change.
fn reads_system_file(key: FontKey) -> bool {
    match key {
        FontKey::System(_) => true,
        FontKey::Bundled(..) | FontKey::Extra(_) => false,
    }
}

/// Captures the system fonts of text into shared copies (see [`FontCapture::capture`]),
/// keeping one copy of each font program.
#[derive(Debug, Default)]
pub(crate) struct FontCapture {
    /// The programs captured so far, with the system font each was read from.
    programs: Vec<(FontKey, Arc<FontProgram>)>,
}

impl FontCapture {
    /// Remembers the programs that the runs of `glyphs` already captured, so that
    /// [`FontCapture::capture`] shares them.
    pub(crate) fn learn(&mut self, glyphs: &TextGlyphs) {
        for run in &glyphs.runs {
            if let RunSource::Captured(program) = &run.source {
                self.remember(run.font, program);
            }
        }
    }

    fn remember(&mut self, key: FontKey, program: &Arc<FontProgram>) {
        if !self
            .programs
            .iter()
            .any(|(_, known)| Arc::ptr_eq(known, program))
        {
            self.programs.push((key, Arc::clone(program)));
        }
    }

    /// Makes every run of `glyphs` that reads a system font from its file read a copy of the
    /// program instead, taken now and shared with the other runs this capture has seen of the
    /// same program (the same bytes and face). Runs already captured are kept and their
    /// programs shared from then on. Runs of other fonts are unchanged: their bytes never
    /// change. A system font that can no longer be read is left as it is.
    pub(crate) fn capture(&mut self, glyphs: &mut TextGlyphs) {
        for run in &mut glyphs.runs {
            match &run.source {
                RunSource::Captured(program) => self.remember(run.font, program),
                RunSource::Handle(handle) => {
                    if !reads_system_file(run.font) {
                        continue;
                    }
                    let key = run.font;
                    let programs = &mut self.programs;
                    let captured = handle.with_data(|data, index| {
                        let known = programs.iter().find(|(known_key, program)| {
                            *known_key == key
                                && program.index == index
                                && program.bytes.as_slice() == data
                        });
                        match known {
                            Some((_, program)) => Arc::clone(program),
                            None => {
                                let program = Arc::new(FontProgram {
                                    bytes: data.to_vec(),
                                    index,
                                });
                                programs.push((key, Arc::clone(&program)));
                                program
                            }
                        }
                    });
                    if let Some(program) = captured {
                        run.source = RunSource::Captured(program);
                    }
                }
            }
        }
    }
}

/// The glyphs a piece of text was drawn with, run by run: for each run the font, the glyph ids
/// and positions, and the text each glyph stands for. A backend that writes text as text, such
/// as PDF, embeds exactly these glyphs. The [default](Default) has no glyphs, as a text record
/// built by hand; such text can only be drawn from its outline.
///
/// The glyphs of a text item in a scene hold the programs of their fonts: a system font is
/// copied when the text enters the scene ([`Scene::push_text`](crate::Scene::push_text),
/// [`Figure::to_scene`](crate::Figure::to_scene)), once per font program and shared by every
/// text item that uses it, so later changes to the font file do not change the scene.
///
/// # Examples
///
/// ```
/// use umber_core::scene::{Item, Scene};
/// use umber_core::{FaceId, Font, FontLibrary, Point, Rgba, TextStyle, Weight};
///
/// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
/// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
/// let black = Rgba { r: 0, g: 0, b: 0, a: 255 };
/// let mut scene = Scene::new(100.0, 30.0, black);
/// scene.push_text(&fonts, "Hi", style, Point { x: 2.0, y: 20.0 }, black)?;
/// let Some(Item::Text(item)) = scene.items.first() else { unreachable!() };
/// assert_eq!(item.glyphs.glyph_count(), 2);
/// assert!(umber_core::TextGlyphs::default().is_empty());
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
    /// use umber_core::scene::{Item, Scene};
    /// use umber_core::{FaceId, Font, FontLibrary, Point, Rgba, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::GEIST])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let black = Rgba { r: 0, g: 0, b: 0, a: 255 };
    /// let mut scene = Scene::new(100.0, 30.0, black);
    /// scene.push_text(&fonts, "", style, Point::default(), black)?;
    /// scene.push_text(&fonts, "a", style, Point::default(), black)?;
    /// let glyphs: Vec<bool> = scene
    ///     .items
    ///     .iter()
    ///     .map(|item| matches!(item, Item::Text(text) if text.glyphs.is_empty()))
    ///     .collect();
    /// assert_eq!(glyphs, [true, false]);
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
    /// use umber_core::scene::{Item, Scene};
    /// use umber_core::{FaceId, Font, FontLibrary, Point, Rgba, TextStyle, Weight};
    ///
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER])?;
    /// let style = TextStyle { face: FaceId::new(0), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let black = Rgba { r: 0, g: 0, b: 0, a: 255 };
    /// let mut scene = Scene::new(100.0, 30.0, black);
    /// scene.push_text(&fonts, "a b", style, Point::default(), black)?;
    /// let Some(Item::Text(item)) = scene.items.first() else { unreachable!() };
    /// assert_eq!(item.glyphs.glyph_count(), 3);
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
                source: RunSource::Handle(handle),
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
    /// with, and the text each glyph stands for. System fonts are still read from their files;
    /// a scene captures them ([`FontCapture`]).
    pub(crate) fn glyphs(&self) -> TextGlyphs {
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
            let Some(first) = members.first().and_then(|&i| self.runs.get(i)) else {
                continue;
            };
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
            for (i, path) in first.source.read(draw).unwrap_or_default() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{FALLBACK_TEST_CODEPOINT, Font, Weight};
    use crate::text::{FaceId, FontLibrary, TextStyle};

    /// `text` laid out in Inter with the fallback fixture as the only system font.
    fn glyphs(library: &FontLibrary, text: &str) -> TextGlyphs {
        let fonts = library.with_figure_fonts(&[Font::INTER]).unwrap();
        let style = TextStyle {
            face: FaceId::new(0),
            weight: Weight::Regular,
            size: 12.0,
            tabular_figures: false,
        };
        fonts.layout(text, &style).unwrap().glyphs()
    }

    fn system_library() -> FontLibrary {
        FontLibrary::with_test_system_fonts(vec![
            include_bytes!("../../tests/fixtures/fallback-only.ttf").to_vec(),
        ])
    }

    /// The captured program of the system font run of `glyphs`.
    fn captured(glyphs: &TextGlyphs) -> Arc<FontProgram> {
        let mut programs = glyphs.runs.iter().filter_map(|run| match &run.source {
            RunSource::Captured(program) => Some(Arc::clone(program)),
            RunSource::Handle(_) => None,
        });
        let program = programs.next().expect("a captured run");
        assert!(programs.next().is_none());
        program
    }

    #[test]
    fn capture_copies_each_system_font_once() {
        let library = system_library();
        let text = format!("A{FALLBACK_TEST_CODEPOINT}");
        let (mut first, mut second) = (glyphs(&library, &text), glyphs(&library, &text));
        let mut capture = FontCapture::default();
        capture.capture(&mut first);
        capture.capture(&mut second);
        assert!(Arc::ptr_eq(&captured(&first), &captured(&second)));
        // A capture that learned the first program shares it too.
        let mut third = glyphs(&library, &text);
        let mut later = FontCapture::default();
        later.learn(&first);
        later.capture(&mut third);
        assert!(Arc::ptr_eq(&captured(&first), &captured(&third)));
        // Captured runs keep the program identity of their copy.
        let ids: Vec<ProgramId> = [&first, &third]
            .iter()
            .map(|glyphs| glyphs.runs[1].run_font().program().unwrap())
            .collect();
        assert_eq!(ids[0], ids[1]);
    }

    #[test]
    fn capture_leaves_other_fonts_alone() {
        let library = FontLibrary::with_fallbacks(vec![
            include_bytes!("../../tests/fixtures/fallback-only.ttf").to_vec(),
        ]);
        let mut text = glyphs(&library, &format!("A{FALLBACK_TEST_CODEPOINT}"));
        FontCapture::default().capture(&mut text);
        assert!(
            text.runs
                .iter()
                .all(|run| matches!(run.source, RunSource::Handle(_)))
        );
    }

    #[test]
    fn equal_indices_of_different_libraries_are_different_programs() {
        let a = FontLibrary::with_fallbacks(vec![
            include_bytes!("../../tests/fixtures/outline-cff.otf").to_vec(),
        ]);
        let b = FontLibrary::with_fallbacks(vec![
            include_bytes!("../../tests/fixtures/outline-variable.ttf").to_vec(),
        ]);
        let program = |library: &FontLibrary, text: &str| {
            glyphs(library, text).runs[0].run_font().program().unwrap()
        };
        assert_ne!(program(&a, "\u{E001}"), program(&b, "\u{E004}"));
        assert_eq!(program(&a, "\u{E001}"), program(&a, "\u{E002}"));
    }
}
