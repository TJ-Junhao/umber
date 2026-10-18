"""Grouping with polars columns: labels, data values, validation and conversion order.
Left out where polars is not installed (the free-threaded CI job)."""

from collections.abc import Callable
from typing import Any, cast

import numpy as np
import polars as pl
import pytest

import umber as ub

type Axes = ub.Axes[Any, Any]
METHODS = ("line", "scatter", "step", "area", "ecdf", "kde", "hist", "bar")
XY = ("line", "scatter", "step", "area")
X = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0]
Y = [1.0, 4.0, 3.0, 2.0, 2.0, 5.0]
GROUPS = ["South", "North", "South", "North", "South", "North"]


def _axes(method: str) -> Axes:
    return ub.Figure(6, 4).add_axes(x=ub.Scale.category() if method == "bar" else ub.Scale.linear())


def _svg(ax: Axes) -> str:
    return ax.figure.to_svg(text=ub.SvgText.EDITABLE)


def _call(ax: Axes, method: str, **kwargs: Any) -> object:
    call = cast(Callable[..., object], getattr(ax, method))
    if method in XY:
        return call(X, Y, **kwargs)
    if method == "bar":
        return call(["Q2", "Q2", "Q1", "Q1", "Q3", "Q3"], Y, **kwargs)
    if method == "hist":
        return call(Y, bins=[0, 2, 4, 6], style=ub.Hist.STEP, **kwargs)
    return call(Y, **kwargs)


@pytest.mark.parametrize("method", METHODS)
def test_by_with_polars_columns(method: str) -> None:
    expected = _axes(method)
    _call(expected, method, by=GROUPS)
    ax = _axes(method)
    _call(ax, method, by=pl.Series("Not a label", GROUPS))
    assert _svg(ax) == _svg(expected)


def test_data_columns_keep_values_without_reading_series_names() -> None:
    actual = ub.Figure(6, 4).add_axes()
    expected = ub.Figure(6, 4).add_axes()
    actual.line(pl.Series(X), pl.Series(Y), by=pl.Series(GROUPS))
    expected.line(X, Y, by=GROUPS)
    assert _svg(actual) == _svg(expected)


def test_by_string_columns_validate_names_per_group(monkeypatch: pytest.MonkeyPatch) -> None:
    from umber import _group

    values = ["B", "A", "D", "C"] * 100
    calls: list[int] = []
    original = cast(Callable[[str, int, object], str], getattr(_group, "_name", None))

    def counted(context: str, index: int, value: object) -> str:
        calls.append(index)
        return original(context, index, value)

    monkeypatch.setattr(_group, "_name", counted)
    groups = _group.split_by("Axes.line()", pl.Series(values), np.arange(len(values)))
    assert [label for label, _ in groups] == ["B", "A", "D", "C"]
    assert calls == [0, 1, 2, 3]


def test_polars_object_names_use_the_same_errors_as_lists() -> None:
    from umber._group import split_by

    by: Any
    for by in (["B", 3, "A"], pl.Series(["B", 3, "A"], dtype=pl.Object)):
        with pytest.raises(ub.DataError, match=r"by\[1\] is 3, not a string"):
            split_by("Axes.line()", by, [1, 2, 3])


@pytest.mark.parametrize("size", [3, 4])
def test_trailing_nul_group_names_keep_conversion_and_error_order(size: int) -> None:
    from umber._group import split_by

    with pytest.raises(ub.DataError) as caught:
        split_by("Axes.line()", pl.Series(["B", "\0", "A"]), np.arange(size))
    expected = "by[1] is empty" if size == 3 else "by has 3 values but columns[0] has 4"
    assert expected in str(caught.value)
    if size == 3:
        groups = split_by("Axes.line()", pl.Series(["A\0", "A", "B"]), np.arange(size))
        assert [name for name, _ in groups] == ["A", "B"]
        assert groups[0][1][0].tolist() == [0, 1]
