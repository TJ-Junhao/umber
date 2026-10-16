"""``Axes.step()`` and ``Step``: the checks of ``line``, the order of x, and native parity."""

from collections.abc import Callable
from pathlib import Path
from typing import Any, cast, get_type_hints

import numpy as np
import numpy.typing as npt
import pandas as pd
import pytest

import umber as ub
from umber import _umber
from umber._errors import details

COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
DASH_HINT = "use Dash.SOLID, Dash.DASHED or Dash.DOTTED."
TEXT_HINT = "pass a str; convert other values with str()."
TOO_LONG_HINT = "shorten the text; it would not fit the figure."
SIZE_HINT = (
    "use Size.times(1.5) for a multiple of the theme's size, Size.pt(2.0) for points, or Size.AUTO."
)
LINE_GAP = "use NaN to leave a gap in the line."
WHERE_HINT = "use Step.POST, Step.PRE or Step.MID."
SORT_HINT = "sort the points by x first."
X_SCALE_HINT = "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."
RESCALE_HINT = "use a linear axis, or rescale the values."
CATEGORY_X_HINT = "pass x=Scale.category() to Figure.add_axes()."
LINEAR, CATEGORY = 0, 2
OUTLINES = 0
AUTO = (0, 0, 1.0)
# A native figure of one cell (rows, cols, share_x, share_y, legend), and the position of its
# axes: row and column AUTO (grid index kind 0) spanning one cell.
ONE_CELL = (1, 1, 0, 0, 0)
ONLY_CELL = (0, 0, 0, 0, 1, 1)
THEMES = [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT]
NATIVE_THEMES = [_umber.Theme.pigment, _umber.Theme.graphite, _umber.Theme.midnight]

type AnyAxes = ub.Axes[Any, Any]


def test_step_y_annotation_matches_the_other_line_marks() -> None:
    y = get_type_hints(ub.Axes.step)["y"]
    for method in (ub.Axes.line, ub.Axes.area, ub.Axes.stem):
        assert y == get_type_hints(method)["y"]


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


def _step(ax: AnyAxes) -> Callable[..., object]:
    """``ax.step`` with arguments of any type, for calls that must be refused."""
    return cast(Callable[..., object], ax.step)


def _type_name(value: object) -> str:
    return "None" if value is None else type(value).__name__


def floats(*values: float) -> npt.NDArray[np.float64]:
    return np.array(values, dtype=np.float64)


# Behaviour


def test_step_returns_the_axes_and_draws_a_step_line() -> None:
    ax = _axes()
    assert ax.step([0, 1, 2], [1.0, 3.0, 2.0]) is ax
    plain = _axes().line([0, 1, 2], [1.0, 3.0, 2.0]).figure.to_svg()
    assert ax.figure.to_svg() != plain


def test_step_is_a_keyword_only_method_of_the_package() -> None:
    assert ub.Axes.step.__module__ == "umber"
    with pytest.raises(TypeError):
        _axes().step([0, 1], [0, 1], ub.Step.POST)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
    assert not hasattr(ub, "step")


def test_step_accepts_lists_arrays_and_series() -> None:
    outputs: set[str] = set()
    for x, y in [
        ([0, 1, 2], [1.0, 3.0, 2.0]),
        ((0.0, 1.0, 2.0), np.array([1, 3, 2], dtype=np.int16)),
        (np.arange(3.0), pd.Series([1.0, 3.0, 2.0])),
        (range(3), np.array([1.0, 3.0, 2.0], dtype=np.float32)),
    ]:
        ax = _axes()
        _step(ax)(x, y)
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


def test_step_on_a_category_axis_takes_names_in_the_order_given() -> None:
    ax = _axes(ub.Scale.category())
    ax.step(["Q1", "Q2", "Q3"], [1.0, 3.0, 2.0])
    ax.step(np.array(["Q4", "Q2"]), [2.0, 1.0])
    assert ">Q4</text>" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)


@pytest.mark.parametrize("category", [False, True])
def test_empty_x_follows_the_x_scale(category: bool) -> None:
    ax = _axes(ub.Scale.category() if category else None)
    before = ax.figure.to_svg()
    ax.step([], [])
    ax.step(np.array([]), np.array([]))
    assert ax.figure.to_svg() == before


def test_the_three_positions_draw_three_different_lines() -> None:
    def svg(where: ub.Step) -> str:
        return _axes().step([0, 1, 3], [1.0, 2.0, 0.0], where=where).figure.to_svg()

    assert len({svg(where) for where in ub.Step}) == 3
    assert svg(ub.Step.POST) == _axes().step([0, 1, 3], [1.0, 2.0, 0.0]).figure.to_svg()


def test_a_nan_leaves_a_gap_and_the_style_arguments_reach_the_figure() -> None:
    plain = _axes().step([0, 1, 2, 3], [1.0, 3.0, 2.0, 4.0]).figure.to_svg()
    gap = _axes().step([0, 1, 2, 3], [1.0, 3.0, float("nan"), 4.0]).figure.to_svg()
    assert gap != plain
    dotted = _axes().step([0, 1], [0, 1], dash=ub.Dash.DOTTED).figure.to_svg()
    assert dotted != _axes().step([0, 1], [0, 1]).figure.to_svg()
    red = _axes().step([0, 1], [0, 1], color=ub.Color.RED).figure.to_svg()
    assert "#D64933" in red
    labelled = _axes().step([0, 1], [0, 1], label="North").figure
    assert ">North</text>" in labelled.to_svg(text=ub.SvgText.EDITABLE)


def test_a_nan_in_x_is_a_gap_and_does_not_count_as_unsorted() -> None:
    ax = _axes().step([0.0, float("nan"), 2.0, 3.0], [1.0, 2.0, 3.0, 4.0])
    assert ax.figure.to_svg()


def test_auto_width_matches_leaving_the_width_out() -> None:
    explicit = _axes().step([0, 1], [1, 2], width=ub.Size.AUTO).figure.to_svg()
    assert explicit == _axes().step([0, 1], [1, 2]).figure.to_svg()
    assert _axes().step([0, 1], [1, 2], width=ub.Size.times(2)).figure.to_svg() != explicit


# Errors

# (keyword, a wrong value, the expected type in the message, hint)
STYLE_TYPE_ERRORS: list[tuple[str, object, str, str]] = [
    ("label", 3, "a str", TEXT_HINT),
    ("color", "red", "of type Color", COLOR_HINT),
    ("dash", "--", "of type Dash", DASH_HINT),
    ("dash", ub.Marker.CIRCLE, "of type Dash", DASH_HINT),
    ("where", "post", "of type Step", WHERE_HINT),
    ("where", ub.Dash.SOLID, "of type Step", WHERE_HINT),
    ("where", 0, "of type Step", WHERE_HINT),
    ("width", 2.0, "of type Size", SIZE_HINT),
    ("width", None, "of type Size", SIZE_HINT),
]


@pytest.mark.parametrize(("keyword", "value", "expected", "hint"), STYLE_TYPE_ERRORS)
def test_style_arguments_are_type_checked(
    keyword: str, value: object, expected: str, hint: str
) -> None:
    ax = _axes().title("Kept").step([0, 1, 2], [1, 3, 2])
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        _step(ax)([0, 1], [0, 1], **{keyword: value})
    assert str(caught.value) == (
        f"Axes.step(): {keyword} is {_type_name(value)}; {keyword} must be {expected}."
    )
    assert details(caught.value) == (keyword, hint)
    assert ax.figure.to_svg() == before


def test_labels_are_capped() -> None:
    with pytest.raises(ub.StyleError) as caught:
        _axes().step([0], [0], label="x" * 1001)
    assert details(caught.value) == ("label", TOO_LONG_HINT)


def test_check_order_is_x_y_where_label_color_dash_width() -> None:
    order = ["where", "label", "color", "dash", "width"]
    bad: dict[str, Any] = {"where": "post", "label": 3, "color": "red", "dash": "--", "width": 2.0}
    ax = _axes()
    for keyword in order:
        remaining = {k: bad[k] for k in order[order.index(keyword) :]}
        with pytest.raises(TypeError) as caught:
            _step(ax)([0.0, 1.0], [1.0, 2.0], **remaining)
        found = details(caught.value)
        assert found is not None
        assert found[0] == keyword
    both = {"x": cast(list[float], "abc"), "y": cast(list[float], "def")}
    with pytest.raises(TypeError) as caught:
        _step(ax)(**both, **bad)
    assert str(caught.value) == "Axes.step(): x is str; x must be a sequence or an array."
    with pytest.raises(TypeError) as caught:
        _step(ax)([0.0, 1.0], cast(list[float], "def"), **bad)
    assert str(caught.value) == "Axes.step(): y is str; y must be a sequence or an array."


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
    step = _step(_axes())
    with pytest.raises(TypeError) as caught:
        step([0, "bad"], None)
    assert _parameter(caught.value) == "y"
    with pytest.raises(TypeError) as caught:
        step([0, "bad"], [1, 2], label="x" * 1001, color="red")
    assert _parameter(caught.value) == "color"
    recorder = _RecordsConversion()
    with pytest.raises(TypeError) as caught:
        step(recorder, None)
    assert _parameter(caught.value) == "y"
    assert not recorder.converted
    with pytest.raises(ub.DataError) as data:
        step([0, "bad"], [1, 2])
    assert _parameter(data.value) == "x"


# (axes factory, x, y, error class, message, parameter, hint)
DATA_ERRORS: list[tuple[Any, object, object, type, str, object, object]] = [
    (
        _axes,
        [0, 3, 2, 5],
        [1.0, 2.0, 3.0, 4.0],
        ub.DataError,
        "Axes.step(): x[2] is 2, below x[1] = 3; step needs x in non-decreasing order.",
        "x",
        SORT_HINT,
    ),
    (
        _axes,
        [0.0, np.nan, -0.5],
        [1.0, 2.0, 3.0],
        ub.DataError,
        "Axes.step(): x[2] is -0.5, below x[0] = 0; step needs x in non-decreasing order.",
        "x",
        SORT_HINT,
    ),
    (
        lambda: _axes(ub.Scale.log()),
        [10.0, 1.0],
        [1.0, 2.0],
        ub.DataError,
        "Axes.step(): x[1] is 1, below x[0] = 10; step needs x in non-decreasing order.",
        "x",
        SORT_HINT,
    ),
    (
        _axes,
        [0, 1, 2],
        [1.0, np.inf, 2.0],
        ub.DataError,
        "Axes.step(): y[1] is inf; values must be finite.",
        "y",
        LINE_GAP,
    ),
    (
        _axes,
        [0, -np.inf],
        [1.0, 2.0],
        ub.DataError,
        "Axes.step(): x[1] is -inf; values must be finite.",
        "x",
        LINE_GAP,
    ),
    (
        _axes,
        [0, 1],
        [1.0],
        ub.DataError,
        "Axes.step(): x has 2 values but y has 1; x and y must have the same length.",
        None,
        None,
    ),
    (
        _axes,
        ["a", "b"],
        [1.0, 2.0],
        ub.DataError,
        "Axes.step(): x holds category names but the x axis is not a category axis; pass numbers.",
        None,
        CATEGORY_X_HINT,
    ),
    (
        lambda: _axes(ub.Scale.category()),
        [1.0, 2.0],
        [1.0, 2.0],
        ub.DataError,
        "Axes.step(): x holds numbers but the x axis is a category axis; pass category names.",
        None,
        X_SCALE_HINT,
    ),
    (
        _axes,
        [0, 1],
        [1.0, None],
        ub.DataError,
        "Axes.step(): y[1] is None; values must be numbers.",
        "y",
        "use NaN for missing values, e.g. series.to_numpy(dtype=float, na_value=np.nan).",
    ),
    (
        lambda: _axes(ub.Scale.log()),
        [1.0, 0.0],
        [1.0, 2.0],
        ub.DataError,
        "Axes.step(): x[1] is 0 on a log axis; values must be positive.",
        "x",
        "use a linear axis, or remove values that are not positive.",
    ),
    (
        _axes,
        [0, 1e301],
        [1.0, 2.0],
        ub.DataError,
        "Axes.step(): x[1] is 1e301; magnitudes must not exceed 1e300.",
        "x",
        None,
    ),
    (
        _axes,
        [0, 1],
        [1.0, -1e301],
        ub.DataError,
        "Axes.step(): y[1] is -1e301; magnitudes must not exceed 1e300.",
        "y",
        None,
    ),
    (
        lambda: _axes(ub.Scale.log()),
        [1.0, 1e-310],
        [1.0, 2.0],
        ub.DataError,
        "Axes.step(): x[1] is 1e-310 on a log axis; values must be at least 1e-300.",
        "x",
        RESCALE_HINT,
    ),
    (
        lambda: ub.Figure(3, 2).add_axes(y=ub.Scale.log()),
        [1.0, 2.0],
        [1.0, 1e-310],
        ub.DataError,
        "Axes.step(): y[1] is 1e-310 on a log axis; values must be at least 1e-300.",
        "y",
        RESCALE_HINT,
    ),
    (
        _axes,
        "abc",
        [1.0],
        TypeError,
        "Axes.step(): x is str; x must be a sequence or an array.",
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
    ax = cast(AnyAxes, make()).title("Kept").step([], [])
    before = ax.figure.to_svg()
    with pytest.raises(cls) as caught:
        _step(ax)(x, y)
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert ax.figure.to_svg() == before


def test_a_y_that_is_not_positive_on_a_log_y_axis_is_a_data_error() -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.log()).title("Kept")
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.step([0, 1], [1.0, 0.0])
    assert str(caught.value) == "Axes.step(): y[1] is 0 on a log axis; values must be positive."
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.step(t, y, label='North')", "y"),
        ("ax.step(x=t, y=y)", "y=y"),
        ("ax.step(t, t, color='red')", "color='red'"),
        ("ax.step(t, t, width=2.0)", "width=2.0"),
        ("ax.step(t, t, where='post')", "where='post'"),
        ("ax.step([0.0, 2.0, 1.0], t)", "[0.0, 2.0, 1.0]"),
        ("ax.step([0.0, 1e301], [1.0, 2.0])", "[0.0, 1e301]"),
        ("ax.step([0.0, 1.0], [1.0, -1e301])", "[1.0, -1e301]"),
        ("ub.Figure().add_axes(x=ub.Scale.log()).step([1.0, 1e-310], [1.0, 2.0])", "[1.0, 1e-310]"),
        ("ax.title('T').step(t,\n    y)", "y"),
    ],
)
def test_errors_highlight_the_argument(tmp_path: Path, call: str, argument: str) -> None:
    setup = "t = [0.0, 1.0, 2.0]\ny = [1.0, float('inf'), 2.0]\nax = ub.Figure().add_axes()"
    err = _raised(tmp_path, call, setup)
    shown = call.splitlines()[-1].strip() if "\n" in call else call
    assert _notes(err)[0] == _marked(shown, argument)


# Rendering parity with the native layer


@pytest.mark.parametrize("where", list(ub.Step), ids=lambda w: w.name)
@pytest.mark.parametrize(
    ("theme", "native_theme"),
    list(zip(THEMES, NATIVE_THEMES, strict=True)),
    ids=["pigment", "graphite", "midnight"],
)
def test_a_step_matches_the_native_figure(
    where: ub.Step, theme: ub.Theme, native_theme: Any
) -> None:
    fig = ub.Figure(4, 3, theme=theme)
    fig.add_axes().step([0, 1, 2, 3], [1, 3, 2, 4], where=where, label="a", width=ub.Size.times(2))
    native_fig = _umber.Figure(4.0, 3.0, native_theme(), *ONE_CELL)
    native_fig.add_axes(*ONLY_CELL, LINEAR, 10.0, LINEAR, 10.0)
    native_fig.step(
        0, 0, floats(0, 1, 2, 3), floats(1, 3, 2, 4), where.value, "a", AUTO, 0, (1, 2.0)
    )
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)
    assert fig.to_png(dpi=60) == native_fig.to_png(60)


def test_a_category_step_matches_the_native_figure() -> None:
    fig = ub.Figure(4, 3, theme=ub.Theme.MIDNIGHT)
    fig.add_axes(x=ub.Scale.category()).step(
        ["Q1", "Q2", "Q3"], [3, 1, 2], where=ub.Step.MID, label="Sales"
    )
    native = _umber.Figure(4.0, 3.0, _umber.Theme.midnight(), *ONE_CELL)
    native.add_axes(*ONLY_CELL, CATEGORY, 10.0, LINEAR, 10.0)
    native.step(0, 0, ["Q1", "Q2", "Q3"], floats(3, 1, 2), 2, "Sales", AUTO, 0, (0, 0.0))
    assert fig.to_png(dpi=60) == native.to_png(60)
    assert fig.to_svg() == native.to_svg(OUTLINES)
