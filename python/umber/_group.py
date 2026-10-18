"""Split long-form columns into groups before making a single native call."""

from collections.abc import Callable
from typing import Any, Final, NamedTuple, cast

import numpy as np
import numpy.typing as npt

from umber import _args, _color
from umber._args import checked_text, short_repr
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
    is_missing,
    names,
)

type Coordinates = FloatArray | MicrosArray | list[str]
type Rows = npt.NDArray[np.intp]
type NativeSeries = tuple[str, FloatArray, tuple[int, int, float]]

_STYLE_HINT: Final = "each group takes the next palette color and its name as the label."
_LENGTH_HINT: Final = "pass one group name per data value."


class _Group(NamedTuple):
    label: str
    rows: Rows
    columns: list[npt.NDArray[Any]]


def _check_style(context: str, label: str, color: object) -> None:
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


def _name(context: str, index: int, value: object) -> str:
    """Validate a name without coercing missing or non-string values."""
    if not isinstance(value, str):
        message = (
            f"by[{index}] is missing; every row needs a group name."
            if is_missing(value)
            else f"by[{index}] is {short_repr(value)}, not a string; group names must be strings."
        )
        raise new_error(
            DataError,
            context,
            message,
            parameter="by",
            hint="give every group a non-empty name."
            if is_missing(value)
            else "convert each name with str() first.",
        )
    if not str.__str__(value):
        raise new_error(
            DataError,
            context,
            f"by[{index}] is empty; group names must be non-empty.",
            parameter="by",
            hint="give every group a non-empty name.",
        )
    return checked_text(context, "by", f"by[{index}]", value, DataError)


def _labels(context: str, by: Values[str]) -> npt.NDArray[Any]:
    """Convert a type-checked column, preserving invalid mixed-list elements."""
    if np.ma.isMaskedArray(by):
        names(context, "by", by)
    array = np.asarray(by) if hasattr(by, "__array__") else np.asarray(by, dtype=object)
    check_one_dimensional(context, "by", array)
    if not len(array):
        raise new_error(
            DataError,
            context,
            "by is empty; pass at least one group.",
            parameter="by",
            hint=_LENGTH_HINT,
        )
    if array.dtype.kind not in "UT":
        # Object columns need element type checks before NumPy can compare their names.
        array = np.asarray([_name(context, i, value) for i, value in enumerate(array.tolist())])
    return array


def _partition(labels: npt.NDArray[Any]) -> tuple[npt.NDArray[Any], Rows, Rows]:
    """Factor names into first-seen order and return each row's category code."""
    unique, first, inverse = np.unique(labels, return_index=True, return_inverse=True)
    order = np.argsort(first)
    codes = np.empty(len(order), dtype=np.intp)
    codes[order] = np.arange(len(order))
    return unique[order], first[order], codes[inverse]


def _split(
    context: str,
    by: Values[str],
    columns: dict[str, Coordinates],
) -> list[_Group]:
    labels = _labels(context, by)
    arrays = [np.asarray(column) for column in columns.values()]
    for name, array in zip(columns, arrays, strict=True):
        check_one_dimensional(context, name, array)
        if len(array) != len(labels):
            raise new_error(
                DataError,
                context,
                f"by has {len(labels)} values but {name} has {len(array)}; "
                "by must have one name per data value.",
                parameter="by",
                hint=_LENGTH_HINT,
            )
    unique, first = np.unique(labels, return_index=True)
    appearance = np.argsort(first)
    checked = {int(i): _name(context, int(first[i]), str(unique[i])) for i in appearance}
    if len(unique) <= 8:
        # A few vectorized comparisons cost less than sorting another full row index.
        rows_by_group = [np.flatnonzero(labels == name) for name in unique]
    else:
        inverse = np.searchsorted(unique, labels)
        order = np.argsort(inverse, kind="stable")
        rows_by_group = np.split(order, np.cumsum(np.bincount(inverse))[:-1])
    return [
        _Group(checked[int(i)], rows_by_group[i], [array[rows_by_group[i]] for array in arrays])
        for i in appearance
    ]


def split_by(
    context: str, by: Values[str], *columns: Values[Any]
) -> list[tuple[str, list[npt.NDArray[Any]]]]:
    """Split columns in first-appearance order, keeping the order of rows in each group.

    Args:
        context: The API call used in errors.
        by: One non-empty string group name per row. Missing names are refused.
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
    data = {f"columns[{i}]": cast(Coordinates, column) for i, column in enumerate(columns)}
    return [(group.label, group.columns) for group in _split(context, by, data)]


def _coordinates(values: npt.NDArray[Any]) -> Coordinates:
    """Keep native numeric/time buffers; names reach the binding as a list."""
    if values.dtype.kind in "UOT":
        return cast(list[str], values.tolist())
    return cast(FloatArray | MicrosArray, values)


def _bar_arguments(
    context: str,
    groups: list[_Group],
    categories: Coordinates,
) -> tuple[list[str], list[NativeSeries], list[_Group]]:
    """Align bars and their input rows with vectorized category codes."""
    unique, _, _ = _partition(np.asarray(categories))
    sorted_order = np.argsort(unique)
    sorted_names = unique[sorted_order]
    series: list[NativeSeries] = []
    aligned: list[_Group] = []
    for group in groups:
        positions = sorted_order[np.searchsorted(sorted_names, group.columns[0])]
        order = np.argsort(positions, kind="stable")
        repeated = positions[order[1:]] == positions[order[:-1]]
        if np.any(repeated):
            row = int(np.min(order[1:][repeated]))
            name = str(group.columns[0][row])
            raise new_error(
                DataError,
                context,
                f"the pair ({short_repr(name)}, {short_repr(group.label)}) repeats at "
                f"categories[{group.rows[row]}]; "
                "each category and group pair must occur at most once.",
                parameter="categories",
                hint="aggregate first, for example with groupby().sum().",
            )
        values = np.full(len(unique), np.nan)
        values[positions] = group.columns[1]
        rows = np.full(len(unique), -1, dtype=np.intp)
        rows[positions] = group.rows
        series.append((group.label, values, _color.native(Color.AUTO)))
        aligned.append(_Group(group.label, rows, []))
    return cast(list[str], unique.tolist()), series, aligned


def _mapped_error(context: str, error: UmberError, groups: list[_Group]) -> UmberError:
    """Render native input references once, using the original row map."""
    info = details(error)
    group_index = cast(int | None, getattr(error, "_umber_group", None))
    if info is None or group_index is None:
        return error
    group = groups[group_index]
    parameter, hint = info
    message = cast(str, getattr(error, "_umber_message", ""))
    name, indices, fragments = cast(
        tuple[str, list[int | None], list[str]], getattr(error, "_umber_input", None)
    )
    if indices:
        name = parameter or name
        parts = [fragments[0]]
        for index, fragment in zip(indices, fragments[1:], strict=True):
            parts.extend((name if index is None else f"{name}[{group.rows[index]}]", fragment))
        message = "".join(parts)
    return new_error(
        type(error),
        context,
        f"in group {short_repr(group.label)}: {message}",
        parameter=parameter,
        hint=hint,
    )


def call(
    context: str,
    by: Values[str] | Auto,
    label: str,
    color: object,
    convert: Callable[[], dict[str, Coordinates]],
    plain: Callable[[tuple[Any, ...], str], None],
    grouped: Callable[[tuple[Any, ...]], None],
) -> None:
    """Check grouping, convert once, and dispatch either ordinary or grouped data."""
    if by is not AUTO:
        check_names_type(context, "by", by)
        _check_style(context, label, color)
    try:
        columns = convert()
    except UmberError as error:
        row = cast(int | None, getattr(error, "_umber_row", None))
        info = details(error)
        if by is AUTO or row is None or info is None:
            raise
        labels = _labels(context, by)
        if row >= len(labels):
            raise
        group = _name(context, row, labels[row])
        message = cast(str, getattr(error, "_umber_message", ""))
        raise new_error(
            type(error),
            context,
            f"in group {short_repr(group)}: {message}",
            parameter=info[0],
            hint=info[1],
        ) from None
    name = _args.text(context, "label", label)
    if by is AUTO:
        plain(tuple(columns.values()), name)
        return
    if len(columns) == 2:
        (left, xs), (right, ys) = columns.items()
        if len(xs) != len(ys):
            raise new_error(
                DataError,
                context,
                f"{left} has {len(xs)} values but {right} has {len(ys)}; "
                f"{left} and {right} must have the same length.",
                parameter=None,
                hint=None,
            )
    groups = _split(context, by, columns)
    if context == "Axes.bar()":
        categories, series, groups = _bar_arguments(context, groups, columns["categories"])
        arguments: tuple[Any, ...] = (categories, series)
    elif len(columns) == 2:
        arguments = ([(g.label, *(_coordinates(c) for c in g.columns)) for g in groups],)
    else:
        series_values = [(g.label, _coordinates(g.columns[0])) for g in groups]
        arguments = (
            series_values
            if context == "Axes.ecdf()"
            else [(*values, _color.native(Color.AUTO)) for values in series_values],
        )
    try:
        grouped(arguments)
    except UmberError as error:
        raise _mapped_error(context, error, groups) from None
