"""Positions that are not values of an axis: ``InPlot`` and ``Between``."""

from collections.abc import Callable
from typing import Self, final

from umber import _args, _umber
from umber._report import api_call


@final
class InPlot:
    """An immutable position in the plot, given as fractions of the plot area.

    ``InPlot(0, 0)`` is the lower left corner of the plot area and ``InPlot(1, 1)`` the upper
    right, whatever the scales and limits of the axes are, so text placed there stays in its
    corner when the data range changes. Pass it as ``at`` to ``Axes.text()``, or as
    ``text_at`` to ``Axes.annotate()``. Positions compare equal when both fractions are equal.

    Example:
        >>> import umber as ub
        >>> ub.InPlot(0.02, 0.96)
        InPlot(0.02, 0.96)
        >>> ub.InPlot(1, 0) == ub.InPlot(1.0, 0.0)
        True
    """

    __slots__ = ("_x", "_y")
    _x: float
    _y: float

    def __init__(self, x: float, y: float) -> None:
        """Make a position from its fractions of the plot's width and height.

        Args:
            x: The fraction of the plot's width, from the left edge; finite, from 0 to 1.
            y: The fraction of the plot's height, from the bottom edge; finite, from 0 to 1.

        Raises:
            TypeError: If ``x`` or ``y`` is not a number.
            DataError: If ``x`` or ``y`` is not finite or lies outside 0 to 1. Text placed
                outside the plot would leave it; use ``Figure.caption()`` for that.

        Example:
            >>> import umber as ub
            >>> ub.InPlot(0.5, 0.5).x
            0.5
        """
        with api_call():
            fraction_x = _args.real("InPlot()", "x", x)
            fraction_y = _args.real("InPlot()", "y", y)
            _umber.check_in_plot(fraction_x, fraction_y)
            # ``+ 0.0`` turns -0.0 into 0.0, so equal positions have equal reprs.
            object.__setattr__(self, "_x", fraction_x + 0.0)
            object.__setattr__(self, "_y", fraction_y + 0.0)

    @property
    def x(self) -> float:
        """The fraction of the plot's width, from the left edge.

        Example:
            >>> import umber as ub
            >>> ub.InPlot(0.25, 0.75).x
            0.25
        """
        return self._x

    @property
    def y(self) -> float:
        """The fraction of the plot's height, from the bottom edge.

        Example:
            >>> import umber as ub
            >>> ub.InPlot(0.25, 0.75).y
            0.75
        """
        return self._y

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, InPlot):
            return NotImplemented
        return (self._x, self._y) == (other._x, other._y)

    def __hash__(self) -> int:
        return hash((self._x, self._y))

    def __repr__(self) -> str:
        return f"InPlot({_shown(self._x)}, {_shown(self._y)})"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(self) -> tuple[Callable[[float, float], "InPlot"], tuple[float, float]]:
        return InPlot, (self._x, self._y)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("an InPlot is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("an InPlot is immutable")


def _shown(fraction: float) -> str:
    """Return ``fraction`` as a short number: ``1`` and ``0`` for the whole ones."""
    return repr(fraction).removesuffix(".0")


@final
class Between:
    r"""An immutable position halfway between two neighbouring categories of a category axis.

    ``Between("2013", "2014")`` is the point halfway between the centres of the categories
    ``"2013"`` and ``"2014"``: where a divider between them goes. Pass it as ``x`` to
    ``Axes.vline()`` on a category x axis, as ``y`` to ``Axes.hline()`` on a category y axis,
    or as both ends of ``Axes.vspan()`` or ``Axes.hspan()`` for a span that covers whole
    categories. The two names may come in either order; they must be categories of the axis
    and neighbours there, which is checked when the figure is rendered, since a mark added later
    may bring them. Positions compare equal when both names are equal, in the same order.

    Example:
        >>> import umber as ub
        >>> ub.Between("2013", "2014")
        Between('2013', '2014')
        >>> ax = ub.Figure().add_axes(x=ub.Scale.category())
        >>> ax = ax.bar(["2012", "2013", "2014", "2015"], [3, 4, 6, 5])
        >>> ax = ax.vline(ub.Between("2013", "2014"), dash=ub.Dash.DASHED)
    """

    __slots__ = ("_first", "_second")
    _first: str
    _second: str

    def __init__(self, first: str, second: str) -> None:
        r"""Make the position between the categories ``first`` and ``second``.

        Args:
            first: The name of one category; it may hold ``"\n"``, as category names may.
            second: The name of a neighbouring category, other than ``first``.

        Raises:
            TypeError: If ``first`` or ``second`` is not a ``str``.
            DataError: If a name holds a control character other than ``"\n"``, or a line or
                paragraph separator.
            StyleError: If a name is longer than the longest text.
            UsageError: If ``first`` and ``second`` are the same name.

        Example:
            >>> import umber as ub
            >>> ub.Between("2026\n(to Oct)", "2025").first
            '2026\n(to Oct)'
        """
        with api_call():
            context = "Between()"
            _args.require_text(context, "first", first)
            _args.require_text(context, "second", second)
            one = _args.category_name(context, "first", "first", first)
            other = _args.category_name(context, "second", "second", second)
            _umber.check_between(one, other)
            object.__setattr__(self, "_first", one)
            object.__setattr__(self, "_second", other)

    @property
    def first(self) -> str:
        """The name of the first category.

        Example:
            >>> import umber as ub
            >>> ub.Between("Q1", "Q2").first
            'Q1'
        """
        return self._first

    @property
    def second(self) -> str:
        """The name of the second category.

        Example:
            >>> import umber as ub
            >>> ub.Between("Q1", "Q2").second
            'Q2'
        """
        return self._second

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Between):
            return NotImplemented
        return (self._first, self._second) == (other._first, other._second)

    def __hash__(self) -> int:
        return hash((self._first, self._second))

    def __repr__(self) -> str:
        return f"Between({self._first!r}, {self._second!r})"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __reduce__(self) -> tuple[Callable[[str, str], "Between"], tuple[str, str]]:
        return Between, (self._first, self._second)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Between is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Between is immutable")
