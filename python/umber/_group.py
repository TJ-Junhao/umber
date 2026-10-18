"""Split long-form columns into groups before making a single native call."""

from typing import Any, Final, cast

import numpy as np
import numpy.typing as npt

from umber import _color
from umber._args import short_repr
from umber._color import Color
from umber._enums import AUTO, Auto
from umber._errors import DataError, UmberError, UsageError, details, new_error
from umber._time import MicrosArray
from umber._types import Values
from umber._values import (
    FloatArray,
    check_axis_type,
    check_names_type,
    check_one_dimensional,
    names,
)

type Coordinates = FloatArray | MicrosArray | list[str]
type NativeSeries = tuple[str, FloatArray, tuple[int, int, float]]

_STYLE_HINT: Final = "each group takes the next palette color and its name as the label."
_LENGTH_HINT: Final = "pass one group name per data value."


def check_type(context: str, by: Values[str] | Auto) -> None:
    """Check the grouping column's container without converting its values."""
    if by is not AUTO:
        check_names_type(context, "by", by)


def check_style(context: str, label: str, color: object) -> None:
    """Refuse an explicit label or color when groups supply both."""
    if str.__str__(label):
        raise new_error(
            UsageError,
            context,
            "label is non-empty with by; groups supply their own labels.",
            parameter="label",
            hint=_STYLE_HINT,
        )
    if color != Color.AUTO:
        raise new_error(
            UsageError,
            context,
            "color is explicit with by; groups take colors from the palette.",
            parameter="color",
            hint=_STYLE_HINT,
        )


def split_by(
    context: str, by: Values[str], *columns: Values[Any]
) -> list[tuple[str, list[npt.NDArray[Any]]]]:
    """Split columns in first-appearance order, keeping the order of rows in each group.

    Args:
        context: The API call used in errors.
        by: One string group name per row. Missing names are refused.
        *columns: One-dimensional columns of the same length as ``by``.

    Returns:
        The group names and their sliced columns. No column metadata is read.

    Raises:
        TypeError: A column has the wrong container type.
        DataError: A column has the wrong shape or length, a group name is invalid, or
            ``by`` is empty.

    Example:
        >>> from umber._group import split_by
        >>> split_by("Axes.line()", ["B", "A", "B"], [1, 2, 3])
        [('B', [array([1, 3])]), ('A', [array([2])])]
    """
    check_names_type(context, "by", by)
    for index, column in enumerate(columns):
        check_axis_type(context, f"columns[{index}]", column)
    # Object dtype preserves missing entries in mixed lists and numeric arrays, so errors
    # name their original row. Masked arrays retain the normal names-conversion error.
    if np.ma.isMaskedArray(by):
        labels = names(context, "by", by)
    elif hasattr(by, "__array__"):
        labels = names(context, "by", np.asarray(by).astype(object, copy=False))
    else:
        labels = names(context, "by", np.asarray(by, dtype=object))
    arrays = [np.asarray(column) for column in columns]
    for index, array in enumerate(arrays):
        check_one_dimensional(context, f"columns[{index}]", array)
        if len(array) != len(labels):
            raise new_error(
                DataError,
                context,
                f"by has {len(labels)} values but column {index} has {len(array)}; "
                "by must have one name per data value.",
                parameter="by",
                hint=_LENGTH_HINT,
            )
    if not labels:
        raise new_error(
            DataError,
            context,
            "by is empty; pass at least one group.",
            parameter="by",
            hint=_LENGTH_HINT,
        )
    indices: dict[str, list[int]] = {}
    for index, label in enumerate(labels):
        indices.setdefault(label, []).append(index)
    return [(label, [array[rows] for array in arrays]) for label, rows in indices.items()]


def _coordinates(values: npt.NDArray[Any]) -> Coordinates:
    """Keep native numeric/time buffers; names reach the binding as a list."""
    if values.dtype.kind in "UOT":
        return cast(list[str], values.tolist())
    return cast(FloatArray | MicrosArray, values)


def xy_arguments(
    context: str, by: Values[str], x: Coordinates, y: Coordinates
) -> list[tuple[str, Coordinates, Coordinates]]:
    """Return labelled pairs of already-converted coordinate columns for the binding."""
    return [
        (label, _coordinates(columns[0]), _coordinates(columns[1]))
        for label, columns in split_by(context, by, x, y)
    ]


def ecdf_arguments(
    context: str, by: Values[str], values: FloatArray | MicrosArray
) -> list[tuple[str, Coordinates]]:
    """Return labelled value columns, preserving numeric and time dtypes."""
    return [(label, _coordinates(columns[0])) for label, columns in split_by(context, by, values)]


def series_arguments(context: str, by: Values[str], values: FloatArray) -> list[NativeSeries]:
    """Return the native series for grouped histograms and densities."""
    return [
        (label, cast(FloatArray, columns[0]), _color.native(Color.AUTO))
        for label, columns in split_by(context, by, values)
    ]


def bar_error(context: str, error: UmberError) -> UmberError:
    """Keep native validation details while pointing at the public long-form arguments."""
    info = details(error)
    if info is None:
        return error
    parameter, hint = info
    return new_error(
        type(error),
        context,
        str(error).removeprefix("Axes.grouped_bar(): "),
        parameter="heights" if parameter == "series" else parameter,
        hint=hint,
    )


def bar_arguments(
    context: str, by: Values[str], categories: list[str], heights: FloatArray
) -> tuple[list[str], list[NativeSeries]]:
    """Align long-form bars to first-seen categories, leaving missing pairs as NaN."""
    groups = split_by(context, by, categories, heights)
    positions = {name: index for index, name in enumerate(dict.fromkeys(categories))}
    series: list[NativeSeries] = []
    for label, columns in groups:
        values = np.full(len(positions), np.nan)
        seen: set[str] = set()
        for category, height in zip(columns[0], columns[1], strict=True):
            name = str(category)
            if name in seen:
                raise new_error(
                    DataError,
                    context,
                    f"the pair ({short_repr(name)}, {short_repr(label)}) repeats; "
                    "each category and group pair must occur at most once.",
                    parameter="categories",
                    hint="aggregate first, for example with groupby().sum().",
                )
            seen.add(name)
            values[positions[name]] = height
        series.append((label, values, _color.native(Color.AUTO)))
    return list(positions), series
