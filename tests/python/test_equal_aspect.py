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


def test_tick_labels_that_grow_keep_the_padding() -> None:
    def build(equal: bool) -> ub.Figure:
        fig = ub.Figure(12, 4, cols=2, legend=ub.Legend.PER_PANEL)
        left = fig.add_axes(col=0).bubbles([0, 20], [0, 9], [1, 1])
        if equal:
            left.equal_aspect()
        fig.add_axes(col=1).line([0, 1], [0, 1]).text("End", at=ub.InPlot(1, 0.5))
        return fig

    assert position(build(True), "2.5")[0] >= 12
    assert position(build(True), "End") == position(build(False), "End")


def test_bubbles_on_a_flat_domain_fit_at_the_figure_size() -> None:
    ax = unit_ticks(ub.Figure(6, 4).add_axes().bubbles([0, 40], [0, 1], [1, 1])).equal_aspect()
    x, y = unit_lengths(ax.figure)
    assert x == pytest.approx(y, rel=1e-3)


def test_a_flat_hexbin_gets_its_height_from_whole_cells() -> None:
    ax = ub.Figure(6, 4).add_axes().hexbin([0, 1e6], [0, 0], gridsize=30).equal_aspect()
    assert ax.figure.to_svg()


def right_of_hexbin(equal: bool) -> ub.Figure:
    fig = ub.Figure(6, 9, cols=2, legend=ub.Legend.PER_PANEL)
    left = fig.add_axes(col=0).hexbin([0, 0.6, 0.2], [0, 4, 8], gridsize=3)
    if equal:
        left.equal_aspect()
    fig.add_axes(col=1).line([0, 1], [0, 1]).text("End", at=ub.InPlot(1, 0.5), align=ub.Align.RIGHT)
    return fig


def test_a_hexbin_leaves_the_panel_beside_it() -> None:
    assert position(right_of_hexbin(True), "End") == position(right_of_hexbin(False), "End")


def test_a_direct_label_only_has_to_fit_the_final_plot() -> None:
    def build(equal: bool) -> ub.Figure:
        fig = ub.Figure(12, 3, cols=2, legend=ub.Legend.PER_PANEL)
        left = fig.add_axes(col=0).line([0, 1], [0, 1], label="W" * 14)
        left.legend(ub.LegendStyle.DIRECT)
        if equal:
            left.equal_aspect()
        fig.add_axes(col=1).line([0, 1], [0, 1])
        return fig

    with pytest.raises(ub.StyleError, match="direct label"):
        build(False).to_svg()
    assert build(True).to_svg()


def test_automatic_ticks_keep_the_side_that_does_not_shrink() -> None:
    def build(equal: bool) -> ub.Figure:
        ax = circle(ub.Figure(3, 9).add_axes()).text(
            "End", at=ub.InPlot(1, 0.5), align=ub.Align.RIGHT
        )
        return (ax.equal_aspect() if equal else ax).figure

    assert position(build(True), "End")[0] == position(build(False), "End")[0]


def test_a_tall_legend_is_not_told_to_make_the_figure_taller() -> None:
    def build(height: float) -> ub.Figure:
        ax = ub.Figure(9, height).add_axes().xlim(0, 10).ylim(0, 1)
        for i in range(8):
            ax.line([0, 1], [0, 1], label=f"Series {i}")
        ax.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT)
        return ax.equal_aspect().figure

    for height in [4, 12]:
        with pytest.raises(ub.StyleError, match="the legend is") as caught:
            build(height).to_svg()
        hint = details(caught.value)
        assert hint is not None
        assert "wider" in str(hint[1])
        assert "taller" not in str(hint[1])


def test_hexbins_that_cannot_be_counted_fail_beside_equal_aspect() -> None:
    def build(equal: bool) -> ub.Figure:
        fig = ub.Figure(12, 4, cols=2, legend=ub.Legend.PER_PANEL)
        left = fig.add_axes(col=0).line([0, 1], [0, 1])
        if equal:
            left.equal_aspect()
        fig.add_axes(col=1).hexbin([0, 1, 1], [0, 0.5, 1]).hexbin([0, 1e20], [0, 1])
        return fig

    for equal in [False, True]:
        with pytest.raises(ub.DataError, match="too far apart"):
            build(equal).to_svg()


def test_long_tick_labels_stay_on_the_canvas() -> None:
    long = "W" * 50
    ax = ub.Figure(6, 1.2).add_axes().line([0, 1], [0, 1]).xlim(0, 1).ylim(0, 1)
    ax.xticks_labeled([0, 1], [long, "end"]).yticks_labeled([0, 1], ["Y" * 40, "top"])
    ax.equal_aspect()
    root = ET.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    node = next(n for n in root.iter(NS + "text") if n.text == long)
    centre = float(node.attrib["x"])
    assert node.attrib.get("text-anchor", "start") == "start"
    assert centre >= 6


def test_padding_counts_towards_the_reach_of_bubbles() -> None:
    ax = ub.Figure(6, 1.16).add_axes().bubbles([0, 1], [0, 1], [1, 1]).equal_aspect()
    assert ax.figure.to_svg()


def test_a_label_with_no_clear_position_beside_equal_aspect_fails() -> None:
    def render(equal: bool) -> str:
        fig = ub.Figure(4, 1.6, cols=2, legend=ub.Legend.PER_PANEL)
        left = fig.add_axes(col=0).line([0, 1], [0, 1])
        if equal:
            left.equal_aspect()
        fig.add_axes(col=1).scatter(
            [0, 50, 50],
            [0, 8, 8],
            annotate=True,
            size=ub.Size.pt(6),
            label_size=ub.Size.pt(18),
            format=ub.TickFormat.fixed(3),
        )
        with pytest.raises(ub.StyleError) as caught:
            fig.to_svg()
        return str(caught.value)

    assert render(True) == render(False)
