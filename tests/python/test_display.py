# pyright: reportPrivateUsage=false
import copy
import os
import struct
import subprocess
import sys
import tempfile
import threading
from collections.abc import Callable, Collection
from pathlib import Path
from types import SimpleNamespace
from typing import cast

import IPython
import IPython.display
import pytest
from IPython.core.formatters import DisplayFormatter

import umber as ub
from umber._display import Bundle
from umber._errors import details

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
VIEWER_HINT = "save the figure with save_png(), save_svg() or save_pdf() and open it yourself."
type Displayable = ub.Figure | ub.Axes[float, float] | ub.PieAxes


class _Process:
    def __init__(self) -> None:
        self.code: int | None = 0
        self.timeouts: list[float | None] = []
        self.waiting = threading.Event()
        self.exited = threading.Event()
        self.reaped = threading.Event()
        self.waiter: threading.Thread | None = None

    def wait(self, *, timeout: float | None = None) -> int:
        self.timeouts.append(timeout)
        if timeout is None:
            self.waiter = threading.current_thread()
            self.waiting.set()
            self.exited.wait()
        if self.code is None:
            assert timeout is not None
            raise subprocess.TimeoutExpired("viewer", timeout)
        self.reaped.set()
        return self.code


class _Viewer:
    def __init__(self) -> None:
        self.process = _Process()
        self.commands: list[list[str]] = []
        self.options: list[dict[str, object]] = []
        self.files: list[str] = []
        self.error: Exception | None = None

    def popen(self, command: list[str], **options: object) -> _Process:
        self.commands.append(command)
        self.options.append(options)
        if self.error is not None:
            raise self.error
        return self.process

    def startfile(self, path: str) -> None:
        self.files.append(path)
        if self.error is not None:
            raise self.error


@pytest.fixture(autouse=True)
def no_real_viewer(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    def refuse(*args: object, **kwargs: object) -> None:
        raise AssertionError("A test tried to start a real viewer")

    monkeypatch.setattr(subprocess, "Popen", refuse)
    monkeypatch.setattr(os, "startfile", refuse, raising=False)
    monkeypatch.setattr(IPython, "get_ipython", lambda: None)
    monkeypatch.setenv("NO_COLOR", "1")
    mkdtemp = tempfile.mkdtemp

    def temporary_directory(*, prefix: str) -> str:
        assert prefix == "umber-"
        return mkdtemp(prefix=prefix, dir=tmp_path)

    monkeypatch.setattr(tempfile, "mkdtemp", temporary_directory)


@pytest.fixture
def viewer(monkeypatch: pytest.MonkeyPatch, no_real_viewer: None) -> _Viewer:
    viewer = _Viewer()
    monkeypatch.setattr(subprocess, "Popen", viewer.popen)
    monkeypatch.setattr(os, "startfile", viewer.startfile)
    return viewer


def _target(kind: str, figure: ub.Figure) -> Displayable:
    if kind == "pie":
        return figure.add_pie_axes().pie(["North", "South"], [3, 1])
    axes = figure.add_axes().line([0, 1], [1, 3])
    return axes if kind == "axes" else figure


def _pixels(png: bytes) -> tuple[int, int]:
    assert png.startswith(PNG_SIGNATURE)
    assert png[12:16] == b"IHDR"
    width, height = struct.unpack(">II", png[16:24])
    return width, height


def _format(value: object) -> Bundle:
    formatter = DisplayFormatter()
    format_value = cast(Callable[[object], Bundle], formatter.format)  # pyright: ignore[reportUnknownMemberType]
    return format_value(value)


def _count_renders(monkeypatch: pytest.MonkeyPatch) -> list[int]:
    resolutions: list[int] = []
    to_png = ub.Figure.to_png

    def render(figure: ub.Figure, *, dpi: int = 200) -> bytes:
        resolutions.append(dpi)
        return to_png(figure, dpi=dpi)

    monkeypatch.setattr(ub.Figure, "to_png", render)
    return resolutions


def _saved_png(tmp_path: Path) -> Path:
    paths = list(tmp_path.glob("umber-*/figure.png"))
    assert len(paths) == 1
    path = paths[0]
    assert _pixels(path.read_bytes()) == (864, 576)
    return path


def _assert_viewer_error(error: ub.UsageError, path: Path, target: Displayable) -> None:
    assert str(error).startswith(f"{type(target).__name__}.show(): ")
    assert str(path) in str(error)
    assert details(error) == (None, VIEWER_HINT)
    assert f"hint: {VIEWER_HINT}" in getattr(error, "__notes__", [])


@pytest.mark.parametrize("kind", ["figure", "axes", "pie"])
def test_mimebundle_png_and_existing_repr(kind: str) -> None:
    figure = ub.Figure(6, 4)
    target = _target(kind, figure)
    data, metadata = target._repr_mimebundle_()
    assert set(data) == {"image/png", "text/plain"}
    assert data["text/plain"] == repr(target)
    png = data["image/png"]
    assert isinstance(png, bytes)
    assert metadata == {"image/png": {"width": 432, "height": 288}}
    assert _pixels(png) == (864, 576)
    assert png == figure.to_png(dpi=144)
    assert not hasattr(target, "_repr_png_")
    assert not hasattr(target, "_repr_svg_")


@pytest.mark.parametrize(
    ("width", "height", "rows", "cols", "size"),
    [
        (ub.AUTO, ub.AUTO, 1, 1, (432, 288)),
        (ub.AUTO, ub.AUTO, 2, 2, (864, 576)),
        (ub.AUTO, 2.5, 2, 2, (864, 180)),
        (3.5, ub.AUTO, 2, 2, (252, 576)),
        (3.5, 2.5, 2, 2, (252, 180)),
    ],
)
def test_mimebundle_resolved_grid_size(
    width: float | ub.Auto,
    height: float | ub.Auto,
    rows: int,
    cols: int,
    size: tuple[int, int],
) -> None:
    figure = ub.Figure(width, height, rows=rows, cols=cols)
    data, metadata = figure._repr_mimebundle_()
    png = data["image/png"]
    assert isinstance(png, bytes)
    assert metadata == {"image/png": {"width": size[0], "height": size[1]}}
    assert _pixels(png) == (2 * size[0], 2 * size[1])


def test_axes_and_pie_display_the_whole_grid() -> None:
    figure = ub.Figure(cols=2)
    axes = figure.add_axes(col=0).line([0, 1], [1, 3])
    pie = figure.add_pie_axes(col=1).pie(["North", "South"], [3, 1])
    expected = figure.to_png(dpi=144)
    for target in (axes, pie):
        data, metadata = target._repr_mimebundle_()
        assert data["image/png"] == expected
        assert metadata == {"image/png": {"width": 864, "height": 288}}


def test_mimebundle_rounds_css_size() -> None:
    _, metadata = ub.Figure(6.01, 4.02)._repr_mimebundle_()
    assert metadata == {"image/png": {"width": round(6.01 * 72), "height": round(4.02 * 72)}}


@pytest.mark.parametrize("clone", [copy.copy, copy.deepcopy])
def test_mimebundle_copied_figure_size(clone: Callable[[ub.Figure], ub.Figure]) -> None:
    figure = ub.Figure(3.5, cols=2)
    assert clone(figure)._repr_mimebundle_()[1] == {"image/png": {"width": 252, "height": 288}}


@pytest.mark.parametrize("kind", ["figure", "axes", "pie"])
@pytest.mark.parametrize(
    ("include", "exclude", "expected"),
    [
        (None, None, {"image/png", "text/plain"}),
        (["image/png"], None, {"image/png"}),
        (("text/plain",), None, {"text/plain"}),
        ([], None, set[str]()),
        (["image/svg+xml"], None, set[str]()),
        (None, {"image/png"}, {"text/plain"}),
        (None, ["text/plain"], {"image/png"}),
        (["image/png"], ["image/png"], set[str]()),
        (None, ["image/png", "text/plain"], set[str]()),
        (None, ["image/svg+xml"], {"image/png", "text/plain"}),
    ],
)
def test_mimebundle_filters(
    monkeypatch: pytest.MonkeyPatch,
    kind: str,
    include: Collection[str] | None,
    exclude: Collection[str] | None,
    expected: set[str],
) -> None:
    target = _target(kind, ub.Figure())
    renders = _count_renders(monkeypatch)
    representations: list[object] = []

    def text_repr(value: object) -> str:
        representations.append(value)
        return "text repr"

    for cls in (ub.Figure, ub.Axes, ub.PieAxes):
        monkeypatch.setattr(cls, "__repr__", text_repr)
    data, metadata = target._repr_mimebundle_(include=include, exclude=exclude)
    assert set(data) == expected
    assert set(metadata) == ({"image/png"} if "image/png" in expected else set())
    assert renders == ([144] if "image/png" in expected else [])
    assert representations == ([target] if "text/plain" in expected else [])
    if "text/plain" in expected:
        assert data["text/plain"] == "text repr"


@pytest.mark.parametrize("kind", ["figure", "axes", "pie"])
def test_formatter_has_one_png_no_svg(monkeypatch: pytest.MonkeyPatch, kind: str) -> None:
    target = _target(kind, ub.Figure())
    renders = _count_renders(monkeypatch)
    data, metadata = _format(target)
    assert set(data) == {"image/png", "text/plain"}
    png = data["image/png"]
    assert isinstance(png, bytes)
    assert _pixels(png) == (864, 576)
    assert metadata == {"image/png": {"width": 432, "height": 288}}
    assert renders == [144]


@pytest.mark.parametrize("kind", ["figure", "axes", "pie"])
def test_mimebundle_render_error_propagates(kind: str) -> None:
    figure = ub.Figure().title("W" * 200)
    target = _target(kind, figure)
    with pytest.raises(ub.StyleError):
        target._repr_mimebundle_()
    assert target._repr_mimebundle_(exclude=["image/png"]) == (
        {"text/plain": repr(target)},
        {},
    )


@pytest.mark.parametrize("kind", ["figure", "axes", "pie"])
def test_show_kernel_displays_figure_once(monkeypatch: pytest.MonkeyPatch, kind: str) -> None:
    figure = ub.Figure()
    target = _target(kind, figure)
    calls: list[object] = []

    def display(value: object) -> None:
        calls.append(value)
        _format(value)

    monkeypatch.setattr(IPython, "get_ipython", lambda: SimpleNamespace(kernel=object()))
    monkeypatch.setattr(IPython.display, "display", display)
    renders = _count_renders(monkeypatch)
    assert target.show() is None
    assert calls == [figure]
    assert renders == [144]
    # A new display must not reuse bytes from the previous show().
    figure.title("Changed")
    figure._repr_mimebundle_()
    assert renders == [144, 144]


@pytest.mark.parametrize("kernel", [False, True])
@pytest.mark.parametrize("kind", ["figure", "axes", "pie"])
def test_show_render_error_propagates(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, kernel: bool, kind: str
) -> None:
    figure = ub.Figure().title("W" * 200)
    target = _target(kind, figure)
    if kernel:
        monkeypatch.setattr(IPython, "get_ipython", lambda: SimpleNamespace(kernel=object()))
    calls: list[object] = []
    monkeypatch.setattr(IPython.display, "display", calls.append)
    with pytest.raises(ub.StyleError):
        target.show()
    assert calls == []
    assert not list(tmp_path.glob("umber-*"))


def test_show_kernel_failure_does_not_cache_png(monkeypatch: pytest.MonkeyPatch) -> None:
    figure = ub.Figure()

    def display(value: object) -> None:
        raise RuntimeError("display failed")

    monkeypatch.setattr(IPython, "get_ipython", lambda: SimpleNamespace(kernel=object()))
    monkeypatch.setattr(IPython.display, "display", display)
    renders = _count_renders(monkeypatch)
    with pytest.raises(RuntimeError, match="display failed"):
        figure.show()
    figure._repr_mimebundle_()
    assert renders == [144, 144]


@pytest.mark.parametrize("kind", ["figure", "axes", "pie"])
@pytest.mark.parametrize("platform", ["darwin", "win32", "linux", "freebsd14"])
def test_show_system_viewer(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, viewer: _Viewer, kind: str, platform: str
) -> None:
    monkeypatch.setattr(sys, "platform", platform)
    monkeypatch.setenv("DISPLAY", ":0")
    assert _target(kind, ub.Figure()).show() is None
    path = _saved_png(tmp_path)
    if platform == "win32":
        assert viewer.files == [str(path)]
        assert viewer.commands == []
    else:
        assert viewer.commands == [["open" if platform == "darwin" else "xdg-open", str(path)]]
        assert viewer.options == [
            {
                "shell": False,
                "stdin": subprocess.DEVNULL,
                "stdout": subprocess.DEVNULL,
                "stderr": subprocess.DEVNULL,
                "start_new_session": True,
            }
        ]
        assert viewer.process.timeouts == [0.1]
        assert viewer.files == []


@pytest.mark.parametrize("shell", [None, SimpleNamespace()])
def test_show_plain_python_and_terminal_ipython_use_viewer(
    monkeypatch: pytest.MonkeyPatch, viewer: _Viewer, shell: object
) -> None:
    monkeypatch.setattr(IPython, "get_ipython", lambda: shell)
    monkeypatch.setattr(sys, "platform", "darwin")
    calls: list[object] = []
    monkeypatch.setattr(IPython.display, "display", calls.append)
    ub.Figure().show()
    assert len(viewer.commands) == 1
    assert calls == []


def test_show_without_ipython(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, viewer: _Viewer
) -> None:
    monkeypatch.setitem(sys.modules, "IPython", None)
    monkeypatch.setattr(sys, "platform", "darwin")
    assert cast(Callable[[], object], ub.Figure().show)() is None
    assert len(viewer.commands) == 1
    _saved_png(tmp_path)


@pytest.mark.parametrize("platform", ["darwin", "linux", "freebsd14"])
@pytest.mark.parametrize("code", [0, 3])
def test_show_reaps_viewer_without_waiting_for_it_to_close(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, viewer: _Viewer, platform: str, code: int
) -> None:
    monkeypatch.setattr(sys, "platform", platform)
    monkeypatch.setenv("DISPLAY", ":0")
    process = viewer.process
    process.code = None
    try:
        assert cast(Callable[[], object], ub.Figure().show)() is None
        assert process.waiting.wait(timeout=5)
        assert process.timeouts == [0.1, None]
        assert process.waiter is not None
        assert process.waiter.daemon
        assert not process.reaped.is_set()
        _saved_png(tmp_path)
    finally:
        process.code = code
        process.exited.set()
        assert process.reaped.wait(timeout=5)
        if process.waiter is not None:
            process.waiter.join(timeout=5)
            assert not process.waiter.is_alive()


@pytest.mark.parametrize("kind", ["figure", "axes", "pie"])
@pytest.mark.parametrize("platform", ["darwin", "linux", "win32"])
@pytest.mark.parametrize("error_type", [FileNotFoundError, PermissionError, NotImplementedError])
def test_show_missing_viewer_raises_usage_error(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    viewer: _Viewer,
    kind: str,
    platform: str,
    error_type: type[Exception],
) -> None:
    monkeypatch.setattr(sys, "platform", platform)
    monkeypatch.setenv("DISPLAY", ":0")
    viewer.error = error_type("viewer missing")
    target = _target(kind, ub.Figure())
    with pytest.raises(ub.UsageError) as caught:
        target.show()
    _assert_viewer_error(caught.value, _saved_png(tmp_path), target)
    assert "viewer missing" in str(caught.value)


@pytest.mark.parametrize("platform", ["darwin", "linux", "win32"])
def test_show_viewer_attribute_error_propagates(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, viewer: _Viewer, platform: str
) -> None:
    monkeypatch.setattr(sys, "platform", platform)
    monkeypatch.setenv("DISPLAY", ":0")
    viewer.error = AttributeError("viewer attribute failure")
    with pytest.raises(AttributeError, match="viewer attribute failure"):
        ub.Figure().show()
    _saved_png(tmp_path)


@pytest.mark.parametrize("kind", ["figure", "axes", "pie"])
@pytest.mark.parametrize("platform", ["darwin", "linux"])
@pytest.mark.parametrize("code", [1, 3, -9])
def test_show_immediate_viewer_failure_raises_usage_error(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    viewer: _Viewer,
    kind: str,
    platform: str,
    code: int,
) -> None:
    monkeypatch.setattr(sys, "platform", platform)
    monkeypatch.setenv("DISPLAY", ":0")
    viewer.process.code = code
    target = _target(kind, ub.Figure())
    with pytest.raises(ub.UsageError) as caught:
        target.show()
    _assert_viewer_error(caught.value, _saved_png(tmp_path), target)
    assert f"exited with status {code}" in str(caught.value)
    assert viewer.process.reaped.is_set()


@pytest.mark.parametrize("kind", ["figure", "axes", "pie"])
@pytest.mark.parametrize("empty", [False, True])
def test_show_linux_without_display_raises_usage_error(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, kind: str, empty: bool
) -> None:
    monkeypatch.setattr(sys, "platform", "linux")
    for name in ("DISPLAY", "WAYLAND_DISPLAY"):
        if empty:
            monkeypatch.setenv(name, "")
        else:
            monkeypatch.delenv(name, raising=False)
    target = _target(kind, ub.Figure())
    with pytest.raises(ub.UsageError) as caught:
        target.show()
    _assert_viewer_error(caught.value, _saved_png(tmp_path), target)
    assert "neither DISPLAY nor WAYLAND_DISPLAY is set" in str(caught.value)


@pytest.mark.parametrize("variable", ["DISPLAY", "WAYLAND_DISPLAY"])
def test_show_linux_accepts_either_display(
    monkeypatch: pytest.MonkeyPatch, viewer: _Viewer, variable: str
) -> None:
    monkeypatch.setattr(sys, "platform", "linux")
    monkeypatch.delenv("DISPLAY", raising=False)
    monkeypatch.delenv("WAYLAND_DISPLAY", raising=False)
    monkeypatch.setenv(variable, ":0")
    ub.Figure().show()
    assert len(viewer.commands) == 1


def test_show_leaves_a_new_file_each_time(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, viewer: _Viewer
) -> None:
    monkeypatch.setattr(sys, "platform", "darwin")
    figure = ub.Figure()
    figure.show()
    figure.show()
    assert len(viewer.commands) == 2
    files = list(tmp_path.glob("umber-*/figure.png"))
    assert len(files) == 2
    assert all(path.read_bytes().startswith(PNG_SIGNATURE) for path in files)
