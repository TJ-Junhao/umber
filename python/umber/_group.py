"""Split long-form columns into groups before making a single native call."""

from collections.abc import Callable, Sized
from dataclasses import dataclass
from functools import partial
from typing import Any, Final, NamedTuple, Protocol, cast

import numpy as np
import numpy.typing as npt

from umber import _args, _color
from umber._args import MAX_TEXT_LENGTH, checked_text, short_repr
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
    columns: dict[str, npt.NDArray[Any]]


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


class _Labels(NamedTuple):
    values: npt.NDArray[Any]
    unique: npt.NDArray[Any]
    first: Rows
    names: list[str]
    checked: bool

    def checked_names(self, context: str) -> list[str]:
        """Validate distinct names in input order, unless already checked."""
        if self.checked:
            return self.names
        checked = self.names.copy()
        for i in np.argsort(self.first):
            index = int(i)
            checked[index] = _name(context, int(self.first[index]), self.names[index])
        return checked

    def at(self, row: int) -> str:
        """Return the name of an original input row."""
        return self.names[cast(int, np.searchsorted(self.unique, self.values[row]))]


def _plain_strings(array: npt.NDArray[Any]) -> bool:
    """Check exact string types in bulk, without calling user-defined methods."""
    types = np.asarray(np.frompyfunc(type, 1, 1)(array), dtype=object)
    return bool(np.all(np.isin(types, np.array([str, np.str_], dtype=object))))


def _column_labels(context: str, by: Values[str]) -> _Labels | None:
    """Use a column's own factorization or small-group comparisons, without importing it."""
    factorize = getattr(by, "factorize", None)
    if callable(factorize):
        categories = getattr(getattr(by, "dtype", None), "categories", None)
        source = np.asarray(by if categories is None else categories)
        if source.ndim != 1 or not _plain_strings(source):
            return None
        try:
            codes, unique = cast(tuple[Rows, Any], factorize(sort=False))
        except TypeError:
            return None
        codes = np.asarray(codes)
        values = np.asarray(unique)
        if codes.ndim != 1 or not len(codes) or np.any(codes < 0):
            return None
        # Unusual string subclasses keep the scalar path's plain-string semantics.
        if not all(type(value) in (str, np.str_) and "\0" not in value for value in values):
            return None
        first = np.full(len(values), len(codes), dtype=np.intp)
        np.minimum.at(first, codes, np.arange(len(codes), dtype=np.intp))
        labels = [_name(context, int(row), value) for row, value in zip(first, values, strict=True)]
        return _Labels(codes, np.arange(len(values)), first, labels, True)

    unique_values = getattr(by, "unique", None)
    if (
        callable(unique_values)
        and isinstance(by, Sized)
        and hasattr(by, "is_null")
        and hasattr(by, "to_numpy")
    ):
        dtype = getattr(by, "dtype", None)
        for check in ("is_object", "is_nested"):
            predicate = getattr(dtype, check, None)
            if callable(predicate) and predicate():
                return None
        # Arrow-backed columns can compare in place; converting every string to NumPy
        # costs more than four comparisons. Larger group counts use the sorted path below.
        unique = cast(Any, unique_values(maintain_order=True))
        if not 0 < len(unique) <= 8:
            return None
        to_list = getattr(unique, "to_list", None)
        if not callable(to_list):
            return None
        values = np.asarray(to_list(), dtype=object)
        if not all(type(value) is str and "\0" not in value for value in values):
            return None
        codes = np.empty(len(by), dtype=np.intp)
        first = np.empty(len(values), dtype=np.intp)
        for i, value in enumerate(values):
            rows = np.flatnonzero(np.asarray(by == value))
            codes[rows] = i
            first[i] = rows[0]
        return _Labels(codes, np.arange(len(values)), first, values.tolist(), False)
    return None


def _labels(context: str, by: Values[str]) -> _Labels:
    """Factor a type-checked column, preserving invalid mixed-list elements."""
    if np.ma.isMaskedArray(by):
        names(context, "by", by)
    native = _column_labels(context, by)
    if native is not None:
        return native
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
    checked = False
    if array.dtype.kind not in "UT":
        original = array
        if _plain_strings(array):
            # Exact strings can be converted in bulk without invoking __str__.
            array = np.asarray(array, dtype=str)
            # Fixed-width Unicode drops trailing NULs. Keep scalar validation for
            # empty or long names so that conversion cannot hide the first error.
            checked = array.dtype.itemsize <= MAX_TEXT_LENGTH * 4 and not np.any(array == "")
        if not checked:
            array = np.asarray(
                [_name(context, i, value) for i, value in enumerate(original.tolist())]
            )
    unique, first = np.unique(array, return_index=True)
    result = _Labels(array, unique, first, unique.tolist(), False)
    if checked:
        result = result._replace(names=result.checked_names(context), checked=True)
    return result


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
        if len(array) != len(labels.values):
            raise new_error(
                DataError,
                context,
                f"by has {len(labels.values)} values but {name} has {len(array)}; "
                "by must have one name per data value.",
                parameter="by",
                hint=_LENGTH_HINT,
            )
    appearance = np.argsort(labels.first)
    checked = labels.checked_names(context)
    if len(labels.unique) <= 8:
        # A few vectorized comparisons cost less than sorting another full row index.
        rows_by_group = [np.flatnonzero(labels.values == name) for name in labels.unique]
    else:
        inverse = np.searchsorted(labels.unique, labels.values)
        order = np.argsort(inverse, kind="stable")
        rows_by_group = np.split(order, np.cumsum(np.bincount(inverse))[:-1])
    return [
        _Group(
            checked[int(i)],
            rows_by_group[i],
            {name: array[rows_by_group[i]] for name, array in zip(columns, arrays, strict=True)},
        )
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
    return [(group.label, list(group.columns.values())) for group in _split(context, by, data)]


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
        positions = sorted_order[np.searchsorted(sorted_names, group.columns["categories"])]
        order = np.argsort(positions, kind="stable")
        repeated = positions[order[1:]] == positions[order[:-1]]
        if np.any(repeated):
            row = int(np.min(order[1:][repeated]))
            name = str(group.columns["categories"][row])
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
        values[positions] = group.columns["heights"]
        rows = np.full(len(unique), -1, dtype=np.intp)
        rows[positions] = group.rows
        series.append((group.label, values, _color.native(Color.AUTO)))
        aligned.append(_Group(group.label, rows, {}))
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
    location = cast(
        tuple[str, list[int | None], list[str]] | None, getattr(error, "_umber_input", None)
    )
    if location is not None:
        name, indices, fragments = location
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


class _XYDraw(Protocol):
    def __call__(self, x: Coordinates, y: Coordinates, *, label: str) -> None: ...


class _BarDraw(Protocol):
    def __call__(self, categories: list[str], heights: FloatArray, *, label: str) -> None: ...


class _ValuesDraw[T](Protocol):
    def __call__(self, values: T, /, *, label: str) -> None: ...


@dataclass(frozen=True, slots=True)
class _GroupedCall:
    invoke: Callable[[], None]
    groups: list[_Group]


@dataclass(frozen=True, slots=True)
class XY:
    """Converters and native calls for a pair of coordinate columns."""

    x: Callable[[], Coordinates]
    y: Callable[[], Coordinates]
    draw: _XYDraw
    draw_groups: Callable[[list[tuple[str, Coordinates, Coordinates]]], None]

    def convert(self) -> dict[str, Coordinates]:
        """Convert named columns once, before either native call."""
        return {"x": self.x(), "y": self.y()}

    def plain(self, columns: dict[str, Coordinates], label: str) -> None:
        """Draw the ordinary call without splitting its columns."""
        self.draw(columns["x"], columns["y"], label=label)

    def grouped(
        self, context: str, columns: dict[str, Coordinates], groups: list[_Group]
    ) -> _GroupedCall:
        """Prepare one native call and its input row maps."""
        series = [
            (group.label, _coordinates(group.columns["x"]), _coordinates(group.columns["y"]))
            for group in groups
        ]
        return _GroupedCall(partial(self.draw_groups, series), groups)


@dataclass(frozen=True, slots=True)
class Bars:
    """Converters and native calls for categories with aligned heights."""

    categories: Callable[[], list[str]]
    heights: Callable[[], FloatArray]
    draw: _BarDraw
    draw_groups: Callable[[list[str], list[NativeSeries]], None]

    def convert(self) -> dict[str, Coordinates]:
        """Convert named columns once, before either native call."""
        return {"categories": self.categories(), "heights": self.heights()}

    def plain(self, columns: dict[str, Coordinates], label: str) -> None:
        """Draw the ordinary call without splitting its columns."""
        self.draw(
            cast(list[str], columns["categories"]),
            cast(FloatArray, columns["heights"]),
            label=label,
        )

    def grouped(
        self, context: str, columns: dict[str, Coordinates], groups: list[_Group]
    ) -> _GroupedCall:
        """Prepare one native call and its input row maps."""
        categories, series, aligned = _bar_arguments(context, groups, columns["categories"])
        return _GroupedCall(partial(self.draw_groups, categories, series), aligned)


@dataclass(frozen=True, slots=True)
class Samples:
    """Converters and native calls for numeric samples with a color per group."""

    values: Callable[[], FloatArray]
    draw: _ValuesDraw[FloatArray]
    draw_groups: Callable[[list[NativeSeries]], None]

    def convert(self) -> dict[str, Coordinates]:
        """Convert named columns once, before either native call."""
        return {"values": self.values()}

    def plain(self, columns: dict[str, Coordinates], label: str) -> None:
        """Draw the ordinary call without splitting its columns."""
        self.draw(cast(FloatArray, columns["values"]), label=label)

    def grouped(
        self, context: str, columns: dict[str, Coordinates], groups: list[_Group]
    ) -> _GroupedCall:
        """Prepare one native call and its input row maps."""
        series = [
            (group.label, cast(FloatArray, group.columns["values"]), _color.native(Color.AUTO))
            for group in groups
        ]
        return _GroupedCall(partial(self.draw_groups, series), groups)


@dataclass(frozen=True, slots=True)
class Ecdf:
    """Converters and native calls for numeric or time samples."""

    values: Callable[[], FloatArray | MicrosArray]
    draw: _ValuesDraw[FloatArray | MicrosArray]
    draw_groups: Callable[[list[tuple[str, Coordinates]]], None]

    def convert(self) -> dict[str, Coordinates]:
        """Convert named columns once, before either native call."""
        return {"values": self.values()}

    def plain(self, columns: dict[str, Coordinates], label: str) -> None:
        """Draw the ordinary call without splitting its columns."""
        self.draw(cast(FloatArray | MicrosArray, columns["values"]), label=label)

    def grouped(
        self, context: str, columns: dict[str, Coordinates], groups: list[_Group]
    ) -> _GroupedCall:
        """Prepare one native call and its input row maps."""
        series = [(group.label, _coordinates(group.columns["values"])) for group in groups]
        return _GroupedCall(partial(self.draw_groups, series), groups)


def call(
    context: str,
    by: Values[str] | Auto,
    label: str,
    color: object,
    method: XY | Bars | Samples | Ecdf,
) -> None:
    """Check grouping, convert once, and dispatch either ordinary or grouped data."""
    if by is not AUTO:
        check_names_type(context, "by", by)
        _check_style(context, label, color)
    try:
        columns = method.convert()
    except UmberError as error:
        row = cast(int | None, getattr(error, "_umber_row", None))
        info = details(error)
        if by is AUTO or row is None or info is None:
            raise
        labels = _labels(context, by)
        if row >= len(labels.values):
            raise
        group = _name(context, row, labels.at(row))
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
        method.plain(columns, name)
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
    grouped = method.grouped(context, columns, groups)
    try:
        grouped.invoke()
    except UmberError as error:
        raise _mapped_error(context, error, grouped.groups) from None
