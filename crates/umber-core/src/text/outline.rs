//! Outline pens: drawing glyph outlines into paths, and testing whether a glyph draws anything.

use read_fonts::types::GlyphId;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlineGlyph, OutlineGlyphCollection, OutlinePen};

use crate::geometry::{Path, Point};

/// Unhinted outlines in font units, at the default location.
pub(super) fn unhinted() -> DrawSettings<'static> {
    DrawSettings::unhinted(Size::unscaled(), LocationRef::default())
}

/// Whether `outline` draws without error and with at least one path command.
pub(super) fn draws_segments(outline: &OutlineGlyph<'_>) -> bool {
    let mut pen = SegmentCount(0);
    outline.draw(unhinted(), &mut pen).is_ok() && pen.0 > 0
}

/// Appends the outline of `glyph` to `path`, mapping font units to points with `scale` and
/// putting the glyph origin at `(dx, dy)`, y-down. A glyph that is missing, or whose outline
/// fails part-way, contributes nothing rather than a broken shape.
pub(super) fn append_glyph(
    outlines: &OutlineGlyphCollection<'_>,
    glyph: GlyphId,
    scale: f32,
    dx: f32,
    dy: f32,
    path: &mut Path,
) {
    let mut builder = PathBuilder {
        path: Path::new(),
        scale,
        dx,
        dy,
    };
    let drawn = outlines
        .get(glyph)
        .is_some_and(|outline| outline.draw(unhinted(), &mut builder).is_ok());
    if drawn {
        path.extend(&builder.path);
    }
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
