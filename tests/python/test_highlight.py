import importlib.util
import inspect
import linecache
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from types import FrameType

import pytest

from umber._ansi import bold, bold_red
from umber._highlight import Highlight, find, render


class Probe:
    """Stands in for an API object; each call records where each argument is found."""

    def __init__(self) -> None:
        self.found: dict[str, str | None] = {}

    def _record(self, callee: str, positional: tuple[str, ...], keywords: tuple[str, ...]) -> None:
        frame = inspect.currentframe()
        assert frame is not None and frame.f_back is not None
        caller = frame.f_back.f_back
        assert caller is not None
        for parameter in positional + keywords:
            h = find(caller, callee, positional, parameter)
            self.found[f"{callee}.{parameter}"] = None if h is None else render(h, color=False)

    def line(self, x: object, y: object, *, label: object = None) -> "Probe":
        self._record("line", ("x", "y"), ("label",))
        return self

    def xlim(self, low: object, high: object) -> "Probe":
        self._record("xlim", ("low", "high"), ())
        return self


def _run(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, source: str) -> dict[str, str | None]:
    """Write ``source`` (defining ``run(p, t, y)``) to a module, run it, return what was found."""
    path = tmp_path / "caller.py"
    path.write_text(source, encoding="utf-8")
    linecache.checkcache(str(path))
    spec = importlib.util.spec_from_file_location("caller", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    monkeypatch.setitem(sys.modules, "caller", module)
    spec.loader.exec_module(module)
    run: Callable[[Probe, object, object], object] = module.__dict__["run"]
    probe = Probe()
    run(probe, [1.0], [2.0])
    return probe.found


def test_positional_arguments(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    found = _run(tmp_path, monkeypatch, "def run(p, t, y):\n    p.line(t, y)\n")
    assert found["line.x"] == "  p.line(t, y)\n         ^"
    assert found["line.y"] == "  p.line(t, y)\n            ^"
    assert found["line.label"] is None


def test_keyword_argument_is_highlighted_whole(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    found = _run(tmp_path, monkeypatch, 'def run(p, t, y):\n    p.line(t, y=y, label="North")\n')
    assert found["line.y"] == '  p.line(t, y=y, label="North")\n            ^^^'
    assert found["line.label"] == '  p.line(t, y=y, label="North")\n                 ^^^^^^^^^^^^^'


def test_wide_argument_is_underlined_across_its_width(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    found = _run(tmp_path, monkeypatch, "def run(p, t, y):\n    p.line(t[0:1], y)\n")
    assert found["line.x"] == "  p.line(t[0:1], y)\n         ^^^^^^"


def test_chained_calls_highlight_inside_the_failing_call(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    found = _run(tmp_path, monkeypatch, "def run(p, t, y):\n    p.line(t, y).xlim(0, 1)\n")
    assert found["line.y"] == "  p.line(t, y).xlim(0, 1)\n            ^"
    assert found["xlim.high"] == "  p.line(t, y).xlim(0, 1)\n                       ^"


def test_multi_line_call(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    source = (
        'def run(p, t, y):\n    p.line(\n        t,\n        y,\n        label="North",\n    )\n'
    )
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.x"] == "  t,\n  ^"
    assert found["line.y"] == "  y,\n  ^"
    assert found["line.label"] == '  label="North",\n  ^^^^^^^^^^^^^'


def test_multi_line_chain_highlights_the_method_on_its_own_line(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = "def run(p, t, y):\n    (p.line(t, y)\n        .xlim(0,\n              1))\n"
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.y"] == "  (p.line(t, y)\n             ^"
    assert found["xlim.low"] == "  .xlim(0,\n        ^"
    assert found["xlim.high"] == "  1))\n  ^"


def test_argument_spanning_lines_is_underlined_to_the_end_of_its_first_line(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = "def run(p, t, y):\n    p.line(t, [y[0],\n               y[0]])\n"
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.y"] == "  p.line(t, [y[0],\n            ^^^^^^"


def test_starred_argument_before_the_parameter_gives_no_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    found = _run(tmp_path, monkeypatch, "def run(p, t, y):\n    p.line(*[t], y)\n")
    assert found["line.x"] is None
    assert found["line.y"] is None


def test_starred_argument_after_the_parameter_keeps_the_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    found = _run(tmp_path, monkeypatch, "def run(p, t, y):\n    p.line(t, *[y])\n")
    assert found["line.x"] == "  p.line(t, *[y])\n         ^"
    assert found["line.y"] is None


def test_keyword_unpacking_gives_no_note(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    found = _run(tmp_path, monkeypatch, 'def run(p, t, y):\n    p.line(t, **{"y": y})\n')
    assert found["line.x"] == '  p.line(t, **{"y": y})\n         ^'
    assert found["line.y"] is None


def test_callee_name_mismatch_gives_no_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    found = _run(tmp_path, monkeypatch, "def run(p, t, y):\n    draw = p.line\n    draw(t, y)\n")
    assert found["line.y"] is None


def test_call_through_map_gives_no_note(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    found = _run(tmp_path, monkeypatch, "def run(p, t, y):\n    list(map(p.line, [t], [y]))\n")
    assert found["line.y"] is None


def test_non_ascii_and_wide_characters(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    source = 'def run(p, t, y):\n    p.line(["数据", "ü"], y, label="北")\n'
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.x"] == '  p.line(["数据", "ü"], y, label="北")\n         ^^^^^^^^^^^^^'
    assert found["line.y"] == '  p.line(["数据", "ü"], y, label="北")\n                        ^'
    assert found["line.label"] == (
        '  p.line(["数据", "ü"], y, label="北")\n                           ^^^^^^^^^^'
    )


def test_tabs_are_single_spaces(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    found = _run(tmp_path, monkeypatch, "def run(p, t, y):\n\tp.line(t,\t\ty)\n")
    assert found["line.y"] == "  p.line(t,  y)\n             ^"


def test_indentation_is_removed(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    source = "def run(p, t, y):\n    if True:\n        if True:\n            p.line(t, y)\n"
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.y"] == "  p.line(t, y)\n            ^"


def test_missing_source_file_gives_no_note(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    path = tmp_path / "gone.py"
    path.write_text("def run(p, t, y):\n    p.line(t, y)\n", encoding="utf-8")
    spec = importlib.util.spec_from_file_location("gone", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    monkeypatch.setitem(sys.modules, "gone", module)
    spec.loader.exec_module(module)
    path.unlink()
    linecache.checkcache(str(path))
    run: Callable[[Probe, object, object], object] = module.__dict__["run"]
    probe = Probe()
    run(probe, [1.0], [2.0])
    assert probe.found["line.y"] is None


def test_source_from_the_command_line_gives_no_note() -> None:
    script = (
        "import inspect\n"
        "from umber._highlight import find\n"
        "def line(x, y):\n"
        "    frame = inspect.currentframe().f_back\n"
        "    print(find(frame, 'line', ('x', 'y'), 'y'))\n"
        "line(1, 2)\n"
    )
    out = subprocess.run(
        [sys.executable, "-c", script], check=True, capture_output=True, text=True
    ).stdout
    assert out == "None\n"


def test_missing_position_information_gives_no_note(tmp_path: Path) -> None:
    path = tmp_path / "script.py"
    path.write_text(
        "import inspect\n"
        "from umber._highlight import find\n"
        "def line(x, y):\n"
        "    frame = inspect.currentframe().f_back\n"
        "    print(find(frame, 'line', ('x', 'y'), 'y'))\n"
        "line(1, 2)\n",
        encoding="utf-8",
    )
    command = [sys.executable, "-X", "no_debug_ranges", str(path)]
    out = subprocess.run(command, check=True, capture_output=True, text=True).stdout
    assert out == "None\n"
    command.remove("no_debug_ranges")
    command.remove("-X")
    out = subprocess.run(command, check=True, capture_output=True, text=True).stdout
    assert out.startswith("Highlight(")


def test_the_test_file_itself_is_a_valid_caller() -> None:
    p = Probe()
    p.line([1.0], [2.0], label="North")
    assert p.found["line.label"] == (
        '  p.line([1.0], [2.0], label="North")\n                       ^^^^^^^^^^^^^'
    )


def _frame_here() -> FrameType:
    frame = inspect.currentframe()
    assert frame is not None
    return frame


def test_render_without_color() -> None:
    h = Highlight(line="f(a, b)", start=5, end=6)
    assert render(h, color=False) == "  f(a, b)\n       ^"


def test_render_with_color_styles_the_argument_and_the_carets() -> None:
    h = Highlight(line="f(a, bc)", start=5, end=7)
    assert render(h, color=True) == f"  f(a, {bold('bc')})\n       {bold_red('^^')}"


def test_render_with_color_counts_wide_characters_by_display_column() -> None:
    h = Highlight(line='f("数", b)', start=8, end=9)
    assert render(h, color=True) == f'  f("数", {bold("b")})\n          {bold_red("^")}'


def test_find_rejects_a_frame_not_in_a_call() -> None:
    assert find(_frame_here(), "line", ("x", "y"), "y") is None
