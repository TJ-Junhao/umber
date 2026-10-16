"""Figures from several threads: parallel figures render as they do serially, and a shared
figure refuses concurrent changes without ever corrupting its state."""

import os
import subprocess
import sys
import sysconfig
import threading
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor

import numpy as np
import pytest

import umber as ub

THEMES = (ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT)
# PyO3's messages when a figure is already borrowed by another thread's call.
BORROW_ERRORS = frozenset({"Already borrowed", "Already mutably borrowed"})
FREE_THREADED = bool(sysconfig.get_config_var("Py_GIL_DISABLED"))


def run_in_threads[T](count: int, work: Callable[[int], T]) -> list[T]:
    """Run ``work(thread)`` on ``count`` threads that start together; results in thread order."""
    start = threading.Barrier(count)

    def task(thread: int) -> T:
        start.wait(timeout=60)
        return work(thread)

    with ThreadPoolExecutor(max_workers=count) as pool:
        futures = [pool.submit(task, thread) for thread in range(count)]
        return [future.result(timeout=300) for future in futures]


def build(index: int) -> ub.Figure:
    """A two-panel figure that differs with ``index`` in theme, data and text."""
    fig = ub.Figure(4, 2.5, cols=2, theme=THEMES[index % len(THEMES)])
    x = np.linspace(0.0, 10.0, 40 + index)
    (
        fig.add_axes(col=0)
        .title(f"Signal {index}")
        .xlabel("Time")
        .line(x, np.sin(x + index), label="sin")
        .scatter(x[::4], np.cos(x[::4] * (1 + index / 10)), label="cos")
        .legend(ub.LegendStyle.CHIPS)
    )
    (
        fig.add_axes(col=1, x=ub.Scale.category())
        .title("Totals")
        .bar(["North", "South", "East"], [1.0 + index, 2.5, 4.0 - index / 10])
    )
    return fig


def render(fig: ub.Figure) -> tuple[bytes, str]:
    return fig.to_png(dpi=40), fig.to_svg()


def test_figures_built_and_rendered_in_parallel_threads() -> None:
    threads, figures = 16, 20
    serial = [render(build(index)) for index in range(figures)]

    def work(thread: int) -> list[tuple[int, tuple[bytes, str]]]:
        # Each thread starts at a different figure, so different figures are built at once.
        order = [(thread + step) % figures for step in range(figures)]
        return [(index, render(build(index))) for index in order]

    for outputs in run_in_threads(threads, work):
        assert len(outputs) == figures
        for index, output in outputs:
            assert output == serial[index], f"figure {index} differs from its serial rendering"


def shared_line(fig: ub.Figure) -> None:
    fig.axes_at().line([0.0, 1.0, 2.0], [1.0, 3.0, 2.0], color=ub.Color.TEAL)


def lines_svg(count: int) -> str:
    fig = ub.Figure(3, 2)
    fig.add_axes().title("Shared")
    for _ in range(count):
        shared_line(fig)
    return fig.to_svg()


def test_one_figure_shared_across_threads_never_crashes() -> None:
    writers, readers, calls = 4, 4, 8
    fig = ub.Figure(3, 2)
    fig.add_axes().title("Shared")
    # Every state the figure can be in: its SVG after each number of added lines.
    states = {lines_svg(count): count for count in range(writers * calls + 1)}

    def work(thread: int) -> tuple[int, list[str], list[str]]:
        added, seen, refused = 0, list[str](), list[str]()
        for _ in range(calls):
            try:
                if thread < writers:
                    shared_line(fig)
                    added += 1
                else:
                    seen.append(fig.to_svg())
            except RuntimeError as err:
                refused.append(str(err))
        return added, seen, refused

    results = run_in_threads(writers + readers, work)
    for _, seen, refused in results:
        assert set(refused) <= BORROW_ERRORS
        for svg in seen:
            assert svg in states, "a render saw a state no sequence of calls produces"
    added = sum(count for count, _, _ in results)
    assert states[fig.to_svg()] == added


@pytest.mark.skipif(not FREE_THREADED, reason="needs a free-threaded interpreter")
def test_import_keeps_the_gil_disabled() -> None:
    # A fresh interpreter, so modules other tests import cannot re-enable the GIL first. An
    # extension that needs the GIL re-enables it with a RuntimeWarning, made an error here.
    env = {name: value for name, value in os.environ.items() if name != "PYTHON_GIL"}
    script = "import sys, umber; print(sys._is_gil_enabled())"
    result = subprocess.run(
        [sys.executable, "-W", "error::RuntimeWarning", "-c", script],
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == "False"
