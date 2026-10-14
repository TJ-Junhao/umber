"""Finding the offending argument in the caller's source, and rendering the caret note.

The call site comes from the caller frame's instruction position (``co_positions``): the
call node with exactly that position is looked up in the parsed source of the caller.
Nothing in the caller's namespace is read; whether the call passes its arguments straight
to the public function is decided from the shape of the call alone:

- The call must end in the public function's name (the class name for a constructor).
- It must not unpack arguments with ``*`` or ``**``.
- Its positional count and keyword names must bind to the function's parameters (read
  from its code object and defaults, never its annotations), and every parameter they
  leave out must still hold its default value in the running call.
- Every argument written as a literal must equal the value its parameter received.

A callable that adds arguments without a frame of its own, such as ``functools.partial``,
breaks the binding, changes a default or contradicts a literal, so it gets no highlight. A
Python wrapper has a frame of its own, which is then the caller, so the highlight lands
inside it. The argument the parameter binds to is the one highlighted.

The highlight is a convenience, and one case is a known limitation: C-level dispatch that
passes the public function different arguments under the same name and count, such as
``operator.methodcaller`` or a callback that a built-in calls (``list.sort(key=...)``), can
make the highlight point at an argument the function did not receive.

Whenever the source or the positions are unavailable, the source is too large to parse
quickly, the call does not fit, or the line cannot be measured in display columns, no
highlight is produced; the message alone stays complete.
"""

import ast
import inspect
import linecache
import unicodedata
from collections.abc import Mapping
from dataclasses import dataclass
from itertools import islice
from types import FrameType, FunctionType
from typing import cast

from umber._ansi import bold, bold_red

_INDENT = "  "
_NOT_LITERAL = object()
_MAX_SOURCE_BYTES = 512 * 1024
"""Larger sources are not parsed: the note is optional and parsing costs grow with size."""

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
    caller: FrameType, public: FrameType, arguments: Mapping[str, object], parameter: str
) -> Highlight | None:
    """Locate the argument for ``parameter`` in the call ``caller`` is executing.

    Args:
        caller: The caller's frame, suspended in the call to the public function.
        public: The frame of the public function, a direct callee of ``caller``.
        arguments: The values the public function's parameters had when it started.
        parameter: The parameter to highlight.

    Returns:
        The highlight, or ``None`` when the call or the argument cannot be located.
    """
    resolved = _function(public)
    if resolved is None:
        return None
    function, binds_first = resolved
    found = _call_node(caller)
    if found is None:
        return None
    call, lines = found
    if _callee_name(call.func) != _public_name(function):
        return None
    argument = _bound_argument(call, function, binds_first, arguments, parameter)
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
    source = "".join(lines)
    try:
        if len(source.encode("utf-8")) > _MAX_SOURCE_BYTES:
            return None
        tree = ast.parse(source)
    except (SyntaxError, ValueError, RecursionError, MemoryError):
        # Also covers a file that changed after it was compiled and no longer parses, and
        # source text that cannot be encoded (UnicodeEncodeError is a ValueError).
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


def _function(public: FrameType) -> tuple[FunctionType, bool] | None:
    """Return the function ``public`` runs and whether its calls bind ``self`` or ``cls``.

    The function is found by its qualified name in its own module and checked to be the one
    running, so nothing outside the package is consulted.
    """
    code = public.f_code
    parts = code.co_qualname.split(".")
    if "<locals>" in parts:
        return None
    value: object = public.f_globals.get(parts[0])
    for part in parts[1:]:
        if not isinstance(value, type):
            return None
        value = vars(value).get(part)
    # A method binds its instance and a class method its class; a static method binds none.
    binds_first = len(parts) > 1 and not isinstance(value, staticmethod)
    if isinstance(value, classmethod | staticmethod):
        wrapper = cast("classmethod[object, ..., object] | staticmethod[..., object]", value)
        value = wrapper.__func__
    if type(value) is not FunctionType or value.__code__ is not code:
        return None
    return value, binds_first


def _public_name(function: FunctionType) -> str:
    """Return the name a call of ``function`` ends in: the class name for ``__init__``."""
    if function.__name__ == "__init__":
        return function.__qualname__.split(".")[-2]
    return function.__name__


def _callee_name(func: ast.expr) -> str | None:
    """Return the name a call expression ends in: ``f`` for ``f()``, ``g`` for ``a.b.g()``."""
    if isinstance(func, ast.Name):
        return func.id
    if isinstance(func, ast.Attribute):
        return func.attr
    return None


def _bound_argument(
    call: ast.Call,
    function: FunctionType,
    binds_first: bool,
    arguments: Mapping[str, object],
    parameter: str,
) -> ast.expr | ast.keyword | None:
    """Return the node ``parameter`` binds to, if the call's shape fits ``function``.

    The binding follows Python's rules for the function's parameters, read from its code
    object and defaults; its annotations are never touched. The bound ``self`` or ``cls``
    parameter identifies the whole call.
    """
    if any(isinstance(arg, ast.Starred) for arg in call.args):
        return None
    if any(keyword.arg is None for keyword in call.keywords):
        return None
    code = function.__code__
    if code.co_flags & (inspect.CO_VARARGS | inspect.CO_VARKEYWORDS):
        return None
    skip = 1 if binds_first else 0
    names = code.co_varnames[: code.co_argcount + code.co_kwonlyargcount]
    positional = names[skip : code.co_argcount]
    positional_only = positional[: max(code.co_posonlyargcount - skip, 0)]
    keyword_only = names[code.co_argcount :]
    defaults = _defaults(function)
    if len(call.args) > len(positional):
        return None
    bound: dict[str, ast.expr | ast.keyword] = dict(zip(positional, call.args, strict=False))
    for keyword in call.keywords:
        name = keyword.arg
        if name is None or name in bound or name in positional_only:
            return None
        if name not in positional and name not in keyword_only:
            return None
        bound[name] = keyword
    for name in (*positional, *keyword_only):
        if name in bound:
            if not _agrees(bound[name], arguments, name):
                return None
            continue
        # A parameter the source leaves out must have its default; anything else was supplied
        # by a callable between the source and the function, such as functools.partial.
        if name not in defaults or name not in arguments or arguments[name] is not defaults[name]:
            return None
    if binds_first and parameter == names[0]:
        return call
    return bound.get(parameter)


def _defaults(function: FunctionType) -> dict[str, object]:
    """Return the default value of each parameter of ``function`` that has one."""
    code = function.__code__
    positional = code.co_varnames[: code.co_argcount]
    values = function.__defaults__ or ()
    defaults: dict[str, object] = dict(
        zip(positional[len(positional) - len(values) :], values, strict=True)
    )
    defaults.update(function.__kwdefaults__ or {})
    return defaults


def _agrees(node: ast.expr | ast.keyword, arguments: Mapping[str, object], name: str) -> bool:
    """Return whether a literal argument has the value the parameter received.

    A callable such as ``functools.partial`` can shift the source's arguments onto other
    parameters; where an argument is a literal, the shift shows as a different value.
    Arguments that are not literals are not evaluated and always agree.
    """
    value = node.value if isinstance(node, ast.keyword) else node
    literal = _literal(value)
    if literal is _NOT_LITERAL:
        return True
    if name not in arguments:
        return False
    received = arguments[name]
    # Both values have the same built-in type, so the comparison runs no user code.
    return type(received) is type(literal) and received == literal


def _literal(node: ast.expr) -> object:
    """Return the value of a constant or a negated numeric constant, else a marker."""
    if isinstance(node, ast.Constant):
        return node.value
    if (
        isinstance(node, ast.UnaryOp)
        and isinstance(node.op, ast.USub)
        and isinstance(node.operand, ast.Constant)
        and type(node.operand.value) in (int, float, complex)
    ):
        number = cast(int | float | complex, node.operand.value)
        return -number
    return _NOT_LITERAL


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
