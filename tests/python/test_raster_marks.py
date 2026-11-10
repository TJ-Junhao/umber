"""Scatters, lines and tripcolors drawn as one image in SVG and PDF output."""

import re
from collections.abc import Callable
from pathlib import Path
from typing import Any

import numpy as np
import numpy.typing as npt
import pytest

import umber as ub
from umber._errors import details

RASTER_DPI_RULE = "is out of range; use a value from 10 to 2400."


def _points(n: int) -> tuple[npt.NDArray[np.float64], npt.NDArray[np.float64]]:
    rng = np.random.default_rng(7)
    return rng.uniform(0, 10, n), rng.uniform(0, 10, n)


def _scatter(render: ub.Render, n: int = 2000) -> ub.Figure:
    x, y = _points(n)
    ax = ub.Figure().add_axes().xlim(0, 10).ylim(0, 10)
    ax.line([0, 10], [1, 9], label="Under")
    ax.scatter(x, y, label="Points", render=render)
    return ax.line([0, 10], [9, 1], label="Over").figure


def _line(render: ub.Render) -> ub.Figure:
    _, y = _points(5000)
    return ub.line(np.arange(5000.0), y, render=render).figure


def _tripcolor(render: ub.Render) -> ub.Figure:
    x, y = _points(400)
    values = x * y
    values[::17] = np.nan
    return ub.Figure().add_axes().tripcolor(x, y, values, render=render).figure


MAKERS: dict[str, Callable[[ub.Render], ub.Figure]] = {
    "scatter": _scatter,
    "line": _line,
    "tripcolor": _tripcolor,
}


def _images(svg: str) -> list[str]:
    return re.findall(r"<image [^>]*/>", svg)


def _pdf_images(pdf: bytes) -> int:
    return pdf.count(b"/Subtype /Image") - pdf.count(b"/SMask ")


def _number(pattern: str, text: str) -> float:
    found = re.search(pattern, text)
    assert found is not None, pattern
    return float(found.group(1))


@pytest.mark.parametrize("name", list(MAKERS))
def test_png_output_is_the_same_for_vector_and_raster(name: str) -> None:
    make = MAKERS[name]
    raster, vector = make(ub.Render.RASTER), make(ub.Render.VECTOR)
    assert raster.to_png(dpi=100) == vector.to_png(dpi=100)


@pytest.mark.parametrize("name", list(MAKERS))
def test_vector_output_holds_one_image_for_a_raster_mark(name: str) -> None:
    make = MAKERS[name]
    raster, vector = make(ub.Render.RASTER), make(ub.Render.VECTOR)
    assert len(_images(raster.to_svg())) == 1
    assert _images(vector.to_svg()) == []
    assert _pdf_images(raster.to_pdf()) == 1
    assert _pdf_images(vector.to_pdf()) == 0


def test_the_default_render_of_the_marks_is_vector() -> None:
    x, y = _points(50)
    plain = ub.Figure().add_axes().scatter(x, y).line(x, y).tripcolor(x, y, x).figure
    chosen = (
        ub.Figure()
        .add_axes()
        .scatter(x, y, render=ub.Render.VECTOR)
        .line(x, y, render=ub.Render.VECTOR)
        .tripcolor(x, y, x, render=ub.Render.VECTOR)
        .figure
    )
    assert plain.to_svg() == chosen.to_svg()
    assert _images(plain.to_svg()) == []


def test_each_group_is_an_image_of_its_own() -> None:
    x, y = _points(100)
    groups = ["a", "b"] * 50
    scatter = ub.scatter(x, y, by=groups, render=ub.Render.RASTER)
    assert len(_images(scatter.figure.to_svg())) == 2
    line = ub.Figure().add_axes().line(x, y, by=groups, render=ub.Render.RASTER)
    assert _pdf_images(line.figure.to_pdf()) == 2


def test_error_bars_and_value_labels_stay_vector() -> None:
    ax = (
        ub.Figure()
        .add_axes()
        .scatter(
            [1, 2, 3],
            [2.0, 3.0, 2.5],
            y_error=([1.5, 2.5, 2.0], [2.5, 3.5, 3.0]),
            connect=True,
            annotate=True,
            place=ub.Place.ABOVE,
            render=ub.Render.RASTER,
        )
    )
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    # The line under the bars and the markers over them are two images; the labels are text.
    assert len(_images(svg)) == 2
    assert ">2.5</text>" in svg


@pytest.mark.parametrize("method", ["scatter", "line", "tripcolor"])
@pytest.mark.parametrize("value", [ub.AUTO, "raster", 1, None])
def test_render_must_be_a_render(method: str, value: object) -> None:
    ax: Any = ub.Figure().add_axes()
    x, y = [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]
    args = (x, y, [1.0, 2.0, 3.0]) if method == "tripcolor" else (x, y)
    context = f"Axes.{method}()"
    with pytest.raises(TypeError) as caught:
        getattr(ax, method)(*args, render=value)
    if value is ub.AUTO:
        assert str(caught.value) == (
            f"{context}: render is ub.AUTO; render must be of type Render, as this mark has no "
            "automatic choice."
        )
    else:
        assert str(caught.value).startswith(f"{context}: render is ")
    assert details(caught.value) == ("render", "use ub.Render.VECTOR or ub.Render.RASTER.")
    assert ax.figure.to_svg() == ub.Figure().add_axes().figure.to_svg()


def test_raster_dpi_sets_the_size_of_the_image() -> None:
    fig = _scatter(ub.Render.RASTER, 300)
    widths: list[int] = []
    for dpi in (150, 300):
        image = _images(fig.to_svg(raster_dpi=dpi))[0]
        width = _number(r' width="([\d.]+)"', image)
        pixels = int(_number(r"/Width (\d+)", fig.to_pdf(raster_dpi=dpi).decode("latin-1")))
        assert abs(width / 72 * dpi - pixels) < 0.05
        widths.append(pixels)
    # Each edge is widened to whole pixels, so the sides differ from double by two at most.
    assert abs(widths[1] - 2 * widths[0]) <= 2


def test_raster_dpi_changes_nothing_without_a_raster_mark(tmp_path: Path) -> None:
    fig = _scatter(ub.Render.VECTOR, 300)
    svg, pdf = fig.to_svg(), fig.to_pdf()
    for dpi in (10, 72, 2400):
        assert fig.to_svg(raster_dpi=dpi) == svg
        assert fig.to_pdf(raster_dpi=dpi) == pdf
        assert ub.to_pdf([fig], raster_dpi=dpi) == pdf
    fig.save_svg(tmp_path / "chart.svg", raster_dpi=10)
    fig.save_pdf(tmp_path / "chart.pdf", raster_dpi=2400)
    assert (tmp_path / "chart.svg").read_text(encoding="utf-8") == svg
    assert (tmp_path / "chart.pdf").read_bytes() == pdf


def test_the_saved_files_hold_the_rendered_images(tmp_path: Path) -> None:
    fig = _scatter(ub.Render.RASTER, 300)
    fig.save_svg(tmp_path / "chart.svg", raster_dpi=120)
    fig.save_pdf(tmp_path / "chart.pdf", raster_dpi=120)
    assert (tmp_path / "chart.svg").read_text(encoding="utf-8") == fig.to_svg(raster_dpi=120)
    assert (tmp_path / "chart.pdf").read_bytes() == fig.to_pdf(raster_dpi=120)
    ub.save_pdf(tmp_path / "pages.pdf", [fig, _line(ub.Render.VECTOR), fig], raster_dpi=120)
    pages = (tmp_path / "pages.pdf").read_bytes()
    assert pages == ub.to_pdf([fig, _line(ub.Render.VECTOR), fig], raster_dpi=120)
    assert _pdf_images(pages) == 2
    assert pages.count(b"/MediaBox") == 3


# Each call that takes raster_dpi, by its error context; the value is passed unchecked.
CALLS: dict[str, Callable[[Any, Any], object]] = {
    "Figure.to_svg()": lambda fig, dpi: fig.to_svg(raster_dpi=dpi),
    "Figure.to_pdf()": lambda fig, dpi: fig.to_pdf(raster_dpi=dpi),
    "Figure.save_svg()": lambda fig, dpi: fig.save_svg("never.svg", raster_dpi=dpi),
    "Figure.save_pdf()": lambda fig, dpi: fig.save_pdf("never.pdf", raster_dpi=dpi),
    "to_pdf()": lambda fig, dpi: ub.to_pdf([fig], raster_dpi=dpi),
    "save_pdf()": lambda fig, dpi: ub.save_pdf("never.pdf", [fig], raster_dpi=dpi),
}


@pytest.mark.parametrize("context", list(CALLS))
@pytest.mark.parametrize("dpi", [9, 2401, 0, -300, 10**30])
def test_raster_dpi_out_of_range_is_a_style_error(
    context: str, dpi: int, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    fig = _scatter(ub.Render.RASTER, 10)
    with pytest.raises(ub.StyleError) as caught:
        CALLS[context](fig, dpi)
    shown = min(max(dpi, -(2**63)), 2**63 - 1)
    assert str(caught.value) == f"{context}: raster_dpi {shown} {RASTER_DPI_RULE}"
    assert details(caught.value) == ("raster_dpi", None)
    assert list(tmp_path.iterdir()) == []


@pytest.mark.parametrize("context", list(CALLS))
@pytest.mark.parametrize("dpi", [300.0, "300", True, None])
def test_raster_dpi_must_be_an_int(
    context: str, dpi: object, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    with pytest.raises(TypeError) as caught:
        CALLS[context](_scatter(ub.Render.RASTER, 10), dpi)
    shown = "None" if dpi is None else type(dpi).__name__
    assert str(caught.value) == f"{context}: raster_dpi is {shown}; raster_dpi must be an int."
    assert list(tmp_path.iterdir()) == []


def test_an_image_larger_than_a_png_may_be_is_a_style_error() -> None:
    fig = _scatter(ub.Render.RASTER, 10)
    with pytest.raises(ub.StyleError) as caught:
        fig.to_pdf(raster_dpi=2400)
    assert str(caught.value).startswith("Figure.to_pdf(): a mark drawn as an image would have ")
    assert str(caught.value).endswith(" pixels at raster_dpi 2400, over 67108864.")
    assert details(caught.value) == (
        "raster_dpi",
        "use a lower raster_dpi, a smaller figure, or render=ub.Render.VECTOR.",
    )
    with pytest.raises(ub.StyleError, match=r"^to_pdf\(\): figures\[1\]: a mark drawn"):
        ub.to_pdf([_scatter(ub.Render.VECTOR, 10), fig], raster_dpi=2400)


def test_a_color_mapped_scatter_keeps_its_colorbar_vector() -> None:
    x, y = _points(500)
    ax = ub.Figure().add_axes().scatter(x, y, color=ub.ColorBy(x * y), render=ub.Render.RASTER)
    svg = ax.colorbar(label="Product").figure.to_svg()
    assert len(_images(svg)) == 1
    assert "<linearGradient" in svg
