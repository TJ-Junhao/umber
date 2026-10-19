"""``Axes``: the plot area of a figure, with its marks, texts, limits and legend."""

import numbers
from collections.abc import Callable, Sequence
from enum import Enum
from functools import partial
from operator import attrgetter
from typing import Any, Final, Generic, Never, Self, cast, final

import numpy as np
from typing_extensions import TypeVar

# The package, for the forward references to ``umber.Figure``: ``_figure`` imports this module,
# so ``Figure`` cannot be imported from it here, but ``typing.get_type_hints`` resolves
# ``umber.Figure`` through the package once it has loaded.
import umber
from umber import _args, _cmap, _color, _group, _scale, _series, _size, _ticks, _time, _umber
from umber._args import instance, integer, is_text_or_buffer, real, type_name
from umber._cmap import CMAP_HINT, Cmap
from umber._color import Color
from umber._color_by import ColorBy, native_color_scale, native_point_color
from umber._enums import (
    AUTO,
    Align,
    Auto,
    ColorbarPosition,
    Contour,
    Dash,
    Hist,
    LegendPosition,
    LegendStyle,
    Marker,
    Render,
    Stat,
    Step,
)
from umber._errors import DataError, StyleError, UmberError, UsageError, details, new_error
from umber._report import api_call
from umber._scale import Scale
from umber._series import Distribution, Series
from umber._size import TEXT_SIZE_HINT as _SIZE_HINT
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
_HIST_HINT: Final = _members_hint(Hist)
_STAT_HINT: Final = _members_hint(Stat)
_BANDWIDTH_HINT: Final = "pass a bandwidth in data units greater than 0, or ub.AUTO."
_MARK_SIZE_HINT: Final = (
    "use Size.times(1.5) for a multiple of the theme's size, Size.pt(2.0) for points, or Size.AUTO."
)
_LEGEND_HINT: Final = _members_hint(LegendStyle)
_LEGEND_POSITION_HINT: Final = (
    _members_hint(LegendPosition) + " Use ub.AUTO for the theme's position."
)
_RENDER_HINT: Final = (
    "use ub.Render.VECTOR or ub.Render.RASTER, or ub.AUTO to choose by the number of cells."
)
_CONTOUR_HINT: Final = _members_hint(Contour)
_FORMAT_HINT: Final = (
    "use TickFormat.fixed(2), TickFormat.percent(), TickFormat.scientific() or TickFormat.AUTO."
)
_ALIGN_HINT: Final = _members_hint(Align)
_AT_HINT: Final = "pass a tuple (x, y) of values of the axes, such as (2.0, 7.5)."
_OFFSET_HINT: Final = "pass a tuple (dx, dy) of points, such as (0.0, 12.0)."


@final
# PEP 695 syntax on Python 3.12 cannot give type parameters defaults, so ``Axes``
# could not mean ``Axes[float, float]``.
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
    __slots__ = ("_col", "_figure", "_native", "_register_twin", "_row", "_target", "_x", "_y")
    _register_twin: "Callable[[_umber.TwinAxes, Scale[X], Scale[float]], Axes[X, float]]"
    _figure: "umber.Figure"
    _native: _umber.Figure
    _row: int
    _target: int | _umber.TwinAxes
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

    def twin_y(self: "Axes[X, float]", *, scale: Scale[float] = Scale.linear()) -> "Axes[X, float]":
        """Add a numeric y axis on the right, sharing this panel's x axis.

        The two axes have independent y scales, limits, ticks and labels. Their x settings
        are shared in both directions. Marks use one palette and legend; grid lines and the
        baseline come from the left axis. Retrieve the right axes again with
        ``Figure.twin_axes_at()``.

        Args:
            scale: The right y scale, ``Scale.linear()`` or ``Scale.log()``.

        Returns:
            New axes in this panel, with the same x value type and numeric y values.

        Raises:
            TypeError: If ``scale`` is not a ``Scale``.
            UsageError: If either y scale is not numeric, these axes already have a twin,
                these axes are a twin, or the grid shares x.

        Example:
            >>> import umber as ub
            >>> fig = ub.Figure()
            >>> left = fig.add_axes(x=ub.Scale.category())
            >>> _ = left.bar(["Jan", "Feb"], [40, 55], label="Rainfall")
            >>> right = left.twin_y()
            >>> _ = right.line(["Jan", "Feb"], [7, 9], label="Temperature")
            >>> right.figure is fig
            True
        """
        with api_call():
            context = "Axes.twin_y()"
            instance(context, "scale", scale, Scale, "pass Scale.linear() or Scale.log().")
            target = self._native.twin_y(self._target, self._col, *_scale.native(scale))
            return self._register_twin(target, self._x, scale)

    def title(self, text: str, *, size: Size = Size.AUTO) -> Self:
        """Set the title, shown above the plot.

        Args:
            text: The title, at most 1,000 characters. An empty string removes it.
            size: ``Size.AUTO`` uses the theme's size, ``Size.times(k)`` multiplies it
                after font scaling, and ``Size.pt(p)`` uses points without font scaling.

        Returns:
            These axes.

        Raises:
            TypeError: If ``text`` is not a ``str`` or ``size`` is not a ``Size``.
            StyleError: If ``text`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().title("Revenue")
        """
        with api_call():
            context = "Axes.title()"
            _args.require_text(context, "text", text)
            instance(context, "size", size, Size, _SIZE_HINT)
            self._native.title(
                self._target, self._col, _args.text(context, "text", text), _size.native(size)
            )
            return self

    def subtitle(self, text: str, *, size: Size = Size.AUTO) -> Self:
        """Set the subtitle, shown under the title.

        Args:
            text: The subtitle, at most 1,000 characters. An empty string removes it.
            size: ``Size.AUTO`` uses the theme's size, ``Size.times(k)`` multiplies it
                after font scaling, and ``Size.pt(p)`` uses points without font scaling.

        Returns:
            These axes.

        Raises:
            TypeError: If ``text`` is not a ``str`` or ``size`` is not a ``Size``.
            StyleError: If ``text`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().title("Revenue").subtitle("Per quarter")
        """
        with api_call():
            context = "Axes.subtitle()"
            _args.require_text(context, "text", text)
            instance(context, "size", size, Size, _SIZE_HINT)
            self._native.subtitle(
                self._target, self._col, _args.text(context, "text", text), _size.native(size)
            )
            return self

    def xlabel(self, text: str, *, size: Size = Size.AUTO) -> Self:
        """Set the x axis label, shown under the x tick labels.

        Args:
            text: The label, at most 1,000 characters. An empty string removes it.
            size: ``Size.AUTO`` uses the theme's size, ``Size.times(k)`` multiplies it
                after font scaling, and ``Size.pt(p)`` uses points without font scaling.

        Returns:
            These axes.

        Raises:
            TypeError: If ``text`` is not a ``str`` or ``size`` is not a ``Size``.
            StyleError: If ``text`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().xlabel("Year")
        """
        with api_call():
            context = "Axes.xlabel()"
            _args.require_text(context, "text", text)
            instance(context, "size", size, Size, _SIZE_HINT)
            self._native.xlabel(
                self._target, self._col, _args.text(context, "text", text), _size.native(size)
            )
            return self

    def ylabel(self, text: str, *, size: Size = Size.AUTO) -> Self:
        """Set the y axis label, shown left of the y tick labels.

        Args:
            text: The label, at most 1,000 characters. An empty string removes it.
            size: ``Size.AUTO`` uses the theme's size, ``Size.times(k)`` multiplies it
                after font scaling, and ``Size.pt(p)`` uses points without font scaling.

        Returns:
            These axes.

        Raises:
            TypeError: If ``text`` is not a ``str`` or ``size`` is not a ``Size``.
            StyleError: If ``text`` is too long.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().ylabel("Revenue")
        """
        with api_call():
            context = "Axes.ylabel()"
            _args.require_text(context, "text", text)
            instance(context, "size", size, Size, _SIZE_HINT)
            self._native.ylabel(
                self._target, self._col, _args.text(context, "text", text), _size.native(size)
            )
            return self

    def xlim(self: SX, low: X, high: X) -> SX:
        """Set the x limits, replacing the automatic ones; for linear, log and time x axes.

        Automatic linear limits follow the data, with 5% of its span added on each side;
        an end at zero stays there for bars, histograms, areas and stems. Ticks are round
        values inside the limits, not the limits themselves. Lines keep their exact x
        extent, cells keep their edges, and bubbles fit their whole markers. Single values
        expand to a usable range. Explicit limits are used as given.

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
                self._native.xlim_time(self._target, self._col, start, end)
                return self
            lower = real(context, "low", low)
            upper = real(context, "high", high)
            self._native.xlim(self._target, self._col, lower, upper)
            return self

    def ylim(self: SY, low: Y, high: Y) -> SY:
        """Set the y limits, replacing the automatic ones.

        Automatic linear limits follow the data, with 5% of its span added on each side;
        an end at zero stays there for bars, histograms, areas and stems. Ticks are round
        values inside the limits, not the limits themselves. Cells keep their edges, and
        bubbles fit their whole markers. Single values expand to a usable range.
        Explicit limits are used as given.

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
                self._native.ylim_time(self._target, self._col, start, end)
                return self
            lower = real(context, "low", low)
            upper = real(context, "high", high)
            self._native.ylim(self._target, self._col, lower, upper)
            return self

    def xticks(self: SX, positions: Values[X]) -> SX:
        """Set the x ticks to ``positions``, replacing the automatic ones.

        Automatic linear ticks are round values inside the limits, at whole multiples of
        a 1, 2, 2.5 or 5 times a power of ten step, including zero when it lies inside.
        Limits follow the data with 5% padding, keeping an end at zero for bars, histograms,
        areas and stems; lines and cells keep their exact position extents. The tick count
        depends on available space and label size. Explicit positions need not follow that grid.

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
                self._native.xticks_time(self._target, self._col, places)
                return self
            self._native.xticks(self._target, self._col, floats(context, "positions", positions))
            return self

    def yticks(self: SY, positions: Values[Y]) -> SY:
        """Set the y ticks to ``positions``, replacing the automatic ones.

        Automatic linear ticks are round values inside the limits, at whole multiples of
        a 1, 2, 2.5 or 5 times a power of ten step, including zero when it lies inside.
        Limits follow the data with 5% padding, keeping an end at zero for bars, histograms,
        areas and stems; lines and cells keep their exact position extents. The tick count
        depends on available space and label size. Explicit positions need not follow that grid.

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
                self._native.yticks_time(self._target, self._col, places)
                return self
            self._native.yticks(self._target, self._col, floats(context, "positions", positions))
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
                self._native.xticks_labeled_time(self._target, self._col, times, texts)
                return self
            places = floats(context, "positions", positions)
            texts = names(context, "labels", labels, check_length=False)
            self._native.xticks_labeled(self._target, self._col, places, texts)
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
                self._native.yticks_labeled_time(self._target, self._col, times, texts)
                return self
            places = floats(context, "positions", positions)
            texts = names(context, "labels", labels, check_length=False)
            self._native.yticks_labeled(self._target, self._col, places, texts)
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
            self._native.xtick_format(self._target, self._col, _ticks.native(checked))
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
            self._native.ytick_format(self._target, self._col, _ticks.native(checked))
            return self

    def baseline(self, *, shown: bool | Auto) -> Self:
        """Set whether these axes draw their baseline.

        Each call replaces the setting for these axes alone; panels sharing an axis do not
        inherit it. Copies of the figure keep the setting. Only drawing changes: the baseline
        stays at zero on the value axis, or at its lower edge when zero is out of range. Limits,
        padding, bar bases, area fills and reference-line clipping stay unchanged. A gridline
        at the baseline tick is kept when the baseline is hidden. In a panel with a twin y
        axis, only the left axes draw a baseline. The twin accepts ``AUTO`` and ``False``;
        ``True`` raises ``UsageError`` instead of being ignored.

        Args:
            shown: ``True`` draws the baseline; ``False`` hides it. ``AUTO`` restores the
                default: draw it when the axes hold a bar, grouped bar, stacked bar, histogram,
                area, stacked area or stem. Other marks do not suppress it.

        Returns:
            These axes.

        Raises:
            TypeError: If ``shown`` is neither a bool nor ``AUTO``. Integers are not accepted.
            UsageError: If ``shown=True`` on two category axes, which have no value axis, or
                on a twin y axes; set the baseline on the left axes instead.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1], [1, 2])
            >>> ax = ax.baseline(shown=True)
            >>> ax = ax.baseline(shown=False)
            >>> ax = ax.baseline(shown=ub.AUTO)
        """
        with api_call():
            flag = (
                None
                if shown is AUTO
                else _args.boolean(
                    "Axes.baseline()",
                    "shown",
                    shown,
                    expected="a bool or ub.AUTO",
                    hint="pass True or False, or ub.AUTO to follow the marks.",
                )
            )
            self._native.baseline(self._target, self._col, flag)
            return self

    def legend(
        self, style: LegendStyle, *, position: LegendPosition | Auto = AUTO, size: Size = Size.AUTO
    ) -> Self:
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

            size: ``Size.times(k)`` multiplies the theme's scaled size; ``Size.pt(p)``
                uses points without font scaling. In a merged legend, only these explicit
                sizes must agree. ``Size.AUTO`` clears this panel's size setting and inherits
                the agreed explicit size, or the theme's size when none is set. Panels with
                ``LegendStyle.NONE`` do not take part in the agreement.

        Returns:
            These axes.

        Raises:
            TypeError: If ``style`` is not a ``LegendStyle``, or ``position`` is neither a
                ``LegendPosition`` nor ``AUTO``, or ``size`` is not a ``Size``.
            UsageError: If ``style`` is ``DIRECT`` and a ``LegendPosition`` is supplied,
                or another panel has set a different size for the merged legend.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1], [1, 2], label="North")
            >>> ax = ax.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT)
            >>> ax = ax.legend(ub.LegendStyle.DIRECT)
        """
        with api_call():
            member = instance("Axes.legend()", "style", style, LegendStyle, _LEGEND_HINT)
            if position is not AUTO:
                instance(
                    "Axes.legend()", "position", position, LegendPosition, _LEGEND_POSITION_HINT
                )
            instance("Axes.legend()", "size", size, Size, _SIZE_HINT)
            if isinstance(position, LegendPosition):
                self._native.legend_at(
                    self._target, self._col, member.value, position.value, _size.native(size)
                )
            else:
                self._native.legend(self._target, self._col, member.value, _size.native(size))
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
        by: Values[str] | Auto = AUTO,
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
            by: One group name per data row, in first-appearance order; ``AUTO`` draws
                one series. Names supply legend labels and palette colors; do not pass
                a non-empty ``label`` or an explicit ``color``. Missing or empty names
                and an empty grouping column are refused. Column names are never read.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``by`` has invalid names or differs in length from the data; or if
                the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, or names and numbers do not match the x
                axis.
            StyleError: If ``label`` is too long.
            UsageError: If ``by`` comes with an explicit color or a non-empty label; or if
                the y axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.line([0, 1, 2, 3], [1.0, 3.0, float("nan"), 2.0], label="North")
            >>> grouped = ub.Figure().add_axes()
            >>> grouped = grouped.line([0, 0, 1, 1], [1, 2, 3, 4], by=["B", "A", "B", "A"])
        """
        with api_call():
            context = "Axes.line()"
            check_axis_type(context, "x", x)
            check_floats_type(context, "y", y)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            style = instance(context, "dash", dash, Dash, _DASH_HINT)
            extent = _size_argument(context, "width", width)
            _group.call(
                context,
                by,
                label,
                color,
                _group.XY(
                    x=partial(self._x_values, context, x),
                    y=partial(self._y_values, context, y),
                    draw=partial(
                        self._native.line,
                        self._target,
                        self._col,
                        color=spec,
                        dash=style.value,
                        width=extent,
                    ),
                    draw_groups=partial(
                        self._native.grouped_line,
                        self._target,
                        self._col,
                        dash=style.value,
                        width=extent,
                    ),
                ),
            )
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
        by: Values[str] | Auto = AUTO,
    ) -> SY:
        """Add a line through the points ``(x[i], y[i])``, in order, and fill the region below it.

        The fill reaches down to the baseline of the axes. The baseline is drawn at zero, or
        at the lower edge when zero is out of range, even alongside other marks. A NaN in ``x``
        or ``y`` leaves a gap in the line and in the fill. The theme's fill style sets how the
        fill looks.

        Args:
            x: The x values: numbers on a linear or log x axis, names on a category x axis.
            y: The y values, one per x value.
            label: The name shown in the legend; an empty string leaves the area out of it.
            color: The area color. ``Color.AUTO`` takes the next color of the theme palette.
            dash: How the line is stroked.
            width: The line width. ``Size.AUTO`` is the line width of the theme.
            by: One group name per data row, in first-appearance order; ``AUTO`` draws
                one series. Names supply legend labels and palette colors; do not pass
                a non-empty ``label`` or an explicit ``color``. Missing or empty names
                and an empty grouping column are refused. Column names are never read.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``by`` has invalid names or differs in length from the data; or if
                the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, or names and numbers do not match the x
                axis.
            StyleError: If ``label`` is too long.
            UsageError: If ``by`` comes with an explicit color or a non-empty label; or if
                the y axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.area([0, 1, 2, 3], [1.0, 3.0, 2.0, 4.0], label="Visitors")
            >>> grouped = ub.Figure().add_axes()
            >>> grouped = grouped.area([0, 0, 1, 1], [1, 2, 3, 4], by=["B", "A", "B", "A"])
        """
        with api_call():
            context = "Axes.area()"
            check_axis_type(context, "x", x)
            check_floats_type(context, "y", y)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            style = instance(context, "dash", dash, Dash, _DASH_HINT)
            extent = _size_argument(context, "width", width)
            _group.call(
                context,
                by,
                label,
                color,
                _group.XY(
                    x=partial(self._x_values, context, x),
                    y=partial(self._y_values, context, y),
                    draw=partial(
                        self._native.area,
                        self._target,
                        self._col,
                        color=spec,
                        dash=style.value,
                        width=extent,
                    ),
                    draw_groups=partial(
                        self._native.grouped_area,
                        self._target,
                        self._col,
                        dash=style.value,
                        width=extent,
                    ),
                ),
            )
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
        by: Values[str] | Auto = AUTO,
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
            by: One group name per data row, in first-appearance order; ``AUTO`` draws
                one series. Names supply legend labels and palette colors; do not pass
                a non-empty ``label`` or an explicit ``color``. Missing or empty names
                and an empty grouping column are refused. Column names are never read.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``by`` has invalid names or differs in length from the data; or if
                the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, names and numbers do not match the
                axes, or a ``ColorBy`` does not have one value per point.
            StyleError: If ``label`` is too long, or a ``ColorBy`` does not suit the color
                range set by ``clim()``.
            UsageError: If ``by`` comes with an explicit color or a non-empty label; or if
                both axes are category axes (type checkers accept such axes, so
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
            >>> grouped = ub.scatter([0, 0, 1, 1], [1, 2, 3, 4], by=["B", "A", "B", "A"])
        """
        with api_call():
            context = "Axes.scatter()"
            check_axis_type(context, "x", x)
            check_axis_type(context, "y", y)
            _args.require_text(context, "label", label)
            spec = native_point_color(context, color)
            shape = instance(context, "marker", marker, Marker, _MARKER_HINT)
            extent = _size_argument(context, "size", size)
            _group.call(
                context,
                by,
                label,
                color,
                _group.XY(
                    x=partial(self._x_values, context, x),
                    y=partial(self._y_values, context, y),
                    draw=partial(
                        self._native.scatter,
                        self._target,
                        self._col,
                        color=spec,
                        marker=shape.value,
                        size=extent,
                    ),
                    draw_groups=partial(
                        self._native.grouped_scatter,
                        self._target,
                        self._col,
                        marker=shape.value,
                        size=extent,
                    ),
                ),
            )
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
        time axis, before padding (and rounding on a log axis). Shared axes fit bubbles from
        every panel sharing the scale. When the bubbles cannot all fit on an automatic axis
        (a marker, outline included, at least
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
            self._native.bubbles(self._target, self._col, xs, ys, values, name, spec, shape.value)
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
        The baseline is drawn at zero, or at the lower edge when zero is out of range,
        even alongside other marks.
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
            self._native.stem(self._target, self._col, xs, ys, name, spec, shape.value, extent)
            return self

    def bar(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        heights: Values[float],
        *,
        label: str = "",
        color: Color = Color.AUTO,
        annotate: bool = False,
        format: TickFormat | Auto = AUTO,
        size: Size = Size.AUTO,
        by: Values[str] | Auto = AUTO,
    ) -> "Axes[X, Y]":
        """Add one bar per category, from the baseline to its height, along the category axis.

        On a category x axis the bars stand on the baseline and a negative height draws below
        it. On a category y axis the bars are horizontal: the categories run top to bottom in
        order of first appearance, and the bars grow rightwards from the baseline, leftwards for
        a negative height. A NaN height leaves its category without a bar. The baseline is
        drawn at zero on the value axis, or at its lower edge when zero is out of range,
        even alongside other marks.

        Args:
            categories: The category names, each at most once per group.
            heights: The bar heights, one per category.
            label: The name shown in the legend; an empty string leaves the bars out of it.
            color: The bar color. ``Color.AUTO`` takes the next color of the theme palette.
            annotate: Whether each bar shows its value just beyond its end, 3 pt away, in
                the theme's text color at the tick label size. Automatic limits grow, only
                when a label does not fit in them, so that every label fits inside the plot.
                With ``by``, every group's bars show their values, as in ``grouped_bar()``.
            format: The format of the values shown by ``annotate``. ``AUTO`` follows the tick
                format of the value axis. ``TickFormat.AUTO``, also the axis's default, writes
                each value on its own to six significant digits in the shortest form, like
                ``%g``: ``42``, ``28.4``, ``0.12345``, in scientific form outside 1e-4 to 1e6.
            size: The size of the values shown by ``annotate``. ``Size.AUTO`` is the tick
                label size, ``Size.times(k)`` multiplies it after font scaling, and
                ``Size.pt(p)`` uses points without font scaling.
            by: One group name per data row, in first-appearance order; ``AUTO`` draws
                one series. Names supply legend labels and palette colors; do not pass
                a non-empty ``label`` or an explicit ``color``. Missing or empty names
                and an empty grouping column are refused. Column names are never read.
                Categories may repeat across groups, but a (category, group) pair may
                not repeat. Missing pairs draw no bar.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``by`` has invalid names or differs in length from the data; or if
                the data cannot be plotted: lengths differ, a category repeats, a
                height is infinite or not positive on a log axis, or neither axis is a
                category axis.
            StyleError: If ``label`` or a category name is too long, or ``format`` is
                ``TickFormat.iso()``. Rendering raises it when a value label does not fit
                inside the plot: under explicit limits, or on a plot too small for it.
            UsageError: If ``by`` comes with an explicit color or a non-empty label, both axes
                are category axes, or ``format`` or ``size`` is given without ``annotate``.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.bar(["North", "South", "East"], [3, 5, 2], label="2025")
            >>> rows = ub.Figure().add_axes(y=ub.Scale.category())
            >>> rows = rows.bar(["North", "South", "East"], [3.5, 5.0, 2.25], annotate=True)
            >>> grouped = ub.bar(["Q1", "Q1", "Q2"], [3, 4, 5], by=["North", "South", "North"])
        """
        with api_call():
            context = "Axes.bar()"
            check_names_type(context, "categories", categories)
            check_floats_type(context, "heights", heights)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            shows, labelled, label_size = _value_labels(context, annotate, format, size)
            _group.call(
                context,
                by,
                label,
                color,
                _group.Bars(
                    categories=partial(names, context, "categories", categories),
                    heights=partial(floats, context, "heights", heights),
                    draw=partial(
                        self._native.bar,
                        self._target,
                        self._col,
                        color=spec,
                        annotate=shows,
                        format=labelled,
                        size=label_size,
                    ),
                    draw_groups=partial(
                        self._native.bar_by,
                        self._target,
                        self._col,
                        annotate=shows,
                        format=labelled,
                        size=label_size,
                    ),
                ),
            )
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
            self._native.box_plot(self._target, self._col, category_names, values, name, spec)
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
            self._native.grouped_box_plot(self._target, self._col, category_names, arguments)
            # The self type only restricts the call: these are the Axes[X, Y] it was made on.
            return cast("Axes[X, Y]", self)

    def grouped_bar(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        series: Sequence[Series],
        *,
        annotate: bool = False,
        format: TickFormat | Auto = AUTO,
        size: Size = Size.AUTO,
    ) -> "Axes[X, Y]":
        """Add grouped bars: in each category, one bar per series, side by side.

        The bars grow and draw a baseline as in ``bar()``, even alongside other marks.
        Within a category they follow the order of the series,
        which is the order of the legend: left to right, or top to bottom on a category y axis.

        Args:
            categories: The category names, each at most once.
            series: One or more ``Series``, each with one value per category. The series
                take their places in the group in this order.
            annotate: Whether each bar shows its value just beyond its end, as in ``bar()``.
            format: The format of the values shown by ``annotate``, as in ``bar()``.
            size: The size of the values shown by ``annotate``, as in ``bar()``.

        Returns:
            These axes.

        Raises:
            TypeError: If ``series`` is not a sequence of ``Series``, or another argument has
                the wrong type.
            DataError: If the data cannot be plotted: ``series`` is empty, a series has a
                different length, a category repeats, a height is infinite or not positive on
                a log axis, or neither axis is a category axis.
            StyleError: If a category name is too long, or ``format`` is ``TickFormat.iso()``.
                Rendering raises it when a value label does not fit inside the plot, as in
                ``bar()``.
            UsageError: If both axes are category axes, or ``format`` or ``size`` is given
                without ``annotate``.

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
            labels = _value_labels(context, annotate, format, size)
            arguments = _series_arguments(context, series)
            category_names = names(context, "categories", categories)
            self._native.grouped_bar(self._target, self._col, category_names, arguments, *labels)
            # The self type only restricts the call: these are the Axes[X, Y] it was made on.
            return cast("Axes[X, Y]", self)

    def stacked_bar(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        series: Sequence[Series],
        *,
        annotate: bool = False,
        format: TickFormat | Auto = AUTO,
        size: Size = Size.AUTO,
    ) -> "Axes[X, Y]":
        """Add stacked bars: in each category, the series stacked on one another.

        Positive and negative values stack separately, above and below the baseline, or on a
        category y axis right and left of it. The baseline is drawn as in ``bar()``, even
        alongside other marks.

        Args:
            categories: The category names, each at most once.
            series: One or more ``Series``, each with one value per category. The first
                series sits on the baseline.
            annotate: Whether each segment shows its value, centred inside it, in black or
                white, whichever contrasts more with the segment, at the tick label size.
            format: The format of the values shown by ``annotate``, as in ``bar()``.
            size: The size of the values shown by ``annotate``, as in ``bar()``.

        Returns:
            These axes.

        Raises:
            TypeError: If ``series`` is not a sequence of ``Series``, or another argument has
                the wrong type.
            DataError: If the data cannot be plotted: ``series`` is empty, a series has a
                different length, a category repeats, a height is infinite or not positive on
                a log axis, a stack exceeds the supported magnitude, or neither axis is a
                category axis.
            StyleError: If a category name is too long, or ``format`` is ``TickFormat.iso()``.
                Rendering raises it when a value label does not fit inside its segment with
                2 pt to spare on every side.
            UsageError: If both axes are category axes, or ``format`` or ``size`` is given
                without ``annotate``.

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
            labels = _value_labels(context, annotate, format, size)
            arguments = _series_arguments(context, series)
            category_names = names(context, "categories", categories)
            self._native.stacked_bar(self._target, self._col, category_names, arguments, *labels)
            # The self type only restricts the call: these are the Axes[X, Y] it was made on.
            return cast("Axes[X, Y]", self)

    def stacked_area(self: SXF, x: Values[X], series: Sequence[Series]) -> SXF:
        """Add non-negative series stacked in order over shared x values.

        Each layer fills between the previous cumulative curve and its own, flat at the
        opacity of an area in every fill style, with its upper curve outlined at the theme's
        line width. The first layer starts at the baseline. A linear y axis includes zero;
        on log y the baseline is the lower limit. A NaN in x or any series breaks every layer
        at that index. Long inputs are reduced per device column, keeping every cumulative
        curve's envelope and exact crossings at explicit limits. The baseline is drawn at
        zero, or at the lower edge when zero is out of range, even alongside other marks.

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
                self._native.stacked_area_time(self._target, self._col, times, arguments)
                return self
            xs = floats(context, "x", x)
            self._native.stacked_area(self._target, self._col, xs, arguments)
            return self

    def hist(
        self: "Axes[float, float]",
        values: Values[float],
        *,
        bins: int | Values[float] = 30,
        style: Hist = Hist.BARS,
        cumulative: bool = False,
        stat: Stat = Stat.COUNT,
        label: str = "",
        color: Color = Color.AUTO,
        by: Values[str] | Auto = AUTO,
    ) -> "Axes[float, float]":
        """Add a histogram: the values counted in bins, one bar per bin.

        ``bins`` is a number of equal-width bins over the range of the values, or the bin
        edges. Bins include their low edge; the last also includes its high edge. NaN values
        are left out of the counts. ``stat`` sets what each bar shows: the count of its bin,
        the fraction of all values (the bars sum to 1), or the density (the fraction over the
        bin's width, so the bars have an area of 1). With ``cumulative=True`` each bar adds up
        the bins up to its own, so the last shows the total, or 1 for fractions. The x axis must
        be linear. The baseline is drawn at zero, or at the lower edge when zero is out of
        range, even alongside other marks.

        Args:
            values: The values to count.
            bins: The number of equal-width bins, from 1 to ``MAX_BINS``, or the bin edges: at
                least two, increasing, holding every finite value.
            style: How the bins are drawn: ``Hist.BARS`` fills one bar per bin, ``Hist.STEP``
                draws only their outline, as one line. ``Hist.STACKED`` stacks the bars of
                groups; for one set of values it draws the same bars as ``Hist.BARS``.
            cumulative: Whether each bar adds up every bin up to its own, from the left.
            stat: What each bar shows: ``Stat.COUNT``, ``Stat.FRACTION`` or ``Stat.DENSITY``.
            label: The name shown in the legend; an empty string leaves the histogram out of
                it.
            color: The bar color. ``Color.AUTO`` takes the next color of the theme palette.
            by: One group name per data row, in first-appearance order; ``AUTO`` draws
                one series. Names supply legend labels and palette colors; do not pass
                a non-empty ``label`` or an explicit ``color``. Missing or empty names
                and an empty grouping column are refused. Column names are never read.
                Groups share the bins. ``Hist.STEP`` normalizes each group separately;
                ``Hist.STACKED`` normalizes their total. ``Hist.BARS`` is refused.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``by`` has invalid names or differs in length from the data; or if
                a value is infinite, the edges are fewer than two, not finite or not
                increasing, finite values lie outside the edges, or the x axis is a category
                axis.
            StyleError: If the number of bins is out of range, or ``label`` is too long.
            UsageError: If ``by`` comes with an explicit color or a non-empty label; or if
                the x axis is a log axis, or ``cumulative=True`` comes with
                ``Stat.DENSITY``, whose densities do not add up.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.hist([1.2, 2.5, 2.7, 3.1, 4.8, 5.0], bins=4, label="Sizes")
            >>> edges = ub.Figure().add_axes()
            >>> edges = edges.hist([0.2, 1.5, 1.7, 4.0], bins=[0, 1, 2, 5], stat=ub.Stat.DENSITY)
            >>> runs = ub.Figure().add_axes()
            >>> runs = runs.hist([3.0, 1.0, 2.0], cumulative=True, stat=ub.Stat.FRACTION)
            >>> outline = ub.Figure().add_axes().hist([1.0, 2.0, 2.0], style=ub.Hist.STEP)
            >>> grouped = ub.hist([1, 2, 3, 4], by=["B", "A", "B", "A"], style=ub.Hist.STEP)
        """
        with api_call():
            context = "Axes.hist()"
            check_floats_type(context, "values", values)
            edges = _holds_edges(bins)
            count = 0
            if edges:
                check_floats_type(context, "bins", bins)
            else:
                count = integer(context, "bins", bins)
            drawn = instance(context, "style", style, Hist, _HIST_HINT)
            running = _args.boolean(context, "cumulative", cumulative)
            shown = instance(context, "stat", stat, Stat, _STAT_HINT)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)

            def draw(samples: FloatArray, *, label: str) -> None:
                self._native.hist(
                    self._target,
                    self._col,
                    samples,
                    floats(context, "bins", bins) if edges else count,
                    drawn.value,
                    running,
                    shown.value,
                    label,
                    spec,
                )

            def draw_groups(series: list[_group.NativeSeries]) -> None:
                self._native.grouped_hist(
                    self._target,
                    self._col,
                    series,
                    floats(context, "bins", bins) if edges else count,
                    drawn.value,
                    running,
                    shown.value,
                )

            _group.call(
                context,
                by,
                label,
                color,
                _group.Samples(
                    values=partial(floats, context, "values", values),
                    draw=draw,
                    draw_groups=draw_groups,
                ),
            )
            return self

    def mesh(
        self: SXY,
        x_edges: Values[X],
        y_edges: Values[Y],
        values: object,
        *,
        cmap: Cmap | Auto = AUTO,
        scale: Scale[float] = Scale.linear(),
        center: float | Auto = AUTO,
        render: Render | Auto = AUTO,
    ) -> SXY:
        """Add rectangular cells whose colors map a row-major matrix of values.

        Cells span their edges exactly, without padding their extents. Other marks on the
        axes, on a numeric or a time axis, keep their usual padding; the domain is the union.
        NaN values leave cells blank. Neither axis may be a category axis; the outer edge
        ranges of a numeric axis must meet the axis precision limits, as for ``xlim()`` and
        ``ylim()``. All color-mapped marks share one color scale. By default a mesh alone
        draws no baseline; one is drawn only alongside a mark that grows from zero, such as
        an area, histogram or stem. ``baseline(shown=...)`` overrides it.

        The cells can be drawn as one image (see ``Render``) only when the edges are evenly
        spaced on screen: evenly spaced numbers on a linear axis, evenly spaced logarithms on a
        log axis, evenly spaced times on a time axis. Every edge must lie within 1/10,000 of a
        cell of its place on the evenly spaced lattice from the first edge to the last (numbers
        may also be off by two units of rounding error of the largest end edge; times are
        compared exactly, in microseconds). When drawn, the image must also be exact on the
        final plot (see ``Render``): no edge more than 0.001 pt from its place, and no cut cell
        reaching more than 10,000 pt past the limits. Otherwise ``AUTO`` draws vector shapes,
        and with ``Render.RASTER`` rendering raises ``StyleError``.

        Args:
            x_edges: Strictly increasing column edges, one more than the number of columns:
                numbers on a linear or log x axis, times on a time x axis.
            y_edges: Strictly increasing row edges, one more than the number of rows: numbers
                on a linear or log y axis, times on a time y axis.
            values: A two-dimensional numeric matrix; rows run along y and columns along x.
            cmap: The colormap, or ``AUTO`` for the theme's colormap.
            scale: A linear or log color scale, independent of the axis scales.
            center: The middle of a diverging mapping, or ``AUTO`` for sequential colors.
            render: ``Render.VECTOR`` draws one rectangle per cell, ``Render.RASTER`` one
                image with one pixel per cell. ``AUTO`` draws an image from 10,000 cells
                (rows times columns) when the edges are evenly spaced, and vector shapes
                otherwise.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If the matrix shape or edges are invalid (a missing time among time
                edges included), or a value is invalid.
            StyleError: If the color center is invalid.
            UsageError: If an axis is a category axis (type checkers refuse such axes, so this is
                raised only for calls they do not check), color scales conflict, or
                ``render`` is ``Render.RASTER`` and the edges are not evenly spaced on screen;
                that message names the first edge off the lattice and where it belongs.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.mesh([0, 1, 3], [0, 2], [[1.0, 4.0]])
            >>> import datetime as dt
            >>> hours = ub.Figure().add_axes(x=ub.Scale.time())
            >>> times = [dt.datetime(2026, 1, 1, hour) for hour in (0, 6, 12)]
            >>> hours = hours.mesh(times, [0, 2], [[1.0, 4.0]])
            >>> ax = ax.mesh([0, 1, 2], [0, 1], [[2.0, 3.0]], render=ub.Render.RASTER)
        """
        with api_call():
            context = "Axes.mesh()"
            _check_coordinates_type(context, "x_edges", x_edges, self._x)
            _check_coordinates_type(context, "y_edges", y_edges, self._y)
            check_floats_type(context, "values", values)
            colors = _cell_colors(context, cmap, scale, center)
            drawn = _render_argument(context, render)
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
            self._native.mesh(self._target, self._col, xs, ys, data, *colors, drawn)
            return self

    def hist2d(
        self: "Axes[float, float]",
        x: Values[float],
        y: Values[float],
        *,
        bins: int | tuple[int, int] = 30,
        cmap: Cmap | Auto = AUTO,
        scale: Scale[float] = Scale.linear(),
        render: Render | Auto = AUTO,
    ) -> "Axes[float, float]":
        """Count paired points in equal-width rectangular bins over their data ranges.

        Widths are equal in axis space: in data on a linear axis, in the logarithm on a log
        axis. A pair with either coordinate NaN is skipped. Empty bins are blank, including
        on a log color scale. Both axes must be numeric; counts share the axes' color scale.
        By default a 2-D histogram alone draws no baseline; one is drawn only alongside a
        mark that grows from zero, such as an area, histogram or stem.
        ``baseline(shown=...)`` overrides it.

        Args:
            x: The x coordinates.
            y: The y coordinates, one per x coordinate.
            bins: One count for both axes, or ``(x_count, y_count)``, each in 1..MAX_BINS.
            cmap: The colormap, or ``AUTO`` for the theme's sequential colormap.
            scale: A linear or log color scale.
            render: ``Render.VECTOR`` draws one rectangle per bin, ``Render.RASTER`` one image
                with one pixel per bin (see ``Render``). ``AUTO`` draws an image from 10,000
                bins (``x_count`` times ``y_count``) and vector shapes below that.

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
            >>> ax = ax.hist2d([0, 1, 1], [0, 2, 2], bins=200, render=ub.Render.VECTOR)
        """
        with api_call():
            context = "Axes.hist2d()"
            check_floats_type(context, "x", x)
            check_floats_type(context, "y", y)
            counts = _rect_counts(context, bins)
            colors = _cell_colors(context, cmap, scale, AUTO)
            drawn = _render_argument(context, render)
            _numeric_cells(context, self, time=False)
            xs, ys = floats(context, "x", x), floats(context, "y", y)
            self._native.hist2d(self._target, self._col, xs, ys, counts, *colors[:3], drawn)
            return self

    def hexbin(
        self: "Axes[float, float]",
        x: Values[float],
        y: Values[float],
        *,
        gridsize: int = 30,
        cmap: Cmap | Auto = AUTO,
        scale: Scale[float] = Scale.linear(),
    ) -> "Axes[float, float]":
        """Count paired points in pointy-top hexagons, regular in screen coordinates.

        Automatic limits include whole hexagons without changing the plot's shape. Explicit
        limits clip cells. Pairs containing NaN are skipped and empty cells stay blank.
        With a narrow explicit ``xlim`` and automatic y limits, hexagons keep their size;
        the y range may grow far beyond the data to keep every hexagon whole and regular.
        Set ``ylim()`` or choose a larger ``gridsize`` to control that range.
        By default a hexbin mark alone draws no baseline; one is drawn only alongside a
        mark that grows from zero, such as an area, histogram or stem.
        ``baseline(shown=...)`` overrides it.

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
            self._native.hexbin(self._target, self._col, xs, ys, count, *colors[:3])
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
        by: Values[str] | Auto = AUTO,
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
            by: One group name per data row, in first-appearance order; ``AUTO`` draws
                one series. Names supply legend labels and palette colors; do not pass
                a non-empty ``label`` or an explicit ``color``. Missing or empty names
                and an empty grouping column are refused. Column names are never read.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``by`` has invalid names or differs in length from the data; or if
                the data cannot be plotted: lengths differ, a value is infinite, a
                value is not positive on a log axis, names and numbers do not match the x
                axis, or x is not in non-decreasing order.
            StyleError: If ``label`` is too long.
            UsageError: If ``by`` comes with an explicit color or a non-empty label; or if
                the y axis is a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.step([0, 1, 3, 4], [1.0, 2.0, float("nan"), 0.5], where=ub.Step.MID)
            >>> grouped = ub.Figure().add_axes()
            >>> grouped = grouped.step([0, 0, 1, 1], [1, 2, 3, 4], by=["B", "A", "B", "A"])
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
            _group.call(
                context,
                by,
                label,
                color,
                _group.XY(
                    x=partial(self._x_values, context, x),
                    y=partial(self._y_values, context, y),
                    draw=partial(
                        self._native.step,
                        self._target,
                        self._col,
                        position=position.value,
                        color=spec,
                        dash=style.value,
                        width=extent,
                    ),
                    draw_groups=partial(
                        self._native.grouped_step,
                        self._target,
                        self._col,
                        position=position.value,
                        dash=style.value,
                        width=extent,
                    ),
                ),
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
                    self._target, self._col, value, name, spec, style.value, extent
                )
            else:
                self._native.hline(self._target, self._col, value, name, spec, style.value, extent)
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
                self._native.vline_time(
                    self._target, self._col, when, name, spec, style.value, extent
                )
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
            self._native.vline(self._target, self._col, position, name, spec, style.value, extent)
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
                self._native.hspan_time(self._target, self._col, start, end, name, spec)
                return self
            lower = real(context, "low", low)
            upper = real(context, "high", high)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            name = _args.text(context, "label", label)
            self._native.hspan(self._target, self._col, lower, upper, name, spec)
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
                self._native.vspan_time(self._target, self._col, start, end, name, spec)
                return self
            lower = real(context, "low", low)
            upper = real(context, "high", high)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            name = _args.text(context, "label", label)
            self._native.vspan(self._target, self._col, lower, upper, name, spec)
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
                self._native.band_time(self._target, self._col, times, lows, highs, name, spec)
            else:
                numbers = cast(FloatArray, xs)
                self._native.band(self._target, self._col, numbers, lows, highs, name, spec)
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
                self._target, self._col, xs, lows, highs, name, spec, extent, flag
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
                self._target, self._col, ys, lows, highs, name, spec, extent, flag
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
            self._native.violin(self._target, self._col, category_names, values, name, spec)
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
            self._native.grouped_violin(self._target, self._col, category_names, arguments)
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
            self._native.clim(self._target, self._col, lo, hi)
            return self

    def colorbar(
        self,
        label: str = "",
        *,
        position: ColorbarPosition | Auto = AUTO,
        format: TickFormat | Auto = AUTO,
        size: Size = Size.AUTO,
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

            size: The label size only; colorbar ticks keep the theme's size. ``Size.AUTO``
                follows the theme, ``Size.times(k)`` multiplies its scaled size, and
                ``Size.pt(p)`` uses points without font scaling.

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
            instance(context, "size", size, Size, _SIZE_HINT)
            name = _args.text(context, "label", label)
            self._native.colorbar(
                self._target,
                self._col,
                name,
                placement,
                None if checked is None else _ticks.native(checked),
                _size.native(size),
            )
            return self

    def heatmap(
        self: "Axes[str, str]",
        x_categories: Values[str],
        y_categories: Values[str],
        values: object,
        *,
        cmap: Cmap | Auto = AUTO,
        scale: Scale[float] = Scale.linear(),
        center: float | Auto = AUTO,
        annotate: bool = False,
        format: TickFormat | Auto = AUTO,
        render: Render | Auto = AUTO,
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
            render: ``Render.VECTOR`` draws one rectangle per cell, ``Render.RASTER`` one
                image with one pixel per cell (see ``Render``). ``AUTO`` draws an image from
                10,000 cells (``len(x_categories) * len(y_categories)``) and vector shapes
                below that. Annotations are text over the cells either way.

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
            >>> im = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
            >>> im = im.heatmap(days, periods, [[1, 2, 3], [4, 5, 6]], render=ub.Render.RASTER)
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
            drawn = _render_argument(context, render)
            if (self._x, self._y) != (_CATEGORY, _CATEGORY):
                # The core refuses the axes before it reads the data, with its own error.
                empty = np.empty(0, dtype=np.float64)
                self._native.heatmap(
                    self._target,
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
                    drawn,
                )
            x_names = names(context, "x_categories", x_categories)
            y_names = names(context, "y_categories", y_categories)
            cells = matrix(
                context,
                "values",
                values,
                (len(y_names), len(x_names)),
                lengths="(len(y_categories), len(x_categories))",
                hint="pass one row per y category.",
            )
            self._native.heatmap(
                self._target,
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
                drawn,
            )
            return self

    def xtick_labels(self, *, size: Size = Size.AUTO) -> Self:
        """Set the x tick label size without changing ticks or their format.

        Args:
            size: ``Size.times(k)`` multiplies the theme's scaled size; ``Size.pt(p)``
                uses points without font scaling. Only these explicit sizes must agree in
                the shared axis group. ``Size.AUTO`` clears this panel's size setting and
                inherits the group's agreed explicit size, or the theme's size when none
                is set. Ticks and their format are unchanged.

        Returns:
            These axes.

        Raises:
            TypeError: If ``size`` is not a ``Size``.
            UsageError: If another panel sharing this axis set a different size.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().xtick_labels(size=ub.Size.pt(10))
        """
        with api_call():
            instance("Axes.xtick_labels()", "size", size, Size, _SIZE_HINT)
            self._native.xtick_labels(self._target, self._col, _size.native(size))
            return self

    def ytick_labels(self, *, size: Size = Size.AUTO) -> Self:
        """Set the y tick label size without changing ticks or their format.

        Args:
            size: ``Size.times(k)`` multiplies the theme's scaled size; ``Size.pt(p)``
                uses points without font scaling. Only these explicit sizes must agree in
                the shared axis group. ``Size.AUTO`` clears this panel's size setting and
                inherits the group's agreed explicit size, or the theme's size when none
                is set. Ticks and their format are unchanged.

        Returns:
            These axes.

        Raises:
            TypeError: If ``size`` is not a ``Size``.
            UsageError: If another panel sharing this axis set a different size.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().ytick_labels(size=ub.Size.pt(10))
        """
        with api_call():
            instance("Axes.ytick_labels()", "size", size, Size, _SIZE_HINT)
            self._native.ytick_labels(self._target, self._col, _size.native(size))
            return self

    def events(
        self: "Axes[float, str] | Axes[Time, str]",
        rows: Values[str],
        positions: Sequence[Values[X]],
        *,
        length: float | Auto = AUTO,
        width: Size = Size.AUTO,
        color: Color = Color.AUTO,
    ) -> "Axes[X, Y]":
        """Add short vertical ticks at event positions, one row per category.

        Rows run from top to bottom in first-appearance order. A numeric or time x axis
        supplies the positions. Empty groups keep their row; NaN and NaT draw no tick.
        Times are rounded to the nearest microsecond, as for other time-axis marks.
        Marks draw in call order. Input data is copied and a failed call changes nothing.

        Args:
            rows: Unique row names.
            positions: One group of numbers or times per row, matching the x axis.
            length: Fraction of the row height, above 0 and at most 1; AUTO uses 0.8.
            width: Tick stroke width. Size.AUTO draws 1 pt ticks.
            color: Tick color; Color.AUTO takes the next palette color.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If rows repeat, group counts differ, or a position is invalid.
            StyleError: If length is non-finite or outside its permitted range.
            UsageError: If x is not numeric or time, or y is not a category axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(y=ub.Scale.category())
            >>> ax = ax.events(["North", "South"], [[1, 3, 5], [2, 4]])
        """
        with api_call():
            context = "Axes.events()"
            check_names_type(context, "rows", rows)
            _series.check_groups_type(context, positions, name="positions", time=self._x == _TIME)
            extent = None if length is AUTO else real(context, "length", length)
            stroke = _size_argument(context, "width", width)
            paint = _color_argument(context, color)
            row_names = names(context, "rows", rows)
            converted: list[FloatArray | MicrosArray] = []
            if self._x == _TIME:
                try:
                    for i, group in enumerate(positions):
                        converted.append(_time.micros(context, f"positions[{i}]", group))
                except (TypeError, UmberError) as error:
                    raise _event_error(context, error) from None
            else:
                converted.extend(_series.group_values(context, positions, name="positions"))
            self._native.events(
                self._target, self._col, row_names, converted, extent, stroke, paint
            )
            return cast("Axes[X, Y]", self)

    def strip(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        groups: Sequence[Values[float]],
        *,
        jitter: float | Auto = AUTO,
        size: Size = Size.AUTO,
        color: Color = Color.AUTO,
        label: str = "",
    ) -> "Axes[X, Y]":
        """Add individual circular samples within each category band.

        Jitter is uniform within the chosen fraction of each band, from a fixed
        SplitMix64 seed for each input category and strip on these axes. Repeated
        renders are byte-identical; unrelated marks do not change the jitter.
        A category x axis gives vertical groups; a category y axis gives horizontal
        groups. NaN is ignored and empty groups draw nothing. The numeric axis may be
        linear or log. Markers use the scatter opacity and outline. Marks draw in call
        order, so call this after box() or violin() to put points above them.
        Input data is copied and a failed call changes nothing.

        Args:
            categories: Unique category names.
            groups: One group of numeric samples per category.
            jitter: Maximum displacement as a band fraction from 0 to 0.5; AUTO uses 0.4.
            size: Marker diameter; Size.AUTO uses the theme's marker size.
            color: Marker color; Color.AUTO takes the next palette color.
            label: The legend label; empty for none. Points use a marker swatch.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If categories repeat, group counts differ, or samples are invalid.
            StyleError: If jitter is non-finite or outside 0 to 0.5, or label is too long.
            UsageError: If the axes are not one category axis and one numeric axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.strip(["North", "South"], [[1, 2, 3], [2, 3, 4]], label="Samples")
        """
        with api_call():
            context = "Axes.strip()"
            check_names_type(context, "categories", categories)
            _series.check_groups_type(context, groups)
            _args.require_text(context, "label", label)
            spread = None if jitter is AUTO else real(context, "jitter", jitter)
            extent = _size_argument(context, "size", size)
            paint = _color_argument(context, color)
            category_names = names(context, "categories", categories)
            values = _series.group_values(context, groups)
            name = _args.text(context, "label", label)
            self._native.strip(
                self._target, self._col, category_names, values, name, paint, extent, spread
            )
            return cast("Axes[X, Y]", self)

    def swarm(
        self: "Axes[str, float] | Axes[float, str]",
        categories: Values[str],
        groups: Sequence[Values[float]],
        *,
        size: Size = Size.AUTO,
        color: Color = Color.AUTO,
        label: str = "",
    ) -> "Axes[X, Y]":
        """Add individual circular samples within each category band.

        Points are sorted by value, then placed at the free offset nearest the centre
        line. When circles do not fit in a category band at the final plot size,
        rendering raises StyleError naming the category; use strip() or violin().
        A category x axis gives vertical groups; a category y axis gives horizontal
        groups. NaN is ignored and empty groups draw nothing. The numeric axis may be
        linear or log. Markers use the scatter opacity and outline. Marks draw in call
        order, so call this after box() or violin() to put points above them.
        Input data is copied and a failed call changes nothing.

        Args:
            categories: Unique category names.
            groups: One group of numeric samples per category.
            size: Marker diameter; Size.AUTO uses the theme's marker size.
            color: Marker color; Color.AUTO takes the next palette color.
            label: The legend label; empty for none. Points use a marker swatch.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If categories repeat, group counts differ, or samples are invalid.
            StyleError: If label is too long, or a category cannot fit its points at render.
            UsageError: If the axes are not one category axis and one numeric axis.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
            >>> ax = ax.swarm(["North", "South"], [[1, 2, 3], [2, 3, 4]], label="Samples")
        """
        with api_call():
            context = "Axes.swarm()"
            check_names_type(context, "categories", categories)
            _series.check_groups_type(context, groups)
            _args.require_text(context, "label", label)
            extent = _size_argument(context, "size", size)
            paint = _color_argument(context, color)
            category_names = names(context, "categories", categories)
            values = _series.group_values(context, groups)
            name = _args.text(context, "label", label)
            self._native.swarm(self._target, self._col, category_names, values, name, paint, extent)
            return cast("Axes[X, Y]", self)

    def ecdf(
        self: SXF,
        values: Values[X],
        *,
        complementary: bool = False,
        label: str = "",
        color: Color = Color.AUTO,
        dash: Dash = Dash.SOLID,
        width: Size = Size.AUTO,
        by: Values[str] | Auto = AUTO,
    ) -> SXF:
        """Add the empirical cumulative distribution (ECDF) of ``values``, as a step line.

        At each distinct value, from the smallest, the line steps up to the fraction of the
        values at or below it: from 0 at the smallest value to 1 at the largest. With
        ``complementary=True`` it steps down to the fraction above each value instead, from 1
        to 0. NaN values and missing times (``NaT``) are left out. The y axis must be linear;
        when it holds only ECDFs, it runs exactly from 0 to 1. A labelled ECDF is listed and
        labelled as a step line is; a direct label goes to its end.

        Args:
            values: The values: numbers on a linear or log x axis, times on a time x axis.
            complementary: Whether to draw the fraction above each value, from 1 down to 0.
            label: The name shown in the legend; an empty string leaves the line out of it.
            color: The line color. ``Color.AUTO`` takes the next color of the theme palette.
            dash: How the line is stroked.
            width: The line width. ``Size.AUTO`` is the line width of the theme.
            by: One group name per data row, in first-appearance order; ``AUTO`` draws
                one series. Names supply legend labels and palette colors; do not pass
                a non-empty ``label`` or an explicit ``color``. Missing or empty names
                and an empty grouping column are refused. Column names are never read.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``by`` has invalid names or differs in length from the data; or if
                no value is finite, a value is infinite or not positive on a log
                axis, or the values do not suit the x axis.
            StyleError: If ``label`` is too long.
            UsageError: If ``by`` comes with an explicit color or a non-empty label; or if
                the y axis is not linear.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.ecdf([12.0, 9.5, 30.1, 12.0, 14.2], label="Latency")
            >>> ax = ax.ecdf([10.0, 11.0, 25.0], complementary=True, dash=ub.Dash.DASHED)
            >>> import datetime as dt
            >>> days = ub.Figure().add_axes(x=ub.Scale.time())
            >>> days = days.ecdf([dt.date(2026, 1, 3), dt.date(2026, 1, 1), dt.date(2026, 1, 9)])
            >>> grouped = ub.Figure().add_axes().ecdf([1, 2, 3, 4], by=["B", "A", "B", "A"])
        """
        with api_call():
            context = "Axes.ecdf()"
            _check_coordinates_type(context, "values", values, self._x)
            flag = _args.boolean(context, "complementary", complementary)
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            style = instance(context, "dash", dash, Dash, _DASH_HINT)
            extent = _size_argument(context, "width", width)
            _group.call(
                context,
                by,
                label,
                color,
                _group.Ecdf(
                    values=partial(_limit_values, context, "values", values, self._x),
                    draw=partial(
                        self._native.ecdf,
                        self._target,
                        self._col,
                        complementary=flag,
                        color=spec,
                        dash=style.value,
                        width=extent,
                    ),
                    draw_groups=partial(
                        self._native.grouped_ecdf,
                        self._target,
                        self._col,
                        complementary=flag,
                        dash=style.value,
                        width=extent,
                    ),
                ),
            )
            return self

    def kde(
        self: "Axes[float, float]",
        values: Values[float],
        *,
        bandwidth: float | Auto = AUTO,
        label: str = "",
        color: Color = Color.AUTO,
        dash: Dash = Dash.SOLID,
        width: Size = Size.AUTO,
        by: Values[str] | Auto = AUTO,
    ) -> "Axes[float, float]":
        """Add the Gaussian kernel density estimate (KDE) of ``values``, as a line.

        The line is the mean of Gaussian kernels placed at the values, in data units, so the
        area under it is 1. It runs from three bandwidths below the smallest value to three
        above the largest, through evenly spaced points at most a quarter bandwidth apart (at
        least 256, at most 16,384). NaN values are left out. The x axis must be linear, and so
        must the y axis, which includes zero. A labelled density is listed and labelled as a
        line is.

        Args:
            values: The values.
            bandwidth: The standard deviation of the kernels, in data units, greater than 0.
                ``AUTO`` uses Scott's rule, as ``violin()`` does, so a violin and a density
                line of the same values agree; it needs at least two distinct values.
            label: The name shown in the legend; an empty string leaves the line out of it.
            color: The line color. ``Color.AUTO`` takes the next color of the theme palette.
            dash: How the line is stroked.
            width: The line width. ``Size.AUTO`` is the line width of the theme.
            by: One group name per data row, in first-appearance order; ``AUTO`` draws
                one series. Names supply legend labels and palette colors; do not pass
                a non-empty ``label`` or an explicit ``color``. Missing or empty names
                and an empty grouping column are refused. Column names are never read.
                Each group has its own density with area 1.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If ``by`` has invalid names or differs in length from the data; or if
                no value is finite, a value is infinite, ``AUTO`` finds fewer than
                two distinct values, the line would reach beyond 1e300, the density would
                exceed 1e300, or the x axis is a category axis.
            StyleError: If ``bandwidth`` is not a finite number greater than 0, is so narrow
                that the line would need more than 16,384 points, or ``label`` is too long.
            UsageError: If ``by`` comes with an explicit color or a non-empty label; or if
                the x axis is a log or time axis, or the y axis is not linear.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes()
            >>> ax = ax.kde([1.2, 2.5, 2.7, 3.1, 4.8, 5.0], label="Sizes")
            >>> ax = ax.kde([1.2, 2.5, 2.7, 3.1, 4.8, 5.0], bandwidth=0.3, dash=ub.Dash.DOTTED)
            >>> grouped = ub.Figure().add_axes().kde([1, 2, 3, 4], by=["B", "A", "B", "A"])
        """
        with api_call():
            context = "Axes.kde()"
            check_floats_type(context, "values", values)
            width_of = (
                None
                if bandwidth is AUTO
                else real(
                    context,
                    "bandwidth",
                    bandwidth,
                    expected="a float or ub.AUTO",
                    hint=_BANDWIDTH_HINT,
                )
            )
            _args.require_text(context, "label", label)
            spec = _color_argument(context, color)
            style = instance(context, "dash", dash, Dash, _DASH_HINT)
            extent = _size_argument(context, "width", width)
            _group.call(
                context,
                by,
                label,
                color,
                _group.Samples(
                    values=partial(floats, context, "values", values),
                    draw=partial(
                        self._native.kde,
                        self._target,
                        self._col,
                        bandwidth=width_of,
                        color=spec,
                        dash=style.value,
                        width=extent,
                    ),
                    draw_groups=partial(
                        self._native.grouped_kde,
                        self._target,
                        self._col,
                        bandwidth=width_of,
                        dash=style.value,
                        width=extent,
                    ),
                ),
            )
            return self

    def text(
        self,
        text: str,
        at: tuple[X, Y],
        *,
        align: Align = Align.LEFT,
        size: Size = Size.AUTO,
        color: Color = Color.AUTO,
    ) -> Self:
        """Write text at a data position.

        The aligned edge of the text, or its centre, is at the point, and so is the vertical
        centre of the text. The text is drawn over the marks and changes neither the limits nor
        the layout; overlap with other elements is not checked.

        Args:
            text: The text, at least one and at most 1,000 characters.
            at: The point ``(x, y)``, each in the values of its axis: a number on a linear or
                log axis, a category name (the category's centre) on a category axis, a time
                on a time axis.
            align: Which edge of the text, or its centre, is at the point.
            size: ``Size.AUTO`` uses the theme's body size, ``Size.times(k)`` multiplies it
                after font scaling, and ``Size.pt(p)`` uses points without font scaling.
            color: The text color. ``Color.AUTO`` is the theme's text color.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type, including ``at`` when it is not a
                pair or a coordinate does not suit its axis.
            DataError: If a coordinate is not finite, beyond 1e300 in magnitude or not
                positive on a log axis. Rendering raises it when no mark of these axes has the
                category of the point.
            StyleError: If ``text`` is too long. Rendering raises it when the point lies
                outside the limits or the text reaches past the figure.
            UsageError: If ``text`` is empty.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().xlim(0, 10).ylim(0, 10)
            >>> ax = ax.text("Target", (5, 5), align=ub.Align.CENTER)
            >>> bars = ub.Figure().add_axes(x=ub.Scale.category()).bar(["Q1", "Q2"], [3, 5])
            >>> bars = bars.text("Best", ("Q2", 5.5))
        """
        with api_call():
            context = "Axes.text()"
            _args.require_text(context, "text", text)
            pair = self._point_types(context, at)
            style = instance(context, "align", align, Align, _ALIGN_HINT)
            instance(context, "size", size, Size, _SIZE_HINT)
            spec = _color_argument(context, color)
            point = self._point(context, pair)
            content = _args.text(context, "text", text)
            self._native.text(
                self._target, self._col, content, point, style.value, _size.native(size), spec
            )
            return self

    def annotate(
        self,
        text: str,
        at: tuple[X, Y],
        *,
        offset: tuple[float, float] = (0.0, 12.0),
        arrow: bool = True,
        size: Size = Size.AUTO,
        color: Color = Color.AUTO,
    ) -> Self:
        """Write text a fixed distance from a data position, with an arrow to it.

        The text sits ``offset`` points from the point, so it looks the same at any data range
        or figure size. The direction of the offset, in eight sectors of 45°, decides which
        part of the text is at the offset point: going round from above, the bottom centre,
        bottom left, middle left, top left, top centre, top right, middle right and bottom
        right, so the text stays on that side and never covers the point. The arrow runs from
        2 pt beyond the text to 3 pt short of the point, with a filled head, in the color of
        the text. The annotation changes neither the limits nor the layout.

        Args:
            text: The text, at least one and at most 1,000 characters.
            at: The point ``(x, y)``, given as for ``text()``.
            offset: Where the text is from the point, in points: x to the right, y upwards.
                It must not be ``(0, 0)``.
            arrow: Whether an arrow runs from the text to the point; it needs at least 10 pt
                between them.
            size: ``Size.AUTO`` uses the theme's body size, ``Size.times(k)`` multiplies it
                after font scaling, and ``Size.pt(p)`` uses points without font scaling.
            color: The color of the text and the arrow. ``Color.AUTO`` is the theme's text
                color.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type, as for ``text()``, or ``offset`` is
                not a pair of numbers.
            DataError: As for ``text()``.
            StyleError: If ``text`` is too long, or ``offset`` is not finite or is zero.
                Rendering raises it as for ``text()``, and when less than 10 pt lie between
                the text and the point for an arrow.
            UsageError: If ``text`` is empty.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes().line([0, 1, 2, 3], [2, 5, 3, 4])
            >>> ax = ax.annotate("Peak", (1, 5), offset=(24, 12))
            >>> ax = ax.annotate("Dip", (2, 3), offset=(18, -22), arrow=False)
        """
        with api_call():
            context = "Axes.annotate()"
            _args.require_text(context, "text", text)
            pair = self._point_types(context, at)
            shift = _offset_argument(context, offset)
            drawn = _args.boolean(context, "arrow", arrow)
            instance(context, "size", size, Size, _SIZE_HINT)
            spec = _color_argument(context, color)
            point = self._point(context, pair)
            content = _args.text(context, "text", text)
            self._native.annotate(
                self._target, self._col, content, point, shift, drawn, _size.native(size), spec
            )
            return self

    def _point_types(self, context: str, at: object) -> tuple[object, object]:
        """Check the types of an ``at`` argument: a pair whose coordinates suit their axes."""
        pair = _pair(at)
        if pair is None:
            message = f"at is {_shown_pair(at)}; at must be a pair (x, y)."
            raise new_error(TypeError, context, message, parameter="at", hint=_AT_HINT)
        _check_coordinate_type(context, self._x, pair[0])
        _check_coordinate_type(context, self._y, pair[1])
        return pair

    def _point(
        self, context: str, pair: tuple[object, object]
    ) -> tuple[float | str | int, float | str | int]:
        """Convert the coordinates of an ``at`` pair whose types were checked to native form.

        A coordinate is a ``float`` on a numeric axis, a name on a category axis and a time in
        microseconds on a time axis.
        """
        x, y = pair
        return _coordinate(context, self._x, x), _coordinate(context, self._y, y)

    def contour(
        self: "Axes[float, float]",
        x: Values[float],
        y: Values[float],
        z: object,
        *,
        levels: int | Values[float] | Auto = AUTO,
        style: Contour = Contour.LINES,
        cmap: Cmap | Auto = AUTO,
        scale: Scale[float] = Scale.linear(),
        center: float | Auto = AUTO,
        color: Color = Color.AUTO,
        annotate: bool = False,
        format: TickFormat | Auto = AUTO,
    ) -> "Axes[float, float]":
        """Add a contour of a field sampled on a rectangular grid: lines, filled bands, or both.

        ``z`` holds the field at the grid points, one row per y coordinate. Each cell of the
        grid lies between four neighbouring points; along its sides the field is linear, and the
        lines run straight across it between the points where the level crosses its sides. A
        cell whose corners alternate above and below a level joins its high corners when the
        mean of its four values is at or above the level, and separates them otherwise. A NaN
        value leaves the four cells around it out of every line and band, as a hole.

        Lines are colored by their level from the colormap, unless ``color`` gives them one
        color. Bands take the color of the middle of their range, and the regions below the
        first level and above the last the colors at the ends of the color range, as values
        beyond ``clim()`` do. The colors come from the color scale of these axes, which every
        color-mapped mark shares; the colorbar shows the levels, as bands for filled contours
        and as ticks on a smooth bar for lines. Automatic limits are the outer coordinates,
        without padding. By default a contour alone draws no baseline.

        A field that changes from cell to cell, such as noise or a checkerboard, makes many short
        lines and many small bands, so vector output grows very large and slow to write and to
        draw. Use fewer levels, or ``Contour.FILLED`` instead of ``Contour.BOTH``; smooth or
        aggregate the data first; or use ``mesh()`` or ``heatmap()`` when the cells themselves
        are what matters. Contours never thin the data on their own.

        Args:
            x: The x coordinates of the grid's columns, strictly increasing, at least two.
            y: The y coordinates of the grid's rows, strictly increasing, at least two.
            z: The field as a two-dimensional array or a list of rows, shaped
                ``(len(y), len(x))``: one row per y coordinate.
            levels: ``AUTO`` for the round values strictly inside the range of the finite
                values that a search for seven round ticks over the range finds (log ticks on a
                log ``scale``); an int from 1 to 64 for about that many round values strictly
                inside the range, and at least one, the middle of the range, on a field that is
                not constant; or the levels themselves, strictly increasing. Levels outside the
                range of the values draw no line. Contours that share a colorbar, on one axes
                or through ``share_color``, need the same levels.
            style: ``Contour.LINES``, ``Contour.FILLED`` or ``Contour.BOTH``.
            cmap: The colormap. ``AUTO`` uses the theme's diverging colormap when ``center``
                is given and its sequential colormap otherwise.
            scale: ``Scale.linear()``, or ``Scale.log()`` for values that are all positive.
            center: The value at the middle of a diverging colormap, as for ``ColorBy``;
                ``AUTO`` for a sequential mapping.
            color: ``Color.AUTO`` colors each line by its level. Any other color draws every
                line of ``Contour.LINES`` in it; the contour is then not color-mapped and draws
                no colorbar.
            annotate: Whether each line at least three times as long as its label carries its
                level's value at its middle, along the line and upright, in the tick font, with
                the line broken under it.
            format: The format of the values that ``annotate`` writes. ``AUTO`` writes them as
                the colorbar writes its ticks (its own format, else the y tick format), so a
                level reads the same on its line and on the bar.

        Returns:
            These axes.

        Raises:
            TypeError: If an argument has the wrong type, or ``scale`` is not linear or log.
            DataError: If ``z`` is not shaped ``(len(y), len(x))``, a coordinate is not finite,
                not strictly increasing or not positive on a log axis, there are fewer than two
                along an axis, or a value is infinite, beyond 1e300 in magnitude, or not
                positive on a log scale.
            StyleError: If a count of levels is not from 1 to 64, given levels are not finite
                and strictly increasing, ``center`` is not finite or comes with a log scale,
                ``format`` is ``TickFormat.iso()``, or the color range set by ``clim()`` does
                not suit ``scale`` or ``center``.
            UsageError: If an axis is not linear or log (type checkers refuse such axes),
                ``color`` is given with filled bands or with ``cmap`` or ``center``,
                ``annotate`` is ``True`` with ``Contour.FILLED``, ``format`` is given without
                ``annotate``, or ``cmap``, ``scale`` or ``center`` differs from those of the
                other color-mapped marks of these axes.

        Example:
            >>> import numpy as np
            >>> import umber as ub
            >>> x, y = np.linspace(-2, 2, 41), np.linspace(-1.5, 1.5, 31)
            >>> z = np.exp(-(x[None, :] ** 2) - y[:, None] ** 2)
            >>> ax = ub.Figure().add_axes().contour(x, y, z, annotate=True)
            >>> filled = ub.Figure().add_axes().contour(x, y, z, style=ub.Contour.BOTH)
            >>> few = ub.Figure().add_axes().contour(x, y, z, levels=[0.25, 0.5, 0.75])
        """
        with api_call():
            context = "Axes.contour()"
            check_floats_type(context, "x", x)
            check_floats_type(context, "y", y)
            counted = _is_count(levels)
            if not counted and levels is not AUTO:
                check_floats_type(context, "levels", levels)
            drawn = instance(context, "style", style, Contour, _CONTOUR_HINT)
            native_cmap, kind, base, middle = native_color_scale(context, cmap, scale, center)
            spec = _color_argument(context, color)
            shows = _args.boolean(context, "annotate", annotate)
            checked = (
                None
                if format is AUTO
                else _ticks.native(instance(context, "format", format, TickFormat, _FORMAT_HINT))
            )
            count = _level_count(context, levels) if counted else None
            _numeric_cells(context, self, time=False)
            xs, ys = floats(context, "x", x), floats(context, "y", y)
            values = matrix(
                context,
                "z",
                z,
                (len(ys), len(xs)),
                lengths="(len(y), len(x))",
                hint="pass one row of z per y coordinate, each with one value per x coordinate.",
            )
            given: int | FloatArray | None = count
            if count is None and levels is not AUTO:
                given = floats(context, "levels", levels)
            self._native.contour(
                self._target,
                self._col,
                xs,
                ys,
                values,
                given,
                drawn.value,
                native_cmap,
                kind,
                base,
                middle,
                spec,
                shows,
                checked,
            )
            return self


def _is_count(levels: object) -> bool:
    """Return whether a ``levels`` argument is a count: an int or NumPy integer, not a ``bool``."""
    return not isinstance(levels, bool | np.bool_) and isinstance(levels, int | np.integer)


def _level_count(context: str, levels: object) -> int:
    """Check a count of levels, from 1 to 64, and return it."""
    count = integer(context, "levels", levels)
    if not 1 <= count <= _umber.MAX_LEVELS:
        raise new_error(
            StyleError,
            context,
            f"levels is {count}; a count of levels must be between 1 and {_umber.MAX_LEVELS}.",
            parameter="levels",
            hint="ask for a few levels, such as levels=7, or pass the level values.",
        )
    return count


def _render_argument(context: str, value: object) -> int | None:
    """Check a ``render`` argument and return its native form: ``None`` for ``AUTO``."""
    if value is AUTO:
        return None
    if not isinstance(value, Render):
        raise new_error(
            TypeError,
            context,
            f"render is {type_name(value)}; render must be a Render or ub.AUTO.",
            parameter="render",
            hint=_RENDER_HINT,
        )
    return int(value.value)


def _color_argument(context: str, value: object) -> tuple[int, int, float]:
    """Check a ``color`` argument and return its native form."""
    return _color.native(instance(context, "color", value, Color, _COLOR_HINT))


def _size_argument(context: str, name: str, value: object) -> tuple[int, float]:
    """Check a ``width`` or ``size`` argument and return its native form."""
    return _size.native(instance(context, name, value, Size, _MARK_SIZE_HINT))


def _value_labels(
    context: str, annotate: object, format: object, size: object
) -> tuple[bool, tuple[int, int, str, str] | None, tuple[int, float]]:
    """Check the value label arguments of a bar method and return their native form.

    A ``format`` of ``AUTO`` is ``None``: the tick format of the value axis.
    """
    shows = _args.boolean(context, "annotate", annotate)
    checked = (
        None
        if format is AUTO
        else _ticks.native(instance(context, "format", format, TickFormat, _FORMAT_HINT))
    )
    return shows, checked, _size.native(instance(context, "size", size, Size, _SIZE_HINT))


def _check_coordinate_type(context: str, scale: Scale[Any], value: object) -> None:
    """Check the type of one coordinate of ``at`` for an axis of ``scale``, converting nothing."""
    if scale == _CATEGORY:
        _args.require_text(context, "at", value)
    elif scale == _TIME:
        _time.check_instant_type(context, "at", value)
    else:
        real(context, "at", value)


def _coordinate(context: str, scale: Scale[Any], value: object) -> float | str | int:
    """Convert one coordinate of ``at``, of a checked type, to its native form for ``scale``.

    The form is a category name, a time in microseconds, or a ``float``.
    """
    if scale == _CATEGORY:
        return _args.checked_text(context, "at", "at", cast(str, value), DataError)
    if scale == _TIME:
        return _time.instant(context, "at", value)
    return real(context, "at", value)


def _offset_argument(context: str, value: object) -> tuple[float, float]:
    """Check an ``offset`` argument, a pair of numbers, and return it as two floats."""
    pair = _pair(value)
    if pair is None:
        message = f"offset is {_shown_pair(value)}; offset must be a pair (dx, dy) of points."
        raise new_error(TypeError, context, message, parameter="offset", hint=_OFFSET_HINT)
    dx, dy = pair
    return real(context, "offset", dx), real(context, "offset", dy)


def _pair(value: object) -> tuple[object, object] | None:
    """Return ``value`` when it is a tuple of two items, else ``None``."""
    if not isinstance(value, tuple):
        return None
    items = cast(tuple[object, ...], value)
    if len(items) != 2:
        return None
    return items[0], items[1]


def _shown_pair(value: object) -> str:
    """Name a value that is not a pair in an error message: its type, or a tuple's length."""
    if isinstance(value, tuple):
        return f"a tuple of {len(cast(tuple[object, ...], value))} values"
    return type_name(value)


def _holds_edges(bins: object) -> bool:
    """Whether a ``bins`` argument is a sequence or an array of edges, not a bin count.

    Text and single numbers, ``bool`` and NumPy scalars included, are not: they go to the
    whole-number check, which names the accepted type.
    """
    if is_text_or_buffer(bins) or isinstance(bins, numbers.Number | np.generic):
        return False
    return hasattr(bins, "__array__") or isinstance(bins, Sequence)


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
    register_twin: Callable[[_umber.TwinAxes, Scale[X], Scale[float]], Axes[X, float]],
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
        register_twin: The figure-owned method that attaches and registers a native twin.

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
    object.__setattr__(axes, "_register_twin", register_twin)
    object.__setattr__(axes, "_row", row)
    object.__setattr__(axes, "_target", row)
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


def _event_error(context: str, error: TypeError | UmberError) -> TypeError | UmberError:
    """Keep nested conversion messages while highlighting the positions argument."""
    info = details(error)
    hint = info[1] if info is not None else "pass one group of numbers or times per row."
    message = str(error).removeprefix(f"{context}: ")
    return new_error(type(error), context, message, parameter="positions", hint=hint)
