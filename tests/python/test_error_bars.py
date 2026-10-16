"""``Axes.y_error_bars()`` and ``Axes.x_error_bars()``: type checks, data errors, the category
refusal, rendering and native parity."""

from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pandas as pd
import pytest

import umber as ub
from umber import _umber
from umber._errors import details

COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
SIZE_HINT = (
    "use Size.times(1.5) for a multiple of the theme's size, Size.pt(2.0) for points, or Size.AUTO."
)
TEXT_HINT = "pass a str; convert other values with str()."
TOO_LONG_HINT = "shorten the text; it would not fit the figure."
BAR_GAP = "use NaN to skip a bar."
SWAPPED_HINT = "check that low and high are not swapped."
LINEAR_X_HINT = "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."
USE_Y_HINT = "x_error_bars needs a numeric x axis; use y_error_bars on a category."
LINEAR, CATEGORY = 0, 2
OUTLINES = 0
AUTO = (0, 0, 1.0)
NO_SIZE = (0, 0.0)
# A native figure of one cell (rows, cols, share_x, share_y, legend, share_color), and the
# axes position: row and column AUTO (grid index kind 0) spanning one cell.
ONE_CELL = (1, 1, 0, 0, 0, 0)
ONLY_CELL = (0, 0, 0, 0, 1, 1)
THEMES = [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT]
NATIVE_THEMES = [_umber.Theme.pigment, _umber.Theme.graphite, _umber.Theme.midnight]
NAN = float("nan")

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


def _y_bars(ax: AnyAxes) -> Callable[..., object]:
    """``ax.y_error_bars`` with arguments of any type, for calls that must be refused."""
    return cast(Callable[..., object], ax.y_error_bars)


def _x_bars(ax: AnyAxes) -> Callable[..., object]:
    """``ax.x_error_bars`` with arguments of any type, for calls that must be refused."""
    return cast(Callable[..., object], ax.x_error_bars)


def _type_name(value: object) -> str:
    return "None" if value is None else type(value).__name__


def _parameter(err: BaseException) -> object:
    found = details(err)
    assert found is not None
    return found[0]


def floats(*values: float) -> npt.NDArray[np.float64]:
    return np.array(values, dtype=np.float64)


# Behaviour


def test_error_bars_return_the_axes_and_draw_strokes() -> None:
    empty = _axes().figure.to_svg()
    ax = _axes()
    assert ax.y_error_bars([0, 1], [1.0, 2.0], [3.0, 4.0]) is ax
    assert ax.figure.to_svg() != empty
    ax = _axes()
    assert ax.x_error_bars([0, 1], [1.0, 2.0], [3.0, 4.0]) is ax
    assert ax.figure.to_svg() != empty


def test_error_bars_are_keyword_only_methods_of_the_package() -> None:
    assert ub.Axes.y_error_bars.__module__ == "umber"
    assert ub.Axes.x_error_bars.__module__ == "umber"
    with pytest.raises(TypeError):
        _y_bars(_axes())([0, 1], [0, 1], [1, 2], "label")
    with pytest.raises(TypeError):
        _x_bars(_axes())([0, 1], [0, 1], [1, 2], "label")
    assert not hasattr(ub, "y_error_bars")


def test_error_bars_accept_lists_arrays_and_series() -> None:
    outputs: set[str] = set()
    for x, low, high in [
        ([0, 1, 2], [1.0, 2.0, 1.0], [3.0, 4.0, 3.0]),
        ((0.0, 1.0, 2.0), np.array([1, 2, 1], dtype=np.int16), (3, 4, 3)),
        (np.arange(3.0), pd.Series([1.0, 2.0, 1.0]), np.array([3, 4, 3], dtype=np.float32)),
        (range(3), [1.0, 2.0, 1.0], pd.Series([3.0, 4.0, 3.0])),
    ]:
        ax = _axes()
        _y_bars(ax)(x, low, high)
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


def test_a_category_x_axis_takes_names_and_draws_at_the_band_centres() -> None:
    ax = _axes(ub.Scale.category())
    ax.y_error_bars(["a", "b", "c"], [1.0, 2.0, 3.0], [2.0, 3.0, 4.0], caps=False)
    assert ax.figure.to_svg() != _axes(ub.Scale.category()).figure.to_svg()
    # Empty data follows the x scale: no names on a category axis.
    ax = _axes(ub.Scale.category())
    ax.y_error_bars([], [], [])
    assert ax.figure.to_svg() == _axes(ub.Scale.category()).figure.to_svg()


def test_empty_and_all_nan_data_draw_nothing() -> None:
    ax = _axes().title("Kept")
    before = ax.figure.to_svg()
    ax.y_error_bars([], [], [])
    ax.x_error_bars(np.array([]), np.array([]), np.array([]))
    ax.y_error_bars([NAN, NAN], [NAN, NAN], [NAN, NAN])
    ax.x_error_bars([NAN], [NAN], [NAN])
    assert ax.figure.to_svg() == before


def test_a_nan_in_any_array_skips_that_bar() -> None:
    x, low, high = [0.0, 1.0, 2.0], [0.0, 0.0, 0.0], [1.0, 2.0, 3.0]
    plain = _axes().y_error_bars(x, low, high).figure.to_svg()
    skipped = {
        _axes().y_error_bars([0.0, NAN, 2.0], low, high).figure.to_svg(),
        _axes().y_error_bars(x, [0.0, NAN, 0.0], high).figure.to_svg(),
        _axes().y_error_bars(x, low, [1.0, NAN, 3.0]).figure.to_svg(),
    }
    assert plain not in skipped


def test_caps_width_and_colour_reach_the_figure() -> None:
    def svg(**options: Any) -> str:
        ax = _axes().title("T")
        return cast(AnyAxes, _y_bars(ax)([1, 2], [1.0, 2.0], [3.0, 5.0], **options)).figure.to_svg()

    plain = svg()
    assert svg(caps=False) != plain
    assert svg(width=ub.Size.times(2)) != plain
    assert svg(width=ub.Size.AUTO) == plain
    assert svg(caps=True) == plain
    assert "#D64933" in svg(color=ub.Color.RED)
    # Caps cost two more segments per bar.
    assert svg().count("M") > svg(caps=False).count("M")


def test_error_bars_take_a_legend_entry_under_every_style() -> None:
    for style in ub.LegendStyle:
        if style is ub.LegendStyle.NONE:
            continue
        ax = _axes().y_error_bars([0, 1], [0.0, 0.0], [1.0, 2.0], label="Interval").legend(style)
        assert ">Interval</text>" in ax.figure.to_svg(text=ub.SvgText.EDITABLE), style
    ax = _axes().x_error_bars([0, 1], [0.0, 0.0], [1.0, 2.0], label="Across")
    ax.line([0, 1], [0, 1], label="Trend").legend(ub.LegendStyle.DIRECT)
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert ">Across</text>" in svg
    assert ">Trend</text>" in svg


# Errors

# (keyword, a wrong value, the expected type in the message, hint)
STYLE_TYPE_ERRORS: list[tuple[str, object, str, str]] = [
    ("label", 3, "a str", TEXT_HINT),
    ("color", "red", "of type Color", COLOR_HINT),
    ("color", None, "of type Color", COLOR_HINT),
    ("width", 2.0, "of type Size", SIZE_HINT),
    ("caps", 1, "a bool", "pass True or False."),
    ("caps", None, "a bool", "pass True or False."),
    ("caps", np.True_, "a bool", "pass True or False."),
]


@pytest.mark.parametrize("method", ["y_error_bars", "x_error_bars"])
@pytest.mark.parametrize(("keyword", "value", "expected", "hint"), STYLE_TYPE_ERRORS)
def test_style_arguments_are_type_checked(
    method: str, keyword: str, value: object, expected: str, hint: str
) -> None:
    ax = _axes().title("Kept")
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        getattr(ax, method)([0, 1], [0, 1], [1, 2], **{keyword: value})
    shown = _type_name(value)
    assert str(caught.value) == (
        f"Axes.{method}(): {keyword} is {shown}; {keyword} must be {expected}."
    ), str(caught.value)
    assert details(caught.value) == (keyword, hint)
    assert ax.figure.to_svg() == before


def test_labels_are_capped() -> None:
    with pytest.raises(ub.StyleError) as caught:
        _axes().y_error_bars([0], [0], [1], label="x" * 1001)
    assert details(caught.value) == ("label", TOO_LONG_HINT)


def test_types_are_checked_in_argument_order() -> None:
    ax = _axes()
    bars = _y_bars(ax)
    with pytest.raises(TypeError) as caught:
        bars("abc", "def", "ghi")
    assert str(caught.value) == "Axes.y_error_bars(): x is str; x must be a sequence or an array."
    with pytest.raises(TypeError) as caught:
        bars([0.0], "def", "ghi")
    assert _parameter(caught.value) == "low"
    with pytest.raises(TypeError) as caught:
        bars([0.0], [0.0], "ghi", label=3)
    assert _parameter(caught.value) == "high"
    with pytest.raises(TypeError) as caught:
        bars([0.0], [0.0], [1.0], label=3, color="red", caps=1)
    assert _parameter(caught.value) == "label"
    with pytest.raises(TypeError) as caught:
        bars([0.0], [0.0], [1.0], color="red", caps=1)
    assert _parameter(caught.value) == "color"
    with pytest.raises(TypeError) as caught:
        bars([0.0], [0.0], [1.0], width=1.0, caps=1)
    assert _parameter(caught.value) == "width"
    with pytest.raises(TypeError) as caught:
        bars([0.0], [0.0], [1.0], caps=1)
    assert _parameter(caught.value) == "caps"
    with pytest.raises(TypeError) as caught:
        _x_bars(ax)("abc", [0.0], [1.0])
    assert str(caught.value) == "Axes.x_error_bars(): y is str; y must be a sequence or an array."


# (method, axes factory, positions, low, high, message, parameter, hint)
DATA_ERRORS: list[tuple[str, Any, object, object, object, str, object, object]] = [
    (
        "y_error_bars",
        _axes,
        [0, 1, 2],
        [0.0, 5.0, 1.0],
        [1.0, 4.0, 2.0],
        "Axes.y_error_bars(): low[1] is 5, above high[1] = 4; low must not exceed high.",
        "low",
        SWAPPED_HINT,
    ),
    (
        "x_error_bars",
        _axes,
        [0, 1, 2],
        [0.0, 5.0, 1.0],
        [1.0, 4.0, 2.0],
        "Axes.x_error_bars(): low[1] is 5, above high[1] = 4; low must not exceed high.",
        "low",
        SWAPPED_HINT,
    ),
    (
        "y_error_bars",
        _axes,
        [0, 1],
        [NAN, 2.0],
        [0.0, 1.0],
        "Axes.y_error_bars(): low[1] is 2, above high[1] = 1; low must not exceed high.",
        "low",
        SWAPPED_HINT,
    ),
    (
        "y_error_bars",
        _axes,
        [0, 1],
        [0.0],
        [1.0, 2.0],
        "Axes.y_error_bars(): x has 2 values but low has 1; x, low and high must have the same"
        " length.",
        None,
        None,
    ),
    (
        "x_error_bars",
        _axes,
        [0, 1],
        [0.0, 1.0],
        [2.0],
        "Axes.x_error_bars(): y has 2 values but high has 1; y, low and high must have the same"
        " length.",
        None,
        None,
    ),
    (
        "y_error_bars",
        _axes,
        [0, np.inf],
        [0.0, 1.0],
        [2.0, 3.0],
        "Axes.y_error_bars(): x[1] is inf; values must be finite.",
        "x",
        BAR_GAP,
    ),
    (
        "x_error_bars",
        _axes,
        [0, np.inf],
        [0.0, 1.0],
        [2.0, 3.0],
        "Axes.x_error_bars(): y[1] is inf; values must be finite.",
        "y",
        BAR_GAP,
    ),
    (
        "y_error_bars",
        _axes,
        [0, 1],
        [0.0, -np.inf],
        [2.0, 3.0],
        "Axes.y_error_bars(): low[1] is -inf; values must be finite.",
        "low",
        BAR_GAP,
    ),
    (
        "x_error_bars",
        _axes,
        [0, 1],
        [0.0, 1.0],
        [np.inf, 3.0],
        "Axes.x_error_bars(): high[0] is inf; values must be finite.",
        "high",
        BAR_GAP,
    ),
    (
        "y_error_bars",
        _axes,
        [0, 1],
        [0.0, 1.0],
        [2.0, None],
        "Axes.y_error_bars(): high[1] is None; values must be numbers.",
        "high",
        "use NaN for missing values, e.g. series.to_numpy(dtype=float, na_value=np.nan).",
    ),
    (
        "y_error_bars",
        lambda: _axes(y=ub.Scale.log()),
        [1.0, 2.0],
        [1.0, 0.0],
        [2.0, 3.0],
        "Axes.y_error_bars(): low[1] is 0 on a log axis; values must be positive.",
        "low",
        "use a linear axis, or remove values that are not positive.",
    ),
    (
        "x_error_bars",
        lambda: _axes(ub.Scale.log()),
        [1.0, 2.0],
        [1.0, 0.0],
        [2.0, 3.0],
        "Axes.x_error_bars(): low[1] is 0 on a log axis; values must be positive.",
        "low",
        "use a linear axis, or remove values that are not positive.",
    ),
    (
        "x_error_bars",
        lambda: _axes(y=ub.Scale.log()),
        [1.0, 0.0],
        [1.0, 1.0],
        [2.0, 3.0],
        "Axes.x_error_bars(): y[1] is 0 on a log axis; values must be positive.",
        "y",
        "use a linear axis, or remove values that are not positive.",
    ),
]


@pytest.mark.parametrize(
    ("method", "make", "positions", "low", "high", "message", "parameter", "hint"), DATA_ERRORS
)
def test_data_errors_leave_the_figure_unchanged(
    method: str,
    make: Any,
    positions: object,
    low: object,
    high: object,
    message: str,
    parameter: object,
    hint: object,
) -> None:
    ax = cast(AnyAxes, make()).title("Kept").y_error_bars([1.0], [1.0], [2.0])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        getattr(ax, method)(positions, low, high)
    assert str(caught.value) == message, message
    assert details(caught.value) == (parameter, hint)
    assert ax.figure.to_svg() == before


def test_names_and_numbers_must_match_the_x_axis() -> None:
    with pytest.raises(ub.DataError) as caught:
        _axes(ub.Scale.category()).y_error_bars([0.0, 1.0], [0.0, 1.0], [1.0, 2.0])
    assert str(caught.value) == (
        "Axes.y_error_bars(): x holds numbers but the x axis is a category axis; pass category"
        " names."
    )
    with pytest.raises(ub.DataError):
        _y_bars(_axes())(["a", "b"], [0.0, 1.0], [1.0, 2.0])


def test_x_error_bars_on_a_category_x_axis_is_a_usage_error() -> None:
    ax = _axes(ub.Scale.category()).title("Kept")
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        _x_bars(ax)([0.0, 1.0], [0.0, 1.0], [1.0, 2.0])
    assert str(caught.value) == (
        "Axes.x_error_bars(): this is a category axis; error bars along x apply to linear and"
        " log axes only."
    )
    assert details(caught.value) == (None, USE_Y_HINT)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.y_error_bars(t, t, bad)", "bad"),
        ("ax.y_error_bars(x=t, low=t, high=bad)", "high=bad"),
        ("ax.y_error_bars(t, bad, t)", "bad"),
        ("ax.y_error_bars(bad, t, t)", "bad"),
        ("ax.y_error_bars(t, t, t, caps=1)", "caps=1"),
        ("ax.y_error_bars(t, high, low)", "high"),
        ("ax.x_error_bars(t, t, bad)", "bad"),
        ("ax.x_error_bars(y=t, low=bad, high=t)", "low=bad"),
        ("ax.x_error_bars(t, t, t, width=2.0)", "width=2.0"),
        ("ax.x_error_bars(t, high, low)", "high"),
        ("ub.Figure().add_axes(y=ub.Scale.log()).y_error_bars(xs, zero, high)", "zero"),
        ("ax.title('T').y_error_bars(t,\n    low, high)", "low"),
    ],
)
def test_errors_highlight_the_argument(tmp_path: Path, call: str, argument: str) -> None:
    setup = (
        "t = xs = [0.0, 1.0, 2.0]\nbad = [1.0, float('inf'), 2.0]\n"
        "low = [0.0, 5.0, 1.0]\nhigh = [1.0, 4.0, 2.0]\nzero = [0.0, 1.0, 2.0]\n"
        "ax = ub.Figure().add_axes()"
    )
    err = _raised(tmp_path, call, setup)
    shown = call.splitlines()[-1].strip() if "\n" in call else call
    assert _notes(err)[0] == _marked(shown, argument)


# Rendering parity with the native layer


@pytest.mark.parametrize(
    ("theme", "native_theme"),
    list(zip(THEMES, NATIVE_THEMES, strict=True)),
    ids=["pigment", "graphite", "midnight"],
)
def test_error_bars_match_the_native_figure(theme: ub.Theme, native_theme: Any) -> None:
    fig = ub.Figure(4, 3, theme=theme)
    fig.add_axes().y_error_bars([0, 1, 2], [1, 2, NAN], [3, 4, 5], label="a").x_error_bars(
        [1, 2], [0, 1], [2, 3], caps=False, width=ub.Size.times(2)
    )
    native_fig = _umber.Figure(4.0, 3.0, native_theme(), *ONE_CELL)
    native_fig.add_axes(*ONLY_CELL, LINEAR, 10.0, LINEAR, 10.0)
    native_fig.y_error_bars(
        0, 0, floats(0, 1, 2), floats(1, 2, NAN), floats(3, 4, 5), "a", AUTO, NO_SIZE, True
    )
    native_fig.x_error_bars(
        0, 0, floats(1, 2), floats(0, 1), floats(2, 3), "", AUTO, (1, 2.0), False
    )
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)
    assert fig.to_png(dpi=60) == native_fig.to_png(60)


def test_a_category_y_error_bar_matches_the_native_figure() -> None:
    fig = ub.Figure(4, 3)
    fig.add_axes(x=ub.Scale.category()).y_error_bars(["a", "b"], [1.0, 2.0], [2.0, 4.0])
    native_fig = _umber.Figure(4.0, 3.0, _umber.Theme.pigment(), *ONE_CELL)
    native_fig.add_axes(*ONLY_CELL, CATEGORY, 10.0, LINEAR, 10.0)
    native_fig.y_error_bars(0, 0, ["a", "b"], floats(1, 2), floats(2, 4), "", AUTO, NO_SIZE, True)
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)


def test_x_error_bars_on_a_category_axis_are_refused_by_the_native_layer_too() -> None:
    native = _umber.Figure(4.0, 3.0, _umber.Theme.pigment(), *ONE_CELL)
    native.add_axes(*ONLY_CELL, CATEGORY, 10.0, LINEAR, 10.0)
    with pytest.raises(ub.UsageError):
        native.x_error_bars(0, 0, floats(0, 1), floats(0, 1), floats(1, 2), "", AUTO, NO_SIZE, True)
