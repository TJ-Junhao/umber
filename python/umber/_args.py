r"""Type checks and conversions for scalar, text and path arguments.

Each function takes the API call's context (such as ``"Axes.title()"``), the parameter name
and the value the caller passed, and returns the value in the plain form the native module
takes. A value of the wrong type raises ``TypeError``; text that is too long, cannot be
encoded as UTF-8 or holds a character text cannot hold raises ``StyleError``. Every error
carries the parameter name and a hint. Range checks on the values themselves belong to the
core library, not to these functions.

Only ``"\n"`` breaks a line, and only in titles, subtitles and the text of ``Axes.text()`` and
``Axes.annotate()`` (``multiline_text``); all other text is one line (``text``,
``checked_text``). No text may hold another control character (U+0000 to U+001F, U+007F to
U+009F) or a line or paragraph separator (U+2028, U+2029).
"""

import math
import operator
import os
import re
import sys
from collections.abc import Iterable
from typing import Any, Final, SupportsFloat, SupportsIndex, cast

from umber._errors import DataError, StyleError, new_error

MAX_TEXT_LENGTH: Final[int] = 1000
"""The longest text argument, in characters: titles, labels and each category name.

Longer text raises ``StyleError``; it could not fit a figure.

Example:
    >>> from umber._args import MAX_TEXT_LENGTH
    >>> MAX_TEXT_LENGTH
    1000
"""

_I64_MIN: Final = -(2**63)
_I64_MAX: Final = 2**63 - 1
_REPR_LENGTH: Final = 40
# Ints with more bits than this are shown by their size: their decimal form is cut anyway, and
# Python refuses to format ints of more than 4300 digits.
_REPR_INT_BITS: Final = 128
_SURROGATE: Final = re.compile(r"[\ud800-\udfff]")
# The characters text of one line may not hold, and those text of several lines may not hold:
# the same but for "\n".
_ONE_LINE_REFUSED: Final = re.compile(r"[\x00-\x1f\x7f-\x9f\u2028\u2029]")
_LINES_REFUSED: Final = re.compile(r"[\x00-\x09\x0b-\x1f\x7f-\x9f\u2028\u2029]")

_TEXT_HINT: Final = "pass a str; convert other values with str()."
_REAL_HINT: Final = "pass a number such as 1.5."
_INTEGER_HINT: Final = "pass a whole number such as 3."
_BOOLEAN_HINT: Final = "pass True or False."
_PATH_HINT: Final = "pass a str or a pathlib.Path."
_BYTES_PATH_HINT: Final = "decode a bytes path with os.fsdecode()."
_TOO_LONG_HINT: Final = "shorten the text; it would not fit the figure."
_SURROGATE_HINT: Final = (
    'remove the character, or replace it: value.encode("utf-8", "replace").decode().'
)
_LINE_BREAK_HINT: Final = "line breaks are supported in titles, subtitles, text() and annotate()."
_SEPARATORS: Final = "\u2028\u2029"
# What a refused character is, after its code point, where its code point alone does not say.
_CHARACTER_NAMES: Final = {
    "\t": " (a tab)",
    "\r": " (a carriage return)",
    "\u2028": " (a line separator)",
    "\u2029": " (a paragraph separator)",
}
_ONE_LINE_HINTS: Final = {
    "\n": _LINE_BREAK_HINT,
    "\t": "use spaces instead of tabs.",
    "\r": "remove the carriage return.",
    "\u2028": "remove the separator.",
    "\u2029": "remove the separator.",
}
_LINES_HINTS: Final = {
    "\t": 'use spaces instead of tabs, and "\\n" for a line break.',
    "\r": 'use "\\n" alone for a line break.',
    "\u2028": 'use "\\n" for a line break.',
    "\u2029": 'use "\\n" for a line break.',
}
_CHARACTER_HINT: Final = "remove the character."


_QUALNAME: Final[Any] = type.__dict__["__qualname__"]


def type_name(value: object) -> str:
    """Return the name of a value's type for an error message; ``None`` is named ``None``.

    The name is read from the type itself, bypassing any ``__qualname__`` a metaclass defines,
    so no user code runs. NumPy scalar names include ``numpy.`` to distinguish them from
    built-in types. It is ``"object"`` if the name cannot be read.

    Args:
        value: Any object.

    Returns:
        The qualified name of the value's type, or ``"None"``.

    Example:
        >>> from umber._args import type_name
        >>> type_name(1.5), type_name(None)
        ('float', 'None')
    """
    if value is None:
        return "None"
    try:
        kind = type(value)
        name: object = _QUALNAME.__get__(kind)
        if not isinstance(name, str):
            return "object"
        name = str.__str__(name)
        np = sys.modules.get("numpy")
        if np is not None and any(kind is scalar for scalar in np.sctypeDict.values()):
            return f"numpy.{name}"
        return name
    except Exception:
        return "object"


def is_text_or_buffer(value: object) -> bool:
    """Whether a value is text or a binary buffer rather than a data container.

    Args:
        value: Any object.

    Returns:
        Whether the value is a ``str``, ``bytes``, ``bytearray`` or ``memoryview``.

    Example:
        >>> from umber._args import is_text_or_buffer
        >>> is_text_or_buffer(bytearray()), is_text_or_buffer([])
        (True, False)
    """
    return isinstance(value, str | bytes | bytearray | memoryview)


def _numpy_scalar_types() -> frozenset[type]:
    """The NumPy number, bool, datetime and timedelta scalar types, if NumPy is imported."""
    np = sys.modules.get("numpy")
    if np is None:
        return frozenset()
    shown = (np.number, np.bool_, np.datetime64, np.timedelta64)
    types = cast(Iterable[type], np.sctypeDict.values())
    return frozenset(kind for kind in types if issubclass(kind, shown))


def short_repr(value: object) -> str:
    """Show a value in an error message, in at most 40 characters, ending with "…" when cut.

    Only values whose representation runs no user code are shown: ``None``, ``bool``,
    ``int`` (more than 128 bits are shown by their size), ``float``, ``str`` (subclasses as
    the plain text they hold) and the NumPy number, bool, datetime and timedelta scalars.
    Any other value, including containers, is shown by its type name.

    Args:
        value: Any object.

    Returns:
        The representation, shortened if needed.

    Example:
        >>> from umber._args import short_repr
        >>> short_repr(2.5), short_repr("North"), short_repr([1, 2])
        ('2.5', "'North'", 'list')
        >>> cut = short_repr("x" * 50)
        >>> len(cut), cut[-1]
        (40, '…')
        >>> short_repr(10**100)
        'int (333 bits)'
    """
    kind = type(value)
    if value is None or kind is bool or kind is float:
        text = repr(value)
    elif kind is int:
        bits = int.bit_length(cast(int, value))
        text = f"int ({bits:,} bits)" if bits > _REPR_INT_BITS else repr(value)
    elif issubclass(kind, str):
        # One character more than fits, so that a longer value is still cut.
        text = repr(str.__str__(cast(str, value))[: _REPR_LENGTH + 1])
    elif kind in _numpy_scalar_types():
        text = repr(value)
    else:
        text = type_name(value)
    text = str.__str__(text)
    return text if len(text) <= _REPR_LENGTH else text[: _REPR_LENGTH - 1] + "…"


def _number_kind(value: object) -> str | None:
    """Return ``"int"`` or ``"float"`` for an accepted number type, else ``None``.

    Accepted are ``int`` (not ``bool``), ``float``, and the NumPy integer and floating scalars
    except ``timedelta64``. NumPy's own classes are used when NumPy is imported; this module
    does not import it, and without it no value can be a NumPy scalar.
    """
    kind = type(value)
    if issubclass(kind, bool):
        return None
    if issubclass(kind, int):
        return "int"
    if issubclass(kind, float):
        return "float"
    np = sys.modules.get("numpy")
    if np is None or isinstance(value, np.timedelta64 | np.bool_):
        return None
    if isinstance(value, np.integer):
        return "int"
    if isinstance(value, np.floating):
        return "float"
    return None


def _wrong_type(context: str, name: str, value: object, expected: str, hint: str) -> TypeError:
    message = f"{name} is {type_name(value)}; {name} must be {expected}."
    return new_error(TypeError, context, message, parameter=name, hint=hint)


def _character_message(label: str, found: re.Match[str]) -> str:
    """The message for a control character or separator ``found`` in the string ``label``."""
    character = found.group()
    code = f"U+{ord(character):04X}{_CHARACTER_NAMES.get(character, '')}"
    reason = (
        "text cannot hold line or paragraph separators."
        if character in _SEPARATORS
        else "text cannot hold control characters."
    )
    return f"{label} holds {code} at index {found.start()}; {reason}"


def _plain(
    context: str,
    name: str,
    label: str,
    value: str,
    invalid: type[DataError] | type[StyleError],
    check_length: bool,
) -> str:
    """Return the string as a plain ``str`` after the length and encoding checks."""
    # The checks run on the plain ``str`` that is passed on, so a subclass that overrides
    # ``__len__`` or ``__str__`` cannot change what they see.
    plain = str.__str__(value)
    if check_length and len(plain) > MAX_TEXT_LENGTH:
        message = (
            f"{label} has {len(plain):,} characters; "
            f"text must be at most {MAX_TEXT_LENGTH:,} characters."
        )
        raise new_error(StyleError, context, message, parameter=name, hint=_TOO_LONG_HINT)
    found = _SURROGATE.search(plain)
    if found is not None:
        message = (
            f"{label} has a lone surrogate at index {found.start()}; text must be valid Unicode."
        )
        raise new_error(invalid, context, message, parameter=name, hint=_SURROGATE_HINT)
    return plain


def checked_text(
    context: str,
    name: str,
    label: str,
    value: str,
    invalid: type[DataError] | type[StyleError],
    *,
    one_line: str,
    check_length: bool = True,
) -> str:
    """Apply the rules of text of one line to a string and return it as a plain ``str``.

    Args:
        context: The API call, such as ``"Axes.bar()"``.
        name: The parameter the string belongs to.
        label: How the message names the string: the parameter, or an element such as
            ``"categories[2]"``.
        value: The string, possibly of a ``str`` subclass.
        invalid: The error class for a string holding a lone surrogate, a line break or
            another refused character: ``StyleError`` for text arguments, ``DataError`` for
            names in data.
        one_line: What the string is, in the plural, for the message on a line break, such as
            ``"category names"``: ``"categories[2] holds a line break at index 5; category
            names are one line."``.
        check_length: Whether to apply the length cap here; false when core checks it in
            the context of the other arguments. UTF-8 encoding and the characters are always
            checked.

    Returns:
        The string as a plain ``str``.

    Raises:
        StyleError: ``check_length`` is true and the string exceeds ``MAX_TEXT_LENGTH``.
        DataError: The string holds a lone surrogate, a line break, another control
            character (U+0000 to U+001F, U+007F to U+009F) or a line or paragraph separator
            (U+2028, U+2029), and ``invalid`` is ``DataError``; ``StyleError`` when
            ``invalid`` is ``StyleError``.

    Example:
        >>> from umber._args import checked_text
        >>> from umber._errors import DataError
        >>> checked_text(
        ...     "Axes.bar()", "categories", "categories[0]", "North", DataError,
        ...     one_line="category names",
        ... )
        'North'
    """
    plain = _plain(context, name, label, value, invalid, check_length)
    found = _ONE_LINE_REFUSED.search(plain)
    if found is not None:
        character = found.group()
        message = (
            f"{label} holds a line break at index {found.start()}; {one_line} are one line."
            if character == "\n"
            else _character_message(label, found)
        )
        hint = _ONE_LINE_HINTS.get(character, _CHARACTER_HINT)
        raise new_error(invalid, context, message, parameter=name, hint=hint)
    return plain


def require_text(context: str, name: str, value: object) -> None:
    """Check that a text argument is a ``str``, without checking its length or encoding.

    Use it to run the type checks of all parameters before any value check.

    Args:
        context: The API call, such as ``"TickFormat.affix()"``.
        name: The parameter name.
        value: The value passed.

    Raises:
        TypeError: The value is not a ``str``.

    Example:
        >>> from umber._args import require_text
        >>> require_text("TickFormat.affix()", "prefix", "$")
    """
    if not isinstance(value, str):
        raise _wrong_type(context, name, value, "a str", _TEXT_HINT)


def text(context: str, name: str, value: object, *, one_line: str, hint: str = _TEXT_HINT) -> str:
    r"""Check a text argument of one line, such as a label, and return it as a plain ``str``.

    Args:
        context: The API call, such as ``"Axes.xlabel()"``.
        name: The parameter name.
        value: The value passed. ``str`` subclasses are accepted.
        one_line: What the text is, in the plural, for the message on a line break, such as
            ``"axis labels"``.
        hint: The hint for a value of another type.

    Returns:
        The text as a plain ``str``.

    Raises:
        TypeError: The value is not a ``str``.
        StyleError: The text is longer than ``MAX_TEXT_LENGTH`` characters, holds a lone
            surrogate, which cannot be encoded as UTF-8, or holds a line break, another
            control character or a line or paragraph separator.

    Example:
        >>> from umber._args import text
        >>> text("Axes.xlabel()", "text", "Revenue", one_line="axis labels")
        'Revenue'
        >>> try:
        ...     text("Axes.xlabel()", "text", "Hours\nUTC", one_line="axis labels")
        ... except ValueError as err:
        ...     print(err)
        Axes.xlabel(): text holds a line break at index 5; axis labels are one line.
    """
    if not isinstance(value, str):
        raise _wrong_type(context, name, value, "a str", hint)
    return checked_text(context, name, name, value, StyleError, one_line=one_line)


def multiline_text(context: str, name: str, value: object) -> str:
    r"""Check a text argument that ``"\n"`` breaks into lines and return it as a plain ``str``.

    Titles, subtitles and the text of ``Axes.text()`` and ``Axes.annotate()`` take lines.

    Args:
        context: The API call, such as ``"Axes.title()"``.
        name: The parameter name.
        value: The value passed. ``str`` subclasses are accepted.

    Returns:
        The text as a plain ``str``.

    Raises:
        TypeError: The value is not a ``str``.
        StyleError: The text is longer than ``MAX_TEXT_LENGTH`` characters, holds a lone
            surrogate, or holds a control character other than ``"\n"`` or a line or
            paragraph separator.

    Example:
        >>> from umber._args import multiline_text
        >>> multiline_text("Axes.title()", "text", "Revenue\nper quarter")
        'Revenue\nper quarter'
        >>> try:
        ...     multiline_text("Axes.title()", "text", "Rev\tenue")
        ... except ValueError as err:
        ...     print(err)
        Axes.title(): text holds U+0009 (a tab) at index 3; text cannot hold control characters.
    """
    if not isinstance(value, str):
        raise _wrong_type(context, name, value, "a str", _TEXT_HINT)
    plain = _plain(context, name, name, value, StyleError, True)
    found = _LINES_REFUSED.search(plain)
    if found is not None:
        hint = _LINES_HINTS.get(found.group(), _CHARACTER_HINT)
        message = _character_message(name, found)
        raise new_error(StyleError, context, message, parameter=name, hint=hint)
    return plain


def real(
    context: str,
    name: str,
    value: object,
    *,
    expected: str = "a float",
    hint: str = _REAL_HINT,
) -> float:
    """Check a number argument and return it as a ``float``.

    Ints, floats and the NumPy integer and floating scalars are accepted. ``bool``, NumPy
    ``bool`` and ``timedelta64``, ``Fraction`` and ``Decimal`` are not. An int too large for a
    float becomes infinity, which the core library then rejects.

    Args:
        context: The API call, such as ``"Axes.xlim()"``.
        name: The parameter name.
        value: The value passed.
        expected: The accepted types to name in an error, including any handled by the caller.
        hint: The hint for a value of another type.

    Returns:
        The value as a ``float``.

    Raises:
        TypeError: The value is not one of the accepted number types.

    Example:
        >>> from umber._args import real
        >>> real("Axes.xlim()", "low", 3)
        3.0
    """
    if isinstance(value, float):
        return float.__float__(value)
    kind = _number_kind(value)
    if kind == "float":
        return float(cast(SupportsFloat, value))
    if kind != "int":
        raise _wrong_type(context, name, value, expected, hint)
    number = operator.index(cast(SupportsIndex, value))
    try:
        return float(number)
    except OverflowError:
        return math.inf if number > 0 else -math.inf


def integer(
    context: str,
    name: str,
    value: object,
    *,
    expected: str = "an int",
    hint: str = _INTEGER_HINT,
) -> int:
    """Check a whole-number argument and return it clamped to the 64-bit signed range.

    Ints and NumPy integer scalars are accepted; ``bool``, NumPy ``bool`` and ``timedelta64``
    are not. Every valid range lies far
    inside the 64-bit range, so a clamped value fails the core library's check as the
    original would.

    Args:
        context: The API call, such as ``"Axes.hist()"``.
        name: The parameter name.
        value: The value passed.
        expected: The accepted types to name in an error, including any handled by the caller.
        hint: The hint for a value of another type.

    Returns:
        The value as an ``int`` between -2**63 and 2**63 - 1.

    Raises:
        TypeError: The value is not an int or a NumPy integer scalar.

    Example:
        >>> from umber._args import integer
        >>> integer("Axes.hist()", "bins", 2**70)
        9223372036854775807
    """
    if _number_kind(value) != "int":
        raise _wrong_type(context, name, value, expected, hint)
    return min(max(operator.index(cast(SupportsIndex, value)), _I64_MIN), _I64_MAX)


def boolean(
    context: str,
    name: str,
    value: object,
    *,
    expected: str = "a bool",
    hint: str = _BOOLEAN_HINT,
) -> bool:
    """Check a flag argument and return it.

    Only ``bool`` is accepted: an int such as ``1`` or a NumPy ``bool`` is a ``TypeError``, so a
    flag is never set by accident.

    Args:
        context: The API call, such as ``"Axes.y_error_bars()"``.
        name: The parameter name.
        value: The value passed.
        expected: The accepted types to name in an error, including any handled by the caller.
        hint: The hint for a value of another type.

    Returns:
        The value, unchanged.

    Raises:
        TypeError: The value is not a ``bool``.

    Example:
        >>> from umber._args import boolean
        >>> boolean("Axes.y_error_bars()", "caps", False)
        False
        >>> try:
        ...     boolean("Axes.y_error_bars()", "caps", 1)
        ... except TypeError as err:
        ...     print(err)
        Axes.y_error_bars(): caps is int; caps must be a bool.
    """
    if not isinstance(value, bool):
        raise _wrong_type(context, name, value, expected, hint)
    return value


def instance[T](context: str, name: str, value: object, cls: type[T], hint: str) -> T:
    """Check that an argument is an instance of ``cls``, such as ``Color`` or an enum.

    Args:
        context: The API call, such as ``"Axes.line()"``.
        name: The parameter name.
        value: The value passed.
        cls: The required class.
        hint: The hint for a value of another type, such as ``"use Dash.SOLID, Dash.DASHED
            or Dash.DOTTED."``.

    Returns:
        The value, unchanged.

    Raises:
        TypeError: The value is not an instance of ``cls``.

    Example:
        >>> from enum import Enum
        >>> from umber._args import instance
        >>> class Dash(Enum):
        ...     SOLID = 0
        >>> try:
        ...     instance("Axes.line()", "dash", "solid", Dash, "use Dash.SOLID.")
        ... except TypeError as err:
        ...     print(err)
        Axes.line(): dash is str; dash must be of type Dash.
    """
    if not isinstance(value, cls):
        raise _wrong_type(context, name, value, f"of type {cls.__name__}", hint)
    return value


def require_path(context: str, name: str, value: object) -> None:
    """Check that a path argument is a ``str`` or path-like object, without calling it.

    A path-like object whose path is ``bytes`` is refused later, by ``path``.

    Args:
        context: The API call, such as ``"Figure.save_png()"``.
        name: The parameter name.
        value: The value passed.

    Raises:
        TypeError: The value is neither a ``str`` nor a path-like object.

    Example:
        >>> from umber._args import require_path
        >>> require_path("Figure.save_png()", "path", "sales.png")
    """
    if not isinstance(value, str | os.PathLike):
        hint = _BYTES_PATH_HINT if isinstance(value, bytes) else _PATH_HINT
        raise _wrong_type(context, name, value, "a str or a path-like object", hint)


def path(context: str, name: str, value: object) -> str:
    """Check a file path argument and return it as a ``str``.

    A ``str`` or an ``os.PathLike`` whose path is a ``str``, such as ``pathlib.Path``, is
    accepted. Bytes paths are not.

    Args:
        context: The API call, such as ``"Figure.save_png()"``.
        name: The parameter name.
        value: The value passed.

    Returns:
        The path as a ``str``.

    Raises:
        TypeError: The value is neither a ``str`` nor a path-like object, or its path is
            ``bytes``.

    Example:
        >>> from pathlib import Path
        >>> from umber._args import path
        >>> path("Figure.save_png()", "path", Path("sales.png"))
        'sales.png'
    """
    if isinstance(value, str):
        return str.__str__(value)
    if isinstance(value, os.PathLike):
        # A path-like object may return bytes. The inner cast only selects an overload; the
        # outer one widens the result, whose type is checked below.
        fspath = cast(object, os.fspath(cast("os.PathLike[str]", value)))
        if isinstance(fspath, str):
            return str.__str__(fspath)
        message = f"{name} is a bytes path; {name} must be a str path."
        raise new_error(TypeError, context, message, parameter=name, hint=_BYTES_PATH_HINT)
    hint = _BYTES_PATH_HINT if isinstance(value, bytes) else _PATH_HINT
    raise _wrong_type(context, name, value, "a str or a path-like object", hint)
