"""``Axes.band()``: type checks, data errors, the category refusal, rendering and native parity."""

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
TEXT_HINT = "pass a str; convert other values with str()."
TOO_LONG_HINT = "shorten the text; it would not fit the figure."
BAND_GAP = "use NaN to leave a gap in the band."
SWAPPED_HINT = "check that low and high are not swapped."
LINEAR_X_HINT = "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."
RESCALE_HINT = "use a linear axis, or rescale the values."
LINEAR, CATEGORY = 0, 2
OUTLINES = 0
AUTO = (0, 0, 1.0)
# A native figure of one cell (rows, cols, share_x, share_y, legend), and the position of its
# axes: row and column AUTO (grid index kind 0) spanning one cell.
ONE_CELL = (1, 1, 0, 0, 0)
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


def _band(ax: AnyAxes) -> Callable[..., object]:
    """``ax.band`` with arguments of any type, for calls that must be refused."""
    return cast(Callable[..., object], ax.band)


def _type_name(value: object) -> str:
    return "None" if value is None else type(value).__name__


def _parameter(err: BaseException) -> object:
    found = details(err)
    assert found is not None
    return found[0]


def floats(*values: float) -> npt.NDArray[np.float64]:
    return np.array(values, dtype=np.float64)


# Behaviour


def test_band_returns_the_axes_and_draws_a_fill() -> None:
    ax = _axes()
    assert ax.band([0, 1, 2], [1.0, 2.0, 1.0], [3.0, 4.0, 3.0]) is ax
    empty = _axes().figure.to_svg()
    assert ax.figure.to_svg() != empty


def test_band_is_a_keyword_only_method_of_the_package() -> None:
    assert ub.Axes.band.__module__ == "umber"
    with pytest.raises(TypeError):
        _axes().band([0, 1], [0, 1], [1, 2], "label")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
    assert not hasattr(ub, "band")


def test_band_accepts_lists_arrays_and_series() -> None:
    outputs: set[str] = set()
    for x, low, high in [
        ([0, 1, 2], [1.0, 2.0, 1.0], [3.0, 4.0, 3.0]),
        ((0.0, 1.0, 2.0), np.array([1, 2, 1], dtype=np.int16), (3, 4, 3)),
        (np.arange(3.0), pd.Series([1.0, 2.0, 1.0]), np.array([3, 4, 3], dtype=np.float32)),
        (range(3), [1.0, 2.0, 1.0], pd.Series([3.0, 4.0, 3.0])),
    ]:
        ax = _axes()
        _band(ax)(x, low, high)
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


def test_empty_and_all_nan_data_draw_nothing() -> None:
    ax = _axes().title("Kept")
    before = ax.figure.to_svg()
    ax.band([], [], [])
    ax.band(np.array([]), np.array([]), np.array([]))
    ax.band([NAN, NAN], [NAN, NAN], [NAN, NAN])
    assert ax.figure.to_svg() == before


def test_a_nan_in_any_array_breaks_the_band() -> None:
    x, low, high = [0.0, 1.0, 2.0, 3.0, 4.0], [0.0] * 5, [1.0, 2.0, 2.0, 3.0, 4.0]
    plain = _axes().band(x, low, high).figure.to_svg()
    gaps = {
        _axes().band([0.0, 1.0, NAN, 3.0, 4.0], low, high).figure.to_svg(),
        _axes().band(x, [0.0, 0.0, NAN, 0.0, 0.0], high).figure.to_svg(),
        _axes().band(x, low, [1.0, 2.0, NAN, 3.0, 4.0]).figure.to_svg(),
    }
    assert plain not in gaps
    assert len(gaps) == 1


def test_a_band_is_one_flat_fill_at_the_area_opacity_under_every_theme() -> None:
    for theme in THEMES:
        fig = ub.Figure(3, 2, theme=theme)
        fig.add_axes().band([0, 1, 2], [1.0, 2.0, 1.0], [3.0, 4.0, 3.0], color=ub.Color.RED)
        svg = fig.to_svg()
        assert svg.count("fill-opacity") >= 1
        assert 'stroke="#D64933"' not in svg
        assert "Gradient" not in svg
        assert "linearGradient" not in svg


def test_a_band_extends_the_domains_and_takes_a_legend_entry() -> None:
    labelled = _axes().band([0, 1], [0.0, 0.0], [1.0, 2.0], label="Interval")
    svg = labelled.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert ">Interval</text>" in svg
    # Under the direct style too, where lines are labelled at their ends.
    direct = _axes().band([0, 1], [0.0, 0.0], [1.0, 2.0], label="Interval")
    direct.legend(ub.LegendStyle.DIRECT)
    assert ">Interval</text>" in direct.figure.to_svg(text=ub.SvgText.EDITABLE)


def test_color_and_label_reach_the_figure() -> None:
    plain = _axes().band([0, 1], [0.0, 0.0], [1.0, 2.0]).figure.to_svg()
    red = _axes().band([0, 1], [0.0, 0.0], [1.0, 2.0], color=ub.Color.RED).figure.to_svg()
    assert red != plain
    assert "#D64933" in red


# Errors

# (keyword, a wrong value, the expected type in the message, hint)
STYLE_TYPE_ERRORS: list[tuple[str, object, str, str]] = [
    ("label", 3, "a str", TEXT_HINT),
    ("color", "red", "of type Color", COLOR_HINT),
    ("color", None, "of type Color", COLOR_HINT),
]


@pytest.mark.parametrize(("keyword", "value", "expected", "hint"), STYLE_TYPE_ERRORS)
def test_style_arguments_are_type_checked(
    keyword: str, value: object, expected: str, hint: str
) -> None:
    ax = _axes().title("Kept").band([0, 1], [0.0, 0.0], [1.0, 2.0])
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        _band(ax)([0, 1], [0, 1], [1, 2], **{keyword: value})
    assert str(caught.value) == (
        f"Axes.band(): {keyword} is {_type_name(value)}; {keyword} must be {expected}."
    )
    assert details(caught.value) == (keyword, hint)
    assert ax.figure.to_svg() == before


def test_labels_are_capped() -> None:
    with pytest.raises(ub.StyleError) as caught:
        _axes().band([0], [0], [1], label="x" * 1001)
    assert details(caught.value) == ("label", TOO_LONG_HINT)


def test_types_are_checked_in_the_order_x_low_high_label_color() -> None:
    ax = _axes()
    band = _band(ax)
    with pytest.raises(TypeError) as caught:
        band("abc", "def", "ghi")
    assert str(caught.value) == "Axes.band(): x is str; x must be a sequence or an array."
    with pytest.raises(TypeError) as caught:
        band([0.0], "def", "ghi")
    assert _parameter(caught.value) == "low"
    with pytest.raises(TypeError) as caught:
        band([0.0], [0.0], "ghi", label=3)
    assert _parameter(caught.value) == "high"
    with pytest.raises(TypeError) as caught:
        band([0.0], [0.0], [1.0], label=3, color="red")
    assert _parameter(caught.value) == "label"
    with pytest.raises(TypeError) as caught:
        band([0.0], [0.0], [1.0], color="red")
    assert _parameter(caught.value) == "color"


# (axes factory, x, low, high, message, parameter, hint)
DATA_ERRORS: list[tuple[Any, object, object, object, str, object, object]] = [
    (
        _axes,
        [0, 1, 2],
        [0.0, 5.0, 1.0],
        [1.0, 4.0, 2.0],
        "Axes.band(): low[1] is 5, above high[1] = 4; low must not exceed high.",
        "low",
        SWAPPED_HINT,
    ),
    (
        _axes,
        [0, 1],
        [np.nan, 2.0],
        [0.0, 1.0],
        "Axes.band(): low[1] is 2, above high[1] = 1; low must not exceed high.",
        "low",
        SWAPPED_HINT,
    ),
    (
        _axes,
        [0, 1],
        [0.0],
        [1.0, 2.0],
        "Axes.band(): x has 2 values but low has 1; x, low and high must have the same length.",
        None,
        None,
    ),
    (
        _axes,
        [0, 1],
        [0.0, 1.0],
        [2.0],
        "Axes.band(): x has 2 values but high has 1; x, low and high must have the same length.",
        None,
        None,
    ),
    (
        _axes,
        [0, np.inf],
        [0.0, 1.0],
        [2.0, 3.0],
        "Axes.band(): x[1] is inf; values must be finite.",
        "x",
        BAND_GAP,
    ),
    (
        _axes,
        [0, 1],
        [0.0, -np.inf],
        [2.0, 3.0],
        "Axes.band(): low[1] is -inf; values must be finite.",
        "low",
        BAND_GAP,
    ),
    (
        _axes,
        [0, 1],
        [0.0, 1.0],
        [np.inf, 3.0],
        "Axes.band(): high[0] is inf; values must be finite.",
        "high",
        BAND_GAP,
    ),
    (
        _axes,
        [0, 1e301],
        [0.0, 1.0],
        [2.0, 3.0],
        "Axes.band(): x[1] is 1e301; magnitudes must not exceed 1e300.",
        "x",
        None,
    ),
    (
        _axes,
        [0, 1],
        [0.0, 1.0],
        [2.0, None],
        "Axes.band(): high[1] is None; values must be numbers.",
        "high",
        "use NaN for missing values, e.g. series.to_numpy(dtype=float, na_value=np.nan).",
    ),
    (
        _axes,
        ["a", "b"],
        [0.0, 1.0],
        [2.0, 3.0],
        "Axes.band(): x[0] is 'a'; values must be numbers.",
        "x",
        "convert text to numbers first, e.g. with float().",
    ),
    (
        lambda: _axes(ub.Scale.log()),
        [1.0, 0.0],
        [1.0, 2.0],
        [2.0, 3.0],
        "Axes.band(): x[1] is 0 on a log axis; values must be positive.",
        "x",
        "use a linear axis, or remove values that are not positive.",
    ),
    (
        lambda: _axes(y=ub.Scale.log()),
        [1.0, 2.0],
        [1.0, 0.0],
        [2.0, 3.0],
        "Axes.band(): low[1] is 0 on a log axis; values must be positive.",
        "low",
        "use a linear axis, or remove values that are not positive.",
    ),
    (
        lambda: _axes(y=ub.Scale.log()),
        [1.0, 2.0],
        [1.0, 2.0],
        [2.0, 1e-310],
        "Axes.band(): high[1] is 1e-310 on a log axis; values must be at least 1e-300.",
        "high",
        RESCALE_HINT,
    ),
]


@pytest.mark.parametrize(("make", "x", "low", "high", "message", "parameter", "hint"), DATA_ERRORS)
def test_data_errors_leave_the_figure_unchanged(
    make: Any,
    x: object,
    low: object,
    high: object,
    message: str,
    parameter: object,
    hint: object,
) -> None:
    ax = cast(AnyAxes, make()).title("Kept").band([1.0], [1.0], [2.0])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        _band(ax)(x, low, high)
    assert str(caught.value) == message, message
    assert details(caught.value) == (parameter, hint)
    assert ax.figure.to_svg() == before


def test_a_category_x_axis_is_a_usage_error() -> None:
    ax = _axes(ub.Scale.category()).title("Kept")
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.band([0.0, 1.0], [0.0, 1.0], [1.0, 2.0])
    assert str(caught.value) == (
        "Axes.band(): this is a category axis; bands along x apply to linear and log axes only."
    )
    assert details(caught.value) == ("x", LINEAR_X_HINT)
    assert ax.figure.to_svg() == before
    # Names are refused as data before the axis is asked.
    with pytest.raises(ub.DataError):
        _band(ax)(["a", "b"], [0.0, 1.0], [1.0, 2.0])


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.band(t, t, bad)", "bad"),
        ("ax.band(x=t, low=t, high=bad)", "high=bad"),
        ("ax.band(t, bad, t)", "bad"),
        ("ax.band(bad, t, t)", "bad"),
        ("ax.band(t, t, t, color='red')", "color='red'"),
        ("ax.band(t, high, low)", "high"),
        ("ub.Figure().add_axes(x=ub.Scale.category()).band(xs, t, high)", "xs"),
        ("ax.title('T').band(t,\n    low, high)", "low"),
    ],
)
def test_errors_highlight_the_argument(tmp_path: Path, call: str, argument: str) -> None:
    setup = (
        "t = xs = [0.0, 1.0, 2.0]\nbad = [1.0, float('inf'), 2.0]\n"
        "low = [0.0, 5.0, 1.0]\nhigh = [1.0, 4.0, 2.0]\nax = ub.Figure().add_axes()"
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
def test_a_band_matches_the_native_figure(theme: ub.Theme, native_theme: Any) -> None:
    fig = ub.Figure(4, 3, theme=theme)
    fig.add_axes().band([0, 1, 2, 3], [1, 2, NAN, 1], [3, 4, 5, 3], label="a")
    native_fig = _umber.Figure(4.0, 3.0, native_theme(), *ONE_CELL)
    native_fig.add_axes(*ONLY_CELL, LINEAR, 10.0, LINEAR, 10.0)
    native_fig.band(0, 0, floats(0, 1, 2, 3), floats(1, 2, NAN, 1), floats(3, 4, 5, 3), "a", AUTO)
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)
    assert fig.to_png(dpi=60) == native_fig.to_png(60)


def test_a_band_on_a_category_axis_is_refused_by_the_native_layer_too() -> None:
    native = _umber.Figure(4.0, 3.0, _umber.Theme.pigment(), *ONE_CELL)
    native.add_axes(*ONLY_CELL, CATEGORY, 10.0, LINEAR, 10.0)
    with pytest.raises(ub.UsageError):
        native.band(0, 0, floats(0, 1), floats(0, 1), floats(1, 2), "", AUTO)
