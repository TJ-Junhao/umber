"""Value labels on scatter points: the value, format, size, color, place, errors and formats."""

import math
from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"
NO_ROOM_HINT = (
    "use place=Place.ABOVE (or another side) to fix the side, a larger figure, or a smaller "
    "label size."
)


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def nodes(ax: ub.Axes[Any, Any]) -> list[ElementTree.Element]:
    """Every editable SVG text of the figure of ``ax``, in drawing order."""
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return list(root.iter(f"{SVG}text"))


def texts(ax: ub.Axes[Any, Any]) -> dict[str, ElementTree.Element]:
    """Every editable SVG text of the figure of ``ax`` by its content; the last one wins, so a
    value label wins over a tick label that reads the same."""
    return {node.text or "": node for node in nodes(ax)}


def position(node: ElementTree.Element) -> tuple[float, float]:
    return float(node.get("x") or "nan"), float(node.get("y") or "nan")


def limited() -> ub.Axes[float, float]:
    return ub.Figure().add_axes().xlim(0, 10).ylim(0, 10)


def test_the_value_is_the_coordinate_on_the_numeric_axis() -> None:
    rows = ub.Figure().add_axes(y=ub.Scale.category())
    rows = rows.scatter([71.25, 74.5], ["Base", "Gate"], annotate=True)
    assert {"71.25", "74.5"} <= set(texts(rows))
    columns = ub.Figure().add_axes(x=ub.Scale.category())
    columns = columns.scatter(["A", "B"], [3.5, 6.25], annotate=True)
    assert {"3.5", "6.25"} <= set(texts(columns))
    plain = ub.Figure().add_axes().scatter([1.5, 2.5], [10.25, 20.75], annotate=True)
    assert {"10.25", "20.75"} <= set(texts(plain))
    assert "1.5" not in texts(plain)


def test_two_category_axes_are_refused_when_the_call_is_made() -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    with pytest.raises(ub.UsageError):
        call(ax.scatter)(["A"], ["B"], annotate=True)


def test_times_are_written_in_the_axis_format() -> None:
    days = np.array(["2026-03-03", "2026-03-17"], dtype="datetime64[D]")
    ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.time())
    ax = ax.scatter(["A", "B"], days, annotate=True)
    assert {"Mar 3", "Mar 17"} <= set(texts(ax))
    iso = ub.Figure().add_axes(y=ub.Scale.time())
    iso = iso.scatter([1, 2], days, annotate=True, format=ub.TickFormat.iso())
    assert {"2026-03-03", "2026-03-17"} <= set(texts(iso))


def test_format_auto_follows_the_value_axis_and_an_explicit_format_wins() -> None:
    ax = ub.Figure().add_axes().ytick_format(ub.TickFormat.percent(0))
    ax = ax.scatter([1, 2], [0.25, 0.5], annotate=True)
    assert {"25%", "50%"} <= set(texts(ax))
    fixed = ub.Figure().add_axes().ytick_format(ub.TickFormat.percent(0))
    fixed = fixed.scatter([1, 2], [0.25, 0.5], annotate=True, format=ub.TickFormat.fixed(2))
    assert {"0.25", "0.50"} <= set(texts(fixed))


def test_label_size() -> None:
    ax = ub.Figure().add_axes().scatter([1], [4.25], annotate=True, label_size=ub.Size.pt(13))
    assert "13" in (texts(ax)["4.25"].get("font-size") or "")


def test_colors_follow_the_series() -> None:
    red = ub.Color.hex("#C81E28")
    ax = ub.Figure().add_axes().scatter([1, 2], [1.25, 2.25], color=red, annotate=True)
    assert texts(ax)["1.25"].get("fill") == "#C81E28"
    assert texts(ax)["1.25"].get("fill-opacity") in (None, "1")
    grouped = (
        ub.Figure().add_axes().scatter([1, 2], [3.25, 4.25], by=["North", "South"], annotate=True)
    )
    found = texts(grouped)
    assert found["3.25"].get("fill") != found["4.25"].get("fill")
    mapped = (
        ub.Figure()
        .add_axes()
        .scatter([1, 2], [5.25, 6.25], color=ub.ColorBy([0.0, 1.0]), annotate=True)
    )
    found = texts(mapped)
    assert found["5.25"].get("fill") == found["6.25"].get("fill")


def test_each_fixed_side() -> None:
    sides: dict[ub.Place, tuple[float, float]] = {}
    for place in [ub.Place.ABOVE, ub.Place.BELOW, ub.Place.LEFT, ub.Place.RIGHT]:
        ax = limited().scatter([5], [5.25], annotate=True, place=place)
        sides[place] = position(texts(ax)["5.25"])
    above, below, left, right = (
        sides[ub.Place.ABOVE],
        sides[ub.Place.BELOW],
        sides[ub.Place.LEFT],
        sides[ub.Place.RIGHT],
    )
    assert above[1] < below[1] and math.isclose(above[0], below[0], abs_tol=1e-3)
    assert left[0] < right[0] and math.isclose(left[1], right[1], abs_tol=1e-3)


def test_fixed_sides_grow_the_limits_and_auto_does_not() -> None:
    for place in [ub.Place.ABOVE, ub.Place.BELOW, ub.Place.LEFT, ub.Place.RIGHT]:
        ax = (
            ub.Figure(3, 1.6).add_axes().scatter([0, 5, 10], [0, 3, 10], annotate=True, place=place)
        )
        ax.figure.to_svg()
    plain = ub.Figure().add_axes().scatter([0, 10], [0, 10])
    auto = ub.Figure().add_axes().scatter([0, 10], [0, 10], annotate=True, place=ub.Place.AUTO)
    ticks = [n.text for n in nodes(plain)]
    assert [n.text for n in nodes(auto)][: len(ticks)] == ticks


def test_a_fixed_side_beyond_explicit_limits_is_a_style_error() -> None:
    ax = ub.Figure().add_axes().ylim(0, 10)
    ax = ax.scatter([1, 2], [5, 9.9], label="Long", annotate=True, place=ub.Place.ABOVE)
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert str(caught.value) == (
        'Figure.to_svg(): the value label "9.9" of the series "Long" does not fit inside the plot.'
    )


def test_auto_avoids_error_bars() -> None:
    free = limited().scatter([5], [5.25], annotate=True)
    blocked = limited().y_error_bars([5], [3], [7]).scatter([5], [5.25], annotate=True)
    (free_x, free_y), (blocked_x, blocked_y) = (
        position(texts(free)["5.25"]),
        position(texts(blocked)["5.25"]),
    )
    # Above, the label is centred on the point; on the right it starts past the point, level
    # with it.
    assert blocked_x > free_x
    assert blocked_y > free_y


def test_no_clear_position_is_a_style_error_with_a_hint() -> None:
    ax = limited()
    grid = [(5 + 0.12 * i, 5 + 0.12 * j) for i in range(-6, 7) for j in range(-6, 7)]
    ax = ax.scatter([x for x, _ in grid], [y for _, y in grid])
    ax = ax.scatter([5], [5], label="Long training", annotate=True)
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_png()
    assert str(caught.value) == (
        'Figure.to_png(): the value label "5" of the series "Long training" has no clear '
        "position: above, right, below and left of its point each leave the plot or overlap a "
        "mark or a label."
    )
    assert details(caught.value) == (None, NO_ROOM_HINT)


def test_label_options_need_annotate() -> None:
    for keyword, value in [
        ("format", ub.TickFormat.fixed(1)),
        ("label_size", ub.Size.pt(9)),
        ("place", ub.Place.BELOW),
    ]:
        ax = ub.Figure().add_axes()
        with pytest.raises(ub.UsageError) as caught:
            call(ax.scatter)([1], [2], **{keyword: value})
        info = details(caught.value)
        assert info is not None and info[0] == keyword
        with pytest.raises(ub.UsageError):
            call(ax.scatter)([1, 2], [2, 3], by=["a", "b"], **{keyword: value})
        assert ax.figure.to_svg() == ub.Figure().add_axes().figure.to_svg()


def test_wrong_option_types_are_refused() -> None:
    for keyword, value in [
        ("annotate", 1),
        ("format", "%.1f"),
        ("label_size", 9),
        ("place", "above"),
    ]:
        with pytest.raises(TypeError) as caught:
            call(ub.Figure().add_axes().scatter)([1], [2], **{keyword: value})
        info = details(caught.value)
        assert info is not None and info[0] == keyword


def test_formats_must_suit_the_value_axis() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Figure().add_axes().scatter([1], [2], annotate=True, format=ub.TickFormat.iso())
    info = details(caught.value)
    assert info is not None and info[0] == "format"
    days = np.array(["2026-03-03"], dtype="datetime64[D]")
    ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.time())
    with pytest.raises(ub.StyleError):
        ax.scatter(["A"], days, annotate=True, format=ub.TickFormat.fixed(1))


def test_points_not_drawn_have_no_label() -> None:
    ax = limited().scatter([1, math.nan, 3], [1.25, 2.25, math.nan], annotate=True)
    found = texts(ax)
    assert "1.25" in found and "2.25" not in found
    mapped = limited().scatter(
        [1, 2], [3.25, 4.25], color=ub.ColorBy([0.0, math.nan]), annotate=True
    )
    assert "4.25" not in texts(mapped)
    outside = limited().scatter([5], [12.5], annotate=True)
    assert "12.5" not in texts(outside)


def test_png_svg_and_pdf_agree() -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.category())
    ax = ax.x_error_bars(["Base", "Gate"], [69.0, 72.0], [73.5, 77.0])
    ax = ax.scatter([71.25, 74.5], ["Base", "Gate"], annotate=True)
    editable = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert ">71.25</text>" in editable and ">74.5</text>" in editable
    assert ax.figure.to_png().startswith(b"\x89PNG")
    assert ax.figure.to_pdf().startswith(b"%PDF")
    again = ub.Figure().add_axes(y=ub.Scale.category())
    again = again.x_error_bars(["Base", "Gate"], [69.0, 72.0], [73.5, 77.0])
    again = again.scatter([71.25, 74.5], ["Base", "Gate"], annotate=True)
    assert again.figure.to_svg() == ax.figure.to_svg()
    assert again.figure.to_png() == ax.figure.to_png()


def test_annotate_false_changes_nothing() -> None:
    plain = ub.Figure().add_axes().scatter([1, 2], [3, 4])
    off = ub.Figure().add_axes().scatter([1, 2], [3, 4], annotate=False)
    assert plain.figure.to_svg() == off.figure.to_svg()
    assert plain.figure.to_png() == off.figure.to_png()


def test_quick_scatter_takes_value_labels() -> None:
    ax = ub.scatter([1, 2], [3.25, 4.75], annotate=True, place=ub.Place.RIGHT)
    assert {"3.25", "4.75"} <= set(texts(ax))
    with pytest.raises(ub.UsageError):
        ub.scatter([1], [2], place=ub.Place.RIGHT)


def test_place_members() -> None:
    assert [(m.name, m.value) for m in ub.Place] == [
        ("AUTO", 0),
        ("ABOVE", 1),
        ("BELOW", 2),
        ("LEFT", 3),
        ("RIGHT", 4),
        ("COLUMN", 5),
    ]
