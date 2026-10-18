#!/usr/bin/env python3
"""Reduce a Rust LCOV report to production code.

Usage: filter_rust_lcov.py REPO_ROOT INPUT.lcov OUTPUT.lcov

llvm-cov counts the bodies of inline test-only items (unit-test modules and helpers) as source
lines, and those bodies run nearly in full, so leaving them in would inflate the percentage. This
script parses each source file with tree-sitter and drops the line records that fall inside an
item whose `cfg` is true only under test (`test`, `all(test, ...)`); `any(test, ...)`, `not(test)`
and every other attribute are kept. It rewrites file names relative to the repository root and
recomputes the per-file totals. It reads no source other than the files the report names and
fails if one of them does not parse. A file reported more than once is merged into one record.

Run it with `uv run --no-project --with tree-sitter==0.25.2 --with tree-sitter-rust==0.24.2`.
"""

import os
import re
import sys
from pathlib import Path

import tree_sitter_rust
from tree_sitter import Language, Node, Parser

COMMENTS = re.compile(r"//[^\n]*|/\*.*?\*/", re.DOTALL)
TOKENS = re.compile(r'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z0-9_]*|[(),=]')
SKIPPED = ("attribute_item", "line_comment", "block_comment")

# Syntax nodes must not outlive their tree (that crashes the interpreter), so keep every parsed
# tree and its source alive until the process exits.
_KEEP: list[object] = []


def _predicate(tokens: list[str], at: int) -> tuple[bool, int]:
    """Parse one cfg predicate at `tokens[at]`; return (compiled only in tests, next position)."""
    name = tokens[at]
    at += 1
    if at < len(tokens) and tokens[at] == "=":
        return False, at + 2  # name = "value"
    if at < len(tokens) and tokens[at] == "(":
        at += 1
        children: list[bool] = []
        while tokens[at] != ")":
            child, at = _predicate(tokens, at)
            children.append(child)
            if tokens[at] == ",":
                at += 1
        at += 1
        if name == "all":
            return any(children), at  # one test-only operand makes the whole conjunction test-only
        if name == "any":
            return bool(children) and all(children), at
        return False, at  # not(...) and anything unknown
    return name == "test", at


def is_test_only_attribute(text: str) -> bool:
    """Whether the text inside `#[...]` is `cfg(P)` with P true only when testing.

    `test` and `all(test, ...)` qualify; `any(test, ...)`, `not(test)`, `cfg_attr` and other
    attributes do not. Comments and whitespace anywhere in the attribute are ignored.
    """
    tokens = TOKENS.findall(COMMENTS.sub(" ", text))
    if len(tokens) < 4:
        return False
    if tokens[0] != "cfg" or tokens[1] != "(" or tokens[-1] != ")":
        return False
    try:
        test_only, end = _predicate(tokens, 2)
    except IndexError:
        return False
    return test_only and end == len(tokens) - 1


def _attribute(node: Node) -> str:
    """The inside of `#[...]` or `#![...]`, which is what the cfg parser reads."""
    text = (node.text or b"").decode()
    return text[text.index("[") + 1 : text.rindex("]")]


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
        if node.type == "inner_attribute_item" and is_test_only_attribute(_attribute(node)):
            # An inner attribute applies to the enclosing file or module.
            enclosing = node.parent or tree.root_node
            spans.append((enclosing.start_point.row + 1, enclosing.end_point.row + 1))
        elif node.type == "attribute_item" and is_test_only_attribute(_attribute(node)):
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
    # One entry per source file: the extension crate includes some core files by path
    # (`crates/umber-py/src/../../umber-core/src/x.rs`), so the same file can be reported twice.
    files: dict[str, dict[int, int]] = {}
    for record in source.read_text().split("end_of_record"):
        rows = [row for row in record.strip().splitlines() if row]
        if not rows:
            continue
        name = next(row[3:] for row in rows if row.startswith("SF:"))
        path = Path(name)
        full = path if path.is_absolute() else root / path
        relative = Path(os.path.normpath(full)).relative_to(root).as_posix()
        spans = test_spans(full.read_bytes(), parser)
        kept = files.setdefault(relative, {})
        for row in rows:
            if row.startswith("DA:"):
                line, count, *_ = row[3:].split(",")
                if not any(first <= int(line) <= last for first, last in spans):
                    kept[int(line)] = kept.get(int(line), 0) + int(count)
    out: list[str] = []
    found = hit = 0
    for relative, kept in sorted(files.items()):
        file_hit = sum(count > 0 for count in kept.values())
        out.append(f"SF:{relative}")
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
