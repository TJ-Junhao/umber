import importlib.util
import inspect
import linecache
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from types import FrameType
from typing import ClassVar

import pytest

from umber._ansi import bold, bold_red
from umber._highlight import Highlight, find, render


def _record(
    found: dict[str, str | None],
    callee: str,
    positional: tuple[str, ...],
    keywords: tuple[str, ...],
) -> None:
    """Record, for each parameter, where ``find`` locates it in the call of the calling method."""
    frame = inspect.currentframe()
    assert frame is not None
    method = frame.f_back
    assert method is not None
    caller = method.f_back
    assert caller is not None
    for parameter in positional + keywords:
        h = find(caller, callee, method.f_code, positional, parameter)
        found[f"{callee}.{parameter}"] = None if h is None else render(h, color=False)


class Probe:
    """Stands in for an API object; each call records where each argument is found."""

    seen: ClassVar[dict[str, str | None]] = {}
    """What class methods recorded, cleared by each run."""

    def __init__(self) -> None:
        self.found: dict[str, str | None] = {}

    def line(self, x: object, y: object, *, label: object = None) -> "Probe":
        _record(self.found, "line", ("x", "y"), ("label",))
        return self

    def xlim(self, low: object, high: object) -> "Probe":
        _record(self.found, "xlim", ("low", "high"), ())
        return self

    @classmethod
    def rgb(cls, r: object, g: object, b: object) -> None:
        _record(cls.seen, "rgb", ("r", "g", "b"), ())


def _run(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, source: str) -> dict[str, str | None]:
    """Write ``source`` (defining ``run(p, t, y)``) to a module, run it, return what was found."""
    path = tmp_path / "caller.py"
    path.write_text(source, encoding="utf-8")
    linecache.checkcache(str(path))
    spec = importlib.util.spec_from_file_location("caller", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    # Imported here, the test module would be a second copy with its own Probe class.
    module.__dict__["Probe"] = Probe
    monkeypatch.setitem(sys.modules, "caller", module)
    spec.loader.exec_module(module)
    run: Callable[[Probe, object, object], object] = module.__dict__["run"]
    probe = Probe()
    Probe.seen.clear()
    run(probe, [1.0], [2.0])
    return probe.found | Probe.seen


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
    # The receiver of xlim is the result of a call, which cannot be checked without running
    # it, so the call target is not provable and the note is left out.
    assert found["xlim.high"] is None


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
    source = "def run(p, t, y):\n    (p\n        .xlim(0,\n              1))\n"
    found = _run(tmp_path, monkeypatch, source)
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
        "    print(find(frame, 'line', line.__code__, ('x', 'y'), 'y'))\n"
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
        "    print(find(frame, 'line', line.__code__, ('x', 'y'), 'y'))\n"
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
    assert find(_frame_here(), "line", _frame_here.__code__, ("x", "y"), "y") is None


def test_alias_with_the_same_name_is_a_direct_call(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    found = _run(tmp_path, monkeypatch, "def run(p, t, y):\n    line = p.line\n    line(t, y)\n")
    assert found["line.y"] == "  line(t, y)\n          ^"


def test_partial_with_the_same_name_gives_no_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # The partial binds x, so the y in the source is the first argument of the partial; a
    # positional mapping would point at the wrong argument.
    source = (
        "import functools\n"
        "def run(p, t, y):\n"
        "    line = functools.partial(p.line, t)\n"
        "    line(y)\n"
    )
    found = _run(tmp_path, monkeypatch, source)
    assert found == {"line.x": None, "line.y": None, "line.label": None}


def test_partial_of_a_class_method_with_the_same_name_gives_no_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = (
        "import functools\n"
        "def run(p, t, y):\n"
        "    rgb = functools.partial(Probe.rgb, 0)\n"
        "    rgb(300, 255)\n"
    )
    found = _run(tmp_path, monkeypatch, source)
    assert found == {"rgb.r": None, "rgb.g": None, "rgb.b": None}


def test_class_method_called_through_the_class(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = "def run(p, t, y):\n    Probe.rgb(0, 300, 255)\n"
    found = _run(tmp_path, monkeypatch, source)
    assert found["rgb.g"] == "  Probe.rgb(0, 300, 255)\n               ^^^"


def test_method_called_unbound_through_the_class_gives_no_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = "def run(p, t, y):\n    Probe.line(p, t, y)\n"
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.y"] is None


def test_method_replaced_on_the_instance_gives_no_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = (
        "import functools\n"
        "class Holder:\n"
        "    pass\n"
        "def run(p, t, y):\n"
        "    h = Holder()\n"
        "    h.line = functools.partial(p.line, t)\n"
        "    h.line(y)\n"
    )
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.y"] is None


def test_combining_mark_does_not_shift_the_carets(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # "e" followed by U+0301 COMBINING ACUTE ACCENT takes one column.
    source = 'def run(p, t, y):\n    p.line(["e\u0301"], y)\n'
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.y"] == '  p.line(["e\u0301"], y)\n                ^'
    assert found["line.x"] == '  p.line(["e\u0301"], y)\n         ^^^^^'


def test_line_with_a_joined_emoji_gives_no_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # A family emoji is several code points joined by U+200D; terminals draw it in one cell
    # pair or several, so no column count is reliable.
    source = 'def run(p, t, y):\n    p.line(["\U0001f468\u200d\U0001f469\u200d\U0001f467"], y)\n'
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.y"] is None


def test_render_with_color_keeps_combining_marks_inside_the_styled_span() -> None:
    h = Highlight(line='f("e\u0301")', start=2, end=5)
    assert (
        render(h, color=True)
        == f"  f({bold(chr(34) + 'e' + chr(0x301) + chr(34))})\n    {bold_red('^^^')}"
    )


def test_unreadable_source_gives_no_note() -> None:
    class FailingLoader:
        def get_source(self, name: str) -> str:
            raise RuntimeError("the source cannot be read")

    namespace: dict[str, object] = {
        "__name__": "unreadable_caller",
        "__loader__": FailingLoader(),
        "Probe": Probe,
    }
    code = compile("def run(p):\n    p.line(1, 2)\n", "unreadable_caller_source.py", "exec")
    exec(code, namespace)
    linecache.checkcache("unreadable_caller_source.py")
    run = namespace["run"]
    assert callable(run)
    probe = Probe()
    run(probe)
    assert probe.found["line.y"] is None
