"""Figure captions: a note under everything else in the figure."""

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

SVG = "{http://www.w3.org/2000/svg}"
CAPTION = "Long: up to 1,000 epochs; patience 100."


def _texts(figure: ub.Figure) -> dict[str, ET.Element]:
    root = ET.fromstring(figure.to_svg(text=ub.SvgText.EDITABLE))
    return {node.text or "": node for node in root.iter(f"{SVG}text")}


def _y(texts: dict[str, ET.Element], text: str) -> float:
    return float(texts[text].attrib["y"])


def _chart() -> ub.Axes[float, float]:
    return ub.Figure().add_axes().line([0, 1, 2], [1.0, 3.0, 2.0]).xlabel("Epoch")


def test_the_caption_sits_under_the_x_label_inside_the_figure() -> None:
    ax = _chart()
    before = _texts(ax.figure)
    assert ax.figure.caption(CAPTION) is ax.figure
    after = _texts(ax.figure)
    assert _y(after, CAPTION) > _y(after, "Epoch")
    assert _y(after, CAPTION) <= 4 * 72 - 12
    assert _y(after, "Epoch") < _y(before, "Epoch")


def test_the_caption_sits_under_a_bottom_legend() -> None:
    theme = ub.Theme.PIGMENT.with_legend_position(ub.LegendPosition.BOTTOM)
    fig = ub.Figure(theme=theme)
    fig.add_axes().line([0, 1, 2], [1.0, 3.0, 2.0], label="North").xlabel("Epoch")
    fig.caption(CAPTION)
    texts = _texts(fig)
    assert _y(texts, "Epoch") < _y(texts, "North") < _y(texts, CAPTION)


def test_the_caption_sits_under_a_bottom_colorbar() -> None:
    fig = ub.Figure()
    ax = fig.add_axes().xlabel("Distance")
    ax.scatter([1, 2, 3], [3.0, 1.0, 2.0], color=ub.ColorBy([0.0, 1.0, 2.0]))
    ax.colorbar("Temperature", position=ub.ColorbarPosition.BOTTOM)
    fig.caption(CAPTION)
    texts = _texts(fig)
    assert _y(texts, "Temperature") < _y(texts, CAPTION)
    assert _y(texts, "Distance") < _y(texts, CAPTION)


@pytest.mark.parametrize(
    ("align", "fraction"),
    [(ub.Align.LEFT, 0.0), (ub.Align.CENTER, 0.5), (ub.Align.RIGHT, 1.0)],
)
def test_the_caption_aligns_like_the_figure_title(align: ub.Align, fraction: float) -> None:
    def x_of(value: ub.Align) -> tuple[float, float]:
        fig = ub.Figure(theme=ub.Theme.PIGMENT.with_title_align(value))
        fig.title("Title").caption("Note").add_axes()
        texts = _texts(fig)
        return float(texts["Title"].attrib["x"]), float(texts["Note"].attrib["x"])

    (_, left), (_, right), (title, actual) = x_of(ub.Align.LEFT), x_of(ub.Align.RIGHT), x_of(align)
    assert left < right
    assert actual == pytest.approx(left + fraction * (right - left), abs=0.002)
    if align is ub.Align.LEFT:
        assert actual == pytest.approx(title, abs=0.002)


def test_line_breaks_stack_the_lines() -> None:
    one = _chart().figure.caption("First")
    two = _chart().figure.caption("First\nSecond")
    texts = _texts(two)
    size = float(texts["Second"].attrib["font-size"])
    assert _y(texts, "Second") - _y(texts, "First") == pytest.approx(1.2 * size, abs=0.01)
    assert _y(texts, "Second") == pytest.approx(_y(_texts(one), "First"), abs=0.01)


def test_default_size_color_and_size_arguments() -> None:
    fig = _chart().figure.caption("Note")
    texts = _texts(fig)
    assert texts["Note"].attrib["font-size"] == texts["Epoch"].attrib["font-size"]
    tick = next(node for text, node in texts.items() if re.fullmatch(r"[0-9.]+", text))
    assert texts["Note"].attrib["fill"] == tick.attrib["fill"]
    size = float(texts["Note"].attrib["font-size"])
    fig.caption("Note", size=ub.Size.times(2))
    assert float(_texts(fig)["Note"].attrib["font-size"]) == pytest.approx(2 * size)
    fig.caption("Note", size=ub.Size.pt(9))
    assert float(_texts(fig)["Note"].attrib["font-size"]) == pytest.approx(9.0)


def test_a_line_too_wide_for_the_figure_raises_with_both_widths() -> None:
    fig = ub.Figure(2, 3)
    fig.add_axes()
    fig.caption(CAPTION)
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    message = str(caught.value)
    # Quoted texts are cut to forty characters.
    assert message.startswith(
        'Figure.to_svg(): the caption "Long: up to 1,000 epochs; patience 100'
    )
    assert message.endswith(" pt wide; the figure has 120 pt.")
    assert details(caught.value) == (
        None,
        'break the line with "\\n", make the figure wider or lower the font scale with '
        "Theme.with_font_scale().",
    )
    fig.caption("Long: up to 1,000\nepochs; patience 100.")
    fig.to_svg()


def test_the_caption_can_be_replaced_and_removed() -> None:
    fig = _chart().figure
    before = fig.to_svg()
    fig.caption("Température")
    assert "Température" in _texts(fig)
    fig.caption("Replacement")
    assert "Température" not in _texts(fig)
    assert "Replacement" in _texts(fig)
    fig.caption("")
    assert fig.to_svg() == before


@pytest.mark.parametrize("value", [None, 3, b"Note", ["Note"]])
def test_a_caption_of_the_wrong_type_changes_nothing(value: object) -> None:
    fig = _chart().figure.caption("Kept")
    before = fig.to_svg()
    with pytest.raises(TypeError) as caught:
        cast(Callable[[object], ub.Figure], fig.caption)(value)
    shown = "None" if value is None else type(value).__name__
    assert str(caught.value) == f"Figure.caption(): text is {shown}; text must be a str."
    assert fig.to_svg() == before
    with pytest.raises(TypeError):
        fig.caption("Note", size=cast(ub.Size, 12))
    assert fig.to_svg() == before


def test_a_caption_of_too_many_characters_changes_nothing() -> None:
    fig = _chart().figure
    assert fig.caption("x" * 1000) is fig
    fig.caption("Kept")
    before = fig.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        fig.caption("x" * 1001)
    assert str(caught.value) == (
        "Figure.caption(): text has 1,001 characters; text must be at most 1,000 characters."
    )
    assert fig.to_svg() == before


def test_a_caption_refuses_control_characters_like_a_subtitle() -> None:
    bad = "Line one\nab\t"
    with pytest.raises(ub.StyleError) as caption:
        ub.Figure().caption(bad)
    with pytest.raises(ub.StyleError) as subtitle:
        ub.Figure().subtitle(bad)
    expected = str(subtitle.value).replace("Figure.subtitle()", "Figure.caption()")
    assert str(caption.value) == expected
    assert details(caption.value) == details(subtitle.value)


def test_copies_keep_the_caption_and_stay_independent() -> None:
    fig = _chart().figure.caption("Original")
    twin = copy.copy(fig)
    deep = copy.deepcopy(fig)
    assert fig.to_svg() == twin.to_svg() == deep.to_svg()
    twin.caption("Changed")
    assert "Original" in _texts(fig)
    assert "Original" in _texts(deep)
    assert "Changed" in _texts(twin)
    assert "Original" not in _texts(twin)


def test_grid_pie_and_quick_figures_take_a_caption() -> None:
    grid = ub.Figure(8, 4, cols=2)
    grid.add_axes(col=0).line([0, 1], [1.0, 2.0]).xlabel("Left")
    grid.add_axes(col=1).line([0, 1], [2.0, 1.0]).xlabel("Right")
    grid.caption(CAPTION)
    texts = _texts(grid)
    assert max(_y(texts, "Left"), _y(texts, "Right")) < _y(texts, CAPTION)

    pie = ub.pie(["A", "B"], [1.0, 3.0]).figure.caption("Share of orders")
    assert "Share of orders" in _texts(pie)

    quick = ub.line([0, 1, 2], [1.0, 3.0, 2.0]).figure.caption("Quick")
    assert "Quick" in _texts(quick)

    fig = ub.Figure()
    fig.add_pie_axes().pie(["A", "B"], [1.0, 3.0])
    fig.caption("Share of orders")
    texts = _texts(fig)
    assert _y(texts, "Share of orders") <= 4 * 72 - 12


def test_png_svg_and_pdf_draw_the_caption() -> None:
    fig = _chart().figure
    plain = (fig.to_png(), fig.to_svg(), fig.to_pdf())
    fig.caption("Footnote under the chart")
    png, svg, pdf = fig.to_png(), fig.to_svg(), fig.to_pdf()
    assert png != plain[0]
    assert svg != plain[1]
    assert pdf != plain[2]
    assert "Footnote under the chart" not in svg
    assert "Footnote under the chart" in fig.to_svg(text=ub.SvgText.EDITABLE)
    assert pdf.startswith(b"%PDF-")


def test_caption_matches_native_rendering() -> None:
    fig = ub.Figure().caption("Note")
    fig.add_axes().title("Panel")
    expected = _umber.Figure(6, 4, native(ub.Theme.PIGMENT), 1, 1, 0, 0, 0, 0)
    expected.figure_caption("Note", (0, 0.0))
    expected.add_axes(0, 0, 0, 0, 1, 1, 0, 10.0, 0, 10.0)
    expected.title(0, 0, "Panel", (0, 0.0))
    assert fig.to_svg() == expected.to_svg(ub.SvgText.OUTLINES.value, 300)


def test_an_annotation_cannot_reach_into_the_caption() -> None:
    theme = ub.Theme.PIGMENT.with_title_align(ub.Align.CENTER)
    fig = ub.Figure(6, 4, theme=theme).caption("Caption caption caption")
    ax = fig.add_axes().line([0, 1], [0.0, 1.0]).xlim(0, 1).ylim(0, 1)
    ax.annotate("Annotation annotation", (0.5, 0.0), offset=(0.0, -25.0), arrow=False)
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    message = str(caught.value)
    assert message.startswith('Figure.to_svg(): the annotation "Annotation annotation" reaches ')
    assert message.endswith(" pt into the caption.")
    assert details(caught.value) == (None, "change align or offset.")
