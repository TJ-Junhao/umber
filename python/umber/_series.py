"""``Series``: named values for grouped bars, stacked bars and stacked areas."""

from collections.abc import Callable
from operator import attrgetter
from typing import Final, Self, cast, final

import numpy as np

from umber import _color
from umber._args import instance, require_text, text
from umber._color import Color
from umber._report import api_call
from umber._types import Values
from umber._values import FloatArray, check_floats_type, floats

_COLOR_HINT: Final = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'


@final
class Series:
    """An immutable series of values for grouped bars, stacked bars and stacked areas.

    Pass series to ``Axes.grouped_bar()``, ``Axes.stacked_bar()`` or ``Axes.stacked_area()``.
    Values are copied when the series is made, so changing the original data has no effect.
    Their type, shape and dtype are checked here; whether they are finite, and positive on a
    log axis, is checked by the chart the series is added to, which reports a bad value
    as ``series[i].values[j]``. A copy of a series is the series itself, and series pickle
    by value.

    Example:
        >>> import umber as ub
        >>> north = ub.Series("North", [3, 5, 2], color=ub.Color.BLUE)
        >>> north
        <umber.Series 'North' (3 values)>
    """

    __slots__ = ("_color", "_label", "_values")
    _label: str
    _values: FloatArray
    _color: Color

    def __init__(self, label: str, values: Values[float], *, color: Color = Color.AUTO) -> None:
        """Create a series of ``values`` named ``label``.

        Args:
            label: The name shown in the legend; an empty string leaves the series out of it.
            values: One value per category for bars, or per x value for stacked areas, in
                the same order. NaN skips a bar or breaks every layer of a stacked area.
            color: The series color. ``Color.AUTO`` takes the next color of the theme palette.

        Raises:
            TypeError: If an argument has the wrong type, such as ``values`` given as a
                ``str``.
            DataError: If ``values`` is not one-dimensional or not numeric, for example
                text, bool or datetime data or a ``None`` among the numbers.
            StyleError: If ``label`` is too long.
            AttributeError: If called again on an existing series, which is immutable.

        Example:
            >>> import numpy as np
            >>> import umber as ub
            >>> south = ub.Series("South", np.array([4.0, 1.5, 3.0]))
        """
        if _is_set(self):
            raise AttributeError("a Series is immutable")
        with api_call():
            context = "Series()"
            require_text(context, "label", label)
            check_floats_type(context, "values", values)
            shade = instance(context, "color", color, Color, _COLOR_HINT)
            name = text(context, "label", label)
            data = floats(context, "values", values)
            _fill(self, name, data, shade)

    def __repr__(self) -> str:
        count = len(self._values)
        return f"<umber.Series {self._label!r} ({count} value{'' if count == 1 else 's'})>"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(
        self,
    ) -> tuple[Callable[[str, FloatArray, Color], "Series"], tuple[str, FloatArray, Color]]:
        return _new, (self._label, self._values, self._color)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Series is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Series is immutable")


def _is_set(series: Series) -> bool:
    """Return whether ``series`` already holds its values."""
    try:
        object.__getattribute__(series, "_label")
    except AttributeError:
        return False
    return True


def _fill(series: Series, label: str, values: FloatArray, color: Color) -> None:
    """Store a read-only copy of ``values`` and the other fields in ``series``."""
    stored = np.array(values, dtype=np.float64, copy=True)
    stored.setflags(write=False)
    object.__setattr__(series, "_label", label)
    object.__setattr__(series, "_values", stored)
    object.__setattr__(series, "_color", color)


def _new(label: str, values: FloatArray, color: Color) -> Series:
    """Rebuild a pickled series from its values."""
    series = object.__new__(Series)
    _fill(series, label, values, color)
    return series


_NATIVE: Final = attrgetter("_label", "_values", "_color")


def native(series: Series) -> tuple[str, FloatArray, tuple[int, int, float]]:
    """Return the native series argument of ``series``: ``(label, values, color)``.

    Args:
        series: The series.

    Returns:
        The label, the read-only ``float64`` values and the native color argument.

    Example:
        >>> import umber as ub
        >>> from umber._series import native
        >>> native(ub.Series("North", [3, 5]))
        ('North', array([3., 5.]), (0, 0, 1.0))
    """
    label, values, color = cast(tuple[str, FloatArray, Color], _NATIVE(series))
    return label, values, _color.native(color)
