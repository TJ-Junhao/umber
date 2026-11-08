"""``Axes.tricontour()``: conversions, exact errors, atomicity, styles, holes, order and
winding, axes, limits, the shared color scale, the colorbar and level labels."""

from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"

# A unit square around a center point: four triangles meeting at point 4, high at the center.
FAN_X = [0.0, 1.0, 1.0, 0.0, 0.5]
FAN_Y = [0.0, 0.0, 1.0, 1.0, 0.5]
PEAK = [0.0, 1.0, 2.0, 1.0, 4.0]


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


def scattered(count: int, seed: int) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    rng = np.random.default_rng(seed)
    x, y = rng.uniform(-2.0, 2.0, count), rng.uniform(-1.5, 1.5, count)
    return x, y, np.exp(-(x**2) - y**2) + 0.2 * x


def test_tricontour_returns_the_axes_and_draws_a_colorbar() -> None:
    x, y, z = scattered(200, 1)
    ax = axes()
    assert ax.tricontour(x, y, z) is ax
    assert "linearGradient" in ax.figure.to_svg()
    filled = axes().tricontour(x, y, z, style=ub.Contour.FILLED)
    # A filled contour's colorbar is made of bands, not a gradient.
    assert "linearGradient" not in filled.figure.to_svg()
    assert any(path.get("fill-rule") == "evenodd" for path in tags(filled, "path"))


@pytest.mark.parametrize("style", list(ub.Contour))
def test_every_style_renders_in_png_svg_and_pdf(style: ub.Contour) -> None:
    x, y, z = scattered(150, 2)
    for theme in (ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT):
        fig = ub.Figure(5, 4, theme=theme)
        fig.add_axes().tricontour(x, y, z, style=style, annotate=style != ub.Contour.FILLED)
        assert fig.to_png(dpi=72).startswith(b"\x89PNG")
        assert fig.to_svg().startswith("<svg")
        assert fig.to_pdf().startswith(b"%PDF")


def test_lists_arrays_and_series_match() -> None:
    inputs: list[tuple[object, object, object]] = [
        (FAN_X, FAN_Y, PEAK),
        (tuple(FAN_X), tuple(FAN_Y), tuple(PEAK)),
        (np.array(FAN_X), np.array(FAN_Y), np.array(PEAK, dtype=np.float32)),
        (pd.Series(FAN_X), pd.Series(FAN_Y), pd.Series(PEAK)),
    ]
    outputs = {
        cast(ub.Axes[float, float], call(axes().tricontour)(*given)).figure.to_svg()
        for given in inputs
    }
    assert len(outputs) == 1


def test_either_winding_and_any_row_order_draw_the_same() -> None:
    given = np.array([[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]])
    flipped = given[::-1, ::-1]

    def drawn(triangles: np.ndarray) -> str:
        ax = axes().tricontour(
            FAN_X, FAN_Y, PEAK, triangles=triangles, style=ub.Contour.BOTH, annotate=True
        )
        return ax.figure.to_svg()

    assert drawn(given) == drawn(flipped)
    x, y, z = scattered(80, 3)
    tri = ub.delaunay(x, y)
    assert (
        axes().tricontour(x, y, z, triangles=tri).figure.to_svg()
        == axes().tricontour(x, y, z).figure.to_svg()
    )


def test_level_labels_are_written_on_the_lines_as_the_colorbar_writes_them() -> None:
    x, y, z = scattered(400, 4)
    ax = axes().tricontour(x, y, z, levels=[0.25, 0.5, 0.75], annotate=True)
    texts = [node.text for node in tags(ax, "text")]
    # Each level is a colorbar tick, and long lines carry it too.
    assert texts.count("0.50") >= 2, texts
    fixed = axes().tricontour(
        x, y, z, levels=[0.25, 0.5, 0.75], annotate=True, format=ub.TickFormat.fixed(1)
    )
    texts = [node.text for node in tags(fixed, "text")]
    assert "0.5" in texts, texts


def test_z_needs_one_value_per_point() -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.tricontour(FAN_X, FAN_Y, PEAK[:4])
    assert str(caught.value) == (
        "Axes.tricontour(): z has 4 values; it must have one per point, 5."
    )
    assert parameter_of(caught.value) == "z"
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("kwargs", "error", "message", "parameter"),
    [
        ({"z": [0.0, 1.0, np.inf, 1.0, 4.0]}, ub.DataError, r"z\[2\] is inf", "z"),
        ({"scale": ub.Scale.log()}, ub.DataError, "values on a log scale must be positive", "z"),
        ({"levels": 0}, ub.StyleError, "a count of levels must be between 1 and 64", "levels"),
        ({"levels": [2.0, 1.0]}, ub.StyleError, "strictly increasing", "levels"),
        (
            {"style": ub.Contour.FILLED, "color": ub.Color.RED},
            ub.UsageError,
            "color is given with style=Contour.FILLED",
            "color",
        ),
        (
            {"style": ub.Contour.FILLED, "annotate": True},
            ub.UsageError,
            "level labels need lines",
            "annotate",
        ),
        ({"format": ub.TickFormat.fixed(1)}, ub.UsageError, "annotate is false", "format"),
        ({"color": ub.Color.RED, "center": 0.0}, ub.UsageError, "center is given", "center"),
    ],
)
def test_option_errors_name_their_argument(
    kwargs: dict[str, object], error: type[Exception], message: str, parameter: str
) -> None:
    arguments: dict[str, object] = {"z": PEAK, **kwargs}
    z = arguments.pop("z")
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(error, match=message) as caught:
        call(ax.tricontour)(FAN_X, FAN_Y, z, **arguments)
    assert str(caught.value).startswith("Axes.tricontour(): ")
    assert parameter_of(caught.value) == parameter
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("kwargs", "parameter"),
    [
        ({"style": "lines"}, "style"),
        ({"levels": "auto"}, "levels"),
        ({"color": "red"}, "color"),
        ({"annotate": 1}, "annotate"),
        ({"format": "%.1f"}, "format"),
        ({"triangles": np.array([[0.0, 1.0, 4.0]])}, "triangles"),
        ({"scale": ub.Scale.time()}, "scale"),
    ],
)
def test_type_errors(kwargs: dict[str, object], parameter: str) -> None:
    with pytest.raises(TypeError) as caught:
        call(axes().tricontour)(FAN_X, FAN_Y, PEAK, **kwargs)
    assert parameter_of(caught.value) == parameter


def test_points_and_triangles_are_checked_as_for_triplot() -> None:
    with pytest.raises(ub.DataError, match=r"triangles\[0\]\[2\] is 5") as caught:
        axes().tricontour(FAN_X, FAN_Y, PEAK, triangles=[[0, 1, 5]])
    assert parameter_of(caught.value) == "triangles"
    with pytest.raises(ub.DataError, match="lie on one line"):
        axes().tricontour([0.0, 1.0, 2.0], [0.0, 1.0, 2.0], [1.0, 2.0, 3.0])
    with pytest.raises(ub.DataError, match="must have the same length"):
        axes().tricontour(FAN_X, FAN_Y[:4], PEAK)


def test_nan_points_make_holes() -> None:
    def bands(z: list[float]) -> int:
        ax = axes().tricontour(FAN_X, FAN_Y, z, style=ub.Contour.FILLED, levels=[0.5])
        return ax.figure.to_svg(text=ub.SvgText.EDITABLE).count("evenodd")

    # Both bands of the level are drawn; a NaN at the center removes all four triangles.
    assert bands(PEAK) - bands([0.0, 1.0, 2.0, 1.0, np.nan]) == 2
    holed = axes().tricontour(FAN_X, FAN_Y, [np.nan, 1.0, 2.0, 1.0, 4.0], style=ub.Contour.BOTH)
    assert holed.figure.to_png(dpi=72).startswith(b"\x89PNG")


def test_numeric_axes_linear_and_log_and_no_others() -> None:
    logs = ub.Figure().add_axes(x=ub.Scale.log())
    x = [1.0, 100.0, 100.0, 1.0, 10.0]
    logs.tricontour(x, FAN_Y, PEAK, style=ub.Contour.BOTH).triplot(x, FAN_Y)
    assert logs.figure.to_svg().startswith("<svg")
    with pytest.raises(ub.DataError, match="coordinates on a log axis must be positive"):
        ub.Figure().add_axes(x=ub.Scale.log()).tricontour(FAN_X, FAN_Y, PEAK)
    timed = ub.Figure().add_axes(x=ub.Scale.time())
    named = ub.Figure().add_axes(y=ub.Scale.category())
    for other in (timed, named):
        ax = cast(ub.Axes[float, float], other)
        with pytest.raises(ub.UsageError):
            ax.tricontour(FAN_X, FAN_Y, PEAK)


def test_shares_the_color_scale_with_tripcolor_and_mesh() -> None:
    ax = axes().tripcolor(FAN_X, FAN_Y, PEAK).tricontour(FAN_X, FAN_Y, PEAK, style=ub.Contour.BOTH)
    ax.mesh([2, 3], [0, 1], [[-6.0]]).colorbar(label="Level")
    assert "Level" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    with pytest.raises(ub.UsageError) as caught:
        ax.tricontour(FAN_X, FAN_Y, PEAK, cmap=ub.Cmap.VIRIDIS)
    assert parameter_of(caught.value) == "cmap"
    centered = axes().tricontour(FAN_X, FAN_Y, PEAK, center=1.0, style=ub.Contour.FILLED)
    assert centered.colorbar().figure.to_png(dpi=72).startswith(b"\x89PNG")
    logged = axes().tricontour(FAN_X, FAN_Y, [1.0, 10.0, 100.0, 10.0, 1e4], scale=ub.Scale.log())
    assert logged.colorbar().figure.to_png(dpi=72).startswith(b"\x89PNG")


def test_contours_on_grids_and_meshes_sharing_a_colorbar_need_the_same_levels() -> None:
    ax = axes().tricontour(FAN_X, FAN_Y, PEAK).contour([0, 1], [0, 1], [[0.0, 1.0], [2.0, 30.0]])
    with pytest.raises(ub.StyleError, match="different levels"):
        ax.figure.to_svg()
    same = axes().tricontour(FAN_X, FAN_Y, PEAK, levels=[1.0, 2.0])
    same.contour([0, 1], [0, 1], [[0.0, 1.0], [2.0, 30.0]], levels=[1.0, 2.0])
    assert same.figure.to_svg().startswith("<svg")


def test_one_color_draws_no_colorbar() -> None:
    ax = axes().tricontour(FAN_X, FAN_Y, PEAK, color=ub.Color.TEAL)
    assert "linearGradient" not in ax.figure.to_svg()
    strokes = {(path.get("stroke") or "").upper() for path in tags(ax, "path")}
    assert "#2E8B8B" in strokes


def test_limits_are_the_extent_of_the_points_and_hold_equal_aspect() -> None:
    x, y = [*FAN_X, 3.0], [*FAN_Y, -1.0]
    ax = axes().tricontour(x, y, [*PEAK, 1.0], triangles=[[0, 1, 4], [1, 2, 4]])
    ax.equal_aspect()
    labels = [text.text for text in tags(ax, "text")]
    assert "0" in labels and "3" in labels
    assert ax.figure.to_png(dpi=72).startswith(b"\x89PNG")


def test_points_no_triangle_draws_do_not_set_the_levels_or_the_colors() -> None:
    x, y, z = [0.0, 1.0, 0.0, 2.0], [0.0, 0.0, 1.0, 2.0], [0.0, 1.0, 2.0, 1e6]
    # With levels from the point no triangle uses, no line would cross the triangle.
    red = axes().tricontour(x, y, z, triangles=[[0, 1, 2]], color=ub.Color.RED)
    strokes = [
        path for path in tags(red, "path") if (path.get("stroke") or "").upper() == "#D64933"
    ]
    assert strokes
    # The colorbar spans the drawn values, 0 to 2, not the point's 1e6.
    mapped = axes().tricontour(x, y, z, triangles=[[0, 1, 2]]).colorbar()

    def number(text: str) -> float | None:
        try:
            return float(text.replace("\u2212", "-").replace(",", ""))
        except ValueError:
            return None

    values = [number(node.text or "") for node in tags(mapped, "text")]
    assert all(value is None or value <= 2.0 for value in values), values
    # Every value is still checked.
    with pytest.raises(ub.DataError, match=r"z\[3\] is inf"):
        axes().tricontour(x, y, [0.0, 1.0, 2.0, np.inf], triangles=[[0, 1, 2]])


def test_no_triangle_left_draws_nothing_and_no_colorbar() -> None:
    gone = [0.0, 1.0, 2.0, 1.0, np.nan]
    for style in ub.Contour:
        ax = axes().tricontour(FAN_X, FAN_Y, gone, style=style)
        svg = ax.figure.to_svg()
        assert "linearGradient" not in svg and "evenodd" not in svg, style
    empty = axes().contour([0, 1], [0, 1], [[np.nan, np.nan], [np.nan, np.nan]])
    assert "linearGradient" not in empty.figure.to_svg()


EMPTY = ([0.0, 1.0, 0.0, 2.0], [0.0, 0.0, 1.0, 2.0], [0.0, 1.0, np.nan, 2.0])


def test_a_mesh_contour_that_draws_nothing_takes_no_part_in_the_colorbar() -> None:
    x, y, z = EMPTY
    alone = axes().tricontour([0, 1, 0], [0, 0, 1], [0, 1, 2]).figure.to_svg()
    ax = axes().tricontour([0, 1, 0], [0, 0, 1], [0, 1, 2])
    ax.tricontour(x, y, z, triangles=[[0, 1, 2]])
    assert ax.figure.to_svg().startswith("<svg")
    given = axes().tricontour([0, 1, 0], [0, 0, 1], [0, 1, 2])
    given.tricontour(x, y, z, triangles=[[0, 1, 2]], levels=[5.0], style=ub.Contour.BOTH)
    assert given.figure.to_svg().startswith("<svg")
    assert "linearGradient" in alone


@pytest.mark.parametrize("under", ["mesh", "tripcolor"])
def test_an_empty_filled_mesh_contour_keeps_the_gradient_colorbar(under: str) -> None:
    x, y, z = EMPTY
    ax = axes()
    if under == "mesh":
        ax.mesh([0, 1, 2], [0, 1], [[1.0, 3.0]])
    else:
        ax.tripcolor(x, y, [1.0, 3.0, 2.0, 2.0])
    ax.tricontour(x, y, z, triangles=[[0, 1, 2]], style=ub.Contour.FILLED).colorbar()
    assert "linearGradient" in ax.figure.to_svg()
