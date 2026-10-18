//! Text as text: the fonts of a page, subset to the glyphs its text items were shaped with and
//! embedded as CID fonts with a `ToUnicode` map, and the operators that show the glyphs.
//!
//! A page's fonts are collected from its text items first ([`FontSet`]), so that each font is
//! subset once, with exactly the glyphs drawn, before the content refers to it ([`Fonts`]).
//! Character ids are the glyphs' ids in the subset (`Identity-H`, and `/CIDToGIDMap
//! /Identity` for TrueType outlines), in the order the glyphs are first used, `.notdef` first.

use std::collections::HashMap;

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Chunk, Content, Filter, Finish, Name, Rect as PdfRect, Ref, Str};
use read_fonts::types::{GlyphId, Tag};
use read_fonts::{FontRef, TableProvider};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::string::StringId;
use subsetter::GlyphRemapper;

use super::super::normalize::text_is_visible;
use super::write::{Refs, compress, rounded, subset_tag};
use crate::scene::TextItem;
use crate::text::{GlyphRun, PlacedGlyph, RunFont, TextGlyphs};

/// The most characters a `ToUnicode` entry maps one glyph to; longer clusters keep their first
/// characters. PDF readers accept at most 512 bytes per entry.
const MAX_GLYPH_CHARS: usize = 32;

/// The fonts of a page's text and the glyphs used of each, collected before anything is
/// written.
#[derive(Default)]
pub(super) struct FontSet<'s> {
    fonts: Vec<FontUse<'s>>,
    index: HashMap<RunFont<'s>, usize>,
}

/// One font of a [`FontSet`]: the glyphs used, in first-use order after `.notdef`, and the text
/// each glyph first stood for.
struct FontUse<'s> {
    font: RunFont<'s>,
    glyphs: GlyphRemapper,
    texts: HashMap<u16, String>,
}

/// Whether a text item can be written as text: it is visible and has glyphs.
fn has_glyphs(item: &TextItem) -> bool {
    text_is_visible(item.color, item.origin) && !item.glyphs.is_empty()
}

impl<'s> FontSet<'s> {
    /// Adds the fonts and glyphs of `item`, if it is drawn as text at all.
    pub(super) fn collect(&mut self, item: &'s TextItem) {
        if !has_glyphs(item) {
            return;
        }
        for run in item.glyphs.runs() {
            let font = run.run_font();
            let slot = *self.index.entry(font).or_insert_with(|| {
                self.fonts.push(FontUse {
                    font,
                    glyphs: GlyphRemapper::new(),
                    texts: HashMap::new(),
                });
                self.fonts.len() - 1
            });
            if let Some(used) = self.fonts.get_mut(slot) {
                used.add(run);
            }
        }
    }

    /// Subsets every font to its glyphs. A font that cannot be read again or subset is left
    /// out; text in it is drawn from its outline.
    pub(super) fn embed(self) -> Fonts<'s> {
        Fonts {
            fonts: self.fonts.into_iter().map(FontUse::embed).collect(),
            index: self.index,
        }
    }
}

impl FontUse<'_> {
    fn add(&mut self, run: &GlyphRun) {
        for (glyph, text) in run.glyphs().iter().zip(run.glyph_texts()) {
            // A font has at most 65,535 glyphs, so a full remapper means a malformed run.
            if self.glyphs.num_gids() == u16::MAX {
                return;
            }
            self.glyphs.remap(glyph.id);
            if !text.is_empty() {
                self.texts
                    .entry(glyph.id)
                    .or_insert_with(|| text.chars().take(MAX_GLYPH_CHARS).collect());
            }
        }
    }

    /// The embedded subset, or `None` when the font cannot be read again or subset.
    fn embed(self) -> Option<Embedded> {
        let FontUse {
            font,
            glyphs,
            texts,
        } = self;
        font.with_data(|data, index| Embedded::new(data, index, glyphs, &texts))
    }
}

/// How a font's glyphs are drawn, which decides how PDF embeds it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Outlines {
    /// TrueType `glyf` outlines: a `CIDFontType2` with the whole subset font as `FontFile2`.
    TrueType,
    /// CFF outlines: a `CIDFontType0` with the subset's bare CFF table as `FontFile3`.
    Cff,
}

/// The values of a font descriptor, in glyph space (1000 units per em).
#[derive(Debug, Clone, PartialEq)]
struct Descriptor {
    /// Whether every glyph has the same advance.
    fixed_pitch: bool,
    /// Whether the glyphs slant.
    italic: bool,
    bbox: [f32; 4],
    italic_angle: f32,
    ascent: f32,
    descent: f32,
    cap_height: f32,
    stem_v: f32,
}

/// A font subset ready to be written.
#[derive(Debug)]
pub(super) struct Embedded {
    outlines: Outlines,
    /// The font program: the subset font, or its CFF table.
    program: Vec<u8>,
    /// The PostScript name with the subset tag, as `BaseFont` and `FontName` write it.
    base_font: String,
    /// Each glyph's character id in the subset.
    cids: GlyphRemapper,
    /// The advance of each character id, in glyph space.
    widths: Vec<f32>,
    /// The text of each character id that stands for any, in character id order.
    texts: Vec<(u16, String)>,
    descriptor: Descriptor,
}

impl Embedded {
    /// The subset of face `index` of `data` holding `glyphs`, which stood for `texts`.
    fn new(
        data: &[u8],
        index: u32,
        glyphs: GlyphRemapper,
        texts: &HashMap<u16, String>,
    ) -> Option<Embedded> {
        let font = FontRef::from_index(data, index).ok()?;
        let metrics = font.metrics(Size::unscaled(), LocationRef::default());
        let per_unit = 1000.0 / f32::from(metrics.units_per_em.max(1));
        let advances = font.glyph_metrics(Size::unscaled(), LocationRef::default());
        let widths = glyphs
            .remapped_gids()
            .map(|gid| {
                rounded(
                    advances
                        .advance_width(GlyphId::new(u32::from(gid)))
                        .unwrap_or(0.0)
                        * per_unit,
                )
            })
            .collect();
        let mut cid_texts: Vec<(u16, String)> = texts
            .iter()
            .filter_map(|(gid, text)| Some((glyphs.get(*gid)?, text.clone())))
            .collect();
        cid_texts.sort_by_key(|(cid, _)| *cid);

        let subset = subsetter::subset(data, index, &glyphs).ok()?;
        let cff = {
            let parsed = FontRef::new(&subset).ok()?;
            match parsed.table_data(Tag::new(b"glyf")) {
                Some(_) => None,
                None => Some(parsed.table_data(Tag::new(b"CFF "))?.as_bytes().to_vec()),
            }
        };
        let (outlines, program) = match cff {
            None => (Outlines::TrueType, subset),
            Some(table) => (Outlines::Cff, table),
        };

        let bounds = font.head().ok().map(|head| {
            [head.x_min(), head.y_min(), head.x_max(), head.y_max()]
                .map(|v| rounded(f32::from(v) * per_unit))
        });
        let weight = font.os2().ok().map_or(400, |os2| os2.us_weight_class());
        let ascent = rounded(metrics.ascent * per_unit);
        let descriptor = Descriptor {
            fixed_pitch: metrics.is_monospace,
            italic: metrics.italic_angle != 0.0,
            bbox: bounds.unwrap_or([0.0, rounded(metrics.descent * per_unit), 1000.0, ascent]),
            italic_angle: rounded(metrics.italic_angle),
            ascent,
            descent: rounded(metrics.descent * per_unit),
            cap_height: metrics.cap_height.map_or(ascent, |h| rounded(h * per_unit)),
            // The usual estimate of the vertical stem width from the weight class.
            stem_v: rounded((10.0 + 0.244 * (f32::from(weight) - 50.0)).max(0.0)),
        };
        let base_font = format!("{}+{}", subset_tag(&program), postscript_name(&font));
        Some(Embedded {
            outlines,
            program,
            base_font,
            cids: glyphs,
            widths,
            texts: cid_texts,
            descriptor,
        })
    }

    /// The character id of `glyph`, which [`FontSet::collect`] added.
    fn cid(&self, glyph: u16) -> u16 {
        self.cids.get(glyph).unwrap_or(0)
    }

    /// The advance of character id `cid` in glyph space.
    fn width(&self, cid: u16) -> f32 {
        self.widths.get(usize::from(cid)).copied().unwrap_or(0.0)
    }
}

/// The font's PostScript name, or its family name without spaces, reduced to the characters a
/// PDF name holds without escapes; `Font` when neither has any.
fn postscript_name(font: &FontRef<'_>) -> String {
    let clean = |id: StringId| -> String {
        font.localized_strings(id)
            .english_or_first()
            .map(|name| {
                name.chars()
                    .filter(|c| c.is_ascii_graphic() && !"()<>[]{}/%#".contains(*c))
                    .take(63)
                    .collect()
            })
            .unwrap_or_default()
    };
    [StringId::POSTSCRIPT_NAME, StringId::FAMILY_NAME]
        .into_iter()
        .map(clean)
        .find(|name| !name.is_empty())
        .unwrap_or_else(|| "Font".to_owned())
}

/// The page's fonts, subset and ready to be written and shown.
pub(super) struct Fonts<'s> {
    /// In first-use order; `None` for a font that could not be embedded.
    fonts: Vec<Option<Embedded>>,
    index: HashMap<RunFont<'s>, usize>,
}

/// The resource name of font `index`.
pub(super) fn font_name(index: usize) -> String {
    format!("F{index}")
}

impl Fonts<'_> {
    /// The resource index and subset of the font of `run`, when it was embedded.
    fn get(&self, run: &GlyphRun) -> Option<(usize, &Embedded)> {
        let index = *self.index.get(&run.run_font())?;
        Some((index, self.fonts.get(index)?.as_ref()?))
    }

    /// Whether `item` can be shown as text: it has glyphs, and every font of them was embedded.
    pub(super) fn can_show(&self, item: &TextItem) -> bool {
        has_glyphs(item) && item.glyphs.runs().iter().all(|run| self.get(run).is_some())
    }

    /// Shows the glyphs of `glyphs` in a text object, in a coordinate system whose origin is
    /// the left end of the baseline and whose y axis points up. Runs whose font was not
    /// embedded are left out; [`Fonts::can_show`] tells whether there are any.
    pub(super) fn show(&self, content: &mut Content, glyphs: &TextGlyphs) {
        let size = rounded(glyphs.size());
        content.begin_text();
        let mut current = None;
        for run in glyphs.runs() {
            let Some((index, font)) = self.get(run) else {
                continue;
            };
            if current != Some(index) {
                content.set_font(Name(font_name(index).as_bytes()), size);
                current = Some(index);
            }
            show_run(content, font, run.glyphs(), size);
        }
        content.end_text();
    }

    /// Writes every embedded font into `chunk` and returns the resource name and object of
    /// each, in first-use order.
    pub(super) fn write(&self, chunk: &mut Chunk, refs: &mut Refs) -> Vec<(String, Ref)> {
        self.fonts
            .iter()
            .enumerate()
            .filter_map(|(index, font)| {
                let font = font.as_ref()?;
                Some((font_name(index), write_font(chunk, refs, font)))
            })
            .collect()
    }
}

/// The character collection of `ToUnicode` maps.
const UCS: SystemInfo<'static> = SystemInfo {
    registry: Str(b"Adobe"),
    ordering: Str(b"UCS"),
    supplement: 0,
};

/// A piece of a `TJ` array: glyph codes, or a move in thousandths of the font size, to the
/// left when positive.
#[derive(Debug, Clone, PartialEq)]
enum Piece {
    Codes(Vec<u8>),
    Move(f32),
}

/// Glyphs on one baseline offset: a text matrix at the first glyph, then one `TJ` array.
#[derive(Debug, Clone, PartialEq)]
struct TextLine {
    x: f32,
    y: f32,
    pieces: Vec<Piece>,
}

/// Shows `glyphs` of one run in `font` at `size`.
fn show_run(content: &mut Content, font: &Embedded, glyphs: &[PlacedGlyph], size: f32) {
    for line in lines(font, glyphs, size) {
        content.set_text_matrix([1.0, 0.0, 0.0, 1.0, line.x, line.y]);
        let mut shown = content.show_positioned();
        let mut items = shown.items();
        for piece in &line.pieces {
            match piece {
                Piece::Codes(codes) => {
                    items.show(Str(codes));
                }
                Piece::Move(amount) => {
                    items.adjust(*amount);
                }
            }
        }
    }
}

/// The lines that show `glyphs` where the shaper put them, in text space (y up). A new line
/// starts at each change of baseline offset; within a line, each glyph is moved from where the
/// previous glyph's width left the pen to its own position.
fn lines(font: &Embedded, glyphs: &[PlacedGlyph], size: f32) -> Vec<TextLine> {
    let mut lines: Vec<TextLine> = Vec::new();
    // Where the previous glyph's width leaves the pen.
    let mut pen = 0.0_f32;
    for glyph in glyphs {
        let (x, y) = (glyph.x, rounded(-glyph.y));
        if lines.last().is_none_or(|line| line.y != y) {
            lines.push(TextLine {
                x: rounded(x),
                y,
                pieces: Vec::new(),
            });
            pen = x;
        }
        let Some(line) = lines.last_mut() else {
            continue;
        };
        // Where this glyph lands, which differs from `x` only by the rounding of the move.
        let shift = rounded((pen - x) * 1000.0 / size);
        if shift != 0.0 {
            line.pieces.push(Piece::Move(shift));
        }
        let placed = pen - shift * size / 1000.0;
        let cid = font.cid(glyph.id);
        match line.pieces.last_mut() {
            Some(Piece::Codes(codes)) => codes.extend(cid.to_be_bytes()),
            _ => line.pieces.push(Piece::Codes(cid.to_be_bytes().to_vec())),
        }
        pen = placed + font.width(cid) * size / 1000.0;
    }
    lines
}

/// Writes `font` as a Type 0 font with its CID font, descriptor, program and `ToUnicode` map,
/// and returns the Type 0 font object.
fn write_font(chunk: &mut Chunk, refs: &mut Refs, font: &Embedded) -> Ref {
    let (type0, cid_font, descriptor, program, to_unicode) = (
        refs.next(),
        refs.next(),
        refs.next(),
        refs.next(),
        refs.next(),
    );
    let base_font = Name(font.base_font.as_bytes());
    let system = SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };

    chunk
        .type0_font(type0)
        .base_font(base_font)
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid_font)
        .to_unicode(to_unicode);

    let mut cid = chunk.cid_font(cid_font);
    cid.subtype(match font.outlines {
        Outlines::TrueType => CidFontType::Type2,
        Outlines::Cff => CidFontType::Type0,
    })
    .base_font(base_font)
    .system_info(system)
    .font_descriptor(descriptor);
    if font.outlines == Outlines::TrueType {
        cid.cid_to_gid_map_predefined(Name(b"Identity"));
    }
    cid.widths().consecutive(0, font.widths.iter().copied());
    cid.finish();

    let d = &font.descriptor;
    // Symbolic: the glyphs are addressed by id, not by a standard Latin encoding.
    let mut flags = FontFlags::SYMBOLIC;
    if d.fixed_pitch {
        flags |= FontFlags::FIXED_PITCH;
    }
    if d.italic {
        flags |= FontFlags::ITALIC;
    }
    let mut desc = chunk.font_descriptor(descriptor);
    desc.name(base_font)
        .flags(flags)
        .bbox(PdfRect::new(d.bbox[0], d.bbox[1], d.bbox[2], d.bbox[3]))
        .italic_angle(d.italic_angle)
        .ascent(d.ascent)
        .descent(d.descent)
        .cap_height(d.cap_height)
        .stem_v(d.stem_v);
    match font.outlines {
        Outlines::TrueType => desc.font_file2(program),
        Outlines::Cff => desc.font_file3(program),
    };
    desc.finish();

    let compressed = compress(&font.program);
    let mut stream = chunk.stream(program, &compressed);
    stream.filter(Filter::FlateDecode);
    match font.outlines {
        Outlines::TrueType => {
            stream.pair(
                Name(b"Length1"),
                i32::try_from(font.program.len()).unwrap_or(i32::MAX),
            );
        }
        Outlines::Cff => {
            stream.pair(Name(b"Subtype"), Name(b"CIDFontType0C"));
        }
    }
    stream.finish();

    let mut cmap = UnicodeCmap::new(Name(b"Adobe-Identity-UCS"), UCS);
    for (cid, text) in &font.texts {
        cmap.pair_with_multiple(*cid, text.chars());
    }
    let cmap = compress(&cmap.finish());
    let mut stream = chunk.cmap(to_unicode, &cmap);
    stream.name(Name(b"Adobe-Identity-UCS")).system_info(UCS);
    stream.filter(Filter::FlateDecode);
    stream.finish();
    type0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;
    use crate::fonts::{Font, Weight};
    use crate::geometry::Point;
    use crate::scene::{Item, Scene};
    use crate::text::{FaceId, FontLibrary, TextStyle};

    const INK: Rgba = Rgba {
        r: 10,
        g: 10,
        b: 10,
        a: 255,
    };

    /// `text` in Inter at 12 pt, as a scene item.
    fn item(text: &str) -> TextItem {
        let fonts = FontLibrary::global()
            .with_figure_fonts(&[Font::INTER])
            .unwrap();
        let style = TextStyle {
            face: FaceId::new(0),
            weight: Weight::Regular,
            size: 12.0,
            tabular_figures: false,
        };
        let mut scene = Scene::new(300.0, 40.0, INK);
        scene
            .push_text(&fonts, text, style, Point { x: 4.0, y: 30.0 }, INK)
            .unwrap();
        match scene.items.pop() {
            Some(Item::Text(item)) => item,
            other => panic!("expected text, got {other:?}"),
        }
    }

    fn embed(items: &[TextItem]) -> Fonts<'_> {
        let mut set = FontSet::default();
        for item in items {
            set.collect(item);
        }
        set.embed()
    }

    #[test]
    fn lines_put_every_glyph_where_the_shaper_did() {
        for text in ["AVATAR Wolf", "Tokyo 1,234.5", "office ffi"] {
            let items = [item(text)];
            let fonts = embed(&items);
            for run in items[0].glyphs.runs() {
                let (_, font) = fonts.get(run).unwrap();
                let mut placed = Vec::new();
                for line in lines(font, run.glyphs(), 12.0) {
                    let mut pen = line.x;
                    for piece in &line.pieces {
                        match piece {
                            Piece::Codes(codes) => {
                                for pair in codes.chunks(2) {
                                    let cid = u16::from_be_bytes([pair[0], pair[1]]);
                                    placed.push((pen, line.y, cid));
                                    pen += font.width(cid) * 12.0 / 1000.0;
                                }
                            }
                            Piece::Move(amount) => pen -= amount * 12.0 / 1000.0,
                        }
                    }
                }
                assert_eq!(placed.len(), run.glyphs().len(), "{text}");
                for (&(x, y, cid), glyph) in placed.iter().zip(run.glyphs()) {
                    assert!((x - glyph.x).abs() < 0.002, "{text}: {x} vs {}", glyph.x);
                    assert_eq!(y, rounded(-glyph.y));
                    assert_eq!(cid, font.cid(glyph.id));
                }
            }
        }
    }

    #[test]
    fn kerned_pairs_move_the_pen() {
        let items = [item("AVAVAV")];
        let fonts = embed(&items);
        let run = &items[0].glyphs.runs()[0];
        let (_, font) = fonts.get(run).unwrap();
        let lines = lines(font, run.glyphs(), 12.0);
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0]
                .pieces
                .iter()
                .any(|p| matches!(p, Piece::Move(m) if *m > 0.0)),
            "{:?}",
            lines[0].pieces
        );
    }

    #[test]
    fn character_ids_follow_first_use_after_notdef() {
        let items = [item("baab")];
        let fonts = embed(&items);
        let run = &items[0].glyphs.runs()[0];
        let (index, font) = fonts.get(run).unwrap();
        assert_eq!(index, 0);
        let cids: Vec<u16> = run.glyphs().iter().map(|g| font.cid(g.id)).collect();
        assert_eq!(cids, [1, 2, 2, 1]);
        assert_eq!(font.widths.len(), 3, ".notdef, b and a");
        assert_eq!(font.texts, [(1, "b".to_owned()), (2, "a".to_owned())]);
        assert_eq!(font.outlines, Outlines::TrueType);
        let (tag, name) = font.base_font.split_once('+').unwrap();
        assert_eq!(tag.len(), 6);
        assert!(name.starts_with("Inter"), "{}", font.base_font);
    }

    #[test]
    fn items_share_the_subset_of_their_font() {
        let items = [item("North"), item("South")];
        let fonts = embed(&items);
        assert_eq!(fonts.fonts.len(), 1);
        let font = fonts.fonts[0].as_ref().unwrap();
        // .notdef, N o r t h S u.
        assert_eq!(font.widths.len(), 8);
    }

    #[test]
    fn text_in_a_font_that_was_not_embedded_cannot_be_shown() {
        let items = [item("Signal")];
        let mut fonts = embed(&items);
        assert!(fonts.can_show(&items[0]));
        fonts.fonts[0] = None;
        assert!(!fonts.can_show(&items[0]));
        let mut chunk = Chunk::new();
        assert!(fonts.write(&mut chunk, &mut Refs::new()).is_empty());
    }

    #[test]
    fn invisible_text_adds_no_font() {
        let mut hidden = item("Hidden");
        hidden.color = Rgba { a: 0, ..INK };
        let items = [hidden];
        assert!(embed(&items).fonts.is_empty());
    }

    #[test]
    fn names_keep_only_plain_characters() {
        let data = crate::fonts::font_bytes(crate::BundledFont::Geist, Weight::SemiBold);
        let font = FontRef::new(data).unwrap();
        let name = postscript_name(&font);
        assert!(name.starts_with("Geist"), "{name}");
        assert!(
            name.chars()
                .all(|c| c.is_ascii_graphic() && !"()<>[]{}/%#".contains(c))
        );
    }
}
