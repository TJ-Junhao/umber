"""``Figure``: a chart's canvas, which holds one axes and renders to PNG and SVG."""

import os
from pathlib import Path
from typing import Any, Final, Never, Self, final

from umber import _args, _axes, _scale, _theme, _umber
from umber._args import instance, integer, real
from umber._axes import Axes, X
from umber._enums import SvgText
from umber._errors import UsageError, new_error
from umber._report import api_call
from umber._scale import Scale
from umber._theme import Theme

_THEME_HINT: Final = "use Theme.PIGMENT, Theme.GRAPHITE or Theme.MIDNIGHT."
_X_SCALE_HINT: Final = "use Scale.linear(), Scale.log() or Scale.category()."
_Y_SCALE_HINT: Final = "use Scale.linear() or Scale.log()."
_SVG_TEXT_HINT: Final = "use SvgText.OUTLINES or SvgText.EDITABLE."


@final
class Figure:
    """A figure: the canvas of one chart, with a size, a theme and one axes.

    Create a figure, add its axes with ``add_axes()``, draw on the axes, then render the
    figure with ``to_png()`` or ``to_svg()``, or write it with ``save_png()`` or
    ``save_svg()``. A figure without axes renders its background only.

    ``copy.copy()`` and ``copy.deepcopy()`` return an independent figure with the same size,
    theme, axes and marks; changing one leaves the other unchanged. The ``axes`` property
    reaches a figure's axes, including those of a copy. Figures cannot be pickled.

    Example:
        >>> import umber as ub
        >>> fig = ub.Figure(6, 4, theme=ub.Theme.GRAPHITE)
        >>> ax = fig.add_axes().line([0, 1, 2], [1.0, 3.0, 2.0]).title("Signal")
        >>> fig.save_svg("signal.svg")
    """

    # ``_axes`` is set only once the native figure holds the axes.
    __slots__ = ("_axes", "_native")
    _native: _umber.Figure
    _axes: Axes[Any, Any] | None

    def __init__(
        self, width: float = 6.0, height: float = 4.0, *, theme: Theme = Theme.PIGMENT
    ) -> None:
        """Create an empty figure of ``width`` by ``height`` inches.

        Args:
            width: The width in inches; finite, above 0 and at most 200.
            height: The height in inches; finite, above 0 and at most 200.
            theme: The theme that styles the figure.

        Raises:
            TypeError: If ``width`` or ``height`` is not a number, or ``theme`` is not a
                ``Theme``.
            StyleError: If ``width`` or ``height`` is out of range.

        Example:
            >>> import umber as ub
            >>> fig = ub.Figure(8, 4.5, theme=ub.Theme.MIDNIGHT)
        """
        with api_call():
            inches_wide = real("Figure()", "width", width)
            inches_high = real("Figure()", "height", height)
            instance("Figure()", "theme", theme, Theme, _THEME_HINT)
            self._native = _umber.Figure(inches_wide, inches_high, _theme.native(theme))
            self._axes = None

    @property
    def axes(self) -> Axes[Any, Any]:
        """The axes added with ``add_axes()``; use it to keep drawing on a copied figure.

        Raises:
            UsageError: If the figure has no axes yet.

        Example:
            >>> import copy
            >>> import umber as ub
            >>> fig = ub.Figure()
            >>> ax = fig.add_axes().line([0, 1], [1, 2])
            >>> fig.axes is ax
            True
            >>> variant = copy.copy(fig)
            >>> variant.axes.title("Variant").figure is variant
            True
        """
        with api_call():
            if self._axes is None:
                raise new_error(
                    UsageError,
                    "Figure.axes",
                    "the figure has no axes; call add_axes() first.",
                    parameter=None,
                    hint="call fig.add_axes() before drawing.",
                )
            return self._axes

    def add_axes(
        self,
        *,
        x: Scale[X] = Scale.linear(),  # type: ignore[assignment]  # noqa: B008
        y: Scale[float] = Scale.linear(),  # noqa: B008
    ) -> Axes[X, float]:
        """Add the axes to this figure and return them; a figure holds one axes.

        Args:
            x: The x scale. ``Scale.linear()`` and ``Scale.log()`` make a numeric x axis;
                ``Scale.category()`` makes an axis of names, as bar charts need.
            y: The y scale: ``Scale.linear()`` or ``Scale.log()``. The y axis is always
                numeric.

        Returns:
            The new axes.

        Raises:
            TypeError: If ``x`` or ``y`` is not a ``Scale``.
            UsageError: If ``y`` is ``Scale.category()``, or the figure already has axes.

        Example:
            >>> import umber as ub
            >>> ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.log())
        """
        with api_call():
            context = "Figure.add_axes()"
            instance(context, "x", x, Scale, _X_SCALE_HINT)
            instance(context, "y", y, Scale, _Y_SCALE_HINT)
            # The core refuses a second axes, then a category y scale.
            self._native.add_axes(*_scale.native(x), *_scale.native(y))
            axes = _axes.attach(self, self._native, x, y)
            self._axes = axes
            return axes

    def to_png(self, *, dpi: int = 200) -> bytes:
        """Render the figure as a PNG image.

        Args:
            dpi: The resolution in pixels per inch, from 10 to 2400. The image is the figure
                size times ``dpi`` pixels.

        Returns:
            The PNG file contents.

        Raises:
            TypeError: If ``dpi`` is not an int.
            StyleError: If ``dpi`` is out of range, or the image would be too large.

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
            StyleError: If ``dpi`` is out of range, or the image would be too large.
            OSError: If the file cannot be written.

        Example:
            >>> import umber as ub
            >>> ub.Figure(3, 2).save_png("empty.png", dpi=100)
        """
        with api_call():
            target = _args.path("Figure.save_png()", "path", path)
            resolution = integer("Figure.save_png()", "dpi", dpi)
            Path(target).write_bytes(self._native.png_for_file(resolution))

    def save_svg(self, path: str | os.PathLike[str], *, text: SvgText = SvgText.OUTLINES) -> None:
        """Render the figure as an SVG document and write it to ``path`` in UTF-8.

        Args:
            path: The file to write; an existing file is replaced.
            text: How text is written; see ``to_svg()``.

        Raises:
            TypeError: If ``path`` is not a ``str`` or path-like object, or ``text`` is not
                an ``SvgText``.
            OSError: If the file cannot be written.

        Example:
            >>> import umber as ub
            >>> ub.Figure(3, 2).save_svg("empty.svg")
        """
        with api_call():
            target = _args.path("Figure.save_svg()", "path", path)
            mode = instance("Figure.save_svg()", "text", text, SvgText, _SVG_TEXT_HINT)
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
        clone._axes = None
        if self._axes is not None:
            x, y = _axes.scales(self._axes)
            clone._axes = _axes.attach(clone, clone._native, x, y)
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
