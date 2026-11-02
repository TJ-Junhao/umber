"""``InPlot``: a position given as fractions of the plot area."""

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
