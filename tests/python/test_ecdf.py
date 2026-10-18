"""``Axes.ecdf()``: the empirical cumulative distribution as a step line from 0 to 1."""

import datetime as dt
from collections.abc import Callable
from typing import Any, cast

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

type Numeric = ub.Axes[float, float]

VALUES = [3.0, 1.0, 2.0, 2.0, float("nan"), 5.5, 1.0, 4.0]
ECDF_NAN = "an ECDF ignores NaN, so use it to leave a value out."
LINEAR_Y_HINT = "pass y=Scale.linear() to Figure.add_axes()."
X_HINT = "use a linear, log or time x axis."


def _svg(ax: ub.Axes[Any, Any]) -> str:
    return ax.figure.to_svg(text=ub.SvgText.EDITABLE)


def _axes(x: ub.Scale[float] | None = None, y: ub.Scale[float] | None = None) -> Numeric:
    return ub.Figure(4, 3).add_axes(x=x or ub.Scale.linear(), y=y or ub.Scale.linear())


def _reference(values: list[Any], *, complementary: bool) -> tuple[list[Any], list[float]]:
    """The ECDF as the points of a post step line: from 0 (or 1) up to 1 (or down to 0)."""
    kept = [v for v in values if v == v]
    distinct = sorted(set(kept))
    n = len(kept)
    below = [sum(1 for v in kept if v <= x) for x in distinct]
    if complementary:
        return [distinct[0], *distinct], [1.0, *[(n - c) / n for c in below]]
    return [distinct[0], *distinct], [0.0, *[c / n for c in below]]


@pytest.mark.parametrize("complementary", [False, True])
def test_ecdf_matches_reference(complementary: bool) -> None:
    x, y = _reference(VALUES, complementary=complementary)
    ecdf = _axes().ylim(0, 1).ecdf(VALUES, complementary=complementary, label="Load")
    step = _axes().ylim(0, 1).step(x, y, where=ub.Step.POST, label="Load")
    assert _svg(ecdf) == _svg(step)


def test_ecdf_y_axis_runs_from_zero_to_one() -> None:
    for complementary in (False, True):
        auto = _axes().ecdf(VALUES, complementary=complementary)
        fixed = _axes().ylim(0, 1).ecdf(VALUES, complementary=complementary)
        assert _svg(auto) == _svg(fixed)


def test_ecdf_on_time_axis() -> None:
    days = [
        dt.datetime(2026, 1, 3),
        dt.datetime(2026, 1, 1),
        dt.datetime(2026, 1, 3),
        dt.datetime(2026, 1, 9, 12),
    ]
    x, y = _reference(days, complementary=False)
    timed = ub.Figure(4, 3).add_axes(x=ub.Scale.time())
    ecdf = timed.ylim(0, 1).ecdf(days)
    step = ub.Figure(4, 3).add_axes(x=ub.Scale.time()).ylim(0, 1).step(x, y)
    assert _svg(ecdf) == _svg(step)
    as_array = ub.Figure(4, 3).add_axes(x=ub.Scale.time()).ylim(0, 1)
    assert _svg(as_array.ecdf(np.array(days, dtype="datetime64[us]"))) == _svg(ecdf)
    with_gap = ub.Figure(4, 3).add_axes(x=ub.Scale.time()).ylim(0, 1)
    with_gap.ecdf([*days, np.datetime64("NaT", "us")])
    assert _svg(with_gap) == _svg(ecdf)


def test_ecdf_on_a_log_axis_steps_at_each_value() -> None:
    values = [1.0, 10.0, 10.0, 1000.0]
    x, y = _reference(values, complementary=False)
    ecdf = _axes(x=ub.Scale.log()).ylim(0, 1).ecdf(values)
    step = _axes(x=ub.Scale.log()).ylim(0, 1).step(x, y)
    assert _svg(ecdf) == _svg(step)


def test_ecdf_accepts_lists_arrays_and_series() -> None:
    drawn = {
        _svg(_axes().ecdf(values))
        for values in (VALUES, tuple(VALUES), np.array(VALUES), pd.Series(VALUES))
    }
    assert len(drawn) == 1


def test_ecdf_takes_the_line_options_and_a_legend_entry() -> None:
    plain = _svg(_axes().ecdf(VALUES))
    styled = _axes().ecdf(
        VALUES, label="Latency", color=ub.Color.RED, dash=ub.Dash.DASHED, width=ub.Size.pt(3)
    )
    drawn = _svg(styled)
    assert drawn != plain
    assert 'stroke="#D64933"' in drawn
    assert ">Latency</text>" in drawn
    auto = _svg(_axes().ecdf(VALUES, width=ub.Size.AUTO, dash=ub.Dash.SOLID))
    assert auto == plain


def test_a_direct_label_sits_at_the_end_of_the_ecdf() -> None:
    x, y = _reference(VALUES, complementary=False)
    ecdf = _axes().ylim(0, 1).legend(ub.LegendStyle.DIRECT).ecdf(VALUES, label="Load")
    step = _axes().ylim(0, 1).legend(ub.LegendStyle.DIRECT).step(x, y, label="Load")
    assert _svg(ecdf) == _svg(step)


# (id, axes, call, error class, message, parameter, hint)
ERRORS: list[tuple[str, Callable[[], Any], Callable[[Any], object], type, str, Any, Any]] = [
    (
        "no finite value",
        _axes,
        lambda ax: ax.ecdf([float("nan")]),
        ub.DataError,
        "Axes.ecdf(): values has no finite value; an ECDF needs at least one.",
        "values",
        "pass at least one value that is not NaN.",
    ),
    (
        "empty",
        _axes,
        lambda ax: ax.ecdf([]),
        ub.DataError,
        "Axes.ecdf(): values has no finite value; an ECDF needs at least one.",
        "values",
        "pass at least one value that is not NaN.",
    ),
    (
        "infinite",
        _axes,
        lambda ax: ax.ecdf([1.0, float("inf")]),
        ub.DataError,
        "Axes.ecdf(): values[1] is inf; values must be finite.",
        "values",
        ECDF_NAN,
    ),
    (
        "not positive on a log axis",
        lambda: _axes(x=ub.Scale.log()),
        lambda ax: ax.ecdf([1.0, 0.0]),
        ub.DataError,
        "Axes.ecdf(): values[1] is 0 on a log axis; values must be positive.",
        "values",
        "use a linear axis, or remove values that are not positive.",
    ),
    (
        "log y",
        lambda: _axes(y=ub.Scale.log()),
        lambda ax: ax.ecdf(VALUES),
        ub.UsageError,
        "Axes.ecdf(): the y axis is a log axis; an ECDF runs from 0 to 1 and needs a linear y "
        "axis.",
        "y",
        LINEAR_Y_HINT,
    ),
    (
        "category y",
        lambda: ub.Figure().add_axes(y=ub.Scale.category()),
        lambda ax: ax.ecdf(VALUES),
        ub.UsageError,
        "Axes.ecdf(): y is a category scale; Axes.ecdf() needs a linear y axis.",
        "y",
        LINEAR_Y_HINT,
    ),
    (
        "time y",
        lambda: ub.Figure().add_axes(y=ub.Scale.time()),
        lambda ax: ax.ecdf(VALUES),
        ub.UsageError,
        "Axes.ecdf(): y is a time scale; Axes.ecdf() needs a linear y axis.",
        "y",
        LINEAR_Y_HINT,
    ),
    (
        "category x",
        lambda: ub.Figure().add_axes(x=ub.Scale.category()),
        lambda ax: ax.ecdf(VALUES),
        ub.DataError,
        "Axes.ecdf(): values holds numbers but the x axis is a category axis; "
        "an ECDF needs a linear, log or time x axis.",
        None,
        X_HINT,
    ),
    (
        "label type",
        _axes,
        lambda ax: ax.ecdf(VALUES, label=cast(str, 3)),
        TypeError,
        "Axes.ecdf(): label is int; label must be a str.",
        "label",
        "pass a str; convert other values with str().",
    ),
    (
        "complementary type",
        _axes,
        lambda ax: ax.ecdf(VALUES, complementary=cast(bool, 1)),
        TypeError,
        "Axes.ecdf(): complementary is int; complementary must be a bool.",
        "complementary",
        "pass True or False.",
    ),
    (
        "times on a linear axis",
        _axes,
        lambda ax: ax.ecdf([dt.datetime(2026, 1, 1)]),
        ub.DataError,
        "Axes.ecdf(): values[0] is datetime; values must be numbers.",
        "values",
        "use numbers, or declare the axis with Scale.time().",
    ),
]


@pytest.mark.parametrize(
    ("make", "call", "cls", "message", "parameter", "hint"),
    [case[1:] for case in ERRORS],
    ids=[case[0] for case in ERRORS],
)
def test_ecdf_errors_leave_the_figure_unchanged(
    make: Callable[[], Any],
    call: Callable[[Any], object],
    cls: type[BaseException],
    message: str,
    parameter: object,
    hint: object,
) -> None:
    ax = make().title("Kept")
    before = _svg(ax)
    with pytest.raises(cls) as caught:
        call(ax)
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert _svg(ax) == before
