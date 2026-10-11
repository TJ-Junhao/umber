"""umber: a plotting library for Python, written in Rust.

Example:
    >>> import umber as ub
    >>> isinstance(ub.__version__, str)
    True
"""

from collections.abc import Callable
from types import FunctionType
from typing import Final

from umber._axes import Axes
from umber._color import Color
from umber._enums import Dash, FillStyle, Font, GridLines, LegendStyle, Marker, SvgText
from umber._errors import DataError, StyleError, UmberError, UsageError
from umber._figure import Figure
from umber._quick import bar, hist, line, scatter
from umber._scale import Scale
from umber._series import BarSeries
from umber._size import Size
from umber._theme import Theme
from umber._types import SupportsArray, Values
from umber._umber import MAX_BINS as _MAX_BINS
from umber._umber import __version__

MAX_BINS: Final[int] = _MAX_BINS
"""The largest bin count a histogram accepts.

Example:
    >>> import umber as ub
    >>> ub.MAX_BINS
    10000
"""

__all__ = [
    "MAX_BINS",
    "Axes",
    "BarSeries",
    "Color",
    "DataError",
    "Dash",
    "FillStyle",
    "Figure",
    "Font",
    "GridLines",
    "LegendStyle",
    "Marker",
    "Scale",
    "Size",
    "StyleError",
    "SupportsArray",
    "SvgText",
    "Theme",
    "UmberError",
    "UsageError",
    "Values",
    "__version__",
    "bar",
    "hist",
    "line",
    "scatter",
]


def _report_package_as_module(*exported: type | Callable[..., object]) -> None:
    """Make each function, class, class method and property report ``umber`` as its module."""
    for obj in exported:
        obj.__module__ = "umber"
        if not isinstance(obj, type):
            continue
        for member in vars(obj).values():
            function = getattr(member, "__func__", member)
            if isinstance(member, property):
                function = member.fget
            if isinstance(function, FunctionType) and not function.__name__.startswith("_"):
                function.__module__ = "umber"


_report_package_as_module(
    Axes,
    BarSeries,
    Color,
    Dash,
    FillStyle,
    Figure,
    Font,
    GridLines,
    LegendStyle,
    Marker,
    Scale,
    Size,
    SupportsArray,
    SvgText,
    Theme,
    DataError,
    StyleError,
    UmberError,
    UsageError,
    bar,
    hist,
    line,
    scatter,
)
