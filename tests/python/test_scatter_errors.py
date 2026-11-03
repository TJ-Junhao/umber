"""Error bars and a connecting line declared on a scatter: x_error, y_error and connect."""

import math
from collections.abc import Callable
from typing import Any, cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details

NAN = math.nan
# A scatter's own bars and line are drawn at half the automatic line width.
HALF = ub.Size.times(0.5)


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def svg(ax: ub.Axes[Any, Any]) -> str:
    return ax.figure.to_svg(text=ub.SvgText.OUTLINES)


def test_own_bars_and_line_draw_as_standalone_marks_in_the_series_color() -> None:
    folds = ["Fold 1", "Fold 2", "Fold 3", "Mean"]
    values = [66.8, 68.1, 65.4, 66.77]
    low, high = [NAN, NAN, NAN, 65.6], [NAN, NAN, NAN, 67.9]
    own = ub.Figure().add_axes(x=ub.Scale.category())
    own = own.scatter(folds, values, label="80 epochs", y_error=(low, high))
    alone = ub.Figure().add_axes(x=ub.Scale.category())
    color = ub.Color.palette(0)
    alone.y_error_bars(folds, low, high, color=color, width=HALF)
    alone.scatter(folds, values, label="80 epochs")
    assert svg(own) == svg(alone)
    # With the line too: the scatter's own line widens the clip of the marks as a line does.
    own = ub.Figure().add_axes(x=ub.Scale.category())
    own = own.scatter(folds, values, label="80 epochs", y_error=(low, high), connect=True)
    alone = ub.Figure().add_axes(x=ub.Scale.category())
    alone.line(folds, values, color=color, width=HALF)
    alone.y_error_bars(folds, low, high, color=color, width=HALF)
    alone.scatter(folds, values, label="80 epochs")
    assert svg(own) == svg(alone)


def test_both_errors_at_once_and_the_following_series_keeps_its_color() -> None:
    own = ub.Figure().add_axes()
    own = own.scatter(
        [1.0, 2.0], [3.0, 4.0], x_error=([0.8, 1.8], [1.2, 2.2]), y_error=([2.5, 3.5], [3.5, 4.5])
    ).scatter([1.0, 2.0], [1.0, 2.0])
    alone = ub.Figure().add_axes()
    color = ub.Color.palette(0)
    alone.x_error_bars([3.0, 4.0], [0.8, 1.8], [1.2, 2.2], color=color, width=HALF)
    alone.y_error_bars([1.0, 2.0], [2.5, 3.5], [3.5, 4.5], color=color, width=HALF)
    alone.scatter([1.0, 2.0], [3.0, 4.0]).scatter([1.0, 2.0], [1.0, 2.0])
    assert svg(own) == svg(alone)


def test_groups_join_and_bar_only_their_own_points() -> None:
    x = [1.0, 2.0, 1.0, 2.0, 3.0]
    y = [1.0, 2.0, 5.0, 6.0, 5.5]
    low = [0.5, 1.5, 4.5, NAN, 5.0]
    high = [1.5, 2.5, 5.5, NAN, 6.0]
    by = ["A", "A", "B", "B", "B"]
    own = ub.Figure().add_axes().scatter(x, y, by=by, y_error=(low, high), connect=True)
    alone = ub.Figure().add_axes()
    for name, rows in (("A", [0, 1]), ("B", [2, 3, 4])):
        color = ub.Color.palette(0 if name == "A" else 1)
        xs, ys = [x[i] for i in rows], [y[i] for i in rows]
        alone.line(xs, ys, color=color, width=HALF)
        alone.y_error_bars(
            xs, [low[i] for i in rows], [high[i] for i in rows], color=color, width=HALF
        )
        alone.scatter(xs, ys, label=name)
    assert svg(own) == svg(alone)


def test_error_arguments_must_be_pairs_of_columns() -> None:
    ax = ub.Figure().add_axes()
    with pytest.raises(TypeError, match=r"y_error is list; y_error must be a tuple"):
        call(ax.scatter)([1.0], [2.0], y_error=[[1.0], [3.0]])
    with pytest.raises(TypeError, match=r"x_error has 3 elements"):
        call(ax.scatter)([1.0], [2.0], x_error=([0.5], [1.5], [2.0]))
    with pytest.raises(TypeError):
        call(ax.scatter)([1.0], [2.0], y_error=(1.0, 3.0))
    with pytest.raises(TypeError, match="connect"):
        call(ax.scatter)([1.0], [2.0], connect=1)


def test_an_error_along_a_category_axis_is_a_usage_error() -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.category())
    with pytest.raises(ub.UsageError) as raised:
        call(ax.scatter)([70.0], ["Base"], y_error=([1.0], [2.0]))
    assert details(raised.value) is not None
    assert "y_error needs a linear, log or time y axis" in str(raised.value)
    assert ax.figure.to_svg()  # The failed call added nothing.


def test_bad_ends_name_the_argument_and_the_index() -> None:
    ax = ub.Figure().add_axes()
    with pytest.raises(ub.DataError, match=r"y_error\[1\] has low 4 above high 3"):
        ax.scatter([1.0, 2.0], [2.0, 3.0], y_error=([1.0, 4.0], [2.0, 3.0]))
    with pytest.raises(ub.DataError, match=r"y_error low has 1 values but the scatter has 2"):
        ax.scatter([1.0, 2.0], [2.0, 3.0], y_error=([1.0], [2.0, 3.0]))
    with pytest.raises(ub.DataError, match=r"x_error high\[0\] is inf"):
        ax.scatter([1.0, 2.0], [2.0, 3.0], x_error=([0.5, 1.5], [math.inf, 2.5]))
    log = ub.Figure().add_axes(y=ub.Scale.log())
    with pytest.raises(ub.DataError, match=r"y_error low\[0\] is 0 on a log axis"):
        log.scatter([1.0], [2.0], y_error=([0.0], [3.0]))
    # In a group, the index is the row of the input.
    with pytest.raises(ub.DataError, match=r"in group 'B': y_error\[2\]"):
        ax.scatter(
            [1.0, 2.0, 3.0],
            [1.0, 2.0, 3.0],
            by=["A", "B", "B"],
            y_error=([0.5, 1.5, 4.0], [1.5, 2.5, 3.5]),
        )


def test_a_time_axis_takes_times_for_its_ends() -> None:
    days = np.array(["2026-03-03", "2026-03-10"], dtype="datetime64[D]")
    low = np.array(["2026-03-01", "2026-03-08"], dtype="datetime64[D]")
    high = np.array(["2026-03-05", "2026-03-20"], dtype="datetime64[D]")
    ax = ub.Figure().add_axes(y=ub.Scale.time())
    ax = ax.scatter([1.0, 2.0], days, y_error=(low, high), connect=True)
    assert ax.figure.to_png(dpi=72).startswith(b"\x89PNG")


def test_the_quick_scatter_passes_them_on() -> None:
    quick = ub.scatter([1.0, 2.0], [2.0, 3.0], y_error=([1.5, 2.5], [2.5, 3.5]), connect=True)
    plain = ub.Figure().add_axes()
    plain = plain.scatter([1.0, 2.0], [2.0, 3.0], y_error=([1.5, 2.5], [2.5, 3.5]), connect=True)
    assert svg(quick) == svg(plain)
