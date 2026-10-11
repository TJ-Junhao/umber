"""The context manager around every public call: error notes and the traceback.

Each public function and method wraps its whole body in ``with api_call(context,
positional):``. When an exception that umber raised on purpose (see ``umber._errors``)
leaves the body:

- If the call came from inside the package (a quick function calling an ``Axes`` method),
  only the traceback is reset; the outer call reports the error.
- Otherwise the caller is user code. Unless the exception was already reported, a note
  highlighting the offending argument in the caller's source and a ``hint:`` note are added.
- The traceback is cleared before the exception continues, so it shows only the user's
  frames, ending at the line that made the call.

Any other exception passes through untouched, with its full traceback.
"""

import inspect
from pathlib import Path
from types import FrameType, TracebackType

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
        """Add notes to an umber error and clear its traceback; never suppress anything."""
        if exc is None:
            return
        found = details(exc)
        if found is None:
            return
        parameter, hint = found
        caller = _caller()
        if not (caller is not None and _in_package(caller)) and not hasattr(exc, _LOCATED):
            self._add_notes(exc, caller, parameter, hint)
            setattr(exc, _LOCATED, True)
        # Without its traceback the exception is re-raised from the caller's frame, so only
        # user frames are shown.
        exc.__traceback__ = None

    def _add_notes(
        self,
        exc: BaseException,
        caller: FrameType | None,
        parameter: str | None,
        hint: str | None,
    ) -> None:
        """Add the highlight note, then the hint note, styled when colour is enabled."""
        color = color_enabled()
        if caller is not None and parameter is not None:
            callee = self._context.removesuffix("()").rpartition(".")[2]
            highlight = find(caller, callee, self._positional, parameter)
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


def _caller() -> FrameType | None:
    """Return the frame that called the public function whose ``with`` block is exiting."""
    frame = inspect.currentframe()
    # This function, then ApiCall.__exit__, then the public function, then its caller.
    for _ in range(3):
        if frame is None:
            return None
        frame = frame.f_back
    return frame


def _in_package(frame: FrameType) -> bool:
    """Return whether ``frame`` runs code from a file inside the package directory."""
    filename = frame.f_code.co_filename
    if filename.startswith("<"):
        return False
    return Path(filename).resolve().is_relative_to(_PACKAGE_DIR.resolve())
