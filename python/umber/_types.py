"""Type aliases for the data of marks: ``SupportsArray``, ``Values`` and ``Time``."""

import datetime
from collections.abc import Sequence
from typing import Any, Protocol, TypeAlias

import numpy as np


class SupportsArray(Protocol):
    """An object NumPy can turn into an array, such as a NumPy array or a pandas or polars Series.

    Any object with an ``__array__`` method qualifies. Its dtype is checked when it is passed
    to a mark, so a Series of strings is accepted for names and refused for numbers.

    Example:
        >>> import numpy as np
        >>> import pandas as pd
        >>> from umber._types import SupportsArray
        >>> def first(values: SupportsArray) -> float:
        ...     return float(np.asarray(values)[0])
        >>> first(pd.Series([2.5, 4.0]))
        2.5
    """

    def __array__(self) -> Any:
        """Return the object's data as a NumPy array."""
        ...


type Values[T] = Sequence[T] | SupportsArray
"""Data accepted by every mark: a sequence such as a list, or an array-like object.

``Values[float]`` takes numbers: a list or tuple of ints and floats, a ``range``, a NumPy
array, or a pandas or polars Series. Integer and ``float16``/``float32`` data is converted
to ``float64``; integers are exact up to 2**53 in magnitude and rounded to the nearest
``float64`` beyond that. Data of dtype bool, datetime, timedelta or complex, masked arrays,
and text or ``None`` among the numbers raise ``DataError``; nothing else is converted. Use
NaN for missing values. Nullable pandas and polars Series need no conversion: they turn
``NA`` and null in numeric data into NaN themselves, and NaN marks a missing point. A
``pd.NA`` inside an object Series is not a number and raises ``DataError``.

``Values[str]`` takes names: a sequence of ``str``, or an array or Series of strings.

Example:
    >>> import numpy as np
    >>> import pandas as pd
    >>> import umber as ub
    >>> ax = ub.line([0, 1, 2], [1, 2.5, 4])
    >>> ax = ub.line(np.arange(3), np.array([1, 2, 3], dtype=np.int32))
    >>> ax = ub.line([0, 1, 2], pd.Series([0.5, float("nan"), 2.0]))
    >>> ax = ub.line([0, 1, 2], pd.Series([1, None, 3], dtype="Int64"))
    >>> ax = ub.bar(pd.Series(["North", "South"]), [3, 5])
"""


Time: TypeAlias = datetime.datetime | datetime.date | np.datetime64  # noqa: UP040
"""A time on a time axis: a ``datetime``, a ``date`` (midnight of that day) or a
``numpy.datetime64``. ``Values[Time]`` also takes NumPy ``datetime64`` arrays of any unit and
naive pandas and polars datetime columns. Times carry no time zone.

Example:
    >>> import datetime as dt
    >>> import umber as ub
    >>> ax = ub.Figure().add_axes(x=ub.Scale.time())
    >>> ax = ax.line([dt.date(2026, 1, 1), dt.datetime(2026, 1, 2, 12, 0)], [1.0, 2.0])
    >>> isinstance(ax, ub.Axes)
    True
"""
