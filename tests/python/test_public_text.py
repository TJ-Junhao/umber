"""Lint: shipped sources must not contain internal-process wording.

Every tracked text file is scanned except generated files (snapshots, lock files), binaries
and `docs/`. Underscores count as word separators, so identifiers such as
`some_reviewer_case` are caught too. Plain "spec" is allowed (OpenType spec, `ColorSpec`);
milestone references such as "M5 clips" are banned, while the "M4 downsampling" algorithm
name is allowed.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]

# Built from fragments so this file does not match its own patterns.
_PATTERNS = [
    r"\b" + "review" + r"ers?\b",
    r"\b" + "review" + r"\s+focus\b",
    r"\b" + "orchestr" + r"ators?\b",
    r"\b" + "tick" + r"ets?\b",
    "spec" + r"\s+(?:" + "section" + "|§)",
    r"\b" + "mo" + r"cks?\b",
    r"\bthe\s+" + "plan" + r"\b",
    r"\b" + "plan" + r"'s\b",
    "PR" + r"\s*#\s*\d+",
    r"\b" + "round" + r"[-\s]?\d+\b",
    "owner" + r"\s+decisions?",
    # Milestone references ("M5 clips ..."); "M4 downsampling" is an algorithm name.
    r"\bM\d+\s+(?!downsampling\b)[A-Za-z]",
]
# The milestone pattern must stay case-sensitive in its capital M, so it is checked apart.
_MILESTONE = re.compile(_PATTERNS[-1])
_GENERAL = re.compile("|".join(f"(?:{p})" for p in _PATTERNS[:-1]), re.IGNORECASE)

_SELF = "tests/python/test_public_text.py"
_SKIP_SUFFIXES = {".snap", ".lock", ".ttf", ".otf", ".png", ".whl", ".so", ".pyd"}


def is_scanned(rel: str) -> bool:
    path = Path(rel)
    return (
        path.parts[0] != "docs"
        and path.suffix not in _SKIP_SUFFIXES
        and rel != _SELF  # this file spells out the banned wording as test data
    )


def line_hits(line: str) -> bool:
    return bool(
        _GENERAL.search(line.replace("_", " ")) or _MILESTONE.search(line.replace("_", " "))
    )


def tracked_files(root: Path) -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", "-z"], cwd=root, check=True, capture_output=True, text=True
    ).stdout
    return [name for name in out.split("\0") if name]


def scan(root: Path, files: list[str]) -> list[str]:
    hits: list[str] = []
    for rel in files:
        path = root / rel
        if not is_scanned(rel) or not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for number, line in enumerate(text.splitlines(), start=1):
            if line_hits(line):
                hits.append(f"{rel}:{number}: {line.strip()}")
    return hits


def test_no_process_references_in_shipped_text() -> None:
    assert scan(ROOT, tracked_files(ROOT)) == []


_B = "review"  # fragments keep the cases below from matching the patterns in this file
_CASES = [
    "some_" + _B + "er_curve_repro_is_skipped",
    "SPEC  " + "SECTION 7",
    "spec\t" + "section 7",
    "spec " + "§ 4",
    "PR  " + "#18",
    "PR" + "#18",
    "round " + "12",
    "Round-" + "3 fixes",
    _B.capitalize() + " Focus 3",
    _B + "ers requested this",
    "the " + "mock data",
    "M5 " + "clips data",
    "the " + "plan",
    "the   " + "plan's original value",
    "an " + "orchestrator note",
    "see " + "ticket 4",
    "owner  " + "decision",
]


@pytest.mark.parametrize("line", _CASES)
def test_banned_wording_is_caught(line: str) -> None:
    assert line_hits(line)


@pytest.mark.parametrize(
    "line",
    [
        "Project points onto the plane.",
        "Planning computes the planar transform.",
        "Follows the OpenType spec.",
        "colour: ColorSpec",
        "Copyright owner: Example",
        "//! M4 downsampling: reduce a long line",
        '<path d="M10 10 L50 10"/>',
        "A preview of the figure; an explicit limit.",
    ],
)
def test_legitimate_wording_is_allowed(line: str) -> None:
    assert not line_hits(line)


@pytest.mark.parametrize(
    "rel",
    [
        "Cargo.toml",
        "crates/umber-core/Cargo.toml",
        "pyproject.toml",
        "README.md",
        "README-ZH.md",
        "crates/umber-core/README.md",
        "crates/umber-core/fonts/MANIFEST.md",
        "crates/umber-core/src/lib.rs",
        "python/umber/__init__.pyi",
        "scripts/build.sh",
        "scripts/fonts/build_fonts.py",
        ".github/workflows/ci.yml",
        "tests/python/test_x.py",
    ],
)
def test_surface_is_scanned_and_reported(rel: str, tmp_path: Path) -> None:
    assert is_scanned(rel)
    target = tmp_path / rel
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("ok\n# the " + _B + "er said so\n", encoding="utf-8")
    assert scan(tmp_path, [rel]) == [f"{rel}:2: # the " + _B + "er said so"]


@pytest.mark.parametrize(
    "rel",
    [
        "docs/notes.md",
        "crates/umber-core/fonts/Inter-Regular.ttf",
        "x/a.png",
        "x/a.snap",
        "uv.lock",
    ],
)
def test_generated_binary_and_docs_are_skipped(rel: str, tmp_path: Path) -> None:
    target = tmp_path / rel
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("the " + _B + "er\n", encoding="utf-8")
    assert not is_scanned(rel)
    assert scan(tmp_path, [rel]) == []
