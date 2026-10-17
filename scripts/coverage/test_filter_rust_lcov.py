#!/usr/bin/env python3
"""Fixture tests for filter_rust_lcov.py.

Run: uv run --no-project --with tree-sitter==0.25.2 --with tree-sitter-rust==0.24.2 \
    scripts/coverage/test_filter_rust_lcov.py
"""

import sys
from pathlib import Path

import tree_sitter_rust
from tree_sitter import Language, Parser

sys.path.insert(0, str(Path(__file__).parent))
from filter_rust_lcov import is_test_only_attribute, test_spans  # noqa: E402

TEST_ONLY = [
    "cfg(test)",
    "cfg( test )",
    "cfg(/* gate */ test // trailing\n)",
    'cfg(all(test, feature = "x"))',
    "cfg(all(unix, test))",
    "cfg(all(not(windows), all(test, unix)))",
    "cfg(any(test, test))",
]
KEPT = [
    'cfg(any(test, feature = "x"))',
    "cfg(any(unix, test))",
    "cfg(not(test))",
    "cfg(all(unix, not(test)))",
    "cfg(all())",
    'cfg(feature = "test")',
    "cfg(testing)",
    "cfg_attr(test, derive(Debug))",
    "derive(Debug)",
    "cfg(test, unix)",
]

SOURCE = b"""fn kept() {}
#[cfg(test)]
mod a {
    fn x() {}
}
#[cfg(any(test, unix))]
fn kept_too() {}
#[cfg(all(test, unix))]
// note
#[allow(dead_code)]
fn b() {
    let _ = 1;
}
#[cfg(not(test))]
fn also_kept() {}
"""


def main() -> None:
    """Check the predicate and the span finder."""
    for text in TEST_ONLY:
        assert is_test_only_attribute(text), text
    for text in KEPT:
        assert not is_test_only_attribute(text), text
    parser = Parser(Language(tree_sitter_rust.language()))
    assert sorted(test_spans(SOURCE, parser)) == [(2, 5), (8, 13)], test_spans(SOURCE, parser)
    inner = b"#![cfg(test)]\nfn a() {}\nfn b() {}\n"
    spans = test_spans(inner, parser)
    assert len(spans) == 1 and spans[0][0] == 1 and spans[0][1] >= 3, spans
    print("ok")


if __name__ == "__main__":
    main()
