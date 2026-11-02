"""Several figures in one PDF: ``to_pdf()`` and ``save_pdf()``."""

import os
from collections.abc import Sequence
from pathlib import Path
from typing import Any, Final, cast

from umber import _args, _umber
from umber._args import type_name
from umber._axes import Axes
from umber._errors import DataError, new_error
from umber._figure import Figure, native
from umber._pie import PieAxes
from umber._report import api_call

_FIGURES_HINT: Final = "pass a list of figures, such as [fig1, fig2]."
_SINGLE_HINT: Final = "use fig.save_pdf(path) for one figure, or wrap it in a list: [fig]."


def _native(context: str, figures: object) -> list[_umber.Figure]:
    """Check ``figures`` and return the native figure of each item, in order."""
    if isinstance(figures, Figure):
        raise new_error(
            TypeError,
            context,
            "figures is a Figure; figures must be a list or tuple of figures.",
            parameter="figures",
            hint=_SINGLE_HINT,
        )
    if isinstance(figures, str | bytes) or not isinstance(figures, Sequence):
        raise new_error(
            TypeError,
            context,
            f"figures is {type_name(figures)}; figures must be a list or tuple of figures.",
            parameter="figures",
            hint=_FIGURES_HINT,
        )
    items = cast(Sequence[object], figures)
    if len(items) == 0:
        raise new_error(
            DataError,
            context,
            "figures is empty; a PDF needs at least one page.",
            parameter="figures",
            hint="pass at least one figure.",
        )
    pages: list[_umber.Figure] = []
    for index, item in enumerate(items):
        figure = item.figure if isinstance(item, Axes | PieAxes) else item
        if not isinstance(figure, Figure):
            raise new_error(
                TypeError,
                context,
                f"figures[{index}] is {type_name(figure)}; each item must be a Figure, or an "
                "Axes or PieAxes of one.",
                parameter="figures",
                hint=_FIGURES_HINT,
            )
        pages.append(native(figure))
    return pages


def to_pdf(figures: Sequence[Figure | Axes[Any, Any] | PieAxes]) -> bytes:
    """Render several figures as one PDF with embedded fonts.

    Each figure is one page at its own size, in list order, and draws what ``Figure.to_pdf()``
    draws for it. A font used on several pages is embedded once. The same figure may appear
    more than once. Every figure is rendered before anything is returned.

    Args:
        figures: The figures, one per page; a list or tuple with at least one item. An axes
            stands for its figure.

    Returns:
        The PDF file contents.

    Raises:
        TypeError: If ``figures`` is a single ``Figure`` or not a list or tuple, or an item is
            not a figure or the axes of one.
        DataError: If ``figures`` is empty.
        DataError, StyleError: If a figure cannot be rendered (see ``Figure.to_pdf()``). The
            error keeps the class the figure's own render raises, and its message begins with
            ``figures[i]: ``, where ``i`` counts from 0.

    Example:
        >>> import umber as ub
        >>> first = ub.line([0, 1, 2], [1, 3, 2]).figure
        >>> second = ub.bar(["a", "b"], [3, 5]).figure
        >>> pdf = ub.to_pdf([first, second])
        >>> pdf.count(b"/MediaBox")
        2
    """
    with api_call():
        return _umber.pdf_pages(_native("to_pdf()", figures), False)


def save_pdf(
    path: str | os.PathLike[str], figures: Sequence[Figure | Axes[Any, Any] | PieAxes]
) -> None:
    """Render several figures as one PDF document and write it to ``path``.

    See ``to_pdf()``. Every figure is rendered before the file is touched, so when a figure
    cannot be rendered no file is written and an existing file at ``path`` is left as it was.

    Args:
        path: The file to write; an existing file is replaced.
        figures: The figures, one per page; a list or tuple with at least one item.

    Raises:
        TypeError: If ``path`` is not a ``str`` or a path-like object returning a string,
            ``figures`` is a single ``Figure`` or not a list or tuple, or an item is not a
            figure or the axes of one.
        DataError: If ``figures`` is empty.
        DataError, StyleError: If a figure cannot be rendered. The error keeps the class the
            figure's own render raises, and its message begins with ``figures[i]: ``, where
            ``i`` counts from 0.
        OSError: If the file cannot be written.

    Example:
        >>> import umber as ub
        >>> first = ub.line([0, 1, 2], [1, 3, 2]).figure
        >>> second = ub.bar(["a", "b"], [3, 5]).figure
        >>> ub.save_pdf("results.pdf", [first, second])
    """
    with api_call():
        _args.require_path("save_pdf()", "path", path)
        target = _args.path("save_pdf()", "path", path)
        pdf = _umber.pdf_pages(_native("save_pdf()", figures), True)
        Path(target).write_bytes(pdf)
