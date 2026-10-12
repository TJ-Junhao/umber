#!/usr/bin/env python3
"""End-to-end Python timings and memory for the installed `umber` package.

Run it with the interpreter that has `umber` installed; it prints one JSON object:

    time_line_1m_png_s      best of 3 wall times for ub.line(t, y).figure.to_png() with 1M points
    build_line_5m_rss_mib   peak RSS growth while ub.line builds a 5M float64 point series
    render_line_5m_rss_mib  peak RSS growth above the built figure while it renders to PNG

The memory numbers come from the process's peak RSS (ru_maxrss), which never decreases, so they are
measured in a fresh child process whose input arrays are the only large allocation
before the measurement. The script guards the single-copy data path: building a 5M point line
stores x and y once each, so RSS should grow by about 80 MiB (two 40 MiB series), not a multiple.
"""

from __future__ import annotations

import json
import platform
import resource
import subprocess
import sys
import time

import numpy as np

import umber as ub


def _maxrss_mib() -> float:
    peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    # Linux reports KiB, macOS bytes.
    return peak / (1024 * 1024) if platform.system() == "Darwin" else peak / 1024


def _data(n: int) -> tuple[np.ndarray, np.ndarray]:
    """Two float64 arrays of `n` values, made in place so the peak RSS stays at their size."""
    t = np.linspace(0.0, 100.0, n)
    y = np.empty_like(t)
    np.sin(t, out=y)
    y *= 10.0
    return t, y


def time_line_1m_png() -> float:
    """Best of three wall times, in seconds, for a 1M point line rendered to PNG."""
    t, y = _data(1_000_000)
    best = float("inf")
    for _ in range(3):
        start = time.perf_counter()
        png = ub.line(t, y).figure.to_png()
        best = min(best, time.perf_counter() - start)
        assert png[1:4] == b"PNG"
    return best


def memory_child() -> dict[str, float]:
    """Build and render a 5M point line; report the peak RSS growth of each step in MiB."""
    t, y = _data(5_000_000)
    before_build = _maxrss_mib()
    axes = ub.line(t, y)
    after_build = _maxrss_mib()
    png = axes.figure.to_png()
    after_render = _maxrss_mib()
    assert png[1:4] == b"PNG"
    return {
        "build_line_5m_rss_mib": round(after_build - before_build, 1),
        "render_line_5m_rss_mib": round(after_render - after_build, 1),
    }


def main() -> None:
    """Print the timing and the memory numbers as JSON."""
    if len(sys.argv) > 1 and sys.argv[1] == "--memory-child":
        print(json.dumps(memory_child()))
        return
    # The memory child starts before this process renders anything: on Linux a child keeps the
    # parent's peak RSS across fork and exec, so a parent that had rendered 1M points would
    # raise the child's starting peak above the memory it is about to measure.
    child = subprocess.run(
        [sys.executable, __file__, "--memory-child"],
        check=True,
        capture_output=True,
        text=True,
    )
    result: dict[str, float] = {"time_line_1m_png_s": round(time_line_1m_png(), 4)}
    result.update(json.loads(child.stdout))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
