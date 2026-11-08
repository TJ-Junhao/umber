from collections.abc import Callable
from typing import cast
from xml.etree import ElementTree

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"

# A unit square around a center point: four triangles meeting at point 4.
FAN_X = [0.0, 1.0, 1.0, 0.0, 0.5]
FAN_Y = [0.0, 0.0, 1.0, 1.0, 0.5]


def axes() -> ub.Axes[float, float]:
    return ub.Figure(6, 4).add_axes()


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def parameter_of(error: BaseException) -> str | None:
    found = details(error)
    assert found is not None
    return found[0]


def paths(ax: ub.Axes[float, float]) -> list[ElementTree.Element]:
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return list(root.iter(f"{SVG}path"))


def scattered(count: int, seed: int) -> tuple[np.ndarray, np.ndarray]:
    rng = np.random.default_rng(seed)
    return rng.uniform(-2.0, 2.0, count), rng.uniform(-1.5, 1.5, count)


def test_delaunay_returns_int64_rows_counter_clockwise_from_their_smallest_index() -> None:
    x, y = scattered(80, 1)
    tri = ub.delaunay(x, y)
    assert tri.dtype == np.int64
    assert tri.ndim == 2 and tri.shape[1] == 3
    a, b, c = (tri[:, k] for k in range(3))
    cross = (x[b] - x[a]) * (y[c] - y[a]) - (y[b] - y[a]) * (x[c] - x[a])
    assert (cross > 0).all()
    assert ((a < b) & (a < c)).all()
    assert tri.tolist() == sorted(tri.tolist())
    assert set(tri.ravel().tolist()) == set(range(80))
    square = ub.delaunay([0, 1, 1, 0], [0, 0, 1, 1])
    assert square.tolist() == [[0, 1, 2], [0, 2, 3]]
    # The result is an ordinary array that can be changed.
    square[0, 0] = 3
    assert square[0, 0] == 3


def test_delaunay_takes_lists_series_and_integer_arrays_alike() -> None:
    x, y = [0.0, 2.0, 1.0, 1.0, 0.0], [0.0, 0.0, 2.0, 0.5, 1.5]
    expected = ub.delaunay(np.array(x), np.array(y)).tolist()
    assert ub.delaunay(x, y).tolist() == expected
    assert ub.delaunay(pd.Series(x), pd.Series(y)).tolist() == expected
    assert ub.delaunay([0, 2, 1], [0, 0, 1]).tolist() == [[0, 1, 2]]


@pytest.mark.parametrize(
    ("x", "y", "message", "parameter"),
    [
        ([0.0, 1.0], [0.0, 1.0], "x and y hold 2 points; a triangle needs at least three", "x"),
        ([0.0, 1.0, 2.0], [0.0, 1.0], "x has 3 values but y has 2", "y"),
        ([0.0, 1.0, 2.0], [0.0, 2.0, 4.0], "all 3 points lie on one line", "x"),
        (
            [0.0, 1.0, 1.0, 0.0, 0.5],
            [0.0, 0.0, 0.0, 0.0, 1.0],
            r"points 1 and 2 are both at \(1, 0\)",
            "x",
        ),
        ([0.0, 1.0, np.nan], [0.0, 1.0, 0.0], r"x\[2\] is NaN; coordinates must be finite", "x"),
        ([0.0, 1.0, 0.0], [0.0, np.inf, 1.0], r"y\[1\] is inf", "y"),
    ],
)
def test_delaunay_refuses_points_it_cannot_triangulate(
    x: list[float], y: list[float], message: str, parameter: str
) -> None:
    with pytest.raises(ub.DataError, match=message) as raised:
        ub.delaunay(x, y)
    assert str(raised.value).startswith("delaunay(): ")
    assert parameter_of(raised.value) == parameter


def test_delaunay_refuses_text_and_single_values() -> None:
    with pytest.raises(TypeError):
        call(ub.delaunay)("abc", [1.0, 2.0, 3.0])
    with pytest.raises(TypeError):
        call(ub.delaunay)([1.0, 2.0, 3.0], 4.0)


def test_triplot_and_tripcolor_return_the_axes_and_render() -> None:
    x, y = scattered(60, 2)
    ax = axes()
    assert ax.tripcolor(x, y, x * y) is ax
    assert ax.triplot(x, y, label="Mesh").scatter(x, y) is ax
    assert len(paths(ax)) > 0
    assert ax.figure.to_png(dpi=72).startswith(b"\x89PNG")
    assert ax.figure.to_pdf().startswith(b"%PDF")


def test_triangles_take_arrays_lists_and_filtered_delaunay_rows() -> None:
    x, y = scattered(90, 3)
    tri = ub.delaunay(x, y)
    keep = np.hypot(x[tri].mean(axis=1), y[tri].mean(axis=1)) > 0.6
    holed = tri[keep]
    svgs = {
        axes().triplot(x, y, triangles=given).figure.to_svg()
        for given in (holed, holed.tolist(), holed.astype(np.int32), holed.astype(np.uint16))
    }
    assert len(svgs) == 1
    assert (
        axes().triplot(x, y, triangles=tri).figure.to_svg() == axes().triplot(x, y).figure.to_svg()
    )


def test_either_winding_and_any_row_order_draw_the_same() -> None:
    given = np.array([[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]])
    values = np.array([1.0, 2.0, 3.0, 4.0])
    flipped = given[::-1, ::-1]

    def drawn(triangles: np.ndarray, on_triangles: np.ndarray) -> str:
        ax = axes().tripcolor(
            FAN_X, FAN_Y, on_triangles, triangles=triangles, on=ub.TriValues.TRIANGLES
        )
        return ax.triplot(FAN_X, FAN_Y, triangles=triangles).figure.to_svg()

    assert drawn(given, values) == drawn(flipped, values[::-1])


@pytest.mark.parametrize(
    ("triangles", "message"),
    [
        (np.zeros((0, 3), dtype=np.int64), "triangles is empty"),
        ([[0, 1, 4], [1, 2, 5]], r"triangles\[1\]\[2\] is 5; indices must be from 0 to 4"),
        ([[0, -1, 4]], r"triangles\[0\]\[1\] is -1"),
        ([[2, 4, 2]], r"triangles\[0\] is \[2, 4, 2\]; its three corners must be different"),
        ([[0, 4, 2]], r"lie on one line, so it has no area"),
        ([[0, 1, 4], [4, 1, 0]], r"triangles\[0\] and triangles\[1\] join the same points"),
        (
            [[0, 1, 4], [1, 2, 4], [0, 1, 2], [1, 3, 2]],
            "the edge from point 1 to point 2 borders triangles",
        ),
        ([0, 1, 4], r"triangles has shape \(3,\); it must be shaped \(n, 3\)"),
        ([[0, 1, 2, 3]], r"triangles has shape \(1, 4\)"),
        ([[0, 1, 2], [0, 1]], "rows of different lengths"),
        (np.ma.masked_array([[0, 1, 4]]), "masked array"),
        (np.array([[0, 1, 2**64 - 1]], dtype=np.uint64), "triangles holds 18446744073709551615"),
    ],
)
def test_wrong_triangles_are_data_errors(triangles: object, message: str) -> None:
    with pytest.raises(ub.DataError, match=message) as raised:
        call(axes().triplot)(FAN_X, FAN_Y, triangles=triangles)
    assert parameter_of(raised.value) == "triangles"
    with pytest.raises(ub.DataError, match=message) as raised:
        call(axes().tripcolor)(FAN_X, FAN_Y, [1.0] * 5, triangles=triangles)
    assert parameter_of(raised.value) == "triangles"


@pytest.mark.parametrize(
    ("triangles", "message"),
    [
        (np.array([[0.0, 1.0, 4.0]]), "triangles is an array of float64"),
        ([[0, 1, 4.0]], "triangles holds float"),
        ([[0, 1, True]], "triangles holds bool"),
        (np.array([[True, False, True]]), "triangles is an array of bool"),
        ("012", "triangles is str"),
        (3, "triangles is int"),
    ],
)
def test_triangles_that_are_not_integers_are_type_errors(triangles: object, message: str) -> None:
    with pytest.raises(TypeError, match=message) as raised:
        call(axes().triplot)(FAN_X, FAN_Y, triangles=triangles)
    assert parameter_of(raised.value) == "triangles"


def test_triplot_label_color_and_legend() -> None:
    ax = axes().triplot(FAN_X, FAN_Y, label="Mesh", color=ub.Color.RED)
    assert "Mesh" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    with pytest.raises(ub.StyleError) as raised:
        axes().triplot(FAN_X, FAN_Y, label="two\nlines")
    assert parameter_of(raised.value) == "label"
    with pytest.raises(TypeError):
        call(axes().triplot)(FAN_X, FAN_Y, color="red")
    with pytest.raises(TypeError):
        call(axes().triplot)(FAN_X, FAN_Y, label=3)


def test_each_edge_is_drawn_once_as_one_path() -> None:
    root = ElementTree.fromstring(
        axes().triplot(FAN_X, FAN_Y, color=ub.Color.RED).figure.to_svg(text=ub.SvgText.EDITABLE)
    )
    stroked = [
        path
        for path in root.iter(f"{SVG}path")
        if path.get("stroke", "none") != "none" and path.get("fill", "none") == "none"
    ]
    edges = [p for p in stroked if (p.get("d") or "").count("M") == 8]
    assert len(edges) == 1


def test_tripcolor_checks_the_number_of_values_against_on() -> None:
    with pytest.raises(ub.DataError, match="on=TriValues.POINTS needs one per point: 5") as raised:
        axes().tripcolor(FAN_X, FAN_Y, [1.0, 2.0, 3.0, 4.0])
    assert parameter_of(raised.value) == "values"
    with pytest.raises(ub.DataError, match="on=TriValues.TRIANGLES needs one per triangle: 4"):
        axes().tripcolor(FAN_X, FAN_Y, [1.0] * 5, on=ub.TriValues.TRIANGLES)
    # Six points and six triangles: both forms are accepted, and differ.
    x, y = [0.0, 3.0, 3.0, 0.0, 1.0, 2.0], [0.0, 0.0, 2.0, 2.0, 1.0, 1.1]
    assert len(ub.delaunay(x, y)) == 6
    values = [0.0, 3.0, 6.0, 9.0, 12.0, 15.0]
    on_points = axes().tripcolor(x, y, values).figure.to_svg()
    on_triangles = axes().tripcolor(x, y, values, on=ub.TriValues.TRIANGLES).figure.to_svg()
    assert on_points != on_triangles
    with pytest.raises(TypeError) as wrong:
        call(axes().tripcolor)(FAN_X, FAN_Y, [1.0] * 5, on="points")
    assert parameter_of(wrong.value) == "on"


def test_nan_leaves_triangles_blank() -> None:
    def filled(values: list[float], on: ub.TriValues) -> int:
        ax = axes().tripcolor(FAN_X, FAN_Y, values, on=on)
        root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
        return sum(
            (path.get("d") or "").count("M")
            for path in root.iter(f"{SVG}path")
            if path.get("fill", "none") not in ("none",) and path.get("stroke", "none") == "none"
        )

    full = filled([0.0, 3.0, 6.0, 9.0, 12.0], ub.TriValues.POINTS)
    # A NaN at a corner blanks the two triangles around it.
    assert filled([0.0, np.nan, 6.0, 9.0, 12.0], ub.TriValues.POINTS) == full - 2
    assert filled([1.0, np.nan, 3.0, 4.0], ub.TriValues.TRIANGLES) == full - 1
    with pytest.raises(ub.DataError, match=r"values\[2\] is inf"):
        axes().tripcolor(FAN_X, FAN_Y, [1.0, 2.0, np.inf, 4.0, 5.0])


def test_tripcolor_shares_the_color_scale_and_takes_a_colorbar() -> None:
    ax = axes().tripcolor(FAN_X, FAN_Y, [0.0, 3.0, 6.0, 9.0, 12.0])
    ax.mesh([2, 3], [0, 1], [[20.0]]).colorbar(label="Level")
    assert "Level" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    with pytest.raises(ub.UsageError) as raised:
        ax.tripcolor(FAN_X, FAN_Y, [1.0] * 5, cmap=ub.Cmap.VIRIDIS)
    assert parameter_of(raised.value) == "cmap"


def test_center_and_scale() -> None:
    centered = axes().tripcolor(FAN_X, FAN_Y, [-6.0, 0.0, 3.0, 6.0, 0.0], center=0).colorbar()
    centered.figure.to_png(dpi=72)
    logged = axes().tripcolor(FAN_X, FAN_Y, [3.0, 30.0, 300.0, 3000.0, 3.0], scale=ub.Scale.log())
    logged.colorbar().figure.to_png(dpi=72)
    with pytest.raises(ub.DataError, match="values on a log scale must be positive"):
        axes().tripcolor(FAN_X, FAN_Y, [1.0, 0.0, 1.0, 1.0, 1.0], scale=ub.Scale.log())
    with pytest.raises(ub.StyleError) as raised:
        axes().tripcolor(FAN_X, FAN_Y, [1.0] * 5, scale=ub.Scale.log(), center=1.0)
    assert parameter_of(raised.value) == "center"
    with pytest.raises(TypeError):
        call(axes().tripcolor)(FAN_X, FAN_Y, [1.0] * 5, scale=ub.Scale.time())


def test_numeric_axes_linear_and_log_and_no_others() -> None:
    logs = ub.Figure().add_axes(x=ub.Scale.log())
    logs.tripcolor([1.0, 100.0, 10.0], [0.0, 0.0, 1.0], [1.0, 2.0, 3.0]).triplot(
        [1.0, 100.0, 10.0], [0.0, 0.0, 1.0]
    )
    logs.figure.to_svg()
    with pytest.raises(ub.DataError, match="coordinates on a log axis must be positive"):
        ub.Figure().add_axes(x=ub.Scale.log()).triplot([0.0, 1.0, 2.0], [0.0, 1.0, 0.0])
    timed = ub.Figure().add_axes(x=ub.Scale.time())
    named = ub.Figure().add_axes(y=ub.Scale.category())
    for other in (timed, named):
        ax = cast(ub.Axes[float, float], other)
        with pytest.raises(ub.UsageError):
            ax.triplot(FAN_X, FAN_Y)
        with pytest.raises(ub.UsageError):
            ax.tripcolor(FAN_X, FAN_Y, [1.0] * 5)


def test_limits_are_the_extent_of_the_points_and_hold_equal_aspect() -> None:
    x, y = [0.0, 1.0, 1.0, 3.0], [0.0, 0.0, 1.0, -1.0]
    ax = axes().triplot(x, y, triangles=[[0, 1, 2]])
    ax.equal_aspect()
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    labels = [text.text for text in root.iter(f"{SVG}text")]
    assert "0" in labels and "3" in labels
    ax.figure.to_png(dpi=72)


def test_png_svg_and_pdf_render_on_every_theme() -> None:
    x, y = scattered(120, 4)
    for theme in (ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT):
        fig = ub.Figure(5, 4, theme=theme)
        ax = fig.add_axes().tripcolor(x, y, np.sin(x) * y).triplot(x, y).colorbar()
        ax.equal_aspect()
        assert fig.to_png(dpi=72).startswith(b"\x89PNG")
        assert fig.to_svg().startswith("<svg")
        assert fig.to_pdf().startswith(b"%PDF")
