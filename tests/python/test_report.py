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
from typing import Self

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
    with api_call():
        _check("f()", "a", a)
        _check("f()", "b", b)
        _check("f()", "c", c)


class Axes:
    def line(self, x, y) -> Self:
        with api_call():
            _check("Axes.line()", "x", x)
            _check("Axes.line()", "y", y)
            return self

    def title(self, text) -> Self:
        with api_call():
            return self

    def xlim(self, low, high) -> "Axes":
        with api_call():
            _check("Axes.xlim()", "low", low)
            _check("Axes.xlim()", "high", high)
            return self

    def unannotated(self):
        return self


class Figure:
    def __init__(self, width, height):
        with api_call():
            _check("Figure()", "width", width)
            _check("Figure()", "height", height)


def line(x, y):
    with api_call():
        return Axes().line(x, y)


def without_parameter():
    with api_call():
        raise new_error(
            UsageError,
            "without_parameter()",
            "this call is not supported; call it differently.",
            parameter=None,
            hint="call it differently.",
        )


def without_hint(a):
    with api_call():
        raise new_error(
            DataError, "without_hint()", "a is bad; a must be good.", parameter="a", hint=None
        )


def wrong_type(a):
    with api_call():
        raise new_error(
            TypeError,
            "wrong_type()",
            "a is str; a must be a float.",
            parameter="a",
            hint="pass a number.",
        )


def unexpected(kind):
    with api_call():
        raise kind("not raised by umber on purpose")


def rebinding(a, *, scale=1.0):
    with api_call():
        scale = scale * 2
        _check("rebinding()", "a", a)


EVENTS = []


def _side_effect():
    EVENTS.append("annotation evaluated")
    return Self


class Annotated(Axes):
    def title(self, text: _side_effect()) -> _side_effect():
        with api_call():
            return self


def apply(fn):
    with api_call():
        fn()
"""


class _Axes(Protocol):
    def line(self, x: int, y: int) -> Self: ...
    def title(self, text: str) -> Self: ...
    def xlim(self, low: int, high: int) -> Self: ...
    def unannotated(self) -> Self: ...


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
    def rebinding(self, a: int, *, scale: float = 1.0) -> None: ...


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


def test_chained_call_highlights_inside_the_outer_call(facade: Facade) -> None:
    ax = facade.Axes()
    t, y = 1, -1
    with pytest.raises(ub.DataError) as info:
        ax.line(t, t).line(t, y)
    assert _notes(info.value) == [
        "  ax.line(t, t).line(t, y)\n                        ^",
        "hint: pass a y of 0 or more.",
    ]
    assert _frames(info.value) == ["test_chained_call_highlights_inside_the_outer_call"]


def test_chained_setter_highlights_the_right_argument(facade: Facade) -> None:
    ax = facade.Axes()
    with pytest.raises(ub.DataError) as info:
        ax.title("x").xlim(0, -1)
    assert _notes(info.value) == [
        '  ax.title("x").xlim(0, -1)\n                        ^^',
        "hint: pass a high of 0 or more.",
    ]


def test_multi_line_chain_highlights_the_right_argument(facade: Facade) -> None:
    ax = facade.Axes()
    t, y = 1, -1
    with pytest.raises(ub.DataError) as info:
        (
            ax.title("x")
            .line(t, t)
            .xlim(0, y)
        )  # fmt: skip
    assert _notes(info.value) == ["  .xlim(0, y)\n           ^", "hint: pass a high of 0 or more."]


def test_chain_through_any_method_highlights_the_right_argument(facade: Facade) -> None:
    # Only the shape of the failing call matters, not what its receiver was returned by.
    ax = facade.Axes()
    with pytest.raises(ub.DataError) as info:
        ax.unannotated().xlim(0, -1)
    assert _notes(info.value) == [
        "  ax.unannotated().xlim(0, -1)\n                           ^^",
        "hint: pass a high of 0 or more.",
    ]


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


# Callers that reach a public function through dynamic name resolution, attribute hooks,
# wrappers or rebinding. Each must either highlight the argument that really caused the error
# or leave the highlight out; never point at another argument or replace the error.
_HINT_ONLY = {
    "local_partial": """
        def run():
            line = partial(facade.line, -1)
            line(123)
    """,
    "closure_partial": """
        def run():
            line = partial(facade.line, -1)
            def inner():
                line(123)
            inner()
    """,
    "global_partial": """
        line = partial(facade.line, -1)
        def run():
            global line
            line(123)
    """,
    "class_body": """
        def run():
            class C:
                line = partial(facade.line, -1)
                line(123)
    """,
    "class_closure": """
        line = facade.line
        def run():
            line = partial(facade.line, -1)
            class C:
                line(123)
    """,
    "comprehension": """
        line = facade.line
        def run():
            [line(123) for line in [partial(facade.line, -1)]]
    """,
    "property": """
        class Holder:
            @property
            def line(self):
                return partial(facade.line, -1)
        def run():
            Holder().line(123)
    """,
    "module_getattr": """
        m = ModuleType("fake")
        m.__getattr__ = lambda name: partial(facade.line, -1)
        def run():
            m.line(123)
    """,
    "instance_getattribute": """
        class Child(facade.Axes):
            __slots__ = ()
            def __getattribute__(self, name):
                if name == "line":
                    return partial(facade.Axes.line.__get__(self, type(self)), -1)
                return super().__getattribute__(name)
        def run():
            Child().line(123)
    """,
    "module_getattribute": """
        class M(ModuleType):
            def __getattribute__(self, name):
                if name == "line":
                    return partial(facade.line, -1)
                return super().__getattribute__(name)
        m = M("fake")
        m.line = facade.line
        def run():
            m.line(123)
    """,
    "metaclass_getattribute": """
        class Meta(type):
            def __getattribute__(cls, name):
                if name == "line":
                    return partial(facade.line, -1)
                return super().__getattribute__(name)
        class Holder(metaclass=Meta):
            line = staticmethod(facade.line)
        def run():
            Holder.line(123)
    """,
    "staticmethod_descriptor": """
        class Wrapped(staticmethod):
            def __get__(self, obj, cls):
                return partial(facade.line, -1)
        class Holder:
            line = Wrapped(facade.line)
        def run():
            Holder.line(123)
    """,
    "metaclass_call": """
        class Meta(type):
            __call__ = staticmethod(partial(type.__call__, facade.Figure, -1))
        class Figure(facade.Figure, metaclass=Meta):
            pass
        def run():
            Figure(123)
    """,
    "global_exec_locals": """
        line = partial(facade.line, -1)
        def run():
            global line
            exec("line = facade.line")
            line(123)
    """,
    "class_global_shadow": """
        line = partial(facade.line, -1)
        class Meta(type):
            @classmethod
            def __prepare__(cls, name, bases):
                return {"line": facade.line}
        def run():
            class C(metaclass=Meta):
                global line
                line(123)
    """,
    "walrus_rebind": """
        def run():
            line = partial(facade.line, -1)
            line(line := facade.line)
    """,
    "argument_rebind": """
        line = partial(facade.line, -1)
        def switch():
            global line
            line = facade.line
            return 123
        def run():
            line(switch())
    """,
    "subclass_staticmethod_partial": """
        class Child(facade.Axes):
            line = staticmethod(partial(facade.Axes.line, facade.Axes(), -1))
        def run():
            Child().title("ok").line(123)
    """,
}

_HIGHLIGHTED = {
    "class_property_hook": (
        """
        class Child(facade.Axes):
            __slots__ = ()
            @property
            def __class__(self):
                raise RuntimeError("class query failed")
        def run():
            p = Child()
            p.line(-1, 123)
        """,
        "  p.line(-1, 123)\n         ^^",
    ),
    "metaclass_mro_hook": (
        """
        class Meta(type):
            def __getattribute__(cls, name):
                if name == "__mro__":
                    raise RuntimeError("mro query failed")
                return super().__getattribute__(name)
        class Child(facade.Axes, metaclass=Meta):
            pass
        def run():
            p = Child()
            p.title("ok").line(-1, 123)
        """,
        '  p.title("ok").line(-1, 123)\n                     ^^',
    ),
    "deferred_annotations": (
        """
        def run():
            p = facade.Annotated()
            p.title("ok").line(-1, 123)
        """,
        '  p.title("ok").line(-1, 123)\n                     ^^',
    ),
    "override_calling_super": (
        """
        class Child(facade.Axes):
            def line(self, x, y):
                return super().line(-1, y)
        def run():
            Child().line(123, 456)
        """,
        "  return super().line(-1, y)\n                      ^^",
    ),
}


def _run_caller(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, facade: Facade, body: str
) -> BaseException:
    """Run ``run()`` from ``body``, written as a user module, and return the error it raised."""
    path = tmp_path / "caller.py"
    header = "from functools import partial\nfrom types import ModuleType\n"
    path.write_text(header + textwrap.dedent(body), encoding="utf-8")
    spec = importlib.util.spec_from_file_location("caller", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    module.__dict__["facade"] = facade
    monkeypatch.setitem(sys.modules, "caller", module)
    spec.loader.exec_module(module)
    run: Callable[[], object] = module.__dict__["run"]
    with pytest.raises(ub.DataError) as info:
        run()
    return info.value


@pytest.mark.parametrize("body", _HINT_ONLY.values(), ids=_HINT_ONLY.keys())
def test_call_through_a_wrapper_or_hook_gets_only_the_hint(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, facade: Facade, body: str
) -> None:
    err = _run_caller(tmp_path, monkeypatch, facade, body)
    parameter = "width" if "Figure" in body else "x"
    assert _notes(err) == [f"hint: pass a {parameter} of 0 or more."]


@pytest.mark.parametrize(("body", "highlight"), _HIGHLIGHTED.values(), ids=_HIGHLIGHTED.keys())
def test_direct_call_with_user_hooks_highlights_the_right_argument(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, facade: Facade, body: str, highlight: str
) -> None:
    events = list(cast(list[str], cast(object, facade).__dict__["EVENTS"]))
    err = _run_caller(tmp_path, monkeypatch, facade, body)
    assert _notes(err) == [highlight, "hint: pass a x of 0 or more."]
    # Reporting evaluates no annotations and runs none of the caller's hooks.
    assert cast(object, facade).__dict__["EVENTS"] == events


class _Property:
    """Replacement stderr streams whose attribute lookup itself fails."""

    def __init__(self, failing: str) -> None:
        self.failing = failing

    def __getattribute__(self, name: str) -> object:
        if name == object.__getattribute__(self, "failing"):
            raise RuntimeError(f"stderr {name} lookup failed")
        return object.__getattribute__(self, name)

    def write(self, text: str) -> int:
        return len(text)

    def fileno(self) -> int:
        raise io.UnsupportedOperation("fileno")

    def isatty(self) -> bool:
        return False


@pytest.mark.parametrize("failing", ["fileno", "isatty"])
def test_failing_stderr_attribute_never_replaces_the_error(
    facade: Facade, monkeypatch: pytest.MonkeyPatch, failing: str
) -> None:
    monkeypatch.delenv("NO_COLOR")
    monkeypatch.delenv("TERM", raising=False)
    monkeypatch.setattr(sys, "stderr", _Property(failing))
    a, b = 1, -1
    with pytest.raises(ub.DataError) as info:
        facade.f(a, b)
    assert _notes(info.value) == [
        "  facade.f(a, b)\n              ^",
        "hint: pass a b of 0 or more.",
    ]


def test_failure_while_building_notes_never_replaces_the_error(
    facade: Facade, monkeypatch: pytest.MonkeyPatch
) -> None:
    def broken(*args: object) -> None:
        raise RuntimeError("note failed")

    monkeypatch.setattr(_report, "find", broken)
    with pytest.raises(ub.DataError) as info:
        facade.f(1, -1)
    assert str(info.value) == "f(): b is -1; b must be 0 or greater."
    assert _frames(info.value) == ["test_failure_while_building_notes_never_replaces_the_error"]


def test_large_source_gets_only_the_hint(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, facade: Facade
) -> None:
    padding = "# " + "x" * 98 + "\n"
    body = padding * 5300 + "def run():\n    facade.line(-1, 123)\n"
    err = _run_caller(tmp_path, monkeypatch, facade, body)
    assert _notes(err) == ["hint: pass a x of 0 or more."]


def test_source_just_under_the_limit_gets_the_highlight(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, facade: Facade
) -> None:
    padding = "# " + "x" * 98 + "\n"
    body = padding * 5000 + "def run():\n    facade.line(-1, 123)\n"
    err = _run_caller(tmp_path, monkeypatch, facade, body)
    assert _notes(err)[0] == "  facade.line(-1, 123)\n              ^^"


def test_parameters_rebound_in_the_body_keep_the_highlight(facade: Facade) -> None:
    # Defaults are compared with the values the call started with, not later ones.
    a = -1
    with pytest.raises(ub.DataError) as info:
        facade.rebinding(a)
    assert _notes(info.value) == [
        "  facade.rebinding(a)\n                   ^",
        "hint: pass a a of 0 or more.",
    ]
