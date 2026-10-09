"""Fetch, verify and (where the license allows) subset the fonts bundled with umber-core.

Run from the repository root:

    uv run --with fonttools python scripts/fonts/build_fonts.py

Pinned release assets are downloaded into a temporary directory and checked against the SHA-256
values below; any mismatch aborts the build. Families whose license header declares a Reserved
Font Name are copied byte-for-byte (subsetting would create a Modified Version). Other families
are subset to the required code points. The script also writes the license files, the test
fixture ``omega-only.ttf`` and ``MANIFEST.md``.
"""

from __future__ import annotations

import hashlib
import io
import re
import sys
import tempfile
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[2]
CORE = ROOT / "crates" / "umber-core"
FONT_DIR = CORE / "fonts"
LICENSE_DIR = FONT_DIR / "LICENSES"
FIXTURE = CORE / "tests" / "fixtures" / "omega-only.ttf"

BUDGET = 3 * 1024 * 1024

# Inclusive ranges; keep both lists in sync with `crates/umber-core/src/fonts.rs`.
# Every bundled font must cover these (U+00AD, the soft hyphen, is deliberately excluded).
REQUIRED_CODEPOINTS = [
    (0x0020, 0x007E),
    (0x00A0, 0x00AC),
    (0x00AE, 0x00FF),
    (0x2013, 0x2014),
    (0x2018, 0x2019),
    (0x201C, 0x201D),
    (0x2022, 0x2022),
    (0x2026, 0x2026),
    (0x2030, 0x2030),
    (0x2039, 0x203A),
    (0x20AC, 0x20AC),
    (0x2212, 0x2212),
]
# Kept when subsetting if the font has them. At least one bundled font must have each of them;
# text layout falls back to the other bundled fonts for the rest.
EXTENDED_CODEPOINTS = [
    (0x0100, 0x017F),
    (0x2010, 0x2012),
    (0x2015, 0x2017),
    (0x201A, 0x201B),
    (0x201E, 0x2021),
    (0x2023, 0x2025),
    (0x2027, 0x2027),
    (0x2031, 0x2038),
]
# Kept in the subsets so that one Greek letter exists in bundled fonts for future use.
EXTRA_CODEPOINTS = [0x03A9]

FEATURES = ["kern", "liga", "tnum"]
WEIGHTS = ["Regular", "SemiBold"]


@dataclass(frozen=True)
class Family:
    """One bundled font family and the pinned release it comes from."""

    name: str  # file-name stem, e.g. "SourceSerif4"
    display: str
    repo: str
    tag: str
    url: str
    asset: str
    sha256: str
    paths: dict[str, str]  # weight -> in-archive font path
    license_path: str  # in-archive license path


FAMILIES = [
    Family(
        "SourceSerif4",
        "Source Serif 4",
        "adobe-fonts/source-serif",
        "4.005R",
        "https://github.com/adobe-fonts/source-serif/releases/download/4.005R/"
        "source-serif-4.005_Desktop.zip",
        "source-serif-4.005_Desktop.zip",
        "549fdb8f9a682bd06944298621404969f6de77c2e422ff3b8244a1dcd6a0c425",
        {
            "Regular": "source-serif-4.005_Desktop/TTF/SourceSerif4-Regular.ttf",
            "SemiBold": "source-serif-4.005_Desktop/TTF/SourceSerif4-Semibold.ttf",
        },
        "source-serif-4.005_Desktop/LICENSE.md",
    ),
    Family(
        "IBMPlexSans",
        "IBM Plex Sans",
        "IBM/plex",
        "@ibm/plex-sans@1.1.0",
        "https://github.com/IBM/plex/releases/download/%40ibm%2Fplex-sans%401.1.0/ibm-plex-sans.zip",
        "ibm-plex-sans.zip",
        "fb365d910566e6d199cc2c15579a7dd9a267128e18431a394ed81f1970c69200",
        {
            "Regular": "ibm-plex-sans/fonts/complete/ttf/IBMPlexSans-Regular.ttf",
            "SemiBold": "ibm-plex-sans/fonts/complete/ttf/IBMPlexSans-SemiBold.ttf",
        },
        "ibm-plex-sans/LICENSE.txt",
    ),
    Family(
        "Geist",
        "Geist",
        "vercel/geist-font",
        "v1.7.2",
        "https://github.com/vercel/geist-font/releases/download/v1.7.2/geist-font-v1.7.2.zip",
        "geist-font-v1.7.2.zip",
        "7fc800d2ac6b92844895196e5041aca55d814c15db70c44f79b3b83ab82b04e2",
        {
            "Regular": "geist-font/Geist/ttf/Geist-Regular.ttf",
            "SemiBold": "geist-font/Geist/ttf/Geist-SemiBold.ttf",
        },
        "geist-font/OFL.txt",
    ),
    Family(
        "Inter",
        "Inter",
        "rsms/inter",
        "v4.1",
        "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip",
        "Inter-4.1.zip",
        "9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e",
        {
            "Regular": "extras/ttf/Inter-Regular.ttf",
            "SemiBold": "extras/ttf/Inter-SemiBold.ttf",
        },
        "LICENSE.txt",
    ),
    Family(
        "JetBrainsMono",
        "JetBrains Mono",
        "JetBrains/JetBrainsMono",
        "v2.304",
        "https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/"
        "JetBrainsMono-2.304.zip",
        "JetBrainsMono-2.304.zip",
        "6f6376c6ed2960ea8a963cd7387ec9d76e3f629125bc33d1fdcd7eb7012f7bbf",
        {
            "Regular": "fonts/ttf/JetBrainsMono-Regular.ttf",
            "SemiBold": "fonts/ttf/JetBrainsMono-SemiBold.ttf",
        },
        "OFL.txt",
    ),
]


def fail(message: str) -> None:
    """Print an error and exit non-zero."""
    print(f"error: {message}", file=sys.stderr)
    sys.exit(1)


def fetch(family: Family, directory: Path) -> Path:
    """Download the family's release asset and verify its SHA-256."""
    target = directory / family.asset
    print(f"fetching {family.url}")
    with urllib.request.urlopen(family.url) as response:  # pinned https URLs
        target.write_bytes(response.read())
    digest = hashlib.sha256(target.read_bytes()).hexdigest()
    if digest != family.sha256:
        fail(f"{family.asset}: SHA-256 mismatch, expected {family.sha256}, got {digest}")
    return target


def reserved_font_name_line(license_text: str) -> str | None:
    """Return the header line declaring a Reserved Font Name, or None.

    Only the copyright header above the OFL body counts. The body defines the term itself.
    """
    for line in license_text.splitlines():
        if re.match(r"^-{5,}\s*$", line):
            break
        if "reserved font name" in line.lower():
            return line.strip()
    return None


def subset_font(data: bytes, codepoints: list[int], features: list[str]) -> bytes:
    """Subset a font to the code points and layout features, without hinting."""
    options = subset.Options()
    options.layout_features = features
    options.hinting = False
    options.name_IDs = ["*"]
    options.name_languages = ["*"]
    options.notdef_outline = True
    options.glyph_names = False
    font = TTFont(io.BytesIO(data))
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=codepoints)
    subsetter.subset(font)
    out = io.BytesIO()
    font.save(out)
    return out.getvalue()


def expand(ranges: list[tuple[int, int]]) -> list[int]:
    """Expand inclusive (low, high) ranges into a flat list of code points."""
    return [cp for lo, hi in ranges for cp in range(lo, hi + 1)]


def check_static_truetype(label: str, data: bytes) -> None:
    """Exit unless the font is a static (non-variable) TrueType font."""
    font = TTFont(io.BytesIO(data))
    if "fvar" in font or "glyf" not in font:
        fail(f"{label}: not a static TrueType font")


def lacking(data: bytes, codepoints: list[int]) -> list[int]:
    """Return the code points that the font has no glyph for."""
    cmap = TTFont(io.BytesIO(data)).getBestCmap()
    return [cp for cp in codepoints if cp not in cmap]


def fmt_cps(cps: list[int]) -> str:
    """Format code points as U+XXXX, comma separated."""
    return ", ".join(f"U+{cp:04X}" for cp in cps)


def main() -> None:
    """Build every font, run the checks, then write the outputs and the manifest."""
    required = expand(REQUIRED_CODEPOINTS)
    extended = expand(EXTENDED_CODEPOINTS)

    sections: list[str] = []
    sizes: list[tuple[str, int]] = []
    outputs: dict[str, bytes] = {}
    licenses: dict[str, bytes] = {}
    inter_regular: bytes | None = None
    problems: list[str] = []
    extended_found: set[int] = set()

    with tempfile.TemporaryDirectory() as tmp:
        for family in FAMILIES:
            archive = zipfile.ZipFile(fetch(family, Path(tmp)))
            license_bytes = archive.read(family.license_path)
            license_text = license_bytes.decode("utf-8")
            if "SIL OPEN FONT LICENSE" not in license_text:
                fail(f"{family.display}: license is not the SIL Open Font License")
            licenses[f"{family.name}-OFL.txt"] = license_bytes

            rfn = reserved_font_name_line(license_text)
            do_subset = rfn is None
            codepoints = required + extended + EXTRA_CODEPOINTS

            file_lines = []
            lacks_extended: set[int] = set()
            for weight in WEIGHTS:
                original = archive.read(family.paths[weight])
                check_static_truetype(f"{family.display} {weight}", original)
                if family.name == "Inter" and weight == "Regular":
                    inter_regular = original
                data = subset_font(original, codepoints, FEATURES) if do_subset else original
                missing = lacking(data, required)
                if missing:
                    problems.append(f"{family.display} {weight} lacks {fmt_cps(missing)}")
                missing_ext = lacking(data, extended)
                lacks_extended.update(missing_ext)
                extended_found.update(set(extended) - set(missing_ext))
                out_name = f"{family.name}-{weight}.ttf"
                outputs[out_name] = data
                sizes.append((out_name, len(data)))
                file_lines.append(
                    f"  - `{out_name}` from `{family.paths[weight]}`: {len(data):,} bytes"
                    f" (original {len(original):,})"
                )

            sections.append(
                "\n".join(
                    [
                        f"## {family.display}",
                        "",
                        f"- Source: https://github.com/{family.repo}, tag `{family.tag}`",
                        f"- Asset: `{family.asset}`, SHA-256 `{family.sha256}`",
                        f"- License: `{family.license_path}` -> `LICENSES/{family.name}-OFL.txt`",
                        "- Reserved Font Name: "
                        + (f'yes, license line: "{rfn}"' if rfn else "none declared in the header"),
                        "- Subset: "
                        + (
                            "no, files are byte-for-byte copies of the archive"
                            if not do_subset
                            else "yes (required and extended code points + U+03A9, features "
                            "kern/liga/tnum, no hinting)"
                        ),
                        "- Extended code points this family lacks: "
                        + (fmt_cps(sorted(lacks_extended)) if lacks_extended else "none"),
                        "- Files:",
                        *file_lines,
                        "",
                    ]
                )
            )

    uncovered = sorted(set(extended) - extended_found)
    if uncovered:
        problems.append(f"extended code points in no bundled font: {fmt_cps(uncovered)}")
    if inter_regular is None:
        fail("Inter Regular not found")
    total = sum(size for _, size in sizes)
    if total > BUDGET:
        problems.append(f"font budget exceeded: {total:,} > {BUDGET:,} bytes")
    if problems:
        fail("; ".join(problems))

    # Everything checked out; only now touch the tree.
    FONT_DIR.mkdir(parents=True, exist_ok=True)
    LICENSE_DIR.mkdir(parents=True, exist_ok=True)
    FIXTURE.parent.mkdir(parents=True, exist_ok=True)
    for name, data in outputs.items():
        (FONT_DIR / name).write_bytes(data)
    for name, text in licenses.items():
        (LICENSE_DIR / name).write_bytes(text)
    fixture = subset_font(inter_regular, EXTRA_CODEPOINTS, [])
    FIXTURE.write_bytes(fixture)

    manifest = [
        "# Bundled font manifest",
        "",
        "Generated by `scripts/fonts/build_fonts.py`; do not edit by hand.",
        "",
        "Fonts under the SIL Open Font License 1.1. A family whose license header declares a",
        "Reserved Font Name is shipped unmodified; other families are subset.",
        "",
        "Every font covers all `REQUIRED_CODEPOINTS`. `EXTENDED_CODEPOINTS` are kept when a font",
        "has them, and each is covered by at least one bundled font.",
        "",
        *sections,
        "## Test fixture",
        "",
        f"- `../tests/fixtures/omega-only.ttf`: Inter Regular subset to U+03A9 only,"
        f" {len(fixture):,} bytes (not part of the font budget).",
        "",
        "## Sizes",
        "",
        *[f"- `{name}`: {size:,} bytes" for name, size in sizes],
        "",
        f"Total: {total:,} bytes of {BUDGET:,} allowed ({total / BUDGET:.1%}).",
        "",
    ]
    (FONT_DIR / "MANIFEST.md").write_text("\n".join(manifest), encoding="utf-8")
    print(f"total font size: {total:,} bytes (budget {BUDGET:,})")


if __name__ == "__main__":
    main()
