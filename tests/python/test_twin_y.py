"""Numeric right axes share x while retaining their own y coordinates."""

import copy
from datetime import datetime
from typing import Any
from xml.etree import ElementTree as ET

import pytest

import umber as ub
from umber._errors import details

HINT = "twin the original axes, or stop sharing x."
NS = "{http://www.w3.org/2000/svg}"


def texts(fig: ub.Figure) -> list[ET.Element]:
    return list(ET.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE)).iter(NS + "text"))


def named(fig: ub.Figure, name: str) -> ET.Element:
    return next(node for node in texts(fig) if "".join(node.itertext()) == name)


def pair() -> tuple[ub.Figure, ub.Axes[float, float], ub.Axes[float, float]]:
    fig = ub.Figure(8, 5)
    left = fig.add_axes().line([0, 1, 2], [1, 2, 3], label="Left")
    right = left.twin_y().line([0, 1, 2], [100, 200, 300], label="Right")
    return fig, left, right


def test_twin_shares_x_exactly() -> None:
    fig, left, right = pair()
    assert right.figure is left.figure is fig
    right.xlim(-1, 4).xticks_labeled([0, 2], ["start", "end"])
    expected = fig.to_svg()
    left.xlim(-1, 4).xticks_labeled([0, 2], ["start", "end"])
    assert fig.to_svg() == expected
    left.xlim(-2, 5)
    changed = fig.to_svg()
    assert changed != expected
    right.xlim(-2, 5)
    assert fig.to_svg() == changed
    assert ["".join(node.itertext()) for node in texts(fig)].count("start") == 1


def test_twin_limits_independent() -> None:
    fig, left, right = pair()
    left.ylim(0, 4).yticks_labeled([0, 4], ["left low", "left high"])
    right.ylim(0, 400).yticks_labeled([0, 400], ["right low", "right high"])
    left.text("left point", (1, 2))
    right.text("right point", (1, 200))
    assert float(named(fig, "left point").attrib["y"]) == pytest.approx(
        float(named(fig, "right point").attrib["y"])
    )
    right.ylim(0, 800)
    assert float(named(fig, "left point").attrib["y"]) < float(
        named(fig, "right point").attrib["y"]
    )


def test_palette_continues_on_twin() -> None:
    fig, _, _ = pair()
    root = ET.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE))
    strokes = [node.attrib["stroke"] for node in root.iter(NS + "path") if "stroke" in node.attrib]
    assert len(set(strokes)) >= 2
    assert named(fig, "Left") is not named(fig, "Right")


@pytest.mark.parametrize("style", list(ub.LegendStyle))
@pytest.mark.parametrize("position", list(ub.LegendPosition))
def test_one_legend_for_both(style: ub.LegendStyle, position: ub.LegendPosition) -> None:
    fig, left, right = pair()
    where = ub.AUTO if style is ub.LegendStyle.DIRECT else position
    right.legend(style, position=where)
    names = ["".join(node.itertext()) for node in texts(fig)]
    assert names.count("Left") == names.count("Right") == (style is not ub.LegendStyle.NONE)
    before = fig.to_svg()
    left.legend(style, position=where)
    assert fig.to_svg() == before


def test_twin_of_twin_rejected() -> None:
    fig, left, right = pair()
    before = fig.to_svg()
    for axes, hint in (
        (right, HINT),
        (left, "use the twin returned by the first twin_y(), or Figure.twin_axes_at()."),
    ):
        with pytest.raises(ub.UsageError) as caught:
            axes.twin_y()
        assert details(caught.value) == (None, hint)
        assert "twin_y()" in str(caught.value)
        assert fig.to_svg() == before


@pytest.mark.parametrize("share", [ub.Share.ROW, ub.Share.COL, ub.Share.ALL])
def test_twin_on_shared_x_rejected(share: ub.Share) -> None:
    fig = ub.Figure(rows=2, cols=2, share_x=share)
    axes = fig.add_axes(row=0, col=0)
    before = fig.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        axes.twin_y()
    assert details(caught.value) == (None, HINT)
    assert fig.to_svg() == before


def test_twin_on_pie_rejected() -> None:
    axes: Any = ub.Figure().add_pie_axes()
    with pytest.raises(ub.UsageError) as caught:
        axes.twin_y()
    assert "this panel has no Cartesian axes" in str(caught.value)
    assert details(caught.value) == (
        None,
        "use Figure.add_axes() for Cartesian marks and axis settings.",
    )


@pytest.mark.parametrize("scale", [ub.Scale.category(), ub.Scale.time(), None, "linear"])
def test_invalid_scale_is_atomic(scale: Any) -> None:
    fig = ub.Figure()
    axes = fig.add_axes()
    before = fig.to_svg()
    with pytest.raises((TypeError, ub.UsageError)) as caught:
        axes.twin_y(scale=scale)
    detail = details(caught.value)
    assert detail is not None and detail[0] == "scale"
    assert fig.to_svg() == before
    axes.twin_y()


def test_twin_requires_numeric_parent_y() -> None:
    axes: Any = ub.Figure().add_axes(y=ub.Scale.category())
    with pytest.raises(ub.UsageError):
        axes.twin_y()


def test_right_labels_title_and_text_sizes() -> None:
    fig, left, right = pair()
    left.ylabel("Left title").yticks_labeled([1, 3], ["L1", "L3"])
    right.ylabel("Right title", size=ub.Size.pt(16))
    right.yticks_labeled([100, 300], ["R100", "R300"]).ytick_labels(size=ub.Size.pt(13))
    assert named(fig, "Right title").attrib["font-size"] == "16"
    assert named(fig, "R100").attrib["font-size"] == "13"
    assert float(named(fig, "R100").attrib["x"]) == float(named(fig, "R300").attrib["x"])
    assert float(named(fig, "R100").attrib["x"]) > float(named(fig, "L1").attrib["x"])
    assert float(named(fig, "Right title").attrib["y"]) < float(named(fig, "R300").attrib["y"])


def test_twin_category_bars_annotations_and_copy() -> None:
    fig = ub.Figure(8, 5)
    left = fig.add_axes(x=ub.Scale.category()).bar(["A", "B"], [1, 2], label="Count")
    right = left.twin_y()
    right.bar(["A", "B"], [100, 200], annotate=True, label="Value")
    right.text("At B", ("B", 150)).annotate("At A", ("A", 100))
    right.ylabel("Value")
    assert {"100", "200", "At A", "At B", "Value"} <= {
        "".join(node.itertext()) for node in texts(fig)
    }
    assert copy.copy(fig).to_svg() == fig.to_svg()


def test_twin_time_x_and_log_y() -> None:
    times = [datetime(2026, 1, 1), datetime(2026, 1, 2)]
    fig = ub.Figure(8, 5)
    left = fig.add_axes(x=ub.Scale.time()).line(times, [1, 2])
    right = left.twin_y(scale=ub.Scale.log()).line(times, [10, 1000])
    right.xlim(times[0], times[1]).ylim(1, 10000)
    right.text("Time point", (times[0], 100)).annotate(
        "Log point", (times[1], 1000), offset=(-12, 12)
    )
    assert named(fig, "Time point") is not None
    assert fig.to_png().startswith(b"\x89PNG")


def test_twin_in_grid_keeps_plots_aligned() -> None:
    fig = ub.Figure(rows=2, cols=2)
    for cell in fig.cells():
        left = cell.add_axes().line([0, 1], [0, 1])
        left.xlim(0, 1).ylim(0, 1)
        left.text("origin", (0, 0))
    upper = fig.axes_at(row=0, col=0)
    upper.twin_y().line([0, 1], [0, 100000]).ylabel("Right axis")
    origins = [node for node in texts(fig) if "".join(node.itertext()) == "origin"]
    assert len(origins) == 4
    assert origins[0].attrib["x"] == origins[2].attrib["x"]
    assert origins[1].attrib["x"] == origins[3].attrib["x"]
    assert origins[0].attrib["y"] == origins[1].attrib["y"]
    assert origins[2].attrib["y"] == origins[3].attrib["y"]


@pytest.mark.parametrize(
    "method, arguments",
    [
        ("line", ([0, 1, 2], [10, 20, 15])),
        ("scatter", ([0, 1, 2], [10, 20, 15])),
        ("bubbles", ([0, 1, 2], [10, 20, 15], [1, 2, 3])),
        ("stem", ([0, 1, 2], [10, 20, 15])),
        ("area", ([0, 1, 2], [10, 20, 15])),
        ("step", ([0, 1, 2], [10, 20, 15])),
        ("band", ([0, 1, 2], [8, 18, 13], [12, 22, 17])),
        ("hline", (15,)),
        ("vline", (1,)),
        ("hspan", (10, 20)),
        ("vspan", (0.5, 1.5)),
        ("hist", ([0, 0.5, 1, 1.5, 2],)),
        ("ecdf", ([0, 0.5, 1, 1.5, 2],)),
        ("kde", ([0, 0.5, 1, 1.5, 2],)),
        ("mesh", ([0, 1, 2], [10, 20, 30], [[1, 2], [3, 4]])),
        ("hist2d", ([0, 0.5, 1, 1.5, 2], [10, 20, 15, 25, 30])),
        ("hexbin", ([0, 0.5, 1, 1.5, 2], [10, 20, 15, 25, 30])),
        ("contour", ([0, 1, 2], [10, 20, 30], [[1, 2, 3], [2, 4, 2], [3, 2, 1]])),
    ],
)
def test_twin_numeric_marks(method: str, arguments: tuple[object, ...]) -> None:
    fig = ub.Figure(10, 6)
    left = fig.add_axes().line([0, 1, 2], [1, 2, 3]).ylabel("Left")
    right = left.twin_y().ylabel("Right")
    assert getattr(right, method)(*arguments) is right
    assert named(fig, "Right") is not None
    assert fig.to_png().startswith(b"\x89PNG")


@pytest.mark.parametrize("method", ["bar", "box", "violin", "strip", "swarm"])
def test_twin_category_marks(method: str) -> None:
    fig = ub.Figure(8, 5)
    left = fig.add_axes(x=ub.Scale.category()).line(["A", "B"], [1, 2])
    right = left.twin_y()
    values = [100, 200] if method == "bar" else [[90, 100, 110], [190, 200, 210]]
    assert getattr(right, method)(["A", "B"], values) is right
    assert fig.to_png().startswith(b"\x89PNG")


@pytest.mark.parametrize("position", [ub.ColorbarPosition.RIGHT, ub.ColorbarPosition.BOTTOM])
def test_twin_colorbars_and_limits_are_independent(position: ub.ColorbarPosition) -> None:
    fig = ub.Figure(10, 6)
    left = fig.add_axes().scatter([0, 1], [1, 2], color=ub.ColorBy([0, 10]))
    right = left.twin_y().scatter([0, 1], [100, 200], color=ub.ColorBy([100, 200], ub.Cmap.MAGMA))
    left.clim(0, 10).colorbar("Left colors", position=position)
    right.clim(100, 200).colorbar("Right colors", position=position)
    a = named(fig, "Left colors")
    b = named(fig, "Right colors")
    coordinate = "x" if position is ub.ColorbarPosition.RIGHT else "y"
    assert float(a.attrib[coordinate]) < float(b.attrib[coordinate])
    assert copy.copy(fig).to_svg() == fig.to_svg()


def test_twin_x_settings_and_category_references() -> None:
    fig = ub.Figure(8, 5)
    left = fig.add_axes(x=ub.Scale.category()).line(["A", "B"], [1, 2])
    right = left.twin_y().line(["B", "C"], [100, 200])
    right.vline("A")
    left.vline("C")
    right.xtick_labels(size=ub.Size.pt(17))
    assert named(fig, "A").attrib["font-size"] == "17"
    assert named(fig, "C").attrib["font-size"] == "17"


def test_twin_grouped_marks_and_value_labels() -> None:
    fig = ub.Figure(10, 6)
    left = fig.add_axes(x=ub.Scale.category()).bar(["A", "B"], [1, 2])
    right = left.twin_y()
    series = [ub.Series("First", [100, 200]), ub.Series("Second", [150, 250])]
    right.grouped_bar(["A", "B"], series, annotate=True)
    right.stacked_bar(["C", "D"], series, annotate=True)
    assert {"100", "200", "150", "250"} <= {"".join(node.itertext()) for node in texts(fig)}


def test_twin_clone_mutation_does_not_change_original() -> None:
    fig, _, _ = pair()
    before = fig.to_svg()
    copied = copy.copy(fig)
    copied.axes_at().xlim(-10, 10)
    assert copied.to_svg() != before
    assert fig.to_svg() == before


def test_left_y_sharing_excludes_twin() -> None:
    fig = ub.Figure(10, 5, rows=1, cols=2, share_y=ub.Share.ALL)
    left = fig.add_axes(row=0, col=0).line([0, 1], [1, 2]).ylim(0, 4)
    other = fig.add_axes(row=0, col=1).line([0, 1], [1, 2])
    right = left.twin_y().line([0, 1], [100, 200]).ylim(0, 400)
    left.text("left middle", (0.5, 2))
    other.text("other middle", (0.5, 2))
    right.text("right middle", (0.5, 200))
    points = [
        named(fig, label).attrib["y"] for label in ("left middle", "other middle", "right middle")
    ]
    assert len(set(points)) == 1


@pytest.mark.parametrize("cols", [1, 2])
@pytest.mark.parametrize("theme", [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT])
def test_overlapping_y_titles_raise(cols: int, theme: ub.Theme) -> None:
    fig = ub.Figure(3 * cols, 3, cols=cols, theme=theme)
    left = fig.add_axes(col=0).line([0, 1], [1, 2])
    left.ylabel("Left axis title with many units")
    left.twin_y().line([0, 1], [100, 200]).ylabel("Right axis title with many units")
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    assert "Left axis title with many units" in str(caught.value)
    assert "Right axis title with many units" in str(caught.value)
    assert details(caught.value) == (None, "shorten a y axis title or widen the figure.")
    if cols > 1:
        assert "row 0, col 0" in str(caught.value)


@pytest.mark.parametrize("clone", [copy.copy, copy.deepcopy])
def test_twin_axes_at_reaches_copied_spanning_panel(clone: Any) -> None:
    fig = ub.Figure(12, 8, rows=2, cols=3)
    left = fig.add_axes(row=1, col=1, col_span=2, x=ub.Scale.category())
    left.bar(["A", "B"], [1, 2])
    right = left.twin_y(scale=ub.Scale.log(base=2)).line(["A", "B"], [8, 32])
    assert fig.twin_axes_at(row=1, col=1, x=ub.Scale.category(), y=ub.Scale.log(base=2)) is right
    before = fig.to_svg()
    copied = clone(fig)
    twin = copied.twin_axes_at(row=1, col=1, x=ub.Scale.category(), y=ub.Scale.log(base=2))
    assert twin.figure is copied
    assert copied.to_svg() == before
    twin.ylim(1, 64).ylabel("Changed")
    assert copied.to_svg() != before
    assert fig.to_svg() == before
    with pytest.raises(ub.UsageError):
        copied.twin_axes_at(row=1, col=2, x=ub.Scale.category(), y=ub.Scale.log(base=2))


def test_twin_axes_at_checks_scales_and_presence() -> None:
    fig = ub.Figure()
    left = fig.add_axes()
    with pytest.raises(ub.UsageError, match="no right-hand y axis"):
        fig.twin_axes_at()
    right = left.twin_y(scale=ub.Scale.log(base=2))
    assert fig.twin_axes_at(y=ub.Scale.log(base=2)) is right
    cases: list[tuple[ub.Scale[Any], ub.Scale[float], str]] = [
        (ub.Scale.category(), ub.Scale.log(base=2), "x"),
        (ub.Scale.linear(), ub.Scale.log(), "y"),
    ]
    for x, y, parameter in cases:
        with pytest.raises(ub.UsageError) as caught:
            fig.twin_axes_at(x=x, y=y)
        assert "Figure.twin_axes_at()" in str(caught.value)
        detail = details(caught.value)
        assert detail is not None and detail[0] == parameter
    with pytest.raises(ub.UsageError) as caught:
        fig.axes_at(y=ub.Scale.log(base=2))
    detail = details(caught.value)
    assert detail is not None and detail[1] is not None and "twin_axes_at" in detail[1]


@pytest.mark.parametrize("parameter", ["x", "y", "row", "col"])
@pytest.mark.parametrize("value", [None, "linear", True])
def test_twin_axes_at_rejects_invalid_arguments(parameter: str, value: Any) -> None:
    fig, _, _ = pair()
    before = fig.to_svg()
    call: Any = fig.twin_axes_at
    with pytest.raises(TypeError) as caught:
        call(**{parameter: value})
    detail = details(caught.value)
    assert detail is not None and detail[0] == parameter
    assert fig.to_svg() == before


def test_native_twin_handle_identifies_its_column() -> None:
    fig: Any = ub.Figure(12, 5, cols=2)
    left = fig.add_axes(col=0).line([0, 1], [1, 2])
    right = left.twin_y().line([0, 1], [100, 200])
    fig.add_axes(col=1).line([0, 1], [3, 4]).twin_y().line([0, 1], [30, 40])
    handle = right._target
    fig._native.ylim(handle, 1, 0, 400)
    expected = fig.to_svg()
    right.ylim(0, 400)
    assert fig.to_svg() == expected
    assert (handle.row, handle.col) == (0, 0)


@pytest.mark.parametrize("theme", [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT])
def test_right_colorbars_precede_right_legend(theme: ub.Theme) -> None:
    fig = ub.Figure(12, 6, theme=theme)
    left = fig.add_axes().scatter([0, 1], [1, 2], color=ub.ColorBy([0, 10]))
    left.line([0, 1], [1, 2], label="Left series")
    right = left.twin_y().scatter([0, 1], [100, 200], color=ub.ColorBy([100, 200]))
    right.line([0, 1], [100, 200], label="Right series")
    left.colorbar("Left colors", position=ub.ColorbarPosition.RIGHT)
    right.colorbar("Right colors", position=ub.ColorbarPosition.RIGHT)
    right.yticks_labeled([100, 200], ["R100", "R200"])
    left.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT)
    xs = [
        float(named(fig, label).attrib["x"])
        for label in ("R100", "Left colors", "Right colors", "Left series")
    ]
    assert xs == sorted(xs)


@pytest.mark.parametrize("theme", [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT])
def test_twin_never_draws_a_second_baseline(theme: ub.Theme) -> None:
    fig = ub.Figure(8, 5, theme=theme)
    left = fig.add_axes(x=ub.Scale.category()).bar(["A", "B"], [2, 3])
    right = left.twin_y(scale=ub.Scale.log()).bar(["A", "B"], [10, 100])
    before = fig.to_svg()
    right.baseline(shown=False)
    assert fig.to_svg() == before
    right.baseline(shown=True)
    assert fig.to_svg() == before
