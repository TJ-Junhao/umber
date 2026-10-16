"""Figure titles, subtitles and immutable title alignment."""

import copy
from collections.abc import Callable
from typing import cast
from xml.etree import ElementTree as ET

import pytest

import umber as ub
from umber import _umber
from umber._errors import details
from umber._theme import native


def _texts(figure: ub.Figure) -> dict[str, ET.Element]:
    root = ET.fromstring(figure.to_svg(text=ub.SvgText.EDITABLE))
    return {node.text or "": node for node in root.iter("{http://www.w3.org/2000/svg}text")}


def test_figure_title_renders_larger_than_panel_titles() -> None:
    fig = ub.Figure(cols=2).title("Whole figure").subtitle("Figure subtitle")
    fig.add_axes(col=0).title("First panel").subtitle("Panel subtitle")
    fig.add_axes(col=1).title("Second panel")
    texts = _texts(fig)
    figure_size = float(texts["Whole figure"].attrib["font-size"])
    for title in ("First panel", "Second panel"):
        assert figure_size == pytest.approx(1.5 * float(texts[title].attrib["font-size"]))
    assert (
        float(texts["Whole figure"].attrib["y"])
        < float(texts["Figure subtitle"].attrib["y"])
        < float(texts["First panel"].attrib["y"])
    )


def test_with_title_align_is_immutable() -> None:
    original = ub.Theme.PIGMENT
    centered = original.with_title_align(ub.Align.CENTER)
    assert centered is not original
    assert centered != original
    assert original == ub.Theme.PIGMENT.with_title_align(ub.Align.LEFT)
    assert centered == original.with_title_align(ub.Align.CENTER)
    left = _texts(ub.Figure(theme=original).title("Title"))["Title"]
    center = _texts(ub.Figure(theme=centered).title("Title"))["Title"]
    assert float(center.attrib["x"]) > float(left.attrib["x"])


@pytest.mark.parametrize(
    ("align", "fraction"),
    [(ub.Align.LEFT, 0.0), (ub.Align.CENTER, 0.5), (ub.Align.RIGHT, 1.0)],
)
def test_alignment_applies_to_figure_and_panel_text(align: ub.Align, fraction: float) -> None:
    def texts_at(value: ub.Align) -> dict[str, ET.Element]:
        fig = ub.Figure(theme=ub.Theme.PIGMENT.with_title_align(value))
        assert fig.title("Figure title").subtitle("Figure subtitle") is fig
        fig.add_axes().title("Panel title").subtitle("Panel subtitle")
        return _texts(fig)

    left = texts_at(ub.Align.LEFT)
    right = texts_at(ub.Align.RIGHT)
    actual = texts_at(align)
    for label in ("Figure title", "Figure subtitle", "Panel title", "Panel subtitle"):
        low = float(left[label].attrib["x"])
        high = float(right[label].attrib["x"])
        assert low < high
        assert float(actual[label].attrib["x"]) == pytest.approx(
            low + fraction * (high - low), abs=0.002
        )


def test_figure_text_matches_native_rendering() -> None:
    fig = ub.Figure().title("Whole figure").subtitle("Figure subtitle")
    fig.add_axes().title("Panel")
    expected = _umber.Figure(6, 4, native(ub.Theme.PIGMENT), 1, 1, 0, 0, 0, 0)
    expected.figure_title("Whole figure")
    expected.figure_subtitle("Figure subtitle")
    expected.add_axes(0, 0, 0, 0, 1, 1, 0, 10.0, 0, 10.0)
    expected.title(0, 0, "Panel")
    assert fig.to_svg() == expected.to_svg(ub.SvgText.OUTLINES.value)


@pytest.mark.parametrize("name", ["title", "subtitle"])
def test_figure_text_can_be_replaced_and_removed(name: str) -> None:
    fig = ub.Figure()
    fig.add_axes().title("Panel title").subtitle("Panel subtitle")
    setter = cast(Callable[[str], ub.Figure], getattr(fig, name))
    before = fig.to_svg()
    assert setter("Température") is fig
    assert "Température" in _texts(fig)
    setter("Replacement")
    assert "Température" not in _texts(fig)
    assert "Replacement" in _texts(fig)
    setter("")
    assert fig.to_svg() == before


@pytest.mark.parametrize("name", ["title", "subtitle"])
@pytest.mark.parametrize("value", [None, 3, b"Title", ["Title"]])
def test_figure_text_type_error_changes_nothing(name: str, value: object) -> None:
    fig = ub.Figure().title("Kept").subtitle("Also kept")
    before = fig.to_svg()
    setter = cast(Callable[[object], ub.Figure], getattr(fig, name))
    with pytest.raises(TypeError) as caught:
        setter(value)
    shown = "None" if value is None else type(value).__name__
    assert str(caught.value) == f"Figure.{name}(): text is {shown}; text must be a str."
    assert details(caught.value) == ("text", "pass a str; convert other values with str().")
    assert fig.to_svg() == before


@pytest.mark.parametrize("name", ["title", "subtitle"])
def test_figure_text_length_error_changes_nothing(name: str) -> None:
    fig = ub.Figure()
    setter = cast(Callable[[str], ub.Figure], getattr(fig, name))
    assert setter("x" * 1000) is fig
    setter("Kept")
    before = fig.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        setter("x" * 1001)
    assert str(caught.value) == (
        f"Figure.{name}(): text has 1,001 characters; text must be at most 1,000 characters."
    )
    assert details(caught.value) == ("text", "shorten the text; it would not fit the figure.")
    assert fig.to_svg() == before


@pytest.mark.parametrize("name", ["title", "subtitle"])
def test_figure_text_surrogate_error_changes_nothing(name: str) -> None:
    fig = ub.Figure().title("Kept")
    before = fig.to_svg()
    setter = cast(Callable[[str], ub.Figure], getattr(fig, name))
    with pytest.raises(ub.StyleError) as caught:
        setter("\ud800")
    assert str(caught.value).startswith(f"Figure.{name}(): text")
    metadata = details(caught.value)
    assert metadata is not None
    assert metadata[0] == "text"
    assert fig.to_svg() == before


def test_figure_text_error_highlights_text() -> None:
    fig = ub.Figure()
    with pytest.raises(TypeError) as caught:
        fig.title(text=cast(str, 42))
    line = "fig.title(text=cast(str, 42))"
    argument = "text=cast(str, 42)"
    assert getattr(caught.value, "__notes__", [])[0] == (
        f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"
    )


@pytest.mark.parametrize("clone", [copy.copy, copy.deepcopy])
def test_copy_keeps_independent_figure_text(clone: Callable[[ub.Figure], ub.Figure]) -> None:
    original = ub.Figure().title("Original").subtitle("Subtitle")
    original.add_axes().title("Panel")
    twin = clone(original)
    before = original.to_svg()
    assert twin.to_svg() == before
    twin.title("Changed").subtitle("")
    assert twin.to_svg() != before
    assert original.to_svg() == before
