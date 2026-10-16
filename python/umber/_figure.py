"""``Figure``: a canvas with a grid of axes, rendered to PNG and SVG."""

import os
from collections.abc import Iterator
from pathlib import Path
from typing import Any, Final, Never, Self, final

from umber import _args, _axes, _cell, _scale, _theme, _umber
from umber._args import instance, integer, real
from umber._axes import Axes, V, X
from umber._cell import Cell
from umber._enums import AUTO, Auto, Legend, Share, SvgText
from umber._errors import new_error
from umber._report import api_call
from umber._scale import Scale
from umber._theme import Theme

_THEME_HINT: Final = "use Theme.PIGMENT, Theme.GRAPHITE or Theme.MIDNIGHT."
_X_SCALE_HINT: Final = "use Scale.linear(), Scale.log(), Scale.category() or Scale.time()."
_Y_SCALE_HINT: Final = "use Scale.linear(), Scale.log() or Scale.time()."
_SVG_TEXT_HINT: Final = "use SvgText.OUTLINES or SvgText.EDITABLE."
_SHARE_HINT: Final = "use Share.NONE, Share.ROW, Share.COL or Share.ALL."
_LEGEND_HINT: Final = "use Legend.MERGED or Legend.PER_PANEL."


def _inches(context: str, name: str, value: float | Auto) -> float | None:
    """Convert a figure size, leaving AUTO for the native grid to resolve."""
    if value is AUTO:
        return None
    return real(
        context,
        name,
        value,
        expected="a float or AUTO",
        hint="pass a number such as 1.5, or ub.AUTO for the default size.",
    )


def _index(context: str, name: str, value: int | Auto) -> tuple[int, int]:
    """Convert a grid index: kind 0 is AUTO, kind 1 is an explicit integer."""
    if value is AUTO:
        return (0, 0)
    return (
        1,
        integer(
            context,
            name,
            value,
            expected="an int or AUTO",
            hint="pass a whole number such as 0, or ub.AUTO for a single row or column.",
        ),
    )


@final
class Figure:
    """A canvas with a size, a theme and a grid of axes.

    Create a figure, add panels with ``add_axes()``, draw on their axes, then render with
    ``to_png()`` or ``to_svg()``, or write with ``save_png()`` or ``save_svg()``. Empty cells
    show the background. Positions count from zero; an axes may span several cells.

    ``copy.copy()`` and ``copy.deepcopy()`` return an independent figure with the same size,
    theme, grid, axes and marks; changing one leaves the other unchanged. ``axes_at()``
    reaches a panel, including one in a copy, with the scales used to create it as a type
    witness. Figures cannot be pickled.

    Example:
        >>> import umber as ub
        >>> fig = ub.Figure(6, 4, theme=ub.Theme.GRAPHITE)
        >>> ax = fig.add_axes().line([0, 1, 2], [1.0, 3.0, 2.0]).title("Signal")
        >>> fig.save_svg("signal.svg")
    """

    # Python state is stored only after the native call succeeds.
    __slots__ = ("_axes", "_cols", "_native", "_rows")
    _native: _umber.Figure
    _axes: dict[tuple[int, int], Axes[Any, Any]]
    _rows: int
    _cols: int

    def __init__(
        self,
        width: float | Auto = AUTO,
        height: float | Auto = AUTO,
        *,
        rows: int = 1,
        cols: int = 1,
        share_x: Share = Share.NONE,
        share_y: Share = Share.NONE,
        legend: Legend = Legend.MERGED,
        theme: Theme = Theme.PIGMENT,
    ) -> None:
        """Create an empty grid of ``rows`` by ``cols`` cells.

        Args:
            width: The whole figure's width in inches; finite, above 0 and at most 200.
                ``AUTO`` chooses 6 inches per column.
            height: The whole figure's height in inches; finite, above 0 and at most 200.
                ``AUTO`` chooses 4 inches per row.
            rows: The number of rows, from 1 to 32.
            cols: The number of columns, from 1 to 32.
            share_x: Which panels share their x scale, limits, ticks and tick format.
            share_y: Which panels share their y scale, limits, ticks and tick format.
            legend: One merged legend or a separate legend per panel.
            theme: The theme that styles the figure.

        Raises:
            TypeError: If an argument has the wrong type; bool is not a number or an index.
            StyleError: If ``width``, ``height``, ``rows`` or ``cols`` is out of range.

        Example:
            >>> import umber as ub
            >>> fig = ub.Figure(rows=2, cols=2, share_x=ub.Share.ALL)
            >>> ax = fig.add_axes(row=0, col=0)
        """
        with api_call():
            context = "Figure()"
            inches_wide = _inches(context, "width", width)
            inches_high = _inches(context, "height", height)
            row_count = integer(context, "rows", rows)
            col_count = integer(context, "cols", cols)
            instance(context, "share_x", share_x, Share, _SHARE_HINT)
            instance(context, "share_y", share_y, Share, _SHARE_HINT)
            instance(context, "legend", legend, Legend, _LEGEND_HINT)
            instance(context, "theme", theme, Theme, _THEME_HINT)
            self._native = _umber.Figure(
                inches_wide,
                inches_high,
                _theme.native(theme),
                row_count,
                col_count,
                share_x.value,
                share_y.value,
                legend.value,
            )
            self._rows = row_count
            self._cols = col_count
            self._axes = {}

    def title(self, text: str) -> Self:
        """Set the figure title, shown above every panel and the merged legend.

        The figure title is 1.5 times the panel title size. Its alignment follows the
        theme's ``with_title_align()`` setting.

        Args:
            text: The title, at most 1,000 characters. An empty string removes it.

        Returns:
            This figure.

        Raises:
            TypeError: If ``text`` is not a ``str``.
            StyleError: If ``text`` is too long or holds a lone surrogate.

        Example:
            >>> import umber as ub
            >>> fig = ub.Figure(cols=2).title("Revenue")
            >>> ax = fig.add_axes(col=0).title("North")
        """
        with api_call():
            self._native.figure_title(_args.text("Figure.title()", "text", text))
            return self

    def subtitle(self, text: str) -> Self:
        """Set the figure subtitle, shown under its title and above every panel.

        Its alignment follows the figure title, as set by ``Theme.with_title_align()``.

        Args:
            text: The subtitle, at most 1,000 characters. An empty string removes it.

        Returns:
            This figure.

        Raises:
            TypeError: If ``text`` is not a ``str``.
            StyleError: If ``text`` is too long or holds a lone surrogate.

        Example:
            >>> import umber as ub
            >>> fig = ub.Figure().title("Revenue").subtitle("Per quarter")
            >>> fig.subtitle("Per year") is fig
            True
        """
        with api_call():
            self._native.figure_subtitle(_args.text("Figure.subtitle()", "text", text))
            return self

    def add_axes(
        self,
        *,
        row: int | Auto = AUTO,
        col: int | Auto = AUTO,
        row_span: int = 1,
        col_span: int = 1,
        # mypy rejects ``Scale[float]`` as the default of ``Scale[X]``; pyright accepts it.
        x: Scale[X] = Scale.linear(),  # type: ignore[assignment]  # noqa: B008
        y: Scale[V] = Scale.linear(),  # type: ignore[assignment]  # noqa: B008
    ) -> Axes[X, V]:
        """Add axes at a grid position and return them.

        Args:
            row: The zero-based top row; ``AUTO`` means 0 only on a one-row figure.
            col: The zero-based left column; ``AUTO`` means 0 only on a one-column figure.
            row_span: The number of rows occupied, at least 1.
            col_span: The number of columns occupied, at least 1.
            x: The x scale. ``Scale.linear()`` and ``Scale.log()`` make a numeric x axis;
                ``Scale.category()`` makes an axis of names, as bar charts need;
                ``Scale.time()`` makes an axis of times.
            y: The y scale: ``Scale.linear()``, ``Scale.log()`` or ``Scale.time()``.

        Returns:
            The new axes.

        Raises:
            TypeError: If an index, span or scale has the wrong type.
            UsageError: If a position is missing, out of range or occupied, a span is invalid,
                ``y`` is a category scale, or shared axes would have incompatible scales.

        Note:
            A wrong explicit annotation, such as ``Axes[str, float]`` on the result of
            ``add_axes()``, is accepted by mypy but rejected by pyright; the runtime still
            raises when a category-only method is called on numeric axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.log())
        """
        with api_call():
            context = "Figure.add_axes()"
            row_index = _index(context, "row", row)
            col_index = _index(context, "col", col)
            rows = integer(context, "row_span", row_span)
            cols = integer(context, "col_span", col_span)
            instance(context, "x", x, Scale, _X_SCALE_HINT)
            instance(context, "y", y, Scale, _Y_SCALE_HINT)
            top, left = self._native.add_axes(
                *row_index, *col_index, rows, cols, *_scale.native(x), *_scale.native(y)
            )
            axes = _axes.attach(self, self._native, x, y, top, left)
            self._axes[top, left] = axes
            return axes

    def axes_at(
        self,
        *,
        row: int | Auto = AUTO,
        col: int | Auto = AUTO,
        x: Scale[X] = Scale.linear(),  # type: ignore[assignment]  # noqa: B008
        y: Scale[V] = Scale.linear(),  # type: ignore[assignment]  # noqa: B008
    ) -> Axes[X, V]:
        """Return existing axes with the given position and scales, including those of a copy.

        Args:
            row: The zero-based top row; ``AUTO`` means 0 only on a one-row figure.
            col: The zero-based left column; ``AUTO`` means 0 only on a one-column figure.
            x: The x scale used to create the axes, including the log base. This type witness
                determines whether the result accepts numeric or category x values.
            y: The y scale used to create the axes, including the log base.

        Returns:
            The axes at this top-left cell; other cells covered by a span do not match.

        Raises:
            TypeError: If an index or scale has the wrong type.
            UsageError: If the position is missing, out of range or not a panel's top-left
                cell, or either scale differs from the one used at creation.

        Example:
            >>> import copy
            >>> import umber as ub
            >>> fig = ub.Figure(cols=2)
            >>> ax = fig.add_axes(col=1, x=ub.Scale.category()).bar(["A"], [2])
            >>> fig.axes_at(col=1, x=ub.Scale.category()) is ax
            True
            >>> twin = copy.copy(fig)
            >>> twin.axes_at(col=1, x=ub.Scale.category()).title("Variant").figure is twin
            True
        """
        with api_call():
            context = "Figure.axes_at()"
            row_index = _index(context, "row", row)
            col_index = _index(context, "col", col)
            instance(context, "x", x, Scale, _X_SCALE_HINT)
            instance(context, "y", y, Scale, _Y_SCALE_HINT)
            position = self._native.axes_at(
                *row_index, *col_index, *_scale.native(x), *_scale.native(y)
            )
            return self._axes[position]

    def cells(self, count: int | Auto = AUTO, /) -> Iterator[Cell]:
        """Iterate over grid cells in row-major order, whether occupied or empty.

        Args:
            count: How many cells to yield, from 1 to ``rows * cols``. ``AUTO`` yields all.

        Returns:
            An iterator of immutable cells, each able to add axes to this figure.

        Raises:
            TypeError: If ``count`` is neither an int nor ``AUTO``; bool is not accepted.
            StyleError: If ``count`` is outside the grid's capacity.

        Example:
            >>> import umber as ub
            >>> fig = ub.Figure(cols=2)
            >>> for cell, values in zip(fig.cells(), ([1, 3], [2, 4])):
            ...     ax = cell.add_axes().line([0, 1], values)
            >>> [(cell.row, cell.col) for cell in fig.cells()]
            [(0, 0), (0, 1)]
        """
        with api_call():
            total = (
                self._rows * self._cols
                if count is AUTO
                else integer("Figure.cells()", "count", count)
            )
            positions = _umber.grid_cells(self._rows, self._cols, total)
            return (_cell.at(self, row, col) for row, col in positions)

    def to_png(self, *, dpi: int = 200) -> bytes:
        """Render the figure as a PNG image.

        Args:
            dpi: The resolution in pixels per inch, from 10 to 2400. The image is the figure
                size times ``dpi`` pixels.

        Returns:
            The PNG file contents.

        Raises:
            TypeError: If ``dpi`` is not an int.
            StyleError: If ``dpi`` is out of range, the plot area or its x tick labels cannot
                fit the figure, or the image would be too large.

        Example:
            >>> import umber as ub
            >>> png = ub.Figure(3, 2).to_png(dpi=100)
            >>> png[1:4]
            b'PNG'
        """
        with api_call():
            resolution = integer("Figure.to_png()", "dpi", dpi)
            return self._native.to_png(resolution)

    def to_svg(self, *, text: SvgText = SvgText.OUTLINES) -> str:
        """Render the figure as an SVG document.

        Args:
            text: How text is written. ``SvgText.OUTLINES`` draws it as paths, which look the
                same everywhere; ``SvgText.EDITABLE`` keeps it as text elements.

        Returns:
            The SVG document.

        Raises:
            TypeError: If ``text`` is not an ``SvgText``.
            StyleError: If the plot area or its x tick labels cannot fit the figure.

        Example:
            >>> import umber as ub
            >>> svg = ub.Figure().to_svg(text=ub.SvgText.EDITABLE)
            >>> svg.startswith("<svg")
            True
        """
        with api_call():
            mode = instance("Figure.to_svg()", "text", text, SvgText, _SVG_TEXT_HINT)
            return self._native.to_svg(mode.value)

    def save_png(self, path: str | os.PathLike[str], *, dpi: int = 200) -> None:
        """Render the figure as a PNG image and write it to ``path``.

        Args:
            path: The file to write; an existing file is replaced.
            dpi: The resolution in pixels per inch, from 10 to 2400.

        Raises:
            TypeError: If ``path`` is not a ``str`` or path-like object, or ``dpi`` is not
                an int.
            StyleError: If ``dpi`` is out of range, the plot area or its x tick labels cannot
                fit the figure, or the image would be too large.
            OSError: If the file cannot be written.

        Example:
            >>> import umber as ub
            >>> ub.Figure(3, 2).save_png("empty.png", dpi=100)
        """
        with api_call():
            _args.require_path("Figure.save_png()", "path", path)
            resolution = integer("Figure.save_png()", "dpi", dpi)
            target = _args.path("Figure.save_png()", "path", path)
            Path(target).write_bytes(self._native.png_for_file(resolution))

    def save_svg(self, path: str | os.PathLike[str], *, text: SvgText = SvgText.OUTLINES) -> None:
        """Render the figure as an SVG document and write it to ``path`` in UTF-8.

        Args:
            path: The file to write; an existing file is replaced.
            text: How text is written; see ``to_svg()``.

        Raises:
            TypeError: If ``path`` is not a ``str`` or path-like object, or ``text`` is not
                an ``SvgText``.
            StyleError: If the plot area or its x tick labels cannot fit the figure.
            OSError: If the file cannot be written.

        Example:
            >>> import umber as ub
            >>> ub.Figure(3, 2).save_svg("empty.svg")
        """
        with api_call():
            _args.require_path("Figure.save_svg()", "path", path)
            mode = instance("Figure.save_svg()", "text", text, SvgText, _SVG_TEXT_HINT)
            target = _args.path("Figure.save_svg()", "path", path)
            svg = self._native.svg_for_file(mode.value)
            # ``newline=""`` writes the document's line ends as they are, on every platform.
            Path(target).write_text(svg, encoding="utf-8", newline="")

    def __copy__(self) -> Self:
        with api_call():
            return self._clone()

    def __deepcopy__(self, memo: dict[int, object]) -> Self:
        with api_call():
            return self._clone()

    def _clone(self) -> Self:
        """Return an independent figure: the native clone, with axes of its own."""
        clone = object.__new__(type(self))
        clone._native = self._native.clone()
        clone._rows = self._rows
        clone._cols = self._cols
        clone._axes = {}
        for (row, col), axes in self._axes.items():
            x, y = _axes.scales(axes)
            clone._axes[row, col] = _axes.attach(clone, clone._native, x, y, row, col)
        return clone

    def __reduce__(self) -> Never:
        with api_call():
            raise new_error(
                TypeError,
                "Figure",
                "a Figure cannot be pickled.",
                parameter=None,
                hint="save it with save_png() or save_svg(), or build it again.",
            )
