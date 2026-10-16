"""Shared values and helpers of the time axis tests, ``test_time.py`` and
``test_time_polars.py``, which runs only where polars is installed."""

import datetime as dt
from collections.abc import Callable
from typing import Any, cast

import numpy as np

import umber as ub
from umber._errors import details

EPOCH = dt.datetime(1970, 1, 1)
NAT = np.iinfo(np.int64).min
TZ_HINT = "convert first with .tz_localize(None) or .tz_convert(...)."

# Whole seconds, so that every unit from seconds down holds them exactly.
TIMES = [
    dt.datetime(2026, 3, 3, 6, 0, 0),
    dt.datetime(2026, 3, 3, 6, 0, 1),
    dt.datetime(1999, 12, 31, 23, 59, 59),
]

type AnyAxes = ub.Axes[Any, Any]


def us(moment: dt.datetime) -> int:
    """The microseconds from 1970-01-01T00:00 to the naive ``moment``."""
    return (moment - EPOCH) // dt.timedelta(microseconds=1)


EXPECTED = np.array([us(t) for t in TIMES], dtype=np.int64)


def parts(err: BaseException) -> tuple[str, object, object]:
    """The message, parameter and hint of an umber error."""
    found = details(err)
    assert found is not None
    return str(err), found[0], found[1]


def call(ax: AnyAxes, name: str) -> Callable[..., object]:
    """The method ``name`` of ``ax`` with arguments of any type, for calls that must be refused."""
    return cast(Callable[..., object], getattr(ax, name))


def time_axes(theme: ub.Theme = ub.Theme.PIGMENT) -> ub.Axes[ub.Time, float]:
    """New axes with a time x axis on a small figure."""
    return ub.Figure(5, 3, theme=theme).add_axes(x=ub.Scale.time())


def draws_like_a_list(values: object) -> None:
    """Assert that a line through ``values``, which hold ``TIMES``, draws as the list does."""
    reference = time_axes().line(list(TIMES), [1.0, 3.0, 2.0]).figure.to_svg()
    drawn = cast(AnyAxes, call(time_axes(), "line")(values, [1.0, 3.0, 2.0]))
    assert drawn.figure.to_svg() == reference
