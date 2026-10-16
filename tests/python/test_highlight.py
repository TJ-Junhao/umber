import importlib.abc
import importlib.machinery
import importlib.util
import inspect
import linecache
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from types import FrameType
from typing import ClassVar, Self

import pytest

from umber._ansi import bold, bold_red
from umber._highlight import Highlight, find, render


def _record(found: dict[str, str | None]) -> None:
    """Record, for each parameter of the calling method, where ``find`` locates its argument."""
    frame = inspect.currentframe()
    assert frame is not None
    method = frame.f_back
    assert method is not None
    caller = method.f_back
    assert caller is not None
    code = method.f_code
    names = code.co_varnames[1 : code.co_argcount + code.co_kwonlyargcount]
    arguments = {name: method.f_locals[name] for name in names}
    for parameter in names:
        h = find(caller, method, arguments, parameter)
        found[f"{code.co_name}.{parameter}"] = None if h is None else render(h, color=False)


class Probe:
    """Stands in for an API object; each call records where each argument is found."""

    seen: ClassVar[dict[str, str | None]] = {}
    """What class methods recorded, cleared by each run."""

    def __init__(self) -> None:
        self.found: dict[str, str | None] = {}

    def line(self, x: object, y: object, *, label: object = None) -> Self:
        _record(self.found)
        return self

    def xlim(self, low: object, high: object) -> Self:
        _record(self.found)
        return self

    def title(self, text: object) -> Self:
        _record(self.found)
        return self

    def size(self, width: float = 6.0, height: float = 4.0) -> Self:
        _record(self.found)
        return self

    def plain(self) -> object:
        return self

    @classmethod
    def rgb(cls, r: object, g: object, b: object) -> None:
        _record(cls.seen)


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
    # line is declared to return "Probe", so the receiver of xlim is a Probe.
    assert found["xlim.high"] == "  p.line(t, y).xlim(0, 1)\n                       ^"


def test_chain_through_a_method_returning_self(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    found = _run(tmp_path, monkeypatch, 'def run(p, t, y):\n    p.title("x").xlim(0, -1)\n')
    assert found["title.text"] == '  p.title("x").xlim(0, -1)\n          ^^^'
    assert found["xlim.high"] == '  p.title("x").xlim(0, -1)\n                       ^^'


def test_long_chain(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    source = 'def run(p, t, y):\n    p.title("a").line(t, y).title("b").xlim(0, 1)\n'
    found = _run(tmp_path, monkeypatch, source)
    assert found["xlim.low"] == (
        '  p.title("a").line(t, y).title("b").xlim(0, 1)\n' + " " * 42 + "^"
    )


@pytest.mark.parametrize("receiver", ["p.plain()", "same(p)", "p" + '.title("x")' * 40])
def test_any_receiver_expression_is_accepted(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, receiver: str
) -> None:
    # Only the shape of the call itself matters, never what its receiver is.
    source = f"def same(p):\n    return p\ndef run(p, t, y):\n    {receiver}.xlim(0, 1)\n"
    found = _run(tmp_path, monkeypatch, source)
    shown = f"  {receiver}.xlim(0, 1)"
    assert found["xlim.high"] == f"{shown}\n{' ' * (len(shown) - 2)}^"


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


def test_multi_line_chain_of_calls(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    source = (
        "def run(p, t, y):\n"
        "    (\n"
        "        p.line(t, y)\n"
        '        .title("x")\n'
        "        .xlim(0,\n"
        "              1)\n"
        "    )\n"
    )
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.y"] == "  p.line(t, y)\n            ^"
    assert found["title.text"] == '  .title("x")\n         ^^^'
    assert found["xlim.low"] == "  .xlim(0,\n        ^"
    assert found["xlim.high"] == "  1)\n  ^"


def test_argument_spanning_lines_is_underlined_to_the_end_of_its_first_line(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = "def run(p, t, y):\n    p.line(t, [y[0],\n               y[0]])\n"
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.y"] == "  p.line(t, [y[0],\n            ^^^^^^"


@pytest.mark.parametrize("call", ["p.line(*[t], y)", "p.line(t, *[y])", 'p.line(t, **{"y": y})'])
def test_unpacked_arguments_give_no_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, call: str
) -> None:
    # The shape of a call that unpacks arguments is not known from the source.
    found = _run(tmp_path, monkeypatch, f"def run(p, t, y):\n    {call}\n")
    assert found == {"line.x": None, "line.y": None, "line.label": None}


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
        "    public = inspect.currentframe()\n"
        "    print(find(public.f_back, public, {'x': x, 'y': y}, 'y'))\n"
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
        "    public = inspect.currentframe()\n"
        "    print(find(public.f_back, public, {'x': x, 'y': y}, 'y'))\n"
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
    frame = _frame_here()
    assert find(frame, frame, {}, "y") is None


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
    class FailingLoader(importlib.abc.InspectLoader):
        def get_source(self, fullname: str) -> str:
            raise RuntimeError("the source cannot be read")

    namespace: dict[str, object] = {
        "__name__": "unreadable_caller",
        "__spec__": importlib.machinery.ModuleSpec("unreadable_caller", FailingLoader()),
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


def test_partial_that_binds_a_keyword_gives_no_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # label is left out of the source but no longer holds its default.
    source = (
        "import functools\n"
        "def run(p, t, y):\n"
        '    line = functools.partial(p.line, label="North")\n'
        "    line(t, y)\n"
    )
    found = _run(tmp_path, monkeypatch, source)
    assert found == {"line.x": None, "line.y": None, "line.label": None}


def test_parameters_left_to_their_defaults(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    found = _run(tmp_path, monkeypatch, "def run(p, t, y):\n    p.size(1.0)\n")
    assert found["size.width"] == "  p.size(1.0)\n         ^^^"
    assert found["size.height"] is None


def test_partial_that_binds_a_parameter_with_a_default_gives_no_note(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # The 2.0 in the source is the height; width came from the partial. Binding the source
    # alone would put 2.0 on width, but height then no longer holds its default.
    source = (
        "import functools\n"
        "def run(p, t, y):\n"
        "    size = functools.partial(p.size, 1.0)\n"
        "    size(2.0)\n"
    )
    found = _run(tmp_path, monkeypatch, source)
    assert found == {"size.width": None, "size.height": None}


def test_python_wrapper_is_the_caller(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    # A wrapper written in Python has its own frame, so the call inside it is highlighted.
    source = "def line(y, x, p):\n    p.line(x, y)\ndef run(p, t, y):\n    line(y, t, p)\n"
    found = _run(tmp_path, monkeypatch, source)
    assert found["line.y"] == "  p.line(x, y)\n            ^"
