"""``Axes``: the plot area of a figure, with its marks, texts, limits and legend."""

from enum import Enum
from operator import attrgetter
from typing import TYPE_CHECKING, Any, Final, Generic, Never, Self, cast, final

import numpy as np
from typing_extensions import TypeVar

from umber import _args, _color, _umber
from umber._args import instance, real
from umber._color import Color
from umber._enums import Dash, LegendStyle, Marker
from umber._errors import new_error
from umber._report import api_call
from umber._scale import Scale
from umber._types import Values
from umber._values import FloatArray, axis_values, floats

if TYPE_CHECKING:
    from umber._figure import Figure

# Old-style, invariant type variables with defaults: ``Figure().add_axes()`` is
# ``Axes[float, float]`` without annotations, and ``Axes[str, float]`` is not assignable to
# ``Axes[float, float]``.
X = TypeVar("X", default=float)
Y = TypeVar("Y", default=float)

_CATEGORY: Final = Scale.category()
_COLOR_HINT: Final = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
_COPY_HINT: Final = "copy the figure instead: copy.copy(ax.figure)."
_PICKLE_HINT: Final = "save the figure with save_png() or save_svg(), or build it again."


def _members_hint(cls: type[Enum]) -> str:
    """Return the hint that lists every member of ``cls``, such as ``use Dash.SOLID, …``."""
    names = [f"{cls.__name__}.{member.name}" for member in cls]
    return f"use {', '.join(names[:-1])} or {names[-1]}."


_DASH_HINT: Final = _members_hint(Dash)
_MARKER_HINT: Final = _members_hint(Marker)
_LEGEND_HINT: Final = _members_hint(LegendStyle)


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
    _figure: "Figure"
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
    def figure(self) -> "Figure":
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
    ) -> Self:
        """Add a line through the points ``(x[i], y[i])``, in order.

        A NaN in ``x`` or ``y`` leaves a gap in the line.

        Args:
            x: The x values: numbers on a linear or log x axis, names on a category x axis.
            y: The y values, one per x value.
            label: The name shown in the legend; an empty string leaves the line out of it.
            color: The line color. ``Color.AUTO`` takes the next color of the theme palette.
            dash: How the line is stroked.

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
            name = _args.text(context, "label", label)
            spec = _color_argument(context, color)
            style = instance(context, "dash", dash, Dash, _DASH_HINT)
            xs = self._x_values(context, x)
            ys = floats(context, "y", y)
            self._native.line(xs, ys, name, spec, style.value)
            return self

    def scatter(
        self,
        x: Values[X],
        y: Values[Y],
        *,
        label: str = "",
        color: Color = Color.AUTO,
        marker: Marker = Marker.CIRCLE,
    ) -> Self:
        """Add a marker at each point ``(x[i], y[i])``.

        A point with a NaN coordinate is left out.

        Args:
            x: The x values: numbers on a linear or log x axis, names on a category x axis.
            y: The y values, one per x value.
            label: The name shown in the legend; an empty string leaves the series out of it.
            color: The marker color. ``Color.AUTO`` takes the next color of the theme palette.
            marker: The marker shape.

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
            name = _args.text(context, "label", label)
            spec = _color_argument(context, color)
            shape = instance(context, "marker", marker, Marker, _MARKER_HINT)
            xs = self._x_values(context, x)
            ys = floats(context, "y", y)
            self._native.scatter(xs, ys, name, spec, shape.value)
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


_SCALES: Final = attrgetter("_x", "_y")


def attach(figure: "Figure", native: _umber.Figure, x: Scale[X], y: Scale[float]) -> Axes[X, float]:
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
