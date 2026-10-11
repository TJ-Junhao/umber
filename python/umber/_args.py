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
import reprlib
from typing import Final, SupportsFloat, SupportsIndex, cast, override

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
_PATH_HINT: Final = "pass a str or a pathlib.Path."
_BYTES_PATH_HINT: Final = "decode a bytes path with os.fsdecode()."
_TOO_LONG_HINT: Final = "shorten the text; it would not fit the figure."
_SURROGATE_HINT: Final = (
    'remove the character, or replace it: value.encode("utf-8", "replace").decode().'
)


def type_name(value: object) -> str:
    """Return the name of a value's type for an error message; ``None`` is named ``None``.

    Args:
        value: Any object.

    Returns:
        The qualified name of the value's type, or ``"None"``.

    Example:
        >>> from umber._args import type_name
        >>> type_name(1.5), type_name(None)
        ('float', 'None')
    """
    return "None" if value is None else type(value).__qualname__


class _BoundedRepr(reprlib.Repr):
    """Show at most a few items and levels of a container, without formatting huge ints."""

    def __init__(self) -> None:
        super().__init__(
            maxlevel=2,
            maxtuple=4,
            maxlist=4,
            maxarray=4,
            maxdict=4,
            maxset=4,
            maxfrozenset=4,
            maxdeque=4,
            maxstring=_REPR_LENGTH,
            maxlong=_REPR_LENGTH,
            maxother=_REPR_LENGTH,
        )

    @override
    def repr_int(self, x: int, level: int) -> str:
        return _int_repr(x)

    @override
    def repr_instance(self, x: object, level: int) -> str:
        try:
            text = repr(x)
        except Exception:
            return type_name(x)
        return text if len(text) <= _REPR_LENGTH else text[: _REPR_LENGTH - 1] + "…"


def _int_repr(value: int) -> str:
    """The decimal form of an int, or its size when it has more than 128 bits."""
    bits = int.bit_length(value)
    if bits > _REPR_INT_BITS:
        return f"{type_name(value)} ({bits:,} bits)"
    return repr(value)


_CONTAINER_REPR: Final = _BoundedRepr()


def short_repr(value: object) -> str:
    """Return ``repr(value)`` cut to at most 40 characters, ending with "…" when cut.

    The work is bounded: long text is cut before it is formatted, containers show only their
    first items, and an int of more than 128 bits is shown by its size. When ``repr`` raises,
    the type name is shown instead. This function never raises.

    Args:
        value: Any object.

    Returns:
        The representation, shortened if needed.

    Example:
        >>> from umber._args import short_repr
        >>> short_repr([1, 2])
        '[1, 2]'
        >>> cut = short_repr("x" * 50)
        >>> len(cut), cut[-1]
        (40, '…')
        >>> short_repr(10**100)
        'int (333 bits)'
    """
    # Types are tested with ``issubclass`` so that ``value`` keeps its declared type.
    kind = type(value)
    try:
        if issubclass(kind, int) and not issubclass(kind, bool):
            text = _int_repr(cast(int, value))
        elif issubclass(kind, str | bytes | bytearray):
            # One character more than fits, so that a longer value is still cut.
            text = repr(cast(str | bytes | bytearray, value)[: _REPR_LENGTH + 1])
        elif issubclass(kind, list | tuple | dict | set | frozenset):
            text = _CONTAINER_REPR.repr(value)
        else:
            text = repr(value)
    except Exception:
        return type_name(value)
    return text if len(text) <= _REPR_LENGTH else text[: _REPR_LENGTH - 1] + "…"


def _number_kind(value: object) -> str | None:
    """Return ``"int"`` or ``"float"`` for an accepted number type, else ``None``.

    Accepted are ``int`` (not ``bool``), ``float``, and the NumPy integer and floating scalars
    except ``timedelta64``. NumPy types are recognised by their class hierarchy, so this
    module does not import NumPy.
    """
    kind = type(value)
    if issubclass(kind, bool):
        return None
    if issubclass(kind, int):
        return "int"
    if issubclass(kind, float):
        return "float"
    numpy_bases = {base.__name__ for base in kind.__mro__ if base.__module__ == "numpy"}
    if "timedelta64" in numpy_bases:
        return None
    if "integer" in numpy_bases:
        return "int"
    if "floating" in numpy_bases:
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
) -> str:
    """Apply the length cap and the UTF-8 rule to a string, and return it as a plain ``str``.

    Args:
        context: The API call, such as ``"Axes.bar()"``.
        name: The parameter the string belongs to.
        label: How the message names the string: the parameter, or an element such as
            ``"categories[2]"``.
        value: The string, possibly of a ``str`` subclass.
        unencodable: The error class for a string holding a lone surrogate: ``StyleError``
            for text arguments, ``DataError`` for names in data.

    Returns:
        The string as a plain ``str``.

    Raises:
        StyleError: The string is longer than ``MAX_TEXT_LENGTH`` characters.
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
    if len(plain) > MAX_TEXT_LENGTH:
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


def text(context: str, name: str, value: object) -> str:
    """Check a text argument, such as a title or a label, and return it as a plain ``str``.

    Args:
        context: The API call, such as ``"Axes.title()"``.
        name: The parameter name.
        value: The value passed. ``str`` subclasses are accepted.

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
        raise _wrong_type(context, name, value, "a str", _TEXT_HINT)
    return checked_text(context, name, name, value, StyleError)


def real(context: str, name: str, value: object) -> float:
    """Check a number argument and return it as a ``float``.

    Ints, floats and the NumPy integer and floating scalars are accepted. ``bool``, NumPy
    ``bool`` and ``timedelta64``, ``Fraction`` and ``Decimal`` are not. An int too large for a
    float becomes infinity, which the core library then rejects.

    Args:
        context: The API call, such as ``"Axes.xlim()"``.
        name: The parameter name.
        value: The value passed.

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
        raise _wrong_type(context, name, value, "a float", _REAL_HINT)
    number = operator.index(cast(SupportsIndex, value))
    try:
        return float(number)
    except OverflowError:
        return math.inf if number > 0 else -math.inf


def integer(context: str, name: str, value: object) -> int:
    """Check a whole-number argument and return it clamped to the 64-bit signed range.

    Ints and NumPy integer scalars are accepted; ``bool``, NumPy ``bool`` and ``timedelta64``
    are not. Every valid range lies far
    inside the 64-bit range, so a clamped value fails the core library's check as the
    original would.

    Args:
        context: The API call, such as ``"Axes.hist()"``.
        name: The parameter name.
        value: The value passed.

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
        raise _wrong_type(context, name, value, "an int", _INTEGER_HINT)
    return min(max(operator.index(cast(SupportsIndex, value)), _I64_MIN), _I64_MAX)


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
        Axes.line(): dash is str; dash must be a Dash.
    """
    if not isinstance(value, cls):
        article = "an" if cls.__name__[:1].lower() in "aeiou" else "a"
        raise _wrong_type(context, name, value, f"{article} {cls.__name__}", hint)
    return value


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
