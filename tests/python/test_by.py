"""Long-form columns split into labelled series without partial changes."""

import datetime as dt
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import pandas as pd
import polars as pl
import pytest

import umber as ub
from umber._errors import details

type Axes = ub.Axes[Any, Any]
METHODS = ("line", "scatter", "step", "area", "ecdf", "kde", "hist", "bar")
XY = ("line", "scatter", "step", "area")
GROUP_HINT = "each group takes the next palette color and its name as the label."
X = [0.0, 0.0, 1.0, 1.0, 2.0, 2.0]
Y = [1.0, 4.0, 3.0, 2.0, 2.0, 5.0]
GROUPS = ["South", "North", "South", "North", "South", "North"]


def _axes(method: str, **kwargs: Any) -> Axes:
    return ub.Figure(6, 4).add_axes(
        x=ub.Scale.category() if method == "bar" else ub.Scale.linear(), **kwargs
    )


def _details(error: BaseException) -> tuple[str | None, str | None]:
    info = details(error)
    assert info is not None
    return info


def _svg(ax: Axes) -> str:
    return ax.figure.to_svg(text=ub.SvgText.EDITABLE)


def _call(ax: Axes, method: str, **kwargs: Any) -> object:
    call = cast(Callable[..., object], getattr(ax, method))
    if method in XY:
        return call(X, Y, **kwargs)
    if method == "bar":
        return call(["Q2", "Q2", "Q1", "Q1", "Q3", "Q3"], Y, **kwargs)
    if method == "hist":
        return call(Y, bins=[0, 2, 4, 6], style=ub.Hist.STEP, **kwargs)
    return call(Y, **kwargs)


@pytest.mark.parametrize("method", XY + ("ecdf", "kde"))
def test_by_splits_in_first_appearance_order(method: str) -> None:
    ax, expected = _axes(method), _axes(method)
    assert _call(ax, method, by=GROUPS) is ax
    draw = cast(Callable[..., object], getattr(expected, method))
    for name, indices in (("South", [0, 2, 4]), ("North", [1, 3, 5])):
        ys = [Y[i] for i in indices]
        if method in XY:
            draw([X[i] for i in indices], ys, label=name)
        else:
            draw(ys, label=name)
    assert _svg(ax) == _svg(expected)


@pytest.mark.parametrize("method", METHODS)
def test_by_with_pandas_and_polars_columns(method: str) -> None:
    expected = _axes(method)
    _call(expected, method, by=GROUPS)
    for column in (pd.Series(GROUPS, name="Not a label"), pl.Series("Not a label", GROUPS)):
        ax = _axes(method)
        _call(ax, method, by=column)
        assert _svg(ax) == _svg(expected)


@pytest.mark.parametrize("factory", [pd.Series, pl.Series])
def test_data_columns_keep_values_without_reading_series_names(factory: Any) -> None:
    actual = ub.Figure(6, 4).add_axes()
    expected = ub.Figure(6, 4).add_axes()
    actual.line(factory(X), factory(Y), by=factory(GROUPS))
    expected.line(X, Y, by=GROUPS)
    assert _svg(actual) == _svg(expected)


@pytest.mark.parametrize("groups", [["Only"] * 6, [""] * 6, GROUPS])
def test_by_awkward_columns(groups: list[str]) -> None:
    ax = _axes("line")
    ax.line(X, Y, by=groups)
    expected = _axes("line")
    for name in dict.fromkeys(groups):
        indices = [i for i, group in enumerate(groups) if group == name]
        expected.line([X[i] for i in indices], [Y[i] for i in indices], label=name)
    assert _svg(ax) == _svg(expected)


def test_by_awkward_columns_forty_groups_cycle_palette() -> None:
    groups = [f"G{i}" for i in range(40)] * 2
    xs, ys = [0.0] * 40 + [1.0] * 40, list(range(40)) * 2
    ax, expected = _axes("line"), _axes("line")
    ax.line(xs, ys, by=groups).legend(ub.LegendStyle.NONE)
    for i in range(40):
        expected.line([0, 1], [i, i], label=f"G{i}")
    expected.legend(ub.LegendStyle.NONE)
    assert _svg(ax) == _svg(expected)


def test_by_awkward_columns_categorical_uses_values_not_codes() -> None:
    actual = _axes("line")
    by = pd.Series(pd.Categorical(GROUPS, categories=["North", "Unused", "South"], ordered=True))
    actual.line(X, Y, by=by)
    expected = _axes("line").line(X, Y, by=GROUPS)
    assert _svg(actual) == _svg(expected)


@pytest.mark.parametrize("missing", [None, float("nan"), pd.NA, pd.NaT])
@pytest.mark.parametrize("method", METHODS)
def test_by_awkward_columns_missing_names_fail_at_first_index(method: str, missing: Any) -> None:
    ax = _axes(method)
    before = _svg(ax)
    column = pd.Series(["South", missing, "South", missing, "South", "North"], dtype=object)
    with pytest.raises(ub.DataError, match=r"by\[1\]") as caught:
        _call(ax, method, by=column)
    assert _details(caught.value)[0] == "by"
    assert _svg(ax) == before


@pytest.mark.parametrize("method", METHODS)
@pytest.mark.parametrize("groups", [["A"], ["A"] * 7])
def test_by_awkward_columns_lengths_differ(method: str, groups: list[str]) -> None:
    ax = _axes(method)
    before = _svg(ax)
    with pytest.raises(ub.DataError, match="by") as caught:
        _call(ax, method, by=groups)
    assert _details(caught.value)[0] == "by"
    assert _details(caught.value)[1]
    assert _svg(ax) == before


@pytest.mark.parametrize("method", METHODS)
@pytest.mark.parametrize("options", [{"color": ub.Color.RED}, {"label": "Combined"}])
def test_by_with_color_rejected(method: str, options: dict[str, object]) -> None:
    ax = _axes(method)
    before = _svg(ax)
    with pytest.raises(ub.UsageError) as caught:
        _call(ax, method, by=GROUPS, **options)
    assert _details(caught.value) == (next(iter(options)), GROUP_HINT)
    assert str(caught.value).startswith(f"Axes.{method}():")
    assert _svg(ax) == before


def test_by_with_color_mapping_rejected() -> None:
    with pytest.raises(ub.UsageError) as caught:
        ub.scatter(X, Y, by=GROUPS, color=ub.ColorBy(Y))
    assert _details(caught.value) == ("color", GROUP_HINT)


@pytest.mark.parametrize("method", METHODS)
def test_by_auto_keeps_current_behavior(method: str) -> None:
    # Unique categories avoid the ordinary bar duplicate-category error.
    if method == "bar":
        actual = ub.bar(["A", "B"], [1, 2], by=ub.AUTO, label="Kept", color=ub.Color.RED)
        expected = ub.bar(["A", "B"], [1, 2], label="Kept", color=ub.Color.RED)
    else:
        actual, expected = _axes(method), _axes(method)
        _call(actual, method, by=ub.AUTO, label="Kept", color=ub.Color.RED)
        _call(expected, method, label="Kept", color=ub.Color.RED)
    assert _svg(actual) == _svg(expected)


def test_bar_by_duplicate_pair_rejected() -> None:
    ax = ub.bar(["Kept"], [1])
    before = _svg(ax)
    with pytest.raises(ub.DataError) as caught:
        ax.bar(["Q1", "Q2", "Q1"], [1, 2, 3], by=["South", "North", "South"])
    assert "Q1" in str(caught.value) and "South" in str(caught.value)
    assert _details(caught.value) == (
        "categories",
        "aggregate first, for example with groupby().sum().",
    )
    assert _svg(ax) == before


@pytest.mark.parametrize("horizontal", [False, True])
def test_bar_by_missing_pair_draws_no_bar(horizontal: bool) -> None:
    def axes() -> Axes:
        figure = ub.Figure(6, 4)
        return (
            figure.add_axes(y=ub.Scale.category())
            if horizontal
            else figure.add_axes(x=ub.Scale.category())
        )

    actual, expected = axes(), axes()
    actual.bar(["Q2", "Q1", "Q3", "Q2"], [2, 3, 4, 5], by=["B", "A", "B", "A"])
    expected.grouped_bar(
        ["Q2", "Q1", "Q3"],
        [ub.Series("B", [2, float("nan"), 4]), ub.Series("A", [5, 3, float("nan")])],
    )
    assert _svg(actual) == _svg(expected)


def test_hist_bars_with_by_rejected() -> None:
    ax = _axes("hist")
    before = _svg(ax)
    with pytest.raises(ub.UsageError) as caught:
        ax.hist(Y, by=GROUPS)
    assert _details(caught.value) == ("style", "use Hist.STACKED or Hist.STEP.")
    assert _svg(ax) == before


@pytest.mark.parametrize("stat", list(ub.Stat))
@pytest.mark.parametrize("style", [ub.Hist.STEP, ub.Hist.STACKED])
def test_hist_by_shares_bins_and_uses_style_normalization(style: ub.Hist, stat: ub.Stat) -> None:
    from umber import _color, _series

    actual = ub.Figure(6, 4).add_axes()
    actual.hist(
        [0.5, 2.5, 3.5, 3.5, 3.5], bins=2, by=["B", "A", "A", "A", "A"], style=style, stat=stat
    )
    expected = ub.Figure(6, 4).add_axes()
    expected._native.grouped_hist(  # pyright: ignore[reportPrivateUsage]
        0,
        0,
        [
            ("B", np.array([0.5]), _color.native(ub.Color.AUTO)),
            _series.native(ub.Series("A", [2.5, 3.5, 3.5, 3.5])),
        ],
        2,
        style.value,
        False,
        stat.value,
    )
    assert _svg(actual) == _svg(expected)


@pytest.mark.parametrize("method", METHODS)
def test_late_bad_group_adds_nothing(method: str) -> None:
    ax = _axes(method)
    if method == "bar":
        ax.bar(["Kept"], [1], label="Kept")
    else:
        ax.line([0, 1], [1, 2], label="Kept")
    before = _svg(ax)
    draw = cast(Callable[..., object], getattr(ax, method))
    with pytest.raises(ub.DataError):
        if method in XY:
            draw([0, 1, 0, 1], [1, 2, 1, float("inf")], by=["Good", "Good", "Bad", "Bad"])
        elif method == "bar":
            draw(["A", "B"], [1, float("inf")], by=["Good", "Bad"])
        elif method == "hist":
            draw([1, 2, float("inf")], by=["Good", "Good", "Bad"], style=ub.Hist.STEP)
        else:
            draw([1, 2, float("nan")], by=["Good", "Good", "Bad"])
    assert _svg(ax) == before
    ax.scatter([0, 1] if method != "bar" else ["A", "B"], [2, 3], label="After")
    expected = _axes(method)
    if method == "bar":
        expected.bar(["Kept"], [1], label="Kept")
    else:
        expected.line([0, 1], [1, 2], label="Kept")
    expected.scatter([0, 1] if method != "bar" else ["A", "B"], [2, 3], label="After")
    assert _svg(ax) == _svg(expected)


def test_step_order_is_checked_per_group_and_failure_is_atomic() -> None:
    ax = ub.Figure().add_axes()
    ax.step([0, 2, 1, 3], [1, 2, 3, 4], by=["A", "B", "A", "B"])
    before = _svg(ax)
    with pytest.raises(ub.DataError):
        ax.step([0, 3, 1, 2], [1, 2, 3, 4], by=["A", "B", "A", "B"])
    assert _svg(ax) == before


@pytest.mark.parametrize("method", XY + ("ecdf",))
def test_by_preserves_time_columns(method: str) -> None:
    days = [dt.date(2026, 1, day) for day in (1, 1, 2, 3)]
    actual = ub.Figure(6, 4).add_axes(x=ub.Scale.time())
    expected = ub.Figure(6, 4).add_axes(x=ub.Scale.time())
    draw = cast(Callable[..., object], getattr(actual, method))
    plain = cast(Callable[..., object], getattr(expected, method))
    if method == "ecdf":
        draw(days, by=["B", "A", "B", "A"])
        plain([days[0], days[2]], label="B")
        plain([days[1], days[3]], label="A")
    else:
        draw(days, [1, 2, 3, 4], by=["B", "A", "B", "A"])
        plain([days[0], days[2]], [1, 3], label="B")
        plain([days[1], days[3]], [2, 4], label="A")
    assert _svg(actual) == _svg(expected)


@pytest.mark.parametrize("method", XY)
def test_by_preserves_category_coordinates(method: str) -> None:
    actual, expected = _axes("bar"), _axes("bar")
    draw = cast(Callable[..., object], getattr(actual, method))
    plain = cast(Callable[..., object], getattr(expected, method))
    draw(["Q2", "Q1", "Q3", "Q3"], [1, 2, 3, 4], by=["B", "A", "B", "A"])
    plain(["Q2", "Q3"], [1, 3], label="B")
    plain(["Q1", "Q3"], [2, 4], label="A")
    assert _svg(actual) == _svg(expected)


@pytest.mark.parametrize("method", ["line", "scatter", "hist", "bar"])
def test_quick_functions_forward_by(method: str) -> None:
    axes: Axes = ub.Figure().add_axes(
        x=ub.Scale.category() if method == "bar" else ub.Scale.linear()
    )
    quick = cast(Callable[..., Axes], getattr(ub, method))
    if method in XY:
        actual = quick(X, Y, by=GROUPS)
    elif method == "bar":
        actual = quick(["Q2", "Q2", "Q1", "Q1", "Q3", "Q3"], Y, by=GROUPS)
    else:
        actual = quick(Y, bins=[0, 2, 4, 6], style=ub.Hist.STEP, by=GROUPS)
    _call(axes, method, by=GROUPS)
    assert _svg(actual) == _svg(axes)


@pytest.mark.parametrize("value", [None, "group", 1, {"A"}, {"A": 1}, b"A"])
def test_by_container_types_are_checked(value: Any) -> None:
    with pytest.raises(TypeError) as caught:
        ub.line([0], [1], by=value)
    assert _details(caught.value)[0] == "by"


@pytest.mark.parametrize(
    "groups", [[["A"]], [1], [True], ["\ud800"], np.ma.array(["A"], mask=[True])]
)
def test_invalid_group_values_fail_before_mutation(groups: Any) -> None:
    ax = _axes("line")
    before = _svg(ax)
    with pytest.raises(ub.DataError) as caught:
        ax.line([0], [1], by=groups)
    assert _details(caught.value)[0] == "by"
    assert _svg(ax) == before


def test_by_error_highlights_the_column(tmp_path: Path) -> None:
    path = tmp_path / "user_code.py"
    line = 'ub.line([0, 1], [1, 2], by=["A", None])'
    path.write_text(f"import umber as ub\n{line}\n")
    with pytest.raises(ub.DataError) as caught:
        exec(compile(path.read_text(), str(path), "exec"), {})
    notes = cast(list[str], getattr(caught.value, "__notes__", []))
    start = line.index("by=")
    assert notes[0] == f"  {line}\n{' ' * (2 + start)}{'^' * len('by=["A", None]')}"


@pytest.mark.parametrize("method", METHODS)
def test_empty_by_is_rejected(method: str) -> None:
    ax = _axes(method)
    before = _svg(ax)
    draw = cast(Callable[..., object], getattr(ax, method))
    with pytest.raises(ub.DataError) as caught:
        if method in XY or method == "bar":
            draw([], [], by=[])
        else:
            draw([], by=[])
    assert _details(caught.value)[0] == "by"
    assert _svg(ax) == before


def test_split_by_preserves_row_order_and_dtypes() -> None:
    from umber._group import split_by

    times = np.array([11, 12, 13], dtype=np.int64)
    groups = split_by("Axes.line()", ["B", "A", "B"], times, [1.0, 2.0, 3.0])
    assert [label for label, _ in groups] == ["B", "A"]
    assert groups[0][1][0].tolist() == [11, 13]
    assert groups[0][1][0].dtype == np.dtype(np.int64)
    assert groups[0][1][1].tolist() == [1.0, 3.0]


@pytest.mark.parametrize("by", [np.array([np.nan, np.nan]), pd.Series([np.nan, np.nan])])
def test_missing_numeric_group_columns_name_first_index(by: Any) -> None:
    with pytest.raises(ub.DataError, match=r"by\[0\]") as caught:
        ub.line([0, 1], [1, 2], by=by)
    assert _details(caught.value)[0] == "by"


def test_column_name_property_is_never_read() -> None:
    class Column:
        def __init__(self, values: list[float] | list[str]) -> None:
            self.values = values

        @property
        def name(self) -> str:
            raise AssertionError("column metadata was accessed")

        def __array__(self) -> Any:
            return np.asarray(self.values)

    actual = ub.line(Column(X), Column(Y), by=Column(GROUPS))
    expected = ub.line(X, Y, by=GROUPS)
    assert _svg(actual) == _svg(expected)
    assert _svg(ub.line(Column(X), Column(Y))) == _svg(ub.line(X, Y))


@pytest.mark.parametrize("method", METHODS)
def test_by_checks_all_argument_types_before_values(method: str) -> None:
    ax = _axes(method)
    with pytest.raises(TypeError) as caught:
        _call(ax, method, by=[None] * 6, color="red")
    assert _details(caught.value)[0] == "color"


def test_by_bar_native_errors_keep_public_context_and_highlight(tmp_path: Path) -> None:
    path = tmp_path / "user_code.py"
    line = 'ax.bar(["A", "B"], [1, float("inf")], by=["Good", "Bad"])'
    path.write_text(
        f"import umber as ub\nax = ub.Figure().add_axes(x=ub.Scale.category())\n{line}\n"
    )
    with pytest.raises(ub.DataError) as caught:
        exec(compile(path.read_text(), str(path), "exec"), {})
    assert str(caught.value).startswith("Axes.bar():")
    assert _details(caught.value)[0] == "heights"
    notes = cast(list[str], getattr(caught.value, "__notes__", []))
    assert "^" in notes[0]


def test_by_label_checks_plain_string_content() -> None:
    class Label(str):
        def __len__(self) -> int:
            return 0

    with pytest.raises(ub.UsageError) as caught:
        ub.line(X, Y, by=GROUPS, label=Label("Combined"))
    assert _details(caught.value) == ("label", GROUP_HINT)
