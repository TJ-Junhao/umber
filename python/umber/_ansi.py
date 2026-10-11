"""Whether error notes are coloured, and the ANSI styles they use.

The decision follows the rules CPython 3.13 applies to its own tracebacks, in this order:

1. Unless Python ignores the environment (``-E``, ``-I``), ``PYTHON_COLORS=0`` turns colour
   off and ``PYTHON_COLORS=1`` turns it on.
2. A non-empty ``NO_COLOR`` turns it off.
3. A non-empty ``FORCE_COLOR`` turns it on.
4. ``TERM=dumb`` turns it off.
5. Otherwise colour is on when ``sys.stderr`` is a terminal. On Windows the console must
   also support virtual terminal sequences, where Python can tell (3.13 and later).

The same rules apply on Python 3.12, where the interpreter itself never colours tracebacks.
"""

import importlib
import io
import os
import sys

_BOLD = "\x1b[1m"
_BOLD_RED = "\x1b[1;31m"
_RESET = "\x1b[0m"


def color_enabled() -> bool:
    """Return whether notes are coloured now, from the environment and ``sys.stderr``."""
    if not sys.flags.ignore_environment:
        python_colors = os.environ.get("PYTHON_COLORS")
        if python_colors == "0":
            return False
        if python_colors == "1":
            return True
    if os.environ.get("NO_COLOR"):
        return False
    if os.environ.get("FORCE_COLOR"):
        return True
    if os.environ.get("TERM") == "dumb":
        return False
    return _stderr_is_terminal() and _console_supports_sequences()


def _stderr_is_terminal() -> bool:
    """Return whether ``sys.stderr`` is attached to a terminal.

    ``sys.stderr`` may be any object the program installed, and this runs while an error is
    being reported, so a stream that cannot answer counts as no terminal instead of raising.
    """
    stderr = sys.stderr
    # Looking an attribute up can run the stream's own code too, so it is guarded as well.
    try:
        fileno = getattr(stderr, "fileno", None)
        if fileno is None:
            return False
        return os.isatty(fileno())
    except io.UnsupportedOperation:
        # A stream without a real descriptor, such as a captured stream, decides itself.
        pass
    except Exception:
        # A closed or detached stream, or a replacement whose fileno fails otherwise.
        return False
    try:
        isatty = getattr(stderr, "isatty", None)
        return isatty is not None and bool(isatty())
    except Exception:
        return False


def _console_supports_sequences() -> bool:
    """Return whether the Windows console interprets ANSI sequences; always true elsewhere."""
    if sys.platform != "win32":
        return True
    try:
        nt = importlib.import_module("nt")
    except ImportError:
        return False
    supports = getattr(nt, "_supports_virtual_terminal", None)
    # Python 3.12 cannot tell, so the terminal check alone decides.
    return supports is None or bool(supports())


def bold(text: str) -> str:
    """Return ``text`` styled bold."""
    return f"{_BOLD}{text}{_RESET}"


def bold_red(text: str) -> str:
    """Return ``text`` styled bold red."""
    return f"{_BOLD_RED}{text}{_RESET}"
