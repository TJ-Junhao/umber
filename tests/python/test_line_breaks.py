"""Line breaks in titles, subtitles, text() and annotate(), and their refusal elsewhere."""

import re
from collections.abc import Callable
from xml.etree import ElementTree as ET

import numpy as np
import pytest

import umber as ub
from umber._errors import details

LINE_BREAK_HINT = "line breaks are supported in titles, subtitles, text() and annotate()."
SVG = "{http://www.w3.org/2000/svg}"
BAD = "Hours\nUTC"
"""Text of one line that holds a line break at index 5."""


def _figure() -> ub.Figure:
    """A figure using every API that takes text of several lines."""
    fig = ub.Figure(9, 4, cols=2).title("Regional growth\nacross regions")
    fig.subtitle("Weekly totals\n2026")
    ax = fig.add_axes(col=0).title("Station output\nDaily mean\nMW")
    ax.subtitle("Measured hourly\n\nall stations")
    ax.line([0, 1, 2, 3], [1.0, 3.0, 2.0, 2.5])
    ax.text("Target\nband", (1, 2.2), align=ub.Align.CENTER)
    ax.annotate("Peak\nof the season", (1, 3), offset=(40, -30))
    pie = fig.add_pie_axes(col=1).title("Orders\nby region")
    pie.pie(["North", "South"], [2, 3])
    return fig


LINES = [
    "Regional growth",
    "across regions",
    "Weekly totals",
    "2026",
    "Station output",
    "Daily mean",
    "MW",
    "Measured hourly",
    "all stations",
    "Target",
    "band",
    "Peak",
    "of the season",
    "Orders",
    "by region",
]


def test_text_of_several_lines_renders_in_every_format() -> None:
    fig = _figure()
    root = ET.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE))
    texts = [node.text or "" for node in root.iter(f"{SVG}text")]
    for line in LINES:
        assert line in texts, line
    assert not any(c in text for text in texts for c in "\n\r\t")
    # Each line is its own element, below the one before it.
    by_text = {node.text: float(node.attrib["y"]) for node in root.iter(f"{SVG}text")}
    for first, second in zip(LINES, LINES[1:], strict=False):
        if first in ("across regions", "2026", "MW", "all stations", "band", "of the season"):
            continue
        assert by_text[first] < by_text[second], (first, second)
    outlines = fig.to_svg(text=ub.SvgText.OUTLINES)
    assert "<text" not in outlines
    assert fig.to_png().startswith(b"\x89PNG")
    assert fig.to_pdf().startswith(b"%PDF-")


def test_a_title_of_several_lines_moves_the_plot_down() -> None:
    def plot_top(title: str) -> float:
        fig = ub.Figure()
        fig.add_axes().title(title).ylabel("Units").line([0, 1], [0.0, 1.0])
        root = ET.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE))
        return next(float(n.attrib["y"]) for n in root.iter(f"{SVG}text") if n.text == "Units")

    one, three = plot_top("Line one"), plot_top("Line one\nLine two\nLine three")
    assert three - one == pytest.approx(2 * 1.2 * 15.0, abs=0.01)


def test_the_reproduction_draws_two_lines() -> None:
    fig = ub.Figure(5, 2.2)
    fig.add_axes().line([0, 1], [0.0, 1.0]).title("Line one\nLine two")
    root = ET.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE))
    texts = [node.text for node in root.iter(f"{SVG}text")]
    assert "Line one" in texts and "Line two" in texts
    assert fig.to_png(dpi=72).startswith(b"\x89PNG")


def _raised(cls: type[Exception], call: Callable[[], object]) -> Exception:
    with pytest.raises(cls) as info:
        call()
    assert type(info.value) is cls
    return info.value


def _underlined(err: BaseException) -> str:
    """The source text the highlight note of ``err`` underlines."""
    notes: list[str] = getattr(err, "__notes__", [])
    assert notes, err
    source, carets = notes[0].split("\n")[:2]
    start = len(carets) - len(carets.lstrip(" "))
    return source[start : len(carets.rstrip())]


def _category_axes() -> "ub.Axes[str, float]":
    return ub.Figure().add_axes(x=ub.Scale.category())


def _color_axes() -> "ub.Axes[float, float]":
    return ub.Figure().add_axes().scatter([1, 2], [2, 3], color=ub.ColorBy([4, 8]))


# Calls whose one-line text holds BAD, with the error class, the message after the context,
# the parameter and the underlined source.
ONE_LINE: list[tuple[str, Callable[[], object], type[Exception], str, str, str]] = [
    (
        "Axes.xlabel()",
        lambda: ub.Figure().add_axes().xlabel(BAD),
        ub.StyleError,
        "text holds a line break at index 5; axis labels are one line.",
        "text",
        "BAD",
    ),
    (
        "Axes.ylabel()",
        lambda: ub.Figure().add_axes().ylabel(BAD),
        ub.StyleError,
        "text holds a line break at index 5; axis labels are one line.",
        "text",
        "BAD",
    ),
    (
        "Axes.line()",
        lambda: ub.Figure().add_axes().line([0, 1], [1, 2], label=BAD),
        ub.StyleError,
        "label holds a line break at index 5; legend labels are one line.",
        "label",
        "label=BAD",
    ),
    (
        "Axes.line()",
        lambda: _category_axes().line(["a", BAD], [1, 2]),
        ub.DataError,
        "x[1] holds a line break at index 5; category names are one line.",
        "x",
        '["a", BAD]',
    ),
    (
        "Axes.line()",
        lambda: ub.Figure().add_axes().line([0, 1], [1, 2], by=["a", BAD]),
        ub.DataError,
        "by[1] holds a line break at index 5; group names are one line.",
        "by",
        'by=["a", BAD]',
    ),
    (
        "Axes.bar()",
        lambda: _category_axes().bar(["a", BAD], [1, 2]),
        ub.DataError,
        "categories[1] holds a line break at index 5; category names are one line.",
        "categories",
        '["a", BAD]',
    ),
    (
        "Axes.xticks_labeled()",
        lambda: ub.Figure().add_axes().xticks_labeled([0, 1], ["a", BAD]),
        ub.DataError,
        "labels[1] holds a line break at index 5; tick labels are one line.",
        "labels",
        '["a", BAD]',
    ),
    (
        "Axes.colorbar()",
        lambda: _color_axes().colorbar(BAD),
        ub.StyleError,
        "label holds a line break at index 5; colorbar labels are one line.",
        "label",
        "BAD",
    ),
    (
        "Figure.colorbar()",
        lambda: ub.Figure(cols=2, share_color=ub.Share.ALL).colorbar(BAD),
        ub.StyleError,
        "label holds a line break at index 5; colorbar labels are one line.",
        "label",
        "BAD",
    ),
    (
        "PieAxes.pie()",
        lambda: ub.Figure().add_pie_axes().pie(["a", BAD], [1, 2]),
        ub.DataError,
        "categories[1] holds a line break at index 5; category names are one line.",
        "categories",
        '["a", BAD]',
    ),
    (
        "TickFormat.affix()",
        lambda: ub.TickFormat.fixed(1).affix(BAD),
        ub.StyleError,
        "prefix holds a line break at index 5; tick labels are one line.",
        "prefix",
        "BAD",
    ),
    (
        "Series()",
        lambda: ub.Series(BAD, [1.0]),
        ub.StyleError,
        "label holds a line break at index 5; legend labels are one line.",
        "label",
        "BAD",
    ),
    (
        "Distribution()",
        lambda: ub.Distribution(BAD, [[1.0]]),
        ub.StyleError,
        "label holds a line break at index 5; legend labels are one line.",
        "label",
        "BAD",
    ),
    (
        "Font.system()",
        lambda: ub.Font.system(BAD),
        ub.StyleError,
        "family holds a line break at index 5; font family names are one line.",
        "family",
        "BAD",
    ),
]


@pytest.mark.parametrize(
    ("context", "call", "cls", "message", "parameter", "underlined"),
    ONE_LINE,
    ids=[f"{case[0]}-{case[4]}" for case in ONE_LINE],
)
def test_text_of_one_line_refuses_a_line_break(
    context: str,
    call: Callable[[], object],
    cls: type[Exception],
    message: str,
    parameter: str,
    underlined: str,
) -> None:
    err = _raised(cls, call)
    assert str(err) == f"{context}: {message}"
    assert details(err) == (parameter, LINE_BREAK_HINT)
    assert _underlined(err) == underlined


REFUSED = [
    ("\t", "U+0009 (a tab)", "text cannot hold control characters."),
    ("\r", "U+000D (a carriage return)", "text cannot hold control characters."),
    ("\x00", "U+0000", "text cannot hold control characters."),
    ("\x85", "U+0085", "text cannot hold control characters."),
    (" ", "U+2028 (a line separator)", "text cannot hold line or paragraph separators."),
]
ONE_LINE_HINTS = {
    "\t": "use spaces instead of tabs.",
    "\r": "remove the carriage return.",
    " ": "remove the separator.",
}
LINES_HINTS = {
    "\t": 'use spaces instead of tabs, and "\\n" for a line break.',
    "\r": 'use "\\n" alone for a line break.',
    " ": 'use "\\n" for a line break.',
}


@pytest.mark.parametrize(("character", "code", "reason"), REFUSED)
def test_text_of_several_lines_refuses_other_control_characters(
    character: str, code: str, reason: str
) -> None:
    bad = f"Line one\nab{character}"
    calls: list[tuple[str, Callable[[], object]]] = [
        ("Axes.title()", lambda: ub.Figure().add_axes().title(bad)),
        ("Axes.subtitle()", lambda: ub.Figure().add_axes().subtitle(bad)),
        ("Figure.title()", lambda: ub.Figure().title(bad)),
        ("Figure.subtitle()", lambda: ub.Figure().subtitle(bad)),
        ("PieAxes.title()", lambda: ub.Figure().add_pie_axes().title(bad)),
        ("Axes.text()", lambda: ub.Figure().add_axes().text(bad, (0.5, 0.5))),
        ("Axes.annotate()", lambda: ub.Figure().add_axes().annotate(bad, (0.5, 0.5))),
    ]
    for context, call in calls:
        err = _raised(ub.StyleError, call)
        assert str(err) == f"{context}: text holds {code} at index 11; {reason}"
        hint = LINES_HINTS.get(character, "remove the character.")
        assert details(err) == ("text", hint)
        assert _underlined(err) == "bad"


@pytest.mark.parametrize(("character", "code", "reason"), REFUSED)
def test_text_of_one_line_refuses_control_characters(
    character: str, code: str, reason: str
) -> None:
    bad = f"ab{character}"
    hint = ONE_LINE_HINTS.get(character, "remove the character.")
    calls: list[tuple[str, Callable[[], object], type[Exception], str]] = [
        ("Axes.xlabel()", lambda: ub.Figure().add_axes().xlabel(bad), ub.StyleError, "text"),
        ("Axes.ylabel()", lambda: ub.Figure().add_axes().ylabel(bad), ub.StyleError, "text"),
        (
            "Axes.scatter()",
            lambda: ub.Figure().add_axes().scatter([1], [2], label=bad),
            ub.StyleError,
            "label",
        ),
        ("Axes.bar()", lambda: _category_axes().bar([bad], [1]), ub.DataError, "categories[0]"),
        (
            "Axes.yticks_labeled()",
            lambda: ub.Figure().add_axes().yticks_labeled([0], [bad]),
            ub.DataError,
            "labels[0]",
        ),
        ("TickFormat.affix()", lambda: ub.TickFormat.AUTO.affix("", bad), ub.StyleError, "suffix"),
        ("Color.hex()", lambda: ub.Color.hex(bad), ub.StyleError, "code"),
        (
            "Axes.hist()",
            lambda: ub.Figure().add_axes().hist([1.0, 2.0], by=["a", bad]),
            ub.DataError,
            "by[1]",
        ),
    ]
    for context, call, cls, label in calls:
        err = _raised(cls, call)
        assert str(err) == f"{context}: {label} holds {code} at index 2; {reason}", context
        found = details(err)
        assert found is not None and found[1] == hint, context


def test_names_in_numpy_arrays_are_checked_as_names() -> None:
    names = np.array(["North", "South\nEast"])
    err = _raised(ub.DataError, lambda: _category_axes().bar(names, [1, 2]))
    assert str(err) == (
        "Axes.bar(): categories[1] holds a line break at index 5; category names are one line."
    )


def test_blank_lines_are_kept_and_an_empty_title_draws_nothing() -> None:
    fig = ub.Figure()
    ax = fig.add_axes().line([0, 1], [0.0, 1.0])
    ax.title("\nMiddle\n")
    root = ET.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE))
    middle = [node for node in root.iter(f"{SVG}text") if node.text == "Middle"]
    assert len(middle) == 1
    ax.title("")
    root = ET.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE))
    assert not [node for node in root.iter(f"{SVG}text") if node.text == "Middle"]


@pytest.mark.parametrize("position", [ub.LegendPosition.RIGHT, ub.LegendPosition.BEST])
def test_the_height_named_for_a_short_figure_leaves_room_for_the_legend(
    position: ub.LegendPosition,
) -> None:
    def build(height: float) -> ub.Figure:
        fig = ub.Figure(6, height)
        ax = fig.add_axes().title("One\nTwo").line([0, 1], [0, 1], label="S")
        ax.legend(ub.LegendStyle.HEADER, position=position)
        return fig

    err = _raised(ub.StyleError, lambda: build(0.5).to_png())
    found = re.search(r"at least (\d+) pt", str(err))
    assert found is not None, err
    assert build(int(found.group(1)) / 72).to_png().startswith(b"\x89PNG")
