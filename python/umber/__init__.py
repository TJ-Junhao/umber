"""umber: a plotting library for Python, written in Rust.

Example:
    >>> import umber as ub
    >>> isinstance(ub.__version__, str)
    True
"""

from types import FunctionType
from typing import Final

from umber._axes import Axes
from umber._color import Color
from umber._enums import Dash, Font, GridLines, LegendStyle, Marker, SvgText
from umber._errors import DataError, StyleError, UmberError, UsageError
from umber._figure import Figure
from umber._scale import Scale
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
    "Color",
    "DataError",
    "Dash",
    "Figure",
    "Font",
    "GridLines",
    "LegendStyle",
    "Marker",
    "Scale",
    "StyleError",
    "SupportsArray",
    "SvgText",
    "Theme",
    "UmberError",
    "UsageError",
    "Values",
    "__version__",
]


def _report_package_as_module(*exported: type) -> None:
    """Make each class, its public methods and its properties report ``umber`` as their module."""
    for cls in exported:
        cls.__module__ = "umber"
        for member in vars(cls).values():
            function = getattr(member, "__func__", member)
            if isinstance(member, property):
                function = member.fget
            if isinstance(function, FunctionType) and not function.__name__.startswith("_"):
                function.__module__ = "umber"


_report_package_as_module(
    Axes,
    Color,
    Dash,
    Figure,
    Font,
    GridLines,
    LegendStyle,
    Marker,
    Scale,
    SupportsArray,
    SvgText,
    Theme,
    DataError,
    StyleError,
    UmberError,
    UsageError,
)
