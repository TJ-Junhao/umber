"""Quick functions: a new figure with one axes and one mark, in a single call.

Each quick function is ``Figure(theme=theme)``, then ``add_axes()``, then the mark method of
the same name, and returns the axes. It registers nothing: every call makes a new figure,
reached through the returned axes' ``figure``. Use ``axes_at()`` to reach its panel again,
passing ``x=Scale.category()`` for a bar chart. Errors found by the mark method keep its
context, such as ``Axes.line(): …``, and the highlight points at the quick function's call.
"""

from typing import Final

from umber._args import instance
from umber._axes import Axes
from umber._color import Color
from umber._color_by import ColorBy
from umber._enums import Dash, Marker
from umber._figure import Figure
from umber._report import api_call
from umber._scale import Scale
from umber._size import Size
from umber._theme import Theme
from umber._types import Values

_THEME_HINT: Final = "use Theme.PIGMENT, Theme.GRAPHITE or Theme.MIDNIGHT."


def line(
    x: Values[float],
    y: Values[float],
    *,
    label: str = "",
    color: Color = Color.AUTO,
    dash: Dash = Dash.SOLID,
    width: Size = Size.AUTO,
    theme: Theme = Theme.PIGMENT,
) -> Axes[float, float]:
    """Plot a line on a new figure and return its axes.

    The same as ``Figure(theme=theme).add_axes().line(x, y, ...)``. A NaN in ``x`` or ``y``
    leaves a gap in the line.

    Args:
        x: The x values.
        y: The y values, one per x value.
        label: The name shown in the legend; an empty string leaves the line out of it.
        color: The line color. ``Color.AUTO`` takes the first color of the theme palette.
        dash: How the line is stroked.
        width: The line width. ``Size.AUTO`` is the line width of the theme.
        theme: The theme of the new figure.

    Returns:
        The axes of the new figure.

    Raises:
        TypeError: If an argument has the wrong type.
        DataError: If the data cannot be plotted: lengths differ, a value is infinite, or
            ``x`` holds names.
        StyleError: If ``label`` is too long.

    Example:
        >>> import umber as ub
        >>> ub.line([0, 1, 2], [1.0, 3.0, 2.0]).title("Signal").figure.save_svg("signal.svg")
    """
    with api_call():
        instance("line()", "theme", theme, Theme, _THEME_HINT)
        axes = Figure(theme=theme).add_axes()
        return axes.line(x, y, label=label, color=color, dash=dash, width=width)


def scatter(
    x: Values[float],
    y: Values[float],
    *,
    label: str = "",
    color: Color | ColorBy = Color.AUTO,
    marker: Marker = Marker.CIRCLE,
    size: Size = Size.AUTO,
    theme: Theme = Theme.PIGMENT,
) -> Axes[float, float]:
    """Plot a marker at each point on a new figure and return its axes.

    The same as ``Figure(theme=theme).add_axes().scatter(x, y, ...)``. A point with a NaN
    coordinate is left out.

    Args:
        x: The x values.
        y: The y values, one per x value.
        label: The name shown in the legend; an empty string leaves the series out of it.
            A scatter colored by ``ColorBy`` has no legend entry and takes no label.
        color: The marker color. ``Color.AUTO`` takes the first color of the theme palette;
            ``ColorBy(values)`` colors each point by its value instead.
        marker: The marker shape.
        size: The marker size. ``Size.AUTO`` is the marker size of the theme.
        theme: The theme of the new figure.

    Returns:
        The axes of the new figure.

    Raises:
        TypeError: If an argument has the wrong type.
        DataError: If the data cannot be plotted: lengths differ, a value is infinite,
            ``x`` holds names, or a ``ColorBy`` does not have one value per point.
        StyleError: If ``label`` is too long.
        UsageError: If a ``ColorBy`` comes with a label.

    Example:
        >>> import umber as ub
        >>> ax = ub.scatter([1, 2, 3], [2.0, 1.0, 3.0], marker=ub.Marker.SQUARE)
        >>> ax.figure.save_png("points.png", dpi=100)
        >>> ax = ub.scatter([1, 2, 3], [2.0, 1.0, 3.0], color=ub.ColorBy([5.0, 1.0, 3.0]))
    """
    with api_call():
        instance("scatter()", "theme", theme, Theme, _THEME_HINT)
        axes = Figure(theme=theme).add_axes()
        return axes.scatter(x, y, label=label, color=color, marker=marker, size=size)


def bar(
    categories: Values[str],
    heights: Values[float],
    *,
    label: str = "",
    color: Color = Color.AUTO,
    theme: Theme = Theme.PIGMENT,
) -> Axes[str, float]:
    """Plot one bar per category on a new figure and return its axes.

    The same as ``Figure(theme=theme).add_axes(x=Scale.category()).bar(categories, heights,
    ...)``. A negative height draws below the baseline; a NaN height leaves its category
    without a bar.

    Args:
        categories: The category names, each at most once.
        heights: The bar heights, one per category.
        label: The name shown in the legend; an empty string leaves the bars out of it.
        color: The bar color. ``Color.AUTO`` takes the first color of the theme palette.
        theme: The theme of the new figure.

    Returns:
        The axes of the new figure, with a category x axis.

    Raises:
        TypeError: If an argument has the wrong type.
        DataError: If the data cannot be plotted: lengths differ, a category repeats, or a
            height is infinite.
        StyleError: If ``label`` or a category name is too long.

    Example:
        >>> import umber as ub
        >>> ax = ub.bar(["North", "South", "East"], [3, 5, 2], theme=ub.Theme.GRAPHITE)
        >>> ax.figure.save_svg("regions.svg")
    """
    with api_call():
        instance("bar()", "theme", theme, Theme, _THEME_HINT)
        axes = Figure(theme=theme).add_axes(x=Scale.category())
        return axes.bar(categories, heights, label=label, color=color)


def hist(
    values: Values[float],
    *,
    bins: int = 30,
    label: str = "",
    color: Color = Color.AUTO,
    theme: Theme = Theme.PIGMENT,
) -> Axes[float, float]:
    """Plot a histogram of ``values`` on a new figure and return its axes.

    The same as ``Figure(theme=theme).add_axes().hist(values, ...)``. NaN values are left
    out of the counts.

    Args:
        values: The values to count.
        bins: The number of equal-width bins, from 1 to ``MAX_BINS``.
        label: The name shown in the legend; an empty string leaves the histogram out of it.
        color: The bar color. ``Color.AUTO`` takes the first color of the theme palette.
        theme: The theme of the new figure.

    Returns:
        The axes of the new figure.

    Raises:
        TypeError: If an argument has the wrong type.
        DataError: If a value is infinite.
        StyleError: If ``bins`` is out of range, or ``label`` is too long.

    Example:
        >>> import umber as ub
        >>> ax = ub.hist([1.2, 2.5, 2.7, 3.1, 4.8, 5.0], bins=4)
        >>> ax.figure.save_png("sizes.png", dpi=100)
    """
    with api_call():
        instance("hist()", "theme", theme, Theme, _THEME_HINT)
        axes = Figure(theme=theme).add_axes()
        return axes.hist(values, bins=bins, label=label, color=color)
