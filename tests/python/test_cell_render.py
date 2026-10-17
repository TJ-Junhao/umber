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
            "Axes.mesh(): x_edges[1] = 1 is off the evenly spaced edges from x_edges[0] = 0 to "
            "x_edges[3] = 4, which put it at 1.3333333333333333; drawing the cells as one image "
            "needs evenly spaced edges on the x axis.",
        ),
        (
            "log",
            "Axes.mesh(): y_edges[1] = 2 is off the evenly spaced logarithms from y_edges[0] = 1 "
            "to y_edges[2] = 16, which put it at 4; drawing the cells as one image needs evenly "
            "spaced edges on the y axis.",
        ),
        (
            "time",
            "Axes.mesh(): x_edges[1] = 2026-02-01 is off the evenly spaced times from x_edges[0] "
            "= 2026-01-01 to x_edges[3] = 2026-04-01, which put it at 2026-01-31; drawing the "
            "cells as one image needs evenly spaced edges on the x axis.",
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
        x, y = [0, 1, 2, 3], [1, 2, 16]
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


def test_microsecond_steps_are_compared_exactly() -> None:
    start = dt.datetime(9999, 1, 1)
    uneven = [start + dt.timedelta(microseconds=us) for us in (0, 1, 9)]
    ax = ub.Figure(6, 4).add_axes(x=ub.Scale.time())
    with pytest.raises(ub.UsageError, match=r"x_edges\[1\] = 9999-01-01T00:00:00.000001 is off"):
        ax.mesh(uneven, [0, 1], [[1.0, 2.0]], render=ub.Render.RASTER)
    late = dt.datetime(2500, 1, 1)
    even = [late + dt.timedelta(microseconds=us) for us in range(4)]
    ax = ub.Figure(6, 4).add_axes(x=ub.Scale.time())
    ax.mesh(even, [0, 1], [[1.0, 2.0, 3.0]], render=ub.Render.RASTER)
    assert len(images(ax.figure)) == 1


def test_edges_must_lie_on_one_lattice() -> None:
    x = 1e12 + np.concatenate([np.arange(601) * 0.1, 60 + np.arange(1, 601) * 0.1008])
    ax = ub.Figure(6, 4).add_axes()
    with pytest.raises(ub.UsageError, match=r"x_edges\[2\] = 1\.0000000000002e12 is off"):
        ax.mesh(x, [0, 1, 2], np.ones((2, 1200)), render=ub.Render.RASTER)
    ax.mesh(x, np.arange(11.0), np.ones((10, 1200)))
    assert not images(ax.figure)
    rounded = 1e12 + np.arange(1201) * 0.1
    ub.Figure(6, 4).add_axes().mesh(rounded, [0, 1], np.ones((1, 1200)), render=ub.Render.RASTER)


def test_raster_refuses_cells_too_wide_for_an_image_when_rendering() -> None:
    def sliver(render: ub.Render | ub.Auto) -> ub.Figure:
        edges = np.arange(101.0)
        ax = ub.Figure(6, 4).add_axes()
        ax.mesh(edges, edges, np.ones((100, 100)), render=render).xlim(50 - 1e-4, 50 + 1e-4)
        return ax.figure

    assert not images(sliver(ub.AUTO))
    with pytest.raises(ub.StyleError) as caught:
        sliver(ub.Render.RASTER).to_svg()
    message = str(caught.value)
    assert message.startswith(
        "Figure.to_svg(): mark 1 of these axes, a mesh, has a cell cut by the x limits that "
        "reaches "
    ), message
    assert details(caught.value) == (
        None,
        "use render=ub.Render.VECTOR or ub.AUTO, or widen the limits.",
    )


ZOOM_HINT = "use render=ub.Render.VECTOR or ub.AUTO."


def test_raster_refuses_edges_that_zooming_moves_on_screen() -> None:
    f = ub.Figure(6, 3)
    f.add_axes().mesh([0, 1.0000999, 2], [0, 1], [[1, 2]], render=ub.Render.RASTER).xlim(0.98, 1.02)
    with pytest.raises(ub.StyleError) as caught:
        f.to_png(dpi=600)
    assert str(caught.value).startswith(
        "Figure.to_png(): mark 1 of these axes, a mesh, has x_edges[1] "
    ), caught.value
    assert details(caught.value) == (None, ZOOM_HINT)
    start = dt.datetime(2500, 1, 1)
    times = [start + dt.timedelta(microseconds=us) for us in (0, 10_000_999, 20_000_000)]
    t = ub.Figure(6, 3)
    t.add_axes(x=ub.Scale.time()).mesh(times, [0, 1], [[1, 2]], render=ub.Render.RASTER).xlim(
        start + dt.timedelta(seconds=9.8), start + dt.timedelta(seconds=10.2)
    )
    with pytest.raises(ub.StyleError, match="a mesh, has x_edges"):
        t.to_png(dpi=600)


@pytest.mark.parametrize("base", [1e299, 1e-299])
def test_narrow_log_edges_far_from_one_are_refused(base: float) -> None:
    x = [base, base * (1 + 5.504e-10), base * (1 + 1.1e-9)]
    ax = ub.Figure(14, 3).add_axes(x=ub.Scale.log())
    with pytest.raises(ub.UsageError, match="is off the evenly spaced logarithms"):
        ax.mesh(x, [0, 1], [[1, 2]], render=ub.Render.RASTER)


def test_a_blank_raster_mesh_renders() -> None:
    f = ub.Figure(6, 4)
    f.add_axes().mesh([0, 1, 2], [0, 1], [[np.nan, np.nan]], render=ub.Render.RASTER).xlim(
        1 - 1e-6, 1 + 1e-6
    )
    assert "<image" not in f.to_svg()


def test_expected_edges_are_written_exactly() -> None:
    ax = ub.Figure(6, 4).add_axes(x=ub.Scale.log())
    with pytest.raises(ub.UsageError, match=", which put it at 1; "):
        ax.mesh([1e-300, 1e-100, 1e300], [0, 1], [[1, 2]], render=ub.Render.RASTER)
    epoch = dt.datetime(1970, 1, 1)
    times = [epoch + dt.timedelta(microseconds=us) for us in (0, 1, 3)]
    tx = ub.Figure(6, 4).add_axes(x=ub.Scale.time())
    with pytest.raises(ub.UsageError, match=r"at 1970-01-01T00:00:00\.000001 plus 0\.5 micro"):
        tx.mesh(times, [0, 1], [[1, 2]], render=ub.Render.RASTER)


def test_hexbin_has_no_render_argument() -> None:
    ax = ub.Figure(6, 4).add_axes()
    with pytest.raises(TypeError, match="render"):
        call(ax.hexbin)([0, 1], [0, 1], render=ub.Render.RASTER)


def test_render_members() -> None:
    assert [(m.name, m.value) for m in ub.Render] == [("VECTOR", 0), ("RASTER", 1)]
    assert ub.Render.__module__ == "umber"
