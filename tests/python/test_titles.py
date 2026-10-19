"""Figure titles, subtitles and immutable title alignment."""

import copy
import re
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
    expected.figure_title("Whole figure", (0, 0.0))
    expected.figure_subtitle("Figure subtitle", (0, 0.0))
    expected.add_axes(0, 0, 0, 0, 1, 1, 0, 10.0, 0, 10.0)
    expected.title(0, 0, "Panel", (0, 0.0))
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


@pytest.mark.parametrize("render", ["to_png", "to_svg", "to_pdf"])
@pytest.mark.parametrize("kind", ["axes", "figure"])
def test_long_title_reproduction_raises_at_render(render: str, kind: str) -> None:
    fig = ub.Figure(6, 4)
    ax = fig.add_axes().line([0, 1], [0.0, 1.0])
    if kind == "axes":
        text = "A very long title that certainly cannot fit in six inches of width at all, size"
        ax.title(text, size=ub.Size.pt(18))
        container = "panel"
        hint = "make the figure wider or pass a smaller size=."
    else:
        text = "A very long figure title that certainly cannot fit in six inches of width at all"
        fig.title(text)
        container = "figure"
        hint = "make the figure wider or lower the font scale with Theme.with_font_scale()."
    call = cast(Callable[[], bytes | str], getattr(fig, render))
    with pytest.raises(ub.StyleError) as caught:
        call()
    shown = '"' + text[:38] + "…"
    match = re.fullmatch(
        re.escape(f"Figure.{render}(): the title {shown} is ")
        + r"(\d+)"
        + re.escape(f" pt wide; the {container} has 408 pt."),
        str(caught.value),
    )
    assert match is not None
    assert int(match[1]) > 408
    assert details(caught.value) == (None, hint)


@pytest.mark.parametrize("render", ["to_png", "to_svg", "to_pdf"])
@pytest.mark.parametrize("align", [ub.Align.LEFT, ub.Align.CENTER, ub.Align.RIGHT])
@pytest.mark.parametrize(
    ("kind", "subtitle"),
    [
        (kind, subtitle)
        for kind in ("figure", "axes", "grid", "span", "twin")
        for subtitle in (False, True)
    ]
    + [("pie", False)],
)
@pytest.mark.parametrize("size", [ub.Size.AUTO, ub.Size.times(1.4), ub.Size.pt(18)])
def test_titles_fit_their_own_room(
    render: str, align: ub.Align, kind: str, subtitle: bool, size: ub.Size
) -> None:
    cols = 3 if kind == "span" else 2 if kind == "grid" else 1
    fig = ub.Figure(6, 4, cols=cols, theme=ub.Theme.PIGMENT.with_title_align(align))
    target: ub.Figure | ub.Axes | ub.PieAxes
    if kind == "figure":
        target = fig
    elif kind == "pie":
        target = fig.add_pie_axes().pie(["A"], [1])
    else:
        ax = fig.add_axes(col=0, col_span=2 if kind == "span" else 1)
        ax.xticks([]).yticks([])
        target = ax
        if kind in ("grid", "span"):
            fig.add_axes(col=cols - 1).xticks([]).yticks([])
        elif kind == "twin":
            target = ax.twin_y().yticks([])
    name = "subtitle" if subtitle else "title"
    setter = cast(Callable[..., object], getattr(target, name))
    setter("W" * 100, size=size)
    call = cast(Callable[[], bytes | str], getattr(fig, render))
    with pytest.raises(ub.StyleError) as caught:
        call()
    prefix = "row 0, col 0: " if cols > 1 else ""
    room = 264 if kind == "span" else 192 if kind == "grid" else 408
    container = "figure" if kind == "figure" else "panel"
    assert str(caught.value).startswith(
        f'Figure.{render}(): {prefix}the {name} "' + "W" * 38 + "… is "
    )
    assert str(caught.value).endswith(f"pt wide; the {container} has {room} pt.")
    hint = (
        "make the figure wider or pass a smaller size=."
        if size == ub.Size.pt(18)
        else "make the figure wider or lower the font scale with Theme.with_font_scale()."
    )
    assert details(caught.value) == (None, hint)
    setter("Fits", size=size)
    assert call()


@pytest.mark.parametrize("render", ["to_png", "to_svg", "to_pdf"])
@pytest.mark.parametrize("kind", ["axes", "grid", "span", "twin-left", "twin-right"])
@pytest.mark.parametrize("axis", ["x", "y"])
@pytest.mark.parametrize("size", [ub.Size.AUTO, ub.Size.times(1.4), ub.Size.pt(18)])
def test_axis_titles_fit_their_own_room(render: str, kind: str, axis: str, size: ub.Size) -> None:
    cols = 3 if kind == "span" else 2 if kind == "grid" else 1
    fig = ub.Figure(6, 4, cols=cols)
    ax = fig.add_axes(col=0, col_span=2 if kind == "span" else 1).xticks([]).yticks([])
    if cols > 1:
        fig.add_axes(col=cols - 1).xticks([]).yticks([])
    elif kind.startswith("twin"):
        right = ax.twin_y().yticks([])
        if kind == "twin-right":
            ax = right
    setter = ax.xlabel if axis == "x" else ax.ylabel
    setter("W" * 100, size=size)
    call = cast(Callable[[], bytes | str], getattr(fig, render))
    with pytest.raises(ub.StyleError) as caught:
        call()
    prefix = "row 0, col 0: " if cols > 1 else ""
    room = 264 if kind == "span" else 192 if kind == "grid" else 408
    side = (
        ("right " if kind == "twin-right" else "left ")
        if axis == "y" and kind.startswith("twin")
        else ""
    )
    shown = '"' + "W" * 38 + "…"
    match = re.fullmatch(
        re.escape(f"Figure.{render}(): {prefix}the {side}{axis} axis label {shown} is ")
        + r"(\d+)"
        + re.escape(f" pt wide; the panel has {room} pt."),
        str(caught.value),
    )
    assert match is not None
    assert int(match[1]) > room
    remedy = (
        "pass a smaller size=."
        if size == ub.Size.pt(18)
        else "lower the font scale with Theme.with_font_scale()."
    )
    assert details(caught.value) == (None, f"make the figure wider or {remedy}")
    setter("Fits", size=size)
    assert call()


@pytest.mark.parametrize("render", ["to_png", "to_svg", "to_pdf"])
@pytest.mark.parametrize("axis", ["x", "y"])
def test_long_axis_title_reproduction_raises_at_render(render: str, axis: str) -> None:
    fig = ub.Figure(6, 4)
    ax = fig.add_axes().line([0, 1], [0.0, 1.0])
    setter = ax.xlabel if axis == "x" else ax.ylabel
    setter("W" * 100)
    call = cast(Callable[[], bytes | str], getattr(fig, render))
    with pytest.raises(ub.StyleError, match=f'the {axis} axis label "W'):
        call()


@pytest.mark.parametrize("align", [ub.Align.LEFT, ub.Align.CENTER, ub.Align.RIGHT])
@pytest.mark.parametrize("subtitle", [False, True])
def test_twin_keeps_single_panel_title_room_with_ticks(align: ub.Align, subtitle: bool) -> None:
    fig = ub.Figure(6, 4, theme=ub.Theme.PIGMENT.with_title_align(align))
    ax = fig.add_axes().xticks([0, 1]).yticks_labeled([0, 1], ["Wide ticks"] * 2)
    text = "W" * 38
    setter = ax.subtitle if subtitle else ax.title
    setter(text, size=ub.Size.pt(10))
    before = _texts(fig)[text]
    ax.twin_y().yticks_labeled([0, 1], ["Right ticks"] * 2)
    after = _texts(fig)[text]
    assert (before.attrib["x"], before.attrib["y"]) == (after.attrib["x"], after.attrib["y"])
    setter("W" * 50, size=ub.Size.pt(10))
    with pytest.raises(ub.StyleError, match="the panel has 408 pt"):
        fig.to_svg()


@pytest.mark.parametrize("kind", ["title", "subtitle"])
def test_figure_text_errors_precede_panel_errors(kind: str) -> None:
    fig = ub.Figure()
    fig.add_axes().title("W" * 100)
    setter = fig.title if kind == "title" else fig.subtitle
    setter("W" * 100)
    with pytest.raises(ub.StyleError, match=f"the {kind} .*the figure has 408 pt"):
        fig.to_svg()
