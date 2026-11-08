"""Triangle meshes: ``delaunay()`` and the ``triangles`` argument of the triangle marks."""

from collections.abc import Sequence
from typing import Any, Final, Never, cast

import numpy as np
import numpy.typing as npt

from umber import _umber
from umber._args import is_text_or_buffer, type_name
from umber._enums import AUTO
from umber._errors import DataError, new_error
from umber._report import api_call
from umber._types import Values
from umber._values import check_floats_type, floats

type IndexArray = npt.NDArray[np.int64]
"""Triangles as an ``int64`` array shaped ``(n, 3)``: three point indices per row."""

type TriangleIndices = npt.NDArray[np.integer[Any]] | Sequence[Sequence[int]]
"""Triangles as given: an integer array shaped ``(n, 3)``, or a list of rows of three ints."""

_SHAPE_HINT: Final = "pass an integer array shaped (n, 3), one row of three point indices each."
_TYPE_HINT: Final = (
    "pass an integer array shaped (n, 3), such as delaunay(x, y) or a part of it, or AUTO."
)
_FLOAT_HINT: Final = "convert with .astype(np.int64) if the values are whole numbers."
_INT64_MAX: Final = np.iinfo(np.int64).max


def delaunay(x: Values[float], y: Values[float]) -> IndexArray:
    """Return the Delaunay triangulation of the points ``(x[i], y[i])``.

    Each row of the result is one triangle: three indices into the points, counter-clockwise
    in data coordinates with y up, starting at the smallest; the rows are sorted. Pass the
    result, or a part of it chosen with NumPy, as ``triangles=`` to ``triplot()`` and
    ``tripcolor()``; ``triangles=AUTO`` is this triangulation.

    It works on the raw coordinates, as ``scipy.spatial.Delaunay`` does, and a log axis changes
    nothing here. For x and y in very different units, or for points shown on a log axis,
    triangulate coordinates scaled to look even, such as ``delaunay(np.log10(x), y)``, and pass
    the result as ``triangles=``. Which diagonal splits four points on one circle is fixed, the
    same on every platform.

    Args:
        x: The x coordinate of each point: finite.
        y: The y coordinate of each point, one per x coordinate: finite.

    Returns:
        An ``int64`` array shaped ``(n, 3)``.

    Raises:
        TypeError: If ``x`` or ``y`` is not a sequence or an array of numbers.
        DataError: If ``x`` and ``y`` differ in length, there are fewer than three points, a
            coordinate is NaN or infinite (there are no missing points: an index counts every
            point), a point repeats an earlier one (the message names both), all points lie on
            one line (decided exactly), or two points lie too close to tell apart, within about
            1e-15 of the largest coordinate along both axes (the message names both). It is
            also raised, with a different message, for points that are not on one line but
            whose coordinates spread over so wide a range of scales, such as x up to 2 and y up
            to 1e-200, that they cannot be triangulated in double precision: the message names
            both spans and the axis of the smaller one, and the hint suggests rescaling, such
            as ``delaunay(x / np.ptp(x), y / np.ptp(y))``, and passing the result as
            ``triangles=``.

    Example:
        >>> import numpy as np
        >>> import umber as ub
        >>> ub.delaunay([0, 1, 1, 0], [0, 0, 1, 1])
        array([[0, 1, 2],
               [0, 2, 3]])
        >>> angle = np.linspace(0, 2 * np.pi, 24, endpoint=False)
        >>> x = np.concatenate([np.cos(angle), 2 * np.cos(angle)])
        >>> y = np.concatenate([np.sin(angle), 2 * np.sin(angle)])
        >>> tri = ub.delaunay(x, y)
        >>> ring = tri[np.hypot(x[tri].mean(axis=1), y[tri].mean(axis=1)) > 1]
        >>> ax = ub.Figure().add_axes().triplot(x, y, triangles=ring)
    """
    with api_call():
        context = "delaunay()"
        check_floats_type(context, "x", x)
        check_floats_type(context, "y", y)
        data = _umber.delaunay(floats(context, "x", x), floats(context, "y", y))
        return np.frombuffer(data, dtype=np.int64).reshape(-1, 3)


def native_triangles(context: str, triangles: object) -> IndexArray | None:
    """Check a ``triangles`` argument and return its native form: ``None`` for ``AUTO``.

    The indices themselves (range, repeated corners, area, edges) are checked by the core.

    Raises:
        TypeError: The argument is not ``AUTO`` and not an integer array or list of rows, such
            as a float array.
        DataError: The argument is not shaped ``(n, 3)``, is a masked array, or holds an index
            beyond the int64 range.
    """
    if triangles is AUTO:
        return None
    if isinstance(triangles, np.ma.MaskedArray):
        message = "triangles is a masked array; choose the rows to keep instead of masking them."
        raise new_error(
            DataError, context, message, parameter="triangles", hint="use triangles[keep]."
        )
    if hasattr(triangles, "__array__"):
        array = _integers(context, np.asarray(cast(npt.ArrayLike, triangles)))
    elif _is_row(triangles):
        array = _rows(context, cast(Sequence[object], triangles))
    else:
        message = (
            f"triangles is {type_name(triangles)}; triangles must be an integer array shaped "
            "(n, 3), or AUTO."
        )
        raise new_error(TypeError, context, message, parameter="triangles", hint=_TYPE_HINT)
    if array.ndim != 2 or array.shape[1] != 3:
        shape = ", ".join(str(n) for n in array.shape) + ("," if array.ndim == 1 else "")
        message = f"triangles has shape ({shape}); it must be shaped (n, 3)."
        raise new_error(DataError, context, message, parameter="triangles", hint=_SHAPE_HINT)
    if array.dtype.kind == "u" and array.size and int(array.max()) > _INT64_MAX:
        _too_large(context, int(array.max()))
    return np.ascontiguousarray(array, dtype=np.int64)


def _integers(context: str, array: npt.NDArray[Any]) -> npt.NDArray[Any]:
    """The array itself when its dtype is an integer dtype; otherwise raise ``TypeError``.

    An ``object`` array is refused too, whatever it holds.
    """
    if array.dtype.kind in "iu":
        return array
    hint = _FLOAT_HINT if array.dtype.kind == "f" else _TYPE_HINT
    message = f"triangles is an array of {array.dtype}; triangle indices must be integers."
    raise new_error(TypeError, context, message, parameter="triangles", hint=hint)


def _rows(context: str, rows: Sequence[object]) -> npt.NDArray[np.int64]:
    """Rows of Python or NumPy ints as an ``int64`` array; other elements raise ``TypeError``.

    Rows of different lengths, or rows mixed with bare indices, raise ``DataError``.
    """
    nested = [_is_row(row) for row in rows]
    widths = {
        len(cast(Sequence[object], row)) for row, inner in zip(rows, nested, strict=True) if inner
    }
    if (any(nested) and not all(nested)) or len(widths) > 1:
        message = "triangles has rows of different lengths; it must be shaped (n, 3)."
        raise new_error(DataError, context, message, parameter="triangles", hint=_SHAPE_HINT)
    if widths:
        items = [item for row in rows for item in cast(Sequence[object], row)]
        shape: tuple[int, ...] = (len(rows), widths.pop())
    else:
        items, shape = list(rows), (len(rows),)
    indices = [_index(context, item) for item in items]
    return np.array(indices, dtype=np.int64).reshape(shape)


def _is_row(value: object) -> bool:
    """Whether ``value`` is a row of indices: a sequence or an array, but not text."""
    return not is_text_or_buffer(value) and isinstance(value, Sequence | np.ndarray)


def _index(context: str, item: object) -> int:
    """The index ``item`` is: a Python or NumPy int within the int64 range, not a ``bool``.

    Raises:
        TypeError: The item is not an int.
        DataError: The item is beyond the int64 range.
    """
    if not isinstance(item, bool) and isinstance(item, int | np.integer):
        index = int(cast(int, item))
        if not -_INT64_MAX - 1 <= index <= _INT64_MAX:
            _too_large(context, index)
        return index
    message = f"triangles holds {type_name(item)}; triangle indices must be integers."
    hint = _FLOAT_HINT if isinstance(item, float | np.floating) else _TYPE_HINT
    raise new_error(TypeError, context, message, parameter="triangles", hint=hint)


def _too_large(context: str, index: int) -> Never:
    """Raise the ``DataError`` for an index beyond the int64 range."""
    message = f"triangles holds {index}; indices must be point indices, from 0."
    hint = "index the points from 0, in the order of x and y."
    raise new_error(DataError, context, message, parameter="triangles", hint=hint)
