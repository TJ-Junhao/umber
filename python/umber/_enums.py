"""The public enums.

Except for ``Auto``, each member's value maps to the matching core enum in the extension.
"""

from enum import Enum, auto, unique
from typing import Final


@unique
class Auto(Enum):
    """Use the default size or position appropriate to the figure.

    Example:
        >>> import umber as ub
        >>> ub.AUTO is ub.Auto.AUTO
        True
    """

    __slots__ = ()

    AUTO = auto()
    """Choose automatically where the parameter accepts ``AUTO``."""

    def __repr__(self) -> str:
        return "AUTO"


AUTO: Final = Auto.AUTO
"""The typed sentinel for automatic figure sizes, positions and cell counts.

Example:
    >>> import umber as ub
    >>> repr(ub.AUTO)
    'AUTO'
"""


@unique
class Share(Enum):
    """Which panels share an axis scale, limits, ticks and tick format.

    Example:
        >>> import umber as ub
        >>> fig = ub.Figure(rows=2, share_x=ub.Share.COL)
    """

    __slots__ = ()

    NONE = 0
    """Each panel has its own scale."""

    ROW = 1
    """Panels in the same row share a scale."""

    COL = 2
    """Panels in the same column share a scale."""

    ALL = 3
    """All panels share a scale."""


@unique
class Legend(Enum):
    """Whether the figure combines panel legends or shows each separately.

    Example:
        >>> import umber as ub
        >>> fig = ub.Figure(cols=2, legend=ub.Legend.MERGED)
    """

    __slots__ = ()

    MERGED = 0
    """One legend for the whole figure."""

    PER_PANEL = 1
    """A separate legend for each panel."""


@unique
class LegendPosition(Enum):
    """Where a panel's legend entries sit.

    Applies to single-panel figures and ``Legend.PER_PANEL``. A merged grid legend stays
    under the figure title; direct labels stay at their line ends. ``BEST`` chooses a
    position inside the plot; the sentinel ``AUTO`` instead uses the theme's position.

    Example:
        >>> import umber as ub
        >>> theme = ub.Theme.PIGMENT.with_legend_position(ub.LegendPosition.RIGHT)
    """

    __slots__ = ()

    TOP_LEFT = 0
    """A row under the panel title, aligned to the plot's left edge (the default)."""

    TOP_RIGHT = 1
    """A row under the panel title, aligned to the plot's right edge."""

    RIGHT = 2
    """A column to the right of the plot."""

    BOTTOM = 3
    """A row below the x tick labels and x axis title."""

    BEST = 4
    """A column inside the plot at the position covering the least drawn data."""


@unique
class Align(Enum):
    """Alignment of figure and panel titles, subtitles and the merged grid legend.

    Example:
        >>> import umber as ub
        >>> theme = ub.Theme.PIGMENT.with_title_align(ub.Align.CENTER)
    """

    __slots__ = ()

    LEFT = 0
    """Align to the left edge (the default)."""

    CENTER = 1
    """Align to the centre."""

    RIGHT = 2
    """Align to the right edge."""


@unique
class Dash(Enum):
    """How a line is stroked.

    Example:
        >>> import umber as ub
        >>> ub.Dash.DASHED.name
        'DASHED'
    """

    SOLID = 0
    """A continuous line."""

    DASHED = 1
    """A line of long dashes."""

    DOTTED = 2
    """A line of dots."""


@unique
class Marker(Enum):
    """The shape of a scatter marker.

    Example:
        >>> import umber as ub
        >>> ub.Marker.CIRCLE.name
        'CIRCLE'
    """

    CIRCLE = 0
    """A filled circle."""

    SQUARE = 1
    """A filled square."""

    TRIANGLE = 2
    """A filled triangle pointing up."""

    DIAMOND = 3
    """A filled diamond."""

    CROSS = 4
    """A cross made of two strokes."""


@unique
class LegendStyle(Enum):
    """How a legend is laid out.

    Example:
        >>> import umber as ub
        >>> ub.LegendStyle.CHIPS.name
        'CHIPS'
    """

    HEADER = 0
    """Entries in a row above the plot."""

    DIRECT = 1
    """Labels placed directly at the end of each series."""

    CHIPS = 2
    """Entries as chips."""

    NONE = 3
    """No legend: no entry row and no direct labels."""


@unique
class FillStyle(Enum):
    """How areas and histogram bins are filled.

    Example:
        >>> import umber as ub
        >>> ub.FillStyle.GRADIENT.name
        'GRADIENT'
    """

    SOLID = 0
    """A flat translucent fill."""

    GRADIENT = 1
    """A fill that fades out toward the baseline."""


@unique
class GridLines(Enum):
    """Which grid lines are drawn.

    The choice describes a chart whose values lie along y. A chart whose values lie along x,
    one with a category y axis beside a numeric x axis, mirrors it: there ``HORIZONTAL`` draws
    the lines at the x ticks and ``VERTICAL`` those at the y ticks.

    Example:
        >>> import umber as ub
        >>> ub.GridLines.BOTH.name
        'BOTH'
    """

    HORIZONTAL = 0
    """Lines at y ticks only."""

    VERTICAL = 1
    """Lines at x ticks only."""

    BOTH = 2
    """Lines at both."""

    NONE = 3
    """No gridlines."""


@unique
class SvgText(Enum):
    """How text is written in SVG output.

    Example:
        >>> import umber as ub
        >>> ub.SvgText.EDITABLE.name
        'EDITABLE'
    """

    OUTLINES = 0
    """Text converted to paths, so it looks the same everywhere."""

    EDITABLE = 1
    """Text kept as text elements, so it can be edited in a vector editor."""


@unique
class Step(Enum):
    """Where a step line changes level between two points.

    Example:
        >>> import umber as ub
        >>> ub.Step.MID.name
        'MID'
    """

    POST = 0
    """Hold each value from its x to the next x, then change level there."""

    PRE = 1
    """Change level at the previous x, then hold each value up to its own x."""

    MID = 2
    """Change level halfway between two x values as drawn: at their mean on a linear x axis,
    halfway between the band centres on a category x axis, and at their geometric mean on a log
    x axis (between 1 and 100, at 10)."""


@unique
class ColorbarPosition(Enum):
    """Where a colorbar sits beside its plot.

    Example:
        >>> import umber as ub
        >>> ub.ColorbarPosition.RIGHT.name
        'RIGHT'
    """

    RIGHT = 0
    """Right of the plot, spanning its height."""

    BOTTOM = 1
    """Below the x axis and any bottom legend, spanning the plot width."""

    NONE = 2
    """Hidden, without reserving space."""


@unique
class Render(Enum):
    """How the cells of a heatmap, a 2-D histogram or a mesh are drawn.

    The cells form a grid of solid colors. As vector shapes, each cell is one filled
    rectangle. As an image, the grid is one image with one pixel per cell, stretched over the
    cells with nearest-neighbour scaling: it looks the same, while the file stays small and
    fast to open. In PNG output each pixel takes the average of the cells it covers, weighted
    by area, so cells meet without seams. In SVG output the image is an ``<image>`` holding a
    PNG. NaN values and empty bins are transparent either way. The marks take ``AUTO`` by
    default, which draws an image from 10,000 cells (rows times columns, such as 100 x 100)
    and vector shapes below that. An image is used only where it is exact: on the final plot,
    no edge of the image lies more than 0.001 pt from the cell edge it stands for, and a cell
    cut by explicit limits reaches at most 10,000 pt past them (the image covers whole cells
    and is clipped at the limits). Limits that zoom in far on a mesh can break either bound;
    then ``AUTO`` draws vector shapes and ``RASTER`` makes rendering raise ``StyleError``.

    Example:
        >>> import umber as ub
        >>> ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
        >>> ax = ax.heatmap(["a", "b"], ["r"], [[1, 2]], render=ub.Render.RASTER)
    """

    __slots__ = ()

    VECTOR = 0
    """One filled rectangle per cell."""

    RASTER = 1
    """One image with one pixel per cell. A mesh needs edges evenly spaced on screen, and
    rendering fails when the image would not be exact on the final plot."""


@unique
class Hist(Enum):
    """How a histogram draws its bins.

    Example:
        >>> import umber as ub
        >>> ax = ub.Figure().add_axes().hist([1.2, 2.5, 2.7, 3.1], bins=3, style=ub.Hist.STEP)
    """

    BARS = 0
    """One filled bar per bin, from the baseline to its height."""

    STEP = 1
    """The outline of the bars only, as one stepped line in the theme's line width: up the
    first bin's low edge, along the top of every bin, and down the last bin's high edge. With
    groups, each group's outline is normalized on its own."""

    STACKED = 2
    """The bars of the groups stacked in each bin, normalized together, so the stacks show the
    whole. Without groups it draws the same bars as ``BARS``."""


@unique
class Stat(Enum):
    """What the height of a histogram's bar shows.

    Example:
        >>> import umber as ub
        >>> ax = ub.Figure().add_axes().hist([1.2, 2.5, 2.7, 3.1], stat=ub.Stat.DENSITY)
    """

    COUNT = 0
    """The number of values in the bin."""

    FRACTION = 1
    """The share of the finite values in the bin: the heights sum to 1."""

    DENSITY = 2
    """The share of the finite values in the bin over the bin's width: the bars have an area
    of 1."""


@unique
class Contour(Enum):
    """How a contour is drawn: lines along its levels, filled bands between them, or both.

    Example:
        >>> import umber as ub
        >>> ax = ub.Figure().add_axes()
        >>> z = [[0.0, 1.0, 2.0], [1.0, 2.0, 3.0]]
        >>> ax = ax.contour([0, 1, 2], [0, 1], z, style=ub.Contour.FILLED)
    """

    __slots__ = ()

    LINES = 0
    """A line along each level, colored by its level from the colormap, or all in one color.
    The colorbar is a smooth bar with a tick at each level."""

    FILLED = 1
    """The bands between the levels, each filled with the color of its middle; the regions
    below the first level and above the last take the colors at the ends of the color range.
    The colorbar shows the bands."""

    BOTH = 2
    """The filled bands, then thin lines along the levels in the theme's text color."""
