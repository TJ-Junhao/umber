"""``Axes.hline()``, ``vline()``, ``hspan()`` and ``vspan()``: layers, type checks, data errors,
the category lookup at render time, and native parity."""

from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import pytest

import umber as ub
from umber import _color, _umber
from umber._errors import details

COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
DASH_HINT = "use Dash.SOLID, Dash.DASHED or Dash.DOTTED."
TEXT_HINT = "pass a str; convert other values with str()."
TOO_LONG_HINT = "shorten the text; it would not fit the figure."
SIZE_HINT = (
    "use Size.times(1.5) for a multiple of the theme's size, Size.pt(2.0) for points, or Size.AUTO."
)
REAL_HINT = "pass a number such as 1.5."
FINITE_HINT = "pass a finite number."
SWAP_HINT = "swap the two values."
CATEGORY_X_HINT = "pass x=Scale.category() to Figure.add_axes()."
LINEAR_X_HINT = "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."
LINEAR, CATEGORY = 0, 2
OUTLINES = 0
AUTO = (0, 0, 1.0)
NO_SIZE = (0, 0.0)
# A native figure of one cell (rows, cols, share_x, share_y, legend), and the position of its
# axes: row and column AUTO (grid index kind 0) spanning one cell.
ONE_CELL = (1, 1, 0, 0, 0)
ONLY_CELL = (0, 0, 0, 0, 1, 1)

type AnyAxes = ub.Axes[Any, Any]


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def _raised(tmp_path: Path, line: str, setup: str = "") -> BaseException:
    """Run ``line`` as user code in a file, so the error is reported at its call site."""
    path = tmp_path / f"user_code_{abs(hash((setup, line)))}.py"
    path.write_text(f"import umber as ub\n{setup}\n{line}\n")
    try:
        exec(compile(path.read_text(), str(path), "exec"), {})
    except Exception as err:
        return err
    raise AssertionError(f"{line} raised nothing")


def _axes(x: ub.Scale[Any] | None = None, y: ub.Scale[Any] | None = None) -> AnyAxes:
    fig = ub.Figure(3, 2)
    return fig.add_axes(x=x or ub.Scale.linear(), y=y or ub.Scale.linear())


def _call(ax: AnyAxes, name: str) -> Callable[..., object]:
    """The method ``name`` of ``ax`` with arguments of any type, for calls that must be refused."""
    return cast(Callable[..., object], getattr(ax, name))


def _parameter(err: BaseException) -> object:
    found = details(err)
    assert found is not None
    return found[0]


# Behaviour

CALLS: dict[str, Callable[[AnyAxes], object]] = {
    "hline": lambda ax: ax.hline(2.0),
    "vline": lambda ax: ax.vline(1.0),
    "hspan": lambda ax: ax.hspan(1.0, 2.0),
    "vspan": lambda ax: ax.vspan(0.5, 1.5),
}


@pytest.mark.parametrize("name", list(CALLS))
def test_each_method_returns_the_axes_and_draws(name: str) -> None:
    ax = _axes().line([0, 1, 2], [1.0, 3.0, 2.0])
    plain = ax.figure.to_svg()
    assert CALLS[name](ax) is ax
    assert ax.figure.to_svg() != plain
    assert getattr(ub.Axes, name).__module__ == "umber"
    assert not hasattr(ub, name)


@pytest.mark.parametrize("name", list(CALLS))
def test_options_are_keyword_only(name: str) -> None:
    count = 2 if name.endswith("span") else 1
    with pytest.raises(TypeError):
        _call(_axes(), name)(*[1.0] * count, "label")


def test_spans_are_drawn_below_data_and_rules_above_whatever_the_call_order() -> None:
    red, green, blue = ub.Color.RED, ub.Color.GREEN, ub.Color.BLUE

    def svg(rules_first: bool) -> str:
        ax = _axes()
        if rules_first:
            ax.hline(2.0, color=red).vline(1.0, color=green)
        ax.line([0, 1, 2], [1.0, 3.0, 2.0], color=blue)
        ax.vspan(0.5, 1.5, color=red).hspan(1.0, 2.0, color=green)
        if not rules_first:
            ax.hline(2.0, color=red).vline(1.0, color=green)
        return ax.figure.to_svg()

    assert svg(True) == svg(False)


def test_a_rule_or_a_span_extends_an_automatic_axis_and_explicit_limits_clip() -> None:
    base = _axes().line([0, 1, 2], [1.0, 3.0, 2.0])
    plain = base.figure.to_svg()
    assert _axes().line([0, 1, 2], [1.0, 3.0, 2.0]).hline(50.0).figure.to_svg() != plain
    assert _axes().line([0, 1, 2], [1.0, 3.0, 2.0]).vline(20.0).figure.to_svg() != plain
    clipped = _axes().line([0, 1, 2], [1.0, 3.0, 2.0]).ylim(0, 4).xlim(0, 2)
    reference = clipped.figure.to_svg()
    # Outside the explicit limits nothing of a rule or a span is drawn.
    far = clipped.hline(50.0).vline(20.0).hspan(10.0, 20.0).vspan(5.0, 6.0)
    assert far.figure.to_svg() == reference


def test_a_label_gives_an_entry_in_the_legend_row_under_every_style() -> None:
    for theme in (ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT):
        fig = ub.Figure(5, 3, theme=theme)
        fig.add_axes().line([0, 1, 2], [1.0, 3.0, 2.0], label="Signal").hline(
            2.0, label="Threshold"
        ).vspan(0.5, 1.0, label="Window")
        svg = fig.to_svg(text=ub.SvgText.EDITABLE)
        for label in ("Signal", "Threshold", "Window"):
            assert f">{label}</text>" in svg, (theme, label)
    ax = _axes().line([0, 1], [1.0, 2.0]).hline(1.5, label="Threshold").legend(ub.LegendStyle.NONE)
    assert ">Threshold</text>" not in ax.figure.to_svg(text=ub.SvgText.EDITABLE)


def test_a_vline_on_a_category_axis_takes_a_name_and_a_numeric_axis_takes_a_number() -> None:
    ax = _axes(ub.Scale.category()).bar(["Q1", "Q2", "Q3"], [1, 2, 3])
    assert ax.vline("Q2") is ax
    assert ax.figure.to_svg()
    numeric = _axes()
    assert numeric.vline(3) is numeric  # an int is a number


def test_a_vline_at_a_category_sits_at_its_band_centre() -> None:
    ax = _axes(ub.Scale.category()).bar(["Q1", "Q2", "Q3"], [1, 2, 3])
    ax.vline("Q2", color=ub.Color.RED)
    other = _axes(ub.Scale.category()).bar(["Q1", "Q2", "Q3"], [1, 2, 3])
    other.vline("Q3", color=ub.Color.RED)
    assert ax.figure.to_svg() != other.figure.to_svg()


# Errors


# (keyword, a wrong value, the expected type in the message, hint)
LABEL_ERROR = ("label", 3, "a str", TEXT_HINT)
COLOR_ERROR = ("color", "red", "of type Color", COLOR_HINT)
DASH_ERROR = ("dash", "--", "of type Dash", DASH_HINT)
WIDTH_ERROR = ("width", 2.0, "of type Size", SIZE_HINT)
RULE_ERRORS = [LABEL_ERROR, COLOR_ERROR, DASH_ERROR, WIDTH_ERROR]
SPAN_ERRORS = [LABEL_ERROR, COLOR_ERROR]
STYLE_ERRORS = [
    (name, args, *error)
    for name, args, errors in [
        ("hline", (1.0,), RULE_ERRORS),
        ("vline", (1.0,), RULE_ERRORS),
        ("hspan", (1.0, 2.0), SPAN_ERRORS),
        ("vspan", (1.0, 2.0), SPAN_ERRORS),
    ]
    for error in errors
]


@pytest.mark.parametrize(("name", "args", "keyword", "value", "expected", "hint"), STYLE_ERRORS)
def test_style_arguments_are_type_checked(
    name: str, args: tuple[float, ...], keyword: str, value: object, expected: str, hint: str
) -> None:
    ax = _axes().title("Kept").line([0, 1], [1, 2])
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        _call(ax, name)(*args, **{keyword: value})
    type_name = type(value).__name__
    assert str(caught.value) == (
        f"Axes.{name}(): {keyword} is {type_name}; {keyword} must be {expected}."
    )
    assert details(caught.value) == (keyword, hint)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("name", ["hspan", "vspan"])
def test_spans_have_no_dash_or_width(name: str) -> None:
    with pytest.raises(TypeError):
        _call(_axes(), name)(1.0, 2.0, dash=ub.Dash.DASHED)
    with pytest.raises(TypeError):
        _call(_axes(), name)(1.0, 2.0, width=ub.Size.AUTO)


@pytest.mark.parametrize(
    ("name", "args", "parameter"),
    [
        ("hline", ("a",), "y"),
        ("hline", (True,), "y"),
        ("hline", (None,), "y"),
        ("vline", ("a",), "x"),
        ("vline", ([1.0],), "x"),
        ("hspan", ("a", 1.0), "low"),
        ("hspan", (1.0, None), "high"),
        ("vspan", (b"1", 2.0), "low"),
        ("vspan", (1.0, "2"), "high"),
    ],
)
def test_values_must_be_numbers(name: str, args: tuple[object, ...], parameter: str) -> None:
    with pytest.raises(TypeError) as caught:
        _call(_axes(), name)(*args)
    assert details(caught.value) == (parameter, REAL_HINT)
    assert str(caught.value).startswith(f"Axes.{name}(): {parameter} is ")


def test_a_vline_x_follows_the_axis_kind() -> None:
    with pytest.raises(TypeError) as caught:
        _call(_axes(ub.Scale.category()), "vline")(1.0)
    assert str(caught.value) == "Axes.vline(): x is float; x must be a str."
    assert details(caught.value) == ("x", TEXT_HINT)
    with pytest.raises(TypeError) as caught:
        _call(_axes(), "vline")("Q1")
    assert str(caught.value) == "Axes.vline(): x is str; x must be a float."
    assert details(caught.value) == ("x", REAL_HINT)


def test_check_order_is_values_label_color_dash_width() -> None:
    bad: dict[str, Any] = {"label": 3, "color": "red", "dash": "--", "width": 2.0}
    order = list(bad)
    ax = _axes()
    for keyword in order:
        remaining = {k: bad[k] for k in order[order.index(keyword) :]}
        with pytest.raises(TypeError) as caught:
            _call(ax, "hline")(1.0, **remaining)
        assert _parameter(caught.value) == keyword
    # The value is checked first; a name too long is a value error after every type check.
    with pytest.raises(TypeError) as caught:
        _call(ax, "hline")("a", **bad)
    assert _parameter(caught.value) == "y"
    with pytest.raises(TypeError) as caught:
        _call(ax, "hspan")(1.0, "b", **{"label": 3})
    assert _parameter(caught.value) == "high"
    with pytest.raises(TypeError) as caught:
        _call(ax, "vline")(1.0, label="x" * 1001, color="red")
    assert _parameter(caught.value) == "color"


def test_labels_are_capped() -> None:
    for name, args in [("hline", (1.0,)), ("vline", (1.0,)), ("hspan", (1.0, 2.0))]:
        with pytest.raises(ub.StyleError) as caught:
            _call(_axes(), name)(*args, label="x" * 1001)
        assert details(caught.value) == ("label", TOO_LONG_HINT)


# (axes factory, method, args, class, message, parameter, hint)
ERRORS: list[tuple[Any, str, tuple[object, ...], type, str, object, object]] = [
    (
        _axes,
        "hline",
        (float("inf"),),
        ub.DataError,
        "Axes.hline(): y is inf; a reference line needs a finite value.",
        "y",
        FINITE_HINT,
    ),
    (
        _axes,
        "hline",
        (float("nan"),),
        ub.DataError,
        "Axes.hline(): y is NaN; a reference line needs a finite value.",
        "y",
        FINITE_HINT,
    ),
    (
        _axes,
        "hline",
        (1e301,),
        ub.DataError,
        "Axes.hline(): y is 1e301; magnitudes must not exceed 1e300.",
        "y",
        None,
    ),
    (
        lambda: _axes(y=ub.Scale.log()),
        "hline",
        (0,),
        ub.DataError,
        "Axes.hline(): y is 0 on a log axis; values must be positive.",
        "y",
        "use a linear axis, or pass a positive value.",
    ),
    (
        _axes,
        "vline",
        (float("-inf"),),
        ub.DataError,
        "Axes.vline(): x is -inf; a reference line needs a finite value.",
        "x",
        FINITE_HINT,
    ),
    (
        lambda: _axes(x=ub.Scale.log()),
        "vline",
        (-2.0,),
        ub.DataError,
        "Axes.vline(): x is -2 on a log axis; values must be positive.",
        "x",
        "use a linear axis, or pass a positive value.",
    ),
    (
        lambda: _axes(ub.Scale.category()),
        "vline",
        ("a\ud800b",),
        ub.DataError,
        "Axes.vline(): x has a lone surrogate at index 1; text must be valid Unicode.",
        "x",
        'remove the character, or replace it: value.encode("utf-8", "replace").decode().',
    ),
    (
        _axes,
        "hspan",
        (5, 2),
        ub.StyleError,
        "Axes.hspan(): low is 5, high is 2; low must be below high.",
        "high",
        SWAP_HINT,
    ),
    (
        _axes,
        "vspan",
        (2, 2),
        ub.StyleError,
        "Axes.vspan(): low is 2, high is 2; low must be below high.",
        "high",
        SWAP_HINT,
    ),
    (
        _axes,
        "hspan",
        (float("inf"), 1.0),
        ub.DataError,
        "Axes.hspan(): low is inf; a reference span needs finite values.",
        "low",
        FINITE_HINT,
    ),
    (
        _axes,
        "vspan",
        (1.0, float("nan")),
        ub.DataError,
        "Axes.vspan(): high is NaN; a reference span needs finite values.",
        "high",
        FINITE_HINT,
    ),
    (
        lambda: _axes(x=ub.Scale.log()),
        "vspan",
        (0, 5),
        ub.DataError,
        "Axes.vspan(): low is 0 on a log axis; values must be positive.",
        "low",
        "use a linear axis, or pass a positive value.",
    ),
    (
        lambda: _axes(ub.Scale.category()),
        "vspan",
        (1, 2),
        ub.UsageError,
        "Axes.vspan(): this is a category axis; spans along x apply to linear and log axes only.",
        None,
        "vspan needs a numeric x axis; use vline on a category.",
    ),
]


@pytest.mark.parametrize(("make", "name", "args", "cls", "message", "parameter", "hint"), ERRORS)
def test_errors_leave_the_figure_unchanged(
    make: Any,
    name: str,
    args: tuple[object, ...],
    cls: type[BaseException],
    message: str,
    parameter: object,
    hint: object,
) -> None:
    ax = cast(AnyAxes, make()).title("Kept")
    before = ax.figure.to_svg()
    with pytest.raises(cls) as caught:
        _call(ax, name)(*args)
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.hline(float('inf'), label='x')", "float('inf')"),
        ("ax.hline(y=float('inf'))", "y=float('inf')"),
        ("ax.vline(1e301)", "1e301"),
        ("ax.hspan(5, 2)", "2"),
        ("ax.hspan(low=5, high=2)", "high=2"),
        ("ax.vspan(float('nan'), 2)", "float('nan')"),
        ("ax.hline(1.0, color='red')", "color='red'"),
        ("ub.Figure().add_axes(x=ub.Scale.category()).vspan(0, 1)", ""),
    ],
)
def test_errors_highlight_the_argument(tmp_path: Path, call: str, argument: str) -> None:
    err = _raised(tmp_path, call, "ax = ub.Figure().add_axes()")
    if not argument:
        # No single argument is at fault: the note holds the hint only.
        assert _notes(err) == ["hint: vspan needs a numeric x axis; use vline on a category."]
        return
    assert _notes(err)[0] == _marked(call, argument)


# Rendering: the categories of a vertical line are looked up at render time


def _unknown_category() -> ub.Figure:
    ax = _axes(ub.Scale.category()).bar(["Q1", "Q2"], [1, 2])
    ax.vline("Q5")
    return ax.figure


UNKNOWN = 'x is "Q5"; no mark on this axes has that category.'
UNKNOWN_HINT = "draw a mark with that category first."


def test_an_unknown_category_fails_at_render_in_the_context_of_each_render_call(
    tmp_path: Path,
) -> None:
    fig = _unknown_category()
    renders: list[tuple[str, Callable[[], object]]] = [
        ("Figure.to_png()", lambda: fig.to_png()),
        ("Figure.to_svg()", lambda: fig.to_svg()),
        ("Figure.save_png()", lambda: fig.save_png(tmp_path / "a.png")),
        ("Figure.save_svg()", lambda: fig.save_svg(tmp_path / "a.svg")),
    ]
    for context, render in renders:
        with pytest.raises(ub.DataError) as caught:
            render()
        assert str(caught.value) == f"{context}: {UNKNOWN}"
        assert details(caught.value) == (None, UNKNOWN_HINT)
    assert not (tmp_path / "a.png").exists()
    assert not (tmp_path / "a.svg").exists()


def test_a_category_that_a_later_mark_brings_is_known_at_render() -> None:
    fig = _unknown_category()
    with pytest.raises(ub.DataError):
        fig.to_svg()
    fig.axes_at(x=ub.Scale.category()).scatter(["Q5"], [3.0])
    assert fig.to_svg()


# Rendering parity with the native layer


def test_the_four_methods_match_the_native_figure() -> None:
    fig = ub.Figure(4, 3, theme=ub.Theme.GRAPHITE)
    fig.add_axes().line([0, 1, 2], [1.0, 3.0, 2.0], label="Signal").hline(
        2.5, label="h", dash=ub.Dash.DASHED, width=ub.Size.times(2)
    ).vline(1.5, label="v").hspan(1.0, 2.0, label="hs").vspan(
        0.5, 1.0, label="vs", color=ub.Color.RED
    )
    native = _umber.Figure(4.0, 3.0, _umber.Theme.graphite(), *ONE_CELL)
    native.add_axes(*ONLY_CELL, LINEAR, 10.0, LINEAR, 10.0)
    native.line(0, 0, floats(0, 1, 2), floats(1, 3, 2), "Signal", AUTO, 0, NO_SIZE)
    native.hline(0, 0, 2.5, "h", AUTO, 1, (1, 2.0))
    native.vline(0, 0, 1.5, "v", AUTO, 0, NO_SIZE)
    native.hspan(0, 0, 1.0, 2.0, "hs", AUTO)
    native.vspan(0, 0, 0.5, 1.0, "vs", _color.native(ub.Color.RED))
    assert fig.to_svg() == native.to_svg(OUTLINES)
    assert fig.to_png(dpi=60) == native.to_png(60)


def test_a_category_vline_matches_the_native_figure() -> None:
    fig = ub.Figure(4, 3, theme=ub.Theme.MIDNIGHT)
    fig.add_axes(x=ub.Scale.category()).bar(["Q1", "Q2", "Q3"], [3, 1, 2]).vline("Q2", label="Go")
    native = _umber.Figure(4.0, 3.0, _umber.Theme.midnight(), *ONE_CELL)
    native.add_axes(*ONLY_CELL, CATEGORY, 10.0, LINEAR, 10.0)
    native.bar(0, 0, ["Q1", "Q2", "Q3"], floats(3, 1, 2), "", AUTO)
    native.vline(0, 0, "Q2", "Go", AUTO, 0, NO_SIZE)
    assert fig.to_svg() == native.to_svg(OUTLINES)
    assert fig.to_png(dpi=60) == native.to_png(60)


def floats(*values: float) -> Any:
    import numpy as np

    return np.array(values, dtype=np.float64)
