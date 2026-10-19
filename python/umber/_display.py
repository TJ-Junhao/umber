"""PNG display bundles and a non-blocking system viewer, without an IPython dependency."""

import os
import subprocess
import sys
import tempfile
import threading
from collections.abc import Callable, Collection
from contextvars import ContextVar
from importlib import import_module
from pathlib import Path, PurePath
from typing import TYPE_CHECKING, Final, cast

from umber._errors import UsageError, new_error

if TYPE_CHECKING:
    from umber._figure import Figure

PNG_DPI: Final = 144
CSS_PIXELS_PER_INCH: Final = 72
_VIEWER_START_TIMEOUT: Final = 0.1
_VIEWER_HINT: Final = (
    "save the figure with save_png(), save_svg() or save_pdf() and open it yourself."
)

# Only a synchronous show() reuses this PNG. A later display always renders current marks.
_SHOWN_PNG: ContextVar[tuple[object, bytes] | None] = ContextVar("umber_shown_png", default=None)
type Bundle = tuple[dict[str, str | bytes], dict[str, dict[str, int]]]


def mimebundle(
    figure: "Figure",
    size_in: tuple[float, float],
    text_repr: Callable[[], str],
    include: Collection[str] | None,
    exclude: Collection[str] | None,
) -> Bundle:
    """Return only the requested PNG and text, rendering nothing when PNG is excluded."""
    data: dict[str, str | bytes] = {}
    metadata: dict[str, dict[str, int]] = {}
    formats = {"image/png", "text/plain"}
    if include is not None:
        formats.intersection_update(include)
    if exclude is not None:
        formats.difference_update(exclude)
    if "image/png" in formats:
        shown = _SHOWN_PNG.get()
        data["image/png"] = (
            shown[1] if shown is not None and shown[0] is figure else figure.to_png(dpi=PNG_DPI)
        )
        width, height = size_in
        metadata["image/png"] = {
            "width": round(width * CSS_PIXELS_PER_INCH),
            "height": round(height * CSS_PIXELS_PER_INCH),
        }
    if "text/plain" in formats:
        data["text/plain"] = text_repr()
    return data, metadata


def show(figure: "Figure", context: str) -> None:
    """Display in a kernel (a shell with a ``kernel`` attribute), or open a saved PNG."""
    display = _kernel_display()
    if display is not None:
        # IPython catches formatter exceptions. Render first so show() raises them, then
        # reuse those bytes when display(figure) requests the bundle instead of rendering twice.
        png = figure.to_png(dpi=PNG_DPI)
        token = _SHOWN_PNG.set((figure, png))
        try:
            display(figure)
        finally:
            _SHOWN_PNG.reset(token)
        return
    png = figure.to_png(dpi=PNG_DPI)
    path = Path(tempfile.mkdtemp(prefix="umber-")) / "figure.png"
    path.write_bytes(png)
    _open_viewer(path, context)


def _kernel_display() -> Callable[[object], None] | None:
    """Import IPython lazily; a terminal shell has no ``kernel`` attribute."""
    try:
        ipython = import_module("IPython")
    except ImportError:
        return None
    get_ipython = cast(Callable[[], object], ipython.get_ipython)
    if not hasattr(get_ipython(), "kernel"):
        return None
    return cast(Callable[[object], None], import_module("IPython.display").display)


def _viewer_error(path: PurePath, context: str, reason: str) -> UsageError:
    """Name the saved file and how to open an image when no viewer can be started."""
    return new_error(
        UsageError,
        context,
        f"could not open a viewer: {reason}; the PNG was saved to {path}.",
        parameter=None,
        hint=_VIEWER_HINT,
    )


def _open_viewer(path: Path, context: str) -> None:
    """Check startup briefly, then reap the child in a daemon thread without blocking.

    This check is best effort: later failures and a successful handoff by ``xdg-open``
    are not detected.
    """
    if sys.platform.startswith("linux") and not (
        os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")
    ):
        raise _viewer_error(path, context, "neither DISPLAY nor WAYLAND_DISPLAY is set")
    try:
        if sys.platform == "win32":
            os.startfile(str(path))
            return
        command = ["open" if sys.platform == "darwin" else "xdg-open", str(path)]
        process = subprocess.Popen(
            command,
            shell=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        try:
            code = process.wait(timeout=_VIEWER_START_TIMEOUT)
        except subprocess.TimeoutExpired:
            threading.Thread(target=process.wait, daemon=True).start()
            return
    except (OSError, NotImplementedError) as error:
        raise _viewer_error(path, context, str(error)) from error
    if code != 0:
        raise _viewer_error(path, context, f"{command[0]} exited with status {code}")
