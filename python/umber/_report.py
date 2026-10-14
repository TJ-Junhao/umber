"""The context manager around every public call: error notes and the traceback.

Each public function and method wraps its whole body in ``with api_call():``. Entering
records the values the parameters start with. When an exception that umber raised on
purpose (see ``umber._errors``) leaves the body:

- If the call came from inside the package (a quick function calling an ``Axes`` method),
  only the traceback is reset; the outer call reports the error.
- Otherwise the caller is user code. Unless the exception was already reported, a note
  highlighting the offending argument in the caller's source and a ``hint:`` note are added.
- The package's frames are removed from the traceback before the exception continues, so
  it shows only user frames: the ones above the call, and any user function the package
  called back into.

Any other exception passes through untouched, with its full traceback. Reporting never
replaces the error: a note that cannot be built is left out.
"""

import inspect
import os
from collections.abc import Mapping
from pathlib import Path
from types import FrameType, TracebackType

from umber._ansi import bold, color_enabled
from umber._errors import details
from umber._highlight import find, render

_PACKAGE_DIR: Path = Path(__file__).resolve().parent
_LOCATED = "_umber_located"


class ApiCall:
    """Report an umber error raised inside one public call; see the module docstring."""

    __slots__ = ("_arguments", "_parameter_map")

    def __init__(self, parameter_map: Mapping[str, str] | None = None) -> None:
        """Create the reporter; ``__enter__`` records the call's arguments."""
        self._arguments: dict[str, object] | None = None
        self._parameter_map: Mapping[str, str] = parameter_map if parameter_map is not None else {}

    def __enter__(self) -> None:
        """Record the values the public function's parameters start with.

        Recording is best effort: where frames cannot be inspected, for example because an
        audit hook refuses it, the call runs exactly as before and only loses the highlight.
        """
        frame: FrameType | None = None
        public: FrameType | None = None
        try:
            frame = inspect.currentframe()
            public = None if frame is None else frame.f_back
            if public is not None:
                code = public.f_code
                values = public.f_locals
                names = code.co_varnames[: code.co_argcount + code.co_kwonlyargcount]
                self._arguments = {name: values[name] for name in names if name in values}
        except Exception:
            self._arguments = None
        finally:
            # This frame refers to itself through ``frame``; dropping the references keeps
            # the caller's objects from waiting for the cycle collector.
            del frame, public

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        """Add notes to an umber error and drop the package's frames; never suppress it."""
        # The recorded arguments are needed at most until here; release them on every path.
        arguments, self._arguments = self._arguments, None
        if exc is None:
            return
        found = details(exc)
        if found is None:
            return
        parameter, hint = found
        public: FrameType | None = None
        caller: FrameType | None = None
        try:
            try:
                public = _public_frame()
                caller = None if public is None else public.f_back
                nested = caller is not None and _in_package(caller)
            except Exception:
                # Frames cannot be inspected: report here, without a highlight.
                public = caller = None
                nested = False
            if not nested and not hasattr(exc, _LOCATED):
                try:
                    self._add_notes(exc, public, caller, arguments, parameter, hint)
                except Exception:
                    # The notes only add to an error that is already complete. Building them
                    # reads the user's source and environment, so a failure there must not
                    # replace the error the user needs to see. BaseException is deliberately
                    # not caught: KeyboardInterrupt, SystemExit and GeneratorExit are requests
                    # to stop, not failures of the notes, and must propagate.
                    pass
                setattr(exc, _LOCATED, True)
            try:
                # The interpreter re-raises the exception with this traceback and adds the
                # frames above as it propagates, so the package frames never reappear.
                exc.__traceback__ = _without_package_frames(exc.__traceback__)
            except Exception:
                # Frames cannot be inspected: the full traceback stays.
                pass
        finally:
            del public, caller

    def _add_notes(
        self,
        exc: BaseException,
        public: FrameType | None,
        caller: FrameType | None,
        arguments: dict[str, object] | None,
        parameter: str | None,
        hint: str | None,
    ) -> None:
        """Add the highlight note, then the hint note, styled when colour is enabled."""
        color = color_enabled()
        if (
            public is not None
            and caller is not None
            and arguments is not None
            and parameter is not None
        ):
            highlight = find(
                caller, public, arguments, self._parameter_map.get(parameter, parameter)
            )
            if highlight is not None:
                exc.add_note(render(highlight, color=color))
        if hint is not None:
            exc.add_note(f"{bold('hint:') if color else 'hint:'} {hint}")


def api_call(*, parameter_map: Mapping[str, str] | None = None) -> ApiCall:
    """Return the reporter to wrap the whole body of a public function or method in.

    Args:
        parameter_map: Map nested error parameters to this call's parameters for highlighting.
            Mapping to ``self`` highlights the method call without changing the error's details.

    Returns:
        A context manager; see the module docstring.
    """
    return ApiCall(parameter_map)


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
