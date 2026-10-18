//! Shaping: the shaper's font callbacks, the OpenType features applied to every run, and a
//! shaper that reuses shape plans across the runs of one font.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use harfrust::{
    Buffer, BufferFlags, Direction, Feature, FontFuncs, GlyphExtents, Script, ShapeOptions,
    ShapePlan, ShaperFont, Tag,
};
use read_fonts::types::GlyphId;

use super::font::{Face, ShapingFont};

/// The most glyph extents computed from system font files in one layout call; see
/// [`SystemExtents`].
pub(super) const EXTENTS_PER_LAYOUT: usize = 4096;

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
pub(super) struct Funcs<'f, 'a> {
    face: &'f Face<'a>,
    extents: Option<SystemExtents<'f>>,
}

impl<'f, 'a> Funcs<'f, 'a> {
    /// Callbacks for a bundled or fallback font, which owns its outline tables.
    pub(super) fn new(face: &'f Face<'a>) -> Self {
        Funcs {
            face,
            extents: None,
        }
    }

    /// Callbacks for a system font: glyph bounds come from `face`, spending `budget`.
    pub(super) fn system(face: &'f Face<'a>, budget: &'f Cell<usize>) -> Self {
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
            .or_else(|| self.face.best_subtable_glyph(c))
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

/// One shaped glyph, in font units.
#[derive(Debug, Clone, Copy)]
pub(super) struct ShapedGlyph {
    pub(super) id: u16,
    pub(super) advance: i32,
    pub(super) x_offset: i32,
    pub(super) y_offset: i32,
    /// The byte offset in the run's text where the characters this glyph stands for start.
    /// Glyphs that share a cluster stand for the same characters together.
    pub(super) cluster: u32,
}

/// The shaped glyphs of one run, with the font's units per em and the text they were shaped
/// from.
pub(super) struct ShapedRun {
    pub(super) units_per_em: i32,
    pub(super) glyphs: Vec<ShapedGlyph>,
    pub(super) text: String,
}

/// Shapes several runs of text with one font, reusing shape plans across runs.
pub(super) struct Shaper<'f> {
    font: &'f ShapingFont,
    shaper: ShaperFont<'f, 'f>,
    features: &'f [Feature],
    plans: Vec<(Direction, Option<Script>, ShapePlan)>,
}

impl<'f> Shaper<'f> {
    pub(super) fn new(
        font: &'f ShapingFont,
        funcs: &'f dyn FontFuncs,
        features: &'f [Feature],
    ) -> Self {
        Shaper {
            font,
            shaper: ShaperFont::new(font).with_font_funcs(Some(funcs)),
            features,
            plans: Vec::new(),
        }
    }

    /// Shapes `text`; `None` if the shaper refuses the buffer.
    pub(super) fn shape(&mut self, text: &str, preserve_ignorables: bool) -> Option<ShapedRun> {
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
                cluster: info.cluster,
            })
            .collect();
        Some(ShapedRun {
            units_per_em: i32::from(self.font.units_per_em()),
            glyphs,
            text: text.to_owned(),
        })
    }
}

/// The OpenType features applied to every run.
pub(super) fn features(tabular_figures: bool) -> Vec<Feature> {
    let mut features = vec![
        Feature::new(Tag::new(b"kern"), 1, ..),
        Feature::new(Tag::new(b"liga"), 1, ..),
    ];
    if tabular_figures {
        features.push(Feature::new(Tag::new(b"tnum"), 1, ..));
    }
    features
}
