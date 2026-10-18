"""Build the outline-format test fixtures of umber-core: CFF, CFF2 and variable fonts.

Run from the repository root:

    uv run --with fonttools==4.66.1 python scripts/fonts/build_outline_fixtures.py

The fonts are drawn here from simple polygons, so they carry no third-party design. Each maps
three private-use code points that no bundled font maps, so text using them is drawn by these
fonts when they are given as fallback fonts:

- ``outline-cff.otf``: CFF outlines (an OpenType ``CFF `` table), U+E001 to U+E003.
- ``outline-variable.ttf``: TrueType outlines with a ``wght`` axis (100 to 900, default 400)
  whose ``gvar`` deltas widen every glyph at the heavy end, U+E004 to U+E006.
- ``outline-cff2.otf``: CFF2 outlines (an OpenType ``CFF2`` table), U+E007 to U+E009.

The output is reproducible: fonttools is pinned and the timestamps are fixed, so two runs
produce byte-identical files.
"""

from __future__ import annotations

import io
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.basePen import AbstractPen
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib.tables.TupleVariation import TupleVariation

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "crates" / "umber-core" / "tests" / "fixtures"

UNITS_PER_EM = 1000
ADVANCE = 600
# 2001-01-01 in seconds since 1904-01-01, the epoch of the `head` table.
TIMESTAMP = 3_061_152_000

Polygon = list[tuple[int, int]]

# Each glyph is a list of closed polygons in font units.
SHAPES: dict[str, list[Polygon]] = {
    "square": [[(80, 0), (520, 0), (520, 440), (80, 440)]],
    "triangle": [[(60, 0), (540, 0), (300, 560)]],
    "diamond": [[(300, -60), (540, 280), (300, 620), (60, 280)]],
    "bar": [[(220, 0), (380, 0), (380, 700), (220, 700)]],
    "wedge": [[(60, 0), (540, 0), (540, 520)]],
    "frame": [
        [(60, 0), (540, 0), (540, 600), (60, 600)],
        [(140, 80), (140, 520), (460, 520), (460, 80)],
    ],
}
NOTDEF: list[Polygon] = [
    [(50, 0), (550, 0), (550, 700), (50, 700)],
    [(100, 50), (100, 650), (500, 650), (500, 50)],
]


def draw(pen: AbstractPen, polygons: list[Polygon]) -> None:
    """Draw closed polygons with a fontTools pen."""
    for polygon in polygons:
        first, *rest = polygon
        pen.moveTo(first)
        for point in rest:
            pen.lineTo(point)
        pen.closePath()


def outlines(glyphs: list[str]) -> list[tuple[str, list[Polygon]]]:
    """The polygons of `.notdef`, the space and every glyph of `glyphs`, in glyph order."""
    return [(".notdef", NOTDEF), ("space", []), *((g, SHAPES[g]) for g in glyphs)]


def builder(glyphs: list[str], codepoints: dict[int, str], cff: bool) -> FontBuilder:
    """A font builder with the glyph order, cmap and metrics of every glyph."""
    fb = FontBuilder(UNITS_PER_EM, isTTF=not cff)
    fb.setupGlyphOrder([".notdef", "space", *glyphs])
    fb.setupCharacterMap({0x20: "space", **codepoints})
    fb.setupHorizontalMetrics({name: (ADVANCE, 0) for name in [".notdef", "space", *glyphs]})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    fb.setupPost()
    return fb


def name_table(fb: FontBuilder, family: str) -> None:
    """The family, style and PostScript names, plus a short copyright notice."""
    fb.setupNameTable(
        {
            "familyName": family,
            "styleName": "Regular",
            "psName": family.replace(" ", "") + "-Regular",
            "copyright": "Test fixture of umber, drawn from simple polygons.",
        }
    )


def finish(fb: FontBuilder) -> bytes:
    """Serialize with fixed timestamps."""
    fb.setupHead(unitsPerEm=UNITS_PER_EM, created=TIMESTAMP, modified=TIMESTAMP)
    fb.font.recalcTimestamp = False
    out = io.BytesIO()
    fb.save(out)
    return out.getvalue()


def cff_font() -> bytes:
    """Three glyphs drawn with CFF outlines."""
    glyphs = ["square", "triangle", "diamond"]
    fb = builder(glyphs, {0xE001: "square", 0xE002: "triangle", 0xE003: "diamond"}, cff=True)
    charstrings = {}
    for name, polygons in outlines(glyphs):
        pen = T2CharStringPen(ADVANCE, None)
        draw(pen, polygons)
        charstrings[name] = pen.getCharString()
    fb.setupCFF("UmberTestCFF-Regular", {"FullName": "Umber Test CFF"}, charstrings, {})
    name_table(fb, "Umber Test CFF")
    return finish(fb)


def cff2_font() -> bytes:
    """Three glyphs drawn with CFF2 outlines."""
    glyphs = ["square", "triangle", "frame"]
    fb = builder(glyphs, {0xE007: "square", 0xE008: "triangle", 0xE009: "frame"}, cff=True)
    charstrings = {}
    for name, polygons in outlines(glyphs):
        pen = T2CharStringPen(None, None, CFF2=True)
        draw(pen, polygons)
        charstrings[name] = pen.getCharString()
    fb.setupCFF2(charstrings)
    name_table(fb, "Umber Test CFF2")
    return finish(fb)


def variable_font() -> bytes:
    """Three glyphs drawn with TrueType outlines, widened by a weight axis."""
    glyphs = ["bar", "wedge", "frame"]
    fb = builder(glyphs, {0xE004: "bar", 0xE005: "wedge", 0xE006: "frame"}, cff=False)
    drawn = {}
    for name, polygons in outlines(glyphs):
        pen = TTGlyphPen(None)
        draw(pen, polygons)
        drawn[name] = pen.glyph()
    fb.setupGlyf(drawn)
    name_table(fb, "Umber Test Variable")
    fb.setupFvar(axes=[("wght", 100, 400, 900, "Weight")], instances=[])
    variations = {}
    for name in glyphs:
        points = [point for polygon in SHAPES[name] for point in polygon]
        # Move every point away from the glyph's vertical centre line at the heavy end, plus
        # the four phantom points, which stay put.
        deltas = [(60 if x > 300 else -60, 0) for x, _ in points] + [(0, 0)] * 4
        variations[name] = [TupleVariation({"wght": (0.0, 1.0, 1.0)}, deltas)]
    fb.setupGvar(variations)
    return finish(fb)


def main() -> None:
    """Write every fixture."""
    FIXTURES.mkdir(parents=True, exist_ok=True)
    for file, data in [
        ("outline-cff.otf", cff_font()),
        ("outline-variable.ttf", variable_font()),
        ("outline-cff2.otf", cff2_font()),
    ]:
        (FIXTURES / file).write_bytes(data)
        print(f"{file}: {len(data):,} bytes")


if __name__ == "__main__":
    main()
