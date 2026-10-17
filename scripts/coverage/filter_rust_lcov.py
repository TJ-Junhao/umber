#!/usr/bin/env python3
"""Reduce a Rust LCOV report to production code.

Usage: filter_rust_lcov.py REPO_ROOT INPUT.lcov OUTPUT.lcov

llvm-cov counts the bodies of inline `#[cfg(test)]` items (unit-test modules and helpers) as
source lines, and those bodies run nearly in full, so leaving them in would inflate the
percentage. This script parses each source file with tree-sitter, drops the line and branch
records that fall inside a `#[cfg(test)]` item, rewrites file names relative to the repository
root and recomputes the per-file totals. It reads no source other than the files the report names
and fails if one of them does not parse.

Run it with `uv run --no-project --with tree-sitter==0.25.2 --with tree-sitter-rust==0.24.2`.
"""

import re
import sys
from pathlib import Path

import tree_sitter_rust
from tree_sitter import Language, Node, Parser

CFG_TEST = re.compile(rb"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")
SKIPPED = ("attribute_item", "line_comment", "block_comment")

# Syntax nodes must not outlive their tree (that crashes the interpreter), so keep every parsed
# tree and its source alive until the process exits.
_KEEP: list[object] = []


def test_spans(source: bytes, parser: Parser) -> list[tuple[int, int]]:
    """Return the 1-based inclusive line ranges of every `#[cfg(test)]` item."""
    tree = parser.parse(source)
    _KEEP.extend([tree, source])
    if tree.root_node.has_error:
        raise SystemExit("syntax error while looking for #[cfg(test)] items")
    spans: list[tuple[int, int]] = []

    # An explicit stack: expression trees can be deep enough to overflow the interpreter's C stack.
    pending: list[Node] = [tree.root_node]
    while pending:
        node = pending.pop()
        if node.type == "attribute_item" and CFG_TEST.fullmatch(node.text or b""):
            item = node.next_named_sibling
            while item is not None and item.type in SKIPPED:
                item = item.next_named_sibling
            if item is not None:
                spans.append((node.start_point.row + 1, item.end_point.row + 1))
        pending.extend(node.named_children)
    return spans


def main(root: Path, source: Path, target: Path) -> None:
    """Filter the report at `source` into `target`."""
    parser = Parser(Language(tree_sitter_rust.language()))
    out: list[str] = []
    found = hit = 0
    for record in source.read_text().split("end_of_record"):
        lines = [row for row in record.strip().splitlines() if row]
        if not lines:
            continue
        name = next(row[3:] for row in lines if row.startswith("SF:"))
        path = Path(name)
        relative = path.relative_to(root) if path.is_absolute() else path
        spans = test_spans((root / relative).read_bytes(), parser)

        counts = {
            int(line): int(count)
            for row in lines
            if row.startswith("DA:")
            for line, count, *_ in [row[3:].split(",")]
        }
        kept = {
            line: count
            for line, count in counts.items()
            if not any(first <= line <= last for first, last in spans)
        }
        file_hit = sum(count > 0 for count in kept.values())
        out.append(f"SF:{relative.as_posix()}")
        out.extend(f"DA:{line},{count}" for line, count in sorted(kept.items()))
        out.extend([f"LF:{len(kept)}", f"LH:{file_hit}", "end_of_record"])
        found += len(kept)
        hit += file_hit
    target.write_text("\n".join(out) + "\n")
    print(f"{target}: {hit}/{found} lines ({100 * hit / found:.2f} %)")


if __name__ == "__main__":
    if len(sys.argv) != 4:
        raise SystemExit(__doc__)
    main(Path(sys.argv[1]).resolve(), Path(sys.argv[2]), Path(sys.argv[3]))
