"""``InPlot``: positions as fractions of the plot area, for ``Axes.text()`` and ``annotate()``."""

import copy
import pickle
from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"
Axes = ub.Axes[Any, Any]


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def position(ax: Axes, content: str) -> tuple[float, float]:
    """The baseline origin of the editable SVG text ``content`` in the figure of ``ax``."""
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    for node in root.iter(f"{SVG}text"):
        if node.text == content:
            return float(node.get("x", "nan")), float(node.get("y", "nan"))
    raise AssertionError(f"no text {content!r}")


def positions(ax: Axes, content: str) -> list[tuple[float, float]]:
    """The baseline origins of every editable SVG text ``content`` in the figure of ``ax``."""
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return [
        (float(node.get("x", "nan")), float(node.get("y", "nan")))
        for node in root.iter(f"{SVG}text")
        if node.text == content
    ]


def numeric() -> ub.Axes[float, float]:
    return ub.Figure(4, 4).add_axes().xlim(0, 10).ylim(0, 10).scatter([0, 10], [0, 10])


def test_value_semantics() -> None:
    a = ub.InPlot(0.02, 0.96)
    assert (a.x, a.y) == (0.02, 0.96)
    assert repr(a) == "InPlot(0.02, 0.96)"
    assert repr(ub.InPlot(1, 0)) == "InPlot(1, 0)"
    assert repr(ub.InPlot(-0.0, 0.5)) == "InPlot(0, 0.5)"
    assert a == ub.InPlot(0.02, 0.96)
    assert a != ub.InPlot(0.96, 0.02)
    assert ub.InPlot(1, 0) == ub.InPlot(1.0, 0.0)
    assert (a == (0.02, 0.96)) is False
    assert hash(a) == hash(ub.InPlot(0.02, 0.96))
    assert len({a, ub.InPlot(0.02, 0.96), ub.InPlot(0.5, 0.5)}) == 2
    assert copy.copy(a) is a
    assert copy.deepcopy(a) is a
    for protocol in range(pickle.HIGHEST_PROTOCOL + 1):
        restored: object = pickle.loads(pickle.dumps(a, protocol))
        assert restored == a


def test_a_position_is_immutable() -> None:
    a = ub.InPlot(0.5, 0.5)
    with pytest.raises(AttributeError):
        call(a.__setattr__)("x", 0.1)
    with pytest.raises(AttributeError):
        call(a.__delattr__)("x")


@pytest.mark.parametrize(
    ("x", "y", "parameter", "shown"),
    [
        (-0.1, 0.5, "x", "-0.1"),
        (1.5, 0.5, "x", "1.5"),
        (float("nan"), 0.5, "x", "NaN"),
        (0.5, float("inf"), "y", "inf"),
        (0.5, -1, "y", "-1"),
        (2, 2, "x", "2"),
    ],
)
def test_fractions_outside_the_plot_name_their_coordinate(
    x: float, y: float, parameter: str, shown: str
) -> None:
    with pytest.raises(ub.DataError) as caught:
        ub.InPlot(x, y)
    assert str(caught.value) == (
        f"InPlot(): {parameter} is {shown}; a position in the plot is a fraction from 0 to 1."
    )
    info = details(caught.value)
    assert info is not None and info[0] == parameter and info[1]


@pytest.mark.parametrize("bad", ["0.5", None, True, np.True_, [0.5], 1j])
def test_non_numbers_and_bools_are_refused(bad: object) -> None:
    for arguments in [(bad, 0.5), (0.5, bad)]:
        with pytest.raises(TypeError) as caught:
            call(ub.InPlot)(*arguments)
        info = details(caught.value)
        assert info is not None and info[0] == ("x" if arguments[0] is bad else "y")


def test_numpy_numbers_are_accepted() -> None:
    assert call(ub.InPlot)(np.float32(0.5), np.int64(1)) == ub.InPlot(0.5, 1)


@pytest.mark.parametrize("align", list(ub.Align))
@pytest.mark.parametrize("fractions", [(0.0, 0.0), (1.0, 1.0), (0.5, 0.5), (0.25, 0.75)])
def test_text_is_placed_by_fractions_of_the_plot(
    align: ub.Align, fractions: tuple[float, float]
) -> None:
    # Points at the plot's corners put its area at the data range 0 to 10 in both directions,
    # so a data position of the same place shows where the plot is.
    by_fraction = numeric().text("n", ub.InPlot(*fractions), align=align)
    data = (fractions[0] * 10, fractions[1] * 10)
    by_data = numeric().text("n", data, align=align)
    x, y = position(by_fraction, "n")
    expected = position(by_data, "n")
    assert x == pytest.approx(expected[0], abs=0.05)
    assert y == pytest.approx(expected[1], abs=0.05)


def test_text_in_the_plot_keeps_line_breaks_and_size() -> None:
    ax = numeric().text("one\ntwo", ub.InPlot(0.5, 0.5), size=ub.Size.pt(10))
    first, second = position(ax, "one"), position(ax, "two")
    # Left-aligned lines start at the same x; each further line is 1.2 sizes below.
    assert first[0] == pytest.approx(second[0], abs=0.05)
    assert second[1] - first[1] == pytest.approx(12.0, abs=0.05)
    single = numeric().text("one", ub.InPlot(0.5, 0.5), size=ub.Size.pt(10))
    assert position(single, "one") == pytest.approx(first, abs=0.05)


def test_text_in_the_plot_works_on_every_kind_of_axis() -> None:
    day = np.array(["2026-01-01", "2026-01-10"], dtype="datetime64[D]")
    figures: list[Axes] = [
        ub.Figure(4, 4).add_axes(x=ub.Scale.log(), y=ub.Scale.log()).scatter([1, 100], [1, 1000]),
        ub.Figure(4, 4).add_axes(x=ub.Scale.category()).bar(["a", "b"], [1, 2]),
        ub.Figure(4, 4).add_axes(x=ub.Scale.time()).scatter(day, [1, 2]),
    ]
    for ax in figures:
        ax.text("mid", ub.InPlot(0.5, 0.5), align=ub.Align.CENTER)
        x, y = position(ax, "mid")
        # Near the middle of the 288 pt square, whatever the scales.
        assert abs(x - 144) < 60 and abs(y - 144) < 60
        assert ax.figure.to_png()


def test_text_in_the_plot_works_on_twin_axes() -> None:
    left = numeric()
    right = left.twin_y().line([0, 10], [100, 900])
    left.text("n", ub.InPlot(0.3, 0.6))
    right.text("n", ub.InPlot(0.3, 0.6))
    first, second = positions(left, "n")
    assert first == pytest.approx(second, abs=0.01)


def test_text_in_the_plot_changes_no_limits() -> None:
    plain = ub.Figure().add_axes().scatter([1, 2, 3], [4, 9, 2])
    noted = ub.Figure().add_axes().scatter([1, 2, 3], [4, 9, 2])
    noted.text("n = 3", ub.InPlot(0.02, 0.96))
    before, after = plain.figure.to_svg(), noted.figure.to_svg()
    assert after.startswith(before[: before.index("</svg>")])


def test_text_in_the_plot_must_stay_on_the_figure() -> None:
    ax = numeric().text("a rather long note", ub.InPlot(1, 1))
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert details(caught.value) == (None, "change align or at.")
    assert "past the right edge of the figure." in str(caught.value)
    captioned = numeric().text("a\nb\nc\nd\ne\nf", ub.InPlot(0.5, 0.0), align=ub.Align.CENTER)
    captioned.figure.caption("Caption")
    with pytest.raises(ub.StyleError):
        captioned.figure.to_svg()


def test_at_still_refuses_other_values() -> None:
    ax = numeric()
    for bad in [(0.5,), [0.5, 0.5], "ab", (ub.InPlot(0.1, 0.1), 1)]:
        with pytest.raises(TypeError) as caught:
            call(ax.text)("x", bad)
        info = details(caught.value)
        assert info is not None and info[0] == "at"
    with pytest.raises(TypeError):
        call(ax.annotate)("x", ub.InPlot(0.5, 0.5))


def test_annotate_text_at_with_and_without_an_arrow() -> None:
    with_arrow = numeric().annotate("n", (2, 2), text_at=ub.InPlot(0.8, 0.85))
    without = numeric().annotate("n", (2, 2), text_at=ub.InPlot(0.8, 0.85), arrow=False)
    assert with_arrow.figure.to_svg().count("<path") > without.figure.to_svg().count("<path")
    plain = numeric().text("n", ub.InPlot(0.8, 0.85), align=ub.Align.CENTER)
    for ax in (with_arrow, without):
        x, y = position(ax, "n")
        assert (x, y) == pytest.approx(position(plain, "n"), abs=0.01)


def test_annotate_text_at_keeps_the_rules_of_its_point() -> None:
    ax = numeric().annotate("n", (11, 5), text_at=ub.InPlot(0.8, 0.85))
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert str(caught.value) == 'Figure.to_svg(): the annotation "n" lies outside the x limits.'
    close = numeric().annotate("n", (5, 5), text_at=ub.InPlot(0.5, 0.5))
    with pytest.raises(ub.StyleError) as caught:
        close.figure.to_svg()
    assert details(caught.value) == (None, "move text_at away from the point, or pass arrow=False.")


def test_text_at_with_an_offset_is_a_usage_error() -> None:
    ax = numeric().annotate("first", (5, 5))
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.annotate("n", (2, 2), offset=(24, 18), text_at=ub.InPlot(0.8, 0.85))
    assert str(caught.value) == (
        "Axes.annotate(): text_at and offset (24, 18) both place the text, in different ways."
    )
    info = details(caught.value)
    assert info is not None and info[0] == "text_at" and info[1]
    assert ax.figure.to_svg() == before
    # The default offset given explicitly is no second placement.
    ax.annotate("n", (2, 2), offset=(0, 12), text_at=ub.InPlot(0.8, 0.85))


def test_text_at_must_be_auto_or_an_in_plot_position() -> None:
    ax = numeric()
    for bad in [(0.5, 0.5), 0.5, "x", None]:
        with pytest.raises(TypeError) as caught:
            call(ax.annotate)("n", (2, 2), text_at=bad)
        info = details(caught.value)
        assert info is not None and info[0] == "text_at"
    assert ax.annotate("n", (2, 2), text_at=ub.AUTO) is ax


def test_png_svg_and_pdf_draw_text_in_the_plot() -> None:
    ax = numeric().text("n = 5 folds", ub.InPlot(0.02, 0.96))
    ax.annotate("best", (5, 5), text_at=ub.InPlot(0.9, 0.2))
    figure = ax.figure
    assert ">n = 5 folds<" in figure.to_svg(text=ub.SvgText.EDITABLE)
    assert "n = 5 folds" not in figure.to_svg(text=ub.SvgText.OUTLINES)
    plain = numeric().figure
    assert figure.to_png() != plain.to_png()
    assert figure.to_pdf() != plain.to_pdf()
