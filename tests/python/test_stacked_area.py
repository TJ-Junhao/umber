"""Stacked area conversion, native parity, errors and argument highlights."""

import copy
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber import _umber
from umber._errors import details

NAN = float("nan")
MIXED_HINT = "use stacked_bar for values with mixed signs."


def _axes() -> ub.Axes[float, float]:
    return ub.Figure(3, 2).add_axes()


def _call(ax: ub.Axes[Any, Any]) -> Callable[..., object]:
    return cast(Callable[..., object], ax.stacked_area)


def test_returns_axes_and_accepts_supported_containers() -> None:
    outputs: set[str] = set()
    for x in ([0, 1, 2], (0.0, 1.0, 2.0), range(3), np.arange(3.0), pd.Series([0, 1, 2])):
        ax = _axes()
        series = (ub.Series("Hydro", [3, 4, 3]), ub.Series("Wind", np.array([1, 2, 4])))
        assert _call(ax)(x, series) is ax
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1
    assert ub.Axes.stacked_area.__module__ == "umber"
    assert not hasattr(ub, "stacked_area")


@pytest.mark.parametrize("theme", [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT])
def test_layers_fill_flat_and_label_each_end(theme: ub.Theme) -> None:
    fig = ub.Figure(theme=theme)
    fig.add_axes().stacked_area(
        [0, 1, 2], [ub.Series("Hydro", [3, 4, 3]), ub.Series("Wind", [1, 2, 4])]
    )
    svg = fig.to_svg(text=ub.SvgText.EDITABLE)
    assert "linearGradient" not in svg
    assert svg.count("fill-opacity") >= 2
    assert ">Hydro</text>" in svg
    assert ">Wind</text>" in svg
    assert len(fig.to_png()) > 100


def test_nan_in_any_series_breaks_every_layer() -> None:
    outputs: set[str] = set()
    cases: list[tuple[list[float], list[float]]] = [
        ([1, 1, NAN, 1, 1], [2] * 5),
        ([1] * 5, [2, 2, NAN, 2, 2]),
    ]
    for a, b in cases:
        ax = _axes().stacked_area(range(5), [ub.Series("a", a), ub.Series("b", b)])
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


def test_data_and_copies_are_independent() -> None:
    x = np.arange(3.0)
    values = np.array([1.0, 2.0, 3.0])
    fig = ub.Figure()
    fig.add_axes().stacked_area(x, [ub.Series("a", values)])
    before = fig.to_svg()
    x[:] = 8
    values[:] = 12
    assert fig.to_svg() == before
    cloned = copy.copy(fig)
    cloned.axes.stacked_area([0, 1, 2], [ub.Series("b", [3, 2, 1])])
    assert cloned.to_svg() != before
    assert fig.to_svg() == before


@pytest.mark.parametrize(
    ("x", "values", "message", "parameter", "hint"),
    [
        ([0], [], "series is empty; pass at least one Series.", "series", None),
        ([0, 1], [[1, 2], [3]], "series[1].values has 1 values; x has 2.", "series", None),
        (
            [0, 1],
            [[1, 2], [3, -4]],
            "series[1].values[1] is -4; stacked areas need values of at least 0.",
            "series",
            MIXED_HINT,
        ),
        (
            [0, 2, 1],
            [[1, 2, 3]],
            "x[2] is 1, below x[1] = 2; stacked_area needs x in non-decreasing order.",
            "x",
            "sort the points by x first.",
        ),
        (
            [0, float("inf")],
            [[1, 2]],
            "x[1] is inf; values must be finite.",
            "x",
            "use NaN to leave a gap in the area.",
        ),
        (
            [0],
            [[float("inf")]],
            "series[0].values[0] is inf; values must be finite.",
            "series",
            "use NaN to leave a gap in the area.",
        ),
        (
            [0],
            [[1e300], [1e300]],
            "the values at x[0] stack to 2e300; each stack must not exceed 1e300.",
            "series",
            None,
        ),
    ],
)
def test_data_errors_are_exact_and_atomic(
    x: list[float], values: list[list[float]], message: str, parameter: str, hint: str | None
) -> None:
    ax = _axes().stacked_area([0, 1], [ub.Series("kept", [1, 2])])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.stacked_area(x, [ub.Series("s", v) for v in values])
    assert str(caught.value) == f"Axes.stacked_area(): {message}"
    assert details(caught.value) == (parameter, hint)
    assert ax.figure.to_svg() == before


def test_category_axis_is_a_usage_error() -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category())
    with pytest.raises(ub.UsageError) as caught:
        _call(ax)([0, 1], [ub.Series("s", [1, 2])])
    assert str(caught.value) == (
        "Axes.stacked_area(): this is a category axis; "
        "stacked areas apply to linear and log axes only."
    )
    assert details(caught.value) == (
        "x",
        "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().",
    )


def test_log_axes_validate_values_and_render_positive_series() -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.log())
    ax.stacked_area([1, 10, 100], [ub.Series("a", [1, 10, 100]), ub.Series("b", [9, 90, 900])])
    assert "linearGradient" not in ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.stacked_area([1], [ub.Series("a", [0])])
    assert str(caught.value) == (
        "Axes.stacked_area(): series[0].values[0] is 0 on a log axis; values must be positive."
    )
    with pytest.raises(ub.DataError) as caught:
        ax.stacked_area([1], [ub.Series("a", [-1])])
    assert details(caught.value) == ("series", MIXED_HINT)


def test_types_precede_conversion_and_native_validation() -> None:
    ax = _axes()
    with pytest.raises(TypeError) as caught:
        _call(ax)("x", "series")
    assert str(caught.value) == "Axes.stacked_area(): x is str; x must be a sequence or an array."
    with pytest.raises(TypeError) as caught:
        _call(ax)([None], "series")
    assert str(caught.value) == (
        "Axes.stacked_area(): series is str; series must be a sequence of Series."
    )
    with pytest.raises(TypeError) as caught:
        _call(ax)([None], [ub.Series("a", [1]), 2])
    assert str(caught.value) == (
        "Axes.stacked_area(): series[1] is int; series[1] must be of type Series."
    )
    with pytest.raises(ub.DataError) as data_error:
        _call(ax)(np.ones((2, 2)), [ub.Series("a", [1])])
    found = details(data_error.value)
    assert found is not None
    assert found[0] == "x"


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ('ax.stacked_area([0, 1], [ub.Series("a", [1, -2])])', '[ub.Series("a", [1, -2])]'),
        ('ax.stacked_area([1, 0], [ub.Series("a", [1, 2])])', "[1, 0]"),
        ('ax.stacked_area(x=[0, 1], series=[ub.Series("a", [1])])', 'series=[ub.Series("a", [1])]'),
    ],
)
def test_error_highlights_the_right_argument(tmp_path: Path, call: str, argument: str) -> None:
    path = tmp_path / "user_code.py"
    path.write_text(f"import umber as ub\nax = ub.Figure().add_axes()\n{call}\n")
    with pytest.raises(ub.DataError) as caught:
        exec(compile(path.read_text(), str(path), "exec"), {})
    notes = list(getattr(caught.value, "__notes__", []))
    expected = f"  {call}\n{' ' * (2 + call.index(argument))}{'^' * len(argument)}"
    assert expected in notes


@pytest.mark.parametrize(
    ("theme", "native_theme"),
    [
        (ub.Theme.PIGMENT, _umber.Theme.pigment),
        (ub.Theme.GRAPHITE, _umber.Theme.graphite),
        (ub.Theme.MIDNIGHT, _umber.Theme.midnight),
    ],
)
def test_facade_matches_native(theme: ub.Theme, native_theme: Callable[[], _umber.Theme]) -> None:
    x = np.arange(3.0)
    a, b = np.array([1.0, 2.0, 3.0]), np.array([3.0, 1.0, 2.0])
    fig = ub.Figure(6, 4, theme=theme)
    fig.add_axes().stacked_area(x, [ub.Series("a", a), ub.Series("b", b)])
    native = _umber.Figure(6, 4, native_theme(), 1, 1, 0, 0, 0)
    native.add_axes(0, 0, 0, 0, 1, 1, 0, 10.0, 0, 10.0)
    native.stacked_area(0, 0, x, [("a", a, (0, 0, 1.0)), ("b", b, (0, 0, 1.0))])
    assert fig.to_svg() == native.to_svg(0)
