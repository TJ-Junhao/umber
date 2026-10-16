"""Conversion of times to the form the native module reads: int64 microseconds.

Times become a one-dimensional, aligned, native-endian ``int64`` NumPy array of microseconds
since 1970-01-01T00:00, with the smallest ``int64`` for a missing time (``NaT``), as NumPy
stores it. A time zone is refused before anything is converted. Only the type, the shape and
the kind of the values are checked here; the core library checks that every time lies between
0001-01-01 and 9999-12-31, so that rule has one home. Values finer than a microsecond are
rounded to the nearest microsecond, halves to even.
"""

import datetime
from collections.abc import Sequence
from typing import Any, Final, cast

import numpy as np
import numpy.typing as npt

from umber._args import is_text_or_buffer, short_repr, type_name
from umber._errors import DataError, new_error
from umber._values import check_one_dimensional, check_times_type, is_missing

type MicrosArray = npt.NDArray[np.int64]
"""A one-dimensional ``int64`` array of microseconds, in native byte order and aligned.

Example:
    >>> import datetime as dt
    >>> from umber._time import micros
    >>> micros("Axes.line()", "x", [dt.datetime(1970, 1, 1, 0, 0, 1)])
    array([1000000])
"""

MISSING: Final = np.iinfo(np.int64).min
_EPOCH: Final = datetime.datetime(1970, 1, 1)
_EPOCH_ORDINAL: Final = _EPOCH.toordinal()
_MICROS_PER_DAY: Final = 86_400_000_000

_TZ_HINT: Final = "convert first with .tz_localize(None) or .tz_convert(...)."
_VALUES_HINT: Final = "pass datetime, date or numpy.datetime64 values."
_TEXT_HINT: Final = "parse the text first, e.g. with datetime.fromisoformat()."
_SCALAR_HINT: Final = "pass a datetime, date or numpy.datetime64."
_MISSING_HINT: Final = "use numpy.datetime64('NaT') for a missing time."

# Microseconds per unit, for the NumPy units coarser than a microsecond whose length is fixed.
_COARSE: Final = {
    "W": 7 * _MICROS_PER_DAY,
    "D": _MICROS_PER_DAY,
    "h": 3_600_000_000,
    "m": 60_000_000,
    "s": 1_000_000,
    "ms": 1_000,
}
# Units per microsecond, for the NumPy units finer than a microsecond.
_FINE: Final = {"ns": 10**3, "ps": 10**6, "fs": 10**9, "as": 10**12}
# Years and months have no fixed length; within this many of 1970 they convert to microseconds
# without overflow (the int64 range holds about 292,000 years of microseconds).
_CALENDAR_REACH: Final = {"Y": 290_000, "M": 290_000 * 12}


def micros(context: str, name: str, values: object) -> MicrosArray:
    """Convert the times of a data argument to int64 microseconds since 1970-01-01T00:00.

    Lists and tuples of ``datetime``, ``date``, ``numpy.datetime64`` and pandas ``Timestamp``
    values, NumPy ``datetime64`` arrays of any unit and naive pandas and polars datetime
    columns are accepted. A ``date`` is midnight of that day; ``NaT`` becomes the smallest
    ``int64``, a gap.

    Args:
        context: The API call, such as ``"Axes.line()"``.
        name: The parameter name.
        values: The data passed.

    Returns:
        A one-dimensional ``int64`` array; empty data gives an empty array.

    Raises:
        TypeError: The data is text, a single value, or a container without an order.
        DataError: The data carries a time zone, is a masked array, is not one-dimensional,
            holds a value that is not a time (``None`` included), or a time too far from 1970
            to convert.

    Example:
        >>> import datetime as dt
        >>> import numpy as np
        >>> from umber._time import micros
        >>> micros("Axes.line()", "x", [dt.date(1970, 1, 2), np.datetime64("NaT", "s")]).tolist()
        [86400000000, -9223372036854775808]
    """
    check_times_type(context, name, values)
    zone = _zone_of_column(values)
    if zone is not None:
        raise _zone_error(context, name, zone, name)
    if isinstance(values, np.ma.MaskedArray):
        message = f"{name} is a masked array; masked values cannot be plotted."
        hint = "fill them with NaT: values.filled(np.datetime64('NaT'))."
        raise new_error(DataError, context, message, parameter=name, hint=hint)
    if hasattr(values, "__array__"):
        array = np.asarray(values)
        check_one_dimensional(context, name, array)
        if array.size == 0:
            return np.empty(0, dtype=np.int64)
        if array.dtype.kind == "M":
            return _datetime64_micros(context, name, array)
        if array.dtype.kind == "O":
            return _element_micros(context, name, cast(list[object], array.tolist()))
        message = f"{name} has dtype {array.dtype}; values must be times on a time axis."
        raise new_error(DataError, context, message, parameter=name, hint=_VALUES_HINT)
    items = cast(Sequence[object], values)
    nested = next((i for i, v in enumerate(items) if _is_sequence(v)), None)
    if nested is not None:
        message = f"{name}[{nested}] is a sequence; values must be times on a time axis."
        hint = "pass one time per point; nested sequences are not flattened."
        raise new_error(DataError, context, message, parameter=name, hint=hint)
    return _element_micros(context, name, list(items))


def instant(context: str, name: str, value: object) -> int:
    """Convert one time argument, such as a limit, to int64 microseconds since 1970-01-01T00:00.

    Args:
        context: The API call, such as ``"Axes.xlim()"``.
        name: The parameter name.
        value: The value passed: a ``datetime``, a ``date`` or a ``numpy.datetime64``.

    Returns:
        The microseconds.

    Raises:
        TypeError: The value is not a time.
        DataError: The value carries a time zone, is ``NaT``, or is too far from 1970 to
            convert.

    Example:
        >>> import datetime as dt
        >>> from umber._time import instant
        >>> instant("Axes.xlim()", "low", dt.date(1970, 1, 2))
        86400000000
    """
    nat = issubclass(type(value), np.datetime64) and bool(
        np.isnat(cast("np.datetime64[Any]", value))
    )
    if nat or (value is not None and is_missing(value)):
        message = f"{name} is NaT; it must be a time."
        hint = "pass a time; NaT marks a missing value."
        raise new_error(DataError, context, message, parameter=name, hint=hint)
    if not isinstance(value, datetime.date | np.datetime64):
        message = f"{name} is {type_name(value)}; {name} must be a time."
        raise new_error(TypeError, context, message, parameter=name, hint=_SCALAR_HINT)
    return _one(context, name, name, cast(object, value))


def _zone_of_column(values: object) -> str | None:
    """The time zone of a pandas or polars column, as given; ``None`` when it has none."""
    dtype: object = getattr(values, "dtype", None)
    if dtype is None or isinstance(dtype, np.dtype):
        return None
    for attribute in ("tz", "time_zone"):
        zone: object = getattr(dtype, attribute, None)
        if zone is not None:
            return str(zone)
    return None


def _zone_of_element(value: object) -> str | None:
    """The time zone of a ``datetime``, as given; ``None`` when it is naive."""
    if not isinstance(value, datetime.datetime) or value.tzinfo is None:
        return None
    if value.tzinfo.utcoffset(value) is None:
        return None
    return str(value.tzinfo)


def _zone_error(context: str, label: str, zone: str, parameter: str) -> DataError:
    message = f"{label} has time zone {zone}; time values must not carry a time zone."
    return new_error(DataError, context, message, parameter=parameter, hint=_TZ_HINT)


def _is_sequence(value: object) -> bool:
    if isinstance(value, np.ndarray):
        return value.ndim > 0
    return not is_text_or_buffer(value) and isinstance(value, Sequence)


def _element_micros(context: str, name: str, items: list[object]) -> MicrosArray:
    """Convert times one by one: those of a list, a tuple or an object array."""
    out = np.empty(len(items), dtype=np.int64)
    for index, value in enumerate(items):
        out[index] = _one(context, name, f"{name}[{index}]", value)
    return out


def _one(context: str, name: str, label: str, value: object) -> int:
    """The microseconds of one time, shown as ``label`` in messages; ``NaT`` is a gap.

    ``None`` and values that are not times are refused, as is a ``datetime`` with a time zone.
    """
    if issubclass(type(value), np.datetime64):
        stamp = cast("np.datetime64[Any]", value)
        return int(_datetime64_micros(context, name, np.array([stamp]), label)[0])
    if value is not None and not isinstance(value, float) and is_missing(value):
        # pandas NaT, which is also a datetime.
        return int(MISSING)
    if isinstance(value, datetime.datetime):
        zone = _zone_of_element(value)
        if zone is not None:
            raise _zone_error(context, label, zone, name)
        to_datetime64 = cast(Any, getattr(value, "to_datetime64", None))
        if to_datetime64 is not None:
            # A pandas Timestamp, which keeps its nanoseconds.
            stamp = cast(np.datetime64, to_datetime64())
            return int(_datetime64_micros(context, name, np.array([stamp]), label)[0])
        elapsed = value - _EPOCH
        return (elapsed.days * 86_400 + elapsed.seconds) * 1_000_000 + elapsed.microseconds
    if isinstance(value, datetime.date):
        return (value.toordinal() - _EPOCH_ORDINAL) * _MICROS_PER_DAY
    shown = short_repr(value)
    if value is None:
        hint = _MISSING_HINT
    elif isinstance(value, str):
        hint = _TEXT_HINT
    else:
        hint = _VALUES_HINT
    message = f"{label} is {shown}; values must be times on a time axis."
    raise new_error(DataError, context, message, parameter=name, hint=hint)


def _datetime64_micros(
    context: str, name: str, array: npt.NDArray[Any], label: str | None = None
) -> MicrosArray:
    """Convert a ``datetime64`` array of any unit to microseconds, rounding finer units.

    Messages name the element at ``i`` as ``name[i]``, or as ``label`` when one is given for
    an array of one element.
    """
    unit, count = np.datetime_data(array.dtype)
    if unit == "generic":
        # A unitless datetime64 array holds only NaT.
        return np.full(len(array), MISSING, dtype=np.int64)
    if count != 1:
        array = array.astype(f"datetime64[{unit}]")
    values = array.view(np.int64)
    missing = values == MISSING
    if unit == "us":
        out = values.astype(np.int64, copy=True)
    elif unit in _FINE:
        factor = _FINE[unit]
        quotient, remainder = np.divmod(values, factor)
        up = (2 * remainder > factor) | ((2 * remainder == factor) & (quotient % 2 == 1))
        out = quotient + up.astype(np.int64)
    else:
        reach = _CALENDAR_REACH.get(unit)
        limit = np.iinfo(np.int64).max // _COARSE[unit] if reach is None else reach
        beyond = np.flatnonzero(~missing & (np.abs(values) > limit))
        if beyond.size:
            at = int(beyond[0])
            raise _beyond_error(context, name, label or f"{name}[{at}]", array[at])
        out = array.astype("datetime64[us]").view(np.int64).copy()
    out[missing] = MISSING
    return np.require(out, dtype="=i8", requirements=["A", "C"])


def _beyond_error(context: str, name: str, label: str, value: np.datetime64) -> DataError:
    """The error for a time too far from 1970 to hold in int64 microseconds.

    Such a time is far outside the range the core library accepts, and it is reported in the
    core library's words, since it cannot reach the core library to be checked there.
    """
    shown = np.datetime_as_string(value, unit="D")
    message = f"{label} is {shown}; times must lie between 0001-01-01 and 9999-12-31."
    return new_error(DataError, context, message, parameter=name, hint=None)
