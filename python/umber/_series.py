"""``BarSeries``: one named series of heights for grouped and stacked bar charts."""

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
class BarSeries:
    """An immutable series of bar heights, one per category, for grouped and stacked bars.

    Pass a list of series to ``Axes.grouped_bar()`` or ``Axes.stacked_bar()``. The heights
    are copied when the series is made, so changing the original data later has no effect.
    Their type, shape and dtype are checked here; whether they are finite, and positive on a
    log axis, is checked by the bar chart the series is added to, which reports a bad height
    as ``series[i].heights[j]``. A copy of a series is the series itself, and series pickle
    by value.

    Example:
        >>> import umber as ub
        >>> north = ub.BarSeries("North", [3, 5, 2], color=ub.Color.BLUE)
        >>> north
        <umber.BarSeries 'North' (3 heights)>
    """

    __slots__ = ("_color", "_heights", "_label")
    _label: str
    _heights: FloatArray
    _color: Color

    def __init__(self, label: str, heights: Values[float], *, color: Color = Color.AUTO) -> None:
        """Create a series of ``heights`` named ``label``.

        Args:
            label: The name shown in the legend; an empty string leaves the series out of it.
            heights: The bar heights, one per category, in category order. A NaN leaves that
                category without a bar for this series.
            color: The bar color. ``Color.AUTO`` takes the next color of the theme palette.

        Raises:
            TypeError: If an argument has the wrong type, such as ``heights`` given as a
                ``str``.
            DataError: If ``heights`` is not one-dimensional or not numeric, for example
                text, bool or datetime data or a ``None`` among the numbers.
            StyleError: If ``label`` is too long.
            AttributeError: If called again on an existing series, which is immutable.

        Example:
            >>> import numpy as np
            >>> import umber as ub
            >>> south = ub.BarSeries("South", np.array([4.0, 1.5, 3.0]))
        """
        if _is_set(self):
            raise AttributeError("a BarSeries is immutable")
        with api_call():
            context = "BarSeries()"
            require_text(context, "label", label)
            check_floats_type(context, "heights", heights)
            shade = instance(context, "color", color, Color, _COLOR_HINT)
            name = text(context, "label", label)
            values = floats(context, "heights", heights)
            _fill(self, name, values, shade)

    def __repr__(self) -> str:
        count = len(self._heights)
        return f"<umber.BarSeries {self._label!r} ({count} height{'' if count == 1 else 's'})>"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(
        self,
    ) -> tuple[Callable[[str, FloatArray, Color], "BarSeries"], tuple[str, FloatArray, Color]]:
        return _new, (self._label, self._heights, self._color)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a BarSeries is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a BarSeries is immutable")


def _is_set(series: BarSeries) -> bool:
    """Return whether ``series`` already holds its values."""
    try:
        object.__getattribute__(series, "_label")
    except AttributeError:
        return False
    return True


def _fill(series: BarSeries, label: str, heights: FloatArray, color: Color) -> None:
    """Store a read-only copy of ``heights`` and the other values in ``series``."""
    stored = np.array(heights, dtype=np.float64, copy=True)
    stored.setflags(write=False)
    object.__setattr__(series, "_label", label)
    object.__setattr__(series, "_heights", stored)
    object.__setattr__(series, "_color", color)


def _new(label: str, heights: FloatArray, color: Color) -> BarSeries:
    """Rebuild a pickled series from its values."""
    series = object.__new__(BarSeries)
    _fill(series, label, heights, color)
    return series


_NATIVE: Final = attrgetter("_label", "_heights", "_color")


def native(series: BarSeries) -> tuple[str, FloatArray, tuple[int, int, float]]:
    """Return the native series argument of ``series``: ``(label, heights, color)``.

    Args:
        series: The series.

    Returns:
        The label, the read-only ``float64`` heights and the native color argument.

    Example:
        >>> import umber as ub
        >>> from umber._series import native
        >>> native(ub.BarSeries("North", [3, 5]))
        ('North', array([3., 5.]), (0, 0, 1.0))
    """
    label, heights, color = cast(tuple[str, FloatArray, Color], _NATIVE(series))
    return label, heights, _color.native(color)
