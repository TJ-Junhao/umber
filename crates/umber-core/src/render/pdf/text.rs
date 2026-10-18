//! Text as text: the fonts of a page, subset to the glyphs its text items were shaped with and
//! embedded as CID fonts with a `ToUnicode` map, and the operators that show the glyphs.
//!
//! A page's fonts are collected from its text items first ([`FontSet`]), so that each font is
//! subset once, with exactly the glyphs drawn, before the content refers to it ([`Fonts`]). A
//! font is one font program (the same bytes and face), wherever its runs came from. Glyph ids
//! in the subset follow the order the glyphs are first used, `.notdef` first, and text is
//! encoded `Identity-H`, two bytes per character code ([`Codes`]).
//!
//! A glyph can stand for different text in different places: `.notdef` for different missing
//! characters, or a base letter alone and with a mark. In a TrueType font each glyph standing
//! for each text has a character code of its own, mapped to its glyph by a `/CIDToGIDMap`
//! stream, so the `ToUnicode` map alone reads every occurrence as written, in every reader. A
//! glyph the shaper moved off the pen, such as a mark over its letter, gets a code whose
//! advance brings the pen back to where the shaper left it, and the next glyph that is not a
//! space starts a line of its own, so readers that place characters by their glyphs (MuPDF,
//! pdf.js) see no gap and no overlap around it. A CFF font ties codes to glyphs one to one
//! (codes are glyph ids in the subset), so its map gives each glyph the text it first stood
//! for; a cluster the map would misread or leave a code of unmapped, and any cluster of
//! several glyphs, is marked with its text as `/ActualText`. pdf.js does not read
//! `/ActualText`, so it extracts such a CFF cluster as the map reads it. Glyphs of a run whose
//! font could not be embedded are drawn as outlines and are not extracted.
//!
//! Text smaller than [`MIN_TEXT_SIZE`] is not written: the document's precision cannot place
//! it, and no rasterizer draws it.

use std::collections::{HashMap, HashSet};

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Chunk, Content, Filter, Finish, Name, Rect as PdfRect, Ref, Str, TextStr};
use read_fonts::types::{GlyphId, Tag};
use read_fonts::{FontRef, TableProvider};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::string::StringId;
use subsetter::GlyphRemapper;

use super::super::normalize::text_is_visible;
use super::write::{Refs, compress, rounded, subset_tag};
use crate::scene::TextItem;
use crate::text::{GlyphRun, PlacedGlyph, ProgramId, RunFont};

/// The most characters a `ToUnicode` entry maps one glyph to; longer clusters keep their first
/// characters. PDF readers accept at most 512 bytes per entry.
const MAX_GLYPH_CHARS: usize = 32;

/// The smallest font size, in points, that text is written at: 0.001 pt, the precision the
/// document writes positions in, is 0.03 device pixels at the highest PNG resolution.
pub(super) const MIN_TEXT_SIZE: f32 = 0.001;

/// The fonts of a page's text and the glyphs used of each, collected before anything is
/// written.
#[derive(Default)]
pub(super) struct FontSet<'s> {
    fonts: Vec<FontUse<'s>>,
    index: HashMap<ProgramId, usize>,
}

/// One font of a [`FontSet`]: the glyphs used, in first-use order after `.notdef`, and each
/// glyph with each text it stands for somewhere, in first-use order.
struct FontUse<'s> {
    font: RunFont<'s>,
    glyphs: GlyphRemapper,
    occurrences: Vec<Occurrence>,
    seen: HashSet<Occurrence>,
}

/// A glyph standing for a text, at most [`MAX_GLYPH_CHARS`] characters of it, with the advance
/// it is shown with.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Occurrence {
    glyph: u16,
    text: String,
    advance: Advance,
}

/// The advance a character code of a TrueType font is shown with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Advance {
    /// The glyph's own advance in the font.
    Font,
    /// An advance in glyph space (thousandths of the font size) that brings the pen back to
    /// where the shaper left it, for a glyph the shaper moved off the pen, such as a mark over
    /// its letter. Readers that place characters by their glyphs would otherwise see a gap
    /// after the mark and extract a space.
    Units(i32),
}

/// The occurrence of `glyph`, standing for `text`, in text of `size` points.
fn occurrence(glyph: &PlacedGlyph, text: &str, size: f32) -> Occurrence {
    let advance = if glyph.offset == 0.0 {
        Advance::Font
    } else {
        Advance::Units(((glyph.advance - glyph.offset) * 1000.0 / size).round() as i32)
    };
    Occurrence {
        glyph: glyph.id,
        text: text.chars().take(MAX_GLYPH_CHARS).collect(),
        advance,
    }
}

/// Whether a text item is written as text: it is visible, has glyphs, and is at least
/// [`MIN_TEXT_SIZE`].
pub(super) fn written(item: &TextItem) -> bool {
    text_is_visible(item.color, item.origin)
        && !item.glyphs.is_empty()
        && item.glyphs.size() >= MIN_TEXT_SIZE
}

impl<'s> FontSet<'s> {
    /// Adds the fonts and glyphs of `item`, if it is drawn as text at all.
    pub(super) fn collect(&mut self, item: &'s TextItem) {
        if !written(item) {
            return;
        }
        for run in item.glyphs.runs() {
            let font = run.run_font();
            // A font that cannot be read is not embedded; its run is drawn as outlines.
            let Some(program) = font.program() else {
                continue;
            };
            let slot = *self.index.entry(program).or_insert_with(|| {
                self.fonts.push(FontUse {
                    font,
                    glyphs: GlyphRemapper::new(),
                    occurrences: Vec::new(),
                    seen: HashSet::new(),
                });
                self.fonts.len() - 1
            });
            if let Some(used) = self.fonts.get_mut(slot) {
                used.add(run, item.glyphs.size());
            }
        }
    }

    /// Subsets every font to its glyphs. A font that cannot be read again or subset is left
    /// out; the runs set in it are drawn as outlines.
    pub(super) fn embed(self) -> Fonts {
        Fonts {
            fonts: self.fonts.into_iter().map(FontUse::embed).collect(),
            index: self.index,
        }
    }
}

impl FontUse<'_> {
    fn add(&mut self, run: &GlyphRun, size: f32) {
        for (glyph, text) in run.glyphs().iter().zip(run.glyph_texts()) {
            // A font has at most 65,535 glyphs, so a full remapper means a malformed run.
            if self.glyphs.num_gids() == u16::MAX {
                return;
            }
            self.glyphs.remap(glyph.id);
            let occurrence = occurrence(glyph, text, size);
            if self.seen.insert(occurrence.clone()) {
                self.occurrences.push(occurrence);
            }
        }
    }

    /// The embedded subset, or `None` when the font cannot be read again or subset.
    fn embed(self) -> Option<Embedded> {
        let FontUse {
            font,
            glyphs,
            occurrences,
            ..
        } = self;
        font.with_data(|data, index| Embedded::new(data, index, glyphs, &occurrences))
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
    /// Each glyph's id in the subset.
    glyphs: GlyphRemapper,
    /// The character codes of the glyphs.
    codes: Codes,
    /// The advance of each character code, in glyph space.
    widths: Vec<f32>,
    descriptor: Descriptor,
}

/// How the glyphs of an embedded font are given character codes (CIDs).
#[derive(Debug)]
enum Codes {
    /// One code for each glyph and each text it stands for, mapped to its glyph by a
    /// `/CIDToGIDMap`, so the `ToUnicode` map reads every occurrence as written. Code 0 is
    /// `.notdef` standing for nothing. Used for TrueType outlines.
    Occurrences {
        codes: HashMap<Occurrence, u16>,
        /// The advance of each code.
        advances: Vec<Advance>,
        /// The subset glyph id of each code.
        glyphs: Vec<u16>,
        /// The text of each code, empty for none.
        texts: Vec<String>,
    },
    /// One code for each glyph, its id in the subset, mapped to the text it first stood for.
    /// Used for CFF outlines, whose subset ties codes to glyphs one to one, and where there
    /// would be more occurrences than codes.
    Glyphs {
        /// The text of each code, empty for none.
        texts: Vec<String>,
    },
}

impl Codes {
    /// Codes for `occurrences` of the glyphs of `glyphs`, one per occurrence when `outlines`
    /// allow it.
    fn new(outlines: Outlines, glyphs: &GlyphRemapper, occurrences: &[Occurrence]) -> Codes {
        let subset = |glyph: u16| glyphs.get(glyph).unwrap_or(0);
        if outlines == Outlines::TrueType && occurrences.len() < usize::from(u16::MAX) {
            let notdef = Occurrence {
                glyph: 0,
                text: String::new(),
                advance: Advance::Font,
            };
            let mut codes = HashMap::from([(notdef, 0)]);
            let (mut subset_glyphs, mut texts, mut advances) =
                (vec![0], vec![String::new()], vec![Advance::Font]);
            for occurrence in occurrences {
                if codes.contains_key(occurrence) {
                    continue;
                }
                codes.insert(
                    occurrence.clone(),
                    u16::try_from(subset_glyphs.len()).unwrap_or(0),
                );
                subset_glyphs.push(subset(occurrence.glyph));
                texts.push(occurrence.text.clone());
                advances.push(occurrence.advance);
            }
            Codes::Occurrences {
                codes,
                advances,
                glyphs: subset_glyphs,
                texts,
            }
        } else {
            let mut texts = vec![String::new(); usize::from(glyphs.num_gids())];
            for occurrence in occurrences {
                if let Some(slot) = texts.get_mut(usize::from(subset(occurrence.glyph)))
                    && slot.is_empty()
                {
                    slot.clone_from(&occurrence.text);
                }
            }
            Codes::Glyphs { texts }
        }
    }

    /// The text of code `code`, empty for none.
    fn text(&self, code: u16) -> &str {
        let texts = match self {
            Codes::Occurrences { texts, .. } | Codes::Glyphs { texts } => texts,
        };
        texts.get(usize::from(code)).map_or("", String::as_str)
    }

    /// The number of codes.
    fn len(&self) -> usize {
        match self {
            Codes::Occurrences { texts, .. } | Codes::Glyphs { texts } => texts.len(),
        }
    }
}

impl Embedded {
    /// The subset of face `index` of `data` holding `glyphs`, which stood for the texts of
    /// `occurrences`.
    fn new(
        data: &[u8],
        index: u32,
        glyphs: GlyphRemapper,
        occurrences: &[Occurrence],
    ) -> Option<Embedded> {
        let font = FontRef::from_index(data, index).ok()?;
        let metrics = font.metrics(Size::unscaled(), LocationRef::default());
        let per_unit = 1000.0 / f32::from(metrics.units_per_em.max(1));
        let advances = font.glyph_metrics(Size::unscaled(), LocationRef::default());
        let glyph_widths: Vec<f32> = glyphs
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

        // At the default instance: CFF2 outlines become TrueType outlines there.
        let subset = subsetter::subset_with_variations(data, index, &[], &glyphs).ok()?;
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
        let codes = Codes::new(outlines, &glyphs, occurrences);
        let glyph_width = |glyph: u16| glyph_widths.get(usize::from(glyph)).copied().unwrap_or(0.0);
        let widths = (0..codes.len())
            .map(|code| match &codes {
                Codes::Occurrences {
                    glyphs, advances, ..
                } => match advances.get(code) {
                    Some(Advance::Units(units)) => *units as f32,
                    Some(Advance::Font) | None => {
                        glyph_width(glyphs.get(code).copied().unwrap_or(0))
                    }
                },
                Codes::Glyphs { .. } => glyph_width(u16::try_from(code).unwrap_or(0)),
            })
            .collect();

        let bounds = font.head().ok().map(|head| {
            [head.x_min(), head.y_min(), head.x_max(), head.y_max()]
                .map(|v| rounded(f32::from(v) * per_unit))
        });
        // The weight of the face embedded, the one place the descriptor reads a weight.
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
            glyphs,
            codes,
            widths,
            descriptor,
        })
    }

    /// The character code of `glyph` standing for `text` in text of `size` points, which
    /// [`FontSet::collect`] added.
    fn cid(&self, glyph: &PlacedGlyph, text: &str, size: f32) -> u16 {
        match &self.codes {
            Codes::Occurrences { codes, .. } => codes
                .get(&occurrence(glyph, text, size))
                .copied()
                .unwrap_or(0),
            Codes::Glyphs { .. } => self.glyphs.get(glyph.id).unwrap_or(0),
        }
    }

    /// The advance of character code `cid` in glyph space.
    fn width(&self, cid: u16) -> f32 {
        self.widths.get(usize::from(cid)).copied().unwrap_or(0.0)
    }

    /// The text the `ToUnicode` map gives character code `cid`, empty for none.
    fn unicode(&self, cid: u16) -> &str {
        self.codes.text(cid)
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
pub(super) struct Fonts {
    /// In first-use order; `None` for a font that could not be embedded.
    fonts: Vec<Option<Embedded>>,
    index: HashMap<ProgramId, usize>,
}

#[cfg(test)]
impl Fonts {
    /// These fonts with font `index` treated as one that could not be embedded.
    pub(super) fn without(mut self, index: usize) -> Fonts {
        if let Some(font) = self.fonts.get_mut(index) {
            *font = None;
        }
        self
    }
}

/// The resource name of font `index`.
pub(super) fn font_name(index: usize) -> String {
    format!("F{index}")
}

impl Fonts {
    /// The resource index and subset of the font of `run`, when it was embedded.
    pub(super) fn get(&self, run: &GlyphRun) -> Option<(usize, &Embedded)> {
        let index = *self.index.get(&run.run_font().program()?)?;
        Some((index, self.fonts.get(index)?.as_ref()?))
    }

    /// Whether any run of `item` is shown as text: the item is [`written`] and some font
    /// of its runs was embedded.
    pub(super) fn shows_any(&self, item: &TextItem) -> bool {
        written(item) && item.glyphs.runs().iter().any(|run| self.get(run).is_some())
    }

    /// Shows the glyphs of `item` whose fonts were embedded in one text object, in a coordinate
    /// system whose origin is the left end of the baseline and whose y axis points up.
    pub(super) fn show(&self, content: &mut Content, item: &TextItem) {
        let size = item.glyphs.size();
        content.begin_text();
        let mut current = None;
        for run in item.glyphs.runs() {
            let Some((index, font)) = self.get(run) else {
                continue;
            };
            if current != Some(index) {
                content.set_font(Name(font_name(index).as_bytes()), size);
                current = Some(index);
            }
            show_run(content, &operations(font, run, size));
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

/// One step of showing a run.
#[derive(Debug, Clone, PartialEq)]
enum Operation {
    /// Start a line of glyphs at `(x, y)` in text space: a text matrix.
    Line { x: f32, y: f32 },
    /// Show glyphs and moves: one `TJ` array.
    Show(Vec<Piece>),
    /// Start marking the glyphs that follow with the text they stand for.
    BeginText(String),
    /// Stop marking.
    EndText,
}

/// Writes `operations`.
fn show_run(content: &mut Content, operations: &[Operation]) {
    for operation in operations {
        match operation {
            Operation::Line { x, y } => {
                content.set_text_matrix([1.0, 0.0, 0.0, 1.0, *x, *y]);
            }
            Operation::Show(pieces) => {
                let mut shown = content.show_positioned();
                let mut items = shown.items();
                for piece in pieces {
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
            Operation::BeginText(text) => {
                content
                    .begin_marked_content_with_properties(Name(b"Span"))
                    .properties()
                    .actual_text(TextStr(text));
            }
            Operation::EndText => {
                content.end_marked_content();
            }
        }
    }
}

/// The operations that show the glyphs of `run` in `font` at `size` where the shaper put them,
/// in text space (y up). A new line starts at each change of baseline offset; within a line,
/// each glyph is moved from where the previous glyph's width left the pen to its own position.
/// A cluster the `ToUnicode` map would read as other text than it stands for, or leave a code
/// of unmapped, and in a font with a code per glyph a cluster of several glyphs, is marked
/// with its text.
fn operations(font: &Embedded, run: &GlyphRun, size: f32) -> Vec<Operation> {
    let mut shower = Shower {
        font,
        size,
        operations: Vec::new(),
        pieces: Vec::new(),
        line: None,
        pen: 0.0,
        after_moved: false,
    };
    let glyphs = run.glyphs();
    let texts = run.glyph_texts();
    for (span, text) in run.clusters() {
        let cluster: Vec<(&PlacedGlyph, u16, &str)> = span
            .filter_map(|i| {
                let glyph = glyphs.get(i)?;
                let glyph_text = texts.get(i).copied().unwrap_or("");
                Some((glyph, font.cid(glyph, glyph_text, size), glyph_text))
            })
            .collect();
        // An unmapped code is read as a replacement character by some readers. Without a code
        // per occurrence, a cluster of several glyphs, such as a letter and a mark placed by
        // the shaper, can be read apart or out of order by readers that place characters by
        // their glyphs.
        let mapped = (cluster.len() == 1 || matches!(font.codes, Codes::Occurrences { .. }))
            && cluster
                .iter()
                .all(|&(_, code, _)| !font.unicode(code).is_empty())
            && cluster
                .iter()
                .map(|&(_, code, _)| font.unicode(code))
                .collect::<String>()
                == text;
        if !mapped {
            shower.flush();
            shower
                .operations
                .push(Operation::BeginText(text.to_owned()));
        }
        for (glyph, code, glyph_text) in cluster {
            shower.glyph(glyph, code, glyph_text);
        }
        if !mapped {
            shower.flush();
            shower.operations.push(Operation::EndText);
        }
    }
    shower.flush();
    shower.operations
}

/// The state of [`operations`] between glyphs.
struct Shower<'f> {
    font: &'f Embedded,
    size: f32,
    operations: Vec<Operation>,
    /// The `TJ` array being built.
    pieces: Vec<Piece>,
    /// The baseline offset of the current line, once one has started.
    line: Option<f32>,
    /// Where the previous glyph's width leaves the pen.
    pen: f32,
    /// Whether a glyph was moved off the pen, such as a mark over its letter, since the last
    /// glyph that is not a space. The next such glyph then starts a line of its own: pdf.js
    /// advances a mark by nothing whatever its width, and would otherwise see that glyph out of
    /// place.
    after_moved: bool,
}

impl Shower<'_> {
    /// Shows `glyph`, which stands for `text`, as code `cid`.
    fn glyph(&mut self, glyph: &PlacedGlyph, cid: u16, text: &str) {
        let (x, y) = (glyph.x, rounded(-glyph.y));
        let space = !text.is_empty() && text.chars().all(char::is_whitespace);
        if (self.after_moved && !space) || self.line != Some(y) {
            self.flush();
            self.operations.push(Operation::Line { x: rounded(x), y });
            self.line = Some(y);
            self.pen = x;
        }
        // Where this glyph lands, which differs from `x` only by the rounding of the move.
        let shift = rounded((self.pen - x) * 1000.0 / self.size);
        if shift != 0.0 {
            self.pieces.push(Piece::Move(shift));
        }
        let placed = self.pen - shift * self.size / 1000.0;
        match self.pieces.last_mut() {
            Some(Piece::Codes(codes)) => codes.extend(cid.to_be_bytes()),
            _ => self.pieces.push(Piece::Codes(cid.to_be_bytes().to_vec())),
        }
        self.pen = placed + self.font.width(cid) * self.size / 1000.0;
        if glyph.offset != 0.0 {
            self.after_moved = true;
        } else if !space {
            self.after_moved = false;
        }
    }

    /// Ends the `TJ` array being built, if it has anything.
    fn flush(&mut self) {
        if !self.pieces.is_empty() {
            self.operations
                .push(Operation::Show(std::mem::take(&mut self.pieces)));
        }
    }
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
    let glyph_map = match (&font.codes, font.outlines) {
        (Codes::Occurrences { glyphs, .. }, _) => {
            let map = refs.next();
            cid.cid_to_gid_map_stream(map);
            Some((map, glyphs))
        }
        (Codes::Glyphs { .. }, Outlines::TrueType) => {
            cid.cid_to_gid_map_predefined(Name(b"Identity"));
            None
        }
        (Codes::Glyphs { .. }, Outlines::Cff) => None,
    };
    cid.widths().consecutive(0, font.widths.iter().copied());
    cid.finish();
    if let Some((map, glyphs)) = glyph_map {
        let bytes: Vec<u8> = glyphs
            .iter()
            .flat_map(|glyph| glyph.to_be_bytes())
            .collect();
        chunk
            .stream(map, &compress(&bytes))
            .filter(Filter::FlateDecode);
    }

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
    for code in (0..font.codes.len()).filter_map(|code| u16::try_from(code).ok()) {
        let text = font.codes.text(code);
        if !text.is_empty() {
            cmap.pair_with_multiple(code, text.chars());
        }
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
        item_in(Font::INTER, text)
    }

    /// `text` in `font` at 12 pt, as a scene item.
    fn item_in(font: Font, text: &str) -> TextItem {
        let fonts = FontLibrary::global().with_figure_fonts(&[font]).unwrap();
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

    fn embed(items: &[TextItem]) -> Fonts {
        let mut set = FontSet::default();
        for item in items {
            set.collect(item);
        }
        set.embed()
    }

    #[test]
    fn operations_put_every_glyph_where_the_shaper_did() {
        for text in ["AVATAR Wolf", "Tokyo 1,234.5", "office ffi"] {
            let items = [item(text)];
            let fonts = embed(&items);
            for run in items[0].glyphs.runs() {
                let (_, font) = fonts.get(run).unwrap();
                let mut placed = Vec::new();
                let (mut pen, mut line_y) = (0.0_f32, 0.0_f32);
                for operation in operations(font, run, 12.0) {
                    match operation {
                        Operation::Line { x, y } => (pen, line_y) = (x, y),
                        Operation::Show(pieces) => {
                            for piece in pieces {
                                match piece {
                                    Piece::Codes(codes) => {
                                        for pair in codes.chunks(2) {
                                            let cid = u16::from_be_bytes([pair[0], pair[1]]);
                                            placed.push((pen, line_y, cid));
                                            pen += font.width(cid) * 12.0 / 1000.0;
                                        }
                                    }
                                    Piece::Move(amount) => pen -= amount * 12.0 / 1000.0,
                                }
                            }
                        }
                        Operation::BeginText(_) | Operation::EndText => {}
                    }
                }
                assert_eq!(placed.len(), run.glyphs().len(), "{text}");
                let texts = run.glyph_texts();
                for ((&(x, y, cid), glyph), glyph_text) in
                    placed.iter().zip(run.glyphs()).zip(texts)
                {
                    assert!((x - glyph.x).abs() < 0.002, "{text}: {x} vs {}", glyph.x);
                    assert_eq!(y, rounded(-glyph.y));
                    assert_eq!(cid, font.cid(glyph, glyph_text, 12.0));
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
        let operations = operations(font, run, 12.0);
        let [Operation::Line { .. }, Operation::Show(pieces)] = operations.as_slice() else {
            panic!("{operations:?}");
        };
        assert!(
            pieces
                .iter()
                .any(|p| matches!(p, Piece::Move(m) if *m > 0.0)),
            "{pieces:?}"
        );
    }

    #[test]
    fn truetype_occurrences_have_codes_of_their_own() {
        // A letter alone and under a mark: the map reads both, so nothing is marked.
        let items = [item_in(Font::SOURCE_SERIF_4, "x x\u{301} x")];
        let fonts = embed(&items);
        let run = &items[0].glyphs.runs()[0];
        let (_, font) = fonts.get(run).unwrap();
        assert!(marks(font, run).is_empty());
        let mapped: String = run
            .glyphs()
            .iter()
            .zip(run.glyph_texts())
            .map(|(glyph, text)| font.unicode(font.cid(glyph, text, 12.0)))
            .collect();
        assert_eq!(mapped, "x x\u{301} x");
    }

    #[test]
    fn cff_glyphs_the_map_misreads_are_marked() {
        let library = FontLibrary::with_fallbacks(vec![
            include_bytes!("../../../tests/fixtures/outline-cff.otf").to_vec(),
        ]);
        let fonts = library.with_figure_fonts(&[Font::INTER]).unwrap();
        let style = TextStyle {
            face: FaceId::new(0),
            weight: Weight::Regular,
            size: 12.0,
            tabular_figures: false,
        };
        let mut scene = Scene::new(300.0, 40.0, INK);
        // The CFF font draws neither mark, so its .notdef stands for both.
        scene
            .push_text(
                &fonts,
                "\u{E001}\u{301}\u{E002}\u{302}",
                style,
                Point::default(),
                INK,
            )
            .unwrap();
        let Some(Item::Text(text)) = scene.items.pop() else {
            panic!("no text");
        };
        let items = [text];
        let fonts = embed(&items);
        let run = &items[0].glyphs.runs()[0];
        let (_, font) = fonts.get(run).unwrap();
        assert_eq!(marks(font, run), ["\u{E001}\u{301}", "\u{E002}\u{302}"]);
        // The map gives .notdef the first mark it stood for.
        let second = run.glyphs().iter().filter(|g| g.id == 0).nth(1).unwrap();
        assert_eq!(font.unicode(font.cid(second, "\u{302}", 12.0)), "\u{301}");
    }

    /// The texts `run` is marked with.
    fn marks(font: &Embedded, run: &GlyphRun) -> Vec<String> {
        operations(font, run, 12.0)
            .into_iter()
            .filter_map(|operation| match operation {
                Operation::BeginText(text) => Some(text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn character_ids_follow_first_use_after_notdef() {
        let items = [item("baab")];
        let fonts = embed(&items);
        let run = &items[0].glyphs.runs()[0];
        let (index, font) = fonts.get(run).unwrap();
        assert_eq!(index, 0);
        let cids: Vec<u16> = run
            .glyphs()
            .iter()
            .zip(run.glyph_texts())
            .map(|(g, text)| font.cid(g, text, 12.0))
            .collect();
        assert_eq!(cids, [1, 2, 2, 1]);
        assert_eq!(font.widths.len(), 3, ".notdef, b and a");
        let texts: Vec<&str> = (0..3).map(|code| font.unicode(code)).collect();
        assert_eq!(texts, ["", "b", "a"]);
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
        assert!(fonts.shows_any(&items[0]));
        fonts.fonts[0] = None;
        assert!(!fonts.shows_any(&items[0]));
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
