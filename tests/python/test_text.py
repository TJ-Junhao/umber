"""``Axes.text()``: positions typed by the axes, alignment, size, color and errors."""

from collections.abc import Callable
from datetime import date, datetime
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def texts(ax: ub.Axes[Any, Any]) -> dict[str, ElementTree.Element]:
    """Every editable SVG text of the figure of ``ax``, by its content."""
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return {node.text or "": node for node in root.iter(f"{SVG}text")}


def numeric() -> ub.Axes[float, float]:
    return ub.Figure(6, 4).add_axes().xlim(0, 10).ylim(0, 10)


def test_text_returns_the_axes_and_writes_the_text() -> None:
    ax = numeric()
    assert ax.text("Peak", (2, 7.5)) is ax
    assert "Peak" in texts(ax)
    theme = ub.Theme.PIGMENT.with_text_color(ub.Color.hex("#123456"))
    themed = ub.Figure(theme=theme).add_axes().xlim(0, 10).ylim(0, 10).text("Peak", (2, 7.5))
    assert texts(themed)["Peak"].get("fill") == "#123456"


def test_text_align_edges() -> None:
    xs = {}
    for align in ub.Align:
        ax = numeric().text("edge", (5, 5), align=align)
        xs[align] = float(texts(ax)["edge"].get("x", "nan"))
    assert xs[ub.Align.LEFT] > xs[ub.Align.CENTER] > xs[ub.Align.RIGHT]
    assert xs[ub.Align.LEFT] - xs[ub.Align.CENTER] == pytest.approx(
        xs[ub.Align.CENTER] - xs[ub.Align.RIGHT], abs=1e-2
    )


def test_text_size_and_color() -> None:
    ax = numeric().text("big", (5, 5), size=ub.Size.pt(15), color=ub.Color.RED)
    node = texts(ax)["big"]
    assert node.get("fill") == "#D64933"
    assert "15" in (node.get("font-size") or "")


def test_text_on_category_and_time_axes() -> None:
    bars = ub.Figure().add_axes(x=ub.Scale.category()).bar(["Jan", "Feb", "Mar"], [24, 31, 42])
    assert bars.text("March", ("Mar", 42.0)) is bars
    assert "March" in texts(bars)
    days = [date(2026, 3, 1), date(2026, 3, 4), date(2026, 3, 9)]
    timed = ub.Figure().add_axes(x=ub.Scale.time()).line(days, [1, 3, 2])
    expected = timed.text("launch", (date(2026, 3, 4), 3.0)).figure.to_svg()
    whens: list[ub.Time] = [datetime(2026, 3, 4), np.datetime64("2026-03-04")]
    for when in whens:
        other = ub.Figure().add_axes(x=ub.Scale.time()).line(days, [1, 3, 2])
        assert other.text("launch", (when, 3.0)).figure.to_svg() == expected


@pytest.mark.parametrize("at", [[1, 2], (1,), (1, 2, 3), "12", 3.0, None])
def test_at_must_be_a_pair(at: object) -> None:
    ax = numeric()
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        call(ax.text)("x", at)
    assert str(caught.value).startswith("Axes.text(): at is ")
    assert str(caught.value).endswith("; at must be a pair (x, y).")
    info = details(caught.value)
    assert info is not None and info[0] == "at" and info[1]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("axes", "at"),
    [
        (lambda: numeric(), ("a", 1.0)),
        (lambda: numeric(), (1.0, "b")),
        (lambda: ub.Figure().add_axes(x=ub.Scale.category()), (1.0, 1.0)),
        (lambda: ub.Figure().add_axes(x=ub.Scale.time()), (1.0, 1.0)),
    ],
)
def test_coordinates_follow_the_axis_kinds(
    axes: Callable[[], ub.Axes[Any, Any]], at: tuple[object, object]
) -> None:
    ax = axes()
    with pytest.raises(TypeError) as caught:
        call(ax.text)("x", at)
    info = details(caught.value)
    assert info is not None and info[0] == "at" and info[1]


@pytest.mark.parametrize(
    ("at", "kind"),
    [((np.nan, 1.0), ub.DataError), ((1.0, np.inf), ub.DataError)],
)
def test_coordinates_must_be_finite(at: tuple[float, float], kind: type[Exception]) -> None:
    ax = numeric()
    with pytest.raises(kind) as caught:
        ax.text("x", at)
    info = details(caught.value)
    assert info is not None and info[0] == "at"


def test_wrong_option_types_are_refused() -> None:
    ax = numeric()
    for keyword, value in [("align", "left"), ("size", 12), ("color", "red")]:
        with pytest.raises(TypeError) as caught:
            call(ax.text)("x", (1, 1), **{keyword: value})
        info = details(caught.value)
        assert info is not None and info[0] == keyword
    with pytest.raises(TypeError) as caught:
        call(ax.text)(3, (1, 1))
    info = details(caught.value)
    assert info is not None and info[0] == "text"
    with pytest.raises(ub.UsageError) as empty:
        ax.text("", (1, 1))
    info = details(empty.value)
    assert info is not None and info[0] == "text"


def test_point_outside_limits_rejected() -> None:
    ax = numeric().text("far", (11, 5))
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert str(caught.value) == 'Figure.to_svg(): the text "far" lies outside the x limits.'
    assert details(caught.value) == (None, "set xlim/ylim to include it, or move the point.")


def test_text_outside_the_figure_rejected() -> None:
    ax = numeric().text("a long line of text that runs off the figure", (9.5, 5))
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_png()
    assert "past the right edge of the figure" in str(caught.value)
    assert details(caught.value) == (None, "change align or offset.")


def test_unknown_category_is_refused_at_render() -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category()).bar(["A"], [3]).text("x", ("Z", 1.0))
    with pytest.raises(ub.DataError) as caught:
        ax.figure.to_svg()
    assert "no mark on these axes has that category" in str(caught.value)


def test_text_does_not_change_limits() -> None:
    plain = ub.Figure().add_axes().scatter([1, 2, 3], [4, 9, 2])
    annotated = ub.Figure().add_axes().scatter([1, 2, 3], [4, 9, 2]).text("top", (2, 9))
    before, after = texts(plain), texts(annotated)
    assert set(before) | {"top"} == set(after)
    for name, node in before.items():
        assert (node.get("x"), node.get("y")) == (after[name].get("x"), after[name].get("y"))
