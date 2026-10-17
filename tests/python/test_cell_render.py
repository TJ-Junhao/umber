"""How heatmap, 2-D histogram and mesh cells are drawn: ``render=`` and ``ub.Render``."""

import datetime as dt
import xml.etree.ElementTree as ET
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details

NS = "{http://www.w3.org/2000/svg}"
RENDERS = [ub.AUTO, ub.Render.VECTOR, ub.Render.RASTER]
TYPE_HINT = "use ub.Render.VECTOR or ub.Render.RASTER, or ub.AUTO to choose by the number of cells."
UNEVEN_HINT = "use render=ub.Render.VECTOR, or evenly spaced edges."


def call(function: object) -> Callable[..., Any]:
    return cast(Callable[..., Any], function)


def images(fig: ub.Figure) -> list[ET.Element]:
    return list(ET.fromstring(fig.to_svg()).iter(f"{NS}image"))


def heatmap(columns: int, rows: int, render: object = ub.AUTO) -> ub.Figure:
    values = np.arange(columns * rows, dtype=float).reshape(rows, columns)
    values[0, 0] = np.nan
    ax = ub.Figure(6, 4).add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    xs = [f"c{i}" for i in range(columns)]
    ys = [f"r{i}" for i in range(rows)]
    call(ax.heatmap)(xs, ys, values, render=render)
    return ax.figure


def hist2d(bins: tuple[int, int], render: object = ub.AUTO) -> ub.Figure:
    rng = np.random.default_rng(7)
    ax = ub.Figure(6, 4).add_axes()
    call(ax.hist2d)(rng.normal(size=2000), rng.normal(size=2000), bins=bins, render=render)
    return ax.figure


def mesh(kind: str, render: object = ub.AUTO, columns: int = 6, rows: int = 4) -> ub.Figure:
    values = np.arange(columns * rows, dtype=float).reshape(rows, columns)
    ys = np.linspace(0, 1, rows + 1)
    if kind == "linear":
        ax: ub.Axes[Any, Any] = ub.Figure(6, 4).add_axes()
        xs: Any = np.linspace(-1, 2, columns + 1)
    elif kind == "log":
        ax = ub.Figure(6, 4).add_axes(x=ub.Scale.log())
        xs = np.geomspace(1, 1000, columns + 1)
    else:
        ax = ub.Figure(6, 4).add_axes(x=ub.Scale.time())
        xs = [dt.datetime(2026, 1, 1) + dt.timedelta(hours=6 * i) for i in range(columns + 1)]
    call(ax.mesh)(xs, ys, values, render=render)
    return ax.figure


def figures(render: object) -> list[ub.Figure]:
    return [
        heatmap(6, 4, render),
        hist2d((9, 7), render),
        mesh("linear", render),
        mesh("log", render),
        mesh("time", render),
    ]


@pytest.mark.parametrize("render", RENDERS, ids=repr)
def test_each_setting_draws_its_form(render: object) -> None:
    for fig in figures(render):
        assert len(images(fig)) == (1 if render is ub.Render.RASTER else 0)


def test_raster_image_has_unsmoothed_scaling() -> None:
    fig = heatmap(6, 4, ub.Render.RASTER)
    [image] = images(fig)
    assert image.get("preserveAspectRatio") == "none"
    assert image.get("image-rendering") == "optimizeSpeed"
    assert image.get("class") == "umber-raster"
    assert (image.get("href") or "").startswith("data:image/png;base64,")
    styles = [style.text for style in ET.fromstring(fig.to_svg()).iter(f"{NS}style")]
    assert styles == [
        "@supports (image-rendering: pixelated) { .umber-raster { image-rendering: pixelated } }"
    ]
    assert "<style" not in heatmap(6, 4, ub.Render.VECTOR).to_svg()


def test_auto_draws_an_image_from_ten_thousand_cells() -> None:
    assert not images(heatmap(99, 101))
    assert len(images(heatmap(100, 100))) == 1
    assert not images(hist2d((99, 101)))
    assert len(images(hist2d((100, 100)))) == 1
    assert not images(mesh("linear", columns=99, rows=101))
    assert len(images(mesh("linear", columns=100, rows=100))) == 1
    assert len(images(mesh("time", columns=100, rows=100))) == 1


def test_auto_draws_uneven_meshes_as_vector_shapes() -> None:
    edges = np.linspace(0, 1, 121)
    edges[60] += 0.002
    ax = ub.Figure(6, 4).add_axes().mesh(edges, np.linspace(0, 1, 101), np.ones((100, 120)))
    assert not images(ax.figure)


def test_output_is_deterministic() -> None:
    for fig in figures(ub.Render.RASTER):
        assert fig.to_png(dpi=100) == fig.to_png(dpi=100)
        assert fig.to_svg() == fig.to_svg()


@pytest.mark.parametrize(
    ("value", "name"), [("raster", "str"), (True, "bool"), (None, "None"), (1, "int")]
)
@pytest.mark.parametrize("method", ["heatmap", "hist2d", "mesh"])
def test_wrong_types_are_atomic(method: str, value: object, name: str) -> None:
    ax: ub.Axes[Any, Any]
    if method == "heatmap":
        ax = ub.Figure(6, 4).add_axes(x=ub.Scale.category(), y=ub.Scale.category())
        ax.heatmap(["a", "b"], ["r"], [[1, 2]])
        args: tuple[object, ...] = (["a", "c"], ["r"], [[1, 2]])
    elif method == "hist2d":
        ax = ub.Figure(6, 4).add_axes().hist2d([0, 1, 2], [0, 1, 2])
        args = ([0, 3], [0, 3])
    else:
        ax = ub.Figure(6, 4).add_axes().mesh([0, 1], [0, 1], [[1.0]])
        args = ([0, 1, 2], [0, 1], [[1.0, 2.0]])
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        call(getattr(ax, method))(*args, render=value)
    assert str(caught.value) == (
        f"Axes.{method}(): render is {name}; render must be a Render or ub.AUTO."
    )
    assert details(caught.value) == ("render", TYPE_HINT)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("argument", ['render="raster"', "render=True", "render=None"])
def test_type_errors_highlight_render(tmp_path: Path, argument: str) -> None:
    line = f'ax.heatmap(["a"], ["b"], [[1.0]], {argument})'
    path = tmp_path / "render_call.py"
    source = (
        "import umber as ub\n"
        "ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())\n"
        f"{line}\n"
    )
    path.write_text(source)
    with pytest.raises(TypeError) as caught:
        exec(compile(source, str(path), "exec"), {})
    marked = f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"
    assert getattr(caught.value, "__notes__", [])[0] == marked


@pytest.mark.parametrize(
    ("kind", "message"),
    [
        (
            "linear",
            "Axes.mesh(): the step from x_edges[2] = 2 to x_edges[3] = 4 is 2, not 1 as from "
            "x_edges[0] to x_edges[1]; drawing the cells as one image needs evenly spaced "
            "edges on the x axis.",
        ),
        (
            "log",
            "Axes.mesh(): the step from y_edges[1] = 2 to y_edges[2] = 3 is a factor of 1.5, "
            "not 2 as from y_edges[0] to y_edges[1]; drawing the cells as one image needs "
            "evenly spaced edges on the y axis.",
        ),
        (
            "time",
            "Axes.mesh(): the step from x_edges[1] = 2026-02-01 to x_edges[2] = 2026-03-01 is "
            "28 days, not 31 days as from x_edges[0] to x_edges[1]; drawing the cells as one "
            "image needs evenly spaced edges on the x axis.",
        ),
    ],
)
def test_raster_refuses_uneven_edges(kind: str, message: str) -> None:
    ax: ub.Axes[Any, Any]
    if kind == "linear":
        ax = ub.Figure(6, 4).add_axes()
        x: Any = [0, 1, 2, 4]
        y: Any = [0, 1]
    elif kind == "log":
        ax = ub.Figure(6, 4).add_axes(y=ub.Scale.log())
        x, y = [0, 1, 2, 3], [1, 2, 3]
    else:
        ax = ub.Figure(6, 4).add_axes(x=ub.Scale.time())
        x = [dt.date(2026, month, 1) for month in (1, 2, 3, 4)]
        y = [0, 1]
    values = np.ones((len(y) - 1, len(x) - 1))
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.mesh(x, y, values, render=ub.Render.RASTER)
    assert str(caught.value) == message
    assert details(caught.value) == ("render", UNEVEN_HINT)
    assert ax.figure.to_svg() == before
    # Without an explicit image the same edges draw vector cells.
    ax.mesh(x, y, values)
    assert not images(ax.figure)


def test_hexbin_has_no_render_argument() -> None:
    ax = ub.Figure(6, 4).add_axes()
    with pytest.raises(TypeError, match="render"):
        call(ax.hexbin)([0, 1], [0, 1], render=ub.Render.RASTER)


def test_render_members() -> None:
    assert [(m.name, m.value) for m in ub.Render] == [("VECTOR", 0), ("RASTER", 1)]
    assert ub.Render.__module__ == "umber"
