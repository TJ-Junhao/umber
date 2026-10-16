"""Numeric cells, binned counts, validation and color mapping."""

import datetime as dt
import math
import re
import xml.etree.ElementTree as ET
from collections.abc import Callable
from typing import Any, cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details


def axes() -> ub.Axes[float, float]:
    return ub.Figure(6, 4).add_axes()


def call(function: object) -> Callable[..., Any]:
    return cast(Callable[..., Any], function)


@pytest.mark.parametrize("bad", [True, "bad", None, np.inf, 1e301])
@pytest.mark.parametrize("array", [False, True])
def test_matrix_errors_name_row_and_column(bad: object, array: bool) -> None:
    values = [[1, 2], [3, bad]]
    matrix = np.asarray(values, dtype=object) if array else values
    with pytest.raises((TypeError, ub.DataError)) as caught:
        axes().mesh([0, 1, 2], [0, 1, 2], matrix)
    assert "values[1, 1]" in str(caught.value)
    found = details(caught.value)
    assert found is not None and found[0] == "values" and found[1]


@pytest.mark.parametrize("axis", ["x", "y", "both"])
def test_hist2d_log_bins_match_transformed_edges(axis: str) -> None:
    x = np.array([1.0, 4.0, 10.0, 20.0, 70.0, 100.0])
    y = x[::-1]
    xe = np.geomspace(1, 100, 4) if axis != "y" else np.linspace(1, 100, 4)
    ye = np.geomspace(1, 100, 4) if axis != "x" else np.linspace(1, 100, 4)
    counts, _, _ = np.histogram2d(x, y, bins=(xe, ye))
    values = counts.T.copy()
    values[values == 0] = np.nan
    xscale = ub.Scale.log() if axis != "y" else ub.Scale.linear()
    yscale = ub.Scale.log() if axis != "x" else ub.Scale.linear()
    actual = ub.Figure(6, 4).add_axes(x=xscale, y=yscale).hist2d(x, y, bins=3)
    expected = ub.Figure(6, 4).add_axes(x=xscale, y=yscale).mesh(xe, ye, values)
    assert actual.figure.to_svg() == expected.figure.to_svg()


@pytest.mark.parametrize(
    ("rows", "share_x", "share_y", "gridsize", "different"),
    [
        (1, ub.Share.NONE, ub.Share.NONE, 10, False),
        (1, ub.Share.ALL, ub.Share.NONE, 10, False),
        (1, ub.Share.ALL, ub.Share.NONE, 30, False),
        (2, ub.Share.ALL, ub.Share.NONE, 10, True),
        (2, ub.Share.ALL, ub.Share.ALL, 10, False),
    ],
)
def test_dense_hexbin_grid_geometry(
    rows: int, share_x: ub.Share, share_y: ub.Share, gridsize: int, different: bool
) -> None:
    i = np.arange(2000)
    x = (i * 0.6180339887 % 1) * 4 - 2
    y = (i * 0.4142135623 % 1) * 4 - 2 + 0.5 * x
    fig = ub.Figure(10, 6, rows=rows, cols=2, share_x=share_x, share_y=share_y)
    for row in range(rows):
        for col in range(2):
            offset = (2 * row + col) * 0.7 if different else 0.0
            fig.add_axes(row=row, col=col).hexbin(x + offset, y + offset, gridsize=gridsize)
    assert_svg_hexagons_regular_and_whole(fig.to_svg())


def assert_svg_hexagons_regular_and_whole(svg: str) -> None:
    count = 0
    root = ET.fromstring(svg)
    for group in root.iter("{http://www.w3.org/2000/svg}g"):
        if "clip-path" not in group.attrib:
            continue
        for path in group.findall("{http://www.w3.org/2000/svg}path"):
            data = path.attrib.get("d", "")
            if "C" in data or path.attrib.get("fill") in (None, "none"):
                continue
            points = [
                (float(x), float(y)) for x, y in re.findall(r"[ML]([-\d.e]+) ([-\d.e]+)", data)
            ]
            assert len(points) == 6, data
            sides = [math.dist(points[j], points[(j + 1) % 6]) for j in range(6)]
            assert min(sides) > 0
            assert max(sides) - min(sides) < 0.003, sides
            count += 1
    assert count >= 10


@pytest.mark.parametrize("shape", [(12, 2.5), (3, 10)])
def test_dense_hexbin_ylim_geometry(shape: tuple[float, float]) -> None:
    rng = np.random.default_rng(194)
    x, y = rng.normal(size=(2, 2000))
    ax = ub.Figure(*shape).add_axes().ylim(-10, 10)
    ax.hexbin(x, y, gridsize=30)
    assert_svg_hexagons_regular_and_whole(ax.figure.to_svg())


def test_dense_hexbin_scatter_geometry() -> None:
    rng = np.random.default_rng(194)
    x, y = rng.normal(size=(2, 2000))
    ax = ub.Figure(12, 2.5).add_axes().hexbin(x, y, gridsize=30)
    ax.scatter([-1, 1], [-1, 1])
    assert_svg_hexagons_regular_and_whole(ax.figure.to_svg())


def test_mesh_cells_span_edges() -> None:
    ax = axes()
    assert ax.mesh([0, 1, 4], [2, 3, 7], [[1, 2], [3, 4]]) is ax
    svg = ax.figure.to_svg()
    assert "linearGradient" in svg
    assert svg == ax.figure.to_svg()


def test_mesh_edges_set_domain() -> None:
    automatic = axes().mesh([-2, 1, 7], [3, 4, 9], [[1, 2], [3, 4]])
    explicit = axes().xlim(-2, 7).ylim(3, 9).mesh([-2, 1, 7], [3, 4, 9], [[1, 2], [3, 4]])
    assert automatic.figure.to_svg() == explicit.figure.to_svg()


@pytest.mark.parametrize("bins", [1, 3, (2, 5), (7, 2)])
def test_hist2d_counts_match_numpy(bins: int | tuple[int, int]) -> None:
    x = np.array([-3, -2, -1, 0, 1, 2, 3, 3, 0.5, np.nan, 100])
    y = np.array([4, 2, 1, 0, -1, -2, -4, -4, 0.5, 100, np.nan])
    valid = np.isfinite(x) & np.isfinite(y)
    counts, xe, ye = np.histogram2d(x[valid], y[valid], bins=bins)
    values = counts.T.copy()
    values[values == 0] = np.nan
    actual = axes().hist2d(x, y, bins=bins)
    expected = axes().mesh(xe, ye, values)
    assert actual.figure.to_svg() == expected.figure.to_svg()


def test_hist2d_log_skips_empty_cells() -> None:
    actual = axes().hist2d([0, 0, 2], [0, 0, 2], bins=2, scale=ub.Scale.log())
    expected = axes().mesh([0, 1, 2], [0, 1, 2], [[2, np.nan], [np.nan, 1]], scale=ub.Scale.log())
    assert actual.figure.to_svg() == expected.figure.to_svg()


@pytest.mark.parametrize("method", ["hist2d", "hexbin"])
def test_empty_counts_draw_no_colorbar(method: str) -> None:
    ax = axes()
    call(getattr(ax, method))([np.nan, 1], [1, np.nan], scale=ub.Scale.log())
    assert "linearGradient" not in ax.figure.to_svg()


def test_mesh_all_nan_draws_no_colorbar() -> None:
    ax = axes().mesh([0, 1], [0, 1], [[np.nan]])
    assert "linearGradient" not in ax.figure.to_svg()


@pytest.mark.parametrize(
    ("xe", "ye", "values", "message", "parameter"),
    [
        (
            [0, 1, 2, 3, 4],
            [0, 1],
            [[1, 2, 3, 4, 5]],
            "x_edges has 5 values; values has 5 columns, so x_edges needs 6.",
            "x_edges",
        ),
        (
            [0, 1],
            [0, 1],
            [[1], [2]],
            "y_edges has 2 values; values has 2 rows, so y_edges needs 3.",
            "y_edges",
        ),
        (
            [0, 1, 1],
            [0, 1],
            [[1, 2]],
            "x_edges[2] is 1; edges must be strictly increasing.",
            "x_edges",
        ),
    ],
)
def test_mesh_errors_are_atomic(
    xe: list[float], ye: list[float], values: object, message: str, parameter: str
) -> None:
    ax = axes().mesh([0, 1], [0, 1], [[1]])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.mesh(xe, ye, values)
    assert str(caught.value) == f"Axes.mesh(): {message}"
    found = details(caught.value)
    assert found is not None and found[0] == parameter and found[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("values", [[1, 2], [[[1]]], [[1], [2, 3]], [[True]], [["1"]]])
def test_mesh_rejects_invalid_matrices(values: object) -> None:
    with pytest.raises((ub.DataError, TypeError)):
        axes().mesh([0, 1], [0, 1], values)


@pytest.mark.parametrize("method, option", [("hist2d", "bins"), ("hexbin", "gridsize")])
@pytest.mark.parametrize("value", [0, -1, 10001])
def test_invalid_bin_counts(method: str, option: str, value: int) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        call(getattr(ax, method))([0, 1], [0, 1], **{option: value})
    found = details(caught.value)
    assert found is not None and found[0] == option and found[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("method", ["mesh", "hist2d", "hexbin"])
def test_category_axes_rejected(method: str) -> None:
    for ax in [
        ub.Figure().add_axes(x=ub.Scale.category()),
        ub.Figure().add_axes(y=ub.Scale.category()),
    ]:
        args = ([0, 1], [0, 1], [[1]]) if method == "mesh" else ([0, 1], [0, 1])
        with pytest.raises(ub.UsageError):
            call(getattr(ax, method))(*args)


@pytest.mark.parametrize("method", ["hist2d", "hexbin"])
@pytest.mark.parametrize("axis", ["x", "y"])
@pytest.mark.parametrize("time_values", [False, True])
def test_time_axes_rejected_before_cell_conversion(
    method: str, axis: str, time_values: bool
) -> None:
    ax = (
        ub.Figure().add_axes(x=ub.Scale.time())
        if axis == "x"
        else ub.Figure().add_axes(y=ub.Scale.time())
    )
    before = ax.figure.to_svg()
    times = [dt.datetime(2026, 1, 1), dt.datetime(2026, 1, 2)]
    x = times if time_values and axis == "x" else [0, 1]
    y = times if time_values and axis == "y" else [0, 1]
    with pytest.raises(ub.UsageError) as caught:
        call(getattr(ax, method))(x, y)
    assert str(caught.value) == f"Axes.{method}(): {axis} must be a numeric axis."
    assert details(caught.value) == (axis, "use Scale.linear() or Scale.log() for both axes.")
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("axis", ["x", "y"])
def test_mesh_category_axes_name_the_axis(axis: str) -> None:
    ax = (
        ub.Figure().add_axes(x=ub.Scale.category())
        if axis == "x"
        else ub.Figure().add_axes(y=ub.Scale.category())
    )
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        call(ax.mesh)([0, 1], [0, 1], [[1]])
    assert str(caught.value) == f"Axes.mesh(): {axis} must be a numeric or time axis."
    assert details(caught.value) == (
        axis,
        "use Scale.linear(), Scale.log() or Scale.time() for both axes.",
    )
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("method", ["mesh", "hist2d", "hexbin"])
@pytest.mark.parametrize("scale", [ub.Scale.category(), ub.Scale.time()])
def test_cell_color_scale_must_be_numeric(method: str, scale: ub.Scale[Any]) -> None:
    ax = axes().scatter([0, 1], [0, 1])
    before = ax.figure.to_svg()
    args = ([0, 1], [0, 1], [[1]]) if method == "mesh" else ([0, 1], [0, 1])
    with pytest.raises(TypeError) as caught:
        call(getattr(ax, method))(*args, scale=scale)
    assert str(caught.value) == (
        f"Axes.{method}(): scale is {scale!r}; scale must be Scale.linear() or Scale.log()."
    )
    assert details(caught.value) == ("scale", "use Scale.linear() or Scale.log().")
    assert ax.figure.to_svg() == before


def test_cell_color_conflicts_are_atomic() -> None:
    ax = axes().mesh([0, 1], [0, 1], [[1]], cmap=ub.Cmap.VIRIDIS)
    before = ax.figure.to_svg()
    for method in ["mesh", "hist2d", "hexbin"]:
        args = ([0, 1], [0, 1], [[1]]) if method == "mesh" else ([0, 1], [0, 1])
        with pytest.raises(ub.UsageError):
            call(getattr(ax, method))(*args, cmap=ub.Cmap.MAGMA)
        assert ax.figure.to_svg() == before


@pytest.mark.parametrize("method", ["hist2d", "hexbin"])
@pytest.mark.parametrize("points", [([], []), ([1], [2]), ([1, 1], [2, 2])])
def test_count_degenerate_ranges(method: str, points: tuple[list[float], list[float]]) -> None:
    ax = axes()
    call(getattr(ax, method))(*points)
    assert ax.figure.to_svg() == ax.figure.to_svg()


@pytest.mark.parametrize("bins", [True, 2.5, (2,), (2, 3, 4), (2, False), (2, 0), 2**100])
def test_hist2d_bad_bins_are_atomic(bins: object) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises((TypeError, ub.StyleError)) as caught:
        call(ax.hist2d)([0, 1], [0, 1], bins=bins)
    found = details(caught.value)
    assert found is not None and found[0] == "bins" and found[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("method", ["mesh", "hist2d", "hexbin"])
def test_cells_on_log_axes(method: str) -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.log())
    args = ([1, 10, 100], [1, 10], [[1, 2]]) if method == "mesh" else ([1, 10, 100], [1, 10, 100])
    call(getattr(ax, method))(*args)
    assert "linearGradient" in ax.figure.to_svg()


@pytest.mark.parametrize("share", [ub.Share.NONE, ub.Share.ALL])
def test_hexbin_shared_axes(share: ub.Share) -> None:
    fig = ub.Figure(12, 5, cols=2, share_x=share, share_y=share)
    fig.add_axes(col=0).hexbin([0, 1, 2], [0, 2, 1], gridsize=5)
    fig.add_axes(col=1).hexbin([-1, 1, 3], [-3, 4, 2], gridsize=7)
    assert fig.to_svg() == fig.to_svg()


def test_empty_counts_do_not_change_other_marks() -> None:
    ax = axes().scatter([0, 1], [0, 2])
    before = ax.figure.to_svg()
    ax.hist2d([], []).hexbin([], [])
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("edges", [[1, 1 + 1e-12], [0, 1e-300]])
def test_mesh_rejects_unresolvable_edge_range(edges: list[float]) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.mesh(edges, [0, 1], [[1]])
    found = details(caught.value)
    assert found is not None and found[0] == "x_edges" and found[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("count", [1, 2, 30, ub.MAX_BINS])
def test_hexbin_one_point_is_renderable(count: int) -> None:
    ax = axes().hexbin([1], [2], gridsize=count)
    assert "linearGradient" in ax.figure.to_svg()


def test_hexbin_explicit_limits_clip() -> None:
    ax = axes().xlim(0.2, 0.8).ylim(0.2, 0.8)
    ax.hexbin([0, 0.5, 1], [0, 0.5, 1], gridsize=3)
    assert ax.figure.to_svg() == ax.figure.to_svg()


def test_hexbin_renders_counts_with_log_color() -> None:
    ax = axes().hexbin([0, 0, 1, 2, np.nan], [0, 0, 1, 2, 1], gridsize=5, scale=ub.Scale.log())
    assert "linearGradient" in ax.figure.to_svg()
    assert ax.figure.to_svg() == ax.figure.to_svg()
