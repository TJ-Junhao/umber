#!/usr/bin/env python3
"""Fail unless the combined line coverage of the given LCOV reports reaches a minimum.

Usage: check_gate.py MIN_PERCENT REPORT.lcov...

The number is computed the way Codecov does, so it equals the one Codecov shows for the same
reports: hits / (hits + partials + misses) over lines, where a line that carries branch records
counts by its branches (all taken: hit, some taken: partial, none taken: miss) and any other line
counts by its execution count. Files are keyed by normalised path, so a file that appears in
several reports is counted once, with its hits and branches merged.

Every report must exist, be complete (each record ends with `end_of_record` and has consistent
LF and LH totals) and contain at least one file with a line to cover; otherwise the gate fails.
"""

import posixpath
import sys
from pathlib import Path

Lines = dict[int, int]
Branches = dict[int, dict[tuple[str, str], int]]


def parse(path: Path) -> dict[str, tuple[Lines, Branches]]:
    """Read one report into {file: (line counts, branch counts)}, or raise ValueError."""
    if not path.is_file():
        raise ValueError("report is missing")
    files: dict[str, tuple[Lines, Branches]] = {}
    name: str | None = None
    lines: Lines = {}
    branches: Branches = {}
    totals: dict[str, int] = {}
    for row in path.read_text().splitlines():
        row = row.strip()
        if row.startswith("SF:"):
            if name is not None:
                raise ValueError(f"record for {name} is not closed")
            name = posixpath.normpath(row[3:].replace("\\", "/")).removeprefix("./")
            lines, branches, totals = {}, {}, {}
        elif name is None:
            if row:
                raise ValueError(f"unexpected row outside a record: {row!r}")
        elif row.startswith("DA:"):
            line, count, *_ = row[3:].split(",")
            lines[int(line)] = lines.get(int(line), 0) + int(count)
        elif row.startswith("BRDA:"):
            line, block, branch, taken = row[5:].split(",")
            slot = branches.setdefault(int(line), {})
            key = (block, branch)
            slot[key] = slot.get(key, 0) + (0 if taken == "-" else int(taken))
        elif row.startswith(("LF:", "LH:")):
            totals[row[:2]] = int(row[3:])
        elif row == "end_of_record":
            if totals.get("LF") != len(lines) or totals.get("LH") != sum(
                c > 0 for c in lines.values()
            ):
                raise ValueError(f"line totals of {name} do not match its DA records")
            prior_lines, prior_branches = files.setdefault(name, ({}, {}))
            for line, count in lines.items():
                prior_lines[line] = prior_lines.get(line, 0) + count
            for line, slot in branches.items():
                merged = prior_branches.setdefault(line, {})
                for key, taken in slot.items():
                    merged[key] = merged.get(key, 0) + taken
            name = None
    if name is not None:
        raise ValueError(f"truncated: record for {name} is not closed")
    if not any(lines for lines, _ in files.values()):
        raise ValueError("no file with a line to cover")
    return files


def classify(files: dict[str, tuple[Lines, Branches]]) -> tuple[int, int, int]:
    """Count (hits, partials, misses) the way Codecov does."""
    hits = partials = misses = 0
    for lines, branches in files.values():
        for line, count in lines.items():
            taken = [t > 0 for t in branches.get(line, {}).values()]
            if taken:
                state = "hit" if all(taken) else "miss" if not any(taken) else "partial"
            else:
                state = "hit" if count > 0 else "miss"
            hits += state == "hit"
            partials += state == "partial"
            misses += state == "miss"
    return hits, partials, misses


def main(minimum: float, reports: list[Path]) -> int:
    """Print the coverage of each report and of all together; return the exit status."""
    merged: dict[str, tuple[Lines, Branches]] = {}
    for report in reports:
        try:
            files = parse(report)
        except (ValueError, OSError) as error:
            print(f"{report}: {error}")
            return 1
        hits, partials, misses = classify(files)
        print(f"{report}: {_summary(hits, partials, misses)}")
        for name, (lines, branches) in files.items():
            prior_lines, prior_branches = merged.setdefault(name, ({}, {}))
            for line, count in lines.items():
                prior_lines[line] = prior_lines.get(line, 0) + count
            for line, slot in branches.items():
                target = prior_branches.setdefault(line, {})
                for key, taken in slot.items():
                    target[key] = target.get(key, 0) + taken
    hits, partials, misses = classify(merged)
    percent = _percent(hits, partials, misses)
    print(f"combined: {_summary(hits, partials, misses)}, minimum {minimum:g} %")
    return int(percent + 1e-9 < minimum)


def _summary(hits: int, partials: int, misses: int) -> str:
    percent = _percent(hits, partials, misses)
    return f"{hits} hit, {partials} partial, {misses} miss ({percent:.2f} %)"


def _percent(hits: int, partials: int, misses: int) -> float:
    return 100 * hits / (hits + partials + misses)


if __name__ == "__main__":
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    raise SystemExit(main(float(sys.argv[1]), [Path(arg) for arg in sys.argv[2:]]))
