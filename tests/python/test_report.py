import functools
import importlib.util
import io
import os
import subprocess
import sys
import textwrap
import traceback
from collections.abc import Callable
from pathlib import Path
from typing import Protocol, Self, cast

import pytest

import umber as ub
from umber import _report
from umber._ansi import bold, bold_red
from umber._errors import details, new_error

# A stand-in for the public API: each call checks its arguments the way umber does and wraps
# its body in api_call. Negative numbers are rejected with the parameter's name and a hint.
_FACADE = """
from umber._errors import DataError, UsageError, new_error
from umber._report import api_call


def _check(context, name, value):
    if value < 0:
        raise new_error(
            DataError,
            context,
            f"{name} is {value}; {name} must be 0 or greater.",
            parameter=name,
            hint=f"pass a {name} of 0 or more.",
        )


def f(a, b, *, c=0):
    with api_call("f()", ("a", "b")):
        _check("f()", "a", a)
        _check("f()", "b", b)
        _check("f()", "c", c)


class Axes:
    def line(self, x, y):
        with api_call("Axes.line()", ("x", "y")):
            _check("Axes.line()", "x", x)
            _check("Axes.line()", "y", y)
            return self


class Figure:
    def __init__(self, width, height):
        with api_call("Figure()", ("width", "height")):
            _check("Figure()", "width", width)
            _check("Figure()", "height", height)


def line(x, y):
    with api_call("line()", ("x", "y")):
        return Axes().line(x, y)


def without_parameter():
    with api_call("without_parameter()", ()):
        raise new_error(
            UsageError,
            "without_parameter()",
            "this call is not supported; call it differently.",
            parameter=None,
            hint="call it differently.",
        )


def without_hint(a):
    with api_call("without_hint()", ("a",)):
        raise new_error(
            DataError, "without_hint()", "a is bad; a must be good.", parameter="a", hint=None
        )


def wrong_type(a):
    with api_call("wrong_type()", ("a",)):
        raise new_error(
            TypeError,
            "wrong_type()",
            "a is str; a must be a float.",
            parameter="a",
            hint="pass a number.",
        )


def unexpected(kind):
    with api_call("unexpected()", ("kind",)):
        raise kind("not raised by umber on purpose")


def apply(fn):
    with api_call("apply()", ("fn",)):
        fn()
"""


class _Axes(Protocol):
    def line(self, x: int, y: int) -> Self: ...


class Facade(Protocol):
    Axes: Callable[[], _Axes]
    Figure: Callable[[int, int], object]

    def f(self, a: int, b: int, *, c: int = 0) -> None: ...
    def line(self, x: int, y: int) -> _Axes: ...
    def without_parameter(self) -> None: ...
    def without_hint(self, a: int) -> None: ...
    def wrong_type(self, a: object) -> None: ...
    def unexpected(self, kind: type[Exception]) -> None: ...
    def apply(self, fn: Callable[[], object]) -> None: ...


def _write_facade(directory: Path) -> Path:
    package = directory / "facade_pkg"
    package.mkdir()
    (package / "facade.py").write_text(_FACADE, encoding="utf-8")
    return package


@pytest.fixture
def no_color(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("PYTHON_COLORS", raising=False)
    monkeypatch.delenv("FORCE_COLOR", raising=False)
    monkeypatch.setenv("NO_COLOR", "1")


@pytest.fixture
def facade(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, no_color: None) -> Facade:
    package = _write_facade(tmp_path)
    monkeypatch.setattr(_report, "_PACKAGE_DIR", package)
    spec = importlib.util.spec_from_file_location("facade", package / "facade.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    monkeypatch.setitem(sys.modules, "facade", module)
    spec.loader.exec_module(module)
    return cast(Facade, module)


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _frames(err: BaseException) -> list[str]:
    return [frame.name for frame in traceback.extract_tb(err.__traceback__)]


def test_notes_are_the_highlight_then_the_hint(facade: Facade) -> None:
    f = facade.f
    a, b = 1, -1
    with pytest.raises(ub.DataError) as info:
        f(a, b)
    assert str(info.value) == "f(): b is -1; b must be 0 or greater."
    assert _notes(info.value) == ["  f(a, b)\n       ^", "hint: pass a b of 0 or more."]


def test_keyword_argument_is_highlighted(facade: Facade) -> None:
    a, b, c = 1, 2, -3
    with pytest.raises(ub.DataError) as info:
        facade.f(a, b, c=c)
    assert _notes(info.value) == [
        "  facade.f(a, b, c=c)\n                 ^^^",
        "hint: pass a c of 0 or more.",
    ]


def test_traceback_holds_only_the_callers_frame(facade: Facade) -> None:
    with pytest.raises(ub.DataError) as info:
        facade.f(1, -1)
    assert _frames(info.value) == ["test_traceback_holds_only_the_callers_frame"]


def test_traceback_holds_every_user_frame_and_no_package_frame(facade: Facade) -> None:
    def plot() -> None:
        facade.line(1, -1)

    with pytest.raises(ub.DataError) as info:
        plot()
    assert _frames(info.value) == [
        "test_traceback_holds_every_user_frame_and_no_package_frame",
        "plot",
    ]


def test_method_call(facade: Facade) -> None:
    ax = facade.Axes()
    t, y = 1, -1
    with pytest.raises(ub.DataError) as info:
        ax.line(t, y)
    assert _notes(info.value) == ["  ax.line(t, y)\n             ^", "hint: pass a y of 0 or more."]
    assert _frames(info.value) == ["test_method_call"]


def test_chained_call_on_a_call_result_gets_only_the_hint(facade: Facade) -> None:
    # The receiver of the failing call is the result of another call, so the call target
    # cannot be proven without running code, and the highlight is left out.
    ax = facade.Axes()
    t, y = 1, -1
    with pytest.raises(ub.DataError) as info:
        ax.line(t, t).line(t, y)
    assert _notes(info.value) == ["hint: pass a y of 0 or more."]
    assert _frames(info.value) == ["test_chained_call_on_a_call_result_gets_only_the_hint"]


def test_partial_with_the_same_name_gets_only_the_hint(facade: Facade) -> None:
    line = functools.partial(facade.line, -1)
    y = 2
    with pytest.raises(ub.DataError) as info:
        line(y)
    assert str(info.value) == "Axes.line(): x is -1; x must be 0 or greater."
    assert _notes(info.value) == ["hint: pass a x of 0 or more."]


def test_constructor_call(facade: Facade) -> None:
    width, height = 4, -3
    with pytest.raises(ub.DataError) as info:
        facade.Figure(width, height)
    assert _notes(info.value) == [
        "  facade.Figure(width, height)\n                       ^^^^^^",
        "hint: pass a height of 0 or more.",
    ]


def test_nested_call_reports_once_at_the_outer_call(facade: Facade) -> None:
    line = facade.line
    t, y = 1, -1
    with pytest.raises(ub.DataError) as info:
        line(t, y)
    # The validating method names itself in the message; the highlight is on the user's call.
    assert str(info.value) == "Axes.line(): y is -1; y must be 0 or greater."
    assert _notes(info.value) == ["  line(t, y)\n          ^", "hint: pass a y of 0 or more."]
    assert _frames(info.value) == ["test_nested_call_reports_once_at_the_outer_call"]


def test_error_already_reported_gets_no_second_notes(facade: Facade) -> None:
    a, b = 1, -1
    with pytest.raises(ub.DataError) as info:
        facade.apply(lambda: facade.f(a, b))
    assert _notes(info.value) == [
        "  facade.apply(lambda: facade.f(a, b))\n                                   ^",
        "hint: pass a b of 0 or more.",
    ]
    # The user's callback stays in the traceback; the package frames around it do not.
    assert _frames(info.value) == ["test_error_already_reported_gets_no_second_notes", "<lambda>"]


def test_user_callback_frame_is_kept_when_the_error_passes_the_package_again(
    facade: Facade,
) -> None:
    def user_callback() -> None:
        facade.line(1, -1)

    with pytest.raises(ub.DataError) as info:
        facade.apply(user_callback)
    assert _frames(info.value) == [
        "test_user_callback_frame_is_kept_when_the_error_passes_the_package_again",
        "user_callback",
    ]
    assert _notes(info.value) == [
        "  facade.line(1, -1)\n                 ^^",
        "hint: pass a y of 0 or more.",
    ]


def test_error_without_parameter_gets_only_the_hint(facade: Facade) -> None:
    with pytest.raises(ub.UsageError) as info:
        facade.without_parameter()
    assert _notes(info.value) == ["hint: call it differently."]
    assert _frames(info.value) == ["test_error_without_parameter_gets_only_the_hint"]


def test_error_without_hint_gets_only_the_highlight(facade: Facade) -> None:
    a = 1
    with pytest.raises(ub.DataError) as info:
        facade.without_hint(a)
    assert _notes(info.value) == ["  facade.without_hint(a)\n                      ^"]


def test_builtin_type_error_raised_by_umber_is_reported(facade: Facade) -> None:
    a = "1"
    with pytest.raises(TypeError) as info:
        facade.wrong_type(a)
    assert str(info.value) == "wrong_type(): a is str; a must be a float."
    assert _notes(info.value) == [
        "  facade.wrong_type(a)\n                    ^",
        "hint: pass a number.",
    ]
    assert _frames(info.value) == ["test_builtin_type_error_raised_by_umber_is_reported"]


@pytest.mark.parametrize("kind", [ValueError, OSError, TypeError, ub.DataError])
def test_exception_not_raised_by_umber_keeps_its_traceback_and_gets_no_notes(
    facade: Facade, kind: type[Exception]
) -> None:
    with pytest.raises(kind) as info:
        facade.unexpected(kind)
    assert _notes(info.value) == []
    assert not hasattr(info.value, "_umber_located")
    assert _frames(info.value) == [
        "test_exception_not_raised_by_umber_keeps_its_traceback_and_gets_no_notes",
        "unexpected",
    ]


def test_color_styles_the_notes_but_never_the_message(
    facade: Facade, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("NO_COLOR")
    monkeypatch.setenv("FORCE_COLOR", "1")
    f = facade.f
    a, b = 1, -1
    with pytest.raises(ub.DataError) as info:
        f(a, b)
    assert "\x1b" not in str(info.value)
    assert "\x1b" not in repr(info.value)
    assert _notes(info.value) == [
        f"  f(a, {bold('b')})\n       {bold_red('^')}",
        f"{bold('hint:')} pass a b of 0 or more.",
    ]


def test_color_is_decided_when_the_error_is_raised(
    facade: Facade, monkeypatch: pytest.MonkeyPatch
) -> None:
    with pytest.raises(ub.DataError) as plain:
        facade.f(1, -1)
    monkeypatch.setenv("PYTHON_COLORS", "1")
    with pytest.raises(ub.DataError) as styled:
        facade.f(1, -1)
    assert "\x1b" not in "".join(_notes(plain.value))
    assert "\x1b" in "".join(_notes(styled.value))


def test_call_from_inside_the_package_is_not_reported(
    facade: Facade, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # A module in the package directory that calls the public function without api_call of
    # its own: the call is a nested one, so nothing reports it.
    (tmp_path / "facade_pkg" / "inside.py").write_text(
        "import facade\ndef run():\n    facade.f(1, -1)\n", encoding="utf-8"
    )
    spec = importlib.util.spec_from_file_location("inside", tmp_path / "facade_pkg" / "inside.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    monkeypatch.setitem(sys.modules, "inside", module)
    spec.loader.exec_module(module)
    run: Callable[[], None] = module.__dict__["run"]
    with pytest.raises(ub.DataError) as info:
        run()
    assert _notes(info.value) == []
    assert not hasattr(info.value, "_umber_located")
    assert _frames(info.value) == ["test_call_from_inside_the_package_is_not_reported", "run"]


class _StreamWithoutIsatty:
    """A replacement stderr whose fileno() has no descriptor and which has no isatty()."""

    def write(self, text: str) -> int:
        return len(text)

    def flush(self) -> None:
        pass

    def fileno(self) -> int:
        raise io.UnsupportedOperation("fileno")


class _FailingStream:
    """A replacement stderr whose every query fails."""

    def write(self, text: str) -> int:
        return len(text)

    def fileno(self) -> int:
        raise RuntimeError("no descriptor")

    def isatty(self) -> bool:
        raise RuntimeError("no terminal")


@pytest.mark.parametrize("stream", [_StreamWithoutIsatty(), _FailingStream()])
def test_unusual_stderr_never_replaces_the_error(
    facade: Facade, monkeypatch: pytest.MonkeyPatch, stream: object
) -> None:
    monkeypatch.delenv("NO_COLOR")
    monkeypatch.delenv("TERM", raising=False)
    monkeypatch.setattr(sys, "stderr", stream)
    a, b = 1, -1
    with pytest.raises(ub.DataError) as info:
        facade.f(a, b)
    assert _notes(info.value) == [
        "  facade.f(a, b)\n              ^",
        "hint: pass a b of 0 or more.",
    ]


def test_the_package_directory_is_umber() -> None:
    assert (_report._PACKAGE_DIR / "_report.py").is_file()  # pyright: ignore[reportPrivateUsage]


def _run_script(tmp_path: Path, body: str, *flags: str) -> tuple[Path, str]:
    """Run ``body`` as a script next to the fake façade package; return its path and stderr."""
    package = _write_facade(tmp_path)
    script = tmp_path / "plot.py"
    header = textwrap.dedent(
        f"""\
        import sys
        from pathlib import Path
        from umber import _report
        _report._PACKAGE_DIR = Path({str(package)!r})
        sys.path.insert(0, {str(package)!r})
        from facade import f
        """
    )
    script.write_text(header + body, encoding="utf-8")
    env = {k: v for k, v in os.environ.items() if k not in ("PYTHON_COLORS", "FORCE_COLOR")}
    env["NO_COLOR"] = "1"
    result = subprocess.run(
        [sys.executable, *flags, str(script)], capture_output=True, text=True, env=env
    )
    assert result.returncode == 1
    return script, result.stderr


def test_printed_error_shows_the_users_line_message_and_notes(tmp_path: Path) -> None:
    script, stderr = _run_script(tmp_path, "a, b = 1, -1\nf(a, b)\n")
    name = f"{ub.DataError.__module__}.{ub.DataError.__qualname__}"
    # Python 3.13 and later underline the call itself in the traceback; 3.12 does not.
    underline = "    ~^^^^^^\n" if sys.version_info >= (3, 13) else ""
    assert stderr == (
        "Traceback (most recent call last):\n"
        f'  File "{script}", line 8, in <module>\n'
        "    f(a, b)\n"
        f"{underline}"
        f"{name}: f(): b is -1; b must be 0 or greater.\n"
        "  f(a, b)\n"
        "       ^\n"
        "hint: pass a b of 0 or more.\n"
    )


def test_printed_error_without_position_information_keeps_the_hint(tmp_path: Path) -> None:
    _, stderr = _run_script(tmp_path, "a, b = 1, -1\nf(a, b)\n", "-X", "no_debug_ranges")
    assert stderr.endswith(
        ": f(): b is -1; b must be 0 or greater.\nhint: pass a b of 0 or more.\n"
    )


def test_new_error_builds_the_message_and_details() -> None:
    err = new_error(
        ub.StyleError,
        "Color.alpha()",
        "a is 2; a must be between 0 and 1.",
        parameter="a",
        hint="x",
    )
    assert isinstance(err, ub.StyleError)
    assert str(err) == "Color.alpha(): a is 2; a must be between 0 and 1."
    assert details(err) == ("a", "x")


def test_new_error_accepts_a_builtin_class_and_missing_details() -> None:
    err = new_error(TypeError, "f()", "a is str; a must be a float.", parameter=None, hint=None)
    assert type(err) is TypeError
    assert details(err) == (None, None)


def test_details_of_other_exceptions_is_none() -> None:
    assert details(ValueError("x")) is None
    assert details(ub.DataError("Axes.line(): y[3] is inf; values must be finite.")) is None
