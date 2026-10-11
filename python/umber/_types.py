"""Type aliases for the data arguments of every mark: ``SupportsArray`` and ``Values``."""

from collections.abc import Sequence
from typing import Any, Protocol


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
NaN for missing values.

``Values[str]`` takes names: a sequence of ``str``, or an array or Series of strings.

Example:
    >>> import numpy as np
    >>> import pandas as pd
    >>> from umber._values import floats
    >>> floats("Axes.line()", "y", [1, 2.5, 4])
    array([1. , 2.5, 4. ])
    >>> floats("Axes.line()", "y", np.array([1, 2, 3], dtype=np.int32))
    array([1., 2., 3.])
    >>> floats("Axes.line()", "y", pd.Series([0.5, float("nan"), 2.0]))
    array([0.5, nan, 2. ])
"""
