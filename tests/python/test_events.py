"""Event rows, time coordinates, validation and atomic calls."""

from collections.abc import Callable
from datetime import date
from typing import Any, cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details


def test_events_one_row_per_category() -> None:
    ax = ub.Figure(5, 3).add_axes(y=ub.Scale.category())
    assert ax.events(["North", "Empty", "South"], [[1, 3], [], [2]]) is ax
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert all(f">{name}</text>" in svg for name in ["North", "Empty", "South"])
    assert svg.index(">North</text>") < svg.index(">South</text>")
    assert ax.figure.to_png(dpi=60)


def test_events_on_time_x() -> None:
    ax = ub.Figure(5, 3).add_axes(x=ub.Scale.time(), y=ub.Scale.category())
    ax.events(["A", "B"], [[date(2026, 1, 1), date(2026, 1, 3)], [date(2026, 1, 2)]])
    expected = ax.figure.to_svg()
    other = ub.Figure(5, 3).add_axes(x=ub.Scale.time(), y=ub.Scale.category())
    other.events(
        ["A", "B"],
        [
            np.array(["2026-01-01", "NaT", "2026-01-03"], dtype="datetime64[D]"),
            np.array(["2026-01-02"], dtype="datetime64[D]"),
        ],
    )
    assert other.figure.to_svg() == expected


@pytest.mark.parametrize("length", [0.0, -0.1, 1.01, np.nan, np.inf])
def test_events_length_errors_are_atomic(length: float) -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.category()).events(["A"], [[1]])
    before = ax.figure.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        ax.events(["B"], [[2]], length=length)
    info = details(caught.value)
    assert info is not None and info[0] == "length" and info[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("rows", "positions", "parameter"),
    [
        (["A"], [], "positions"),
        (["A", "A"], [list[float](), list[float]()], "rows"),
        (["A"], [[np.inf]], "positions"),
    ],
)
def test_events_data_errors_are_atomic(
    rows: list[str], positions: list[list[float]], parameter: str
) -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.category()).events(["C"], [[3]])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.events(rows, positions)
    info = details(caught.value)
    assert info is not None and info[0] == parameter and info[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("x", "y"),
    [
        (ub.Scale.linear(), ub.Scale.linear()),
        (ub.Scale.category(), ub.Scale.linear()),
        (ub.Scale.category(), ub.Scale.category()),
    ],
)
def test_events_require_continuous_x_and_category_y(x: ub.Scale[Any], y: ub.Scale[Any]) -> None:
    ax = ub.Figure().add_axes(x=x, y=y)
    with pytest.raises(ub.UsageError):
        cast(Callable[..., object], ax.events)(["A"], [[1]])


def test_events_copy_data_and_skip_nan() -> None:
    values = np.array([1.0, np.nan, 2.0])
    ax = ub.Figure().add_axes(y=ub.Scale.category()).events(["A"], [values])
    expected = ax.figure.to_svg()
    values[:] = 100
    assert ax.figure.to_svg() == expected
    other = ub.Figure().add_axes(y=ub.Scale.category()).events(["A"], [[1, 2]])
    assert other.figure.to_svg() == expected


@pytest.mark.parametrize("positions", ["bad", [None], [["bad"]], [np.ones((2, 2))]])
def test_events_nested_errors_highlight_positions(positions: object) -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.category())
    with pytest.raises((TypeError, ub.DataError)) as caught:
        cast(Callable[..., object], ax.events)(["A"], positions)
    info = details(caught.value)
    assert info is not None and info[0] == "positions" and info[1]
    assert "groups" not in str(caught.value)


def test_events_time_conversion_errors_highlight_positions() -> None:
    from datetime import UTC, datetime

    ax = ub.Figure().add_axes(x=ub.Scale.time(), y=ub.Scale.category())
    with pytest.raises(ub.DataError) as caught:
        ax.events(["A"], [[datetime(2026, 1, 1, tzinfo=UTC)]])
    info = details(caught.value)
    assert info is not None and info[0] == "positions" and info[1]


@pytest.mark.parametrize("value", [0.0, -1.0, 1e-310])
def test_events_log_validation_is_atomic(value: float) -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.category())
    ax.events(["A"], [[1, 10]])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.events(["B"], [[value]])
    info = details(caught.value)
    assert info is not None and info[0] == "positions"
    assert ax.figure.to_svg() == before
