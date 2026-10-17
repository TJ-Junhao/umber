#!/usr/bin/env python3
"""End-to-end Python timings of umber next to matplotlib.

Each case times the Python call that builds a figure through the encoded bytes: ``to_png()`` or
``to_svg()`` for umber, ``savefig`` to a ``BytesIO`` for matplotlib (Agg backend, default style,
``plt.close`` included). Figures are 6 x 4 in, PNG at 200 dpi, both libraries with identical
deterministic input data that is built outside the timed region. Every case runs once as a
warm-up, then ``--runs`` times (at least 7 are meant); the table shows the median with the
interquartile range (IQR) and the raw samples stay in the JSON. Each library runs in its own
process, so they share no caches. The samples come in two interleaved batches, the libraries
alternating per case and swapping order in the second batch, so changing host load affects both.
The import time of each is measured in fresh interpreters.

Usage::

    python scripts/bench/python_vs_matplotlib.py run --tag multi --out DIR
    taskset -c 0 python scripts/bench/python_vs_matplotlib.py run --tag single --out DIR
    python scripts/bench/python_vs_matplotlib.py combine DIR

``run`` writes ``DIR/<tag>.json``; ``combine`` joins the runs found in ``DIR`` into
``DIR/results.json`` and ``DIR/results.md``. The ratio in the table is matplotlib's median divided
by umber's, so a ratio above 1 means umber is faster. umber has no thread setting of its own: it
composites scatter markers on up to 8 threads, as many as ``std::thread::available_parallelism``
reports, which follows the CPU affinity, so ``taskset -c 0`` is the single-threaded measure.

What the two libraries do differently, with both left at their defaults as users get them:

* umber reduces a dense, sorted line to the points that can show (M4 downsampling) and draws a
  large cell grid as one image. matplotlib simplifies line paths by default (``path.simplify``)
  but draws every cell of a ``pcolormesh``; the heatmap cases use ``imshow``, which a matplotlib
  user picks for a 1,000 x 1,000 array and which also draws one image.
* matplotlib's ``hist2d`` and ``hexbin`` count the bins with NumPy in Python before drawing;
  umber counts them in Rust. The hexbin row is an approximate workload, not identical work:
  the grids are built differently (a gridsize of 50 gives matplotlib 50 hexagons across, umber
  50 columns of its own hexagon grid). Empty hexagons are not drawn by either (``mincnt=1``).
* ``hist2d``, ``hexbin`` and the heatmap draw a colorbar in umber, so the matplotlib side adds
  ``fig.colorbar`` too and both plot areas shrink alike. The matplotlib SVG heatmap uses
  ``interpolation="none"`` so the embedded image holds one pixel per cell, as umber's does;
  the PNG heatmap keeps the default, since both libraries then resample to the output pixels.
* matplotlib's default text is paths in SVG (``svg.fonttype = "path"``), as umber's default
  ``SvgText.OUTLINES`` is, so SVG sizes and times compare like with like.
* The hexbin input is the normal sample clamped to [-3, 3] on both axes for both libraries:
  umber refuses the unclipped sample at grid size 50 (its hexagon bounds do not settle).
* The 4 x 5 grid uses a 12 x 8 in figure, because 6 x 4 in leaves no room for 20 panels.
* The heatmap names its 1,000 columns and rows with strings for umber, whose heatmap is
  category based; matplotlib's ``imshow`` labels the axes with indices.
"""

from __future__ import annotations

import argparse
import io
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

import numpy as np

WIDTH, HEIGHT, DPI = 6.0, 4.0, 200
GRID_WIDTH, GRID_HEIGHT = 12.0, 8.0

# Case name in the table order, with a short description.
CASES: dict[str, str] = {
    "line_1m_png": "1M-point line, PNG",
    "line_1m_svg": "1M-point line, SVG",
    "line_time_1m_png": "1M-point line on a time axis, PNG",
    "scatter_100k_png": "100k-point scatter, PNG",
    "hist_1m_png": "Histogram of 1M values (30 bins), PNG",
    "hist2d_1m_png": "2-D histogram of 1M points (100 x 100 bins), PNG",
    "hexbin_1m_png": "Hexbin of 1M points (grid size 50), PNG",
    "heatmap_1000_png": "Heatmap of 1,000 x 1,000 cells, PNG",
    "heatmap_1000_svg": "Heatmap of 1,000 x 1,000 cells, SVG",
    "grid_4x5_png": "4 x 5 panels of a 10k-point line, PNG",
    "small_line_png": "Small figure: 1k-point line x 2, title, labels, legend, PNG",
    "bar_5_png": "5-bar chart, PNG",
}


def _line_data(n: int) -> tuple[np.ndarray, np.ndarray]:
    """A sorted x and a noisy sine y of ``n`` points."""
    rng = np.random.default_rng(1)
    x = np.linspace(0.0, 100.0, n)
    return x, 10.0 * np.sin(x) + rng.normal(0.0, 0.5, n)


def _points(n: int) -> tuple[np.ndarray, np.ndarray]:
    """``n`` correlated normal points."""
    rng = np.random.default_rng(2)
    x = rng.normal(0.0, 1.0, n)
    return x, 0.5 * x + rng.normal(0.0, 1.0, n)


def _hexbin_points(n: int) -> tuple[np.ndarray, np.ndarray]:
    """``n`` correlated normal points clipped to +-3 standard deviations."""
    x, y = _points(n)
    return np.clip(x, -3.0, 3.0), np.clip(y, -3.0, 3.0)


def _times(n: int) -> np.ndarray:
    """``n`` second-spaced ``datetime64[s]`` times from 2024-01-01."""
    return np.datetime64("2024-01-01T00:00:00") + np.arange(n).astype("timedelta64[s]")


def _cells() -> np.ndarray:
    """A 1,000 x 1,000 matrix of smooth values with noise."""
    rng = np.random.default_rng(3)
    axis = np.linspace(0.0, 6.0, 1000)
    return np.sin(axis)[:, None] * np.cos(axis)[None, :] + rng.normal(0.0, 0.1, (1000, 1000))


def _umber_cases() -> dict[str, Callable[[], Callable[[], int]]]:
    """For each case, a setup that builds the data and returns the timed function."""
    import umber as ub

    def png(fig: ub.Figure) -> int:
        return len(fig.to_png(dpi=DPI))

    def line_png() -> Callable[[], int]:
        x, y = _line_data(1_000_000)
        return lambda: png(ub.line(x, y).figure)

    def line_svg() -> Callable[[], int]:
        x, y = _line_data(1_000_000)
        return lambda: len(ub.line(x, y).figure.to_svg())

    def line_time() -> Callable[[], int]:
        x, (_, y) = _times(1_000_000), _line_data(1_000_000)

        def build() -> int:
            fig = ub.Figure()
            fig.add_axes(x=ub.Scale.time()).line(x, y)
            return png(fig)

        return build

    def scatter() -> Callable[[], int]:
        x, y = _points(100_000)
        return lambda: png(ub.scatter(x, y).figure)

    def hist() -> Callable[[], int]:
        values, _ = _points(1_000_000)
        return lambda: png(ub.hist(values, bins=30).figure)

    def hist2d() -> Callable[[], int]:
        x, y = _points(1_000_000)

        def build() -> int:
            fig = ub.Figure()
            fig.add_axes().hist2d(x, y, bins=(100, 100))
            return png(fig)

        return build

    def hexbin() -> Callable[[], int]:
        x, y = _hexbin_points(1_000_000)

        def build() -> int:
            fig = ub.Figure()
            fig.add_axes().hexbin(x, y, gridsize=50)
            return png(fig)

        return build

    def heatmap(svg: bool) -> Callable[[], int]:
        values = _cells()
        names = [str(i) for i in range(1000)]

        def build() -> int:
            fig = ub.Figure()
            fig.add_axes(x=ub.Scale.category(), y=ub.Scale.category()).heatmap(names, names, values)
            return len(fig.to_svg()) if svg else png(fig)

        return build

    def grid() -> Callable[[], int]:
        x, y = _line_data(10_000)

        def build() -> int:
            fig = ub.Figure(GRID_WIDTH, GRID_HEIGHT, rows=4, cols=5)
            for cell in fig.cells():
                cell.add_axes().line(x, y)
            return png(fig)

        return build

    def small() -> Callable[[], int]:
        x, y = _line_data(1000)
        y2 = np.cos(x) * 8.0

        def build() -> int:
            ax = ub.line(x, y, label="signal")
            ax.line(x, y2, label="reference")
            ax.title("Signal and reference").xlabel("time (s)").ylabel("amplitude")
            return png(ax.figure)

        return build

    def bars() -> Callable[[], int]:
        names = ["north", "south", "east", "west", "centre"]
        heights = [3.0, 5.0, 2.0, 4.0, 6.0]
        return lambda: png(ub.bar(names, heights).figure)

    return {
        "line_1m_png": line_png,
        "line_1m_svg": line_svg,
        "line_time_1m_png": line_time,
        "scatter_100k_png": scatter,
        "hist_1m_png": hist,
        "hist2d_1m_png": hist2d,
        "hexbin_1m_png": hexbin,
        "heatmap_1000_png": lambda: heatmap(False),
        "heatmap_1000_svg": lambda: heatmap(True),
        "grid_4x5_png": grid,
        "small_line_png": small,
        "bar_5_png": bars,
    }


def _matplotlib_cases() -> dict[str, Callable[[], Callable[[], int]]]:
    """For each case, a setup that builds the data and returns the timed function."""
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    def save(fig: Any, fmt: str) -> int:
        buffer = io.BytesIO()
        fig.savefig(buffer, format=fmt, dpi=DPI)
        plt.close(fig)
        return buffer.tell()

    def single() -> tuple[Any, Any]:
        return plt.subplots(figsize=(WIDTH, HEIGHT))

    def line(fmt: str) -> Callable[[], Callable[[], int]]:
        def setup() -> Callable[[], int]:
            x, y = _line_data(1_000_000)

            def build() -> int:
                fig, ax = single()
                ax.plot(x, y)
                return save(fig, fmt)

            return build

        return setup

    def line_time() -> Callable[[], int]:
        x, (_, y) = _times(1_000_000), _line_data(1_000_000)

        def build() -> int:
            fig, ax = single()
            ax.plot(x, y)
            return save(fig, "png")

        return build

    def scatter() -> Callable[[], int]:
        x, y = _points(100_000)

        def build() -> int:
            fig, ax = single()
            ax.scatter(x, y)
            return save(fig, "png")

        return build

    def hist() -> Callable[[], int]:
        values, _ = _points(1_000_000)

        def build() -> int:
            fig, ax = single()
            ax.hist(values, bins=30)
            return save(fig, "png")

        return build

    def hist2d() -> Callable[[], int]:
        x, y = _points(1_000_000)

        def build() -> int:
            fig, ax = single()
            *_, mappable = ax.hist2d(x, y, bins=(100, 100))
            fig.colorbar(mappable, ax=ax)
            return save(fig, "png")

        return build

    def hexbin() -> Callable[[], int]:
        x, y = _hexbin_points(1_000_000)

        def build() -> int:
            fig, ax = single()
            mappable = ax.hexbin(x, y, gridsize=50, mincnt=1)
            fig.colorbar(mappable, ax=ax)
            return save(fig, "png")

        return build

    def heatmap(fmt: str) -> Callable[[], Callable[[], int]]:
        def setup() -> Callable[[], int]:
            values = _cells()

            def build() -> int:
                fig, ax = single()
                # An SVG embeds the image pixels; without interpolation it holds one pixel
                # per cell, as umber's does, instead of a resampled copy at plot size.
                image = ax.imshow(
                    values, aspect="auto", interpolation="none" if fmt == "svg" else None
                )
                fig.colorbar(image, ax=ax)
                return save(fig, fmt)

            return build

        return setup

    def grid() -> Callable[[], int]:
        x, y = _line_data(10_000)

        def build() -> int:
            fig, axes = plt.subplots(4, 5, figsize=(GRID_WIDTH, GRID_HEIGHT))
            for ax in axes.flat:
                ax.plot(x, y)
            return save(fig, "png")

        return build

    def small() -> Callable[[], int]:
        x, y = _line_data(1000)
        y2 = np.cos(x) * 8.0

        def build() -> int:
            fig, ax = single()
            ax.plot(x, y, label="signal")
            ax.plot(x, y2, label="reference")
            ax.set_title("Signal and reference")
            ax.set_xlabel("time (s)")
            ax.set_ylabel("amplitude")
            ax.legend()
            return save(fig, "png")

        return build

    def bars() -> Callable[[], int]:
        names = ["north", "south", "east", "west", "centre"]
        heights = [3.0, 5.0, 2.0, 4.0, 6.0]

        def build() -> int:
            fig, ax = single()
            ax.bar(names, heights)
            return save(fig, "png")

        return build

    return {
        "line_1m_png": line("png"),
        "line_1m_svg": line("svg"),
        "line_time_1m_png": line_time,
        "scatter_100k_png": scatter,
        "hist_1m_png": hist,
        "hist2d_1m_png": hist2d,
        "hexbin_1m_png": hexbin,
        "heatmap_1000_png": heatmap("png"),
        "heatmap_1000_svg": heatmap("svg"),
        "grid_4x5_png": grid,
        "small_line_png": small,
        "bar_5_png": bars,
    }


def worker(library: str, case: str, runs: int) -> dict[str, Any]:
    """Time one case of one library in this process and return its samples."""
    if library == "umber":
        import umber

        version, setups = umber.__version__, _umber_cases()
    else:
        import matplotlib

        version, setups = matplotlib.__version__, _matplotlib_cases()
    try:
        build = setups[case]()
        size = build()  # warm-up
        times = []
        for _ in range(runs):
            start = time.perf_counter()
            size = build()
            times.append((time.perf_counter() - start) * 1000.0)
    except Exception as error:  # noqa: BLE001 - a case that fails is reported, not hidden
        return {"version": version, "error": f"{type(error).__name__}: {error}"}
    return {"version": version, "runs_ms": times, "output_bytes": size}


def import_time_ms(library: str, runs: int) -> list[float]:
    """Wall times in milliseconds to import a library, each in a fresh interpreter."""
    statement = "import umber" if library == "umber" else "import matplotlib.pyplot"
    code = f"import time; t = time.perf_counter(); {statement}; print(time.perf_counter() - t)"
    times = []
    for _ in range(runs):
        out = subprocess.run(
            [sys.executable, "-c", code], check=True, capture_output=True, text=True
        )
        times.append(float(out.stdout) * 1000.0)
    return times


def spread(samples: list[float]) -> dict[str, float]:
    """The median, the quartiles and the interquartile range of ``samples``."""
    q1, median, q3 = statistics.quantiles(samples, n=4, method="inclusive")
    return {"median_ms": median, "q1_ms": q1, "q3_ms": q3, "iqr_ms": q3 - q1}


def cpu_model() -> str:
    """The CPU model name from ``/proc/cpuinfo``, or the platform's processor string."""
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or "unknown"


def _load() -> list[float] | None:
    """The load averages, or ``None`` where the platform has none."""
    return list(os.getloadavg()) if hasattr(os, "getloadavg") else None


def run(tag: str, out: Path, runs: int) -> int:
    """Time both libraries and write ``out/<tag>.json``; return an exit code.

    The samples come in two batches. In each batch the libraries alternate per case, and the
    second batch swaps the order, so a change of host load reaches both libraries.
    """
    libraries = ("umber", "matplotlib")
    batches, loads = 2, [_load()]
    per_batch = -(-runs // batches)
    samples: dict[str, dict[str, Any]] = {lib: {"version": "", "cases": {}} for lib in libraries}
    imports: dict[str, list[float]] = {lib: [] for lib in libraries}
    for batch in range(batches):
        for index, case in enumerate(CASES):
            order = libraries if (index + batch) % 2 == 0 else libraries[::-1]
            for library in order:
                child = subprocess.run(
                    [sys.executable, __file__, "worker", library, case, "--runs", str(per_batch)],
                    check=True,
                    capture_output=True,
                    text=True,
                )
                result = json.loads(child.stdout)
                samples[library]["version"] = result["version"]
                entry = samples[library]["cases"].setdefault(case, {"runs_ms": []})
                if "error" in result:
                    entry["error"] = result["error"]
                else:
                    entry["runs_ms"] += result["runs_ms"]
                    entry["output_bytes"] = result["output_bytes"]
        for library in libraries if batch % 2 == 0 else libraries[::-1]:
            imports[library] += import_time_ms(library, -(-max(runs, 7) // batches))
        loads.append(_load())
    errors = []
    for library, data in samples.items():
        for case, entry in data["cases"].items():
            if "error" in entry:
                errors.append(f"{library}/{case}: {entry['error']}")
            else:
                entry.update(spread(entry["runs_ms"]))
        data["import_runs_ms"] = imports[library]
        data.update({f"import_{k}": v for k, v in spread(imports[library]).items()})
    affinity = len(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None
    document = {
        "tag": tag,
        "runs": per_batch * batches,
        "machine": {
            "cpu": cpu_model(),
            "cores": os.cpu_count(),
            "cores_usable": affinity,
            "load_average_before_and_after_each_batch": loads,
            "platform": platform.platform(),
            "python": platform.python_version(),
            "numpy": np.__version__,
        },
        "libraries": samples,
    }
    out.mkdir(parents=True, exist_ok=True)
    (out / f"{tag}.json").write_text(json.dumps(document, indent=2) + "\n")
    for line in errors:
        print(f"error: {line}", file=sys.stderr)
    return 1 if errors else 0


def _format_ms(value: float) -> str:
    """A time in milliseconds with a precision that suits its size."""
    return f"{value:,.0f}" if value >= 100 else f"{value:.1f}"


def _cell(entry: dict[str, Any], prefix: str = "") -> str:
    """A median with its interquartile range, such as ``63.5 (IQR 2.1)``."""
    return f"{_format_ms(entry[prefix + 'median_ms'])} (IQR {_format_ms(entry[prefix + 'iqr_ms'])})"


def _table(document: dict[str, Any]) -> list[str]:
    """The Markdown table lines for one run."""
    umber, mpl = document["libraries"]["umber"], document["libraries"]["matplotlib"]
    lines = [
        "| Case | umber, median (IQR) ms | matplotlib, median (IQR) ms | ratio |",
        "| --- | ---: | ---: | ---: |",
    ]
    for name, description in CASES.items():
        a, b = umber["cases"].get(name, {}), mpl["cases"].get(name, {})
        if "median_ms" not in a or "median_ms" not in b:
            lines.append(f"| {description} | error | error | |")
            continue
        lines.append(
            f"| {description} | {_cell(a)} | {_cell(b)} | {b['median_ms'] / a['median_ms']:.1f}x |"
        )
    lines.append(
        f"| import (fresh interpreter) | {_cell(umber, 'import_')} | {_cell(mpl, 'import_')} "
        f"| {mpl['import_median_ms'] / umber['import_median_ms']:.1f}x |"
    )
    return lines


def combine(directory: Path) -> None:
    """Join the runs in ``directory`` into ``results.json`` and ``results.md``."""
    titles = {"multi": "All CPUs", "single": "One CPU (`taskset -c 0`)"}
    documents = {
        path.stem: json.loads(path.read_text())
        for path in sorted(directory.glob("*.json"))
        if path.stem != "results"
    }
    (directory / "results.json").write_text(json.dumps(documents, indent=2) + "\n")
    first = next(iter(documents.values()))
    machine, libraries = first["machine"], first["libraries"]
    lines = [
        "## umber and matplotlib, end to end from Python",
        "",
        f"umber {libraries['umber']['version']}, matplotlib {libraries['matplotlib']['version']}, "
        f"Python {machine['python']}, NumPy {machine['numpy']}.",
        f"CPU: {machine['cpu']}, {machine['cores']} logical cores. {machine['platform']}.",
        f"Median and interquartile range (IQR) of {first['runs']} runs per case, in two "
        "interleaved batches with one warm-up each, each library in its own process. "
        "Ratio is matplotlib's time divided by umber's: above 1, umber is faster.",
        "",
        "Each time is the Python call that builds the figure through the encoded bytes, "
        "6 x 4 in, PNG at 200 dpi, identical data for both. Both libraries run with their "
        "defaults: umber downsamples dense sorted lines and draws large cell grids as one "
        "image; matplotlib simplifies line paths and the heatmap uses `imshow`. matplotlib's "
        "`hist2d` and `hexbin` count their bins in NumPy, and the hexbin row is an approximate "
        "workload because the hexagon grids are built differently. All three colored charts "
        "draw a colorbar. The 4 x 5 grid is 12 x 8 in.",
        "",
    ]
    for tag in ("multi", "single"):
        if tag not in documents:
            continue
        document = documents[tag]
        loads = document["machine"]["load_average_before_and_after_each_batch"]
        usable = document["machine"]["cores_usable"]
        lines += [
            f"### {titles[tag]}",
            "",
            f"CPUs usable: {usable}. 1-minute load average at the start and after each batch: "
            + (", ".join(f"{load[0]:.2f}" for load in loads if load) or "unknown")
            + ".",
            "",
            *_table(document),
            "",
        ]
    (directory / "results.md").write_text("\n".join(lines))
    print("\n".join(lines))


def main() -> int:
    """Parse the command line and run the chosen step."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    runner = sub.add_parser("run", help="time both libraries and write DIR/<tag>.json")
    runner.add_argument("--tag", required=True, help="name of the run, e.g. multi or single")
    runner.add_argument("--out", type=Path, required=True, help="output directory")
    runner.add_argument("--runs", type=int, default=7, help="timed runs per case (default 7)")
    inner = sub.add_parser("worker", help="time one case of one library and print JSON")
    inner.add_argument("library", choices=["umber", "matplotlib"])
    inner.add_argument("case", choices=list(CASES))
    inner.add_argument("--runs", type=int, default=7)
    joiner = sub.add_parser("combine", help="write results.json and results.md from a directory")
    joiner.add_argument("directory", type=Path)
    args = parser.parse_args()
    if args.command == "worker":
        print(json.dumps(worker(args.library, args.case, args.runs)))
        return 0
    if args.command == "combine":
        combine(args.directory)
        return 0
    return run(args.tag, args.out, args.runs)


if __name__ == "__main__":
    sys.exit(main())
