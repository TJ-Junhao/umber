"""``Axes.contour()``: conversions, exact errors, atomicity, styles, labels and the colorbar."""

from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"
X = np.linspace(-2.0, 2.0, 21)
Y = np.linspace(-1.5, 1.5, 16)
Z = np.exp(-(X[None, :] ** 2) - Y[:, None] ** 2) + 0.2 * X[None, :]


def axes() -> ub.Axes[float, float]:
    return ub.Figure(6, 4).add_axes()


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def parameter_of(error: BaseException) -> str | None:
    found = details(error)
    assert found is not None
    return found[0]


def tags(ax: ub.Axes[Any, Any], name: str) -> list[ElementTree.Element]:
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return list(root.iter(f"{SVG}{name}"))


def test_contour_returns_the_axes_and_draws_a_colorbar() -> None:
    ax = axes()
    assert ax.contour(X, Y, Z) is ax
    assert "linearGradient" in ax.figure.to_svg()
    filled = axes().contour(X, Y, Z, style=ub.Contour.FILLED)
    # A filled contour's colorbar is made of bands, not a gradient.
    assert "linearGradient" not in filled.figure.to_svg()
    assert any(path.get("fill-rule") == "evenodd" for path in tags(filled, "path"))


def test_lists_arrays_and_frames_match() -> None:
    outputs: set[str] = set()
    rows = [[0.0, 1.0, 2.0], [1.0, 2.0, 3.0]]
    inputs: list[object] = [
        rows,
        [tuple(row) for row in rows],
        np.array(rows),
        np.array(rows, dtype=np.float32),
        pd.DataFrame(rows),
    ]
    for z in inputs:
        outputs.add(axes().contour([0, 1, 2], (0.0, 1.0), z).figure.to_svg())
    assert len(outputs) == 1
    series = axes().contour(pd.Series([0, 1, 2]), np.array([0, 1]), rows)
    assert series.figure.to_svg() in outputs


@pytest.mark.parametrize("style", list(ub.Contour))
def test_every_style_renders(style: ub.Contour) -> None:
    ax = axes().contour(X, Y, Z, style=style, annotate=style != ub.Contour.FILLED)
    assert ax.figure.to_png().startswith(b"\x89PNG")


def test_level_labels_are_written_on_the_lines() -> None:
    ax = axes().contour(X, Y, Z, levels=[0.25, 0.5, 0.75], annotate=True)
    texts = [node.text for node in tags(ax, "text")]
    # Each level is a colorbar tick, and long lines carry it too.
    assert texts.count("0.50") >= 2, texts
    fixed = axes().contour(
        X, Y, Z, levels=[0.25, 0.5, 0.75], annotate=True, format=ub.TickFormat.fixed(1)
    )
    texts = [node.text for node in tags(fixed, "text")]
    assert "0.8" in texts and "0.75" in texts, texts


def test_shape_error_exact() -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.contour([0, 1, 2], [0, 1, 2, 3], np.zeros((3, 4)))
    assert str(caught.value) == (
        "Axes.contour(): z has shape (3, 4); it must be (len(y), len(x)) = (4, 3)."
    )
    assert details(caught.value) == (
        "z",
        "pass one row of z per y coordinate, each with one value per x coordinate.",
    )
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("x", "y", "z", "message", "parameter"),
    [
        (
            [0.0],
            [0.0, 1.0],
            [[1.0], [2.0]],
            "x has 1 coordinate; a contour grid needs at least two along each axis.",
            "x",
        ),
        (
            [0.0, 2.0, 1.0],
            [0.0, 1.0],
            np.zeros((2, 3)),
            "x[2] is 1, not above x[1] = 2; coordinates must be strictly increasing.",
            "x",
        ),
        (
            [0.0, 1.0],
            [0.0, float("nan")],
            np.zeros((2, 2)),
            "y[1] is NaN; coordinates must be finite.",
            "y",
        ),
        (
            [0.0, 1.0],
            [0.0, 1.0],
            [[0.0, float("inf")], [1.0, 2.0]],
            "z[0][1] is inf; values must be finite.",
            "z",
        ),
    ],
)
def test_data_errors_exact(x: object, y: object, z: object, message: str, parameter: str) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.contour(cast(Any, x), cast(Any, y), z)
    assert str(caught.value) == f"Axes.contour(): {message}"
    assert parameter_of(caught.value) == parameter
    assert ax.figure.to_svg() == before


def test_annotate_with_filled_rejected() -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.contour(X, Y, Z, style=ub.Contour.FILLED, annotate=True)
    assert str(caught.value) == (
        "Axes.contour(): annotate is True with style=Contour.FILLED; level labels need lines."
    )
    assert details(caught.value) == ("annotate", "use Contour.LINES or Contour.BOTH.")
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("kwargs", "error", "message", "parameter"),
    [
        (
            {"levels": 0},
            ub.StyleError,
            "levels is 0; a count of levels must be between 1 and 64.",
            "levels",
        ),
        (
            {"levels": -3},
            ub.StyleError,
            "levels is -3; a count of levels must be between 1 and 64.",
            "levels",
        ),
        (
            {"levels": 65},
            ub.StyleError,
            "levels is 65; a count of levels must be between 1 and 64.",
            "levels",
        ),
        ({"levels": []}, ub.StyleError, "levels is empty; pass at least one level.", "levels"),
        (
            {"levels": [1.0, 0.5]},
            ub.StyleError,
            "levels[1] is 0.5, not above levels[0] = 1; levels must be strictly increasing.",
            "levels",
        ),
        (
            {"style": ub.Contour.BOTH, "color": ub.Color.RED},
            ub.UsageError,
            "color is given with style=Contour.BOTH; filled bands take their colors from the "
            "colormap.",
            "color",
        ),
        (
            {"color": ub.Color.RED, "cmap": ub.Cmap.MAGMA},
            ub.UsageError,
            "cmap is given but color draws every line in one color; the lines are colored by "
            "level only with color=Color.AUTO.",
            "cmap",
        ),
        (
            {"format": ub.TickFormat.fixed(1)},
            ub.UsageError,
            "format is given but annotate is false; only annotated contours show level labels.",
            "format",
        ),
        (
            {"annotate": True, "format": ub.TickFormat.iso()},
            ub.StyleError,
            "format is TickFormat.iso(), which writes times; contour levels are numbers.",
            "format",
        ),
        (
            {"scale": ub.Scale.log(), "center": 1.0},
            ub.StyleError,
            "center is 1 with a log scale; a centered color scale is linear.",
            "center",
        ),
    ],
)
def test_option_errors_exact(
    kwargs: dict[str, object], error: type[ub.UmberError], message: str, parameter: str
) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(error) as caught:
        call(ax.contour)(X, Y, np.abs(Z) + 0.1, **kwargs)
    assert str(caught.value) == f"Axes.contour(): {message}"
    assert parameter_of(caught.value) == parameter
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("kwargs", "parameter"),
    [
        ({"levels": True}, "levels"),
        ({"levels": 2.5}, "levels"),
        ({"levels": "7"}, "levels"),
        ({"style": "filled"}, "style"),
        ({"style": 1}, "style"),
        ({"color": "#FF0000"}, "color"),
        ({"cmap": "viridis"}, "cmap"),
        ({"scale": ub.Scale.time()}, "scale"),
        ({"center": "0"}, "center"),
        ({"annotate": 1}, "annotate"),
        ({"format": "%.1f"}, "format"),
    ],
)
def test_type_errors(kwargs: dict[str, object], parameter: str) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        call(ax.contour)(X, Y, Z, **kwargs)
    assert parameter_of(caught.value) == parameter
    assert ax.figure.to_svg() == before


def test_axes_must_be_linear_or_log() -> None:
    for scales in ({"x": ub.Scale.category()}, {"y": ub.Scale.time()}):
        ax = cast(Any, ub.Figure().add_axes(**cast(Any, scales)))
        with pytest.raises(ub.UsageError):
            ax.contour(X, Y, Z)
    logs = ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.log())
    logs.contour(X + 3.0, Y + 2.0, Z)
    with pytest.raises(ub.DataError) as caught:
        logs.contour(X, Y + 2.0, Z)
    assert str(caught.value) == (
        "Axes.contour(): x[0] is -2; coordinates on a log axis must be positive."
    )


def test_one_color_draws_no_colorbar() -> None:
    ax = axes().contour(X, Y, Z, color=ub.Color.TEAL)
    assert "linearGradient" not in ax.figure.to_svg()
    strokes = {(path.get("stroke") or "").upper() for path in tags(ax, "path")}
    assert "#2E8B8B" in strokes


def test_contours_share_the_color_scale() -> None:
    ax = axes().contour(X, Y, Z)
    with pytest.raises(ub.UsageError) as caught:
        ax.contour(X, Y, Z, cmap=ub.Cmap.MAGMA)
    assert parameter_of(caught.value) == "cmap"
    ax.contour(X, Y, Z, style=ub.Contour.FILLED)
    ax.clim(-1.0, 2.0)
    assert ax.figure.to_png().startswith(b"\x89PNG")


def test_nan_values_are_holes() -> None:
    z = Z.copy()
    z[5:9, 8:12] = np.nan
    ax = axes().contour(X, Y, z, style=ub.Contour.BOTH)
    assert ax.figure.to_png().startswith(b"\x89PNG")
    empty = axes().contour(X, Y, np.full(Z.shape, np.nan), style=ub.Contour.FILLED)
    # Nothing to map: no colorbar.
    assert "fill-rule" not in empty.figure.to_svg()
