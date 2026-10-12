"""Conversion of data arguments to the two forms the native module reads.

Numbers become a one-dimensional, aligned, native-endian ``float64`` NumPy array; names
become a ``list`` of ``str``. Only the type, shape and dtype are checked here. Finiteness,
magnitudes, log ranges, lengths and duplicates are checked by the core library, so each rule
has one home. A container of the wrong type raises ``TypeError``; data that cannot be plotted
raises ``DataError``, naming the first offending element where there is one. Ints and
``float16``/``float32`` data are the only values converted; nothing else is coerced.
"""

import datetime
import math
import numbers
from collections.abc import Iterable, Iterator, Mapping, Sequence, Set
from typing import Any, Final, cast

import numpy as np
import numpy.typing as npt

from umber._args import checked_text, short_repr, type_name
from umber._errors import DataError, new_error

type FloatArray = npt.NDArray[np.float64]
"""A one-dimensional ``float64`` array in native byte order, aligned, of any stride.

Example:
    >>> from umber._values import floats
    >>> floats("Axes.hist()", "values", [1, 2]).dtype
    dtype('float64')
"""

type _Data = npt.NDArray[Any] | Sequence[object]

_SINGLE_HINT: Final = "wrap a single value in a list."
_NAMES_HINT: Final = 'pass a list of names, e.g. ["A", "B"].'
_AXIS_HINT: Final = 'pass a list of names or numbers, e.g. ["A", "B"] or [1.0, 2.0].'
_ITERATOR_HINT: Final = "wrap it in list()."
_SET_HINT: Final = "a set has no order; pass a list, e.g. sorted(values)."
_MAPPING_HINT: Final = "pass the values as a list, e.g. list(values.values())."
_OTHER_HINT: Final = "pass a list or an array."
_MASKED_HINT: Final = "fill them with NaN: values.filled(np.nan)."
_MASKED_NAMES_HINT: Final = 'fill them with a name first, e.g. values.filled("n/a").'
_NDIM_HINT: Final = "pass one column, or flatten it with .ravel()."
_BOOL_DTYPE_HINT: Final = "convert with .astype(float) if 0 and 1 are meant."
_BOOL_HINT: Final = "use 1.0 and 0.0 if numbers are meant."
_TIME_HINT: Final = "convert to numbers, e.g. seconds since the first value."
_COMPLEX_HINT: Final = "pass the real part, values.real, or the magnitude, abs(values)."
_WIDE_FLOAT_HINT: Final = "convert with .astype(np.float64); umber plots in float64 precision."
_STRUCTURED_HINT: Final = "pass one numeric field of the structured array."
_MISSING_HINT: Final = (
    "use NaN for missing values, e.g. series.to_numpy(dtype=float, na_value=np.nan)."
)
_TEXT_HINT: Final = "convert text to numbers first, e.g. with float()."
_NESTED_HINT: Final = "pass one number per point; nested sequences are not flattened."
_NUMBER_HINT: Final = "convert the values with float() first."
_NAME_HINT: Final = "convert each name with str() first."
_NAME_DTYPE_HINT: Final = "convert them with .astype(str) first."
_MISSING_NAME_HINT: Final = 'give every category a name, e.g. "n/a" for missing ones.'
_MIXED_HINT: Final = "pass only names or only numbers."

_BOOLS: Final = (bool, np.bool_)
_TIMES: Final = (datetime.date, datetime.time, datetime.timedelta, np.datetime64, np.timedelta64)


def _data(context: str, name: str, values: object, *, str_hint: str, masked_hint: str) -> _Data:
    """Check the container of a data argument and return it as an array or a sequence.

    Objects with ``__array__`` are converted with ``np.asarray``; other sequences are returned
    as they are. Text, scalars, masked arrays and containers without an order are refused.
    """
    _check_container(context, name, values, str_hint=str_hint, masked_hint=masked_hint)
    if hasattr(values, "__array__"):
        return np.asarray(values)
    return cast(Sequence[object], values)


def _check_container(
    context: str,
    name: str,
    values: object,
    *,
    str_hint: str,
    masked_hint: str,
    refuse_masked: bool = True,
) -> None:
    """Refuse a data argument whose container type is not accepted, without converting it.

    A masked array is a value problem, not a type problem: with ``refuse_masked`` false it
    passes, and the conversion refuses it later.
    """
    # Types are tested with ``issubclass`` where only an error follows, so that ``values``
    # keeps its declared type for the messages.
    kind = type(values)
    if issubclass(kind, str | bytes | bytearray):
        raise _container_error(context, name, values, str_hint)
    if issubclass(kind, numbers.Number | np.generic):
        raise _container_error(context, name, values, _SINGLE_HINT)
    if refuse_masked and issubclass(kind, np.ma.MaskedArray):
        message = f"{name} is a masked array; masked values cannot be plotted."
        raise new_error(DataError, context, message, parameter=name, hint=masked_hint)
    if hasattr(values, "__array__") or isinstance(values, Sequence):
        return
    if issubclass(kind, Iterator):
        hint = _ITERATOR_HINT
    elif issubclass(kind, Set):
        hint = _SET_HINT
    elif issubclass(kind, Mapping):
        hint = _MAPPING_HINT
    else:
        hint = _OTHER_HINT
    raise _container_error(context, name, values, hint)


def check_floats_type(context: str, name: str, values: object) -> None:
    """Check the type of a numeric data argument without converting it.

    A masked array passes; the conversion refuses it.

    Args:
        context: The API call, such as ``"Axes.xticks_labeled()"``.
        name: The parameter name.
        values: The value passed.

    Raises:
        TypeError: The value is text, a single value, or a container without an order.

    Example:
        >>> from umber._values import check_floats_type
        >>> check_floats_type("Axes.xticks()", "positions", [1, 2])
    """
    _check_container(
        context, name, values, str_hint=_SINGLE_HINT, masked_hint=_MASKED_HINT, refuse_masked=False
    )


def check_names_type(context: str, name: str, values: object) -> None:
    """Check the type of a names argument without converting it.

    Args:
        context: The API call, such as ``"Axes.xticks_labeled()"``.
        name: The parameter name.
        values: The value passed.

    Raises:
        TypeError: The value is text, a single value, or a container without an order.

    Example:
        >>> from umber._values import check_names_type
        >>> check_names_type("Axes.xticks_labeled()", "labels", ["a", "b"])
    """
    _check_container(
        context,
        name,
        values,
        str_hint=_NAMES_HINT,
        masked_hint=_MASKED_NAMES_HINT,
        refuse_masked=False,
    )


def check_axis_type(context: str, name: str, values: object) -> None:
    """Check the type of x values, which are names or numbers, without converting them.

    Args:
        context: The API call, such as ``"Axes.line()"``.
        name: The parameter name.
        values: The value passed.

    Raises:
        TypeError: The value is text, a single value, or a container without an order.

    Example:
        >>> from umber._values import check_axis_type
        >>> check_axis_type("Axes.line()", "x", ["Q1", "Q2"])
    """
    _check_container(
        context, name, values, str_hint=_AXIS_HINT, masked_hint=_MASKED_HINT, refuse_masked=False
    )


def _container_error(context: str, name: str, values: object, hint: str) -> TypeError:
    message = f"{name} is {type_name(values)}; {name} must be a sequence or an array."
    return new_error(TypeError, context, message, parameter=name, hint=hint)


def _data_error(context: str, name: str, message: str, hint: str) -> DataError:
    return new_error(DataError, context, message, parameter=name, hint=hint)


def _check_one_dimensional(context: str, name: str, array: npt.NDArray[Any]) -> None:
    if array.ndim != 1:
        message = f"{name} has {array.ndim} dimensions; values must be one-dimensional."
        raise _data_error(context, name, message, _NDIM_HINT)


def _is_missing(value: object) -> bool:
    """Whether a value marks a missing entry: ``None``, NaN, or pandas ``NA`` or ``NaT``."""
    if value is None or (isinstance(value, float) and math.isnan(value)):
        return True
    kind = type(value)
    return kind.__module__.startswith("pandas") and kind.__name__ in ("NAType", "NaTType")


def _is_float64_or_narrower(dtype: np.dtype[Any]) -> bool:
    """Whether data of this dtype converts to ``float64`` exactly."""
    return dtype.kind in "iu" or (dtype.kind == "f" and dtype.itemsize <= 8)


def _is_number(value: object) -> bool:
    """Whether a value converts to ``float64`` without changing its meaning.

    Accepted are ints (not ``bool``), floats, and the NumPy integer and floating scalars and
    unmasked zero-dimensional arrays whose type is ``float64`` or narrower (not
    ``timedelta64``). Masked values never are.
    """
    if type(value) is float or type(value) is int:
        return True
    kind = type(value)
    if issubclass(kind, np.ma.MaskedArray | np.timedelta64 | bool):
        return False
    if issubclass(kind, np.generic | np.ndarray):
        array = cast("np.generic[Any] | npt.NDArray[Any]", value)
        return array.ndim == 0 and _is_float64_or_narrower(array.dtype)
    return issubclass(kind, int | float)


def _is_nested(value: object) -> bool:
    """Whether a value is a sequence or an array of at least one dimension (not masked)."""
    if isinstance(value, np.ndarray):
        return value.ndim > 0 and not isinstance(value, np.ma.MaskedArray)
    return isinstance(value, Sequence) and not isinstance(value, str | bytes | bytearray)


_CHUNK: Final = 256


def _elements(array: npt.NDArray[Any]) -> Iterator[object]:
    """Yield the elements of a one-dimensional array as Python objects, a few at a time.

    Converting in chunks keeps memory bounded and lets a scan stop at the first bad element.
    """
    for start in range(0, len(array), _CHUNK):
        yield from cast(list[object], array[start : start + _CHUNK].tolist())


def _first_non_number(items: Iterable[object]) -> tuple[int, object] | None:
    return next(((i, value) for i, value in enumerate(items) if not _is_number(value)), None)


def _element_error(context: str, name: str, index: int, value: object, *, mixed: bool) -> DataError:
    """The error for an element of numeric data that is not an int or a float."""
    value = _scalar(value)
    label = f"{name}[{index}]"
    shown = short_repr(value)
    if _is_masked(value):
        message, hint = f"{label} is masked; masked values cannot be plotted.", _MASKED_HINT
    elif mixed and isinstance(value, str):
        message, hint = f"{label} is {shown}; {name} mixes names and numbers.", _MIXED_HINT
    elif _is_missing(value):
        message, hint = f"{label} is {shown}; values must be numbers.", _MISSING_HINT
    elif isinstance(value, _BOOLS):
        message, hint = f"{label} is {shown}; values must be numbers.", _BOOL_HINT
    elif isinstance(value, str | bytes):
        message, hint = f"{label} is {shown}; values must be numbers.", _TEXT_HINT
    elif isinstance(value, _TIMES):
        message, hint = f"{label} is {shown}; values must be numbers.", _TIME_HINT
    elif isinstance(value, complex | np.complexfloating):
        message, hint = f"{label} is {shown}; values must be real numbers.", _COMPLEX_HINT
    elif isinstance(value, np.floating):
        message = f"{label} is {shown}; values must be float64 or narrower."
        hint = _WIDE_FLOAT_HINT
    elif _is_nested(value):
        message, hint = f"{label} is a sequence; values must be numbers.", _NESTED_HINT
    elif isinstance(value, numbers.Number):
        message, hint = f"{label} is {shown}; values must be int or float.", _NUMBER_HINT
    else:
        message, hint = f"{label} is {shown}; values must be numbers.", _NUMBER_HINT
    return _data_error(context, name, message, hint)


def _is_masked(value: object) -> bool:
    return issubclass(type(value), np.ma.MaskedArray)


def _scalar(value: object) -> object:
    """The scalar an unmasked zero-dimensional array holds, so that messages show it."""
    if issubclass(type(value), np.ndarray) and not _is_masked(value):
        array = cast(npt.NDArray[Any], value)
        if array.ndim == 0:
            return cast(object, array[()])
    return value


def _dtype_error(context: str, name: str, array: npt.NDArray[Any]) -> DataError:
    """The error for numeric data whose dtype is neither numeric nor text."""
    dtype = array.dtype
    kind = dtype.kind
    if kind == "b":
        required, hint = "values must be numbers", _BOOL_DTYPE_HINT
    elif kind in "Mm":
        required, hint = "values must be numbers", _TIME_HINT
    elif kind == "c":
        required, hint = "values must be real numbers", _COMPLEX_HINT
    elif kind == "f":
        required, hint = "values must be float64 or narrower", _WIDE_FLOAT_HINT
    elif kind == "V":
        required, hint = "values must be numbers", _STRUCTURED_HINT
    else:
        required, hint = "values must be numbers", _NUMBER_HINT
    return _data_error(context, name, f"{name} has dtype {dtype}; {required}.", hint)


def _to_float(value: object) -> float:
    """Convert a checked number; an int too large for a float becomes infinity."""
    number = cast(float, value)
    try:
        return float(number)
    except OverflowError:
        return float("inf") if number > 0 else float("-inf")


def _native(array: npt.NDArray[Any]) -> FloatArray:
    # ``np.require`` copies only data that is not aligned, native-endian float64. The view
    # drops an explicit byte order such as the "<d" of a ctypes buffer, without copying.
    return np.require(array, dtype="=f8", requirements=["A"]).view(np.float64)


def _sequence_array(
    context: str, name: str, items: Sequence[object], *, mixed: bool
) -> npt.NDArray[Any]:
    """Check each element of a sequence of numbers, then convert it with ``np.asarray``.

    Elements are checked before NumPy sees them, because NumPy would silently turn a masked
    element into NaN and a bool among numbers into 0 or 1. Nested sequences are left to
    ``np.asarray``, which gives them a shape; the shape check then reports them.
    """
    if isinstance(items, range):
        return np.asarray(items)
    nested: int | None = None
    for index, value in enumerate(items):
        if _is_number(value):
            continue
        if not _is_nested(value):
            # A nested element before this one makes the data invalid too; the first
            # offending element is reported, whatever is wrong with it.
            first = index if nested is None else nested
            raise _element_error(context, name, first, items[first], mixed=mixed)
        if nested is None:
            nested = index
    try:
        return np.asarray(items)
    except ValueError:
        # Raised for nested sequences of different lengths; report the first one.
        if nested is None:
            raise
        raise _element_error(context, name, nested, items[nested], mixed=mixed) from None


def _floats(context: str, name: str, data: _Data, *, mixed: bool) -> FloatArray:
    if isinstance(data, np.ndarray):
        array = data
    else:
        array = _sequence_array(context, name, data, mixed=mixed)
    _check_one_dimensional(context, name, array)
    if array.size == 0:
        return np.empty(0, dtype=np.float64)
    kind = array.dtype.kind
    if _is_float64_or_narrower(array.dtype):
        return _native(array)
    if kind == "O":
        found = _first_non_number(_elements(array))
        if found is not None:
            raise _element_error(context, name, found[0], found[1], mixed=mixed)
        try:
            return array.astype(np.float64)
        except OverflowError:
            converted = (_to_float(value) for value in _elements(array))
            return np.fromiter(converted, dtype=np.float64, count=len(array))
    if kind in "SUT":
        first = next(_elements(array[:1]))
        raise _element_error(context, name, 0, first, mixed=mixed)
    raise _dtype_error(context, name, array)


def _name_error(context: str, name: str, index: int, value: object, *, mixed: bool) -> DataError:
    """The error for an element of names that is not a ``str``."""
    label = f"{name}[{index}]"
    shown = short_repr(value)
    if _is_masked(value):
        message, hint = f"{label} is masked; masked values cannot be plotted.", _MASKED_NAMES_HINT
    elif _is_missing(value):
        message, hint = f"{label} is {shown}; categories must be strings.", _MISSING_NAME_HINT
    elif mixed and _is_number(value):
        message, hint = f"{label} is {shown}; {name} mixes names and numbers.", _MIXED_HINT
    else:
        message, hint = f"{label} is {shown}; categories must be strings.", _NAME_HINT
    return _data_error(context, name, message, hint)


def _names(context: str, name: str, data: _Data, *, mixed: bool) -> list[str]:
    if isinstance(data, np.ndarray):
        _check_one_dimensional(context, name, data)
        if data.size == 0:
            return []
        if data.dtype.kind not in "UTOS":
            message = f"{name} has dtype {data.dtype}; categories must be strings."
            raise _data_error(context, name, message, _NAME_DTYPE_HINT)
        items: Iterable[object] = _elements(data)
    else:
        items = data
    result: list[str] = []
    for index, value in enumerate(items):
        if not isinstance(value, str):
            raise _name_error(context, name, index, value, mixed=mixed)
        result.append(checked_text(context, name, f"{name}[{index}]", value, DataError))
    return result


def _starts_with_name(data: _Data) -> bool:
    if isinstance(data, np.ndarray):
        if data.ndim != 1 or data.size == 0:
            return False
        kind = data.dtype.kind
        return kind in "UT" or (kind == "O" and isinstance(data[0], str))
    return len(data) > 0 and isinstance(data[0], str)


def floats(context: str, name: str, values: object) -> FloatArray:
    """Convert numeric data, such as y values or bar heights, to a ``float64`` array.

    Lists, tuples, ranges and other sequences, NumPy arrays and pandas or polars Series are
    accepted. Integer and ``float16``/``float32`` data is converted to ``float64``; an
    aligned, native-endian ``float64`` array is returned without a copy.

    Args:
        context: The API call, such as ``"Axes.line()"``.
        name: The parameter name.
        values: The data passed.

    Returns:
        A one-dimensional ``float64`` array; empty data gives an empty array.

    Raises:
        TypeError: The data is text, a single value, or a container without an order, such
            as a set or a generator.
        DataError: The data is a masked array, is not one-dimensional, has a dtype that is
            not a number (bool, datetime, timedelta, complex, text), or holds an element that
            is not an int or a float, such as ``None``.

    Example:
        >>> import numpy as np
        >>> from umber._values import floats
        >>> floats("Axes.line()", "y", (1, 2.5))
        array([1. , 2.5])
        >>> try:
        ...     floats("Axes.line()", "y", [1.0, None, 3.0])
        ... except ValueError as err:
        ...     print(err)
        Axes.line(): y[1] is None; values must be numbers.
    """
    data = _data(context, name, values, str_hint=_SINGLE_HINT, masked_hint=_MASKED_HINT)
    return _floats(context, name, data, mixed=False)


def names(context: str, name: str, values: object) -> list[str]:
    """Convert names, such as bar categories, to a ``list`` of ``str``.

    Sequences of ``str`` (``str`` subclasses become ``str``), NumPy string or object arrays
    and pandas or polars Series of strings are accepted. Each name follows the text rules:
    at most ``MAX_TEXT_LENGTH`` characters, and no lone surrogates.

    Args:
        context: The API call, such as ``"Axes.bar()"``.
        name: The parameter name.
        values: The data passed.

    Returns:
        The names; empty data gives an empty list.

    Raises:
        TypeError: The data is text, a single value, or a container without an order.
        DataError: The data is a masked array, is not one-dimensional, has a non-text dtype,
            holds an element that is not a ``str``, or a name holds a lone surrogate.
        StyleError: A name is longer than ``MAX_TEXT_LENGTH`` characters.

    Example:
        >>> from umber._values import names
        >>> names("Axes.bar()", "categories", ("North", "South"))
        ['North', 'South']
    """
    data = _data(context, name, values, str_hint=_NAMES_HINT, masked_hint=_MASKED_NAMES_HINT)
    return _names(context, name, data, mixed=False)


def axis_values(context: str, name: str, values: object) -> FloatArray | list[str]:
    """Convert x values, which are names or numbers, to a ``list`` of ``str`` or an array.

    Data whose first element is a ``str`` is converted as names, anything else as numbers.
    Empty data gives an empty array; the caller decides whether a category axis needs an
    empty list instead.

    Args:
        context: The API call, such as ``"Axes.line()"``.
        name: The parameter name.
        values: The data passed.

    Returns:
        The names, or a one-dimensional ``float64`` array.

    Raises:
        TypeError: As for ``floats``.
        DataError: As for ``floats`` and ``names``; also when names and numbers are mixed.
        StyleError: A name is longer than ``MAX_TEXT_LENGTH`` characters.

    Example:
        >>> from umber._values import axis_values
        >>> axis_values("Axes.line()", "x", ["Q1", "Q2"])
        ['Q1', 'Q2']
        >>> axis_values("Axes.line()", "x", [2024, 2025])
        array([2024., 2025.])
    """
    data = _data(context, name, values, str_hint=_AXIS_HINT, masked_hint=_MASKED_HINT)
    if _starts_with_name(data):
        return _names(context, name, data, mixed=True)
    return _floats(context, name, data, mixed=True)
