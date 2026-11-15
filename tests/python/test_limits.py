"""Limits fixed at one end with ``ub.AUTO``, and whole-number ticks on integer data."""

import datetime as dt
from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"
LIMIT_HINT = "pass a number such as 1.5, or ub.AUTO for an automatic limit."
BOTH_AUTO = "low and high are both AUTO; at least one limit must be set."
BOTH_AUTO_HINT = "pass a value for low or high; without a call both limits are automatic."


def texts(ax: ub.Axes[Any, Any]) -> list[str]:
    """Every editable SVG text of the figure of ``ax``, in drawing order."""
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return [node.text or "" for node in root.iter(f"{SVG}text")]


def numbers(ax: ub.Axes[Any, Any]) -> list[float]:
    """The texts of the figure of ``ax`` that read as numbers: its numeric tick labels."""
    out: list[float] = []
    for text in texts(ax):
        try:
            out.append(float(text.replace("−", "-")))
        except ValueError:
            continue
    return out


def test_years_take_whole_steps_without_decimals() -> None:
    years = list(range(2015, 2027))
    ax = ub.Figure(6, 3).add_axes().line(years, [1.0] * len(years))
    labels = [t for t in texts(ax) if t.startswith("20")]
    assert len(labels) >= 2, labels
    assert all("." not in label for label in labels), labels


def test_integer_counts_on_a_bar_axis_take_whole_steps() -> None:
    ax = ub.Figure(5, 4).add_axes(x=ub.Scale.category()).bar(["a", "b", "c"], [1, 3, 2])
    assert numbers(ax) == [0.0, 1.0, 2.0, 3.0]


def test_a_fraction_keeps_the_round_steps() -> None:
    whole = ub.Figure(5, 4).add_axes().line([0, 1, 2], [0, 1, 0])
    assert sorted(set(numbers(whole))) == [0.0, 1.0, 2.0]
    fraction = ub.Figure(5, 4).add_axes().line([0, 1, 2], [0, 1, 0.5])
    assert any(value % 1 for value in numbers(fraction))


@pytest.mark.parametrize("name", ["xlim", "ylim"])
def test_one_auto_limit_fixes_the_other(name: str) -> None:
    ax = ub.Figure(5, 4).add_axes().line([1, 2, 3], [4, 6, 5])
    before = ax.figure.to_svg()
    limit = cast(Callable[..., object], getattr(ax, name))
    assert limit(0, ub.AUTO) is ax
    low = ax.figure.to_svg()
    assert low != before
    assert limit(ub.AUTO, 10) is ax
    assert ax.figure.to_svg() not in (before, low)


def test_a_fixed_low_starts_the_axis_there() -> None:
    ax = ub.Figure(5, 4).add_axes().line([0, 1, 2], [41.5, 47.8, 49.3]).ylim(0, ub.AUTO)
    assert min(numbers(ax)) == 0.0
    assert max(numbers(ax)) >= 40.0


def test_both_auto_is_refused() -> None:
    ax = ub.Figure().add_axes()
    with pytest.raises(ub.UsageError) as caught:
        ax.ylim(ub.AUTO, ub.AUTO)
    assert str(caught.value) == f"Axes.ylim(): {BOTH_AUTO}"
    assert details(caught.value) == ("low", BOTH_AUTO_HINT)


@pytest.mark.parametrize(
    ("low", "high", "parameter"), [("0", ub.AUTO, "low"), (ub.AUTO, None, "high")]
)
def test_the_fixed_limit_must_be_a_number(low: object, high: object, parameter: str) -> None:
    with pytest.raises(TypeError) as caught:
        cast(Callable[..., object], ub.Figure().add_axes().xlim)(low, high)
    assert str(caught.value).startswith(f"Axes.xlim(): {parameter} is ")
    assert str(caught.value).endswith(f"{parameter} must be a float or AUTO.")
    assert details(caught.value) == (parameter, LIMIT_HINT)


def test_a_fixed_limit_is_checked_at_once() -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.log())
    with pytest.raises(ub.StyleError) as caught:
        ax.ylim(0, ub.AUTO)
    assert str(caught.value) == "Axes.ylim(): low is 0; a log scale needs positive limits."
    assert details(caught.value) == ("low", "use a linear axis if a limit is not positive.")
    with pytest.raises(ub.StyleError) as infinite:
        ub.Figure().add_axes().xlim(ub.AUTO, float("inf"))
    assert str(infinite.value) == "Axes.xlim(): high is inf; a limit must be finite."


def test_a_category_axis_refuses_one_limit() -> None:
    ax: ub.Axes[Any, Any] = ub.Figure().add_axes(x=ub.Scale.category())
    with pytest.raises(ub.UsageError) as caught:
        ax.xlim(0, ub.AUTO)
    assert str(caught.value) == (
        "Axes.xlim(): this is a category axis; limits apply to linear and log axes only."
    )


def test_data_beyond_the_fixed_limit_fails_the_render() -> None:
    ax = ub.Figure().add_axes().line([0, 1], [2, 3]).ylim(10, ub.AUTO)
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert str(caught.value) == (
        "Figure.to_svg(): the y limits are (10, AUTO), and every y value lies below 10, the "
        "nearest at 3; the lower limit must leave some data on the plot."
    )
    assert details(caught.value) == (
        None,
        "lower the limit to reach the data, or set both limits with ylim().",
    )


def test_a_time_axis_takes_one_fixed_time() -> None:
    days = [dt.date(2026, 1, d) for d in (5, 6, 8, 9)]
    ax = ub.Figure(6, 3).add_axes(x=ub.Scale.time()).line(days, [1.0, 2.0, 1.5, 3.0])
    before = ax.figure.to_svg()
    assert ax.xlim(dt.date(2026, 1, 1), ub.AUTO) is ax
    assert ax.figure.to_svg() != before
    ax.xlim(ub.AUTO, dt.date(2026, 1, 20))
    ax.figure.to_svg()
    with pytest.raises(ub.StyleError, match="every x value lies above"):
        ax.xlim(ub.AUTO, dt.date(2025, 12, 1)).figure.to_svg()
    with pytest.raises(TypeError):
        cast(Callable[..., object], ax.xlim)(ub.AUTO, 3.0)


def test_a_shared_one_sided_limit_conflict_names_both() -> None:
    fig = ub.Figure(8, 3, rows=1, cols=2, share_x=ub.Share.ALL)
    fig.add_axes(row=0, col=0).line([0, 1], [1, 2]).xlim(0, ub.AUTO)
    right = fig.add_axes(row=0, col=1).line([0, 1], [1, 2])
    with pytest.raises(ub.UsageError) as caught:
        right.xlim(ub.AUTO, 5)
    assert str(caught.value) == (
        "Axes.xlim(): x limits are shared with row 0, col 0, which set (0, AUTO)."
    )


def test_value_labels_widen_the_automatic_end() -> None:
    ax = ub.Figure(5, 4).add_axes(x=ub.Scale.category())
    ax.bar(["a", "b", "c"], [1, 3, 2], annotate=True).ylim(0, ub.AUTO)
    assert "3" in texts(ax)


def test_a_hexbin_holds_whole_cells_beyond_its_fixed_end() -> None:
    ax = ub.Figure(6, 4).add_axes()
    ax.hexbin(list(range(20)), [i % 7 for i in range(20)], gridsize=7).xlim(0, ub.AUTO)
    assert max(numbers(ax)) >= 20


def test_a_limit_without_room_for_the_other_is_refused() -> None:
    ax = ub.Figure().add_axes()
    with pytest.raises(ub.StyleError) as caught:
        ax.ylim(1e300, ub.AUTO)
    assert str(caught.value) == (
        "Axes.ylim(): low is 1e300; it leaves no room for an automatic limit above it within "
        "the supported values."
    )
    assert details(caught.value) == (
        "low",
        "set both limits, or move this one away from the end of the range.",
    )
