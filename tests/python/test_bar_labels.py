"""Value labels on plain, grouped and stacked bars: text, format, size, room and errors."""

from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def texts(ax: ub.Axes[Any, Any]) -> dict[str, ElementTree.Element]:
    """Every editable SVG text of the figure of ``ax``, by its content."""
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return {node.text or "": node for node in root.iter(f"{SVG}text")}


def vertical() -> ub.Axes[str, float]:
    return ub.Figure().add_axes(x=ub.Scale.category())


def horizontal() -> ub.Axes[float, str]:
    return ub.Figure().add_axes(y=ub.Scale.category())


SERIES = [ub.Series("Online", [24.5, 31.2]), ub.Series("Retail", [18.2, 26.3])]


def test_bar_labels_vertical_horizontal_grouped_stacked() -> None:
    up = vertical().bar(["A", "B", "C"], [28.4, 42.0, -16.5], annotate=True)
    assert {"28.4", "42", "−16.5"} <= set(texts(up))
    side = horizontal().bar(["A", "B"], [28.4, 52.8], annotate=True)
    assert {"28.4", "52.8"} <= set(texts(side))
    grouped = vertical().grouped_bar(["Q1", "Q2"], SERIES, annotate=True)
    assert {"24.5", "31.2", "18.2", "26.3"} <= set(texts(grouped))
    stacked = vertical().stacked_bar(
        ["Q1", "Q2"],
        [ub.Series("Core", [32, 38]), ub.Series("Other", [18, 22])],
        annotate=True,
    )
    found = texts(stacked)
    assert {"32", "38", "18", "22"} <= set(found)
    assert {found[label].get("fill") for label in ["32", "18"]} <= {"#000000", "#FFFFFF"}


def test_labels_are_off_by_default() -> None:
    plain = vertical().bar(["A"], [3])
    assert plain.figure.to_svg() == vertical().bar(["A"], [3], annotate=False).figure.to_svg()


def test_format_and_size() -> None:
    ax = vertical().bar(
        ["A"], [3], annotate=True, format=ub.TickFormat.fixed(2), size=ub.Size.pt(11)
    )
    node = texts(ax)["3.00"]
    assert "11" in (node.get("font-size") or "")
    percent = vertical().ytick_format(ub.TickFormat.percent()).bar(["A"], [0.25], annotate=True)
    assert "25%" in texts(percent)


def test_format_and_size_need_annotate() -> None:
    for keyword, value in [("format", ub.TickFormat.fixed(1)), ("size", ub.Size.pt(9))]:
        ax = vertical()
        with pytest.raises(ub.UsageError) as caught:
            call(ax.bar)(["A"], [1], **{keyword: value})
        info = details(caught.value)
        assert info is not None and info[0] == keyword and info[1]
        with pytest.raises(ub.UsageError):
            call(ax.stacked_bar)(["A"], [ub.Series("s", [1])], **{keyword: value})
        assert ax.figure.to_svg() == vertical().figure.to_svg()


def test_wrong_option_types_are_refused() -> None:
    for keyword, value in [("annotate", 1), ("format", "%.1f"), ("size", 9)]:
        for method in ["bar", "grouped_bar", "stacked_bar"]:
            ax = vertical()
            data: list[object] = [["A"], [1]] if method == "bar" else [["A"], [ub.Series("s", [1])]]
            with pytest.raises(TypeError) as caught:
                call(getattr(ax, method))(*data, **{keyword: value})
            info = details(caught.value)
            assert info is not None and info[0] == keyword


def test_iso_format_is_refused() -> None:
    with pytest.raises(ub.StyleError) as caught:
        vertical().bar(["A"], [1], annotate=True, format=ub.TickFormat.iso())
    info = details(caught.value)
    assert info is not None and info[0] == "format"


def test_automatic_limits_grow_to_hold_the_labels() -> None:
    ax = ub.Figure(4.7, 2.55).add_axes(x=ub.Scale.category())
    ax.bar(["A", "B", "C", "D", "E", "F"], [50, 60, 80, 70, 65, 97], annotate=True)
    assert "97" in texts(ax)


def test_label_beyond_explicit_limits_names_the_bar() -> None:
    ax = vertical().ylim(0, 100).bar(["A", "B"], [50, 99], annotate=True)
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert str(caught.value) == (
        'Figure.to_svg(): the value label "99" of the bar "B" does not fit inside the plot.'
    )
    assert details(caught.value) == (
        None,
        "widen ylim (or xlim), or let the limits be automatic.",
    )


def test_stacked_label_that_does_not_fit_names_the_segment() -> None:
    ax = vertical().stacked_bar(
        ["Q1", "Q2"],
        [ub.Series("Core", [32, 38]), ub.Series("Service", [21, 0.8])],
        annotate=True,
    )
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert str(caught.value).startswith(
        'Figure.to_svg(): the value label "0.8" of the series "Service" in the bar "Q2" needs '
    )


def test_automatic_labels_write_each_value_on_its_own() -> None:
    ax = vertical().bar(["A", "B"], [0.12345, 0.9], annotate=True)
    assert {"0.12345", "0.9"} <= set(texts(ax))


def test_quick_bar_takes_value_labels() -> None:
    ax = ub.bar(["A", "B"], [3.5, 2], annotate=True, format=ub.TickFormat.fixed(1))
    assert {"3.5", "2.0"} <= set(texts(ax))
    sized = ub.bar(["A"], [3], annotate=True, size=ub.Size.pt(11))
    assert "11" in (texts(sized)["3"].get("font-size") or "")
    with pytest.raises(ub.UsageError):
        ub.bar(["A"], [3], format=ub.TickFormat.fixed(1))


def test_tiny_stacked_segment_reports_the_room_error() -> None:
    ax = vertical().stacked_bar(
        ["A"],
        [ub.Series("Large", [100]), ub.Series("Small", [0.01])],
        annotate=True,
        format=ub.TickFormat.fixed(2),
    )
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert str(caught.value).startswith(
        'Figure.to_svg(): the value label "0.01" of the series "Small" in the bar "A" needs '
    )
