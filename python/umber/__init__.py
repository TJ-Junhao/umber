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
from umber._cell import Cell
from umber._cmap import Cmap
from umber._color import Color
from umber._color_by import ColorBy
from umber._enums import (
    AUTO,
    Align,
    Auto,
    ColorbarPosition,
    Dash,
    FillStyle,
    Font,
    GridLines,
    Legend,
    LegendPosition,
    LegendStyle,
    Marker,
    Render,
    Share,
    Step,
    SvgText,
)
from umber._errors import DataError, StyleError, UmberError, UsageError
from umber._figure import Figure
from umber._quick import bar, hist, line, scatter
from umber._scale import Scale
from umber._series import Distribution, Series
from umber._size import Size
from umber._theme import Theme
from umber._ticks import TickFormat
from umber._types import SupportsArray, Time, Values
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
    "AUTO",
    "MAX_BINS",
    "Align",
    "Auto",
    "Axes",
    "Cell",
    "Series",
    "Cmap",
    "Color",
    "ColorBy",
    "ColorbarPosition",
    "DataError",
    "Dash",
    "Distribution",
    "FillStyle",
    "Figure",
    "Font",
    "GridLines",
    "Legend",
    "LegendPosition",
    "LegendStyle",
    "Marker",
    "Render",
    "Scale",
    "Share",
    "Size",
    "Step",
    "StyleError",
    "SupportsArray",
    "SvgText",
    "Theme",
    "TickFormat",
    "Time",
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
    Align,
    Auto,
    Axes,
    Cell,
    Series,
    Cmap,
    Color,
    ColorBy,
    ColorbarPosition,
    Dash,
    Distribution,
    FillStyle,
    Figure,
    Font,
    GridLines,
    Legend,
    LegendPosition,
    LegendStyle,
    Marker,
    Render,
    Scale,
    Share,
    Size,
    Step,
    SupportsArray,
    SvgText,
    Theme,
    TickFormat,
    DataError,
    StyleError,
    UmberError,
    UsageError,
    bar,
    hist,
    line,
    scatter,
)
