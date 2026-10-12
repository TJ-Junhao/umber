"""``Axes``: the plot area of a figure, with its marks, texts, limits and legend."""

from collections.abc import Sequence
from enum import Enum
from operator import attrgetter
from typing import Any, Final, Generic, Never, Self, cast, final

import numpy as np
from typing_extensions import TypeVar

# The package, for the forward references to ``umber.Figure``: ``_figure`` imports this module,
# so ``Figure`` cannot be imported from it here, but ``typing.get_type_hints`` resolves
# ``umber.Figure`` through the package once it has loaded.
import umber
from umber import _args, _color, _series, _size, _ticks, _umber
from umber._args import instance, integer, real, type_name
from umber._color import Color
from umber._enums import Dash, LegendStyle, Marker
from umber._errors import new_error
from umber._report import api_call
from umber._scale import Scale
from umber._series import BarSeries
from umber._size import Size
from umber._ticks import TickFormat
from umber._types import Values
from umber._values import (
    FloatArray,
    axis_values,
    check_axis_type,
    check_floats_type,
    check_names_type,
    floats,
    names,
)

# Old-style, invariant type variables with defaults: ``Figure().add_axes()`` is
# ``Axes[float, float]`` without annotations, and ``Axes[str, float]`` is not assignable to
# ``Axes[float, float]``.
X = TypeVar("X", default=float)
Y = TypeVar("Y", default=float)

_CATEGORY: Final = Scale.category()
_COLOR_HINT: Final = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
_COPY_HINT: Final = "copy the figure instead: copy.copy(ax.figure)."
_PICKLE_HINT: Final = "save the figure with save_png() or save_svg(), or build it again."
_SERIES_HINT: Final = 'pass a list of series, e.g. [BarSeries("North", [3, 5])].'
_SERIES_ITEM_HINT: Final = 'make each series with BarSeries(), e.g. BarSeries("North", [3, 5]).'


def _members_hint(cls: type[Enum]) -> str:
    """Return the hint that lists every member of ``cls``, such as ``use Dash.SOLID, …``."""
    names = [f"{cls.__name__}.{member.name}" for member in cls]
    return f"use {', '.join(names[:-1])} or {names[-1]}."


_DASH_HINT: Final = _members_hint(Dash)
_MARKER_HINT: Final = _members_hint(Marker)
_SIZE_HINT: Final = (
    "use Size.times(1.5) for a multiple of the theme's size, Size.pt(2.0) for points, or Size.AUTO."
)
_LEGEND_HINT: Final = _members_hint(LegendStyle)
_FORMAT_HINT: Final = (
    "use TickFormat.fixed(2), TickFormat.percent(), TickFormat.scientific() or TickFormat.AUTO."
)


@final
class Axes(Generic[X, Y]):  # noqa: UP046
    """The plot area of a figure: marks, titles, axis labels, limits and the legend.

    An axes is created by ``Figure.add_axes()`` and belongs to that figure. The type
    parameters are the types of the x and y values: ``Axes[float, float]`` for numeric axes,
    ``Axes[str, float]`` for a category x axis. Every method validates its arguments at the
    call and returns the axes, so calls chain. A failed call changes nothing.

    Example:
        >>> import umber as ub
        >>> ax = ub.Figure().add_axes()
        >>> ax.line([0, 1, 2], [1.0, 3.0, 2.0]).title("Signal") is ax
        True
    """

    # The native figure is the one the axes' figure owns; it holds every mark and text.
    __slots__ = ("_figure", "_native", "_x", "_y")
    _figure: "umber.Figure"
    _native: _umber.Figure
    _x: Scale[X]
    _y: Scale[float]

    def __init__(self) -> None:
        """Refuse construction; axes are added to a figure with ``Figure.add_axes()``."""
        with api_call():
            raise new_error(
                TypeError,
                "Axes()",
                "Axes has no public constructor; axes belong to a figure.",
                parameter=None,
                hint="call add_axes() on a Figure: ub.Figure().add_axes().",
            )

    @property
    def figure(self) -> "umber.Figure":
        """The figure these axes belong to; use it to render or save the chart.

        Example:
            >>> import umber as ub
            >>> fig = ub.Figure()
            >>> fig.add_axes().figure is fig
            True
        """
        return self._figure

    def title(self, text: str) -> Self:
        """Set the title, shown above the plot.

        Args:
            text: The title, at most 1,000 characters. An empty string removes it.

        Returns:
            These axes.

        Raises:
            TypeError: If ``text`` is not a ``str``.
            StyleError: If ``text`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().title("Revenue")
        """
        with api_call():
            self._native.title(_args.text("Axes.title()", "text", text))
            return self

    def subtitle(self, text: str) -> Self:
        """Set the subtitle, shown under the title.

        Args:
            text: The subtitle, at most 1,000 characters. An empty string removes it.

        Returns:
            These axes.

        Raises:
            TypeError: If ``text`` is not a ``str``.
            StyleError: If ``text`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().title("Revenue").subtitle("Per quarter")
        """
        with api_call():
            self._native.subtitle(_args.text("Axes.subtitle()", "text", text))
            return self

    def xlabel(self, text: str) -> Self:
        """Set the x axis label, shown under the x tick labels.

        Args:
            text: The label, at most 1,000 characters. An empty string removes it.

        Returns:
            These axes.

        Raises:
            TypeError: If ``text`` is not a ``str``.
            StyleError: If ``text`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().xlabel("Year")
        """
        with api_call():
            self._native.xlabel(_args.text("Axes.xlabel()", "text", text))
            return self

    def ylabel(self, text: str) -> Self:
        """Set the y axis label, shown left of the y tick labels.

        Args:
            text: The label, at most 1,000 characters. An empty string removes it.

        Returns:
            These axes.

        Raises:
            TypeError: If ``text`` is not a ``str``.
            StyleError: If ``text`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().ylabel("Revenue")
        """
        with api_call():
            self._native.ylabel(_args.text("Axes.ylabel()", "text", text))
            return self

    def xlim(self: "Axes[float, Y]", low: float, high: float) -> "Axes[float, Y]":
        """Set the x limits, replacing the automatic ones; for linear and log x axes.

        Args:
            low: The lower limit; finite, and positive on a log axis.
            high: The upper limit; finite and above ``low``.

        Returns:
            These axes.

        Raises:
            TypeError: If ``low`` or ``high`` is not a number.
            StyleError: If the limits are not finite, ``low`` is not below ``high``, or a
                limit is not positive on a log axis.
            UsageError: If the x axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 5, 10], [1, 4, 2]).xlim(0, 8)
        """
        with api_call():
            lower = real("Axes.xlim()", "low", low)
            upper = real("Axes.xlim()", "high", high)
            self._native.xlim(lower, upper)
            return self

    def ylim(self: "Axes[X, float]", low: float, high: float) -> "Axes[X, float]":
        """Set the y limits, replacing the automatic ones.

        Args:
            low: The lower limit; finite, and positive on a log axis.
            high: The upper limit; finite and above ``low``.

        Returns:
            These axes.

        Raises:
            TypeError: If ``low`` or ``high`` is not a number.
            StyleError: If the limits are not finite, ``low`` is not below ``high``, or a
                limit is not positive on a log axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1, 2], [1, 4, 2]).ylim(0, 5)
        """
        with api_call():
            lower = real("Axes.ylim()", "low", low)
            upper = real("Axes.ylim()", "high", high)
            self._native.ylim(lower, upper)
            return self

    def xticks(self: "Axes[float, Y]", positions: Values[float]) -> "Axes[float, Y]":
        """Set the x ticks to ``positions``, replacing the automatic ones.

        The positions are used in the order given. An empty list leaves the x axis without
        ticks and gridlines. Calling any x tick method again replaces the ticks and drops
        labels set with ``xticks_labeled()``. Without explicit limits, the x axis
        covers both the data and the ticks; with explicit limits, ticks outside them are not
        drawn.

        Args:
            positions: The tick positions: finite, within the supported magnitudes, positive
                on a log axis, and strictly increasing.

        Returns:
            These axes.

        Raises:
            TypeError: If ``positions`` is not a sequence or an array.
            DataError: If ``positions`` cannot be read as numbers.
            StyleError: If a position is not finite, out of range, not positive on a log
                axis, or not above the one before it.
            UsageError: If the x axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 12, 24], [1, 3, 2])
            >>> ax = ax.xticks([0, 12, 24])
        """
        with api_call():
            self._native.xticks(floats("Axes.xticks()", "positions", positions))
            return self

    def yticks(self: "Axes[X, float]", positions: Values[float]) -> "Axes[X, float]":
        """Set the y ticks to ``positions``, replacing the automatic ones.

        The positions are used in the order given. An empty list leaves the y axis without
        ticks and gridlines. Calling any y tick method again replaces the ticks and drops
        labels set with ``yticks_labeled()``. Without explicit limits, the y axis
        covers both the data and the ticks; with explicit limits, ticks outside them are not
        drawn.

        Args:
            positions: The tick positions: finite, within the supported magnitudes, positive
                on a log axis, and strictly increasing.

        Returns:
            These axes.

        Raises:
            TypeError: If ``positions`` is not a sequence or an array.
            DataError: If ``positions`` cannot be read as numbers.
            StyleError: If a position is not finite, out of range, not positive on a log
                axis, or not above the one before it.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 12, 24], [1, 3, 2])
            >>> ax = ax.yticks([0, 12, 24])
        """
        with api_call():
            self._native.yticks(floats("Axes.yticks()", "positions", positions))
            return self

    def xticks_labeled(
        self: "Axes[float, Y]", positions: Values[float], labels: Values[str]
    ) -> "Axes[float, Y]":
        """Set the x ticks to ``positions``, each with the label at the same index of ``labels``.

        The labels are drawn as given. The ticks follow the rules of ``Axes.xticks()``.
        Labels cannot be combined with a tick format other than ``TickFormat.AUTO`` on the same
        axis: whichever of ``xticks_labeled()`` and ``xtick_format()`` is called second
        raises ``UsageError``.

        Args:
            positions: The tick positions, as for ``Axes.xticks()``.
            labels: One label per position, each at most 1,000 characters.

        Returns:
            These axes.

        Raises:
            TypeError: If ``positions`` or ``labels`` has the wrong type.
            DataError: If ``positions`` cannot be read as numbers or ``labels`` as names.
            StyleError: If a position is not valid, the number of labels differs from the
                number of positions, or a label is too long.
            UsageError: If the x axis is a category axis, or the x tick format is not
                ``TickFormat.AUTO``.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1, 2], [1, 3, 2])
            >>> ax = ax.xticks_labeled([0, 1, 2], ["low", "mid", "high"])
        """
        with api_call():
            context = "Axes.xticks_labeled()"
            check_floats_type(context, "positions", positions)
            check_names_type(context, "labels", labels)
            places = floats(context, "positions", positions)
            texts = names(context, "labels", labels)
            self._native.xticks_labeled(places, texts)
            return self

    def yticks_labeled(
        self: "Axes[X, float]", positions: Values[float], labels: Values[str]
    ) -> "Axes[X, float]":
        """Set the y ticks to ``positions``, each with the label at the same index of ``labels``.

        The labels are drawn as given. The ticks follow the rules of ``Axes.yticks()``.
        Labels cannot be combined with a tick format other than ``TickFormat.AUTO`` on the same
        axis: whichever of ``yticks_labeled()`` and ``ytick_format()`` is called second
        raises ``UsageError``.

        Args:
            positions: The tick positions, as for ``Axes.yticks()``.
            labels: One label per position, each at most 1,000 characters.

        Returns:
            These axes.

        Raises:
            TypeError: If ``positions`` or ``labels`` has the wrong type.
            DataError: If ``positions`` cannot be read as numbers or ``labels`` as names.
            StyleError: If a position is not valid, the number of labels differs from the
                number of positions, or a label is too long.
            UsageError: If the y tick format is not
                ``TickFormat.AUTO``.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1, 2], [1, 3, 2])
            >>> ax = ax.yticks_labeled([0, 1, 2], ["low", "mid", "high"])
        """
        with api_call():
            context = "Axes.yticks_labeled()"
            check_floats_type(context, "positions", positions)
            check_names_type(context, "labels", labels)
            places = floats(context, "positions", positions)
            texts = names(context, "labels", labels)
            self._native.yticks_labeled(places, texts)
            return self

    def xtick_format(self: "Axes[float, Y]", format: TickFormat) -> "Axes[float, Y]":
        """Set the format of the x tick labels.

        The format applies to the automatic ticks and to positions set with ``Axes.xticks()``.
        Calling it again replaces the format; the ticks stay as they are.

        Args:
            format: How the labels are written. Any format other than ``TickFormat.AUTO``,
                including one with only a prefix or a suffix, conflicts with labels set by
                ``Axes.xticks_labeled()``.

        Returns:
            These axes.

        Raises:
            TypeError: If ``format`` is not a ``TickFormat``.
            UsageError: If the x axis is a category axis, or the x ticks have labels and
                ``format`` is not ``TickFormat.AUTO``.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1], [0.1, 0.4])
            >>> ax = ax.xtick_format(ub.TickFormat.percent())
        """
        with api_call():
            checked = instance("Axes.xtick_format()", "format", format, TickFormat, _FORMAT_HINT)
            self._native.xtick_format(_ticks.native(checked))
            return self

    def ytick_format(self: "Axes[X, float]", format: TickFormat) -> "Axes[X, float]":
        """Set the format of the y tick labels.

        The format applies to the automatic ticks and to positions set with ``Axes.yticks()``.
        Calling it again replaces the format; the ticks stay as they are.

        Args:
            format: How the labels are written. Any format other than ``TickFormat.AUTO``,
                including one with only a prefix or a suffix, conflicts with labels set by
                ``Axes.yticks_labeled()``.

        Returns:
            These axes.

        Raises:
            TypeError: If ``format`` is not a ``TickFormat``.
            UsageError: If the y ticks have labels and ``format`` is not
                ``TickFormat.AUTO``.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1], [0.1, 0.4])
            >>> ax = ax.ytick_format(ub.TickFormat.percent())
        """
        with api_call():
            checked = instance("Axes.ytick_format()", "format", format, TickFormat, _FORMAT_HINT)
            self._native.ytick_format(_ticks.native(checked))
            return self

    def legend(self, style: LegendStyle) -> Self:
        """Set the legend style, overriding the theme's.

        The legend lists the marks that have a non-empty ``label``; ``LegendStyle.NONE``
        hides it.

        Args:
            style: How the legend is laid out.

        Returns:
            These axes.

        Raises:
            TypeError: If ``style`` is not a ``LegendStyle``.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1], [1, 2], label="North")
            >>> ax = ax.legend(ub.LegendStyle.DIRECT)
        """
        with api_call():
            member = instance("Axes.legend()", "style", style, LegendStyle, _LEGEND_HINT)
            self._native.legend(member.value)
            return self

    def line(
        self,
        x: Values[X],
        y: Values[Y],
        *,
        label: str = "",
        color: Color = Color.AUTO,
        dash: Dash = Dash.SOLID,
        width: Size = Size.AUTO,
    ) -> Self:
        """Add a line through the points ``(x[i], y[i])``, in order.

        A NaN in ``x`` or ``y`` leaves a gap in the line.

        Args:
            x: The x values: numbers on a linear or log x axis, names on a category x axis.
            y: The y values, one per x value.
            label: The name shown in the legend; an empty string leaves the line out of it.
            color: The line color. ``Color.AUTO`` takes the next color of the theme palette.
            dash: How the line is stroked.
            width: The line width. ``Size.AUTO`` is the line width of the theme.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, or names and numbers do not match the x
                axis.
            StyleError: If ``label`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.line([0, 1, 2, 3], [1.0, 3.0, float("nan"), 2.0], label="North")
        """
        with api_call():
            context = "Axes.line()"
            check_axis_type(context, "x", x)
            check_floats_type(context, "y", y)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            style = instance(context, "dash", dash, Dash, _DASH_HINT)
            extent = _size_argument(context, "width", width)
            xs = self._x_values(context, x)
            ys = floats(context, "y", y)
            name = _args.text(context, "label", label)
            self._native.line(xs, ys, name, spec, style.value, extent)
            return self

    def area(
        self,
        x: Values[X],
        y: Values[Y],
        *,
        label: str = "",
        color: Color = Color.AUTO,
        dash: Dash = Dash.SOLID,
        width: Size = Size.AUTO,
    ) -> Self:
        """Add a line through the points ``(x[i], y[i])``, in order, and fill the region below it.

        The fill reaches down to the baseline of the axes. A NaN in ``x`` or ``y`` leaves a gap in
        the line and in the fill. The theme's fill style sets how the fill looks.

        Args:
            x: The x values: numbers on a linear or log x axis, names on a category x axis.
            y: The y values, one per x value.
            label: The name shown in the legend; an empty string leaves the area out of it.
            color: The area color. ``Color.AUTO`` takes the next color of the theme palette.
            dash: How the line is stroked.
            width: The line width. ``Size.AUTO`` is the line width of the theme.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, or names and numbers do not match the x
                axis.
            StyleError: If ``label`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.area([0, 1, 2, 3], [1.0, 3.0, 2.0, 4.0], label="Visitors")
        """
        with api_call():
            context = "Axes.area()"
            check_axis_type(context, "x", x)
            check_floats_type(context, "y", y)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            style = instance(context, "dash", dash, Dash, _DASH_HINT)
            extent = _size_argument(context, "width", width)
            xs = self._x_values(context, x)
            ys = floats(context, "y", y)
            name = _args.text(context, "label", label)
            self._native.area(xs, ys, name, spec, style.value, extent)
            return self

    def scatter(
        self,
        x: Values[X],
        y: Values[Y],
        *,
        label: str = "",
        color: Color = Color.AUTO,
        marker: Marker = Marker.CIRCLE,
        size: Size = Size.AUTO,
    ) -> Self:
        """Add a marker at each point ``(x[i], y[i])``.

        A point with a NaN coordinate is left out.

        Args:
            x: The x values: numbers on a linear or log x axis, names on a category x axis.
            y: The y values, one per x value.
            label: The name shown in the legend; an empty string leaves the series out of it.
            color: The marker color. ``Color.AUTO`` takes the next color of the theme palette.
            marker: The marker shape.
            size: The marker size. ``Size.AUTO`` is the marker size of the theme.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, or names and numbers do not match the x
                axis.
            StyleError: If ``label`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.scatter([1, 2, 3], [2.0, 1.0, 3.0], marker=ub.Marker.DIAMOND)
        """
        with api_call():
            context = "Axes.scatter()"
            check_axis_type(context, "x", x)
            check_floats_type(context, "y", y)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            shape = instance(context, "marker", marker, Marker, _MARKER_HINT)
            extent = _size_argument(context, "size", size)
            xs = self._x_values(context, x)
            ys = floats(context, "y", y)
            name = _args.text(context, "label", label)
            self._native.scatter(xs, ys, name, spec, shape.value, extent)
            return self

    def bar(
        self: "Axes[str, float]",
        categories: Values[str],
        heights: Values[float],
        *,
        label: str = "",
        color: Color = Color.AUTO,
    ) -> "Axes[str, float]":
        """Add one bar per category, from the baseline to its height; for a category x axis.

        A negative height draws below the baseline. A NaN height leaves its category without
        a bar.

        Args:
            categories: The category names, each at most once.
            heights: The bar heights, one per category.
            label: The name shown in the legend; an empty string leaves the bars out of it.
            color: The bar color. ``Color.AUTO`` takes the next color of the theme palette.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the data cannot be plotted: lengths differ, a category repeats, a
                height is infinite or not positive on a log axis, or the x axis is not a
                category axis.
            StyleError: If ``label`` or a category name is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.bar(["North", "South", "East"], [3, 5, 2], label="2025")
        """
        with api_call():
            context = "Axes.bar()"
            check_names_type(context, "categories", categories)
            check_floats_type(context, "heights", heights)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            category_names = names(context, "categories", categories)
            values = floats(context, "heights", heights)
            name = _args.text(context, "label", label)
            self._native.bar(category_names, values, name, spec)
            return self

    def grouped_bar(
        self: "Axes[str, float]", categories: Values[str], series: Sequence[BarSeries]
    ) -> "Axes[str, float]":
        """Add grouped bars: in each category, one bar per series, side by side.

        Args:
            categories: The category names, each at most once.
            series: One or more ``BarSeries``, each with one height per category. The series
                take their places in the group in this order.

        Returns:
            These axes.

        Raises:
            TypeError: If ``series`` is not a sequence of ``BarSeries``, or ``categories`` has
                the wrong type.
            DataError: If the data cannot be plotted: ``series`` is empty, a series has a
                different length, a category repeats, a height is infinite or not positive on
                a log axis, or the x axis is not a category axis.
            StyleError: If a category name is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.grouped_bar(
            ...     ["Q1", "Q2", "Q3"],
            ...     [ub.BarSeries("North", [3, 5, 2]), ub.BarSeries("South", [4, 1, 3])],
            ... )
        """
        with api_call():
            context = "Axes.grouped_bar()"
            check_names_type(context, "categories", categories)
            arguments = _series_arguments(context, series)
            category_names = names(context, "categories", categories)
            self._native.grouped_bar(category_names, arguments)
            return self

    def stacked_bar(
        self: "Axes[str, float]", categories: Values[str], series: Sequence[BarSeries]
    ) -> "Axes[str, float]":
        """Add stacked bars: in each category, the series stacked on one another.

        Positive and negative heights stack separately, above and below the baseline.

        Args:
            categories: The category names, each at most once.
            series: One or more ``BarSeries``, each with one height per category. The first
                series sits on the baseline.

        Returns:
            These axes.

        Raises:
            TypeError: If ``series`` is not a sequence of ``BarSeries``, or ``categories`` has
                the wrong type.
            DataError: If the data cannot be plotted: ``series`` is empty, a series has a
                different length, a category repeats, a height is infinite or not positive on
                a log axis, a stack exceeds the supported magnitude, or the x axis is not a
                category axis.
            StyleError: If a category name is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.stacked_bar(
            ...     ["Q1", "Q2", "Q3"],
            ...     [ub.BarSeries("North", [3, 5, 2]), ub.BarSeries("South", [4, 1, 3])],
            ... )
        """
        with api_call():
            context = "Axes.stacked_bar()"
            check_names_type(context, "categories", categories)
            arguments = _series_arguments(context, series)
            category_names = names(context, "categories", categories)
            self._native.stacked_bar(category_names, arguments)
            return self

    def hist(
        self: "Axes[float, float]",
        values: Values[float],
        *,
        bins: int = 30,
        label: str = "",
        color: Color = Color.AUTO,
    ) -> "Axes[float, float]":
        """Add a histogram: the count of ``values`` in equal-width bins over their range.

        NaN values are left out of the counts. The x axis must be linear.

        Args:
            values: The values to count.
            bins: The number of bins, from 1 to ``MAX_BINS``.
            label: The name shown in the legend; an empty string leaves the histogram out of
                it.
            color: The bar color. ``Color.AUTO`` takes the next color of the theme palette.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If a value is infinite, or the x axis is a category axis.
            StyleError: If ``bins`` is out of range, or ``label`` is too long.
            UsageError: If the x axis is a log axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.hist([1.2, 2.5, 2.7, 3.1, 4.8, 5.0], bins=4, label="Sizes")
        """
        with api_call():
            context = "Axes.hist()"
            check_floats_type(context, "values", values)
            count = integer(context, "bins", bins)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            data = floats(context, "values", values)
            name = _args.text(context, "label", label)
            self._native.hist(data, count, name, spec)
            return self

    def _x_values(self, context: str, x: object) -> FloatArray | list[str]:
        """Convert x values; empty data is an empty list of names on a category x axis."""
        values = axis_values(context, "x", x)
        if isinstance(values, np.ndarray) and values.size == 0 and self._x == _CATEGORY:
            return []
        return values

    def __copy__(self) -> Never:
        with api_call():
            raise new_error(
                TypeError,
                "Axes",
                "an Axes cannot be copied on its own; it belongs to its figure.",
                parameter=None,
                hint=_COPY_HINT,
            )

    def __deepcopy__(self, memo: dict[int, object]) -> Never:
        with api_call():
            raise new_error(
                TypeError,
                "Axes",
                "an Axes cannot be copied on its own; it belongs to its figure.",
                parameter=None,
                hint=_COPY_HINT,
            )

    def __reduce__(self) -> Never:
        with api_call():
            raise new_error(
                TypeError,
                "Axes",
                "an Axes cannot be pickled.",
                parameter=None,
                hint=_PICKLE_HINT,
            )


def _color_argument(context: str, value: object) -> tuple[int, int, float]:
    """Check a ``color`` argument and return its native form."""
    return _color.native(instance(context, "color", value, Color, _COLOR_HINT))


def _size_argument(context: str, name: str, value: object) -> tuple[int, float]:
    """Check a ``width`` or ``size`` argument and return its native form."""
    return _size.native(instance(context, name, value, Size, _SIZE_HINT))


def _series_arguments(
    context: str, value: object
) -> list[tuple[str, FloatArray, tuple[int, int, float]]]:
    """Check a ``series`` argument and return the native form of each series."""
    if isinstance(value, str | bytes) or not isinstance(value, Sequence):
        message = f"series is {type_name(value)}; series must be a sequence of BarSeries."
        raise new_error(TypeError, context, message, parameter="series", hint=_SERIES_HINT)
    items = cast(Sequence[object], value)
    arguments: list[tuple[str, FloatArray, tuple[int, int, float]]] = []
    for index, item in enumerate(items):
        if not isinstance(item, BarSeries):
            label = f"series[{index}]"
            message = f"{label} is {type_name(item)}; {label} must be of type BarSeries."
            raise new_error(TypeError, context, message, parameter="series", hint=_SERIES_ITEM_HINT)
        arguments.append(_series.native(item))
    return arguments


_SCALES: Final = attrgetter("_x", "_y")


def attach(
    figure: "umber.Figure", native: _umber.Figure, x: Scale[X], y: Scale[float]
) -> Axes[X, float]:
    """Return new axes of ``figure`` with the scales ``x`` and ``y``.

    The native figure must already hold axes with these scales; this builds the Python side
    only, because ``Axes`` has no public constructor.

    Args:
        figure: The figure the axes belong to.
        native: The native figure ``figure`` owns.
        x: The x scale.
        y: The y scale; linear or log.

    Returns:
        The axes.

    Example:
        >>> import umber as ub
        >>> fig = ub.Figure()
        >>> ax = fig.add_axes()  # attaches the axes once the native figure holds them
        >>> ax.figure is fig
        True
    """
    axes = cast(Axes[X, float], object.__new__(Axes))
    object.__setattr__(axes, "_figure", figure)
    object.__setattr__(axes, "_native", native)
    object.__setattr__(axes, "_x", x)
    object.__setattr__(axes, "_y", y)
    return axes


def scales(axes: Axes[Any, Any]) -> tuple[Scale[Any], Scale[float]]:
    """Return the x and y scales of ``axes``.

    Args:
        axes: The axes.

    Returns:
        The x scale and the y scale.

    Example:
        >>> import umber as ub
        >>> from umber._axes import scales
        >>> scales(ub.Figure().add_axes(x=ub.Scale.category()))
        (Scale.category(), Scale.linear())
    """
    return cast(tuple[Scale[Any], Scale[float]], _SCALES(axes))
