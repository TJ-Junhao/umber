"""The font build script keeps its own code point lists; they must equal the Rust constants.

`scripts/fonts/build_fonts.py` subsets fonts to `REQUIRED_CODEPOINTS` and `EXTENDED_CODEPOINTS`,
and `crates/umber-core/src/fonts.rs` declares the same ranges for the tests that check every
bundled file. The script is read as source (it needs `fontTools`, a build-time dependency only).
"""

from __future__ import annotations

import ast
import re
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "fonts" / "build_fonts.py"
FONTS_RS = ROOT / "crates" / "umber-core" / "src" / "fonts.rs"

NAMES = ["REQUIRED_CODEPOINTS", "EXTENDED_CODEPOINTS"]


def script_ranges(name: str, text: str) -> list[tuple[int, int]]:
    """Return the inclusive ranges that the script assigns to the module constant ``name``."""
    for node in ast.parse(text).body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == name for target in node.targets
        ):
            value = ast.literal_eval(node.value)
            return [(int(lo), int(hi)) for lo, hi in value]
    raise AssertionError(f"{name} is not assigned in {SCRIPT}")


def rust_ranges(name: str, text: str) -> list[tuple[int, int]]:
    """Return the inclusive ranges of the Rust constant ``name`` declared as ``&[(u32, u32)]``.

    Fails on any content it cannot parse, so valid Rust syntax is never silently ignored.
    """
    match = re.search(rf"const {name}: &\[\(u32, u32\)\] = &\[(.*?)\];", text, re.DOTALL)
    assert match is not None, f"{name} is not declared in {FONTS_RS}"
    body = match.group(1)
    hex_literal = r"0x([0-9A-Fa-f][0-9A-Fa-f_]*)"
    pair = rf"\(\s*{hex_literal}\s*,\s*{hex_literal}\s*\)"
    pairs = re.findall(pair, body)
    assert pairs, f"{name} in {FONTS_RS} holds no ranges"
    unparsed = re.sub(pair, "", body).replace(",", "").strip()
    assert not unparsed, f"{name} in {FONTS_RS} contains unparsed range syntax: {unparsed}"
    return [(int(lo.replace("_", ""), 16), int(hi.replace("_", ""), 16)) for lo, hi in pairs]


@pytest.mark.parametrize("name", NAMES)
def test_script_lists_match_the_rust_constants(name: str) -> None:
    script = script_ranges(name, SCRIPT.read_text(encoding="utf-8"))
    rust = rust_ranges(name, FONTS_RS.read_text(encoding="utf-8"))
    assert script == rust, (
        f"{name} differs between {SCRIPT.relative_to(ROOT)} and {FONTS_RS.relative_to(ROOT)}"
    )


def test_the_check_notices_an_edited_list() -> None:
    text = SCRIPT.read_text(encoding="utf-8")
    edited = text.replace("(0x2212, 0x2212),", "(0x2212, 0x2213),", 1)
    assert edited != text
    rust = rust_ranges("REQUIRED_CODEPOINTS", FONTS_RS.read_text(encoding="utf-8"))
    assert script_ranges("REQUIRED_CODEPOINTS", edited) != rust


def test_rust_ranges_parse_underscores_and_reject_unknown_syntax() -> None:
    header = "const REQUIRED_CODEPOINTS: &[(u32, u32)] = &["
    assert rust_ranges("REQUIRED_CODEPOINTS", header + "(0x1_0000, 0x1_0000), (0x20, 0x7E)];") == [
        (0x10000, 0x10000),
        (0x20, 0x7E),
    ]
    with pytest.raises(AssertionError, match="unparsed"):
        rust_ranges("REQUIRED_CODEPOINTS", header + "(0x20, 0x7E), (65, 66)];")
