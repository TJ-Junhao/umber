"""Equal aspect: one data unit equally long on both axes, its refusals and its formats."""

import copy
from typing import Any
from xml.etree import ElementTree as ET

import numpy as np
import pytest

import umber as ub
from umber._errors import details

NS = "{http://www.w3.org/2000/svg}"
TWIN_HINT = "draw the second y scale on a panel of its own."


def circle(ax: ub.Axes[float, float]) -> ub.Axes[float, float]:
    angle = np.linspace(0, 2 * np.pi, 121)
    return ax.line(np.cos(angle), np.sin(angle))


def unit_ticks(ax: ub.Axes[float, float]) -> ub.Axes[float, float]:
    return ax.xticks_labeled([0, 1], ["x0", "x1"]).yticks_labeled([0, 1], ["y0", "y1"])


def position(fig: ub.Figure, name: str) -> tuple[float, float]:
    """The centre of a text's baseline in the editable SVG, in points."""
    root = ET.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE))
    node = next(n for n in root.iter(NS + "text") if "".join(n.itertext()) == name)
    x, y = float(node.attrib["x"]), float(node.attrib["y"])
    return x, y


def unit_lengths(fig: ub.Figure) -> tuple[float, float]:
    """The points per data unit on x and y, read from the labels of ``unit_ticks``."""
    x0, y0 = position(fig, "x0")[0], position(fig, "y0")[1]
    x1, y1 = position(fig, "x1")[0], position(fig, "y1")[1]
    return x1 - x0, y0 - y1


@pytest.mark.parametrize(
    ("width", "height", "limits"),
    [
        (9, 3, None),
        (3, 9, None),
        (9, 3, ((-2.0, 2.0), (-1.0, 1.0))),
        (3, 9, ((-1.0, 1.0), (-3.0, 3.0))),
    ],
)
def test_one_data_unit_is_equally_long_on_both_axes(
    width: float, height: float, limits: tuple[tuple[float, float], tuple[float, float]] | None
) -> None:
    ax = unit_ticks(circle(ub.Figure(width, height).add_axes())).equal_aspect()
    if limits is not None:
        ax.xlim(*limits[0]).ylim(*limits[1])
    x, y = unit_lengths(ax.figure)
    assert x == pytest.approx(y, rel=1e-3)


def test_returns_the_axes_and_a_second_call_changes_nothing() -> None:
    ax = circle(ub.Figure(9, 3).add_axes())
    assert ax.equal_aspect() is ax
    once = ax.figure.to_svg()
    ax.equal_aspect()
    assert ax.figure.to_svg() == once


def test_copies_keep_the_setting() -> None:
    fig = ub.Figure(9, 3)
    circle(fig.add_axes()).equal_aspect()
    assert copy.copy(fig).to_svg() == fig.to_svg()


def test_it_changes_the_picture_in_every_format() -> None:
    def build(equal: bool) -> ub.Figure:
        ax = circle(ub.Figure(9, 3).add_axes())
        return (ax.equal_aspect() if equal else ax).figure

    equal, plain = build(True), build(False)
    assert equal.to_svg() != plain.to_svg()
    assert equal.to_png() != plain.to_png()
    assert equal.to_pdf() != plain.to_pdf()


def test_a_panel_without_the_setting_keeps_its_place() -> None:
    def build(equal: bool) -> ub.Figure:
        fig = ub.Figure(12, 3, cols=2, legend=ub.Legend.PER_PANEL)
        left = fig.add_axes(col=0).xticks([-1, 1]).yticks([-1, 1]).title("Left")
        circle(left)
        right = circle(fig.add_axes(col=1).xticks([-1, 1]).yticks([-1, 1]))
        if equal:
            right.equal_aspect()
        return fig

    assert position(build(True), "Left") == position(build(False), "Left")


@pytest.mark.parametrize(
    ("scales", "needle"),
    [
        ({"x": ub.Scale.log()}, "x is a log axis"),
        ({"y": ub.Scale.log()}, "y is a log axis"),
        ({"x": ub.Scale.time()}, "x is a time axis"),
        ({"y": ub.Scale.time()}, "y is a time axis"),
        ({"x": ub.Scale.category()}, "x is a category axis"),
        ({"y": ub.Scale.category()}, "y is a category axis"),
    ],
)
def test_other_axes_are_refused_at_the_call(scales: dict[str, Any], needle: str) -> None:
    ax: Any = ub.Figure().add_axes(**scales)
    with pytest.raises(ub.UsageError, match=needle) as caught:
        ax.equal_aspect()
    assert str(caught.value).startswith("Axes.equal_aspect(): ")
    axis = needle[0]
    assert details(caught.value) == (None, f"pass {axis}=Scale.linear() to Figure.add_axes().")


def test_a_twin_is_refused_in_either_order() -> None:
    with pytest.raises(ub.UsageError, match="these axes are a twin") as caught:
        ub.Figure().add_axes().twin_y().equal_aspect()
    assert details(caught.value) == (None, TWIN_HINT)
    left = ub.Figure().add_axes()
    left.twin_y()
    with pytest.raises(ub.UsageError, match="these axes have a twin"):
        left.equal_aspect()
    with pytest.raises(ub.UsageError, match="these axes have equal aspect"):
        ub.Figure().add_axes().equal_aspect().twin_y()


def test_a_shared_axis_is_refused_at_the_call() -> None:
    fig = ub.Figure(cols=2, share_y=ub.Share.ROW)
    fig.add_axes(col=0)
    with pytest.raises(ub.UsageError, match="y is shared with another panel"):
        fig.add_axes(col=1).equal_aspect()


def test_a_panel_added_later_is_refused_when_rendering() -> None:
    fig = ub.Figure(cols=2, share_x=ub.Share.ALL)
    circle(fig.add_axes(col=0)).equal_aspect()
    circle(fig.add_axes(col=1))
    with pytest.raises(ub.UsageError, match="row 0, col 0: x is shared"):
        fig.to_svg()


def test_limits_that_leave_no_room_are_refused_when_rendering() -> None:
    ax = circle(ub.Figure().add_axes()).xlim(0, 1e6).ylim(0, 1).equal_aspect()
    with pytest.raises(ub.StyleError, match="equal aspect leaves the plot"):
        ax.figure.to_svg()


def test_in_plot_positions_are_fractions_of_the_smaller_plot() -> None:
    def build(equal: bool) -> ub.Figure:
        ax = unit_ticks(circle(ub.Figure(9, 3).add_axes()))
        ax.text("Note", at=ub.InPlot(1.0, 0.5), align=ub.Align.RIGHT)
        return (ax.equal_aspect() if equal else ax).figure

    assert position(build(True), "Note")[0] < position(build(False), "Note")[0]
