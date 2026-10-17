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
from umber import _args, _cmap, _color, _scale, _series, _size, _ticks, _time, _umber
from umber._args import instance, integer, is_text_or_buffer, real, type_name
from umber._cmap import CMAP_HINT, Cmap
from umber._color import Color
from umber._color_by import ColorBy, native_color_scale, native_point_color
from umber._enums import (
    AUTO,
    Auto,
    ColorbarPosition,
    Dash,
    LegendPosition,
    LegendStyle,
    Marker,
    Step,
)
from umber._errors import DataError, StyleError, UsageError, new_error
from umber._report import api_call
from umber._scale import Scale
from umber._series import Distribution, Series
from umber._size import Size
from umber._ticks import TickFormat
from umber._time import MicrosArray
from umber._types import Time, Values
from umber._values import (
    FloatArray,
    axis_values,
    check_axis_type,
    check_floats_type,
    check_names_type,
    check_times_type,
    floats,
    matrix,
    names,
)

# Old-style, invariant type variables with defaults: ``Figure().add_axes()`` is
# ``Axes[float, float]`` without annotations, and ``Axes[str, float]`` is not assignable to
# ``Axes[float, float]``.
X = TypeVar("X", default=float)
Y = TypeVar("Y", default=float)
# Axes whose x axis, or whose y axis, is linear, log or time: the self types of the methods that
# take numbers or times along that axis, such as limits, ticks and spans, and refuse a category
# axis there. A bound on the receiver is checked by both pyright and mypy; the values the method
# takes follow the axis through ``X`` and ``Y``.
SX = TypeVar("SX", bound="Axes[float, Any] | Axes[Time, Any]")
SY = TypeVar("SY", bound="Axes[Any, float] | Axes[Any, Time]")
# Axes with a linear, log or time x axis beside a linear or log y axis: bands and stacked areas.
SXF = TypeVar("SXF", bound="Axes[float, float] | Axes[Time, float]")
# Axes whose x and y axes are each linear, log or time: bubbles.
SXY = TypeVar(
    "SXY",
    bound="Axes[float, float] | Axes[Time, float] | Axes[float, Time] | Axes[Time, Time]",
)

_CATEGORY: Final = Scale.category()
_TIME: Final = Scale.time()
_COLOR_HINT: Final = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
_COPY_HINT: Final = "copy the figure instead: copy.copy(ax.figure)."
_PICKLE_HINT: Final = "save the figure with save_png() or save_svg(), or build it again."
_SERIES_HINT: Final = 'pass a list of series, e.g. [Series("North", [3, 5])].'
_SERIES_ITEM_HINT: Final = 'make each series with Series(), e.g. Series("North", [3, 5]).'


def _members_hint(cls: type[Enum]) -> str:
    """Return the hint that lists every member of ``cls``, such as ``use Dash.SOLID, …``."""
    names = [f"{cls.__name__}.{member.name}" for member in cls]
    return f"use {', '.join(names[:-1])} or {names[-1]}."


_DASH_HINT: Final = _members_hint(Dash)
_MARKER_HINT: Final = _members_hint(Marker)
_STEP_HINT: Final = _members_hint(Step)
_SIZE_HINT: Final = (
    "use Size.times(1.5) for a multiple of the theme's size, Size.pt(2.0) for points, or Size.AUTO."
)
_LEGEND_HINT: Final = _members_hint(LegendStyle)
_LEGEND_POSITION_HINT: Final = (
    _members_hint(LegendPosition) + " Use ub.AUTO for the theme's position."
)
_FORMAT_HINT: Final = (
    "use TickFormat.fixed(2), TickFormat.percent(), TickFormat.scientific() or TickFormat.AUTO."
)


@final
class Axes(Generic[X, Y]):  # noqa: UP046
    """The plot area of a figure: marks, titles, axis labels, limits and the legend.

    An axes is created by ``Figure.add_axes()`` or ``Cell.add_axes()`` and belongs to that
    figure. Retrieve it with ``Figure.axes_at()`` using the same position and scales.
    The type parameters are the types of the x and y values: ``Axes[float, float]`` for numeric
    axes, ``Axes[str, float]`` for a category x axis, ``Axes[float, str]`` for a category y
    axis, on which bars, boxes and violins are horizontal and a scatter is a dot plot,
    ``Axes[str, str]`` for two category axes, which take a ``heatmap()``, and
    ``Axes[Time, float]`` for a time x axis. Every method validates its arguments at the call
    and returns the axes, so calls chain. A failed call changes nothing.

    Example:
        >>> import umber as ub
        >>> ax = ub.Figure().add_axes()
        >>> ax.line([0, 1, 2], [1.0, 3.0, 2.0]).title("Signal") is ax
        True
    """

    # The native figure is the one the axes' figure owns; it holds every mark and text. The
    # axes are found there by the row and column of their top-left cell.
    __slots__ = ("_col", "_figure", "_native", "_row", "_x", "_y")
    _figure: "umber.Figure"
    _native: _umber.Figure
    _row: int
    _col: int
    _x: Scale[X]
    _y: Scale[Y]

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
            self._native.title(self._row, self._col, _args.text("Axes.title()", "text", text))
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
            self._native.subtitle(self._row, self._col, _args.text("Axes.subtitle()", "text", text))
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
            self._native.xlabel(self._row, self._col, _args.text("Axes.xlabel()", "text", text))
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
            self._native.ylabel(self._row, self._col, _args.text("Axes.ylabel()", "text", text))
            return self

    def xlim(self: SX, low: X, high: X) -> SX:
        """Set the x limits, replacing the automatic ones; for linear, log and time x axes.

        Automatic linear ticks are whole multiples of their step. On a value axis with
        nonnegative data, automatic linear limits and padding stop at zero. A positions axis
        keeps its padding below zero; explicit limits are used as given. Tick density follows
        screen direction, not the orientation of the marks.

        Args:
            low: The lower limit; finite, and positive on a log axis; a time on a time axis.
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
            context = "Axes.xlim()"
            if self._x == _TIME:
                start = _time.instant(context, "low", low)
                end = _time.instant(context, "high", high)
                self._native.xlim_time(self._row, self._col, start, end)
                return self
            lower = real(context, "low", low)
            upper = real(context, "high", high)
            self._native.xlim(self._row, self._col, lower, upper)
            return self

    def ylim(self: SY, low: Y, high: Y) -> SY:
        """Set the y limits, replacing the automatic ones.

        Automatic linear ticks are whole multiples of their step. On a value axis with
        nonnegative data, automatic linear limits stop at zero, including padding and the
        extra step below the data. A positions axis keeps its padding below zero; explicit
        limits are used as given.

        Args:
            low: The lower limit; finite, and positive on a log axis; a time on a time axis.
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
            context = "Axes.ylim()"
            if self._y == _TIME:
                start = _time.instant(context, "low", low)
                end = _time.instant(context, "high", high)
                self._native.ylim_time(self._row, self._col, start, end)
                return self
            lower = real(context, "low", low)
            upper = real(context, "high", high)
            self._native.ylim(self._row, self._col, lower, upper)
            return self

    def xticks(self: SX, positions: Values[X]) -> SX:
        """Set the x ticks to ``positions``, replacing the automatic ones.

        Automatic linear ticks are whole multiples of their step, with a tick at zero when
        the domain includes it. Their target density is about one interval per 100 pt along
        x (45 pt along y); labels may require fewer ticks. Explicit positions need not follow
        that grid.

        The positions are used in the order given. An empty list leaves the x axis without
        ticks and gridlines. Calling any x tick method again replaces the ticks and drops
        labels set with ``xticks_labeled()``. Without explicit limits, the x axis
        covers both the data and the ticks; with explicit limits, ticks outside them are not
        drawn.

        Args:
            positions: The tick positions: finite, within the supported magnitudes, positive
                on a log axis, and strictly increasing; times on a time axis, labelled for the
                unit they share (years, months, days or the clock).

        Returns:
            These axes.

        Raises:
            TypeError: If ``positions`` is not a sequence or an array.
            DataError: If ``positions`` cannot be read as numbers, or as times on a time axis.
            StyleError: If a position is not finite, out of range, not positive on a log
                axis, or not above the one before it.
            UsageError: If the x axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 12, 24], [1, 3, 2])
            >>> ax = ax.xticks([0, 12, 24])
        """
        with api_call():
            context = "Axes.xticks()"
            if self._x == _TIME:
                places = _time.micros(context, "positions", positions)
                self._native.xticks_time(self._row, self._col, places)
                return self
            self._native.xticks(self._row, self._col, floats(context, "positions", positions))
            return self

    def yticks(self: SY, positions: Values[Y]) -> SY:
        """Set the y ticks to ``positions``, replacing the automatic ones.

        Automatic linear ticks are whole multiples of their step, with a tick at zero when
        the domain includes it. Their target density is about one interval per 45 pt along
        y (100 pt along x); labels may require fewer ticks. Explicit positions need not follow
        that grid.

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
            context = "Axes.yticks()"
            if self._y == _TIME:
                places = _time.micros(context, "positions", positions)
                self._native.yticks_time(self._row, self._col, places)
                return self
            self._native.yticks(self._row, self._col, floats(context, "positions", positions))
            return self

    def xticks_labeled(self: SX, positions: Values[X], labels: Values[str]) -> SX:
        """Set the x ticks to ``positions``, each with the label at the same index of ``labels``.

        The labels are drawn as given. The ticks follow the rules of ``Axes.xticks()``.
        Labels too wide to stay inside the figure make rendering raise ``StyleError``.
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
            if self._x == _TIME:
                times = _time.micros(context, "positions", positions)
                texts = names(context, "labels", labels, check_length=False)
                self._native.xticks_labeled_time(self._row, self._col, times, texts)
                return self
            places = floats(context, "positions", positions)
            texts = names(context, "labels", labels, check_length=False)
            self._native.xticks_labeled(self._row, self._col, places, texts)
            return self

    def yticks_labeled(self: SY, positions: Values[Y], labels: Values[str]) -> SY:
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
            if self._y == _TIME:
                times = _time.micros(context, "positions", positions)
                texts = names(context, "labels", labels, check_length=False)
                self._native.yticks_labeled_time(self._row, self._col, times, texts)
                return self
            places = floats(context, "positions", positions)
            texts = names(context, "labels", labels, check_length=False)
            self._native.yticks_labeled(self._row, self._col, places, texts)
            return self

    def xtick_format(self: SX, format: TickFormat) -> SX:
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
            UsageError: If the x axis is a category axis, the format does not suit the axis
                (``TickFormat.iso()`` labels time axes only, number formats numeric ones), or
                the x ticks have labels and ``format`` is not ``TickFormat.AUTO``.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1], [0.1, 0.4])
            >>> ax = ax.xtick_format(ub.TickFormat.percent())
        """
        with api_call():
            checked = instance("Axes.xtick_format()", "format", format, TickFormat, _FORMAT_HINT)
            self._native.xtick_format(self._row, self._col, _ticks.native(checked))
            return self

    def ytick_format(self: SY, format: TickFormat) -> SY:
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
            UsageError: If the format does not suit the axis (``TickFormat.iso()`` labels
                time axes only, number formats numeric ones), or the y ticks have labels and
                ``format`` is not ``TickFormat.AUTO``.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1], [0.1, 0.4])
            >>> ax = ax.ytick_format(ub.TickFormat.percent())
        """
        with api_call():
            checked = instance("Axes.ytick_format()", "format", format, TickFormat, _FORMAT_HINT)
            self._native.ytick_format(self._row, self._col, _ticks.native(checked))
            return self

    def legend(self, style: LegendStyle, *, position: LegendPosition | Auto = AUTO) -> Self:
        """Set the legend style and position, overriding the theme's.

        The legend lists marks with a non-empty ``label``; ``LegendStyle.NONE`` hides it.
        Positions apply to single-panel figures and ``Legend.PER_PANEL``. A merged grid
        legend stays under the figure title and subtitle and follows their alignment.
        A swatch shows a mark's fill as drawn: composited over the plot background, which
        leaves it opaque over an opaque background. Gradient fills show their most opaque end
        (0.32 of the series opacity for areas, 0.95 for histograms). Area and stacked-area
        swatches are filled rectangles under an edge in the line's color, dash and width;
        scatter, stem and bubble swatches show the marker fill at the theme's marker alpha.
        Lines, steps, reference lines and error bars keep the series color, as do direct
        labels and the outlines of box and violin swatches.

        Args:
            style: How the legend is laid out.
            position: Where the panel legend sits. ``AUTO`` uses the theme's position and
                clears any previous override. ``LegendPosition.BEST`` instead chooses the
                position inside the plot that covers the least data.

        Returns:
            These axes.

        Raises:
            TypeError: If ``style`` is not a ``LegendStyle``, or ``position`` is neither a
                ``LegendPosition`` nor ``AUTO``.
            UsageError: If ``style`` is ``DIRECT`` and a ``LegendPosition`` is supplied.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1], [1, 2], label="North")
            >>> ax = ax.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT)
            >>> ax = ax.legend(ub.LegendStyle.DIRECT)
        """
        with api_call():
            member = instance("Axes.legend()", "style", style, LegendStyle, _LEGEND_HINT)
            if position is AUTO:
                self._native.legend(self._row, self._col, member.value)
            else:
                placement = instance(
                    "Axes.legend()", "position", position, LegendPosition, _LEGEND_POSITION_HINT
                )
                self._native.legend_at(self._row, self._col, member.value, placement.value)
            return self

    def line(
        self: SY,
        x: Values[X],
        y: Values[Y],
        *,
        label: str = "",
        color: Color = Color.AUTO,
        dash: Dash = Dash.SOLID,
        width: Size = Size.AUTO,
    ) -> SY:
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
            UsageError: If the y axis is a category axis.

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
            ys = self._y_values(context, y)
            name = _args.text(context, "label", label)
            self._native.line(self._row, self._col, xs, ys, name, spec, style.value, extent)
            return self

    def area(
        self: SY,
        x: Values[X],
        y: Values[Y],
        *,
        label: str = "",
        color: Color = Color.AUTO,
        dash: Dash = Dash.SOLID,
        width: Size = Size.AUTO,
    ) -> SY:
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
            UsageError: If the y axis is a category axis.

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
            ys = self._y_values(context, y)
            name = _args.text(context, "label", label)
            self._native.area(self._row, self._col, xs, ys, name, spec, style.value, extent)
            return self

    def scatter(
        self,
        x: Values[X],
        y: Values[Y],
        *,
        label: str = "",
        color: Color | ColorBy = Color.AUTO,
        marker: Marker = Marker.CIRCLE,
        size: Size = Size.AUTO,
    ) -> Self:
        """Add a marker at each point ``(x[i], y[i])``.

        A point with a NaN coordinate is left out. On a category y axis the scatter is a dot
        plot: each point sits at the centre of its category's band.

        Args:
            x: The x values: numbers on a linear or log x axis, names on a category x axis.
            y: The y values, one per x value: numbers on a linear or log y axis, names on a
                category y axis.
            label: The name shown in the legend; an empty string leaves the series out of it.
                A scatter colored by ``ColorBy`` has no legend entry and takes no label.
            color: The marker color. ``Color.AUTO`` takes the next color of the theme palette.
                ``ColorBy(values)`` colors each point by its value instead; its ``cmap``,
                ``scale`` and ``center`` must match the other color-mapped marks of these axes.
            marker: The marker shape.
            size: The marker size. ``Size.AUTO`` is the marker size of the theme.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, names and numbers do not match the
                axes, or a ``ColorBy`` does not have one value per point.
            StyleError: If ``label`` is too long, or a ``ColorBy`` does not suit the color
                range set by ``clim()``.
            UsageError: If both axes are category axes (type checkers accept such axes, so
                this is raised when the call runs), a ``ColorBy`` comes with a label, or its
                ``cmap``, ``scale`` or ``center`` differs from those of the other color-mapped
                marks of these axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.scatter([1, 2, 3], [2.0, 1.0, 3.0], marker=ub.Marker.DIAMOND)
            >>> ax = ax.scatter([1, 2, 3], [3.0, 2.5, 1.0], color=ub.ColorBy([0.2, 0.5, 0.9]))
            >>> dots = ub.Figure().add_axes(y=ub.Scale.category())
            >>> dots = dots.scatter([78.7, 81.0], ["United States", "Germany"])
        """
        with api_call():
            context = "Axes.scatter()"
            check_axis_type(context, "x", x)
            check_axis_type(context, "y", y)
            _args.require_text(context, "label", label)
            spec = native_point_color(context, color)
            shape = instance(context, "marker", marker, Marker, _MARKER_HINT)
            extent = _size_argument(context, "size", size)
            xs = self._x_values(context, x)
            ys = self._y_values(context, y)
            name = _args.text(context, "label", label)
            self._native.scatter(self._row, self._col, xs, ys, name, spec, shape.value, extent)
            return self

    def bubbles(
        self: SXY,
        x: Values[X],
        y: Values[Y],
        sizes: Values[float],
        *,
        label: str = "",
        color: Color | ColorBy = Color.AUTO,
        marker: Marker = Marker.CIRCLE,
    ) -> SXY:
        """Add markers whose areas are proportional to non-negative size values.

        Bubble marks on these axes share one size scale. The largest size among points with
        finite x and y coordinates has a radius of 18 points. Zero sizes and NaN coordinates
        draw nothing; a zero-size point's position still counts in the axis domains like any
        point. Markers draw in data order, without downsampling, with the theme's marker outline.
        Automatic limits fit each whole marker and its outline on the final plot, along each
        axis by the marker shape's own extent, in log space on a log axis and in seconds on a
        time axis, before tick rounding and padding. Shared axes fit bubbles from every panel
        sharing the scale. When
        the bubbles cannot all fit on an automatic axis (a marker, outline included, at least
        as long as the plot along it, limits beyond the supported numeric range, or limits
        that do not settle), that axis's limits come from the data positions alone and the
        plot clips the bubbles that overflow; rendering does not raise. Explicit limits still
        clip; scatter limits are unchanged. Legend swatches and size circles show the marker
        fill composited over the plot background.
        Up to three reference circles at their true radius appear once, after the first labelled
        bubble entry, formatted like the y ticks. Repeated labels keep the larger circle; labels
        that round to zero are omitted. If all labels round to zero, only the largest reference
        circle remains, with an automatic label for its size, without the y format or its affixes.
        An empty or all-zero size scale shows only label entries.

        A merged figure legend also shows only one set, after its first labelled bubble entry,
        using that panel's scale and y tick format. Panels do not share a size scale unless
        their size values give them the same maximum. A time y axis has no number format, so
        the size references take the automatic one.

        Args:
            x: The x coordinates: numbers on a linear or log x axis, times on a time x axis. A
                missing time (``NaT``) leaves its bubble out.
            y: The y coordinates, one per x coordinate: numbers on a linear or log y axis,
                times on a time y axis.
            sizes: Finite values from 0 to 1e300, one per point; area is proportional to value.
            label: The legend label; empty skips this mark's legend entry.
            color: The marker color. ``Color.AUTO`` takes the next palette color.
                ``ColorBy(values)`` colors each bubble by its value instead, as for
                ``scatter()``; the label entry and size references are then drawn in the
                theme's text color.
            marker: The marker shape; size references are always circles.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If lengths differ, coordinates are invalid for their axes, a size
                is negative, non-finite, or above 1e300, or a ``ColorBy`` does not have one
                value per point.
            UsageError: If either axis is a category axis (type checkers refuse such axes, so
                this is raised only for calls they do not check), or a ``ColorBy`` differs in
                ``cmap``, ``scale`` or ``center`` from the other color-mapped marks.
            StyleError: If ``label`` is too long, or a ``ColorBy`` does not suit the color
                range set by ``clim()``.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.bubbles([1, 2, 3], [4, 2, 5], [100, 400, 870], label="Cities")
            >>> import datetime as dt
            >>> daily = ub.Figure().add_axes(x=ub.Scale.time())
            >>> days = [dt.date(2026, 1, 1), dt.date(2026, 1, 3)]
            >>> daily = daily.bubbles(days, [4, 2], [100, 400])
        """
        with api_call():
            context = "Axes.bubbles()"
            _check_coordinates_type(context, "x", x, self._x)
            _check_coordinates_type(context, "y", y, self._y)
            check_floats_type(context, "sizes", sizes)
            _args.require_text(context, "label", label)
            spec = native_point_color(context, color)
            shape = instance(context, "marker", marker, Marker, _MARKER_HINT)
            xs = _limit_values(context, "x", x, self._x)
            ys = _limit_values(context, "y", y, self._y)
            values = floats(context, "sizes", sizes)
            name = _args.text(context, "label", label)
            self._native.bubbles(self._row, self._col, xs, ys, values, name, spec, shape.value)
            return self

    def stem(
        self: SY,
        x: Values[X],
        y: Values[Y],
        *,
        label: str = "",
        color: Color = Color.AUTO,
        marker: Marker = Marker.CIRCLE,
        size: Size = Size.AUTO,
    ) -> SY:
        """Add a line from the baseline to each value, with a marker at its tip.

        A linear y domain includes zero. On a log y axis, stems start at the baseline.
        Category x values sit at band centres. A NaN coordinate leaves out that point.
        Explicit limits clip stems and markers; points are not downsampled. A labelled
        series takes a legend row entry, including under ``LegendStyle.DIRECT``.

        Args:
            x: Numbers on a linear or log x axis, names on a category x axis.
            y: The y values, one per x value.
            label: The legend label; an empty string leaves the series out of the legend.
            color: The stem and marker color. ``Color.AUTO`` takes the next palette color.
            marker: The marker shape, drawn as a scatter marker.
            size: The marker diameter. ``Size.AUTO`` uses the theme's marker size. The stem
                width stays at 1.5 points regardless of this size.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If lengths differ, a value is infinite, a magnitude is too large, a
                value is not positive or is too small on a log axis, or x has the wrong kind.
            StyleError: If ``label`` is too long.
            UsageError: If the y axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.stem([0, 1, 2, 3], [4.0, -2.0, 1.0, 0.5], label="Response")
        """
        with api_call():
            context = "Axes.stem()"
            check_axis_type(context, "x", x)
            check_floats_type(context, "y", y)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            shape = instance(context, "marker", marker, Marker, _MARKER_HINT)
            extent = _size_argument(context, "size", size)
            xs = self._x_values(context, x)
            ys = self._y_values(context, y)
            name = _args.text(context, "label", label)
            self._native.stem(self._row, self._col, xs, ys, name, spec, shape.value, extent)
            return self

    def bar(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        heights: Values[float],
        *,
        label: str = "",
        color: Color = Color.AUTO,
    ) -> "Axes[X, Y]":
        """Add one bar per category, from the baseline to its height, along the category axis.

        On a category x axis the bars stand on the baseline and a negative height draws below
        it. On a category y axis the bars are horizontal: the categories run top to bottom in
        order of first appearance, and the bars grow rightwards from the baseline, leftwards for
        a negative height. A NaN height leaves its category without a bar.

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
                height is infinite or not positive on a log axis, or neither axis is a
                category axis.
            StyleError: If ``label`` or a category name is too long.
            UsageError: If both axes are category axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.bar(["North", "South", "East"], [3, 5, 2], label="2025")
            >>> rows = ub.Figure().add_axes(y=ub.Scale.category())
            >>> rows = rows.bar(["North", "South", "East"], [3, 5, 2])
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
            self._native.bar(self._row, self._col, category_names, values, name, spec)
            # The self type only restricts the call: these are the Axes[X, Y] it was made on.
            return cast("Axes[X, Y]", self)

    def box(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        groups: Sequence[Values[float]],
        *,
        label: str = "",
        color: Color = Color.AUTO,
    ) -> "Axes[X, Y]":
        """Add a box plot for each category, ignoring NaN samples.

        Quartiles use linear interpolation (Hyndman-Fan type 7, NumPy's default). The fences
        lie 1.5 times the interquartile range beyond the quartiles; samples outside them are
        outliers. Whiskers reach the most extreme samples inside the fences, clamped to the
        quartiles so they never extend into the box. Empty groups draw nothing; when the
        quartiles coincide, only the median and any outliers are drawn. The value axis domain
        includes whiskers and outliers. Boxes occupy half of each category band and use
        the theme's area opacity. On a category y axis the boxes are horizontal, the vertical
        form with the axes swapped.

        Args:
            categories: The category names, each appearing once.
            groups: One group of numeric samples per category, in category order.
            label: The legend label; an empty string omits the legend entry.
            color: The box color. ``Color.AUTO`` takes the next palette color.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If group counts differ, categories repeat, or samples are not
                numeric, are infinite, exceed the supported range or are not positive on
                a log axis.
            StyleError: If a label or category name is too long.
            UsageError: If neither axis or both axes are category axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.box(["North", "South"], [[1, 2, 4, 9], [2, 3, 5]], label="Samples")
            >>> rows = ub.Figure().add_axes(y=ub.Scale.category())
            >>> rows = rows.box(["North", "South"], [[1, 2, 4, 9], [2, 3, 5]])
        """
        with api_call():
            context = "Axes.box()"
            check_names_type(context, "categories", categories)
            _series.check_groups_type(context, groups)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            category_names = names(context, "categories", categories)
            values = _series.group_values(context, groups)
            name = _args.text(context, "label", label)
            self._native.box_plot(self._row, self._col, category_names, values, name, spec)
            # The self type only restricts the call: these are the Axes[X, Y] it was made on.
            return cast("Axes[X, Y]", self)

    def grouped_box(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        distributions: Sequence[Distribution],
    ) -> "Axes[X, Y]":
        """Add distributions side by side within each category band.

        Statistics and NaN handling are as in ``box()``. The group occupies half the band;
        boxes are 2 pt apart, or touch if that gap would leave them narrower than the gap.
        Each distribution has its own color and legend entry, in input order: left to right,
        or top to bottom on a category y axis.

        Args:
            categories: The category names, each appearing once.
            distributions: One or more distributions, each with one group per category.

        Returns:
            These axes.

        Raises:
            TypeError: If categories or distributions have the wrong type.
            DataError: If distributions are empty, group counts differ, categories repeat,
                or a sample is infinite, exceeds the supported range or is not positive
                on a log axis.
            StyleError: If a category name is too long.
            UsageError: If neither axis or both axes are category axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.grouped_box(["2026"], [
            ...     ub.Distribution("North", [[1, 2, 4]]),
            ...     ub.Distribution("South", [[2, 3, 5]]),
            ... ])
        """
        with api_call():
            context = "Axes.grouped_box()"
            check_names_type(context, "categories", categories)
            arguments = _distribution_arguments(context, distributions)
            category_names = names(context, "categories", categories)
            self._native.grouped_box_plot(self._row, self._col, category_names, arguments)
            # The self type only restricts the call: these are the Axes[X, Y] it was made on.
            return cast("Axes[X, Y]", self)

    def grouped_bar(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        series: Sequence[Series],
    ) -> "Axes[X, Y]":
        """Add grouped bars: in each category, one bar per series, side by side.

        The bars grow as in ``bar()``. Within a category they follow the order of the series,
        which is the order of the legend: left to right, or top to bottom on a category y axis.

        Args:
            categories: The category names, each at most once.
            series: One or more ``Series``, each with one value per category. The series
                take their places in the group in this order.

        Returns:
            These axes.

        Raises:
            TypeError: If ``series`` is not a sequence of ``Series``, or ``categories`` has
                the wrong type.
            DataError: If the data cannot be plotted: ``series`` is empty, a series has a
                different length, a category repeats, a height is infinite or not positive on
                a log axis, or neither axis is a category axis.
            StyleError: If a category name is too long.
            UsageError: If both axes are category axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.grouped_bar(
            ...     ["Q1", "Q2", "Q3"],
            ...     [ub.Series("North", [3, 5, 2]), ub.Series("South", [4, 1, 3])],
            ... )
        """
        with api_call():
            context = "Axes.grouped_bar()"
            check_names_type(context, "categories", categories)
            arguments = _series_arguments(context, series)
            category_names = names(context, "categories", categories)
            self._native.grouped_bar(self._row, self._col, category_names, arguments)
            # The self type only restricts the call: these are the Axes[X, Y] it was made on.
            return cast("Axes[X, Y]", self)

    def stacked_bar(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        series: Sequence[Series],
    ) -> "Axes[X, Y]":
        """Add stacked bars: in each category, the series stacked on one another.

        Positive and negative values stack separately, above and below the baseline, or on a
        category y axis right and left of it.

        Args:
            categories: The category names, each at most once.
            series: One or more ``Series``, each with one value per category. The first
                series sits on the baseline.

        Returns:
            These axes.

        Raises:
            TypeError: If ``series`` is not a sequence of ``Series``, or ``categories`` has
                the wrong type.
            DataError: If the data cannot be plotted: ``series`` is empty, a series has a
                different length, a category repeats, a height is infinite or not positive on
                a log axis, a stack exceeds the supported magnitude, or neither axis is a
                category axis.
            StyleError: If a category name is too long.
            UsageError: If both axes are category axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.stacked_bar(
            ...     ["Q1", "Q2", "Q3"],
            ...     [ub.Series("North", [3, 5, 2]), ub.Series("South", [4, 1, 3])],
            ... )
        """
        with api_call():
            context = "Axes.stacked_bar()"
            check_names_type(context, "categories", categories)
            arguments = _series_arguments(context, series)
            category_names = names(context, "categories", categories)
            self._native.stacked_bar(self._row, self._col, category_names, arguments)
            # The self type only restricts the call: these are the Axes[X, Y] it was made on.
            return cast("Axes[X, Y]", self)

    def stacked_area(self: SXF, x: Values[X], series: Sequence[Series]) -> SXF:
        """Add non-negative series stacked in order over shared x values.

        Each layer fills between the previous cumulative curve and its own, flat at the
        opacity of an area in every fill style, with its upper curve outlined at the theme's
        line width. The first layer starts at the baseline. A linear y axis includes zero;
        on log y the baseline is the lower limit. A NaN in x or any series breaks every layer
        at that index. Long inputs are reduced per device column, keeping every cumulative
        curve's envelope and exact crossings at explicit limits.

        Args:
            x: Numeric x values in non-decreasing order, NaN apart; times on a time x axis,
                ``NaT`` apart.
            series: One or more ``Series``, each as long as x, with non-negative values.
                Their order is the stacking order. Each series takes its own color and an
                area's legend swatch. Under ``LegendStyle.DIRECT``, each label goes at the
                end of its cumulative upper curve, as a line's does.

        Returns:
            These axes.

        Raises:
            TypeError: If x has the wrong type or series is not a sequence of ``Series``.
            DataError: If series is empty, lengths differ, x is not sorted, a value is
                negative or infinite, a value violates its scale's bounds, or a cumulative
                total exceeds the supported magnitude. Log values must be positive.
            UsageError: If either axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.stacked_area(
            ...     [2020, 2021, 2022],
            ...     [ub.Series("Hydro", [12, 13, 12]), ub.Series("Wind", [6, 8, 10])],
            ... )
        """
        with api_call():
            context = "Axes.stacked_area()"
            check_floats_type(context, "x", x)
            arguments = _series_arguments(context, series)
            if self._x == _TIME:
                times = _time.micros(context, "x", x)
                self._native.stacked_area_time(self._row, self._col, times, arguments)
                return self
            xs = floats(context, "x", x)
            self._native.stacked_area(self._row, self._col, xs, arguments)
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
            self._native.hist(self._row, self._col, data, count, name, spec)
            return self

    def mesh(
        self: SXY,
        x_edges: Values[X],
        y_edges: Values[Y],
        values: object,
        *,
        cmap: Cmap | Auto = AUTO,
        scale: Scale[float] = Scale.linear(),  # noqa: B008
        center: float | Auto = AUTO,
    ) -> SXY:
        """Add rectangular cells whose colors map a row-major matrix of values.

        Cells span their edges exactly, without padding their extents. Other marks on the
        axes, on a numeric or a time axis, keep their usual padding; the domain is the union.
        NaN values leave cells blank. Neither axis may be a category axis; the outer edge
        ranges of a numeric axis must meet the axis precision limits, as for ``xlim()`` and
        ``ylim()``. All color-mapped marks share one color scale.

        Args:
            x_edges: Strictly increasing column edges, one more than the number of columns:
                numbers on a linear or log x axis, times on a time x axis.
            y_edges: Strictly increasing row edges, one more than the number of rows: numbers
                on a linear or log y axis, times on a time y axis.
            values: A two-dimensional numeric matrix; rows run along y and columns along x.
            cmap: The colormap, or ``AUTO`` for the theme's colormap.
            scale: A linear or log color scale, independent of the axis scales.
            center: The middle of a diverging mapping, or ``AUTO`` for sequential colors.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the matrix shape or edges are invalid (a missing time among time
                edges included), or a value is invalid.
            StyleError: If the color center is invalid.
            UsageError: If an axis is a category axis (type checkers refuse such axes, so this is
                raised only for calls they do not check), or color scales conflict.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.mesh([0, 1, 3], [0, 2], [[1.0, 4.0]])
            >>> import datetime as dt
            >>> hours = ub.Figure().add_axes(x=ub.Scale.time())
            >>> times = [dt.datetime(2026, 1, 1, hour) for hour in (0, 6, 12)]
            >>> hours = hours.mesh(times, [0, 2], [[1.0, 4.0]])
        """
        with api_call():
            context = "Axes.mesh()"
            _check_coordinates_type(context, "x_edges", x_edges, self._x)
            _check_coordinates_type(context, "y_edges", y_edges, self._y)
            check_floats_type(context, "values", values)
            colors = _cell_colors(context, cmap, scale, center)
            _numeric_cells(context, self, time=True)
            xs = _limit_values(context, "x_edges", x_edges, self._x)
            ys = _limit_values(context, "y_edges", y_edges, self._y)
            data, rows, columns = _matrix(context, values)
            for name, edges, count, dimension in (
                ("x_edges", xs, columns, "columns"),
                ("y_edges", ys, rows, "rows"),
            ):
                if len(edges) != count + 1:
                    raise new_error(
                        DataError,
                        context,
                        f"{name} has {len(edges)} values; values has {count} {dimension}, "
                        f"so {name} needs {count + 1}.",
                        parameter=name,
                        hint=f"pass one more edge than the number of {dimension}.",
                    )
            self._native.mesh(self._row, self._col, xs, ys, data, *colors)
            return self

    def hist2d(
        self: "Axes[float, float]",
        x: Values[float],
        y: Values[float],
        *,
        bins: int | tuple[int, int] = 30,
        cmap: Cmap | Auto = AUTO,
        scale: Scale[float] = Scale.linear(),  # noqa: B008
    ) -> "Axes[float, float]":
        """Count paired points in equal-width rectangular bins over their data ranges.

        Widths are equal in axis space: in data on a linear axis, in the logarithm on a log
        axis. A pair with either coordinate NaN is skipped. Empty bins are blank, including
        on a log color scale. Both axes must be numeric; counts share the axes' color scale.

        Args:
            x: The x coordinates.
            y: The y coordinates, one per x coordinate.
            bins: One count for both axes, or ``(x_count, y_count)``, each in 1..MAX_BINS.
            cmap: The colormap, or ``AUTO`` for the theme's sequential colormap.
            scale: A linear or log color scale.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If coordinates are invalid or their lengths differ.
            StyleError: If a bin count is out of range.
            UsageError: If an axis is not numeric, or color scales conflict.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.hist2d([0, 1, 1], [0, 2, 2], bins=(4, 3))
        """
        with api_call():
            context = "Axes.hist2d()"
            check_floats_type(context, "x", x)
            check_floats_type(context, "y", y)
            counts = _rect_counts(context, bins)
            colors = _cell_colors(context, cmap, scale, AUTO)
            _numeric_cells(context, self, time=False)
            xs, ys = floats(context, "x", x), floats(context, "y", y)
            self._native.hist2d(self._row, self._col, xs, ys, counts, *colors[:3])
            return self

    def hexbin(
        self: "Axes[float, float]",
        x: Values[float],
        y: Values[float],
        *,
        gridsize: int = 30,
        cmap: Cmap | Auto = AUTO,
        scale: Scale[float] = Scale.linear(),  # noqa: B008
    ) -> "Axes[float, float]":
        """Count paired points in pointy-top hexagons, regular in screen coordinates.

        Automatic limits include whole hexagons without changing the plot's shape. Explicit
        limits clip cells. Pairs containing NaN are skipped and empty cells stay blank.
        With a narrow explicit ``xlim`` and automatic y limits, hexagons keep their size;
        the y range may grow far beyond the data to keep every hexagon whole and regular.
        Set ``ylim()`` or choose a larger ``gridsize`` to control that range.

        Args:
            x: The x coordinates on numeric axes.
            y: The y coordinates, one per x coordinate.
            gridsize: The number of hexagons across the x data range, in 1..MAX_BINS.
            cmap: The colormap, or ``AUTO`` for the theme's sequential colormap.
            scale: A linear or log color scale; counts share the axes' color scale.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If coordinates are invalid or their lengths differ.
            StyleError: If ``gridsize`` is out of range.
            UsageError: If an axis is not numeric, or color scales conflict.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.hexbin([0, 1, 1], [0, 2, 2], gridsize=5, scale=ub.Scale.log())
        """
        with api_call():
            context = "Axes.hexbin()"
            check_floats_type(context, "x", x)
            check_floats_type(context, "y", y)
            count = _cell_count(context, "gridsize", gridsize)
            colors = _cell_colors(context, cmap, scale, AUTO)
            _numeric_cells(context, self, time=False)
            xs, ys = floats(context, "x", x), floats(context, "y", y)
            self._native.hexbin(self._row, self._col, xs, ys, count, *colors[:3])
            return self

    def step(
        self: SY,
        x: Values[X],
        y: Values[Y],
        *,
        where: Step = Step.POST,
        label: str = "",
        color: Color = Color.AUTO,
        dash: Dash = Dash.SOLID,
        width: Size = Size.AUTO,
    ) -> SY:
        """Add a step line through the points ``(x[i], y[i])``, in order.

        The line runs horizontally and changes level vertically, at the place ``where`` sets.
        A NaN in ``x`` or ``y`` leaves a gap in the line. On a linear or log x axis, ``x`` must
        be in non-decreasing order; on a category x axis the line runs between the band
        centres, in the order given.

        Args:
            x: The x values: numbers on a linear or log x axis, names on a category x axis.
            y: The y values, one per x value.
            where: Where the line changes level: ``Step.POST`` holds each value from its x to
                the next x, ``Step.PRE`` holds it back from its x to the previous x, and
                ``Step.MID`` changes level halfway between two x values as drawn: at their
                mean on a linear x axis, halfway between the band centres on a category x
                axis, and at their geometric mean on a log x axis (between 1 and 100, at 10).
            label: The name shown in the legend; an empty string leaves the line out of it.
            color: The line color. ``Color.AUTO`` takes the next color of the theme palette.
            dash: How the line is stroked.
            width: The line width. ``Size.AUTO`` is the line width of the theme.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, names and numbers do not match the x
                axis, or x is not in non-decreasing order.
            StyleError: If ``label`` is too long.
            UsageError: If the y axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.step([0, 1, 3, 4], [1.0, 2.0, float("nan"), 0.5], where=ub.Step.MID)
        """
        with api_call():
            context = "Axes.step()"
            check_axis_type(context, "x", x)
            check_floats_type(context, "y", y)
            position = instance(context, "where", where, Step, _STEP_HINT)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            style = instance(context, "dash", dash, Dash, _DASH_HINT)
            extent = _size_argument(context, "width", width)
            xs = self._x_values(context, x)
            ys = self._y_values(context, y)
            name = _args.text(context, "label", label)
            self._native.step(
                self._row, self._col, xs, ys, position.value, name, spec, style.value, extent
            )
            return self

    def hline(
        self: SY,
        y: Y,
        *,
        label: str = "",
        color: Color = Color.AUTO,
        dash: Dash = Dash.SOLID,
        width: Size = Size.AUTO,
    ) -> SY:
        """Add a horizontal reference line across the whole plot at ``y``.

        The line is drawn above the data marks. Without explicit y limits the y axis extends to
        include ``y``; with limits, a line outside them is not drawn.

        Args:
            y: The y value of the line; finite, and positive on a log y axis; a time on a time
                y axis.
            label: The name shown in the legend; an empty string leaves the line out of it.
                A reference line always takes an entry in the legend row, also under
                ``LegendStyle.DIRECT``.
            color: The line color. ``Color.AUTO`` takes the next color of the theme palette.
            dash: How the line is stroked.
            width: The line width. ``Size.AUTO`` is the line width of the theme.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``y`` is not finite, or not positive on a log axis.
            StyleError: If ``label`` is too long.
            UsageError: If the y axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1, 2], [1.0, 3.0, 2.0])
            >>> ax = ax.hline(2.5, label="Threshold", dash=ub.Dash.DASHED)
        """
        with api_call():
            context = "Axes.hline()"
            timed = self._y == _TIME
            value = _time.instant(context, "y", y) if timed else real(context, "y", y)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            style = instance(context, "dash", dash, Dash, _DASH_HINT)
            extent = _size_argument(context, "width", width)
            name = _args.text(context, "label", label)
            if isinstance(value, int):
                self._native.hline_time(
                    self._row, self._col, value, name, spec, style.value, extent
                )
            else:
                self._native.hline(self._row, self._col, value, name, spec, style.value, extent)
            return self

    def vline(
        self,
        x: X,
        *,
        label: str = "",
        color: Color = Color.AUTO,
        dash: Dash = Dash.SOLID,
        width: Size = Size.AUTO,
    ) -> Self:
        """Add a vertical reference line at ``x`` across the plot.

        On a linear or log x axis ``x`` is a number, and without explicit x limits the x axis
        extends to include it. On a category x axis ``x`` is a category name, and the line is
        drawn at the centre of that category. A name does not add a category: it must be the
        name of one of the categories of the other marks of these axes, which is checked when
        the figure is rendered (``DataError`` from ``to_png()``, ``to_svg()`` and the save
        methods). The line is drawn above the data marks. It runs the full height of the plot,
        except when the baseline sits at the y axis's lower limit (0 is not inside a linear y
        domain): then it ends at the baseline, since only padding lies below it.

        Args:
            x: The x value of the line: a finite number, positive on a log axis, a category
                name, or a time on a time axis.
            label: The name shown in the legend; an empty string leaves the line out of it.
                A reference line always takes an entry in the legend row, also under
                ``LegendStyle.DIRECT``.
            color: The line color. ``Color.AUTO`` takes the next color of the theme palette.
            dash: How the line is stroked.
            width: The line width. ``Size.AUTO`` is the line width of the theme.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type, including a number on a category
                axis or a name on a linear or log axis.
            DataError: If ``x`` is not finite, or not positive on a log axis.
            StyleError: If ``label`` is too long.
            UsageError: If both axes are category axes. Type checkers accept such axes,
                so this is raised when the call runs.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 30, 60], [1.0, 3.0, 2.0])
            >>> ax = ax.vline(31, label="Event")
            >>> cats = ub.Figure().add_axes(x=ub.Scale.category()).bar(["Q1", "Q2"], [4, 6])
            >>> cats = cats.vline("Q2")
        """
        with api_call():
            context = "Axes.vline()"
            if self._x == _TIME:
                when = _time.instant(context, "x", x)
                _args.require_text(context, "label", label)
                spec = _color_argument(context, color)
                style = instance(context, "dash", dash, Dash, _DASH_HINT)
                extent = _size_argument(context, "width", width)
                name = _args.text(context, "label", label)
                self._native.vline_time(self._row, self._col, when, name, spec, style.value, extent)
                return self
            # The kind of x follows the x axis: a name on a category axis, a number otherwise.
            if self._x == _CATEGORY:
                _args.require_text(context, "x", x)
            position: float | str = cast(str, x) if self._x == _CATEGORY else real(context, "x", x)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            style = instance(context, "dash", dash, Dash, _DASH_HINT)
            extent = _size_argument(context, "width", width)
            if isinstance(position, str):
                position = _args.checked_text(context, "x", "x", position, DataError)
            name = _args.text(context, "label", label)
            self._native.vline(self._row, self._col, position, name, spec, style.value, extent)
            return self

    def hspan(
        self: SY,
        low: Y,
        high: Y,
        *,
        label: str = "",
        color: Color = Color.AUTO,
    ) -> SY:
        """Add a horizontal reference span between the y values ``low`` and ``high``.

        The span fills the region between the two values across the whole plot, at the opacity
        of an area, and is drawn below the data marks. Without explicit y limits the y axis
        extends to include both values; with limits, the span is clipped to them.

        Args:
            low: The lower y value; finite, and positive on a log y axis.
            high: The upper y value; finite and above ``low``.
            label: The name shown in the legend; an empty string leaves the span out of it.
                A reference span always takes an entry in the legend row, also under
                ``LegendStyle.DIRECT``.
            color: The fill color. ``Color.AUTO`` takes the next color of the theme palette.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``low`` or ``high`` is not finite, or not positive on a log axis.
            StyleError: If ``low`` is not below ``high``, or ``label`` is too long.
            UsageError: If the y axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1, 2], [1.0, 3.0, 2.0])
            >>> ax = ax.hspan(1.5, 2.5, label="Normal range")
        """
        with api_call():
            context = "Axes.hspan()"
            if self._y == _TIME:
                start = _time.instant(context, "low", low)
                end = _time.instant(context, "high", high)
                _args.require_text(context, "label", label)
                spec = _color_argument(context, color)
                name = _args.text(context, "label", label)
                self._native.hspan_time(self._row, self._col, start, end, name, spec)
                return self
            lower = real(context, "low", low)
            upper = real(context, "high", high)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            name = _args.text(context, "label", label)
            self._native.hspan(self._row, self._col, lower, upper, name, spec)
            return self

    def vspan(
        self: SX,
        low: X,
        high: X,
        *,
        label: str = "",
        color: Color = Color.AUTO,
    ) -> SX:
        """Add a vertical reference span between the x values ``low`` and ``high``.

        The span fills the region between the two values across the plot, at the opacity of an
        area, and is drawn below the data marks. It has the full height of the plot, except when
        the baseline sits at the y axis's lower limit (0 is not inside a linear y domain): then
        it ends at the baseline. Without explicit x limits the x axis extends to include both
        values; with limits, the span is clipped to them. It needs a linear or log x axis: on a
        category axis use ``vline()``.

        Args:
            low: The lower x value; finite, and positive on a log x axis.
            high: The upper x value; finite and above ``low``.
            label: The name shown in the legend; an empty string leaves the span out of it.
                A reference span always takes an entry in the legend row, also under
                ``LegendStyle.DIRECT``.
            color: The fill color. ``Color.AUTO`` takes the next color of the theme palette.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``low`` or ``high`` is not finite, or not positive on a log axis.
            StyleError: If ``low`` is not below ``high``, or ``label`` is too long.
            UsageError: If the x axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 30, 60], [1.0, 3.0, 2.0])
            >>> ax = ax.vspan(40, 50, label="Window")
        """
        with api_call():
            context = "Axes.vspan()"
            if self._x == _TIME:
                start = _time.instant(context, "low", low)
                end = _time.instant(context, "high", high)
                _args.require_text(context, "label", label)
                spec = _color_argument(context, color)
                name = _args.text(context, "label", label)
                self._native.vspan_time(self._row, self._col, start, end, name, spec)
                return self
            lower = real(context, "low", low)
            upper = real(context, "high", high)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            name = _args.text(context, "label", label)
            self._native.vspan(self._row, self._col, lower, upper, name, spec)
            return self

    def band(
        self: SXF,
        x: Values[X],
        low: Values[float],
        high: Values[float],
        *,
        label: str = "",
        color: Color = Color.AUTO,
    ) -> SXF:
        """Add a band between the curves ``(x[i], low[i])`` and ``(x[i], high[i])``.

        The band is a flat fill at the opacity of an area, with no outline, whatever the fill
        style of the theme. Without explicit limits both curves extend the y axis and ``x``
        the x axis, as the values of a line do. A NaN in ``x``, ``low`` or ``high`` breaks the
        band there into separate pieces. A long band is reduced to the points that keep its
        envelope at the resolution of the plot. It needs a linear or log x axis.

        Args:
            x: The x values, numbers in non-decreasing order, NaN apart; times on a time x
                axis, ``NaT`` apart.
            low: The lower curve, one value per x value.
            high: The upper curve, one value per x value; no value of ``low`` may exceed the
                value of ``high`` at the same index.
            label: The name shown in the legend; an empty string leaves the band out of it.
                A band always takes an entry in the legend row, also under
                ``LegendStyle.DIRECT``.
            color: The fill color. ``Color.AUTO`` takes the next color of the theme palette.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, ``x`` is not in non-decreasing order, or a
                value of ``low`` is above the value of ``high`` at the same index.
            StyleError: If ``label`` is too long.
            UsageError: If either axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.band([0, 1, 2, 3], [1.0, 1.5, float("nan"), 1.0], [2.0, 3.0, 3.5, 2.5])
        """
        with api_call():
            context = "Axes.band()"
            check_floats_type(context, "x", x)
            check_floats_type(context, "low", low)
            check_floats_type(context, "high", high)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            xs: FloatArray | MicrosArray = (
                _time.micros(context, "x", x) if self._x == _TIME else floats(context, "x", x)
            )
            lows = floats(context, "low", low)
            highs = floats(context, "high", high)
            name = _args.text(context, "label", label)
            if xs.dtype == np.int64:
                times = cast(MicrosArray, xs)
                self._native.band_time(self._row, self._col, times, lows, highs, name, spec)
            else:
                numbers = cast(FloatArray, xs)
                self._native.band(self._row, self._col, numbers, lows, highs, name, spec)
            return self

    def y_error_bars(
        self: SY,
        x: Values[X],
        low: Values[Y],
        high: Values[Y],
        *,
        label: str = "",
        color: Color = Color.AUTO,
        width: Size = Size.AUTO,
        caps: bool = True,
    ) -> SY:
        """Add a vertical error bar at each ``x[i]``, from ``low[i]`` to ``high[i]``.

        On a category x axis the bar is drawn at the centre of the category. With ``caps`` each
        end of a bar has a short line across it, as wide as the cap width of the theme. Without
        explicit limits ``low`` and ``high`` extend the y axis and ``x`` the x axis, as the
        values of a scatter do. With limits, a bar partly outside the plot keeps the part
        inside, and loses the cap that is outside. A NaN in ``x``, ``low`` or ``high`` skips
        that bar. Error bars cannot be placed on the bars of ``grouped_bar``: there is no way
        to address the offset of one series of a group.

        Args:
            x: The x values: numbers on a linear or log x axis, names on a category x axis.
            low: The lower end of each bar, one value per x value; times on a time y axis.
            high: The upper end of each bar, one value per x value; no value of ``low`` may
                exceed the value of ``high`` at the same index.
            label: The name shown in the legend; an empty string leaves the bars out of it.
                Error bars always take an entry in the legend row, also under
                ``LegendStyle.DIRECT``.
            color: The color of the bars and caps. ``Color.AUTO`` takes the next color of the
                theme palette.
            width: The line width of the bars and caps. ``Size.AUTO`` is the line width of the
                theme.
            caps: Whether each end has a cap.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, names and numbers do not match the x axis,
                or a value of ``low`` is above the value of ``high`` at the same index.
            StyleError: If ``label`` is too long.
            UsageError: If the y axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.y_error_bars(["a", "b"], [3.5, 4.0], [4.5, 6.0], label="95% interval")
        """
        with api_call():
            context = "Axes.y_error_bars()"
            check_axis_type(context, "x", x)
            check_floats_type(context, "low", low)
            check_floats_type(context, "high", high)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            extent = _size_argument(context, "width", width)
            flag = _args.boolean(context, "caps", caps)
            xs = self._x_values(context, x)
            lows = _limit_values(context, "low", low, self._y)
            highs = _limit_values(context, "high", high, self._y)
            name = _args.text(context, "label", label)
            self._native.y_error_bars(
                self._row, self._col, xs, lows, highs, name, spec, extent, flag
            )
            return self

    def x_error_bars(
        self: SX,
        y: Values[Y],
        low: Values[X],
        high: Values[X],
        *,
        label: str = "",
        color: Color = Color.AUTO,
        width: Size = Size.AUTO,
        caps: bool = True,
    ) -> SX:
        """Add a horizontal error bar at each ``y[i]``, from ``low[i]`` to ``high[i]``.

        With ``caps`` each end of a bar has a short line across it, as wide as the cap width of
        the theme. Without explicit limits ``low`` and ``high`` extend the x axis and ``y`` the
        y axis, as the values of a scatter do. With limits, a bar partly outside the plot keeps
        the part inside, and loses the cap that is outside. A NaN in ``y``, ``low`` or ``high``
        skips that bar. It needs a linear or log x axis. On a category y axis each bar sits at
        the centre of its category's band.

        Args:
            y: The y values: numbers on a linear or log y axis, names on a category y axis,
                times on a time y axis.
            low: The left end of each bar, one value per y value; times on a time x axis.
            high: The right end of each bar, one value per y value; no value of ``low`` may
                exceed the value of ``high`` at the same index.
            label: The name shown in the legend; an empty string leaves the bars out of it.
                Error bars always take an entry in the legend row, also under
                ``LegendStyle.DIRECT``.
            color: The color of the bars and caps. ``Color.AUTO`` takes the next color of the
                theme palette.
            width: The line width of the bars and caps. ``Size.AUTO`` is the line width of the
                theme.
            caps: Whether each end has a cap.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, names and numbers do not match the y
                axis, or a value of ``low`` is above the value of ``high`` at the same index.
            StyleError: If ``label`` is too long.
            UsageError: If the x axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.x_error_bars([1.0, 2.0], [0.5, 1.2], [1.5, 2.0], caps=False)
            >>> rows = ub.Figure().add_axes(y=ub.Scale.category())
            >>> rows = rows.x_error_bars(["North", "South"], [0.5, 1.2], [1.5, 2.0])
        """
        with api_call():
            context = "Axes.x_error_bars()"
            check_axis_type(context, "y", y)
            check_floats_type(context, "low", low)
            check_floats_type(context, "high", high)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            extent = _size_argument(context, "width", width)
            flag = _args.boolean(context, "caps", caps)
            ys = self._y_values(context, y)
            lows = _limit_values(context, "low", low, self._x)
            highs = _limit_values(context, "high", high, self._x)
            name = _args.text(context, "label", label)
            self._native.x_error_bars(
                self._row, self._col, ys, lows, highs, name, spec, extent, flag
            )
            return self

    def _x_values(self, context: str, x: object) -> FloatArray | list[str] | MicrosArray:
        """Convert x values; empty data is an empty list of names on a category x axis.

        On a time x axis the values are times, converted to microseconds.
        """
        if self._x == _TIME:
            return _time.micros(context, "x", x)
        return _scale_values(context, "x", x, self._x)

    def _y_values(self, context: str, y: object) -> FloatArray | list[str] | MicrosArray:
        """Convert y values; empty data is an empty list of names on a category y axis.

        On a time y axis the values are times, converted to microseconds.
        """
        if self._y == _TIME:
            return _time.micros(context, "y", y)
        return _scale_values(context, "y", y, self._y)

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

    def violin(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        groups: Sequence[Values[float]],
        *,
        label: str = "",
        color: Color = Color.AUTO,
    ) -> "Axes[X, Y]":
        """Add a violin density for each category, ignoring NaN samples.

        Gaussian kernels use Scott bandwidth: the sample standard deviation (ddof 1) times
        the sample count to the power -1/5. Densities use 128 evenly spaced points from min
        to max, with no tails outside the data. Every violin has the same maximum width,
        0.6 of its category band, and the theme's area opacity. Type-7 quartiles are drawn
        at half their local width; the median spans the full local width. Empty groups
        draw nothing; fewer than two distinct finite values draw a median line only. On a
        category y axis the violins are horizontal, the vertical form with the axes swapped.

        Args:
            categories: The category names, each appearing once.
            groups: One group of numeric samples per category, in category order.
            label: The legend label; an empty string omits the legend entry.
            color: The violin color. ``Color.AUTO`` takes the next palette color.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If group counts differ, categories repeat, or samples are not
                numeric, are infinite, exceed the supported range or are not positive on
                a log axis.
            StyleError: If a label or category name is too long.
            UsageError: If neither axis or both axes are category axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.violin(["North", "South"], [[1, 2, 4, 9], [2, 3, 5]], label="Samples")
            >>> rows = ub.Figure().add_axes(y=ub.Scale.category())
            >>> rows = rows.violin(["North", "South"], [[1, 2, 4, 9], [2, 3, 5]])
        """
        with api_call():
            context = "Axes.violin()"
            check_names_type(context, "categories", categories)
            _series.check_groups_type(context, groups)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            category_names = names(context, "categories", categories)
            values = _series.group_values(context, groups)
            name = _args.text(context, "label", label)
            self._native.violin(self._row, self._col, category_names, values, name, spec)
            # The self type only restricts the call: these are the Axes[X, Y] it was made on.
            return cast("Axes[X, Y]", self)

    def grouped_violin(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        distributions: Sequence[Distribution],
    ) -> "Axes[X, Y]":
        """Add distributions side by side within each category band.

        Statistics and NaN handling are as in ``violin()``. The group occupies 0.6 of the band;
        violins are 2 pt apart, or touch if that gap would leave them narrower than the gap.
        Each distribution has its own color and legend entry, in input order: left to right,
        or top to bottom on a category y axis.

        Args:
            categories: The category names, each appearing once.
            distributions: One or more distributions, each with one group per category.

        Returns:
            These axes.

        Raises:
            TypeError: If categories or distributions have the wrong type.
            DataError: If distributions are empty, group counts differ, categories repeat,
                or a sample is infinite, exceeds the supported range or is not positive
                on a log axis.
            StyleError: If a category name is too long.
            UsageError: If neither axis or both axes are category axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.grouped_violin(["2026"], [
            ...     ub.Distribution("North", [[1, 2, 4]]),
            ...     ub.Distribution("South", [[2, 3, 5]]),
            ... ])
        """
        with api_call():
            context = "Axes.grouped_violin()"
            check_names_type(context, "categories", categories)
            arguments = _distribution_arguments(context, distributions)
            category_names = names(context, "categories", categories)
            self._native.grouped_violin(self._row, self._col, category_names, arguments)
            # The self type only restricts the call: these are the Axes[X, Y] it was made on.
            return cast("Axes[X, Y]", self)

    def clim(self, low: float, high: float) -> Self:
        """Set the color range of the color-mapped marks of these axes.

        Values outside the range take the colormap's end colors. Without it, the range runs
        from the smallest to the largest finite mapped value. On a centered color scale, the
        range is symmetric about the center and reaches the farther of the two limits.

        Args:
            low: The value at the low end of the colormap.
            high: The value at the high end of the colormap.

        Returns:
            These axes.

        Raises:
            TypeError: If a limit is not a number.
            StyleError: If a limit is not finite, ``low`` is not below ``high``, a limit is
                not positive on a log color scale, or the range does not hold the center.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().clim(0, 100)
            >>> ax = ax.scatter([1, 2], [3.0, 4.0], color=ub.ColorBy([20.0, 140.0]))
        """
        with api_call():
            context = "Axes.clim()"
            lo = real(context, "low", low)
            hi = real(context, "high", high)
            self._native.clim(self._row, self._col, lo, hi)
            return self

    def colorbar(
        self,
        label: str = "",
        *,
        position: ColorbarPosition | Auto = AUTO,
        format: TickFormat | Auto = AUTO,
    ) -> Self:
        """Set the colorbar label, position and tick format.

        A colorbar is drawn automatically for finite mapped values. ``RIGHT`` places it
        beside the plot, before a right legend. ``BOTTOM`` places it below the x axis and
        any bottom legend. The label sits above the bar. Each call replaces all settings.

        Args:
            label: The horizontal label above the colorbar; empty draws no label.
            position: ``RIGHT``, ``BOTTOM`` or ``NONE``. ``AUTO`` follows the theme and
                clears a previous override.
            format: The tick format. ``AUTO`` follows the y tick format and clears a
                previous override.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            StyleError: If the label exceeds 1,000 characters or holds a lone surrogate.
            UsageError: If no color-mapped mark has been added yet.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().scatter([1, 2], [2, 3], color=ub.ColorBy([4, 8]))
            >>> ax = ax.colorbar("Intensity", position=ub.ColorbarPosition.BOTTOM)
        """
        with api_call():
            context = "Axes.colorbar()"
            _args.require_text(context, "label", label)
            placement = (
                None
                if position is AUTO
                else instance(
                    context,
                    "position",
                    position,
                    ColorbarPosition,
                    _members_hint(ColorbarPosition) + " Use ub.AUTO for the theme's position.",
                ).value
            )
            checked = (
                None
                if format is AUTO
                else instance(
                    context,
                    "format",
                    format,
                    TickFormat,
                    _FORMAT_HINT,
                )
            )
            name = _args.text(context, "label", label)
            self._native.colorbar(
                self._row,
                self._col,
                name,
                placement,
                None if checked is None else _ticks.native(checked),
            )
            return self

    def heatmap(
        self: "Axes[str, str]",
        x_categories: Values[str],
        y_categories: Values[str],
        values: object,
        *,
        cmap: Cmap | Auto = AUTO,
        scale: Scale[float] = Scale.linear(),  # noqa: B008
        center: float | Auto = AUTO,
        annotate: bool = False,
        format: TickFormat | Auto = AUTO,
    ) -> "Axes[str, str]":
        """Add a heatmap: one cell per pair of an x and a y category, colored by its value.

        The x categories run left to right and the y categories top to bottom, and the cells
        fill their category bands exactly, with no gridlines over them. A NaN value leaves its
        cell empty. The colors come from the color scale of these axes, which every
        color-mapped mark shares; ``clim()`` sets its range, and the colorbar explains it.

        Args:
            x_categories: The column names, left to right, each at most once.
            y_categories: The row names, top to bottom, each at most once.
            values: The values as a two-dimensional array or a list of rows, shaped
                ``(len(y_categories), len(x_categories))``: one row per y category.
            cmap: The colormap. ``AUTO`` uses the theme's diverging colormap when ``center``
                is given and its sequential colormap otherwise.
            scale: ``Scale.linear()``, or ``Scale.log()`` for values that are all positive.
            center: The value at the middle of a diverging colormap, as for ``ColorBy``;
                ``AUTO`` for a sequential mapping.
            annotate: Whether each cell shows its value at its center, in the tick font, in
                black or white, whichever contrasts more with the cell.
            format: The format of the values shown by ``annotate``. ``AUTO`` follows the y
                tick format, which writes each value with the decimals of the colorbar's tick
                step, or with up to 12 significant digits on a log scale.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type, or ``scale`` is not linear or log.
            DataError: If ``values`` is not shaped ``(len(y_categories), len(x_categories))``,
                a category repeats, or a value is infinite, beyond 1e300 in magnitude, or not
                positive on a log scale.
            StyleError: If a category name is too long, ``center`` is not finite or comes
                with a log scale, ``format`` is ``TickFormat.iso()``, or the color range set by
                ``clim()`` does not suit ``scale`` or ``center``.
            UsageError: If either axis is not a category axis (type checkers refuse such
                axes), ``format`` is given without ``annotate``, or ``cmap``, ``scale`` or
                ``center`` differs from those of the other color-mapped marks of these axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
            >>> days, periods = ["Mon", "Tue", "Wed"], ["Day", "Night"]
            >>> ax = ax.heatmap(days, periods, [[12, 18, 15], [4, 7, 6]], annotate=True)
        """
        with api_call():
            context = "Axes.heatmap()"
            check_names_type(context, "x_categories", x_categories)
            check_names_type(context, "y_categories", y_categories)
            native_cmap, kind, base, middle = native_color_scale(context, cmap, scale, center)
            shows = _args.boolean(context, "annotate", annotate)
            checked = (
                None
                if format is AUTO
                else _ticks.native(instance(context, "format", format, TickFormat, _FORMAT_HINT))
            )
            if (self._x, self._y) != (_CATEGORY, _CATEGORY):
                # The core refuses the axes before it reads the data, with its own error.
                empty = np.empty(0, dtype=np.float64)
                self._native.heatmap(
                    self._row,
                    self._col,
                    [],
                    [],
                    empty,
                    native_cmap,
                    kind,
                    base,
                    middle,
                    shows,
                    checked,
                )
            x_names = names(context, "x_categories", x_categories)
            y_names = names(context, "y_categories", y_categories)
            cells = matrix(context, "values", values, (len(y_names), len(x_names)))
            self._native.heatmap(
                self._row,
                self._col,
                x_names,
                y_names,
                cells,
                native_cmap,
                kind,
                base,
                middle,
                shows,
                checked,
            )
            return self


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
    if is_text_or_buffer(value) or not isinstance(value, Sequence):
        message = f"series is {type_name(value)}; series must be a sequence of Series."
        raise new_error(TypeError, context, message, parameter="series", hint=_SERIES_HINT)
    items = cast(Sequence[object], value)
    arguments: list[tuple[str, FloatArray, tuple[int, int, float]]] = []
    for index, item in enumerate(items):
        if not isinstance(item, Series):
            label = f"series[{index}]"
            message = f"{label} is {type_name(item)}; {label} must be of type Series."
            raise new_error(TypeError, context, message, parameter="series", hint=_SERIES_ITEM_HINT)
        arguments.append(_series.native(item))
    return arguments


def _distribution_arguments(
    context: str,
    value: object,
) -> list[tuple[str, list[FloatArray], tuple[int, int, float]]]:
    """Check distributions and return their native arguments without copying samples."""
    hint = 'pass a list of distributions, e.g. [Distribution("North", [[1, 2]])].'
    if is_text_or_buffer(value) or not isinstance(value, Sequence):
        raise new_error(
            TypeError,
            context,
            f"distributions is {type_name(value)}; "
            "distributions must be a sequence of Distribution.",
            parameter="distributions",
            hint=hint,
        )
    arguments: list[tuple[str, list[FloatArray], tuple[int, int, float]]] = []
    for index, item in enumerate(cast(Sequence[object], value)):
        if not isinstance(item, Distribution):
            name = f"distributions[{index}]"
            raise new_error(
                TypeError,
                context,
                f"{name} is {type_name(item)}; {name} must be of type Distribution.",
                parameter="distributions",
                hint=hint,
            )
        arguments.append(_series.native_distribution(item))
    return arguments


def _cell_colors(
    context: str, cmap: object, scale: object, center: object
) -> tuple[_umber.Cmap | None, int, float, float | None]:
    """Check cell color options and convert them without constructing a second mapping."""
    colormap = None if cmap is AUTO else instance(context, "cmap", cmap, Cmap, CMAP_HINT)
    instance(context, "scale", scale, Scale, "use Scale.linear() or Scale.log().")
    transform = cast(Scale[Any], scale)
    if transform in (_CATEGORY, _TIME):
        raise new_error(
            TypeError,
            context,
            f"scale is {transform!r}; scale must be Scale.linear() or Scale.log().",
            parameter="scale",
            hint="use Scale.linear() or Scale.log().",
        )
    middle = None if center is AUTO else real(context, "center", center)
    return None if colormap is None else _cmap.native(colormap), *_scale.native(transform), middle


def _numeric_cells(context: str, axes: Axes[Any, Any], *, time: bool) -> None:
    """Reject category axes, and time axes unless ``time`` allows them, before converting cells."""
    for name, scale in zip(("x", "y"), scales(axes), strict=True):
        if scale == _CATEGORY or (scale == _TIME and not time):
            allowed = "a numeric or time axis" if time else "a numeric axis"
            hint = (
                "use Scale.linear(), Scale.log() or Scale.time() for both axes."
                if time
                else "use Scale.linear() or Scale.log() for both axes."
            )
            raise new_error(
                UsageError,
                context,
                f"{name} must be {allowed}.",
                parameter=name,
                hint=hint,
            )


def _cell_count(context: str, name: str, value: object) -> int:
    """Check a count before converting it to a native unsigned integer."""
    count = integer(context, name, value)
    if not 1 <= count <= umber.MAX_BINS:
        raise new_error(
            StyleError,
            context,
            f"{name} is {count}; it must be between 1 and {umber.MAX_BINS}.",
            parameter=name,
            hint="choose a positive count no greater than MAX_BINS.",
        )
    return count


def _rect_counts(context: str, bins: object) -> tuple[int, int]:
    """Normalize one count or a pair of counts."""
    if isinstance(bins, tuple):
        pair = cast(tuple[object, ...], bins)
        if len(pair) != 2:
            raise new_error(
                TypeError,
                context,
                "bins must be an int or a pair of ints.",
                parameter="bins",
                hint="pass bins=30 or bins=(30, 20).",
            )
        return _cell_count(context, "bins", pair[0]), _cell_count(context, "bins", pair[1])
    count = _cell_count(context, "bins", bins)
    return count, count


def _matrix(context: str, values: object) -> tuple[FloatArray, int, int]:
    """Flatten a numeric matrix without coercing bool or text elements into numbers."""
    hint = "pass a non-empty two-dimensional matrix of numbers; use NaN for blank cells."
    if isinstance(values, np.ma.MaskedArray):
        raise new_error(
            DataError,
            context,
            "values is a masked array; masked values cannot be plotted.",
            parameter="values",
            hint=hint,
        )
    try:
        # Object dtype preserves the element types of nested Python sequences.
        array = np.asarray(values, dtype=None if hasattr(values, "__array__") else object)
    except (TypeError, ValueError):
        raise new_error(
            DataError,
            context,
            "values must be a rectangular two-dimensional matrix.",
            parameter="values",
            hint=hint,
        ) from None
    if array.ndim != 2 or 0 in array.shape:
        raise new_error(
            DataError,
            context,
            "values must be a non-empty two-dimensional matrix.",
            parameter="values",
            hint=hint,
        )
    rows, columns = array.shape
    try:
        flattened = floats(context, "values", array.reshape(-1))
    except DataError as error:
        prefix = f"{context}: values["
        index, closing, rest = str(error).removeprefix(prefix).partition("]")
        if str(error).startswith(prefix) and closing and index.isdecimal():
            row, column = divmod(int(index), columns)
            error.args = (f"{context}: values[{row}, {column}]{rest}",)
        raise
    return flattened, rows, columns


_SCALES: Final = attrgetter("_x", "_y")


def _scale_values(
    context: str, name: str, values: object, scale: Scale[Any]
) -> FloatArray | list[str]:
    """Convert the values ``name`` of an axis of ``scale``, as ``axis_values()`` does.

    Empty data converts to an empty array of numbers, which a category axis would refuse; on
    a category axis it is an empty list of names instead.
    """
    converted = axis_values(context, name, values)
    if isinstance(converted, np.ndarray) and converted.size == 0 and scale == _CATEGORY:
        return []
    return converted


def _check_coordinates_type(context: str, name: str, values: object, scale: Scale[Any]) -> None:
    """Check the type of values along an axis of ``scale``: numbers, or times on a time axis."""
    if scale == _TIME:
        check_times_type(context, name, values)
    else:
        check_floats_type(context, name, values)


def _limit_values(
    context: str, name: str, values: object, scale: Scale[Any]
) -> FloatArray | MicrosArray:
    """Convert limits, edges or coordinates along an axis of ``scale``: numbers, or times."""
    if scale == _TIME:
        return _time.micros(context, name, values)
    return floats(context, name, values)


def attach(
    figure: "umber.Figure",
    native: _umber.Figure,
    x: Scale[X],
    y: Scale[Y],
    row: int,
    col: int,
) -> Axes[X, Y]:
    """Return new axes of ``figure`` with the scales ``x`` and ``y``.

    The native figure must already hold axes with these scales whose top-left cell is at
    ``row``, ``col``; this builds the Python side only, because ``Axes`` has no public
    constructor.

    Args:
        figure: The figure the axes belong to.
        native: The native figure ``figure`` owns.
        x: The x scale.
        y: The y scale.
        row: The row of the axes' top-left cell.
        col: The column of the axes' top-left cell.

    Returns:
        The axes.

    Example:
        >>> import umber as ub
        >>> fig = ub.Figure()
        >>> ax = fig.add_axes()  # attaches the axes once the native figure holds them
        >>> ax.figure is fig
        True
    """
    axes = cast(Axes[X, Y], object.__new__(Axes))
    object.__setattr__(axes, "_figure", figure)
    object.__setattr__(axes, "_native", native)
    object.__setattr__(axes, "_row", row)
    object.__setattr__(axes, "_col", col)
    object.__setattr__(axes, "_x", x)
    object.__setattr__(axes, "_y", y)
    return axes


_CELL: Final = attrgetter("_row", "_col")


def cell(axes: Axes[Any, Any]) -> tuple[int, int]:
    """Return the row and column of the top-left cell of ``axes``.

    Args:
        axes: The axes.

    Returns:
        The row and the column.

    Example:
        >>> import umber as ub
        >>> from umber._axes import cell
        >>> cell(ub.Figure().add_axes())
        (0, 0)
    """
    return cast(tuple[int, int], _CELL(axes))


def scales(axes: Axes[Any, Any]) -> tuple[Scale[Any], Scale[Any]]:
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
    return cast(tuple[Scale[Any], Scale[Any]], _SCALES(axes))
