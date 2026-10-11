"""Finding the offending argument in the caller's source, and rendering the caret note.

The call site comes from the caller frame's instruction position (``co_positions``): the
call node with exactly that position is looked up in the parsed source of the caller.
Arguments are mapped to parameters only when the call provably invokes the function that
reports the error (see ``umber._callee``); a call through ``functools.partial``, another
wrapper or an expression that cannot be resolved without running code gets no highlight.

Whenever the source or the positions are unavailable, the call cannot be matched, or the
line cannot be measured in display columns, no highlight is produced; the message alone
stays complete.
"""

import ast
import linecache
import unicodedata
from dataclasses import dataclass
from itertools import islice
from types import CodeType, FrameType

from umber._ansi import bold, bold_red
from umber._callee import calls

_INDENT = "  "
_MISSING = object()

# Code points whose display width depends on the characters around them (joiners, emoji
# presentation and modifiers, flags, conjoining jamo, tags). A line holding any of them gets
# no highlight, because carets under it could land in the wrong column.
_UNMEASURABLE = (
    (0x1160, 0x11FF),  # Hangul jamo vowels and final consonants
    (0x200D, 0x200D),  # zero width joiner
    (0xD7B0, 0xD7FF),  # Hangul jamo extended-B
    (0xFE00, 0xFE0F),  # variation selectors
    (0x1F1E6, 0x1F1FF),  # regional indicators
    (0x1F3FB, 0x1F3FF),  # emoji modifiers
    (0xE0000, 0xE007F),  # tags
    (0xE0100, 0xE01EF),  # variation selectors supplement
)


@dataclass(frozen=True, slots=True)
class Highlight:
    """One source line with the span of the offending argument on it.

    Attributes:
        line: The source line as shown: indentation removed, each tab as one space.
        start: The display column where the argument starts.
        end: The display column where the argument ends on this line (exclusive).
    """

    line: str
    start: int
    end: int


def find(
    frame: FrameType, callee: str, target: CodeType, positional: tuple[str, ...], parameter: str
) -> Highlight | None:
    """Locate ``parameter`` in the call ``frame`` is executing.

    Args:
        frame: The caller's frame, suspended in the call to the API.
        callee: The name the call must end in, such as ``"line"`` for ``ax.line(...)``.
        target: The code of the API function that reports the error; the call must be
            provably a direct call of it.
        positional: The parameters that may be passed by position, in order, without
            ``self``.
        parameter: The parameter to highlight.

    Returns:
        The highlight, or ``None`` when the call or the argument cannot be located.
    """
    found = _call_node(frame)
    if found is None:
        return None
    call, lines = found
    if _callee_name(call.func) != callee or not calls(call.func, frame, target):
        return None
    argument = _argument(call, positional, parameter)
    if argument is None:
        return None
    # Columns are UTF-8 byte offsets, as in the source the interpreter compiled.
    end: int | None = None  # None: the argument continues past this line.
    if argument.end_lineno == argument.lineno and argument.end_col_offset is not None:
        end = argument.end_col_offset
    return _shown(lines[argument.lineno - 1], argument.col_offset, end)


def render(h: Highlight, *, color: bool) -> str:
    """Render ``h`` as a note: the line, and carets under the argument, each indented.

    Args:
        h: The highlight.
        color: Whether to style the argument bold and the carets bold red.

    Returns:
        Two lines, each indented by two spaces: the source line, then carets under the
        argument.
    """
    carets = "^" * (h.end - h.start)
    line = h.line
    if color:
        first = _index_at_column(line, h.start)
        last = _index_at_column(line, h.end)
        line = line[:first] + bold(line[first:last]) + line[last:]
        carets = bold_red(carets)
    return f"{_INDENT}{line}\n{_INDENT}{' ' * h.start}{carets}"


def _call_node(frame: FrameType) -> tuple[ast.Call, list[str]] | None:
    """Return the call ``frame`` is executing, as a node of its parsed source, and the lines."""
    code = frame.f_code
    position = next(islice(code.co_positions(), frame.f_lasti // 2, None), None)
    if position is None:
        return None
    first_line, last_line, first_col, last_col = position
    if first_line is None or last_line is None or first_col is None or last_col is None:
        return None
    try:
        lines = linecache.getlines(code.co_filename, frame.f_globals)
    except Exception:
        # linecache asks the module's loader for its source, which may fail in any way.
        return None
    if last_line > len(lines):
        return None
    try:
        tree = ast.parse("".join(lines))
    except (SyntaxError, ValueError, RecursionError, MemoryError):
        # Also covers a file that changed after it was compiled and no longer parses.
        return None
    for node in ast.walk(tree):
        if not (
            isinstance(node, ast.Call)
            and node.end_lineno == last_line
            and node.end_col_offset == last_col
        ):
            continue
        if node.lineno == first_line and node.col_offset == first_col:
            return node, lines
        # For a method whose name is on a later line than the start of the call, the
        # interpreter starts the position at the name, counting its length in code points.
        func = node.func
        if (
            isinstance(func, ast.Attribute)
            and func.end_lineno == first_line
            and func.end_col_offset is not None
            and func.end_col_offset - len(func.attr) == first_col
        ):
            return node, lines
    return None


def _callee_name(func: ast.expr) -> str | None:
    """Return the name a call expression ends in: ``f`` for ``f()``, ``g`` for ``a.b.g()``."""
    if isinstance(func, ast.Name):
        return func.id
    if isinstance(func, ast.Attribute):
        return func.attr
    return None


def _argument(
    call: ast.Call, positional: tuple[str, ...], parameter: str
) -> ast.expr | ast.keyword | None:
    """Return the node of ``parameter`` in ``call``: its keyword, else its positional slot."""
    for keyword in call.keywords:
        if keyword.arg == parameter:
            return keyword
    if parameter not in positional:
        return None
    index = positional.index(parameter)
    if index >= len(call.args):
        return None
    if any(isinstance(arg, ast.Starred) for arg in call.args[: index + 1]):
        return None
    return call.args[index]


def _shown(raw: str, start: int, end: int | None) -> Highlight | None:
    """Build the highlight of byte columns ``start..end`` of ``raw`` (``end=None``: line end)."""
    try:
        data = raw.rstrip("\r\n").encode("utf-8")
        head = data[:start].decode("utf-8")
        body = (data[start:] if end is None else data[start:end]).decode("utf-8")
    except UnicodeError:
        return None
    text = raw.rstrip("\r\n")
    if end is None:
        body = body.rstrip()
    indent = len(text) - len(text.lstrip(" \t\f"))
    if len(head) < indent or not body:
        return None
    line = text[indent:].rstrip().replace("\t", " ")
    if not _measurable(line):
        return None
    first = _width(head[indent:].replace("\t", " "))
    return Highlight(line=line, start=first, end=first + _width(body.replace("\t", " ")))


def _measurable(text: str) -> bool:
    """Return whether every character of ``text`` has a width independent of its neighbours."""
    for ch in text:
        point = ord(ch)
        if unicodedata.category(ch) == "Cc":
            return False
        if any(low <= point <= high for low, high in _UNMEASURABLE):
            return False
    return True


def _char_width(ch: str) -> int:
    """Return the display width of one character: 0, 1, or 2 for wide and fullwidth ones."""
    if unicodedata.combining(ch) or unicodedata.category(ch) in ("Mn", "Me", "Cf"):
        return 0
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


def _width(text: str) -> int:
    """Return the display width of ``text``."""
    return sum(_char_width(ch) for ch in text)


def _index_at_column(text: str, column: int) -> int:
    """Return the index of the first visible character of ``text`` at display ``column``.

    Zero-width characters belong to the character before them, so they stay on its side of
    the split.
    """
    width = 0
    for index, ch in enumerate(text):
        char_width = _char_width(ch)
        if width >= column and char_width > 0:
            return index
        width += char_width
    return len(text)
