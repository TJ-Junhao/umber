import io
import os
import sys
import types

import pytest

from umber import _ansi

_VARIABLES = ("PYTHON_COLORS", "NO_COLOR", "FORCE_COLOR", "TERM")


class _Stream:
    """A stand-in for ``sys.stderr`` with a file descriptor."""

    def fileno(self) -> int:
        return 2

    def isatty(self) -> bool:
        raise AssertionError("os.isatty decides for a stream with a descriptor")


class _CapturedStream:
    """A stream without a real descriptor, like the ones test runners capture into."""

    def __init__(self, tty: bool) -> None:
        self.tty = tty

    def fileno(self) -> int:
        raise io.UnsupportedOperation("fileno")

    def isatty(self) -> bool:
        return self.tty


class _NoFileno:
    def isatty(self) -> bool:
        return True


class _Closed:
    def fileno(self) -> int:
        raise ValueError("I/O operation on closed file")

    def isatty(self) -> bool:
        raise ValueError("I/O operation on closed file")


@pytest.fixture
def env(monkeypatch: pytest.MonkeyPatch) -> pytest.MonkeyPatch:
    """Start from no colour variables and a non-Windows platform."""
    for name in _VARIABLES:
        monkeypatch.delenv(name, raising=False)
    monkeypatch.setattr(sys, "platform", "linux")
    return monkeypatch


def _stderr(monkeypatch: pytest.MonkeyPatch, stream: object, *, tty: bool = True) -> None:
    """Replace ``sys.stderr``; descriptor 2 is a terminal when ``tty`` is true.

    Called in the test body: the test runner reassigns ``sys.stderr`` between the setup and
    call phases, so a fixture cannot replace it.
    """

    def isatty(fd: int) -> bool:
        return tty and fd == 2

    monkeypatch.setattr(sys, "stderr", stream)
    monkeypatch.setattr(os, "isatty", isatty)


def _set(monkeypatch: pytest.MonkeyPatch, variables: dict[str, str]) -> None:
    for name, value in variables.items():
        monkeypatch.setenv(name, value)


@pytest.mark.parametrize(
    ("variables", "tty", "expected"),
    [
        ({}, True, True),
        ({}, False, False),
        # 1. PYTHON_COLORS wins over everything else.
        ({"PYTHON_COLORS": "0", "FORCE_COLOR": "1"}, True, False),
        ({"PYTHON_COLORS": "1", "NO_COLOR": "1"}, False, True),
        ({"PYTHON_COLORS": "1", "TERM": "dumb"}, False, True),
        # Other values of PYTHON_COLORS are ignored.
        ({"PYTHON_COLORS": "2"}, True, True),
        ({"PYTHON_COLORS": ""}, False, False),
        # 2. A non-empty NO_COLOR comes before FORCE_COLOR.
        ({"NO_COLOR": "1", "FORCE_COLOR": "1"}, True, False),
        ({"NO_COLOR": ""}, True, True),
        # 3. A non-empty FORCE_COLOR comes before TERM and the TTY check.
        ({"FORCE_COLOR": "1", "TERM": "dumb"}, False, True),
        ({"FORCE_COLOR": ""}, False, False),
        # 4. TERM=dumb comes before the TTY check.
        ({"TERM": "dumb"}, True, False),
        ({"TERM": "xterm-256color"}, True, True),
    ],
)
def test_color_follows_the_interpreter_precedence(
    env: pytest.MonkeyPatch, variables: dict[str, str], tty: bool, expected: bool
) -> None:
    _set(env, variables)
    _stderr(env, _Stream(), tty=tty)
    assert _ansi.color_enabled() is expected


@pytest.mark.parametrize(
    ("variables", "expected"),
    [
        ({"PYTHON_COLORS": "1"}, False),
        ({"PYTHON_COLORS": "0"}, True),
        ({"PYTHON_COLORS": "0", "NO_COLOR": "1"}, False),
        ({"PYTHON_COLORS": "1", "FORCE_COLOR": "1"}, True),
    ],
)
def test_python_colors_is_ignored_when_python_ignores_the_environment(
    env: pytest.MonkeyPatch, variables: dict[str, str], expected: bool
) -> None:
    _set(env, variables)
    _stderr(env, _Stream(), tty=expected)
    env.setattr(sys, "flags", types.SimpleNamespace(ignore_environment=1))
    assert _ansi.color_enabled() is expected


def test_stderr_without_fileno_is_not_a_terminal(env: pytest.MonkeyPatch) -> None:
    _stderr(env, _NoFileno())
    assert _ansi.color_enabled() is False


def test_missing_stderr_is_not_a_terminal(env: pytest.MonkeyPatch) -> None:
    _stderr(env, None)
    assert _ansi.color_enabled() is False


@pytest.mark.parametrize("tty", [True, False])
def test_stderr_without_a_descriptor_answers_itself(env: pytest.MonkeyPatch, tty: bool) -> None:
    _stderr(env, _CapturedStream(tty))
    assert _ansi.color_enabled() is tty


class _NoDescriptorNoIsatty:
    def fileno(self) -> int:
        raise io.UnsupportedOperation("fileno")


class _Failing:
    def fileno(self) -> int:
        raise RuntimeError("no descriptor")

    def isatty(self) -> bool:
        raise RuntimeError("no terminal")


class _FailingIsatty:
    def fileno(self) -> int:
        raise io.UnsupportedOperation("fileno")

    def isatty(self) -> bool:
        raise RuntimeError("no terminal")


@pytest.mark.parametrize("stream", [_NoDescriptorNoIsatty(), _Failing(), _FailingIsatty()])
def test_stderr_that_cannot_answer_is_not_a_terminal(
    env: pytest.MonkeyPatch, stream: object
) -> None:
    _stderr(env, stream)
    assert _ansi.color_enabled() is False


def test_closed_stderr_is_not_a_terminal(env: pytest.MonkeyPatch) -> None:
    _stderr(env, _Closed())
    assert _ansi.color_enabled() is False


@pytest.mark.parametrize(("supported", "expected"), [(True, True), (False, False)])
def test_windows_console_must_support_sequences(
    env: pytest.MonkeyPatch, supported: bool, expected: bool
) -> None:
    nt = types.ModuleType("nt")
    nt.__dict__["_supports_virtual_terminal"] = lambda: supported
    env.setitem(sys.modules, "nt", nt)
    env.setattr(sys, "platform", "win32")
    _stderr(env, _Stream())
    assert _ansi.color_enabled() is expected


def test_windows_without_the_console_query_uses_the_terminal_check(
    env: pytest.MonkeyPatch,
) -> None:
    env.setitem(sys.modules, "nt", types.ModuleType("nt"))
    env.setattr(sys, "platform", "win32")
    _stderr(env, _Stream())
    assert _ansi.color_enabled() is True


def test_windows_console_query_does_not_override_force_color(env: pytest.MonkeyPatch) -> None:
    nt = types.ModuleType("nt")
    nt.__dict__["_supports_virtual_terminal"] = lambda: False
    env.setitem(sys.modules, "nt", nt)
    env.setattr(sys, "platform", "win32")
    _stderr(env, _Stream())
    env.setenv("FORCE_COLOR", "1")
    assert _ansi.color_enabled() is True


def test_styles_wrap_and_reset() -> None:
    assert _ansi.bold("hint:") == "\x1b[1mhint:\x1b[0m"
    assert _ansi.bold_red("^^") == "\x1b[1;31m^^\x1b[0m"
