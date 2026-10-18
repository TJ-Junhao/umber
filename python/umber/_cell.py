"""An immutable grid position attached to a figure."""

from typing import Self, final

import umber
from umber._args import instance
from umber._axes import Axes, X, Y
from umber._errors import new_error
from umber._report import api_call
from umber._scale import Scale


@final
class Cell:
    """An immutable grid position yielded by ``Figure.cells()``.

    A cell stays attached to its figure. Adding axes occupies that cell; iterating the cells
    again includes occupied positions, and adding axes there raises ``UsageError``.

    Example:
        >>> import umber as ub
        >>> fig = ub.Figure(cols=2)
        >>> cell = next(fig.cells())
        >>> cell
        Cell(row=0, col=0)
        >>> ax = cell.add_axes().line([0, 1], [1, 2])
    """

    __slots__ = ("_col", "_figure", "_row")
    _figure: "umber.Figure"
    _row: int
    _col: int

    def __init__(self) -> None:
        """Refuse construction; cells are obtained with ``Figure.cells()``."""
        with api_call():
            raise new_error(
                TypeError,
                "Cell()",
                "Cell has no public constructor; cells belong to a figure.",
                parameter=None,
                hint="iterate over a Figure's cells: ub.Figure().cells().",
            )

    @property
    def row(self) -> int:
        """The zero-based row of this cell.

        Example:
            >>> import umber as ub
            >>> next(ub.Figure().cells()).row
            0
        """
        return self._row

    @property
    def col(self) -> int:
        """The zero-based column of this cell.

        Example:
            >>> import umber as ub
            >>> [cell.col for cell in ub.Figure(cols=2).cells()]
            [0, 1]
        """
        return self._col

    def add_axes(
        self,
        *,
        # mypy rejects a concrete default for a TypeVar that has a default; pyright accepts it.
        x: Scale[X] = Scale.linear(),  # type: ignore[assignment]
        # mypy rejects a concrete default for a TypeVar that has a default; pyright accepts it.
        y: Scale[Y] = Scale.linear(),  # type: ignore[assignment]
    ) -> Axes[X, Y]:
        """Add axes at this cell of its figure and return them.

        Args:
            x: The x scale: numeric, category or time.
            y: The y scale, numeric, category or time; a category y axis makes horizontal bars.

        Returns:
            The new axes, occupying this cell only.

        Raises:
            TypeError: If either scale has the wrong type.
            UsageError: If the cell is occupied, or the scales conflict with a shared group.
                These errors retain ``Figure.add_axes()``.

        Example:
            >>> import umber as ub
            >>> fig = ub.Figure(cols=2)
            >>> cell = next(fig.cells())
            >>> ax = cell.add_axes(x=ub.Scale.category()).bar(["A", "B"], [3, 5])
            >>> fig.axes_at(col=0, x=ub.Scale.category()) is ax
            True
        """
        with api_call(parameter_map={"row": "self", "col": "self"}):
            instance(
                "Cell.add_axes()",
                "x",
                x,
                Scale,
                "use Scale.linear(), Scale.log(), Scale.category() or Scale.time().",
            )
            instance(
                "Cell.add_axes()",
                "y",
                y,
                Scale,
                "use Scale.linear(), Scale.log(), Scale.category() or Scale.time().",
            )
            return self._figure.add_axes(row=self._row, col=self._col, x=x, y=y)

    def __repr__(self) -> str:
        return f"Cell(row={self._row}, col={self._col})"

    def __copy__(self) -> Self:
        return self

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        return self

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Cell is immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("a Cell is immutable")


def at(figure: "umber.Figure", row: int, col: int) -> Cell:
    """Attach a cell to a figure after the native grid has checked its position."""
    cell = object.__new__(Cell)
    object.__setattr__(cell, "_figure", figure)
    object.__setattr__(cell, "_row", row)
    object.__setattr__(cell, "_col", col)
    return cell
