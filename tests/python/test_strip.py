"""Strip determinism, shared band-axis checks and atomic validation."""

from collections.abc import Callable
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pytest

import umber as ub
from umber._errors import details


def test_strip_is_deterministic() -> None:
    outputs: set[str] = set()
    for _ in range(3):
        ax = ub.Figure(5, 3).add_axes(x=ub.Scale.category())
        assert ax.strip(["A", "B"], [[1, 2, 2, 3], [4, 5]], label="Samples") is ax
        outputs.add(ax.figure.to_svg())
        assert ax.figure.to_svg() == ax.figure.to_svg()
    assert len(outputs) == 1


@pytest.mark.parametrize("method", ["strip", "swarm"])
@pytest.mark.parametrize("horizontal", [False, True])
def test_points_support_both_band_orientations(method: str, horizontal: bool) -> None:
    fig = ub.Figure(5, 3)
    ax = fig.add_axes(y=ub.Scale.category()) if horizontal else fig.add_axes(x=ub.Scale.category())
    getattr(ax, method)(["B", "A"], [[1, np.nan, 2], []], color=ub.Color.RED, label="Samples")
    svg = fig.to_svg(text=ub.SvgText.EDITABLE)
    assert ">B</text>" in svg and ">A</text>" in svg and ">Samples</text>" in svg
    assert "#D64933" in svg
    assert fig.to_png(dpi=60)


@pytest.mark.parametrize("jitter", [-0.1, 0.51, np.nan, np.inf])
def test_strip_jitter_errors_are_atomic(jitter: float) -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category()).strip(["A"], [[1]])
    before = ax.figure.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        ax.strip(["B"], [[2]], jitter=jitter)
    info = details(caught.value)
    assert info is not None and info[0] == "jitter" and info[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("method", ["strip", "swarm"])
@pytest.mark.parametrize(
    ("categories", "groups", "parameter"),
    [
        (["A"], [], "groups"),
        (["A", "A"], [list[float](), list[float]()], "categories"),
        (["A"], [[np.inf]], "groups"),
    ],
)
def test_points_data_errors_are_atomic(
    method: str, categories: list[str], groups: list[list[float]], parameter: str
) -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category()).box(["C"], [[3, 4]])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        getattr(ax, method)(categories, groups)
    info = details(caught.value)
    assert info is not None and info[0] == parameter
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("method", ["strip", "swarm"])
@pytest.mark.parametrize(
    ("x", "y"),
    [
        (ub.Scale.linear(), ub.Scale.linear()),
        (ub.Scale.time(), ub.Scale.category()),
        (ub.Scale.category(), ub.Scale.time()),
        (ub.Scale.category(), ub.Scale.category()),
    ],
)
def test_points_reuse_band_axis_restrictions(
    method: str, x: ub.Scale[Any], y: ub.Scale[Any]
) -> None:
    ax = ub.Figure().add_axes(x=x, y=y)
    with pytest.raises(ub.UsageError):
        getattr(ax, method)(["A"], [[1]])


class RecordsConversion:
    def __init__(self) -> None:
        self.converted = False

    def __array__(self, dtype: object = None, copy: object = None) -> npt.NDArray[np.float64]:
        self.converted = True
        return np.array([1.0])


@pytest.mark.parametrize("method", ["strip", "swarm", "events"])
def test_type_checks_precede_conversion(method: str) -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.category())
    data = RecordsConversion()
    with pytest.raises(TypeError):
        cast(Callable[..., object], getattr(ax, method))(["A"], [data], color="red")
    assert not data.converted
    with pytest.raises(TypeError):
        cast(Callable[..., object], getattr(ax, method))(["A", "B"], [data, None])
    assert not data.converted


@pytest.mark.parametrize("method", ["strip", "swarm"])
@pytest.mark.parametrize("horizontal", [False, True])
def test_points_copy_data_on_log_axes(method: str, horizontal: bool) -> None:
    fig = ub.Figure(5, 3)
    ax = (
        fig.add_axes(x=ub.Scale.log(), y=ub.Scale.category())
        if horizontal
        else fig.add_axes(x=ub.Scale.category(), y=ub.Scale.log())
    )
    values = np.array([1.0, np.nan, 10.0, 100.0])
    getattr(ax, method)(["A"], [values])
    before = fig.to_svg()
    values[:] = 999
    assert fig.to_svg() == before
    with pytest.raises(ub.DataError) as caught:
        getattr(ax, method)(["B"], [[0]])
    info = details(caught.value)
    assert info is not None and info[0] == "groups" and info[1]
    assert fig.to_svg() == before


@pytest.mark.parametrize("method", ["strip", "swarm", "events"])
def test_empty_groups_and_missing_data_render(method: str) -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.category())
    getattr(ax, method)(["Empty", "Missing"], [[], [np.nan]])
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert ">Empty</text>" in svg and ">Missing</text>" in svg
    assert ax.figure.to_png(dpi=60)
