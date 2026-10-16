"""Time axes with polars data: datetime columns of each unit, date columns, nulls as gaps, and
a time zone refused. Left out where polars is not installed (the free-threaded CI job)."""

import datetime as dt

import numpy as np
import polars as pl
import pytest

import umber as ub
from time_cases import (
    EXPECTED,
    NAT,
    TIMES,
    TZ_HINT,
    call,
    draws_like_a_list,
    parts,
    time_axes,
    us,
)
from umber import _time


@pytest.mark.parametrize(
    "dtype", [pl.Datetime("ms"), pl.Datetime("us"), pl.Datetime("ns")], ids=["ms", "us", "ns"]
)
def test_polars_datetimes_round_trip(dtype: pl.Datetime) -> None:
    values = pl.Series(TIMES, dtype=dtype)
    converted = _time.micros("Axes.line()", "x", values)
    assert converted.dtype == np.int64
    np.testing.assert_array_equal(converted, EXPECTED)
    draws_like_a_list(values)


def test_polars_dates_are_midnight() -> None:
    dates = pl.Series([dt.date(2026, 3, 3)], dtype=pl.Date)
    midnight = us(dt.datetime(2026, 3, 3))
    np.testing.assert_array_equal(_time.micros("Axes.line()", "x", dates), [midnight])


def test_polars_null_leaves_gap() -> None:
    values = pl.Series(
        [dt.datetime(2026, 1, 1), None, dt.datetime(2026, 1, 3)], dtype=pl.Datetime("us")
    )
    first = us(dt.datetime(2026, 1, 1))
    np.testing.assert_array_equal(
        _time.micros("Axes.line()", "x", values), [first, NAT, first + 2 * 86400 * 10**6]
    )
    drawn = call(time_axes(), "line")(values, [1.0, 2.0, 3.0])
    assert isinstance(drawn, ub.Axes)


def test_polars_time_zone_rejected() -> None:
    values = pl.Series(TIMES, dtype=pl.Datetime("us")).dt.replace_time_zone("Europe/Berlin")
    ax = time_axes().line(list(TIMES), [1.0, 2.0, 3.0])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        call(ax, "line")(values, [1.0] * 3)
    assert parts(caught.value) == (
        "Axes.line(): x has time zone Europe/Berlin; time values must not carry a time zone.",
        "x",
        TZ_HINT,
    )
    assert ax.figure.to_svg() == before
