#!/usr/bin/env python3
"""Markdown table comparing two benchmark runs, for the job summary.

    compare.py BASE_CRITERION_DIR HEAD_CRITERION_DIR BASE_PY_JSON HEAD_PY_JSON

The criterion directories are `target/criterion` of each revision; the Python files are the
JSON that `python_e2e.py` printed. A ratio is head / base, so above 1 means head is slower or
larger. The table is information only: this script never fails on a ratio.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def criterion_medians_ms(root: Path) -> dict[str, float]:
    """Median time in milliseconds of every benchmark under a `target/criterion` directory."""
    medians: dict[str, float] = {}
    for estimates in sorted(root.glob("*/*/new/estimates.json")):
        name = f"{estimates.parts[-4]}/{estimates.parts[-3]}"
        medians[name] = json.loads(estimates.read_text())["median"]["point_estimate"] / 1e6
    return medians


def cell(value: float | None, unit: str) -> str:
    """A table cell: the value with its unit, or `n/a` when the revision has none."""
    return "n/a" if value is None else f"{value:.2f}{unit}"


def row(name: str, base: float | None, head: float | None, unit: str) -> str:
    """One table row: the name, both values and head / base."""
    ratio = "n/a" if base is None or head is None or base == 0 else f"{head / base:.2f}x"
    return f"| `{name}` | {cell(base, unit)} | {cell(head, unit)} | {ratio} |"


def table(title: str, base: dict[str, float], head: dict[str, float], unit: str) -> list[str]:
    """A Markdown table of every name in either run."""
    lines = [
        f"### {title}",
        "",
        "| benchmark | base | head | head / base |",
        "| --- | ---: | ---: | ---: |",
    ]
    for name in sorted(base.keys() | head.keys()):
        lines.append(row(name, base.get(name), head.get(name), unit))
    return [*lines, ""]


def main() -> None:
    """Print the comparison of the two runs named on the command line."""
    base_dir, head_dir, base_py, head_py = (Path(a) for a in sys.argv[1:5])
    out = ["## Benchmarks, base vs head", ""]
    out += table(
        "Rust (criterion, median)",
        criterion_medians_ms(base_dir),
        criterion_medians_ms(head_dir),
        " ms",
    )
    py_base = json.loads(base_py.read_text())
    py_head = json.loads(head_py.read_text())
    out += table("Python end to end (seconds, MiB of RSS)", py_base, py_head, "")
    out.append(
        "Both revisions ran in the same job on the same runner, one after the other. "
        "Shared runners are noisy: read ratios as hints and look for large, repeatable "
        "changes. Times are never a pass/fail gate."
    )
    print("\n".join(out))


if __name__ == "__main__":
    main()
