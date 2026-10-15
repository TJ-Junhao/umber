"""Immutable ``Series`` for bars and stacked areas, and ``Distribution`` for grouped boxes."""

from collections.abc import Callable, Sequence
from operator import attrgetter
from typing import Final, Self, cast, final

import numpy as np

from umber import _color
from umber._args import instance, is_text_or_buffer, require_text, text, type_name
from umber._color import Color
from umber._errors import UmberError, details, new_error
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


@final
class Distribution:
    """An immutable named distribution, with one group of samples per category.

    Pass distributions to ``Axes.grouped_box()``. Samples are copied when the distribution
    is made; changing the original data has no effect. Type, shape and dtype are checked
    here; finiteness and log-axis constraints are checked when adding the mark. NaN is
    ignored. Copies return the same immutable distribution; pickle stores it by value.

    Example:
        >>> import umber as ub
        >>> ub.Distribution("North", [[1, 2, 3], [2, 4]])
        <umber.Distribution 'North' (2 groups)>
    """

    __slots__ = ("_color", "_groups", "_label")
    _label: str
    _groups: tuple[FloatArray, ...]
    _color: Color

    def __init__(
        self, label: str, groups: Sequence[Values[float]], *, color: Color = Color.AUTO
    ) -> None:
        """Create a distribution with one group of samples per category.

        Args:
            label: The legend label; an empty string omits the legend entry.
            groups: One-dimensional numeric samples for each category, in category order.
                Empty and all-NaN groups draw nothing.
            color: The distribution color. ``Color.AUTO`` uses the theme palette.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If a group is not one-dimensional or not numeric.
            StyleError: If the label is too long.
            AttributeError: If called again on an existing immutable distribution.

        Example:
            >>> import umber as ub
            >>> d = ub.Distribution("North", [[1, 2], []], color=ub.Color.BLUE)
        """
        if hasattr(self, "_label"):
            raise AttributeError("a Distribution is immutable")
        with api_call():
            context = "Distribution()"
            require_text(context, "label", label)
            check_groups_type(context, groups)
            shade = instance(context, "color", color, Color, _COLOR_HINT)
            name = text(context, "label", label)
            data = group_values(context, groups)
            _fill_distribution(self, name, data, shade)

    def __repr__(self) -> str:
        count = len(self._groups)
        return f"<umber.Distribution {self._label!r} ({count} group{'' if count == 1 else 's'})>"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(
        self,
    ) -> tuple[
        Callable[[str, tuple[FloatArray, ...], Color], "Distribution"],
        tuple[str, tuple[FloatArray, ...], Color],
    ]:
        return _new_distribution, (self._label, self._groups, self._color)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Distribution is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Distribution is immutable")


def _fill_distribution(
    distribution: Distribution, label: str, groups: Sequence[FloatArray], color: Color
) -> None:
    """Store immutable copies of the samples and the other fields."""
    stored = tuple(np.array(g, dtype=np.float64, copy=True) for g in groups)
    for group in stored:
        group.setflags(write=False)
    object.__setattr__(distribution, "_label", label)
    object.__setattr__(distribution, "_groups", stored)
    object.__setattr__(distribution, "_color", color)


def _new_distribution(label: str, groups: tuple[FloatArray, ...], color: Color) -> Distribution:
    """Rebuild a pickled distribution from its samples."""
    distribution = object.__new__(Distribution)
    _fill_distribution(distribution, label, groups, color)
    return distribution


def _group_error(context: str, error: Exception) -> Exception:
    """Keep the indexed message but highlight the enclosing groups argument."""
    info = details(error)
    if info is None:
        return error
    return new_error(
        type(error),
        context,
        str(error).removeprefix(f"{context}: "),
        parameter="groups",
        hint=info[1],
    )


def check_groups_type(context: str, value: object) -> None:
    """Check the groups container and every group's type without conversion.

    Args:
        context: The API call receiving the groups.
        value: The groups argument.

    Raises:
        TypeError: If the container or any group has the wrong type.

    Example:
        >>> from umber._series import check_groups_type
        >>> check_groups_type("Axes.box()", [[1, 2], []])
    """
    if is_text_or_buffer(value) or not isinstance(value, Sequence):
        raise new_error(
            TypeError,
            context,
            f"groups is {type_name(value)}; groups must be a sequence of numeric groups.",
            parameter="groups",
            hint="pass a list of groups, e.g. [[1, 2], [3, 4]].",
        )
    for index, group in enumerate(cast(Sequence[object], value)):
        try:
            check_floats_type(context, f"groups[{index}]", group)
        except (TypeError, UmberError) as error:
            raise _group_error(context, error) from None


def group_values(context: str, value: Sequence[object]) -> list[FloatArray]:
    """Convert each group, preserving indexed messages and the groups highlight.

    Args:
        context: The API call receiving the groups.
        value: Groups whose container types have been checked.

    Returns:
        One native float array per group.

    Raises:
        DataError: If a group has an unsupported shape or dtype.

    Example:
        >>> from umber._series import group_values
        >>> group_values("Axes.box()", [[1, 2]])
        [array([1., 2.])]
    """
    result: list[FloatArray] = []
    for index, group in enumerate(value):
        try:
            result.append(floats(context, f"groups[{index}]", group))
        except (TypeError, UmberError) as error:
            raise _group_error(context, error) from None
    return result


_DISTRIBUTION_NATIVE: Final = attrgetter("_label", "_groups", "_color")


def native_distribution(
    distribution: Distribution,
) -> tuple[str, list[FloatArray], tuple[int, int, float]]:
    """Return the native label, groups and color of a distribution.

    Args:
        distribution: The immutable distribution.

    Returns:
        The label, read-only sample arrays, and native color argument.

    Example:
        >>> import umber as ub
        >>> from umber._series import native_distribution
        >>> native_distribution(ub.Distribution("North", [[1, 2]]))
        ('North', [array([1., 2.])], (0, 0, 1.0))
    """
    label, groups, color = cast(
        tuple[str, tuple[FloatArray, ...], Color], _DISTRIBUTION_NATIVE(distribution)
    )
    return label, list(groups), _color.native(color)
