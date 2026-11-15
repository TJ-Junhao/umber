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

SHRUNK_HINT = 'make the figure wider, shorten the {name}, or break it into lines with "\\n".'
LONG = "Quarterly revenue by region and sales channel"


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
    assert fig.to_svg() == expected.to_svg(ub.SvgText.OUTLINES.value, 300)


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
        at = ""
    else:
        text = "A very long figure title that certainly cannot fit in six inches of width at all"
        fig.title(text)
        container = "figure"
        hint = SHRUNK_HINT.format(name="title")
        at = " at 15 pt, the smallest size it shrinks to"
    call = cast(Callable[[], bytes | str], getattr(fig, render))
    with pytest.raises(ub.StyleError) as caught:
        call()
    shown = '"' + text[:38] + "…"
    match = re.fullmatch(
        re.escape(f"Figure.{render}(): the title {shown} is ")
        + r"(\d+)"
        + re.escape(f" pt wide{at}; the {container} has 408 pt."),
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
    if size == ub.Size.AUTO:
        # The text shrank to its floor and is still too wide there.
        floor = "15" if kind == "figure" and not subtitle else "8.5"
        at = f" at {floor} pt, the smallest size it shrinks to"
        hint = SHRUNK_HINT.format(name=name)
    else:
        at = ""
        hint = (
            "make the figure wider or pass a smaller size=."
            if size == ub.Size.pt(18)
            else "make the figure wider or lower the font scale with Theme.with_font_scale()."
        )
    assert str(caught.value).endswith(f"pt wide{at}; the {container} has {room} pt.")
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


def _size(element: ET.Element) -> float:
    return float(element.attrib["font-size"])


def test_a_title_too_wide_shrinks_in_half_points() -> None:
    fig = ub.Figure(3.5, 2)
    fig.add_axes().line([0, 1], [0.0, 1.0]).title(LONG)
    size = _size(_texts(fig)[LONG])
    assert 8.5 <= size < 15
    assert (size * 2).is_integer()
    assert fig.to_png()


def test_a_title_that_fits_keeps_its_size() -> None:
    fig = ub.Figure(6, 4)
    fig.add_axes().title("Revenue")
    assert _size(_texts(fig)["Revenue"]) == 15


def test_a_title_with_size_does_not_shrink() -> None:
    fig = ub.Figure(3.5, 2)
    fig.add_axes().title(LONG, size=ub.Size.pt(15))
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    assert details(caught.value) == (None, "make the figure wider or pass a smaller size=.")


def test_a_title_too_wide_at_the_axis_label_size_raises() -> None:
    fig = ub.Figure(2, 2)
    fig.add_axes().title(LONG + " " + LONG)
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    message = str(caught.value)
    assert " pt wide at 8.5 pt, the smallest size it shrinks to; the panel has 120 pt." in message
    assert details(caught.value) == (None, SHRUNK_HINT.format(name="title"))


def test_panel_titles_shrink_to_one_size() -> None:
    fig = ub.Figure(10, 3, cols=3)
    fig.add_axes(col=0).title(LONG)
    fig.add_axes(col=1).title("Rain")
    fig.add_axes(col=2).title("Wind", size=ub.Size.pt(12))
    fig.title("Weather")
    texts = _texts(fig)
    shrunk = _size(texts[LONG])
    assert 8.5 <= shrunk < 15
    assert _size(texts["Rain"]) == shrunk
    assert _size(texts["Wind"]) == 12
    assert _size(texts["Weather"]) == 22.5


def test_the_figure_title_shrinks_on_its_own() -> None:
    fig = ub.Figure(5.5, 3).title(LONG)
    fig.add_axes().title("Rain")
    texts = _texts(fig)
    assert 15 <= _size(texts[LONG]) < 22.5
    assert _size(texts["Rain"]) == 15


def test_every_line_of_a_title_shrinks_together() -> None:
    fig = ub.Figure(3.5, 3)
    fig.add_axes().title(f"Revenue\n{LONG}")
    texts = _texts(fig)
    size = _size(texts[LONG])
    assert size < 15
    assert _size(texts["Revenue"]) == size


def test_shrunk_titles_leave_the_plot_more_room() -> None:
    def plot_top(title: str) -> float:
        fig = ub.Figure(3.5, 3)
        fig.add_axes().xticks([]).yticks([]).title(title).ylabel("Load")
        return float(_texts(fig)["Load"].attrib["y"])

    assert plot_top(LONG) < plot_top("Revenue")


def test_grid_headers_start_at_the_y_axis_label() -> None:
    fig = ub.Figure(8, 3, cols=2, legend=ub.Legend.PER_PANEL)
    left = fig.add_axes(col=0).line([0, 1], [0, 2000], label="Sunny").title("Days")
    left.ylabel("Share of days (%)").subtitle("By type")
    right = fig.add_axes(col=1).line([0, 1], [0, 1]).title("Hours")
    right.yticks_labeled([0, 1], ["low", "highest"])
    texts = _texts(fig)
    x = float(texts["Share of days (%)"].attrib["x"])
    assert float(texts["Days"].attrib["x"]) == pytest.approx(x, abs=0.01)
    assert float(texts["By type"].attrib["x"]) == pytest.approx(x, abs=0.01)
    assert float(texts["Sunny"].attrib["x"]) > x
    # The second panel's title starts at its widest y tick label, left of its plot.
    widest = float(texts["highest"].attrib["x"])
    assert widest < float(texts["low"].attrib["x"])
    assert float(texts["Hours"].attrib["x"]) == pytest.approx(widest, abs=0.01)


def test_figure_texts_stay_at_the_figure_margin() -> None:
    fig = ub.Figure(8, 3, cols=2).title("Weather").subtitle("Two stations").caption("Survey")
    fig.add_axes(col=0).line([0, 1], [0, 2000]).ylabel("Share of days (%)")
    fig.add_axes(col=1).line([0, 1], [0, 1])
    texts = _texts(fig)
    for text in ("Weather", "Two stations", "Survey", "Share of days (%)"):
        assert float(texts[text].attrib["x"]) == pytest.approx(12, abs=0.01)


@pytest.mark.parametrize(("width", "cols"), [(3.5, 1), (7, 2)])
def test_titles_shrink_before_the_height_is_checked(width: float, cols: int) -> None:
    text = "Quarterly revenue by region and sales channel"
    fig = ub.Figure(width, 1, cols=cols)
    for col in range(cols):
        fig.add_axes(col=col).xticks([]).yticks([]).title("\n".join([text] * 3))
    assert fig.to_svg()
    assert _size(_texts(fig)[text]) < 15


@pytest.mark.parametrize("cols", [1, 2])
def test_headers_over_equal_aspect_plots_start_at_the_y_axis_label(cols: int) -> None:
    fig = ub.Figure(6 * cols, 4, cols=cols)
    for col in range(cols):
        ax = fig.add_axes(col=col).xlim(0, 20).ylim(0, 1)
        ax.title(f"T{col}").ylabel(f"Y{col}").equal_aspect()
    texts = _texts(fig)
    for col in range(cols):
        assert float(texts[f"T{col}"].attrib["x"]) == pytest.approx(
            float(texts[f"Y{col}"].attrib["x"]), abs=0.01
        )


def _inset_legend(height: float, position: ub.LegendPosition) -> ub.Figure:
    fig = ub.Figure(3.5, height / 72)
    ax = fig.add_axes().title("Days").ylabel("Share").xlim(0, 1).ylim(0, 1)
    ax.xticks_labeled([0, 1], ["W" * 10, "1"])
    ax.yticks_labeled([0, 1], ["0", "1"]).legend(ub.LegendStyle.HEADER, position=position)
    for i in range(8):
        ax.line([0, 1], [0, 1], label=f"RRR{i}")
    return fig


@pytest.mark.parametrize(
    ("height", "position"),
    [(119, ub.LegendPosition.TOP_LEFT), (121, ub.LegendPosition.BOTTOM)],
)
def test_a_short_figure_with_an_inset_legend_names_its_height(
    height: float, position: ub.LegendPosition
) -> None:
    with pytest.raises(ub.StyleError) as caught:
        _inset_legend(height, position).to_svg()
    message = str(caught.value)
    assert "colorbar" not in message
    found = re.search(r"at least (\d+) pt", message)
    assert found is not None, message
    assert _inset_legend(int(found[1]), position).to_svg()
