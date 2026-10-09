"""umber: a plotting library for Python, written in Rust.

Example:
    >>> import umber as ub
    >>> isinstance(ub.__version__, str)
    True
"""

from umber._errors import DataError, StyleError, UmberError, UsageError
from umber._umber import __version__

__all__ = ["DataError", "StyleError", "UmberError", "UsageError", "__version__"]
