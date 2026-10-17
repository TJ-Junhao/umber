"""``Axes.area()`` and ``FillStyle``: the checks of ``line``, the fill, and native parity."""

from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pandas as pd
import pytest

import umber as ub
from baseline_check import assert_only_baseline_added
from umber import _umber
from umber._errors import details

COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
DASH_HINT = "use Dash.SOLID, Dash.DASHED or Dash.DOTTED."
TEXT_HINT = "pass a str; convert other values with str()."
TOO_LONG_HINT = "shorten the text; it would not fit the figure."
SIZE_HINT = (
    "use Size.times(1.5) for a multiple of the theme's size, Size.pt(2.0) for points, or Size.AUTO."
)
AREA_GAP = "use NaN to leave a gap in the area."
X_SCALE_HINT = "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."
RESCALE_HINT = "use a linear axis, or rescale the values."
CATEGORY_X_HINT = "pass x=Scale.category() to Figure.add_axes()."
LINEAR, CATEGORY = 0, 2
OUTLINES = 0
AUTO = (0, 0, 1.0)
# A native figure of one cell (rows, cols, share_x, share_y, legend, share_color), and the
# axes position: row and column AUTO (grid index kind 0) spanning one cell.
ONE_CELL = (1, 1, 0, 0, 0, 0)
ONLY_CELL = (0, 0, 0, 0, 1, 1)
THEMES = [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT]
NATIVE_THEMES = [_umber.Theme.pigment, _umber.Theme.graphite, _umber.Theme.midnight]

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


def _axes(x: ub.Scale[Any] | None = None) -> AnyAxes:
    fig = ub.Figure(3, 2)
    return fig.add_axes() if x is None else fig.add_axes(x=x)


def _area(ax: AnyAxes) -> Callable[..., object]:
    """``ax.area`` with arguments of any type, for calls that must be refused."""
    return cast(Callable[..., object], ax.area)


def _type_name(value: object) -> str:
    return "None" if value is None else type(value).__name__


def floats(*values: float) -> npt.NDArray[np.float64]:
    return np.array(values, dtype=np.float64)


# Behaviour


def test_area_returns_the_axes_and_draws_the_fill() -> None:
    ax = _axes()
    assert ax.area([0, 1, 2], [1.0, 3.0, 2.0]) is ax
    plain = _axes().line([0, 1, 2], [1.0, 3.0, 2.0]).figure.to_svg()
    assert ax.figure.to_svg() != plain


def test_area_is_a_keyword_only_method_of_the_package() -> None:
    assert ub.Axes.area.__module__ == "umber"
    with pytest.raises(TypeError):
        _axes().area([0, 1], [0, 1], "label")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
    assert not hasattr(ub, "area")


def test_area_accepts_lists_arrays_and_series() -> None:
    outputs: set[str] = set()
    for x, y in [
        ([0, 1, 2], [1.0, 3.0, 2.0]),
        ((0.0, 1.0, 2.0), np.array([1, 3, 2], dtype=np.int16)),
        (np.arange(3.0), pd.Series([1.0, 3.0, 2.0])),
        (range(3), np.array([1.0, 3.0, 2.0], dtype=np.float32)),
    ]:
        ax = _axes()
        _area(ax)(x, y)
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


def test_area_on_a_category_axis_takes_names() -> None:
    ax = _axes(ub.Scale.category())
    ax.area(["Q1", "Q2", "Q3"], [1.0, 3.0, 2.0])
    ax.area(np.array(["Q2", "Q4"]), [2.0, 1.0])
    assert ">Q4</text>" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)


@pytest.mark.parametrize("category", [False, True])
def test_empty_x_follows_the_x_scale(category: bool) -> None:
    ax = _axes(ub.Scale.category() if category else None)
    before = ax.figure.to_svg()
    ax.area([], [])
    after = ax.figure.to_svg()
    assert_only_baseline_added(before, after)
    ax.area(np.array([]), np.array([]))
    assert ax.figure.to_svg() == after


def test_a_nan_leaves_a_gap_and_the_style_arguments_reach_the_figure() -> None:
    plain = _axes().area([0, 1, 2, 3], [1.0, 3.0, 2.0, 4.0]).figure.to_svg()
    gap = _axes().area([0, 1, 2, 3], [1.0, 3.0, float("nan"), 4.0]).figure.to_svg()
    assert gap != plain
    dotted = _axes().area([0, 1], [0, 1], dash=ub.Dash.DOTTED).figure.to_svg()
    assert dotted != _axes().area([0, 1], [0, 1]).figure.to_svg()
    red = _axes().area([0, 1], [0, 1], color=ub.Color.RED).figure.to_svg()
    assert "#D64933" in red
    labelled = _axes().area([0, 1], [0, 1], label="North").figure
    assert ">North</text>" in labelled.to_svg(text=ub.SvgText.EDITABLE)


def test_auto_width_matches_leaving_the_width_out() -> None:
    explicit = _axes().area([0, 1], [1, 2], width=ub.Size.AUTO).figure.to_svg()
    assert explicit == _axes().area([0, 1], [1, 2]).figure.to_svg()
    assert _axes().area([0, 1], [1, 2], width=ub.Size.times(2)).figure.to_svg() != explicit


# Errors

# (keyword, a wrong value, the expected type in the message, hint)
STYLE_TYPE_ERRORS: list[tuple[str, object, str, str]] = [
    ("label", 3, "a str", TEXT_HINT),
    ("color", "red", "of type Color", COLOR_HINT),
    ("dash", "--", "of type Dash", DASH_HINT),
    ("dash", ub.Marker.CIRCLE, "of type Dash", DASH_HINT),
    ("width", 2.0, "of type Size", SIZE_HINT),
    ("width", None, "of type Size", SIZE_HINT),
]


@pytest.mark.parametrize(("keyword", "value", "expected", "hint"), STYLE_TYPE_ERRORS)
def test_style_arguments_are_type_checked(
    keyword: str, value: object, expected: str, hint: str
) -> None:
    ax = _axes().title("Kept").area([0, 1, 2], [1, 3, 2])
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        _area(ax)([0, 1], [0, 1], **{keyword: value})
    assert str(caught.value) == (
        f"Axes.area(): {keyword} is {_type_name(value)}; {keyword} must be {expected}."
    )
    assert details(caught.value) == (keyword, hint)
    assert ax.figure.to_svg() == before


def test_labels_are_capped() -> None:
    with pytest.raises(ub.StyleError) as caught:
        _axes().area([0], [0], label="x" * 1001)
    assert details(caught.value) == ("label", TOO_LONG_HINT)


def test_check_order_is_x_y_label_color_dash_width() -> None:
    order = ["label", "color", "dash", "width"]
    bad: dict[str, Any] = {"label": 3, "color": "red", "dash": "--", "width": 2.0}
    ax = _axes()
    for keyword in order:
        remaining = {k: bad[k] for k in order[order.index(keyword) :]}
        with pytest.raises(TypeError) as caught:
            _area(ax)([0.0, 1.0], [1.0, 2.0], **remaining)
        found = details(caught.value)
        assert found is not None
        assert found[0] == keyword
    both = {"x": cast(list[float], "abc"), "y": cast(list[float], "def")}
    with pytest.raises(TypeError) as caught:
        _area(ax)(**both, **bad)
    assert str(caught.value) == "Axes.area(): x is str; x must be a sequence or an array."
    with pytest.raises(TypeError) as caught:
        _area(ax)([0.0, 1.0], cast(list[float], "def"), **bad)
    assert str(caught.value) == "Axes.area(): y is str; y must be a sequence or an array."


class _RecordsConversion:
    """A data object that notes whether it was converted to an array."""

    def __init__(self) -> None:
        self.converted = False

    def __array__(self, dtype: object = None, copy: object = None) -> npt.NDArray[np.float64]:
        self.converted = True
        return np.array([0.0, 1.0])


def _parameter(err: BaseException) -> object:
    found = details(err)
    assert found is not None
    return found[0]


def test_types_are_checked_before_any_conversion() -> None:
    area = _area(_axes())
    with pytest.raises(TypeError) as caught:
        area([0, "bad"], None)
    assert _parameter(caught.value) == "y"
    with pytest.raises(TypeError) as caught:
        area([0, "bad"], [1, 2], label="x" * 1001, color="red")
    assert _parameter(caught.value) == "color"
    recorder = _RecordsConversion()
    with pytest.raises(TypeError) as caught:
        area(recorder, None)
    assert _parameter(caught.value) == "y"
    assert not recorder.converted
    with pytest.raises(ub.DataError) as data:
        area([0, "bad"], [1, 2])
    assert _parameter(data.value) == "x"


# (axes factory, x, y, error class, message, parameter, hint)
DATA_ERRORS: list[tuple[Any, object, object, type, str, object, object]] = [
    (
        _axes,
        [0, 1, 2],
        [1.0, np.inf, 2.0],
        ub.DataError,
        "Axes.area(): y[1] is inf; values must be finite.",
        "y",
        AREA_GAP,
    ),
    (
        _axes,
        [0, -np.inf],
        [1.0, 2.0],
        ub.DataError,
        "Axes.area(): x[1] is -inf; values must be finite.",
        "x",
        AREA_GAP,
    ),
    (
        _axes,
        [0, 1],
        [1.0],
        ub.DataError,
        "Axes.area(): x has 2 values but y has 1; x and y must have the same length.",
        None,
        None,
    ),
    (
        _axes,
        ["a", "b"],
        [1.0, 2.0],
        ub.DataError,
        "Axes.area(): x holds category names but the x axis is not a category axis; pass numbers.",
        None,
        CATEGORY_X_HINT,
    ),
    (
        lambda: _axes(ub.Scale.category()),
        [1.0, 2.0],
        [1.0, 2.0],
        ub.DataError,
        "Axes.area(): x holds numbers but the x axis is a category axis; pass category names.",
        None,
        X_SCALE_HINT,
    ),
    (
        _axes,
        [0, 1],
        [1.0, None],
        ub.DataError,
        "Axes.area(): y[1] is None; values must be numbers.",
        "y",
        "use NaN for missing values, e.g. series.to_numpy(dtype=float, na_value=np.nan).",
    ),
    (
        lambda: _axes(ub.Scale.log()),
        [1.0, 0.0],
        [1.0, 2.0],
        ub.DataError,
        "Axes.area(): x[1] is 0 on a log axis; values must be positive.",
        "x",
        "use a linear axis, or remove values that are not positive.",
    ),
    (
        _axes,
        [0, 1e301],
        [1.0, 2.0],
        ub.DataError,
        "Axes.area(): x[1] is 1e301; magnitudes must not exceed 1e300.",
        "x",
        None,
    ),
    (
        _axes,
        [0, 1],
        [1.0, -1e301],
        ub.DataError,
        "Axes.area(): y[1] is -1e301; magnitudes must not exceed 1e300.",
        "y",
        None,
    ),
    (
        lambda: _axes(ub.Scale.log()),
        [1.0, 1e-310],
        [1.0, 2.0],
        ub.DataError,
        "Axes.area(): x[1] is 1e-310 on a log axis; values must be at least 1e-300.",
        "x",
        RESCALE_HINT,
    ),
    (
        lambda: ub.Figure(3, 2).add_axes(y=ub.Scale.log()),
        [1.0, 2.0],
        [1.0, 1e-310],
        ub.DataError,
        "Axes.area(): y[1] is 1e-310 on a log axis; values must be at least 1e-300.",
        "y",
        RESCALE_HINT,
    ),
    (
        _axes,
        "abc",
        [1.0],
        TypeError,
        "Axes.area(): x is str; x must be a sequence or an array.",
        "x",
        'pass a list of names or numbers, e.g. ["A", "B"] or [1.0, 2.0].',
    ),
]


@pytest.mark.parametrize(("make", "x", "y", "cls", "message", "parameter", "hint"), DATA_ERRORS)
def test_data_errors_leave_the_figure_unchanged(
    make: Any,
    x: object,
    y: object,
    cls: type[BaseException],
    message: str,
    parameter: object,
    hint: object,
) -> None:
    ax = cast(AnyAxes, make()).title("Kept").area([], [])
    before = ax.figure.to_svg()
    with pytest.raises(cls) as caught:
        _area(ax)(x, y)
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert ax.figure.to_svg() == before


def test_a_y_that_is_not_positive_on_a_log_y_axis_is_a_data_error() -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.log()).title("Kept")
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.area([0, 1], [1.0, 0.0])
    assert str(caught.value) == "Axes.area(): y[1] is 0 on a log axis; values must be positive."
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.area(t, y, label='North')", "y"),
        ("ax.area(x=t, y=y)", "y=y"),
        ("ax.area(t, t, color='red')", "color='red'"),
        ("ax.area(t, t, width=2.0)", "width=2.0"),
        ("ax.area([0.0, 1e301], [1.0, 2.0])", "[0.0, 1e301]"),
        ("ax.area([0.0, 1.0], [1.0, -1e301])", "[1.0, -1e301]"),
        ("ub.Figure().add_axes(x=ub.Scale.log()).area([1.0, 1e-310], [1.0, 2.0])", "[1.0, 1e-310]"),
        ("ax.title('T').area(t,\n    y)", "y"),
    ],
)
def test_errors_highlight_the_argument(tmp_path: Path, call: str, argument: str) -> None:
    setup = "t = [0.0, 1.0, 2.0]\ny = [1.0, float('inf'), 2.0]\nax = ub.Figure().add_axes()"
    err = _raised(tmp_path, call, setup)
    shown = call.splitlines()[-1].strip() if "\n" in call else call
    assert _notes(err)[0] == _marked(shown, argument)


# Rendering parity with the native layer


@pytest.mark.parametrize("fill", list(ub.FillStyle), ids=lambda f: f.name)
@pytest.mark.parametrize(
    ("theme", "native_theme"),
    list(zip(THEMES, NATIVE_THEMES, strict=True)),
    ids=["pigment", "graphite", "midnight"],
)
def test_an_area_matches_the_native_figure(
    fill: ub.FillStyle, theme: ub.Theme, native_theme: Any
) -> None:
    fig = ub.Figure(4, 3, theme=theme.with_fill_style(fill))
    fig.add_axes().area([0, 1, 2, 3], [1, 3, 2, 4], label="a", width=ub.Size.times(2))
    native_fig = _umber.Figure(4.0, 3.0, native_theme().with_fill_style(fill.value), *ONE_CELL)
    native_fig.add_axes(*ONLY_CELL, LINEAR, 10.0, LINEAR, 10.0)
    native_fig.area(0, 0, floats(0, 1, 2, 3), floats(1, 3, 2, 4), "a", AUTO, 0, (1, 2.0))
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)
    assert fig.to_png(dpi=60) == native_fig.to_png(60)


def test_the_fill_style_changes_the_rendering() -> None:
    def svg(fill: ub.FillStyle) -> str:
        fig = ub.Figure(4, 3, theme=ub.Theme.PIGMENT.with_fill_style(fill))
        fig.add_axes().area([0, 1, 2], [1, 3, 2])
        return fig.to_svg()

    assert svg(ub.FillStyle.SOLID) != svg(ub.FillStyle.GRADIENT)


def test_a_category_area_matches_the_native_figure() -> None:
    fig = ub.Figure(4, 3, theme=ub.Theme.MIDNIGHT)
    fig.add_axes(x=ub.Scale.category()).area(["Q1", "Q2", "Q3"], [3, 1, 2], label="Sales")
    native = _umber.Figure(4.0, 3.0, _umber.Theme.midnight(), *ONE_CELL)
    native.add_axes(*ONLY_CELL, CATEGORY, 10.0, LINEAR, 10.0)
    native.area(0, 0, ["Q1", "Q2", "Q3"], floats(3, 1, 2), "Sales", AUTO, 0, (0, 0.0))
    assert fig.to_png(dpi=60) == native.to_png(60)
    assert fig.to_svg() == native.to_svg(OUTLINES)
