"""Pie panels: circular parts of a whole, without Cartesian axes."""

from collections.abc import Sequence
from typing import TYPE_CHECKING, Never, Self

import umber
from umber import _args, _color, _size, _ticks, _umber, _values
from umber._axes import Axes
from umber._color import Color
from umber._enums import AUTO, Auto, LegendPosition, LegendStyle
from umber._errors import UsageError, new_error
from umber._report import api_call
from umber._size import TEXT_SIZE_HINT, Size
from umber._ticks import TickFormat
from umber._types import Values


class PieAxes:
    """A circular panel with ``pie``, ``title`` and ``legend`` methods only.

    Create it with ``Figure.add_pie_axes()`` or ``ub.pie()``. The panel belongs to its
    ``figure``, which renders and saves it. A panel holds at most one pie. Failed calls
    leave it unchanged. Cartesian methods are unavailable.

    Example:
        >>> import umber as ub
        >>> ax = ub.Figure().add_pie_axes().pie(["North", "South"], [3, 1])
        >>> ax.title("Orders").figure.save_svg("orders.svg")
    """

    __slots__ = ("_col", "_figure", "_native", "_row")
    _figure: "umber.Figure"
    _native: _umber.Figure
    _row: int
    _col: int

    def __init__(self) -> None:
        """Refuse construction; use ``Figure.add_pie_axes()``."""
        with api_call():
            raise new_error(
                TypeError,
                "PieAxes()",
                "PieAxes has no public constructor; panels belong to a figure.",
                parameter=None,
                hint="call ub.Figure().add_pie_axes().",
            )

    @property
    def figure(self) -> "umber.Figure":
        """The figure that renders and saves this panel.

        Example:
            >>> import umber as ub
            >>> fig = ub.Figure()
            >>> fig.add_pie_axes().figure is fig
            True
        """
        return self._figure

    # Static callers see no Cartesian methods; dynamic callers get a chart-specific error.
    if not TYPE_CHECKING:

        def __getattr__(self, name: str) -> Never:
            if not name.startswith("_") and callable(getattr(Axes, name, None)):
                with api_call():
                    raise new_error(
                        UsageError,
                        f"PieAxes.{name}()",
                        "this panel has no Cartesian axes; use pie, title or legend.",
                        parameter=None,
                        hint="use Figure.add_axes() for Cartesian marks and axis settings.",
                    )
            raise AttributeError(name)

    def pie(
        self,
        categories: Values[str],
        values: Values[float],
        *,
        hole: float = 0.0,
        annotate: bool = False,
        format: TickFormat | Auto = AUTO,
        colors: Sequence[Color] | Auto = AUTO,
        size: Size = Size.AUTO,
    ) -> Self:
        """Draw this panel's pie, or a donut when ``hole`` is positive.

        Slices start at twelve o'clock, clockwise in input order. Names fit inside their
        slices or move outside with leader lines. Outside labels keep their vertical order.
        The default legend style is ``DIRECT``, regardless of the theme. ``HEADER`` and
        ``CHIPS`` replace direct names with a panel legend; ``NONE`` hides names.

        Args:
            categories: Unique slice names, one per value.
            values: Finite nonnegative values, with at least one positive value. Zero
                values draw no slice; their names appear only in a legend.
            hole: Inner radius as a fraction of outer radius, from 0 inclusive to 1 exclusive.
            annotate: Write each share under its name inside, or alongside it outside.
                Shares remain visible when a legend replaces the names.
            format: Share format; ``AUTO`` uses percent with no decimal places.
            colors: One Color per slice. ``AUTO`` uses the theme palette without cycling;
                more slices than palette colors need explicit colors.
            size: Direct label size: ``Size.AUTO`` follows the body's theme size,
                ``Size.times(k)`` scales it and ``Size.pt(p)`` uses points.

        Returns:
            This pie panel.

        Raises:
            TypeError: If an argument has the wrong type.
            DataError: If names repeat, lengths differ, or values are invalid or all zero.
            StyleError: If the hole or colors are invalid, or labels cannot fit at render.
            UsageError: If this panel already holds a pie.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_pie_axes()
            >>> ax = ax.pie(["Web", "App", "Other"], [50, 35, 15], hole=0.5, annotate=True)
        """
        with api_call():
            context = "PieAxes.pie()"
            _values.check_names_type(context, "categories", categories)
            _values.check_floats_type(context, "values", values)
            inner = _args.real(context, "hole", hole)
            notes = _args.boolean(context, "annotate", annotate)
            if format is not AUTO:
                _args.instance(
                    context,
                    "format",
                    format,
                    TickFormat,
                    "use a TickFormat or ub.AUTO for whole percentages.",
                )
            palette = None if colors is AUTO else _color.native_sequence(context, colors)
            _args.instance(context, "size", size, Size, TEXT_SIZE_HINT)
            names = _values.names(context, "categories", categories)
            numbers = _values.floats(context, "values", values)
            self._native.pie(
                self._row,
                self._col,
                names,
                numbers,
                inner,
                notes,
                None if format is AUTO else _ticks.native(format),
                palette,
                _size.native(size),
            )
            return self

    def title(self, text: str, *, size: Size = Size.AUTO) -> Self:
        """Set the title above this pie panel.

        Args:
            text: Title text, at most 1,000 characters; empty removes it.
            size: ``Size.AUTO`` follows the theme, ``Size.times(k)`` multiplies the scaled
                title size and ``Size.pt(p)`` uses points without font scaling.

        Returns:
            This pie panel.

        Raises:
            TypeError: If text or size has the wrong type.
            StyleError: If text is too long or cannot be displayed.

        Example:
            >>> import umber as ub
            >>> ax = ub.pie(["A", "B"], [1, 2]).title("Parts")
        """
        with api_call():
            context = "PieAxes.title()"
            _args.require_text(context, "text", text)
            _args.instance(context, "size", size, Size, TEXT_SIZE_HINT)
            self._native.pie_title(
                self._row, self._col, _args.text(context, "text", text), _size.native(size)
            )
            return self

    def legend(
        self, style: LegendStyle, *, position: LegendPosition | Auto = AUTO, size: Size = Size.AUTO
    ) -> Self:
        """Set the panel's legend, which never joins a merged figure legend.

        Args:
            style: ``DIRECT`` labels slices; ``HEADER`` and ``CHIPS`` replace direct names
                with a legend including zero values; ``NONE`` hides names.
            position: ``AUTO`` follows the theme. An explicit position requires a style
                other than ``DIRECT``.
            size: Legend text size, independent of other panels' legend sizes.

        Returns:
            This pie panel.

        Raises:
            TypeError: If an argument has the wrong type.
            UsageError: If ``DIRECT`` is given an explicit position.

        Example:
            >>> import umber as ub
            >>> ax = ub.pie(["A", "B"], [1, 2]).legend(ub.LegendStyle.CHIPS)
        """
        with api_call():
            context = "PieAxes.legend()"
            _args.instance(
                context,
                "style",
                style,
                LegendStyle,
                "use LegendStyle.DIRECT, HEADER, CHIPS or NONE.",
            )
            if position is not AUTO:
                _args.instance(
                    context,
                    "position",
                    position,
                    LegendPosition,
                    "use a LegendPosition or ub.AUTO.",
                )
            _args.instance(context, "size", size, Size, TEXT_SIZE_HINT)
            self._native.pie_legend(
                self._row,
                self._col,
                style.value,
                None if position is AUTO else position.value,
                _size.native(size),
            )
            return self


def attach(figure: "umber.Figure", native: _umber.Figure, row: int, col: int) -> PieAxes:
    """Attach the Python handle after the native circular panel exists."""
    axes = object.__new__(PieAxes)
    object.__setattr__(axes, "_figure", figure)
    object.__setattr__(axes, "_native", native)
    object.__setattr__(axes, "_row", row)
    object.__setattr__(axes, "_col", col)
    return axes
