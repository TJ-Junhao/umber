"""Build ``tests/fonts/UmberFixtureSerif-Variable.ttf`` from Source Serif 4 Variable.

Run from the repository root:

    uv run --with fonttools==4.66.1 python scripts/fonts/build_variable_fixture.py

The source is ``tests/fonts/SourceSerif4Variable-Roman.ttf`` (Source Serif 4 release
4.005R, SIL Open Font License 1.1). The fixture is a variable font whose default instance is
not the regular weight, for the tests of named instances:

- the optical size axis is pinned at its default, 20, which leaves a ``wght`` axis only;
- the default weight moves from 400 to 300, the Light instance;
- the named instances are ExtraLight 200, Light 300, Regular 400, Bold 700 and Black 900:
  Semibold 600 is dropped, so the bold instance is Bold 700;
- it is subset to Basic Latin, Latin-1 Supplement and a few punctuation marks, with the
  ``kern``, ``liga`` and ``tnum`` features and no hinting;
- every name that holds "Source" is renamed to "Umber Fixture Serif", as the license's
  Reserved Font Name requires of a modified version; the copyright notice is kept.

The output is reproducible: fonttools is pinned and the timestamps are fixed, so two runs
produce byte-identical files.
"""

from __future__ import annotations

import io
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "tests" / "fonts" / "SourceSerif4Variable-Roman.ttf"
TARGET = ROOT / "tests" / "fonts" / "UmberFixtureSerif-Variable.ttf"

FAMILY = "Umber Fixture Serif"
# 2001-01-01 in seconds since 1904-01-01, the epoch of the `head` table.
TIMESTAMP = 3_061_152_000
CODEPOINTS = [
    *range(0x20, 0x7F),
    *range(0xA0, 0x100),
    0x2013,
    0x2014,
    0x2018,
    0x2019,
    0x201C,
    0x201D,
    0x2022,
    0x2026,
    0x2212,
]


def rename(font: TTFont) -> None:
    """Replace the Reserved Font Name in every name except the copyright and trademark."""
    for record in font["name"].names:
        if record.nameID in (0, 7):
            continue
        text = record.toUnicode()
        renamed = text.replace("Source Serif 4", FAMILY).replace(
            "SourceSerif4", FAMILY.replace(" ", "")
        )
        if renamed != text:
            record.string = renamed


def main() -> None:
    """Write the fixture."""
    font = TTFont(SOURCE)
    font = instancer.instantiateVariableFont(font, {"opsz": 20, "wght": (200, 300, 900)})
    fvar = font["fvar"]
    fvar.instances = [i for i in fvar.instances if i.coordinates["wght"] != 600]
    options = subset.Options()
    options.layout_features = ["kern", "liga", "tnum"]
    options.hinting = False
    options.name_IDs = ["*"]
    options.name_languages = ["*"]
    options.notdef_outline = True
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=CODEPOINTS)
    subsetter.subset(font)
    rename(font)
    font["head"].created = TIMESTAMP
    font["head"].modified = TIMESTAMP
    font.recalcTimestamp = False
    out = io.BytesIO()
    font.save(out)
    data = out.getvalue()
    TARGET.write_bytes(data)
    print(f"{TARGET.name}: {len(data):,} bytes")


if __name__ == "__main__":
    main()
