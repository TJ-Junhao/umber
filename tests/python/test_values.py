"""Tests of the conversion of data arguments: numbers, names and x values."""

import ast
import ctypes
import datetime
import doctest
import tracemalloc
from collections.abc import Callable, Iterator, Sequence
from decimal import Decimal
from fractions import Fraction
from pathlib import Path
from types import ModuleType
from typing import overload, override

import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl
import pytest

import umber as ub
from umber import _types, _values
from umber._args import short_repr
from umber._errors import details
from umber._values import FloatArray, axis_values, floats, names

SINGLE_HINT = "wrap a single value in a list."
NAMES_HINT = 'pass a list of names, e.g. ["A", "B"].'
AXIS_HINT = 'pass a list of names or numbers, e.g. ["A", "B"] or [1.0, 2.0].'
ITERATOR_HINT = "wrap it in list()."
SET_HINT = "a set has no order; pass a list, e.g. sorted(values)."
MAPPING_HINT = "pass the values as a list, e.g. list(values.values())."
OTHER_HINT = "pass a list or an array."
MASKED_HINT = "fill them with NaN: values.filled(np.nan)."
MASKED_NAMES_HINT = 'fill them with a name first, e.g. values.filled("n/a").'
NDIM_HINT = "pass one column, or flatten it with .ravel()."
BOOL_DTYPE_HINT = "convert with .astype(float) if 0 and 1 are meant."
BOOL_HINT = "use 1.0 and 0.0 if numbers are meant."
TIME_HINT = "convert to numbers, e.g. seconds since the first value."
COMPLEX_HINT = "pass the real part, values.real, or the magnitude, abs(values)."
WIDE_FLOAT_HINT = "convert with .astype(np.float64); umber plots in float64 precision."
STRUCTURED_HINT = "pass one numeric field of the structured array."
MISSING_HINT = "use NaN for missing values, e.g. series.to_numpy(dtype=float, na_value=np.nan)."
TEXT_HINT = "convert text to numbers first, e.g. with float()."
NESTED_HINT = "pass one number per point; nested sequences are not flattened."
NUMBER_HINT = "convert the values with float() first."
NAME_HINT = "convert each name with str() first."
NAME_DTYPE_HINT = "convert them with .astype(str) first."
MISSING_NAME_HINT = 'give every category a name, e.g. "n/a" for missing ones.'
MIXED_HINT = "pass only names or only numbers."
TOO_LONG_HINT = "shorten the text; it would not fit the figure."
SURROGATE_HINT = 'remove the character, or replace it: value.encode("utf-8", "replace").decode().'


def error(cls: type[BaseException], call: Callable[[], object]) -> tuple[str, object, object]:
    """Run a call that must raise exactly ``cls``; return its message, parameter and hint."""
    with pytest.raises(cls) as info:
        call()
    assert type(info.value) is cls
    found = details(info.value)
    assert found is not None
    return str(info.value), found[0], found[1]


def y_error(cls: type[BaseException], values: object) -> tuple[str, object, object]:
    return error(cls, lambda: floats("Axes.line()", "y", values))


def category_error(cls: type[BaseException], values: object) -> tuple[str, object, object]:
    return error(cls, lambda: names("Axes.bar()", "categories", values))


def x_error(cls: type[BaseException], values: object) -> tuple[str, object, object]:
    return error(cls, lambda: axis_values("Axes.line()", "x", values))


def assert_native(result: FloatArray, expected: Sequence[float]) -> None:
    """The array is what the native module reads: 1-D, aligned, native-endian float64."""
    assert isinstance(result, np.ndarray)
    assert result.ndim == 1
    assert result.dtype == np.float64
    assert result.dtype.isnative
    assert result.flags.aligned
    assert memoryview(result).format == "d"
    np.testing.assert_array_equal(result, np.asarray(expected, dtype=np.float64))


class Points(Sequence[float]):
    """A user-defined sequence."""

    def __init__(self, *values: float) -> None:
        self._values = values

    @overload
    def __getitem__(self, index: int) -> float: ...
    @overload
    def __getitem__(self, index: slice) -> Sequence[float]: ...
    @override
    def __getitem__(self, index: int | slice) -> float | Sequence[float]:
        return self._values[index]

    @override
    def __len__(self) -> int:
        return len(self._values)


class ArrayLike:
    """An object that only implements ``__array__``."""

    def __array__(self) -> npt.NDArray[np.float64]:
        return np.array([1.0, 2.0])


def generated() -> Iterator[float]:
    yield 1.0


# --- containers ---


@pytest.mark.parametrize(
    ("values", "shown", "hint"),
    [
        ("1.5", "str", SINGLE_HINT),
        (np.str_("1.5"), "str_", SINGLE_HINT),
        (b"12", "bytes", SINGLE_HINT),
        (bytearray(b"12"), "bytearray", SINGLE_HINT),
        (3, "int", SINGLE_HINT),
        (2.5, "float", SINGLE_HINT),
        (True, "bool", SINGLE_HINT),
        (Decimal(1), "Decimal", SINGLE_HINT),
        (np.float64(1.0), "float64", SINGLE_HINT),
        (np.int64(1), "int64", SINGLE_HINT),
        (np.True_, "bool", SINGLE_HINT),
        (np.datetime64("2020-01-01"), "datetime64", SINGLE_HINT),
        (None, "None", OTHER_HINT),
        (object(), "object", OTHER_HINT),
        (generated(), "generator", ITERATOR_HINT),
        (iter([1.0]), "list_iterator", ITERATOR_HINT),
        (map(float, [1]), "map", ITERATOR_HINT),
        ({1.0, 2.0}, "set", SET_HINT),
        (frozenset({1.0}), "frozenset", SET_HINT),
        ({"a": 1.0}.keys(), "dict_keys", SET_HINT),
        ({"a": 1.0}, "dict", MAPPING_HINT),
        ({"a": 1.0}.values(), "dict_values", OTHER_HINT),
    ],
)
def test_floats_rejects_values_that_are_not_sequences_or_arrays(
    values: object, shown: str, hint: str
) -> None:
    assert y_error(TypeError, values) == (
        f"Axes.line(): y is {shown}; y must be a sequence or an array.",
        "y",
        hint,
    )


@pytest.mark.parametrize(
    ("values", "shown", "hint"),
    [
        ("North", "str", NAMES_HINT),
        (b"North", "bytes", NAMES_HINT),
        (3, "int", SINGLE_HINT),
        (None, "None", OTHER_HINT),
        ((name for name in ["a"]), "generator", ITERATOR_HINT),
        ({"a"}, "set", SET_HINT),
        ({"a": 1}, "dict", MAPPING_HINT),
    ],
)
def test_names_rejects_values_that_are_not_sequences_or_arrays(
    values: object, shown: str, hint: str
) -> None:
    assert category_error(TypeError, values) == (
        f"Axes.bar(): categories is {shown}; categories must be a sequence or an array.",
        "categories",
        hint,
    )


def test_axis_values_rejects_text_with_its_own_hint() -> None:
    assert x_error(TypeError, "Q1") == (
        "Axes.line(): x is str; x must be a sequence or an array.",
        "x",
        AXIS_HINT,
    )


@pytest.mark.parametrize(
    "values",
    [
        np.ma.array([1.0, 2.0], mask=[False, True]),
        np.ma.array([1.0, 2.0]),
        np.ma.array([[1.0]], mask=[[True]]),
    ],
)
def test_masked_arrays_are_refused(values: object) -> None:
    message = "{} is a masked array; masked values cannot be plotted."
    assert y_error(ub.DataError, values) == (
        "Axes.line(): " + message.format("y"),
        "y",
        MASKED_HINT,
    )
    assert x_error(ub.DataError, values) == (
        "Axes.line(): " + message.format("x"),
        "x",
        MASKED_HINT,
    )
    masked_names = np.ma.array(["a", "b"], mask=[False, True])
    assert category_error(ub.DataError, masked_names) == (
        "Axes.bar(): " + message.format("categories"),
        "categories",
        MASKED_NAMES_HINT,
    )


# --- numbers: accepted input ---


@pytest.mark.parametrize(
    ("values", "expected"),
    [
        ([1, 2.5, -3], [1.0, 2.5, -3.0]),
        ((1.0, 2.0), [1.0, 2.0]),
        (range(3), [0.0, 1.0, 2.0]),
        (range(10, 4, -3), [10.0, 7.0]),
        (Points(1.5, 2.5), [1.5, 2.5]),
        (ArrayLike(), [1.0, 2.0]),
        ([np.float32(0.5), np.int16(2)], [0.5, 2.0]),
        ([float("nan"), float("inf")], [float("nan"), float("inf")]),
        (memoryview(np.array([1.0, 2.0])), [1.0, 2.0]),
    ],
)
def test_floats_converts_sequences(values: object, expected: list[float]) -> None:
    assert_native(floats("Axes.line()", "y", values), expected)


@pytest.mark.parametrize(
    "dtype",
    ["f2", "f4", "f8", "i1", "i2", "i4", "i8", "u1", "u2", "u4", "u8", ">f8", ">f4", ">i4", "<f8"],
)
def test_floats_converts_numeric_dtypes_exactly(dtype: str) -> None:
    values = np.array([0, 1, 2, 100], dtype=dtype)
    assert_native(floats("Axes.line()", "y", values), [0.0, 1.0, 2.0, 100.0])


def test_floats_keeps_float_fractions_exactly() -> None:
    for dtype in ("f2", "f4"):
        values = np.array([0.1, -2.75, 65504], dtype=dtype)
        result = floats("Axes.line()", "y", values)
        assert result.tolist() == [float(v) for v in values]


def test_integers_are_exact_up_to_two_to_the_53() -> None:
    exact = np.array([2**53, -(2**53)], dtype=np.int64)
    assert floats("Axes.line()", "y", exact).tolist() == [2.0**53, -(2.0**53)]
    rounded = np.array([2**53 + 1], dtype=np.int64)
    assert floats("Axes.line()", "y", rounded).tolist() == [2.0**53]
    assert floats("Axes.line()", "y", np.array([2**64 - 1], dtype=np.uint64)).tolist() == [2.0**64]


def test_contiguous_native_float64_is_not_copied() -> None:
    values = np.array([1.0, 2.0, 3.0])
    result = floats("Axes.line()", "y", values)
    assert np.shares_memory(result, values)
    values[0] = 9.0
    assert result[0] == 9.0


def test_strided_aligned_float64_is_not_copied() -> None:
    values = np.arange(10.0)
    for view in (values[::2], values[::-1], np.arange(12.0).reshape(3, 4)[:, 1]):
        result = floats("Axes.line()", "y", view)
        assert np.shares_memory(result, view)
        assert_native(result, view.tolist())


def test_float_series_is_not_copied() -> None:
    series = pd.Series([1.0, 2.0, 3.0])
    result = floats("Axes.line()", "y", series)
    assert np.shares_memory(result, series.to_numpy())
    assert_native(result, [1.0, 2.0, 3.0])


def test_misaligned_float64_is_copied_to_aligned_memory() -> None:
    raw = np.zeros(8 * 3 + 1, dtype=np.uint8)
    values = np.frombuffer(raw.data, dtype=np.float64, count=3, offset=1)
    assert not values.flags.aligned
    assert_native(floats("Axes.line()", "y", values), [0.0, 0.0, 0.0])


def test_big_endian_float64_is_converted_to_native_order() -> None:
    values = np.array([1.5, -2.0], dtype=">f8")
    assert_native(floats("Axes.line()", "y", values), [1.5, -2.0])
    assert_native(floats("Axes.line()", "y", values[::-1]), [-2.0, 1.5])


def test_ctypes_buffer_loses_its_explicit_byte_order_without_a_copy() -> None:
    buffer = (ctypes.c_double * 3)(1.0, 2.0, 3.0)
    source = np.asarray(buffer)
    assert source.dtype.byteorder in "<>"
    result = floats("Axes.line()", "y", source)
    assert np.shares_memory(result, source)
    assert_native(result, [1.0, 2.0, 3.0])


@pytest.mark.parametrize(
    "values",
    [[], (), range(0), np.array([]), np.array([], dtype=bool), np.array([], dtype="U1")],
)
def test_empty_numbers_give_an_empty_array(values: object) -> None:
    assert_native(floats("Axes.line()", "y", values), [])


# --- numbers: shape ---


@pytest.mark.parametrize(
    ("values", "dimensions"),
    [
        (np.array(1.0), 0),
        (np.ones((2, 3)), 2),
        (np.empty((0, 3)), 2),
        ([[1.0, 2.0], [3.0, 4.0]], 2),
        (np.ones((1, 1, 1)), 3),
        (pd.DataFrame({"a": [1.0]}), 2),
    ],
)
def test_floats_requires_one_dimension(values: object, dimensions: int) -> None:
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y has {dimensions} dimensions; values must be one-dimensional.",
        "y",
        NDIM_HINT,
    )


@pytest.mark.parametrize(
    ("values", "index"), [([1.0, [2.0, 3.0]], 1), ([[1.0, 2.0], [3.0]], 0), ([1.0, (2.0,)], 1)]
)
def test_ragged_sequences_name_the_first_nested_element(values: object, index: int) -> None:
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y[{index}] is a sequence; values must be numbers.",
        "y",
        NESTED_HINT,
    )


# --- numbers: dtypes ---


@pytest.mark.parametrize(
    "values",
    [np.array([True, False]), pd.Series([True]), pl.Series([True, False])],
)
def test_bool_data_is_refused(values: object) -> None:
    assert y_error(ub.DataError, values) == (
        "Axes.line(): y has dtype bool; values must be numbers.",
        "y",
        BOOL_DTYPE_HINT,
    )


@pytest.mark.parametrize(
    ("values", "index", "shown"),
    [
        ([1.0, True], 1, "True"),
        ((np.False_, 2), 0, "np.False_"),
        ([0, 1, False], 2, "False"),
        ([True, False], 0, "True"),
        ([1.0, np.array(True)], 1, "np.True_"),
    ],
)
def test_a_bool_among_numbers_is_refused(values: object, index: int, shown: str) -> None:
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y[{index}] is {shown}; values must be numbers.",
        "y",
        BOOL_HINT,
    )


@pytest.mark.parametrize(
    ("values", "dtype"),
    [
        (np.array(["2020-01-01"], dtype="datetime64[ns]"), "datetime64[ns]"),
        (np.array([1, 2], dtype="timedelta64[s]"), "timedelta64[s]"),
        (pd.Series(pd.to_datetime(["2020-01-01"])), str(pd.to_datetime(["2020-01-01"]).dtype)),
        (pd.Series(pd.to_timedelta([1, 2], unit="s")), str(pd.to_timedelta([1], unit="s").dtype)),
    ],
)
def test_datetime_and_timedelta_data_is_refused(values: object, dtype: str) -> None:
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y has dtype {dtype}; values must be numbers.",
        "y",
        TIME_HINT,
    )


@pytest.mark.parametrize(
    ("values", "index", "shown"),
    [
        ([1.0, datetime.datetime(2020, 1, 2)], 1, "datetime"),
        ([np.datetime64("2020-01-01", "D")], 0, "np.datetime64('2020-01-01')"),
        ([datetime.date(2020, 1, 2)], 0, "date"),
        ([1.0, datetime.timedelta(seconds=3)], 1, "timedelta"),
        ([1, np.datetime64("2020-01-01")], 1, "np.datetime64('2020-01-01')"),
        (np.array([1, np.timedelta64(2, "s")], dtype=object), 1, "np.timedelta64(2,'s')"),
        (
            pd.Series(pd.to_datetime(["2020-01-01"]).tz_localize("UTC")),
            0,
            "Timestamp",
        ),
    ],
)
def test_datetime_elements_are_refused(values: object, index: int, shown: str) -> None:
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y[{index}] is {shown}; values must be numbers.",
        "y",
        TIME_HINT,
    )


@pytest.mark.parametrize(
    ("values", "dtype"),
    [
        (np.array([1 + 2j]), "complex128"),
        (np.array([1j], "c8"), "complex64"),
    ],
)
def test_complex_data_is_refused(values: object, dtype: str) -> None:
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y has dtype {dtype}; values must be real numbers.",
        "y",
        COMPLEX_HINT,
    )


@pytest.mark.parametrize("values", [np.array([1.0, 2j], dtype=object), [1.0, 2j], (1, 2j)])
def test_complex_elements_are_refused(values: object) -> None:
    assert y_error(ub.DataError, values) == (
        "Axes.line(): y[1] is complex; values must be real numbers.",
        "y",
        COMPLEX_HINT,
    )


def test_structured_data_is_refused() -> None:
    values = np.zeros(2, dtype=[("a", "<f8"), ("b", "<i4")])
    assert y_error(ub.DataError, values) == (
        "Axes.line(): y has dtype [('a', '<f8'), ('b', '<i4')]; values must be numbers.",
        "y",
        STRUCTURED_HINT,
    )


@pytest.mark.skipif(np.dtype(np.longdouble).itemsize <= 8, reason="long double is float64 here")
def test_floats_wider_than_float64_are_refused() -> None:
    values = np.array([1.0], dtype=np.longdouble)
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y has dtype {values.dtype}; values must be float64 or narrower.",
        "y",
        WIDE_FLOAT_HINT,
    )


@pytest.mark.skipif(np.dtype(np.longdouble).itemsize > 8, reason="long double is wider here")
def test_long_double_that_is_float64_is_accepted() -> None:
    assert_native(floats("Axes.line()", "y", np.array([1.5], dtype=np.longdouble)), [1.5])


@pytest.mark.parametrize(
    ("values", "index", "shown"),
    [
        (np.array(["1", "2"]), 0, "'1'"),
        ([1.0, "a"], 1, "'a'"),
        ([1.0, 2.0, np.str_("b")], 2, "'b'"),
        (np.array([b"a"]), 0, "bytes"),
        ([b"x", 1.0], 0, "bytes"),
        (np.array(["a"], dtype=np.dtypes.StringDType()), 0, "'a'"),
        (pd.Series(["a", "b"]), 0, "'a'"),
        (pl.Series(["a", "b"]), 0, "'a'"),
        (np.array([1.0, "a"], dtype=object), 1, "'a'"),
        ([1.0, "x" * 50], 1, "'" + "x" * 38 + "…"),
    ],
)
def test_text_among_numbers_names_the_first_text_element(
    values: object, index: int, shown: str
) -> None:
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y[{index}] is {shown}; values must be numbers.",
        "y",
        TEXT_HINT,
    )


@pytest.mark.parametrize(
    ("values", "index", "shown"),
    [
        ([1.0, None, 3.0], 1, "None"),
        ([None], 0, "None"),
        (pd.Series([1.0, pd.NA], dtype=object), 1, "NAType"),
        (pd.Series([1.0, pd.NaT], dtype=object), 1, "NaTType"),
        (np.array([2.0, None], dtype=object), 1, "None"),
    ],
)
def test_missing_values_are_refused_with_the_nan_hint(
    values: object, index: int, shown: str
) -> None:
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y[{index}] is {shown}; values must be numbers.",
        "y",
        MISSING_HINT,
    )


@pytest.mark.parametrize(
    ("values", "index", "shown"),
    [([1.0, Decimal("1.5")], 1, "Decimal"), ([Fraction(1, 3)], 0, "Fraction")],
)
def test_other_number_types_are_refused(values: object, index: int, shown: str) -> None:
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y[{index}] is {shown}; values must be int or float.",
        "y",
        NUMBER_HINT,
    )


def test_other_objects_are_refused() -> None:
    assert y_error(ub.DataError, [1.0, {"a": 1}]) == (
        "Axes.line(): y[1] is dict; values must be numbers.",
        "y",
        NUMBER_HINT,
    )


def test_object_arrays_of_numbers_are_converted() -> None:
    values = np.array([1, 2.5, np.float32(0.5), np.int8(3)], dtype=object)
    assert_native(floats("Axes.line()", "y", values), [1.0, 2.5, 0.5, 3.0])


def test_ints_beyond_the_float_range_become_infinity() -> None:
    # The core library then reports them as infinite values.
    assert_native(floats("Axes.line()", "y", [1, 10**400, -(10**400)]), [1, np.inf, -np.inf])


# --- numbers: pandas and polars ---


def test_pandas_series() -> None:
    assert_native(floats("Axes.line()", "y", pd.Series([1.5, 2.5])), [1.5, 2.5])
    assert_native(floats("Axes.line()", "y", pd.Series([1, 2], dtype="Int64")), [1.0, 2.0])
    with_na = floats("Axes.line()", "y", pd.Series([1, None], dtype="Int64"))
    assert_native(with_na, [1.0, np.nan])
    assert_native(floats("Axes.line()", "y", pd.Series([1.0, 2.0], index=[5, 9])), [1.0, 2.0])


def test_polars_series() -> None:
    assert_native(floats("Axes.line()", "y", pl.Series([1.0, None, 3.0])), [1.0, np.nan, 3.0])
    assert_native(floats("Axes.line()", "y", pl.Series([1, 2])), [1.0, 2.0])


# --- names ---


class Name(str):
    def __str__(self) -> str:
        return "overridden"


@pytest.mark.parametrize(
    ("values", "expected"),
    [
        (["North", "South"], ["North", "South"]),
        (("A",), ["A"]),
        ([Name("East"), np.str_("West")], ["East", "West"]),
        (np.array(["a", "bb"]), ["a", "bb"]),
        (np.array(["a", "b"], dtype=object), ["a", "b"]),
        (np.array(["a"], dtype=np.dtypes.StringDType()), ["a"]),
        (pd.Series(["a", "b"]), ["a", "b"]),
        (pd.Series(["a", "b"], dtype="category"), ["a", "b"]),
        (pd.Series(["a", "b"], dtype="string"), ["a", "b"]),
        (pl.Series(["a", "b"]), ["a", "b"]),
        (pl.Series(["a", "b"], dtype=pl.Categorical), ["a", "b"]),
        ([], []),
        (np.array([]), []),
        (np.array([], dtype=bool), []),
        (range(0), []),
    ],
)
def test_names_converts_text(values: object, expected: list[str]) -> None:
    result = names("Axes.bar()", "categories", values)
    assert result == expected
    assert type(result) is list
    assert all(type(name) is str for name in result)


@pytest.mark.parametrize(
    ("values", "index", "shown", "hint"),
    [
        (["a", 1], 1, "1", NAME_HINT),
        (["a", 2.5], 1, "2.5", NAME_HINT),
        (["a", True], 1, "True", NAME_HINT),
        ([b"a"], 0, "bytes", NAME_HINT),
        (["a", ["b"]], 1, "list", NAME_HINT),
        (range(2), 0, "0", NAME_HINT),
        (["a", None], 1, "None", MISSING_NAME_HINT),
        (["a", float("nan")], 1, "nan", MISSING_NAME_HINT),
        (np.array([b"a"]), 0, "bytes", NAME_HINT),
        (np.array(["a", 1], dtype=object), 1, "1", NAME_HINT),
        (pd.Series(["a", None]), 1, "nan", MISSING_NAME_HINT),
        (pd.Series(["a", None], dtype="string"), 1, "NAType", MISSING_NAME_HINT),
        (pl.Series(["a", None]), 1, "None", MISSING_NAME_HINT),
    ],
)
def test_names_must_be_strings(values: object, index: int, shown: str, hint: str) -> None:
    assert category_error(ub.DataError, values) == (
        f"Axes.bar(): categories[{index}] is {shown}; categories must be strings.",
        "categories",
        hint,
    )


@pytest.mark.parametrize(
    ("values", "dtype"),
    [
        (np.array([1.0, 2.0]), "float64"),
        (np.array([1, 2], dtype=np.int32), "int32"),
        (np.array([True]), "bool"),
        (pd.Series([1.5]), "float64"),
        (pl.Series([1, 2]), "int64"),
    ],
)
def test_names_refuses_numeric_dtypes(values: object, dtype: str) -> None:
    assert category_error(ub.DataError, values) == (
        f"Axes.bar(): categories has dtype {dtype}; categories must be strings.",
        "categories",
        NAME_DTYPE_HINT,
    )


@pytest.mark.parametrize(
    ("values", "dimensions"),
    [(np.array("a"), 0), (np.array([["a"], ["b"]]), 2), (np.empty((0, 2), dtype="U1"), 2)],
)
def test_names_requires_one_dimension(values: object, dimensions: int) -> None:
    assert category_error(ub.DataError, values) == (
        f"Axes.bar(): categories has {dimensions} dimensions; values must be one-dimensional.",
        "categories",
        NDIM_HINT,
    )


@pytest.mark.parametrize("make", [list, np.array])
def test_each_name_is_capped_at_the_text_length(make: Callable[[list[str]], object]) -> None:
    assert names("Axes.bar()", "categories", make(["a", "b" * 1000])) == ["a", "b" * 1000]
    assert category_error(ub.StyleError, make(["a", "b" * 1001])) == (
        "Axes.bar(): categories[1] has 1,001 characters; text must be at most 1,000 characters.",
        "categories",
        TOO_LONG_HINT,
    )


@pytest.mark.parametrize("make", [list, np.array])
def test_names_with_a_lone_surrogate_are_refused(make: Callable[[list[str]], object]) -> None:
    assert category_error(ub.DataError, make(["a", "b\udfff"])) == (
        "Axes.bar(): categories[1] has a lone surrogate at index 1; text must be valid Unicode.",
        "categories",
        SURROGATE_HINT,
    )


def test_names_are_checked_in_order() -> None:
    message, _, _ = category_error(ub.DataError, ["\ud800", 1])
    assert message.startswith("Axes.bar(): categories[0] has a lone surrogate")
    message, _, _ = category_error(ub.DataError, ["a", 1, "\ud800"])
    assert message.startswith("Axes.bar(): categories[1] is 1;")


# --- x values ---


@pytest.mark.parametrize(
    ("values", "expected"),
    [
        (["Q1", "Q2"], ["Q1", "Q2"]),
        ((np.str_("a"),), ["a"]),
        (np.array(["a", "b"]), ["a", "b"]),
        (np.array(["a", "b"], dtype=object), ["a", "b"]),
        (np.array(["a"], dtype=np.dtypes.StringDType()), ["a"]),
        (pd.Series(["a", "b"]), ["a", "b"]),
        (pd.Series(["a", "b"], dtype="category"), ["a", "b"]),
        (pl.Series(["a", "b"]), ["a", "b"]),
    ],
)
def test_axis_values_converts_names(values: object, expected: list[str]) -> None:
    assert axis_values("Axes.line()", "x", values) == expected


@pytest.mark.parametrize(
    ("values", "expected"),
    [
        ([2024, 2025], [2024.0, 2025.0]),
        (range(2), [0.0, 1.0]),
        (np.array([1.5]), [1.5]),
        (pd.Series([1, 2]), [1.0, 2.0]),
        (pl.Series([1.0, None]), [1.0, np.nan]),
        ([], []),
        (np.array([], dtype="U1"), []),
        (np.array([], dtype=object), []),
    ],
)
def test_axis_values_converts_numbers(values: object, expected: list[float]) -> None:
    result = axis_values("Axes.line()", "x", values)
    assert isinstance(result, np.ndarray)
    assert_native(result, expected)


def test_axis_values_does_not_copy_float64() -> None:
    values = np.arange(4.0)
    result = axis_values("Axes.scatter()", "x", values)
    assert isinstance(result, np.ndarray)
    assert np.shares_memory(result, values)


@pytest.mark.parametrize(
    ("values", "index", "shown"),
    [
        (["a", "b", 3.0], 2, "3.0"),
        (["a", 1], 1, "1"),
        (np.array(["a", np.int64(4)], dtype=object), 1, "np.int64(4)"),
        ([1.0, "a"], 1, "'a'"),
        ([0, 1, "b"], 2, "'b'"),
        (np.array([2.0, "a"], dtype=object), 1, "'a'"),
    ],
)
def test_axis_values_refuses_mixed_names_and_numbers(
    values: object, index: int, shown: str
) -> None:
    assert x_error(ub.DataError, values) == (
        f"Axes.line(): x[{index}] is {shown}; x mixes names and numbers.",
        "x",
        MIXED_HINT,
    )


def test_axis_values_reports_missing_names_and_other_elements() -> None:
    assert x_error(ub.DataError, ["a", None]) == (
        "Axes.line(): x[1] is None; categories must be strings.",
        "x",
        MISSING_NAME_HINT,
    )
    assert x_error(ub.DataError, ["a", True]) == (
        "Axes.line(): x[1] is True; categories must be strings.",
        "x",
        NAME_HINT,
    )
    assert x_error(ub.DataError, [1.0, None]) == (
        "Axes.line(): x[1] is None; values must be numbers.",
        "x",
        MISSING_HINT,
    )


def test_axis_values_applies_the_text_rules_to_names() -> None:
    message, parameter, hint = x_error(ub.StyleError, ["a", "b" * 1001])
    assert message.startswith("Axes.line(): x[1] has 1,001 characters;")
    assert (parameter, hint) == ("x", TOO_LONG_HINT)


def test_axis_values_requires_one_dimension() -> None:
    assert x_error(ub.DataError, np.array([["a"]])) == (
        "Axes.line(): x has 2 dimensions; values must be one-dimensional.",
        "x",
        NDIM_HINT,
    )


# --- attribute docstrings, which --doctest-modules does not collect ---


def _alias_doc(module_path: Path, name: str) -> str:
    body = ast.parse(module_path.read_text(encoding="utf-8")).body
    for statement, following in zip(body, body[1:], strict=False):
        if isinstance(statement, ast.TypeAlias) and statement.name.id == name:
            assert isinstance(following, ast.Expr)
            assert isinstance(following.value, ast.Constant)
            assert isinstance(following.value.value, str)
            return following.value.value
    raise AssertionError(f"{name} is not documented")


@pytest.mark.usefixtures("isolated_environment")
@pytest.mark.parametrize(("module", "name"), [(_types, "Values"), (_values, "FloatArray")])
def test_alias_examples_run(module: ModuleType, name: str) -> None:
    filename = str(module.__file__)
    doc = _alias_doc(Path(filename), name)
    test = doctest.DocTestParser().get_doctest(doc, {}, name, filename, 0)
    assert test.examples
    runner = doctest.DocTestRunner()
    runner.run(test)
    assert runner.summarize(verbose=False).failed == 0


# --- masked elements, bounded messages and wide floats inside sequences ---


@pytest.mark.parametrize(
    ("values", "index"),
    [
        ([1.0, np.ma.masked, 3.0], 1),
        ((np.ma.masked,), 0),
        ([1.0, np.ma.array([1.0, 2.0], mask=[False, True])[1]], 1),
        ([np.ma.array(2.0)], 0),
        ([1.0, np.ma.array(2.0, mask=True)], 1),
        (np.array([1.0, np.ma.masked], dtype=object), 1),
    ],
    ids=["list", "tuple", "masked-scalar", "zero-dim", "zero-dim-masked", "object-array"],
)
def test_masked_elements_are_refused(values: object, index: int) -> None:
    assert y_error(ub.DataError, values) == (
        f"Axes.line(): y[{index}] is masked; masked values cannot be plotted.",
        "y",
        MASKED_HINT,
    )
    assert x_error(ub.DataError, values) == (
        f"Axes.line(): x[{index}] is masked; masked values cannot be plotted.",
        "x",
        MASKED_HINT,
    )


def test_masked_names_are_refused() -> None:
    assert category_error(ub.DataError, ["a", np.ma.masked]) == (
        "Axes.bar(): categories[1] is masked; masked values cannot be plotted.",
        "categories",
        MASKED_NAMES_HINT,
    )


def test_unmasked_zero_dimensional_arrays_are_numbers() -> None:
    assert_native(floats("Axes.line()", "y", [np.array(1.5), np.array(2, dtype=np.int8)]), [1.5, 2])


class FakeLength(str):
    def __len__(self) -> int:
        return 1


def test_name_cap_ignores_an_overridden_length() -> None:
    assert category_error(ub.StyleError, ["a", FakeLength("b" * 1001)]) == (
        "Axes.bar(): categories[1] has 1,001 characters; text must be at most 1,000 characters.",
        "categories",
        TOO_LONG_HINT,
    )


def test_messages_do_not_format_huge_values() -> None:
    assert y_error(ub.DataError, [Fraction(10**5000, 3)]) == (
        "Axes.line(): y[0] is Fraction; values must be int or float.",
        "y",
        NUMBER_HINT,
    )
    bits = (10**5000).bit_length()
    assert y_error(ub.DataError, [{"a": 10**5000}]) == (
        "Axes.line(): y[0] is dict; values must be numbers.",
        "y",
        NUMBER_HINT,
    )
    assert category_error(ub.DataError, [10**5000]) == (
        f"Axes.bar(): categories[0] is int ({bits:,} bits); categories must be strings.",
        "categories",
        NAME_HINT,
    )


def object_array(items: list[object]) -> npt.NDArray[np.object_]:
    return np.array(items, dtype=object)


@pytest.mark.skipif(np.dtype(np.longdouble).itemsize <= 8, reason="long double is float64 here")
@pytest.mark.parametrize("make", [list, object_array])
def test_wide_float_elements_are_refused(make: Callable[[list[object]], object]) -> None:
    wide = np.longdouble(2)
    assert y_error(ub.DataError, make([1.0, wide])) == (
        f"Axes.line(): y[1] is {short_repr(wide)}; values must be float64 or narrower.",
        "y",
        WIDE_FLOAT_HINT,
    )


@pytest.mark.skipif(np.dtype(np.longdouble).itemsize > 8, reason="long double is wider here")
@pytest.mark.parametrize("make", [list, object_array])
def test_long_double_elements_that_are_float64_are_accepted(
    make: Callable[[list[object]], object],
) -> None:
    assert_native(floats("Axes.line()", "y", make([1.0, np.longdouble(2)])), [1.0, 2.0])


def test_names_stop_at_the_first_invalid_element_without_converting_the_rest() -> None:
    values = np.broadcast_to(np.array(["x" * 1001]), (20_000,))
    tracemalloc.start()
    try:
        message, _, _ = category_error(ub.StyleError, values)
        _, peak = tracemalloc.get_traced_memory()
    finally:
        tracemalloc.stop()
    assert message.startswith("Axes.bar(): categories[0] has 1,001 characters;")
    assert peak < 4_000_000


def test_nullable_missing_values_arrive_as_nan() -> None:
    # pandas and polars turn NA and null in numeric data into NaN themselves; NaN is a gap.
    for values in (
        pd.Series([1, None, 3], dtype="Int64"),
        pd.Series([1.5, None], dtype="Float64"),
        pl.Series([1, None, 3]),
        pl.Series([1.5, None]),
    ):
        result = floats("Axes.line()", "y", values)
        assert np.isnan(result[1])
        assert_native(result, np.asarray(values, dtype=np.float64).tolist())


@pytest.mark.parametrize(
    "values",
    [[[1.0], None], [[1.0], True], [(1.0, 2.0), np.ma.masked], [[1.0], 2j], [np.ones(2), "a"]],
    ids=["none", "bool", "masked", "complex", "text"],
)
def test_the_earliest_offending_element_is_reported(values: object) -> None:
    assert y_error(ub.DataError, values) == (
        "Axes.line(): y[0] is a sequence; values must be numbers.",
        "y",
        NESTED_HINT,
    )
