"""How an umber error looks in IPython, the shell Jupyter runs code in.

IPython formats tracebacks itself. These tests record what it does with the ``__notes__`` that
umber attaches (the hint and the highlighted argument), so the user guide can state it.
"""

import io
from collections.abc import Callable, Iterator
from contextlib import redirect_stderr, redirect_stdout
from typing import cast

import pytest
from IPython.core.interactiveshell import ExecutionResult, InteractiveShell

CELL = """\
import umber as ub
ub.line([0.0, 1.0], [1.0, float("inf")], label="North")
"""
MESSAGE = "Axes.line(): y[1] is inf; values must be finite."
ESCAPE = "\x1b"


@pytest.fixture
def shell() -> Iterator[InteractiveShell]:
    InteractiveShell.clear_instance()
    instance = InteractiveShell.instance()
    instance.colors = "nocolor"
    yield instance
    InteractiveShell.clear_instance()


def _run_cell(shell: InteractiveShell) -> ExecutionResult:
    # IPython ships no annotations for ``run_cell``.
    run = cast(Callable[[str], ExecutionResult], shell.run_cell)  # pyright: ignore[reportUnknownMemberType]
    return run(CELL)


def _run(shell: InteractiveShell) -> tuple[str, str]:
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        result = _run_cell(shell)
    assert isinstance(result.error_in_exec, ValueError)
    return out.getvalue(), err.getvalue()


def test_ipython_shows_the_message(shell: InteractiveShell) -> None:
    out, err = _run(shell)
    assert MESSAGE in out + err


def test_ipython_keeps_the_notes_on_the_exception(shell: InteractiveShell) -> None:
    result = _run_cell(shell)
    error = result.error_in_exec
    assert error is not None
    notes = getattr(error, "__notes__", [])
    assert notes[-1] == "hint: use NaN to leave a gap in the line."


def test_ipython_prints_the_notes(shell: InteractiveShell) -> None:
    # Observed with IPython 9.17: the notes follow the message in the traceback, on stdout.
    out, err = _run(shell)
    assert err == ""
    assert out.endswith(
        f"DataError: {MESSAGE}\n"
        '  ub.line([0.0, 1.0], [1.0, float("inf")], label="North")\n'
        "                      ^^^^^^^^^^^^^^^^^^^\n"
        "hint: use NaN to leave a gap in the line.\n"
    )


def test_force_color_reaches_the_notes_in_ipython(
    shell: InteractiveShell, monkeypatch: pytest.MonkeyPatch
) -> None:
    # IPython prints the notes as they are, so FORCE_COLOR styles them (hint and carets).
    monkeypatch.delenv("NO_COLOR", raising=False)
    monkeypatch.setenv("FORCE_COLOR", "1")
    out, _ = _run(shell)
    assert "\x1b[1mhint:\x1b[0m use NaN to leave a gap in the line." in out
    assert "\x1b[1;31m^^^^^^^^^^^^^^^^^^^\x1b[0m" in out


def test_no_escape_codes_without_force_color(
    shell: InteractiveShell, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("FORCE_COLOR", raising=False)
    monkeypatch.delenv("PYTHON_COLORS", raising=False)
    monkeypatch.setenv("NO_COLOR", "1")
    out, err = _run(shell)
    assert ESCAPE not in out + err
