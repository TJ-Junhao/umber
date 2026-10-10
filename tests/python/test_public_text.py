"""Lint: shipped sources must not contain internal-process wording."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Built from fragments so this file does not match its own patterns.
_PATTERNS = [
    r"\b" + "review" + r"er\b",
    r"\b" + "orchestr" + r"ator\b",
    r"\b" + "tick" + r"et\b",
    "spec " + "section",
    "spec " + "§",
    r"\b" + "mo" + r"cks?\b",
    "the " + "plan",
    "plan" + "'s",
    "PR " + r"#\d+",
    r"\b" + "round" + r"[- ]?\d\b",
    "owner " + "decision",
]
BANNED = re.compile("|".join(f"(?:{p})" for p in _PATTERNS), re.IGNORECASE)

_GLOBS = [
    "crates/**/*.rs",
    "python/**/*.py",
    "python/**/*.pyi",
    "tests/**/*.py",
    "scripts/**/*.py",
    "README.md",
    "README-ZH.md",
]


def scan(root: Path) -> list[str]:
    hits: list[str] = []
    seen: set[Path] = set()
    for pattern in _GLOBS:
        for path in sorted(root.glob(pattern)):
            if path in seen or not path.is_file() or "target" in path.relative_to(root).parts:
                continue
            seen.add(path)
            text = path.read_text(encoding="utf-8", errors="replace")
            for number, line in enumerate(text.splitlines(), start=1):
                if BANNED.search(line):
                    hits.append(f"{path.relative_to(root)}:{number}: {line.strip()}")
    return hits


def test_no_process_references_in_shipped_text() -> None:
    assert scan(ROOT) == []


def test_scan_reports_file_and_line(tmp_path: Path) -> None:
    (tmp_path / "python").mkdir()
    (tmp_path / "python" / "x.py").write_text("ok\n# the " + "review" + "er said so\n")
    assert scan(tmp_path) == ["python/x.py:2: # the " + "review" + "er said so"]
