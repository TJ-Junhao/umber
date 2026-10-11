"""Finding the offending argument in the caller's source, and rendering the caret note.

The call site comes from the caller frame's instruction position (``co_positions``), the
argument from parsing that call with ``ast``. Whenever the source or the positions are not
available, or the call cannot be matched, no highlight is produced; the message alone stays
complete.
"""

import ast
import linecache
import unicodedata
from dataclasses import dataclass
from itertools import islice
from types import FrameType

from umber._ansi import bold, bold_red

_INDENT = "  "


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
    frame: FrameType, callee: str, positional: tuple[str, ...], parameter: str
) -> Highlight | None:
    """Locate ``parameter`` in the call ``frame`` is executing.

    Args:
        frame: The caller's frame, suspended in the call to the API.
        callee: The name the call must end in, such as ``"line"`` for ``ax.line(...)``.
        positional: The parameters that may be passed by position, in order, without
            ``self``.
        parameter: The parameter to highlight.

    Returns:
        The highlight, or ``None`` when the call or the argument cannot be located.
    """
    span = _call_span(frame)
    if span is None:
        return None
    first_line, first_col, source = span
    try:
        tree = ast.parse("(" + source + ")", mode="eval")
    except (SyntaxError, ValueError, RecursionError):
        return None
    call = tree.body
    if not (isinstance(call, ast.Call) and _callee_name(call.func) == callee):
        return None
    argument = _argument(call, positional, parameter)
    if argument is None:
        return None
    # The parsed text's first line is "(" followed by the source from first_col; its later
    # lines are whole source lines. Columns are UTF-8 byte offsets throughout.
    shift = first_col - 1 if argument.lineno == 1 else 0
    end: int | None = None  # None: the argument continues past this line.
    if argument.end_lineno == argument.lineno and argument.end_col_offset is not None:
        end = argument.end_col_offset + shift
    raw = _source_line(frame, first_line + argument.lineno - 1)
    if raw is None:
        return None
    return _shown(raw, argument.col_offset + shift, end)


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


def _call_span(frame: FrameType) -> tuple[int, int, str] | None:
    """Return the first line, first byte column and source text of the call in ``frame``."""
    code = frame.f_code
    position = next(islice(code.co_positions(), frame.f_lasti // 2, None), None)
    if position is None:
        return None
    first_line, last_line, first_col, last_col = position
    if first_line is None or last_line is None or first_col is None or last_col is None:
        return None
    lines: list[bytes] = []
    for number in range(first_line, last_line + 1):
        line = _source_line(frame, number)
        if line is None:
            return None
        lines.append(line.encode("utf-8"))
    if first_line == last_line:
        text = lines[0][first_col:last_col]
    else:
        text = lines[0][first_col:] + b"".join(lines[1:-1]) + lines[-1][:last_col]
    try:
        return first_line, first_col, text.decode("utf-8")
    except UnicodeDecodeError:
        return None


def _source_line(frame: FrameType, number: int) -> str | None:
    """Return line ``number`` (1-based) of the frame's source file, or ``None``."""
    lines = linecache.getlines(frame.f_code.co_filename, frame.f_globals)
    if not 1 <= number <= len(lines):
        return None
    return lines[number - 1]


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
    data = raw.rstrip("\r\n").encode("utf-8")
    try:
        head = data[:start].decode("utf-8")
        body = (data[start:] if end is None else data[start:end]).decode("utf-8")
    except UnicodeDecodeError:
        return None
    text = data.decode("utf-8")
    if end is None:
        body = body.rstrip()
    indent = len(text) - len(text.lstrip(" \t\f"))
    if len(head) < indent or not body:
        return None
    line = text[indent:].rstrip().replace("\t", " ")
    first = _width(head[indent:].replace("\t", " "))
    return Highlight(line=line, start=first, end=first + _width(body.replace("\t", " ")))


def _width(text: str) -> int:
    """Return the display width of ``text``: wide and fullwidth characters take two columns."""
    return sum(2 if unicodedata.east_asian_width(ch) in "WF" else 1 for ch in text)


def _index_at_column(text: str, column: int) -> int:
    """Return the index of the character of ``text`` at display ``column``."""
    width = 0
    for index, ch in enumerate(text):
        if width >= column:
            return index
        width += _width(ch)
    return len(text)
