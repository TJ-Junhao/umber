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
class Font(Enum):
    """A font family bundled with umber.

    Example:
        >>> import umber as ub
        >>> ub.Font.INTER.name
        'INTER'
    """

    SOURCE_SERIF_4 = 0
    """Source Serif 4, a serif face."""

    IBM_PLEX_SANS = 1
    """IBM Plex Sans, a sans-serif face."""

    GEIST = 2
    """Geist, a sans-serif face."""

    INTER = 3
    """Inter, a sans-serif face."""

    JETBRAINS_MONO = 4
    """JetBrains Mono, a monospaced face."""


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
