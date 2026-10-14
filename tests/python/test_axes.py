# pyright: reportPrivateUsage=false
import copy
import os
import pickle
import subprocess
import sys
import traceback
import typing
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'
DASH_HINT = "use Dash.SOLID, Dash.DASHED or Dash.DOTTED."
MARKER_HINT = "use Marker.CIRCLE, Marker.SQUARE, Marker.TRIANGLE, Marker.DIAMOND or Marker.CROSS."
LEGEND_HINT = "use LegendStyle.HEADER, LegendStyle.DIRECT, LegendStyle.CHIPS or LegendStyle.NONE."
TEXT_HINT = "pass a str; convert other values with str()."
TOO_LONG_HINT = "shorten the text; it would not fit the figure."
NUMBER_HINT = "pass a number such as 1.5."
LINE_GAP = "use NaN to leave a gap in the line."
X_SCALE_HINT = "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."
CATEGORY_X_HINT = "pass x=Scale.category() to Figure.add_axes()."
COPY_HINT = "copy the figure instead: copy.copy(ax.figure)."
TEXTS = ["title", "subtitle", "xlabel", "ylabel"]

type AnyAxes = ub.Axes[Any, Any]


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


def _frames(err: BaseException) -> list[str]:
    return [entry.name for entry in traceback.extract_tb(err.__traceback__)]


def _package_frames(err: BaseException) -> list[str]:
    package = Path(ub.__file__).resolve().parent
    return [
        entry.name
        for entry in traceback.extract_tb(err.__traceback__)
        if Path(entry.filename).resolve().is_relative_to(package)
    ]


def _axes(x: ub.Scale[Any] | None = None) -> AnyAxes:
    fig = ub.Figure(3, 2)
    return fig.add_axes() if x is None else fig.add_axes(x=x)


def _method(ax: AnyAxes, name: str) -> Callable[..., object]:
    return cast(Callable[..., object], getattr(ax, name))


def _type_name(value: object) -> str:
    return "None" if value is None else type(value).__name__


# Construction and structure


def test_construction_is_refused_with_a_hint() -> None:
    for make in (ub.Axes, ub.Axes[float, float]):
        with pytest.raises(TypeError) as caught:
            make()
        assert str(caught.value) == (
            "Axes(): Axes has no public constructor; axes belong to a figure."
        )
        assert details(caught.value) == (
            None,
            "call add_axes() on a Figure: ub.Figure().add_axes().",
        )
        assert _package_frames(caught.value) == []
        assert _frames(caught.value)[0] == "test_construction_is_refused_with_a_hint"


def test_figure_property() -> None:
    fig = ub.Figure()
    ax = fig.add_axes()
    assert ax.figure is fig
    with pytest.raises(AttributeError):
        ax.figure = ub.Figure()  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue]


def test_figure_property_type_hints_resolve() -> None:
    getter = cast(property, vars(ub.Axes)["figure"]).fget
    assert getter is not None
    assert typing.get_type_hints(getter) == {"return": ub.Figure}


def test_attach_type_hints_resolve() -> None:
    from umber._axes import attach

    assert typing.get_type_hints(attach)["figure"] is ub.Figure


def test_axes_have_no_instance_dictionary() -> None:
    ax = _axes()
    assert not hasattr(ax, "__dict__")
    with pytest.raises(AttributeError):
        ax.line = ax.line  # type: ignore[method-assign]


def test_axes_report_the_package_as_their_module() -> None:
    assert ub.Axes.__module__ == "umber"
    marks = [
        "line",
        "area",
        "scatter",
        "bar",
        "grouped_bar",
        "stacked_bar",
        "hist",
        "step",
        "hline",
        "vline",
        "hspan",
        "vspan",
    ]
    ticks = ["xticks", "yticks", "xticks_labeled", "yticks_labeled", "xtick_format", "ytick_format"]
    for name in [*TEXTS, "xlim", "ylim", "legend", *marks, *ticks]:
        assert getattr(ub.Axes, name).__module__ == "umber"
    figure = cast(property, vars(ub.Axes)["figure"])
    assert figure.fget is not None
    assert figure.fget.__module__ == "umber"


@pytest.mark.parametrize("duplicate", [copy.copy, copy.deepcopy])
def test_copying_is_refused(duplicate: Callable[[AnyAxes], AnyAxes]) -> None:
    ax = _axes()
    with pytest.raises(TypeError) as caught:
        duplicate(ax)
    assert str(caught.value) == (
        "Axes: an Axes cannot be copied on its own; it belongs to its figure."
    )
    assert details(caught.value) == (None, COPY_HINT)
    assert _notes(caught.value) == [f"hint: {COPY_HINT}"]
    # The ``copy`` module's frame is not umber's; it stays.
    assert _frames(caught.value) == ["test_copying_is_refused", duplicate.__name__]


@pytest.mark.parametrize("protocol", range(pickle.HIGHEST_PROTOCOL + 1))
def test_pickling_is_refused(protocol: int) -> None:
    with pytest.raises(TypeError) as caught:
        pickle.dumps(_axes(), protocol)
    assert str(caught.value) == "Axes: an Axes cannot be pickled."
    assert details(caught.value) == (
        None,
        "save the figure with save_png() or save_svg(), or build it again.",
    )
    assert _frames(caught.value) == ["test_pickling_is_refused"]


# Chaining


def test_every_method_returns_the_same_axes() -> None:
    ax = _axes()
    assert ax.title("T") is ax
    assert ax.subtitle("S") is ax
    assert ax.xlabel("X") is ax
    assert ax.ylabel("Y") is ax
    assert ax.xlim(0, 1) is ax
    assert ax.ylim(0, 1) is ax
    assert ax.legend(ub.LegendStyle.CHIPS) is ax
    assert ax.line([0, 1], [0, 1]) is ax
    assert ax.scatter([0, 1], [0, 1]) is ax
    chained = _axes().line([0, 1], [1, 2]).scatter([0, 1], [2, 1]).title("T").xlim(0, 2)
    assert isinstance(chained, ub.Axes)


# Texts


@pytest.mark.parametrize("name", TEXTS)
def test_texts_are_drawn(name: str) -> None:
    ax = _axes()
    _method(ax, name)("Température")
    assert ">Température</text>" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    _method(ax, name)("")
    assert ">Température</text>" not in ax.figure.to_svg(text=ub.SvgText.EDITABLE)


@pytest.mark.parametrize("name", TEXTS)
@pytest.mark.parametrize("value", [None, 3, b"Title", ["Title"]])
def test_texts_must_be_str(name: str, value: object) -> None:
    with pytest.raises(TypeError) as caught:
        _method(_axes(), name)(value)
    assert str(caught.value) == (f"Axes.{name}(): text is {_type_name(value)}; text must be a str.")
    assert details(caught.value) == ("text", TEXT_HINT)


@pytest.mark.parametrize("name", TEXTS)
def test_texts_are_capped(name: str) -> None:
    _method(_axes(), name)("x" * 1000)
    with pytest.raises(ub.StyleError) as caught:
        _method(_axes(), name)("x" * 1001)
    assert str(caught.value) == (
        f"Axes.{name}(): text has 1,001 characters; text must be at most 1,000 characters."
    )
    assert details(caught.value) == ("text", TOO_LONG_HINT)


def test_text_error_highlights_the_text(tmp_path: Path) -> None:
    line = "ub.Figure().add_axes().line([0, 1], [1, 2]).title(42).xlabel('x')"
    err = _raised(tmp_path, line)
    assert isinstance(err, TypeError)
    assert _notes(err) == [_marked(line, "42"), f"hint: {TEXT_HINT}"]


# Limits


@pytest.mark.parametrize("name", ["xlim", "ylim"])
def test_limits_change_the_output(name: str) -> None:
    ax = _axes().line([0, 1, 2], [1, 3, 2])
    before = ax.figure.to_svg()
    _method(ax, name)(-1, np.float32(5.5))
    assert ax.figure.to_svg() != before


@pytest.mark.parametrize("name", ["xlim", "ylim"])
def test_limits_out_of_order(name: str) -> None:
    ax = _axes()
    with pytest.raises(ub.StyleError) as caught:
        _method(ax, name)(2, 1)
    assert str(caught.value) == (
        f"Axes.{name}(): the limits are 2 and 1; both must be finite and the lower limit "
        "must be below the upper limit."
    )


@pytest.mark.parametrize("name", ["xlim", "ylim"])
@pytest.mark.parametrize(("low", "high", "parameter"), [("0", 1, "low"), (0, None, "high")])
def test_limits_must_be_numbers(name: str, low: object, high: object, parameter: str) -> None:
    with pytest.raises(TypeError) as caught:
        _method(_axes(), name)(low, high)
    value = low if parameter == "low" else high
    assert str(caught.value) == (
        f"Axes.{name}(): {parameter} is {_type_name(value)}; {parameter} must be a float."
    )
    assert details(caught.value) == (parameter, NUMBER_HINT)


def test_limits_on_a_category_axis_are_refused() -> None:
    ax = cast(ub.Axes[float, float], _axes(ub.Scale.category()))
    with pytest.raises(ub.UsageError) as caught:
        ax.xlim(0, 1)
    assert str(caught.value) == (
        "Axes.xlim(): this is a category axis; limits apply to linear and log axes only."
    )
    assert details(caught.value) == (None, X_SCALE_HINT)


def test_limit_error_highlights_the_limit(tmp_path: Path) -> None:
    line = "ub.Figure().add_axes(y=ub.Scale.log()).ylim(0, 10)"
    err = _raised(tmp_path, line)
    assert isinstance(err, ub.StyleError)
    assert _notes(err) == [
        _marked(line, "0"),
        "hint: use a linear axis if either limit is not positive.",
    ]


# Legend


def test_legend_changes_the_output() -> None:
    ax = _axes().line([0, 1], [1, 2], label="North")
    before = ax.figure.to_svg()
    ax.legend(ub.LegendStyle.NONE)
    assert ax.figure.to_svg() != before


@pytest.mark.parametrize("style", ["CHIPS", 2, None, ub.GridLines.NONE])
def test_legend_style_must_be_a_legend_style(style: object) -> None:
    with pytest.raises(TypeError) as caught:
        _axes().legend(cast(ub.LegendStyle, style))
    assert str(caught.value) == (
        f"Axes.legend(): style is {_type_name(style)}; style must be of type LegendStyle."
    )
    assert details(caught.value) == ("style", LEGEND_HINT)


def test_legend_error_highlights_the_style(tmp_path: Path) -> None:
    line = "ub.Figure().add_axes().legend(ub.GridLines.NONE)"
    err = _raised(tmp_path, line)
    assert _notes(err) == [_marked(line, "ub.GridLines.NONE"), f"hint: {LEGEND_HINT}"]


# Lines and scatter plots


@pytest.mark.parametrize("name", ["line", "scatter"])
def test_marks_accept_lists_arrays_and_series(name: str) -> None:
    outputs: set[str] = set()
    for x, y in [
        ([0, 1, 2], [1.0, 3.0, 2.0]),
        ((0.0, 1.0, 2.0), np.array([1, 3, 2], dtype=np.int16)),
        (np.arange(3.0), pd.Series([1.0, 3.0, 2.0])),
        (range(3), np.array([1.0, 3.0, 2.0], dtype=np.float32)),
    ]:
        ax = _axes()
        _method(ax, name)(x, y)
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


@pytest.mark.parametrize("name", ["line", "scatter"])
def test_marks_on_a_category_axis_take_names(name: str) -> None:
    ax = _axes(ub.Scale.category())
    _method(ax, name)(["Q1", "Q2", "Q3"], [1.0, 3.0, 2.0])
    _method(ax, name)(np.array(["Q2", "Q4"]), [2.0, 1.0])
    assert ">Q4</text>" in ax.figure.to_svg(text=ub.SvgText.EDITABLE)


@pytest.mark.parametrize("name", ["line", "scatter"])
@pytest.mark.parametrize("category", [False, True])
def test_empty_x_follows_the_x_scale(name: str, category: bool) -> None:
    ax = _axes(ub.Scale.category() if category else None)
    before = ax.figure.to_svg()
    _method(ax, name)([], [])
    _method(ax, name)(np.array([]), np.array([]))
    assert ax.figure.to_svg() == before


def test_style_arguments_reach_the_figure() -> None:
    plain = _axes().line([0, 1], [0, 1]).figure.to_svg()
    assert _axes().line([0, 1], [0, 1], dash=ub.Dash.DOTTED).figure.to_svg() != plain
    red = _axes().line([0, 1], [0, 1], color=ub.Color.RED).figure.to_svg()
    assert 'stroke="#D64933"' in red
    labelled = _axes().line([0, 1], [0, 1], label="North").figure
    assert ">North</text>" in labelled.to_svg(text=ub.SvgText.EDITABLE)
    dots = _axes().scatter([0, 1], [0, 1]).figure.to_svg()
    crosses = _axes().scatter([0, 1], [0, 1], marker=ub.Marker.CROSS).figure.to_svg()
    assert dots != crosses


# (method, keyword, a wrong value, the expected type in the message, hint)
STYLE_TYPE_ERRORS: list[tuple[str, str, object, str, str]] = [
    ("line", "label", 3, "a str", TEXT_HINT),
    ("line", "color", "red", "of type Color", COLOR_HINT),
    ("line", "dash", "--", "of type Dash", DASH_HINT),
    ("line", "dash", ub.Marker.CIRCLE, "of type Dash", DASH_HINT),
    ("scatter", "label", None, "a str", TEXT_HINT),
    ("scatter", "color", (1, 0, 0), "of type Color", COLOR_HINT),
    ("scatter", "marker", "o", "of type Marker", MARKER_HINT),
]


@pytest.mark.parametrize(("name", "keyword", "value", "expected", "hint"), STYLE_TYPE_ERRORS)
def test_style_arguments_are_type_checked(
    name: str, keyword: str, value: object, expected: str, hint: str
) -> None:
    ax = _axes()
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        _method(ax, name)([0, 1], [0, 1], **{keyword: value})
    assert str(caught.value) == (
        f"Axes.{name}(): {keyword} is {_type_name(value)}; {keyword} must be {expected}."
    )
    assert details(caught.value) == (keyword, hint)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("name", ["line", "scatter"])
def test_labels_are_capped(name: str) -> None:
    with pytest.raises(ub.StyleError) as caught:
        _method(_axes(), name)([0], [0], label="x" * 1001)
    assert details(caught.value) == ("label", TOO_LONG_HINT)


def test_x_type_is_checked_before_the_style_arguments() -> None:
    with pytest.raises(TypeError) as caught:
        _axes().line(cast(list[float], "abc"), [1.0], color=cast(ub.Color, "red"))
    assert _parameter(caught.value) == "x"


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


@pytest.mark.parametrize("name", ["line", "scatter"])
def test_types_are_checked_before_any_conversion(name: str) -> None:
    method = _method(_axes(), name)
    with pytest.raises(TypeError) as caught:
        method([0, "bad"], None)
    assert _parameter(caught.value) == "y"
    with pytest.raises(TypeError) as caught:
        method([0, "bad"], [1, 2], label="x" * 1001, color="red")
    assert _parameter(caught.value) == "color"
    recorder = _RecordsConversion()
    with pytest.raises(TypeError) as caught:
        method(recorder, None)
    assert _parameter(caught.value) == "y"
    assert not recorder.converted
    with pytest.raises(ub.DataError) as data:
        method([0, "bad"], [1, 2])
    assert _parameter(data.value) == "x"


def test_x_is_converted_before_y() -> None:
    with pytest.raises(TypeError) as caught:
        _axes().line(cast(list[float], "abc"), cast(list[float], "def"))
    assert str(caught.value) == "Axes.line(): x is str; x must be a sequence or an array."


# (method, axes factory, x, y, error class, message, parameter, hint)
DATA_ERRORS: list[tuple[str, Callable[[], AnyAxes], object, object, type, str, object, object]] = [
    (
        "line",
        _axes,
        [0, 1, 2],
        [1.0, np.inf, 2.0],
        ub.DataError,
        "Axes.line(): y[1] is inf; values must be finite.",
        "y",
        LINE_GAP,
    ),
    (
        "line",
        _axes,
        [0, 1],
        [1.0],
        ub.DataError,
        "Axes.line(): x has 2 values but y has 1; x and y must have the same length.",
        None,
        None,
    ),
    (
        "line",
        _axes,
        ["a", "b"],
        [1.0, 2.0],
        ub.DataError,
        "Axes.line(): x holds category names but the x axis is not a category axis; pass numbers.",
        None,
        CATEGORY_X_HINT,
    ),
    (
        "line",
        lambda: _axes(ub.Scale.category()),
        [1.0, 2.0],
        [1.0, 2.0],
        ub.DataError,
        "Axes.line(): x holds numbers but the x axis is a category axis; pass category names.",
        None,
        X_SCALE_HINT,
    ),
    (
        "line",
        _axes,
        [0, 1],
        [1.0, None],
        ub.DataError,
        "Axes.line(): y[1] is None; values must be numbers.",
        "y",
        "use NaN for missing values, e.g. series.to_numpy(dtype=float, na_value=np.nan).",
    ),
    (
        "scatter",
        _axes,
        [0, -np.inf],
        [1.0, 2.0],
        ub.DataError,
        "Axes.scatter(): x[1] is -inf; values must be finite.",
        "x",
        "use NaN to leave a point out.",
    ),
    (
        "scatter",
        lambda: _axes(ub.Scale.log()),
        [1.0, 0.0],
        [1.0, 2.0],
        ub.DataError,
        "Axes.scatter(): x[1] is 0 on a log axis; values must be positive.",
        "x",
        "use a linear axis, or remove values that are not positive.",
    ),
    (
        "scatter",
        _axes,
        "abc",
        [1.0],
        TypeError,
        "Axes.scatter(): x is str; x must be a sequence or an array.",
        "x",
        'pass a list of names or numbers, e.g. ["A", "B"] or [1.0, 2.0].',
    ),
]


@pytest.mark.parametrize(
    ("name", "make", "x", "y", "cls", "message", "parameter", "hint"), DATA_ERRORS
)
def test_data_errors_leave_the_figure_unchanged(
    name: str,
    make: Callable[[], AnyAxes],
    x: object,
    y: object,
    cls: type[BaseException],
    message: str,
    parameter: object,
    hint: object,
) -> None:
    ax = make().title("Kept").line([], [])
    before = ax.figure.to_svg()
    with pytest.raises(cls) as caught:
        _method(ax, name)(x, y)
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.line(t, y, label='North')", "y"),
        ("ax.line(x=t, y=y)", "y=y"),
        ("ax.scatter(y, t)", "y"),
        ("ax.line(t, y, color='red')", "color='red'"),
        ("ax.scatter(t, t, marker=ub.Dash.SOLID)", "marker=ub.Dash.SOLID"),
        ("ax.title('T').line(t,\n    y)", "y"),
    ],
)
def test_mark_errors_highlight_the_argument(tmp_path: Path, call: str, argument: str) -> None:
    setup = "t = [0.0, 1.0, 2.0]\ny = [1.0, float('inf'), 2.0]\nax = ub.Figure().add_axes()"
    err = _raised(tmp_path, call, setup)
    highlight = _notes(err)[0]
    shown = call.splitlines()[-1].strip() if "\n" in call else call
    assert highlight == _marked(shown, argument)


def test_full_error_output_shows_the_user_line_the_argument_and_the_hint(tmp_path: Path) -> None:
    script = tmp_path / "plot.py"
    script.write_text(
        "import numpy as np\n"
        "import umber as ub\n"
        "\n"
        "t = np.linspace(0.0, 1.0, 20)\n"
        "y = np.sin(t)\n"
        "y[17] = np.inf\n"
        "\n"
        "ax = ub.Figure().add_axes()\n"
        'ax.line(t, y, label="North")\n'
    )
    environment = {
        key: value
        for key, value in os.environ.items()
        if key not in {"FORCE_COLOR", "PYTHON_COLORS"}
    }
    environment["NO_COLOR"] = "1"
    finished = subprocess.run(
        [sys.executable, "plot.py"],
        cwd=tmp_path,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )
    assert finished.returncode == 1
    shown = finished.stderr.replace(str(script.resolve()), "plot.py").replace(
        str(script), "plot.py"
    )
    # From Python 3.13 the interpreter underlines the failing call itself.
    underline = "    ~~~~~~~^^^^^^^^^^^^^^^^^^^^^\n" if sys.version_info >= (3, 13) else ""
    assert shown == (
        "Traceback (most recent call last):\n"
        '  File "plot.py", line 9, in <module>\n'
        '    ax.line(t, y, label="North")\n'
        f"{underline}"
        "umber.DataError: Axes.line(): y[17] is inf; values must be finite.\n"
        '  ax.line(t, y, label="North")\n'
        "             ^\n"
        "hint: use NaN to leave a gap in the line.\n"
    )


FAILING_SETTERS: dict[str, Callable[[AnyAxes], object]] = {
    "title type": lambda ax: ax.title(cast(str, 3)),
    "title length": lambda ax: ax.title("x" * 1001),
    "subtitle type": lambda ax: ax.subtitle(cast(str, None)),
    "xlabel length": lambda ax: ax.xlabel("x" * 1001),
    "ylabel type": lambda ax: ax.ylabel(cast(str, b"y")),
    "xlim order": lambda ax: ax.xlim(1, 0),
    "xlim type": lambda ax: ax.xlim(0, cast(float, "1")),
    "ylim infinite": lambda ax: ax.ylim(0, float("inf")),
    "legend type": lambda ax: ax.legend(cast(ub.LegendStyle, "CHIPS")),
}


@pytest.mark.parametrize("call", FAILING_SETTERS.values(), ids=FAILING_SETTERS.keys())
def test_failed_setters_leave_the_figure_unchanged(call: Callable[[AnyAxes], object]) -> None:
    ax = _axes().line([0, 1, 2], [1, 3, 2], label="North").title("Kept").xlim(0, 2)
    before = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    with pytest.raises((TypeError, ub.StyleError)):
        call(ax)
    assert ax.figure.to_svg(text=ub.SvgText.EDITABLE) == before


@pytest.mark.parametrize("name", ["line", "scatter"])
def test_masked_x_is_a_value_error_after_the_types(name: str) -> None:
    method = _method(_axes(), name)
    masked = np.ma.array([1.0, 2.0])
    with pytest.raises(TypeError) as caught:
        method(masked, None)
    assert _parameter(caught.value) == "y"
    with pytest.raises(ub.DataError) as data:
        method(masked, [1.0, 2.0])
    assert _parameter(data.value) == "x"
