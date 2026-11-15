"""``Between``: reference lines and spans between neighbouring categories of a category axis."""

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
BETWEEN_HINT = "pass Between(a, b), with a and b two neighbouring categories."
NEIGHBOURS_HINT = "use two neighbouring categories."
YEARS = ["2012", "2013", "2014", "2015"]


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def years() -> ub.Axes[str, float]:
    return ub.Figure(4, 3).add_axes(x=ub.Scale.category()).bar(YEARS, [3, 4, 6, 5])


def rows() -> ub.Axes[float, str]:
    return ub.Figure(4, 3).add_axes(y=ub.Scale.category()).bar(["N", "S", "E"], [3, 4, 6])


def svg_paths(figure: ub.Figure) -> list[str]:
    root = ElementTree.fromstring(figure.to_svg())
    return [node.get("d", "") for node in root.iter(f"{SVG}path")]


def test_value_semantics() -> None:
    a = ub.Between("2013", "2014")
    assert (a.first, a.second) == ("2013", "2014")
    assert repr(a) == "Between('2013', '2014')"
    assert a == ub.Between("2013", "2014")
    assert a != ub.Between("2014", "2013")
    assert (a == ("2013", "2014")) is False
    assert hash(a) == hash(ub.Between("2013", "2014"))
    assert copy.copy(a) is a and copy.deepcopy(a) is a
    for protocol in range(pickle.HIGHEST_PROTOCOL + 1):
        restored: object = pickle.loads(pickle.dumps(a, protocol))
        assert restored == a
    with pytest.raises(AttributeError):
        call(a.__setattr__)("first", "x")
    with pytest.raises(AttributeError):
        call(a.__delattr__)("first")
    # A name may break into lines, as the name of a category may.
    assert ub.Between("2026\n(to Oct)", "2025").first == "2026\n(to Oct)"


@pytest.mark.parametrize(
    ("first", "second", "cls", "message", "parameter"),
    [
        (1, "a", TypeError, "first is int; first must be a str.", "first"),
        ("a", None, TypeError, "second is None; second must be a str.", "second"),
        (
            "a\tb",
            "c",
            ub.DataError,
            "first holds U+0009 (a tab) at index 1; text cannot hold control characters.",
            "first",
        ),
        (
            "2013",
            "2013",
            ub.UsageError,
            'first and second are both "2013"; a point between categories needs two categories.',
            "second",
        ),
        ("a", "x" * 1001, ub.StyleError, "second has 1,001 characters", "second"),
    ],
)
def test_a_between_needs_two_different_category_names(
    first: object, second: object, cls: type[Exception], message: str, parameter: str
) -> None:
    with pytest.raises(cls) as caught:
        call(ub.Between)(first, second)
    assert type(caught.value) is cls
    assert str(caught.value).startswith(f"Between(): {message}")
    found = details(caught.value)
    assert found is not None and found[0] == parameter


def test_a_divider_sits_halfway_between_the_two_categories() -> None:
    plain = years()
    divided = years().vline(ub.Between("2014", "2013"), dash=ub.Dash.DASHED, label="Policy")
    assert divided.figure.to_svg() != plain.figure.to_svg()
    assert "Policy" in divided.figure.to_svg(text=ub.SvgText.EDITABLE)
    # The same divider at the band edge, drawn through the native layer, is the same picture.
    again = years().vline(ub.Between("2013", "2014"), dash=ub.Dash.DASHED, label="Policy")
    assert again.figure.to_svg() == divided.figure.to_svg()


def test_spans_cover_whole_categories_on_either_category_axis() -> None:
    ax = years().vspan(ub.Between("2012", "2013"), ub.Between("2014", "2015"), label="Window")
    assert "Window" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    h = rows().hline(ub.Between("N", "S")).hspan(ub.Between("N", "S"), ub.Between("S", "E"))
    assert h.figure.to_png()


def test_rows_of_events_on_a_time_axis_take_a_divider_and_a_span() -> None:
    day = np.datetime64("2026-03-01")
    one = np.timedelta64(1, "D")
    ax = ub.Figure(5, 3).add_axes(x=ub.Scale.time(), y=ub.Scale.category())
    ax.events(["Build", "Test", "Deploy"], [[day, day + one], [day + 2 * one], [day + 3 * one]])
    plain = ax.figure.to_svg()
    ax.hline(ub.Between("Test", "Build"), dash=ub.Dash.DASHED, label="Release")
    ax.hspan(ub.Between("Build", "Test"), ub.Between("Test", "Deploy"), label="Testing")
    drawn = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert drawn != plain
    assert "Release" in drawn and "Testing" in drawn
    assert ax.figure.to_png()
    # Two category axes still take no divider.
    heat = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    with pytest.raises(ub.UsageError, match="needs a numeric x axis"):
        call(heat.hline)(ub.Between("a", "b"))


def test_a_between_needs_a_category_axis() -> None:
    cases: list[tuple[Callable[[], object], str, str]] = [
        (
            lambda: call(ub.Figure().add_axes().vline)(ub.Between("a", "b")),
            "Axes.vline(): x is a point between categories but the x axis is a linear axis; "
            "Between needs a category axis.",
            "x",
        ),
        (
            lambda: call(ub.Figure().add_axes(x=ub.Scale.time()).vline)(ub.Between("a", "b")),
            "Axes.vline(): x is a point between categories but the x axis is a time axis; "
            "Between needs a category axis.",
            "x",
        ),
        (
            lambda: call(ub.Figure().add_axes(x=ub.Scale.log()).vspan)(
                ub.Between("a", "b"), ub.Between("b", "c")
            ),
            "Axes.vspan(): low is a point between categories but the x axis is a log axis; "
            "Between needs a category axis.",
            "low",
        ),
        (
            lambda: call(ub.Figure().add_axes().hline)(ub.Between("a", "b")),
            "Axes.hline(): y is a point between categories but the y axis is a linear axis; "
            "Between needs a category axis.",
            "y",
        ),
    ]
    for make, message, parameter in cases:
        with pytest.raises(ub.UsageError) as caught:
            make()
        assert str(caught.value) == message
        found = details(caught.value)
        assert found is not None and found[0] == parameter


def test_a_number_on_a_category_axis_points_to_between() -> None:
    cases: list[tuple[Callable[[], object], str, str]] = [
        (
            lambda: call(rows().hline)(2.0),
            "Axes.hline(): y is a number but the y axis is a category axis; place it between "
            "two categories instead.",
            "y",
        ),
        (
            lambda: call(rows().hspan)(1.0, 2.0),
            "Axes.hspan(): low is a number but the y axis is a category axis; place it between "
            "two categories instead.",
            "low",
        ),
    ]
    for make, message, parameter in cases:
        with pytest.raises(ub.UsageError) as caught:
            make()
        assert str(caught.value) == message
        assert details(caught.value) == (parameter, BETWEEN_HINT)


def test_both_ends_of_a_span_between_categories_are_between_positions() -> None:
    with pytest.raises(TypeError) as caught:
        call(years().vspan)(ub.Between("2012", "2013"), 2.0)
    assert str(caught.value) == (
        "Axes.vspan(): high is float; a span between categories needs a Between position at "
        "both ends."
    )
    assert details(caught.value) == (
        "high",
        'pass both ends as Between positions, e.g. ub.Between("2013", "2014").',
    )


def test_two_category_axes_take_no_divider() -> None:
    heat = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    with pytest.raises(ub.UsageError) as caught:
        call(heat.vline)(ub.Between("a", "b"))
    assert str(caught.value) == (
        "Axes.vline(): x is a category scale; Axes.vline() needs a numeric x axis."
    )


RENDER_ERRORS: list[tuple[Callable[[ub.Axes[str, float]], object], type[Exception], str]] = [
    (
        lambda ax: ax.vline(ub.Between("2015", "2016")),
        ub.DataError,
        'x is "2016"; no mark on this axes has that category.',
    ),
    (
        lambda ax: ax.vline(ub.Between("2012", "2014")),
        ub.UsageError,
        'x is Between("2012", "2014"), whose categories are not neighbours: "2012" is at '
        'index 0 and "2014" at index 2 of the x axis\'s categories.',
    ),
    (
        lambda ax: ax.vspan(ub.Between("2014", "2015"), ub.Between("2012", "2013")),
        ub.StyleError,
        'low is Between("2014", "2015"), high is Between("2012", "2013"); low must come '
        "before high along the x axis.",
    ),
]


@pytest.mark.parametrize(("add", "cls", "message"), RENDER_ERRORS)
def test_names_are_checked_when_the_figure_is_rendered(
    add: Callable[[ub.Axes[str, float]], object], cls: type[Exception], message: str
) -> None:
    ax = years()
    add(ax)
    renders: list[tuple[str, Callable[[], object]]] = [
        ("Figure.to_png()", ax.figure.to_png),
        ("Figure.to_svg()", ax.figure.to_svg),
    ]
    for context, render in renders:
        with pytest.raises(cls) as caught:
            render()
        assert str(caught.value) == f"{context}: {message}"
        found = details(caught.value)
        assert found is not None and found[0] is None


def test_a_divider_before_the_marks_that_bring_its_categories_renders() -> None:
    ax = ub.Figure(4, 3).add_axes(x=ub.Scale.category()).vline(ub.Between("b", "a"))
    with pytest.raises(ub.DataError):
        ax.figure.to_svg()
    ax.bar(["a", "b"], [1, 2])
    assert ax.figure.to_svg()


def test_shared_category_axes_look_up_the_categories_of_every_panel() -> None:
    fig = ub.Figure(8, 3, cols=2, share_x=ub.Share.ALL)
    fig.add_axes(col=0, x=ub.Scale.category()).bar(["2012", "2013"], [1, 2]).vline(
        ub.Between("2013", "2014")
    )
    fig.add_axes(col=1, x=ub.Scale.category()).bar(["2014", "2015"], [3, 4])
    assert fig.to_svg()


def test_the_divider_matches_the_native_figure() -> None:
    from umber import _umber

    fig = ub.Figure(4, 3, theme=ub.Theme.MIDNIGHT)
    ax = fig.add_axes(x=ub.Scale.category()).bar(["Q1", "Q2", "Q3"], [3, 1, 2])
    ax.vline(ub.Between("Q1", "Q2"), label="Go")
    ax.vspan(ub.Between("Q1", "Q2"), ub.Between("Q2", "Q3"))
    native = _umber.Figure(4.0, 3.0, _umber.Theme.midnight(), 1, 1, 0, 0, 0, 0)
    native.add_axes(0, 0, 0, 0, 1, 1, 2, 10.0, 0, 10.0)
    values: Any = __import__("numpy").array([3.0, 1.0, 2.0])
    native.bar(0, 0, ["Q1", "Q2", "Q3"], values, "", (0, 0, 1.0), False, None, (0, 0.0))
    native.vline_between(0, 0, ("Q1", "Q2"), "Go", (0, 0, 1.0), 0, (0, 0.0))
    native.vspan_between(0, 0, ("Q1", "Q2"), ("Q2", "Q3"), "", (0, 0, 1.0))
    assert fig.to_svg() == native.to_svg(0, 300)


def test_a_category_name_with_a_line_break_is_one_key_everywhere() -> None:
    name = "2026\n(to Oct)"
    ax = ub.Figure(5, 3).add_axes(x=ub.Scale.category()).bar(["2025", name], [4, 3])
    ax.line([name], [3.5]).vline(name).vline(ub.Between("2025", name))
    ax.text("partial year", (name, 3.0))
    ax.xtick_highlight([name])
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    texts = [node.text for node in root.iter(f"{SVG}text")]
    assert "2026" in texts and "(to Oct)" in texts
    assert name not in texts
