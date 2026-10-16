"""Stems: conversions, errors, atomicity, highlights and native rendering parity."""

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

GAP_HINT = "use NaN to leave a point out."
POSITIVE_HINT = "use a linear axis, or remove values that are not positive."
RESCALE_HINT = "use a linear axis, or rescale the values."
COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
SIZE_HINT = (
    "use Size.times(1.5) for a multiple of the theme's size, Size.pt(2.0) for points, or Size.AUTO."
)


def axes() -> ub.Axes[float, float]:
    return ub.Figure(4, 3).add_axes()


def stem(ax: ub.Axes[Any, Any]) -> Callable[..., object]:
    return cast(Callable[..., object], ax.stem)


def test_returns_self_and_has_keyword_only_options() -> None:
    ax = axes()
    assert ax.stem([0, 1, 2], [3, -2, 1]) is ax
    assert ub.Axes.stem.__module__ == "umber"
    assert not hasattr(ub, "stem")
    with pytest.raises(TypeError):
        stem(ax)([0, 1], [1, 2], "Response")


def test_lists_arrays_and_series_have_identical_output() -> None:
    outputs: set[str] = set()
    for x, y in [
        ([0, 1, 2], [3.0, -2.0, 1.0]),
        ((0.0, 1.0, 2.0), np.array([3, -2, 1], dtype=np.int16)),
        (np.arange(3.0), pd.Series([3.0, -2.0, 1.0])),
        (range(3), np.array([3.0, -2.0, 1.0], dtype=np.float32)),
    ]:
        ax = axes()
        stem(ax)(x, y)
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


def test_category_names_can_repeat_and_numeric_x_need_not_be_sorted() -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category())
    ax.stem(["b", "a", "b"], [1.0, 3.0, 2.0])
    ax.stem(np.array(["c", "a"]), [2.0, 1.0])
    assert ">c</text>" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert axes().stem([3, 0, 1], [1, -2, 4]).figure.to_svg()


@pytest.mark.parametrize("category", [False, True])
def test_empty_stems_do_not_change_the_figure(category: bool) -> None:
    fig = ub.Figure()
    ax = fig.add_axes(x=ub.Scale.category()) if category else fig.add_axes()
    before = ax.figure.to_svg()
    ax.stem([], [])
    ax.stem(np.array([]), np.array([]))
    assert ax.figure.to_svg() == before


def test_styling_gaps_and_empty_contributions() -> None:
    normal = axes().stem([0, 1, 2], [2, -1, 4]).figure.to_svg()
    assert normal == axes().stem([0, 1, 2], [2, -1, 4], size=ub.Size.AUTO).figure.to_svg()
    assert normal != axes().stem([0, 1, 2], [2, -1, 4], size=ub.Size.times(2)).figure.to_svg()
    assert normal != axes().stem([0, 1, 2], [2, np.nan, 4]).figure.to_svg()
    red = axes().stem([0, 1], [1, 2], color=ub.Color.RED).figure.to_svg()
    assert "#D64933" in red
    ax = axes().line([1.1, 2.3], [10, 12])
    before = ax.figure.to_svg()
    ax.stem([np.nan], [np.nan])
    assert ax.figure.to_svg() == before
    ax = axes().yticks([10, 12])
    before = ax.figure.to_svg()
    ax.stem([], [])
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("style", list(ub.LegendStyle))
def test_label_is_present_unless_the_legend_is_disabled(style: ub.LegendStyle) -> None:
    ax = axes().legend(style).stem([0, 1], [1, 2], label="Response")
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert (">Response</text>" in svg) == (style != ub.LegendStyle.NONE)


STYLE_ERRORS: list[tuple[str, object, str, str]] = [
    ("label", 3, "a str", "pass a str; convert other values with str()."),
    ("color", "red", "of type Color", COLOR_HINT),
    (
        "marker",
        "circle",
        "of type Marker",
        "use Marker.CIRCLE, Marker.SQUARE, Marker.TRIANGLE, Marker.DIAMOND or Marker.CROSS.",
    ),
    ("size", 2.0, "of type Size", SIZE_HINT),
    ("size", None, "of type Size", SIZE_HINT),
]


@pytest.mark.parametrize(("keyword", "value", "expected", "hint"), STYLE_ERRORS)
def test_style_types_are_checked_atomically(
    keyword: str, value: object, expected: str, hint: str
) -> None:
    ax = axes().stem([0, 1], [1, 2])
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        stem(ax)([0, 1], [1, 2], **{keyword: value})
    typename = "None" if value is None else type(value).__name__
    assert str(caught.value) == (
        f"Axes.stem(): {keyword} is {typename}; {keyword} must be {expected}."
    )
    assert details(caught.value) == (keyword, hint)
    assert ax.figure.to_svg() == before


class RecordsConversion:
    def __init__(self) -> None:
        self.converted = False

    def __array__(self, dtype: object = None, copy: object = None) -> npt.NDArray[np.float64]:
        self.converted = True
        return np.array([0.0, 1.0])


def test_all_type_checks_precede_conversion_and_labels_are_capped() -> None:
    data = RecordsConversion()
    with pytest.raises(TypeError) as caught:
        stem(axes())(data, [1, 2], color="red")
    assert details(caught.value) == ("color", COLOR_HINT)
    assert not data.converted
    with pytest.raises(TypeError) as caught:
        stem(axes())([0, "bad"], None)
    assert str(caught.value) == "Axes.stem(): y is None; y must be a sequence or an array."
    with pytest.raises(ub.StyleError) as caught_style:
        axes().stem([0], [1], label="x" * 1001)
    assert details(caught_style.value) == (
        "label",
        "shorten the text; it would not fit the figure.",
    )


DATA_ERRORS: list[tuple[object, object, str, str | None, str | None]] = [
    ([0, 1], [1], "x has 2 values but y has 1; x and y must have the same length.", None, None),
    ([0, -np.inf], [1, 2], "x[1] is -inf; values must be finite.", "x", GAP_HINT),
    ([0, 1], [1, np.inf], "y[1] is inf; values must be finite.", "y", GAP_HINT),
    ([0, 1e301], [1, 2], "x[1] is 1e301; magnitudes must not exceed 1e300.", "x", None),
    ([0, 1], [1, -1e301], "y[1] is -1e301; magnitudes must not exceed 1e300.", "y", None),
    (
        [0, 1],
        [1, None],
        "y[1] is None; values must be numbers.",
        "y",
        "use NaN for missing values, e.g. series.to_numpy(dtype=float, na_value=np.nan).",
    ),
    (
        ["a"],
        [1],
        "x holds category names but the x axis is not a category axis; pass numbers.",
        None,
        "pass x=Scale.category() to Figure.add_axes().",
    ),
]


@pytest.mark.parametrize(("x", "y", "message", "parameter", "hint"), DATA_ERRORS)
def test_data_errors_leave_the_figure_unchanged(
    x: object, y: object, message: str, parameter: str | None, hint: str | None
) -> None:
    ax = axes().stem([0, 1], [1, 2])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        stem(ax)(x, y)
    assert str(caught.value) == f"Axes.stem(): {message}"
    assert details(caught.value) == (parameter, hint)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("along_x", [False, True])
@pytest.mark.parametrize(
    ("value", "text", "hint"),
    [
        (0.0, "0 on a log axis; values must be positive.", POSITIVE_HINT),
        (-1.0, "-1 on a log axis; values must be positive.", POSITIVE_HINT),
        (1e-310, "1e-310 on a log axis; values must be at least 1e-300.", RESCALE_HINT),
    ],
)
def test_log_errors(along_x: bool, value: float, text: str, hint: str) -> None:
    ax = ub.Figure().add_axes(
        x=ub.Scale.log() if along_x else ub.Scale.linear(),
        y=ub.Scale.linear() if along_x else ub.Scale.log(),
    )
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.stem([1, value] if along_x else [1, 2], [1, 2] if along_x else [1, value])
    parameter = "x" if along_x else "y"
    assert str(caught.value) == f"Axes.stem(): {parameter}[1] is {text}"
    assert details(caught.value) == (parameter, hint)
    assert ax.figure.to_svg() == before


def test_numeric_x_on_category_axes_is_rejected() -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category())
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        stem(ax)([1, 2], [1, 2])
    assert str(caught.value) == (
        "Axes.stem(): x holds numbers but the x axis is a category axis; pass category names."
    )
    assert details(caught.value) == (
        None,
        "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().",
    )
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.stem(t, y)", "y"),
        ("ax.stem(x=t, y=y)", "y=y"),
        ("ax.stem(t, t, color='red')", "color='red'"),
        ("ax.stem(t, t, marker='circle')", "marker='circle'"),
        ("ax.stem(t, t, size=2.0)", "size=2.0"),
        ("ax.stem([0.0, 1e301], [1.0, 2.0])", "[0.0, 1e301]"),
        ("ax.title('T').stem(t,\n    y)", "y"),
    ],
)
def test_errors_highlight_the_right_argument(tmp_path: Path, call: str, argument: str) -> None:
    source = tmp_path / "user_code.py"
    source.write_text(
        "import umber as ub\nt = [0.0, 1.0]\ny = [1.0, float('inf')]\n"
        f"ax = ub.Figure().add_axes()\n{call}\n"
    )
    with pytest.raises((TypeError, ub.DataError)) as caught:
        exec(compile(source.read_text(), str(source), "exec"), {})
    shown = call.splitlines()[-1].strip() if "\n" in call else call
    expected = f"  {shown}\n{' ' * (2 + shown.index(argument))}{'^' * len(argument)}"
    assert list(getattr(caught.value, "__notes__", []))[0] == expected


@pytest.mark.parametrize("marker", list(ub.Marker))
@pytest.mark.parametrize(
    ("theme", "native_theme"),
    [
        (ub.Theme.PIGMENT, _umber.Theme.pigment),
        (ub.Theme.GRAPHITE, _umber.Theme.graphite),
        (ub.Theme.MIDNIGHT, _umber.Theme.midnight),
    ],
)
def test_stems_match_native_rendering(
    marker: ub.Marker, theme: ub.Theme, native_theme: Callable[[], _umber.Theme]
) -> None:
    fig = ub.Figure(4, 3, theme=theme)
    fig.add_axes().stem([0, 1, 2], [3, -2, 1], marker=marker, label="Response", size=ub.Size.pt(8))
    native = _umber.Figure(4.0, 3.0, native_theme(), 1, 1, 0, 0, 0, 0)
    native.add_axes(0, 0, 0, 0, 1, 1, 0, 10.0, 0, 10.0)
    native.stem(
        0,
        0,
        np.array([0.0, 1.0, 2.0]),
        np.array([3.0, -2.0, 1.0]),
        "Response",
        (0, 0, 1.0),
        marker.value,
        (2, 8.0),
    )
    assert fig.to_svg() == native.to_svg(0)
    assert fig.to_png(dpi=60) == native.to_png(60)


def test_category_stems_match_native_rendering() -> None:
    fig = ub.Figure(4, 3)
    fig.add_axes(x=ub.Scale.category()).stem(["b", "a", "b"], [3, -2, 1], label="Response")
    native = _umber.Figure(4.0, 3.0, _umber.Theme.pigment(), 1, 1, 0, 0, 0, 0)
    native.add_axes(0, 0, 0, 0, 1, 1, 2, 10.0, 0, 10.0)
    native.stem(
        0, 0, ["b", "a", "b"], np.array([3.0, -2.0, 1.0]), "Response", (0, 0, 1.0), 0, (0, 0.0)
    )
    assert fig.to_svg() == native.to_svg(0)
