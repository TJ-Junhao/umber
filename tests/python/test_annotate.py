"""``Axes.annotate()``: the side of the text follows the offset, the arrow, and errors."""

from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def position(ax: ub.Axes[Any, Any], content: str) -> tuple[float, float]:
    """The baseline origin of the editable SVG text ``content`` in the figure of ``ax``."""
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    for node in root.iter(f"{SVG}text"):
        if node.text == content:
            return float(node.get("x", "nan")), float(node.get("y", "nan"))
    raise AssertionError(f"no text {content!r}")


def numeric() -> ub.Axes[float, float]:
    return ub.Figure(4, 4).add_axes().xlim(0, 1).ylim(0, 1)


def test_annotate_returns_the_axes() -> None:
    ax = numeric()
    assert ax.annotate("Peak", (0.5, 0.5)) is ax
    assert ax.annotate("Dip", (0.5, 0.5), offset=(18.0, -22.0), arrow=False) is ax
    assert ax.figure.to_png()


def test_annotate_side_follows_offset() -> None:
    right = position(numeric().annotate("n", (0.5, 0.5), offset=(30, 0)), "n")
    left = position(numeric().annotate("n", (0.5, 0.5), offset=(-30, 0)), "n")
    above = position(numeric().annotate("n", (0.5, 0.5), offset=(0, 30)), "n")
    below = position(numeric().annotate("n", (0.5, 0.5), offset=(0, -30)), "n")
    assert left[0] < above[0] < right[0]
    assert above[0] == pytest.approx(below[0])
    assert above[1] < right[1] < below[1]
    assert right[1] == pytest.approx(left[1])


def test_arrow_is_drawn_only_when_asked() -> None:
    with_arrow = numeric().annotate("n", (0.5, 0.5), offset=(24, 18))
    without = numeric().annotate("n", (0.5, 0.5), offset=(24, 18), arrow=False)
    assert with_arrow.figure.to_svg().count("<path") > without.figure.to_svg().count("<path")


def test_arrow_stops_short_of_point() -> None:
    ax = numeric().annotate("n", (0.5, 0.5), offset=(0, 6))
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert str(caught.value) == (
        'Figure.to_svg(): the arrow of the annotation "n" has 6 pt between the text and the '
        "point; an arrow needs at least 10 pt."
    )
    assert details(caught.value) == (None, "pass a larger offset, or arrow=False.")
    assert numeric().annotate("n", (0.5, 0.5), offset=(0, 6), arrow=False).figure.to_svg()


@pytest.mark.parametrize(
    ("offset", "kind"),
    [
        ((0.0, 0.0), ub.StyleError),
        ((float("nan"), 1.0), ub.StyleError),
        ((1.0,), TypeError),
        ([1.0, 2.0], TypeError),
        (("a", 1.0), TypeError),
    ],
)
def test_offset_errors_are_atomic(offset: object, kind: type[Exception]) -> None:
    ax = numeric().annotate("first", (0.5, 0.5))
    before = ax.figure.to_svg()
    with pytest.raises(kind) as caught:
        call(ax.annotate)("x", (0.5, 0.5), offset=offset)
    info = details(caught.value)
    assert info is not None and info[0] == "offset" and info[1]
    assert ax.figure.to_svg() == before


def test_wrong_option_types_are_refused() -> None:
    ax = numeric()
    for keyword, value in [("arrow", 1), ("size", 12), ("color", "red")]:
        with pytest.raises(TypeError) as caught:
            call(ax.annotate)("x", (0.5, 0.5), **{keyword: value})
        info = details(caught.value)
        assert info is not None and info[0] == keyword
    with pytest.raises(TypeError) as caught:
        call(ax.annotate)("x", [0.5, 0.5])
    info = details(caught.value)
    assert info is not None and info[0] == "at"


def test_annotation_does_not_change_limits() -> None:
    plain = ub.Figure().add_axes().scatter([1, 2, 3], [4, 9, 2])
    annotated = ub.Figure().add_axes().scatter([1, 2, 3], [4, 9, 2])
    annotated.annotate("low", (3, 2), offset=(-30, 20))
    before = plain.figure.to_svg()
    after = annotated.figure.to_svg()
    # Everything the plain figure draws is drawn the same; the annotation comes last.
    assert after.startswith(before[: before.index("</svg>")])
    assert position(annotated, "low")[0] > 0


def test_text_outside_the_figure_rejected() -> None:
    ax = numeric().annotate("off", (0.95, 0.5), offset=(200, 0))
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert details(caught.value) == (None, "change align or offset.")


def test_annotate_checks_every_type_before_the_point_values() -> None:
    category = ub.Figure().add_axes(x=ub.Scale.category())
    timed = ub.Figure().add_axes(x=ub.Scale.time())
    cases: list[tuple[ub.Axes[Any, Any], tuple[object, float]]] = [
        (category, ("a" * 1001, 1.0)),
        (timed, (np.datetime64("NaT", "D"), 1.0)),
    ]
    for ax, at in cases:
        with pytest.raises(TypeError) as caught:
            call(ax.annotate)("x", at, size=12)
        info = details(caught.value)
        assert info is not None and info[0] == "size"
