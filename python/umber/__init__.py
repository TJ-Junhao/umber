"""umber: a plotting library for Python, written in Rust.

Example:
    >>> import umber as ub
    >>> isinstance(ub.__version__, str)
    True
"""

from umber._color import Color
from umber._enums import Dash, Font, GridLines, LegendStyle, Marker, SvgText
from umber._errors import DataError, StyleError, UmberError, UsageError
from umber._scale import Scale
from umber._umber import MAX_BINS, __version__

__all__ = [
    "MAX_BINS",
    "Color",
    "DataError",
    "Dash",
    "Font",
    "GridLines",
    "LegendStyle",
    "Marker",
    "Scale",
    "StyleError",
    "SvgText",
    "UmberError",
    "UsageError",
    "__version__",
]

for _exported in (
    Color,
    Dash,
    Font,
    GridLines,
    LegendStyle,
    Marker,
    Scale,
    SvgText,
    DataError,
    StyleError,
    UmberError,
    UsageError,
):
    _exported.__module__ = "umber"
del _exported
