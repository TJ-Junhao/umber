"""The context manager around every public call: error notes and the traceback.

Each public function and method wraps its whole body in ``with api_call(context,
positional):``. When an exception that umber raised on purpose (see ``umber._errors``)
leaves the body:

- If the call came from inside the package (a quick function calling an ``Axes`` method),
  only the traceback is reset; the outer call reports the error.
- Otherwise the caller is user code. Unless the exception was already reported, a note
  highlighting the offending argument in the caller's source and a ``hint:`` note are added.
- The package's frames are removed from the traceback before the exception continues, so
  it shows only user frames: the ones above the call, and any user function the package
  called back into.

Any other exception passes through untouched, with its full traceback. Reporting never
raises: a note that cannot be built is left out.
"""

import inspect
import os
from pathlib import Path
from types import CodeType, FrameType, TracebackType

from umber._ansi import bold, color_enabled
from umber._errors import details
from umber._highlight import find, render

_PACKAGE_DIR: Path = Path(__file__).resolve().parent
_LOCATED = "_umber_located"


class ApiCall:
    """Report an umber error raised inside one public call; see the module docstring."""

    __slots__ = ("_context", "_positional")

    def __init__(self, context: str, positional: tuple[str, ...]) -> None:
        """Create the reporter for the call ``context`` (such as ``"Axes.line()"``).

        Args:
            context: The context of the public call, as in its error messages.
            positional: The parameters the call accepts by position, in order, without
                ``self``.
        """
        self._context = context
        self._positional = positional

    def __enter__(self) -> None:
        """Enter the call; nothing to set up."""

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        """Add notes to an umber error and drop the package's frames; never suppress it."""
        if exc is None:
            return
        found = details(exc)
        if found is None:
            return
        parameter, hint = found
        public = _public_frame()
        caller = None if public is None else public.f_back
        if not (caller is not None and _in_package(caller)) and not hasattr(exc, _LOCATED):
            target = None if public is None else public.f_code
            self._add_notes(exc, caller, target, parameter, hint)
            setattr(exc, _LOCATED, True)
        # The interpreter re-raises the exception with this traceback and adds the frames
        # above as it propagates, so the package frames never reappear.
        exc.__traceback__ = _without_package_frames(exc.__traceback__)

    def _add_notes(
        self,
        exc: BaseException,
        caller: FrameType | None,
        target: CodeType | None,
        parameter: str | None,
        hint: str | None,
    ) -> None:
        """Add the highlight note, then the hint note, styled when colour is enabled."""
        color = color_enabled()
        if caller is not None and target is not None and parameter is not None:
            callee = self._context.removesuffix("()").rpartition(".")[2]
            highlight = find(caller, callee, target, self._positional, parameter)
            if highlight is not None:
                exc.add_note(render(highlight, color=color))
        if hint is not None:
            exc.add_note(f"{bold('hint:') if color else 'hint:'} {hint}")


def api_call(context: str, positional: tuple[str, ...]) -> ApiCall:
    """Return the reporter to wrap the body of the public call ``context`` in.

    Args:
        context: The context of the public call, as in its error messages, such as
            ``"Axes.line()"`` or ``"Figure()"``.
        positional: The parameters the call accepts by position, in order, without
            ``self``.

    Returns:
        A context manager; see the module docstring.
    """
    return ApiCall(context, positional)


def _public_frame() -> FrameType | None:
    """Return the frame of the public function whose ``with`` block is exiting."""
    frame = inspect.currentframe()
    # This function, then ApiCall.__exit__, then the public function.
    for _ in range(2):
        if frame is None:
            return None
        frame = frame.f_back
    return frame


def _without_package_frames(tb: TracebackType | None) -> TracebackType | None:
    """Return the traceback ``tb`` without the entries of frames inside the package."""
    kept: list[TracebackType] = []
    while tb is not None:
        if not _in_package(tb.tb_frame):
            kept.append(tb)
        tb = tb.tb_next
    head: TracebackType | None = None
    for entry in reversed(kept):
        head = TracebackType(head, entry.tb_frame, entry.tb_lasti, entry.tb_lineno)
    return head


def _in_package(frame: FrameType) -> bool:
    """Return whether ``frame`` runs code from a file inside the package directory."""
    filename = frame.f_code.co_filename
    if filename.startswith("<"):
        return False
    try:
        path = Path(os.path.realpath(filename))
        package = Path(os.path.realpath(_PACKAGE_DIR))
    except (OSError, ValueError):
        # A file name the file system cannot resolve is not one of the package's files.
        return False
    return path.is_relative_to(package)
