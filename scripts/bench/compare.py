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


def cell(value: float | None, unit: str, digits: int) -> str:
    """A table cell: the value with its unit, or `n/a` when the revision has none.

    `digits` is the number of significant digits, so seconds keep their precision and MiB do not
    grow trailing zeros.
    """
    return "n/a" if value is None else f"{value:.{digits}g}{unit}"


def row(name: str, base: float | None, head: float | None, unit: str, digits: int) -> str:
    """One table row: the name, both values and head / base."""
    ratio = "n/a" if base is None or head is None or base == 0 else f"{head / base:.2f}x"
    return f"| `{name}` | {cell(base, unit, digits)} | {cell(head, unit, digits)} | {ratio} |"


def table(
    title: str, base: dict[str, float], head: dict[str, float], unit: str, digits: int
) -> list[str]:
    """A Markdown table of every name in either run; head only when `base` is empty."""
    if not base:
        lines = [f"### {title}", "", "| benchmark | head |", "| --- | ---: |"]
        lines += [f"| `{n}` | {cell(head[n], unit, digits)} |" for n in sorted(head)]
        return [*lines, ""]
    lines = [
        f"### {title}",
        "",
        "| benchmark | base | head | head / base |",
        "| --- | ---: | ---: | ---: |",
    ]
    for name in sorted(base.keys() | head.keys()):
        lines.append(row(name, base.get(name), head.get(name), unit, digits))
    return [*lines, ""]


def main() -> None:
    """Print the comparison of the two runs named on the command line.

    A base path that does not exist stands for a base without benchmarks; the tables then hold
    the head only.
    """
    base_dir, head_dir, base_py, head_py = (Path(a) for a in sys.argv[1:5])
    has_base = base_dir.is_dir() and base_py.is_file()
    out = ["## Benchmarks, base vs head" if has_base else "## Benchmarks, head only", ""]
    if not has_base:
        out += ["Base revision has no benchmarks; head results only.", ""]
    out += table(
        "Rust (criterion, median)",
        criterion_medians_ms(base_dir) if has_base else {},
        criterion_medians_ms(head_dir),
        " ms",
        4,
    )
    py_base = json.loads(base_py.read_text()) if has_base else {}
    py_head = json.loads(head_py.read_text())
    out += table("Python end to end (seconds, MiB of RSS)", py_base, py_head, "", 3)
    if has_base:
        out.append("Both revisions ran in the same job on the same runner, one after the other. ")
    out.append(
        "Shared runners are noisy: read ratios as hints and look for large, repeatable "
        "changes. Times are never a pass/fail gate."
    )
    print("\n".join(out))


if __name__ == "__main__":
    main()
