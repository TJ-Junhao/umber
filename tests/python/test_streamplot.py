from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"
X = np.linspace(-3.0, 3.0, 31)
Y = np.linspace(-2.0, 2.0, 21)
U = 1.0 + 0.2 * np.sin(Y)[:, None] + 0.0 * X
V = 0.4 * np.cos(X)[None, :] + 0.0 * Y[:, None]


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


def cylinder() -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    x, y = np.linspace(-3.0, 3.0, 61), np.linspace(-2.0, 2.0, 41)
    gx, gy = np.meshgrid(x, y)
    square = gx**2 + gy**2
    with np.errstate(divide="ignore", invalid="ignore"):
        u = 1 - (gx**2 - gy**2) / square**2
        v = -2 * gx * gy / square**2
    u[square < 1] = np.nan
    v[square < 1] = np.nan
    return x, y, u, v


def test_streamplot_returns_the_axes_and_draws() -> None:
    ax = axes()
    assert ax.streamplot(X, Y, U, V) is ax
    assert len(paths(ax)) > 0
    ax.figure.to_png()
    ax.figure.to_pdf()


def test_lists_arrays_and_frames_match() -> None:
    inputs: list[tuple[object, object]] = [
        (U, V),
        (U.tolist(), V.tolist()),
        (np.asfortranarray(U), V[:, ::-1][:, ::-1]),
        (pd.DataFrame(U), pd.DataFrame(V)),
    ]
    outputs = {axes().streamplot(X, Y, u, v).figure.to_svg() for u, v in inputs}
    assert len(outputs) == 1


def test_points_and_meshgrid_are_refused() -> None:
    with pytest.raises(ub.DataError, match="streamlines need a grid") as raised:
        axes().streamplot([0.0, 1.0], [0.0, 1.0], [1.0, 1.0], [0.0, 0.0])
    assert parameter_of(raised.value) == "u"
    grid_x, _ = np.meshgrid(X, Y)
    with pytest.raises(ub.DataError, match="coordinates must be one-dimensional"):
        axes().streamplot(grid_x, Y, U, V)
    with pytest.raises(ub.DataError, match=r"shaped \(len\(y\), len\(x\)\)"):
        axes().streamplot(X, Y, U[:, :-1], V)


def test_coordinates_are_checked_as_for_a_contour() -> None:
    with pytest.raises(ub.DataError, match="coordinates must be finite") as raised:
        axes().streamplot([0.0, np.nan, 2.0], [0.0, 1.0], np.ones((2, 3)), np.ones((2, 3)))
    assert parameter_of(raised.value) == "x"
    with pytest.raises(ub.DataError, match="strictly increasing"):
        axes().streamplot([0.0, 2.0, 1.0], [0.0, 1.0], np.ones((2, 3)), np.ones((2, 3)))
    with pytest.raises(ub.DataError, match="needs at least two along each axis"):
        axes().streamplot([0.0], [0.0, 1.0], np.ones((2, 1)), np.ones((2, 1)))


def test_wrong_argument_types_are_refused() -> None:
    cases: list[tuple[dict[str, object], str]] = [
        ({"density": "dense"}, "density"),
        ({"starts": [0.5, 0.5]}, "starts"),
        ({"starts": ([0.5],)}, "starts"),
        ({"arrows": "yes"}, "arrows"),
        ({"label": 3}, "label"),
        ({"color": "red"}, "color"),
    ]
    for keywords, parameter in cases:
        with pytest.raises(TypeError) as raised:
            call(axes().streamplot)(X, Y, U, V, **keywords)
        assert parameter_of(raised.value) == parameter, keywords


def test_invalid_values_are_refused_at_the_call() -> None:
    with pytest.raises(ub.StyleError, match="it must be from 0.1 to 10") as raised:
        axes().streamplot(X, Y, U, V, density=20)
    assert parameter_of(raised.value) == "density"
    with pytest.raises(ub.StyleError):
        axes().streamplot(X, Y, U, V, label="a\nb")
    with pytest.raises(ub.UsageError, match="color-mapped streamlines have no legend entry"):
        axes().streamplot(X, Y, U, V, color=ub.ColorBy(U), label="Flow")


def test_starts_draw_one_line_each_and_are_checked() -> None:
    few = axes().streamplot(X, Y, U, V, starts=([-2.0, 0.0, 0.0], [0.0, 0.0, 0.01]))
    many = axes().streamplot(X, Y, U, V)
    assert few.figure.to_svg() != many.figure.to_svg()
    messages: list[tuple[tuple[list[float], list[float]], str]] = [
        (([5.0], [0.0]), "lies outside the grid"),
        (([0.0, 1.0], [0.0]), "they must have the same length"),
        (([], []), "starts is empty"),
        (([np.nan], [0.0]), "a start must be finite"),
    ]
    for starts, message in messages:
        with pytest.raises(ub.DataError, match=message) as raised:
            axes().streamplot(X, Y, U, V, starts=starts)
        assert parameter_of(raised.value) == "starts"
    x, y, u, v = cylinder()
    with pytest.raises(ub.DataError, match="lies in a hole"):
        axes().streamplot(x, y, u, v, starts=([0.0], [0.0]))


def test_a_color_mapping_takes_the_shape_of_u_and_draws_a_colorbar() -> None:
    x, y, u, v = cylinder()
    speed = ub.ColorBy(np.hypot(u, v))
    ax = axes().streamplot(x, y, u, v, color=speed).colorbar(label="Speed")
    assert len(paths(ax)) > 10
    with pytest.raises(ub.DataError, match="color has shape") as raised:
        axes().streamplot(x, y, u, v, color=ub.ColorBy(np.ones((2, 2))))
    assert parameter_of(raised.value) == "color"
    with pytest.raises(ub.DataError, match=r"color has shape \(5,\)"):
        axes().streamplot(x, y, u, v, color=ub.ColorBy(np.ones(5)))


def test_arrows_density_and_label_change_the_drawing() -> None:
    plain = axes().streamplot(X, Y, U, V).figure.to_svg()
    assert axes().streamplot(X, Y, U, V, arrows=False).figure.to_svg() != plain
    assert axes().streamplot(X, Y, U, V, density=2).figure.to_svg() != plain
    labelled = axes().streamplot(X, Y, U, V, label="Flow").figure.to_svg(text=ub.SvgText.EDITABLE)
    assert "Flow" in labelled


def test_log_axes_take_streamlines_and_time_and_category_axes_do_not() -> None:
    logs = ub.Figure().add_axes(x=ub.Scale.log())
    xs = np.array([1.0, 10.0, 100.0])
    logs.streamplot(xs, [0.0, 1.0], np.ones((2, 3)) * xs, np.zeros((2, 3)))
    logs.figure.to_svg()
    for scales in ({"x": ub.Scale.time()}, {"y": ub.Scale.category()}):
        ax = cast(Any, ub.Figure().add_axes(**cast(Any, scales)))
        with pytest.raises(ub.UsageError, match="must be a numeric axis"):
            call(ax.streamplot)(X, Y, U, V)


def test_equal_aspect_and_holes_draw() -> None:
    x, y, u, v = cylinder()
    ax = axes().streamplot(x, y, u, v).equal_aspect()
    ax.figure.to_png()


def test_the_same_figure_draws_the_same_bytes() -> None:
    x, y, u, v = cylinder()
    first = axes().streamplot(x, y, u, v, density=2).figure
    second = axes().streamplot(x, y, u, v, density=2).figure
    assert first.to_png() == second.to_png()
    assert first.to_svg() == second.to_svg()
