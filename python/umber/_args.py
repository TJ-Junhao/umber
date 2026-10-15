"""Type checks and conversions for scalar, text and path arguments.

Each function takes the API call's context (such as ``"Axes.title()"``), the parameter name
and the value the caller passed, and returns the value in the plain form the native module
takes. A value of the wrong type raises ``TypeError``; text that is too long or cannot be
encoded as UTF-8 raises ``StyleError``. Every error carries the parameter name and a hint.
Range checks on the values themselves belong to the core library, not to these functions.
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


_QUALNAME: Final[Any] = type.__dict__["__qualname__"]


def type_name(value: object) -> str:
    """Return the name of a value's type for an error message; ``None`` is named ``None``.

    The name is read from the type itself, bypassing any ``__qualname__`` a metaclass defines,
    so no user code runs. It is ``"object"`` if the name cannot be read.

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
        name: object = _QUALNAME.__get__(type(value))
        return str.__str__(name) if isinstance(name, str) else "object"
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


def checked_text(
    context: str,
    name: str,
    label: str,
    value: str,
    unencodable: type[DataError] | type[StyleError],
    *,
    check_length: bool = True,
) -> str:
    """Apply text rules to a string and return it as a plain ``str``.

    Args:
        context: The API call, such as ``"Axes.bar()"``.
        name: The parameter the string belongs to.
        label: How the message names the string: the parameter, or an element such as
            ``"categories[2]"``.
        value: The string, possibly of a ``str`` subclass.
        unencodable: The error class for a string holding a lone surrogate: ``StyleError``
            for text arguments, ``DataError`` for names in data.
        check_length: Whether to apply the length cap here; false when core checks it in
            the context of the other arguments. UTF-8 encoding is always checked.

    Returns:
        The string as a plain ``str``.

    Raises:
        StyleError: ``check_length`` is true and the string exceeds ``MAX_TEXT_LENGTH``.
        DataError: The string holds a lone surrogate and ``unencodable`` is ``DataError``.

    Example:
        >>> from umber._args import checked_text
        >>> from umber._errors import DataError
        >>> checked_text("Axes.bar()", "categories", "categories[0]", "North", DataError)
        'North'
    """
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
        raise new_error(unencodable, context, message, parameter=name, hint=_SURROGATE_HINT)
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


def text(context: str, name: str, value: object, *, hint: str = _TEXT_HINT) -> str:
    """Check a text argument, such as a title or a label, and return it as a plain ``str``.

    Args:
        context: The API call, such as ``"Axes.title()"``.
        name: The parameter name.
        value: The value passed. ``str`` subclasses are accepted.
        hint: The hint for a value of another type.

    Returns:
        The text as a plain ``str``.

    Raises:
        TypeError: The value is not a ``str``.
        StyleError: The text is longer than ``MAX_TEXT_LENGTH`` characters or holds a lone
            surrogate, which cannot be encoded as UTF-8.

    Example:
        >>> from umber._args import text
        >>> text("Axes.title()", "text", "Revenue")
        'Revenue'
        >>> try:
        ...     text("Axes.title()", "text", 42)
        ... except TypeError as err:
        ...     print(err)
        Axes.title(): text is int; text must be a str.
    """
    if not isinstance(value, str):
        raise _wrong_type(context, name, value, "a str", hint)
    return checked_text(context, name, name, value, StyleError)


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


def boolean(context: str, name: str, value: object) -> bool:
    """Check a flag argument and return it.

    Only ``bool`` is accepted: an int such as ``1`` or a NumPy ``bool`` is a ``TypeError``, so a
    flag is never set by accident.

    Args:
        context: The API call, such as ``"Axes.y_error_bars()"``.
        name: The parameter name.
        value: The value passed.

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
        raise _wrong_type(context, name, value, "a bool", _BOOLEAN_HINT)
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
