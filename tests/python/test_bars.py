# pyright: reportPrivateUsage=false
import copy
import pickle
import traceback
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pandas as pd
import pytest

import umber as ub
from umber._errors import details
from umber._series import native

COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
TEXT_HINT = "pass a str; convert other values with str()."
TOO_LONG_HINT = "shorten the text; it would not fit the figure."
SERIES_HINT = 'pass a list of series, e.g. [Series("North", [3, 5])].'
SERIES_ITEM_HINT = 'make each series with Series(), e.g. Series("North", [3, 5]).'
CATEGORY_X_HINT = "pass x=Scale.category() to Figure.add_axes()."
BAR_GAP = "use NaN to leave a category without a bar."
SERIES_GAP = "use NaN to omit that height."
NOT_POSITIVE = "use a linear axis, or remove values that are not positive."
REPEAT = "combine repeated categories and sum their heights."
REPEAT_IN_SERIES = "combine repeated categories and sum their heights within each series."
FINITE = "values must be finite."
CATEGORIES_HINT = 'pass a list of names, e.g. ["A", "B"].'

type Bars = ub.Axes[str, float]


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def _raised(tmp_path: Path, line: str, setup: str = "") -> BaseException:
    """Run ``line`` as user code in a file, so the error is reported at its call site."""
    # A file per line: ``linecache`` would serve the first content for a reused name.
    path = tmp_path / f"user_code_{abs(hash((setup, line)))}.py"
    path.write_text(f"import umber as ub\n{setup}\n{line}\n")
    try:
        exec(compile(path.read_text(), str(path), "exec"), {})
    except Exception as err:
        return err
    raise AssertionError(f"{line} raised nothing")


def _package_frames(err: BaseException) -> list[str]:
    package = Path(ub.__file__).resolve().parent
    return [
        entry.name
        for entry in traceback.extract_tb(err.__traceback__)
        if Path(entry.filename).resolve().is_relative_to(package)
    ]


def _type_name(value: object) -> str:
    return "None" if value is None else type(value).__name__


def _bars(y: ub.Scale[float] | None = None) -> Bars:
    fig = ub.Figure(3, 2)
    if y is None:
        return fig.add_axes(x=ub.Scale.category())
    return fig.add_axes(x=ub.Scale.category(), y=y)


def _numeric() -> Bars:
    """Numeric axes, typed as bar axes so the core's own axis check is reached."""
    return cast(Bars, ub.Figure(3, 2).add_axes())


def _series(*values: float, label: str = "s") -> ub.Series:
    return ub.Series(label, list(values))


def _values(series: ub.Series) -> npt.NDArray[np.float64]:
    return native(series)[1]


# Series


def test_series_holds_its_values() -> None:
    series = ub.Series("North", [3, 5, 2], color=ub.Color.BLUE)
    label, values, color = native(series)
    assert label == "North"
    assert values.dtype == np.float64
    assert values.tolist() == [3.0, 5.0, 2.0]
    assert color == (2, 0x3366B8FF, 1.0)
    assert native(ub.Series("", []))[2] == (0, 0, 1.0)


def test_series_accepts_lists_arrays_and_series() -> None:
    for values in (
        [1, 2.5],
        (1.0, 2.5),
        np.array([1.0, 2.5], dtype=np.float32),
        pd.Series([1.0, 2.5]),
    ):
        assert _values(ub.Series("s", values)).tolist() == [1.0, 2.5]


def test_series_values_are_a_read_only_copy() -> None:
    source = np.array([1.0, 2.0, 3.0])
    series = ub.Series("s", source)
    values = _values(series)
    assert not np.shares_memory(values, source)
    source[0] = 99.0
    assert values.tolist() == [1.0, 2.0, 3.0]
    assert not values.flags.writeable
    with pytest.raises(ValueError, match="read-only"):
        values[0] = 5.0
    assert values.flags.c_contiguous


def test_series_copies_strided_input() -> None:
    source = np.arange(6.0)[::2]
    assert _values(ub.Series("s", source)).tolist() == [0.0, 2.0, 4.0]


def test_series_is_immutable() -> None:
    series = _series(1)
    with pytest.raises(AttributeError, match="immutable"):
        series._label = "other"
    with pytest.raises(AttributeError, match="immutable"):
        del series._values
    with pytest.raises(AttributeError, match="immutable"):
        series.__init__("other", [2.0])
    assert native(series)[0] == "s"
    assert not hasattr(series, "__dict__")


def test_series_repr_shows_label_and_length() -> None:
    assert repr(ub.Series("North", [3, 5, 2])) == "<umber.Series 'North' (3 values)>"
    assert repr(ub.Series("One", [3])) == "<umber.Series 'One' (1 value)>"
    assert repr(ub.Series("", [])) == "<umber.Series '' (0 values)>"


@pytest.mark.parametrize("duplicate", [copy.copy, copy.deepcopy])
def test_series_copy_is_the_series(duplicate: Callable[[ub.Series], ub.Series]) -> None:
    series = _series(1, 2)
    assert duplicate(series) is series


@pytest.mark.parametrize("protocol", range(pickle.HIGHEST_PROTOCOL + 1))
def test_series_pickles_by_value(protocol: int) -> None:
    series = ub.Series("North", [3, np.nan, 2], color=ub.Color.RED.alpha(0.5))
    restored = cast(ub.Series, pickle.loads(pickle.dumps(series, protocol)))
    assert restored is not series
    assert repr(restored) == repr(series)
    label, values, color = native(restored)
    assert (label, color) == (native(series)[0], native(series)[2])
    np.testing.assert_array_equal(values, _values(series))
    assert not values.flags.writeable
    ax = _bars().grouped_bar(["a", "b", "c"], [restored])
    assert ax.figure.to_svg() == _bars().grouped_bar(["a", "b", "c"], [series]).figure.to_svg()


def test_series_reports_the_package_as_its_module() -> None:
    assert ub.Series.__module__ == "umber"
    assert not hasattr(ub, "BarSeries")


def test_series_error_names_values() -> None:
    series = cast(Callable[..., object], ub.Series)
    with pytest.raises(ub.DataError) as info:
        series("A", [[1.0]])
    assert str(info.value).startswith("Series(): values")
    assert _parameter(info.value) == "values"


# (arguments, keywords, error class, message, parameter, hint)
SERIES_ERRORS: list[tuple[tuple[object, ...], dict[str, object], type, str, str, str]] = [
    (
        (3, [1.0]),
        {},
        TypeError,
        "Series(): label is int; label must be a str.",
        "label",
        TEXT_HINT,
    ),
    (
        ("x" * 1001, [1.0]),
        {},
        ub.StyleError,
        "Series(): label has 1,001 characters; text must be at most 1,000 characters.",
        "label",
        TOO_LONG_HINT,
    ),
    (
        ("s", [1.0]),
        {"color": "red"},
        TypeError,
        "Series(): color is str; color must be of type Color.",
        "color",
        COLOR_HINT,
    ),
    (
        ("s", "abc"),
        {},
        TypeError,
        "Series(): values is str; values must be a sequence or an array.",
        "values",
        "wrap a single value in a list.",
    ),
    (
        ("s", 3.0),
        {},
        TypeError,
        "Series(): values is float; values must be a sequence or an array.",
        "values",
        "wrap a single value in a list.",
    ),
    (
        ("s", np.ones((2, 2))),
        {},
        ub.DataError,
        "Series(): values has 2 dimensions; values must be one-dimensional.",
        "values",
        "pass one column, or flatten it with .ravel().",
    ),
    (
        ("s", np.array([True, False])),
        {},
        ub.DataError,
        "Series(): values has dtype bool; values must be numbers.",
        "values",
        "convert with .astype(float) if 0 and 1 are meant.",
    ),
    (
        ("s", [1.0, None]),
        {},
        ub.DataError,
        "Series(): values[1] is None; values must be numbers.",
        "values",
        "use NaN for missing values, e.g. series.to_numpy(dtype=float, na_value=np.nan).",
    ),
]


@pytest.mark.parametrize(
    ("arguments", "keywords", "cls", "message", "parameter", "hint"), SERIES_ERRORS
)
def test_series_checks_type_shape_and_dtype(
    arguments: tuple[object, ...],
    keywords: dict[str, object],
    cls: type[BaseException],
    message: str,
    parameter: str,
    hint: str,
) -> None:
    make = cast(Callable[..., object], ub.Series)
    with pytest.raises(cls) as caught:
        make(*arguments, **keywords)
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert _package_frames(caught.value) == []


class _RecordsConversion:
    """A data object that notes whether it was converted to an array."""

    def __init__(self) -> None:
        self.converted = False

    def __array__(self, dtype: object = None, copy: object = None) -> npt.NDArray[np.float64]:
        self.converted = True
        return np.array([1.0, 2.0])


def _parameter(err: BaseException) -> object:
    found = details(err)
    assert found is not None
    return found[0]


def test_series_types_are_checked_before_any_conversion() -> None:
    series = cast(Callable[..., object], ub.Series)
    with pytest.raises(TypeError) as caught:
        series("s", ["a"], color="red")
    assert _parameter(caught.value) == "color"
    with pytest.raises(TypeError) as caught:
        series("x" * 1001, None)
    assert _parameter(caught.value) == "values"
    with pytest.raises(TypeError) as caught:
        series(3, "abc", color="red")
    assert _parameter(caught.value) == "label"
    recorder = _RecordsConversion()
    with pytest.raises(TypeError) as caught:
        series("s", recorder, color="red")
    assert _parameter(caught.value) == "color"
    assert not recorder.converted
    with pytest.raises(ub.StyleError) as long:
        series("x" * 1001, [1.0])
    assert _parameter(long.value) == "label"
    with pytest.raises(ub.StyleError) as first:
        series("x" * 1001, ["a"])
    assert _parameter(first.value) == "label"


def test_series_leaves_finite_and_log_checks_to_the_chart() -> None:
    series = ub.Series("s", [np.inf, -1.0])
    assert _values(series)[0] == np.inf
    with pytest.raises(ub.DataError) as caught:
        _bars().grouped_bar(["a", "b"], [series])
    assert str(caught.value) == f"Axes.grouped_bar(): series[0].values[0] is inf; {FINITE}"


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ('ub.Series("North", "abc")', '"abc"'),
        ('ub.Series("North", [1, None])', "[1, None]"),
        ("ub.Series(label=3, values=[1])", "label=3"),
        ('ub.Series("North", [1], color="blue")', 'color="blue"'),
    ],
)
def test_series_errors_highlight_the_argument(tmp_path: Path, call: str, argument: str) -> None:
    err = _raised(tmp_path, call)
    assert _notes(err)[0] == _marked(call, argument)


# Bars


def test_bar_methods_return_the_same_axes() -> None:
    ax = _bars()
    assert ax.bar(["a"], [1.0]) is ax
    assert ax.grouped_bar(["b"], [_series(1)]) is ax
    assert ax.stacked_bar(["c"], [_series(1), _series(2)]) is ax
    assert isinstance(ax.bar(["d"], [1.0]).title("T"), ub.Axes)


def test_bar_accepts_lists_arrays_and_series() -> None:
    outputs: set[str] = set()
    data: list[tuple[ub.Values[str], ub.Values[float]]] = [
        (["a", "b", "c"], [1, 3, 2]),
        (("a", "b", "c"), np.array([1.0, 3.0, 2.0], dtype=np.float32)),
        (np.array(["a", "b", "c"]), pd.Series([1.0, 3.0, 2.0])),
        (pd.Series(["a", "b", "c"]), np.array([1, 3, 2], dtype=np.int64)),
    ]
    for categories, heights in data:
        outputs.add(_bars().bar(categories, heights).figure.to_svg())
    assert len(outputs) == 1


def test_bar_arguments_reach_the_figure() -> None:
    plain = _bars().bar(["North", "South"], [3, 5]).figure
    assert ">North</text>" in plain.to_svg(text=ub.SvgText.EDITABLE)
    red = _bars().bar(["a"], [1], color=ub.Color.RED).figure.to_svg()
    assert 'fill="#D64933"' in red
    labelled = _bars().bar(["a"], [1], label="2025").figure
    assert ">2025</text>" in labelled.to_svg(text=ub.SvgText.EDITABLE)


def test_grouped_and_stacked_bars_differ() -> None:
    series = [_series(1, 2, label="North"), _series(3, 1, label="South")]
    grouped = _bars().grouped_bar(["a", "b"], series).figure.to_svg()
    stacked = _bars().stacked_bar(["a", "b"], series).figure.to_svg()
    assert grouped != stacked
    empty = _bars().figure.to_svg()
    assert empty not in (grouped, stacked)


@pytest.mark.parametrize("name", ["grouped_bar", "stacked_bar"])
def test_series_order_and_colors_reach_the_figure(name: str) -> None:
    red = ub.Series("North", [1, 2], color=ub.Color.RED)
    blue = ub.Series("South", [2, 1], color=ub.Color.BLUE)
    method = cast(Callable[..., Bars], getattr(_bars(), name))
    svg = method(["a", "b"], (red, blue)).figure.to_svg(text=ub.SvgText.EDITABLE)
    assert 'fill="#D64933"' in svg
    assert 'fill="#3366B8"' in svg
    assert svg.index(">North</text>") < svg.index(">South</text>")


@pytest.mark.parametrize("name", ["grouped_bar", "stacked_bar"])
def test_one_series_object_can_be_reused(name: str) -> None:
    series = _series(1, 2)
    ax = _bars()
    method = cast(Callable[..., Bars], getattr(ax, name))
    method(["a", "b"], [series])
    method(["c", "d"], [series])
    assert ">d</text>" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)


@pytest.mark.parametrize("name", ["grouped_bar", "stacked_bar"])
@pytest.mark.parametrize("series", ["abc", b"ab", None, 3, {_series(1)}, iter([_series(1)])])
def test_series_must_be_a_sequence(name: str, series: object) -> None:
    ax = _bars()
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        cast(Callable[..., object], getattr(ax, name))(["a"], series)
    assert str(caught.value) == (
        f"Axes.{name}(): series is {_type_name(series)}; series must be a sequence of Series."
    )
    assert details(caught.value) == ("series", SERIES_HINT)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("name", ["grouped_bar", "stacked_bar"])
@pytest.mark.parametrize("item", ["North", [1.0], None, ("s", [1.0])])
def test_series_items_must_be_series(name: str, item: object) -> None:
    ax = _bars()
    with pytest.raises(TypeError) as caught:
        cast(Callable[..., object], getattr(ax, name))(["a"], [_series(1), item])
    assert str(caught.value) == (
        f"Axes.{name}(): series[1] is {_type_name(item)}; series[1] must be of type Series."
    )
    assert details(caught.value) == ("series", SERIES_ITEM_HINT)


@pytest.mark.parametrize("name", ["grouped_bar", "stacked_bar"])
def test_categories_type_is_checked_before_series(name: str) -> None:
    method = cast(Callable[..., object], getattr(_bars(), name))
    with pytest.raises(TypeError) as caught:
        method("abc", "xyz")
    assert details(caught.value) == ("categories", CATEGORIES_HINT)
    with pytest.raises(TypeError) as caught:
        method([1, 2], None)
    assert details(caught.value) == ("series", SERIES_HINT)
    recorder = _RecordsConversion()
    with pytest.raises(TypeError) as caught:
        method(recorder, None)
    assert _parameter(caught.value) == "series"
    assert not recorder.converted
    with pytest.raises(ub.DataError) as data:
        method([1, 2], [ub.Series("s", [1.0, 2.0])])
    assert _parameter(data.value) == "categories"


def test_bar_types_are_checked_before_any_conversion() -> None:
    bar = cast(Callable[..., object], _bars().bar)
    with pytest.raises(TypeError) as caught:
        bar([1, 2], None)
    assert _parameter(caught.value) == "heights"
    with pytest.raises(TypeError) as caught:
        bar([1, 2], [1.0, 2.0], label="x" * 1001, color="red")
    assert _parameter(caught.value) == "color"
    recorder = _RecordsConversion()
    with pytest.raises(TypeError) as caught:
        bar(recorder, None)
    assert _parameter(caught.value) == "heights"
    assert not recorder.converted
    with pytest.raises(ub.DataError) as data:
        bar([1, 2], [1.0, 2.0])
    assert _parameter(data.value) == "categories"


# (method, axes factory, call, error class, message, parameter, hint)
BAR_ERRORS: list[tuple[str, Callable[[], Bars], Callable[[Bars], object], type, str, Any, Any]] = [
    (
        "bar length",
        _bars,
        lambda ax: ax.bar(["a", "b"], [1.0]),
        ub.DataError,
        "Axes.bar(): categories has 2 values but heights has 1; categories and heights must "
        "have the same length.",
        None,
        None,
    ),
    (
        "bar linear x",
        _numeric,
        lambda ax: ax.bar(["a"], [1.0]),
        ub.DataError,
        "Axes.bar(): categories holds names but the x axis is not a category axis; "
        "use a category x axis.",
        None,
        CATEGORY_X_HINT,
    ),
    (
        "bar repeated category",
        _bars,
        lambda ax: ax.bar(["a", "a"], [1.0, 2.0]),
        ub.DataError,
        'Axes.bar(): categories[1] repeats "a" from categories[0]; each category must appear once.',
        "categories",
        REPEAT,
    ),
    (
        "bar infinite",
        _bars,
        lambda ax: ax.bar(["a", "b"], [1.0, np.inf]),
        ub.DataError,
        f"Axes.bar(): heights[1] is inf; {FINITE}",
        "heights",
        BAR_GAP,
    ),
    (
        "bar log not positive",
        lambda: _bars(ub.Scale.log()),
        lambda ax: ax.bar(["a", "b"], [1.0, 0.0]),
        ub.DataError,
        "Axes.bar(): heights[1] is 0 on a log axis; values must be positive.",
        "heights",
        NOT_POSITIVE,
    ),
    (
        "bar heights type",
        _bars,
        lambda ax: ax.bar(["a"], cast(list[float], "1")),
        TypeError,
        "Axes.bar(): heights is str; heights must be a sequence or an array.",
        "heights",
        "wrap a single value in a list.",
    ),
    (
        "bar categories type",
        _bars,
        lambda ax: ax.bar(cast(list[str], "ab"), [1.0, 2.0]),
        TypeError,
        "Axes.bar(): categories is str; categories must be a sequence or an array.",
        "categories",
        'pass a list of names, e.g. ["A", "B"].',
    ),
    (
        "bar categories not names",
        _bars,
        lambda ax: ax.bar(cast(list[str], ["a", 2]), [1.0, 2.0]),
        ub.DataError,
        "Axes.bar(): categories[1] is 2; categories must be strings.",
        "categories",
        "convert each name with str() first.",
    ),
    (
        "bar label type",
        _bars,
        lambda ax: ax.bar(["a"], [1.0], label=cast(str, 3)),
        TypeError,
        "Axes.bar(): label is int; label must be a str.",
        "label",
        TEXT_HINT,
    ),
    (
        "bar color type",
        _bars,
        lambda ax: ax.bar(["a"], [1.0], color=cast(ub.Color, "red")),
        TypeError,
        "Axes.bar(): color is str; color must be of type Color.",
        "color",
        COLOR_HINT,
    ),
    (
        "grouped no series",
        _bars,
        lambda ax: ax.grouped_bar(["a"], []),
        ub.DataError,
        "Axes.grouped_bar(): series is empty; pass at least one series.",
        "series",
        None,
    ),
    (
        "grouped series length",
        _bars,
        lambda ax: ax.grouped_bar(["a"], [_series(1), _series()]),
        ub.DataError,
        "Axes.grouped_bar(): categories has 1 values but series[1].values has 0; each series "
        "must have one value per category.",
        "series",
        None,
    ),
    (
        "grouped repeated category",
        _bars,
        lambda ax: ax.grouped_bar(["a", "a"], [_series(1, 2)]),
        ub.DataError,
        'Axes.grouped_bar(): categories[1] repeats "a" from categories[0]; each category must '
        "appear once.",
        "categories",
        REPEAT_IN_SERIES,
    ),
    (
        "grouped infinite",
        _bars,
        lambda ax: ax.grouped_bar(["a", "b"], [_series(1, 2), _series(1, np.inf)]),
        ub.DataError,
        f"Axes.grouped_bar(): series[1].values[1] is inf; {FINITE}",
        "series",
        SERIES_GAP,
    ),
    (
        "grouped log not positive",
        lambda: _bars(ub.Scale.log()),
        lambda ax: ax.grouped_bar(["a"], [_series(-2)]),
        ub.DataError,
        "Axes.grouped_bar(): series[0].values[0] is -2 on a log axis; values must be positive.",
        "series",
        NOT_POSITIVE,
    ),
    (
        "grouped linear x",
        _numeric,
        lambda ax: ax.grouped_bar(["a"], [_series(1)]),
        ub.DataError,
        "Axes.grouped_bar(): categories holds names but the x axis is not a category axis; "
        "use a category x axis.",
        None,
        CATEGORY_X_HINT,
    ),
    (
        "stacked no series",
        _bars,
        lambda ax: ax.stacked_bar(["a"], ()),
        ub.DataError,
        "Axes.stacked_bar(): series is empty; pass at least one series.",
        "series",
        None,
    ),
    (
        "stacked series length",
        _bars,
        lambda ax: ax.stacked_bar(["a", "b"], [_series(1, 2), _series(1)]),
        ub.DataError,
        "Axes.stacked_bar(): categories has 2 values but series[1].values has 1; each series "
        "must have one value per category.",
        "series",
        None,
    ),
    (
        "stacked repeated category",
        _bars,
        lambda ax: ax.stacked_bar(["a", "b", "a"], [_series(1, 2, 3)]),
        ub.DataError,
        'Axes.stacked_bar(): categories[2] repeats "a" from categories[0]; each category must '
        "appear once.",
        "categories",
        REPEAT_IN_SERIES,
    ),
    (
        "stacked infinite",
        _bars,
        lambda ax: ax.stacked_bar(["a"], [_series(-np.inf)]),
        ub.DataError,
        f"Axes.stacked_bar(): series[0].values[0] is -inf; {FINITE}",
        "series",
        SERIES_GAP,
    ),
    (
        "stacked log not positive",
        lambda: _bars(ub.Scale.log()),
        lambda ax: ax.stacked_bar(["a"], [_series(-1)]),
        ub.DataError,
        "Axes.stacked_bar(): series[0].values[0] is -1 on a log axis; values must be positive.",
        "series",
        NOT_POSITIVE,
    ),
    (
        "stacked total",
        _bars,
        lambda ax: ax.stacked_bar(["a"], [_series(1e300), _series(1e300)]),
        ub.DataError,
        'Axes.stacked_bar(): the heights of categories[0] ("a") stack to 2e300; each stack '
        "must stay within 1e300 on both sides of 0.",
        "series",
        None,
    ),
    (
        "stacked linear x",
        _numeric,
        lambda ax: ax.stacked_bar(["a"], [_series(1)]),
        ub.DataError,
        "Axes.stacked_bar(): categories holds names but the x axis is not a category axis; "
        "use a category x axis.",
        None,
        CATEGORY_X_HINT,
    ),
]


@pytest.mark.parametrize(
    ("make", "call", "cls", "message", "parameter", "hint"),
    [case[1:] for case in BAR_ERRORS],
    ids=[case[0] for case in BAR_ERRORS],
)
def test_bar_errors_leave_the_figure_unchanged(
    make: Callable[[], Bars],
    call: Callable[[Bars], object],
    cls: type[BaseException],
    message: str,
    parameter: object,
    hint: object,
) -> None:
    ax = make().title("Kept")
    before = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    with pytest.raises(cls) as caught:
        call(ax)
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert _package_frames(caught.value) == []
    assert ax.figure.to_svg(text=ub.SvgText.EDITABLE) == before


# (axes factory source, height, message after the bad height, hint)
RANGE_ERRORS: list[tuple[str, str, str, str | None]] = [
    (
        "ub.Figure().add_axes(x=ub.Scale.category())",
        "1e301",
        "is 1e301; magnitudes must not exceed 1e300.",
        None,
    ),
    (
        "ub.Figure().add_axes(x=ub.Scale.category())",
        "-1e301",
        "is -1e301; magnitudes must not exceed 1e300.",
        None,
    ),
    (
        "ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.log())",
        "1e-301",
        "is 1e-301 on a log axis; values must be at least 1e-300.",
        "use a linear axis, or rescale the values.",
    ),
]
# (method, the call with ``{h}`` for the bad height, the highlighted argument, the label of
# the bad height in the message, the parameter)
RANGE_CALLS: list[tuple[str, str, str, str, str]] = [
    ("bar", "ax.bar(['a', 'b'], [1.0, {h}])", "[1.0, {h}]", "heights[1]", "heights"),
    (
        "grouped_bar",
        "ax.grouped_bar(['a', 'b'], [ub.Series('s', [1.0, {h}])])",
        "[ub.Series('s', [1.0, {h}])]",
        "series[0].values[1]",
        "series",
    ),
    (
        "stacked_bar",
        "ax.stacked_bar(['a', 'b'], [ub.Series('s', [1.0, {h}])])",
        "[ub.Series('s', [1.0, {h}])]",
        "series[0].values[1]",
        "series",
    ),
]


@pytest.mark.parametrize(
    ("make", "height", "rule", "hint"),
    RANGE_ERRORS,
    ids=["above 1e300", "below -1e300", "below 1e-300 on a log axis"],
)
@pytest.mark.parametrize(
    ("name", "call", "argument", "label", "parameter"),
    RANGE_CALLS,
    ids=[case[0] for case in RANGE_CALLS],
)
def test_heights_out_of_range_leave_the_figure_unchanged(
    tmp_path: Path,
    make: str,
    height: str,
    rule: str,
    hint: str | None,
    name: str,
    call: str,
    argument: str,
    label: str,
    parameter: str,
) -> None:
    line = call.format(h=height)
    message = f"Axes.{name}(): {label} {rule}"
    ax = cast(Bars, eval(make, {"ub": ub})).title("Kept")
    before = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    with pytest.raises(ub.DataError) as caught:
        eval(line, {"ub": ub, "ax": ax})
    assert type(caught.value) is ub.DataError
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert _package_frames(caught.value) == []
    assert ax.figure.to_svg(text=ub.SvgText.EDITABLE) == before
    err = _raised(tmp_path, line, f"ax = {make}")
    assert type(err) is ub.DataError
    assert str(err) == message
    notes = [_marked(line, argument.format(h=height))]
    assert _notes(err) == notes + ([] if hint is None else [f"hint: {hint}"])


@pytest.mark.parametrize("name", ["bar", "grouped_bar", "stacked_bar"])
def test_category_names_are_capped(name: str) -> None:
    ax = _bars()
    method = cast(Callable[..., object], getattr(ax, name))
    data: object = [1.0] if name == "bar" else [_series(1)]
    with pytest.raises(ub.StyleError) as caught:
        method(["x" * 1001], data)
    assert str(caught.value) == (
        f"Axes.{name}(): categories[0] has 1,001 characters; text must be at most 1,000 characters."
    )
    assert details(caught.value) == ("categories", TOO_LONG_HINT)


SETUP = "\n".join(
    [
        "inf = float('inf')",
        "cats = ub.Figure().add_axes(x=ub.Scale.category())",
        "good = ub.Series('North', [1.0, 2.0])",
        "bad = ub.Series('South', [1.0, inf])",
    ]
)


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("cats.bar(['a', 'b'], [1.0, inf], label='North')", "[1.0, inf]"),
        ("cats.bar(['a', 'a'], [1.0, 2.0])", "['a', 'a']"),
        ("cats.bar(categories=['a', 'b'], heights=[1, inf])", "heights=[1, inf]"),
        ("cats.bar(['a'], [1.0], color='red')", "color='red'"),
        ("cats.grouped_bar(['a', 'b'], [good, bad])", "[good, bad]"),
        ("cats.grouped_bar(['a', 'b'], series=[good, bad])", "series=[good, bad]"),
        ("cats.stacked_bar(['a', 'b'], [good, 'bad'])", "[good, 'bad']"),
        ("cats.title('T').stacked_bar(['a', 'b'],\n    [good, bad])", "[good, bad]"),
    ],
)
def test_bar_errors_highlight_the_argument(tmp_path: Path, call: str, argument: str) -> None:
    err = _raised(tmp_path, call, SETUP)
    shown = call.splitlines()[-1].strip() if "\n" in call else call
    assert _notes(err)[0] == _marked(shown, argument)


def test_grouped_error_output_shows_the_series_and_the_hint(tmp_path: Path) -> None:
    call = "cats.grouped_bar(['a', 'b'], [good, bad])"
    err = _raised(tmp_path, call, SETUP)
    assert str(err) == f"Axes.grouped_bar(): series[1].values[1] is inf; {FINITE}"
    assert _notes(err) == [_marked(call, "[good, bad]"), f"hint: {SERIES_GAP}"]


def test_masked_heights_are_a_value_error_after_the_types() -> None:
    masked = np.ma.array([1.0, 2.0])
    series = cast(Callable[..., object], ub.Series)
    with pytest.raises(TypeError) as caught:
        series("s", masked, color="red")
    assert _parameter(caught.value) == "color"
    with pytest.raises(ub.DataError) as data:
        series("s", masked)
    assert _parameter(data.value) == "values"
    with pytest.raises(TypeError) as caught:
        cast(Callable[..., object], _bars().bar)(masked, None)
    assert _parameter(caught.value) == "heights"
