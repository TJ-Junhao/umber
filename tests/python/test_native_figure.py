"""Tests of the native extension's themes, figures, marks and rendering."""

import struct
import threading
from collections.abc import Callable
from typing import Any

import numpy as np
import numpy.typing as npt
import pytest

import umber as ub
from umber import _umber

type Call = Callable[[], object]
# An error case: its id, a figure factory, the failing call, and the class, message, parameter
# and hint of the error it raises.
type ErrorCase = tuple[
    str,
    Callable[[], _umber.Figure],
    Callable[[_umber.Figure], object],
    type[BaseException],
    str,
    object,
    object,
]

AUTO = (0, 0, 1.0)
NO_SIZE = (0, 0.0)
RED = (2, 0xD64933FF, 1.0)
BLUE = (2, 0x3366B8FF, 1.0)
LINEAR, LOG, CATEGORY = 0, 1, 2
OUTLINES, EDITABLE = 0, 1
CONCRETE_HINT = "use Color.hex(), Color.rgb() or a named color."
X_SCALE_HINT = "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."
CATEGORY_X_HINT = "pass x=Scale.category() to Figure.add_axes()."
LINEAR_X_HINT = "pass x=Scale.linear() to Figure.add_axes()."


def floats(*values: float) -> npt.NDArray[np.float64]:
    return np.array(values, dtype=np.float64)


def details(err: BaseException) -> tuple[object, object]:
    """The parameter and hint the native layer stored on an exception it raised."""
    attributes = vars(err)
    return attributes["_umber_parameter"], attributes["_umber_hint"]


def figure(x: int = LINEAR, y: int = LINEAR, *, base: float = 10.0) -> _umber.Figure:
    """A 3 by 2 inch figure with axes of the given scale kinds."""
    fig = _umber.Figure(3.0, 2.0, _umber.Theme.pigment())
    fig.add_axes(x, base, y, base)
    return fig


def line_figure() -> _umber.Figure:
    fig = figure()
    fig.title("Signal")
    fig.line(floats(0, 1, 2), floats(1, 3, 2), "north", AUTO, 0, NO_SIZE)
    return fig


def png_size(png: bytes) -> tuple[int, int]:
    """The pixel width and height in a PNG's IHDR chunk."""
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    assert png[12:16] == b"IHDR"
    width, height = struct.unpack(">II", png[16:24])
    return width, height


def assert_reported(
    call: Call, cls: type[BaseException], message: str, parameter: object, hint: object
) -> None:
    with pytest.raises(cls) as caught:
        call()
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)


def assert_internal(call: Call, cls: type[BaseException], message: str) -> None:
    with pytest.raises(cls) as caught:
        call()
    assert type(caught.value) is cls
    assert str(caught.value) == f"internal error: {message}"
    assert "_umber_parameter" not in vars(caught.value)


# Themes


def test_presets_compare_by_value_and_are_not_hashable() -> None:
    assert _umber.Theme.pigment() == _umber.Theme.pigment()
    assert _umber.Theme.pigment() != _umber.Theme.graphite()
    assert _umber.Theme.graphite() != _umber.Theme.midnight()
    assert _umber.Theme.pigment() != "pigment"
    with pytest.raises(TypeError):
        hash(_umber.Theme.pigment())


def test_themes_have_no_constructor() -> None:
    with pytest.raises(TypeError):
        _umber.Theme()


SETTERS: list[tuple[str, object, object]] = [
    ("with_palette", [RED, BLUE], [BLUE]),
    ("with_background", RED, BLUE),
    ("with_text_color", RED, BLUE),
    ("with_muted_color", RED, BLUE),
    ("with_grid_color", RED, BLUE),
    ("with_baseline_color", RED, BLUE),
    ("with_grid_lines", 0, 3),
    ("with_grid_dash", 0, 2),
    ("with_title_font", 0, 4),
    ("with_body_font", 1, 3),
    ("with_legend_style", 0, 2),
    ("with_fill_style", 0, 1),
    ("with_font_scale", 0.5, 2.0),
    ("with_line_width", 0.5, 3.0),
    ("with_marker_size", 2.0, 8.0),
    ("with_marker_alpha", 0.0, 1.0),
    ("with_baseline_width", 0.0, 2.0),
    ("with_bar_corner_radius", 0.0, 4.0),
    ("with_hist_gap", 0.0, 3.0),
]


def test_every_with_method_is_bound() -> None:
    bound = {name for name in vars(_umber.Theme) if name.startswith("with_")}
    assert bound == {name for name, _, _ in SETTERS}


@pytest.mark.parametrize(("name", "first", "second"), SETTERS)
def test_with_methods_return_a_new_theme_and_keep_the_original(
    name: str, first: object, second: object
) -> None:
    original = _umber.Theme.midnight()
    one = getattr(original, name)(first)
    other = getattr(original, name)(second)
    assert isinstance(one, _umber.Theme)
    assert one == getattr(original, name)(first)
    assert one != other
    assert original == _umber.Theme.midnight()


@pytest.mark.parametrize(
    "name",
    ["with_background", "with_text_color", "with_muted_color", "with_grid_color"]
    + ["with_baseline_color"],
)
@pytest.mark.parametrize(
    ("color", "shown"), [(AUTO, "Color.AUTO"), ((1, 3, 0.5), "Color.palette(3)")]
)
def test_theme_colors_must_be_concrete(name: str, color: object, shown: str) -> None:
    theme = _umber.Theme.pigment()
    assert_reported(
        lambda: getattr(theme, name)(color),
        ub.StyleError,
        f"Theme.{name}(): value is {shown}; theme colors must be concrete colors.",
        "value",
        CONCRETE_HINT,
    )


@pytest.mark.parametrize(
    ("colors", "message"),
    [
        ([RED, BLUE, AUTO], "colors[2] is Color.AUTO"),
        ([(1, 4, 0.5), AUTO], "colors[0] is Color.palette(4)"),
    ],
)
def test_palette_colors_must_be_concrete(
    colors: list[tuple[int, int, float]], message: str
) -> None:
    assert_reported(
        lambda: _umber.Theme.pigment().with_palette(colors),
        ub.StyleError,
        f"Theme.with_palette(): {message}; theme colors must be concrete colors.",
        "colors",
        CONCRETE_HINT,
    )


@pytest.mark.parametrize("count", [0, 17])
def test_palettes_hold_one_to_sixteen_colors(count: int) -> None:
    assert_reported(
        lambda: _umber.Theme.pigment().with_palette([RED] * count),
        ub.StyleError,
        f"Theme.with_palette(): palette has {count} colors; a palette needs 1 to 16 colors.",
        "colors",
        None,
    )
    _umber.Theme.pigment().with_palette([RED] * 16)  # no exception


GREATER_THAN_0 = "a finite number greater than 0"
AT_LEAST_0 = "a finite number of at least 0"


@pytest.mark.parametrize(
    ("name", "value", "shown", "required"),
    [
        ("font_scale", 0.0, "0", f"{GREATER_THAN_0} and at most 10"),
        ("font_scale", 11.0, "11", f"{GREATER_THAN_0} and at most 10"),
        ("line_width", -1.0, "-1", GREATER_THAN_0),
        ("marker_size", float("nan"), "NaN", GREATER_THAN_0),
        ("marker_alpha", 1.5, "1.5", "a finite number from 0.0 to 1.0"),
        ("baseline_width", -0.5, "-0.5", AT_LEAST_0),
        ("bar_corner_radius", float("inf"), "inf", AT_LEAST_0),
        ("hist_gap", -1.0, "-1", AT_LEAST_0),
    ],
)
def test_numeric_theme_values_are_validated(
    name: str, value: float, shown: str, required: str
) -> None:
    theme = _umber.Theme.pigment()
    assert_reported(
        lambda: getattr(theme, f"with_{name}")(value),
        ub.StyleError,
        f"Theme.with_{name}(): {name} is {shown}; {name} must be {required}.",
        "value",
        None,
    )


@pytest.mark.parametrize(
    ("name", "value", "enum"),
    [
        ("with_grid_lines", 4, "GridLines"),
        ("with_grid_dash", 3, "Dash"),
        ("with_title_font", 5, "Font"),
        ("with_body_font", -1, "Font"),
        ("with_legend_style", 4, "LegendStyle"),
    ],
)
def test_unknown_enum_values_are_internal_errors(name: str, value: int, enum: str) -> None:
    theme = _umber.Theme.pigment()
    assert_internal(
        lambda: getattr(theme, name)(value), ValueError, f"{value} is not a value of {enum}"
    )


def test_unknown_color_kinds_are_internal_errors() -> None:
    theme = _umber.Theme.pigment()
    assert_internal(lambda: theme.with_background((3, 0, 1.0)), ValueError, "3 is not a color kind")
    assert_internal(
        lambda: theme.with_palette([RED, (7, 0, 1.0)]), ValueError, "7 is not a color kind"
    )


# Figures


def test_figure_uses_its_theme() -> None:
    pigment = _umber.Figure(3.0, 2.0, _umber.Theme.pigment())
    midnight = _umber.Figure(3.0, 2.0, _umber.Theme.midnight())
    assert pigment.to_svg(OUTLINES) != midnight.to_svg(OUTLINES)
    custom = _umber.Theme.pigment().with_background(BLUE)
    assert 'fill="#3366B8"' in _umber.Figure(3.0, 2.0, custom).to_svg(OUTLINES)


@pytest.mark.parametrize(
    ("width", "height", "parameter", "shown"),
    [
        (0.0, 4.0, "width", "0"),
        (201.0, 4.0, "width", "201"),
        (4.0, float("inf"), "height", "inf"),
        (4.0, -1.0, "height", "-1"),
    ],
)
def test_figure_sizes_are_validated(
    width: float, height: float, parameter: str, shown: str
) -> None:
    assert_reported(
        lambda: _umber.Figure(width, height, _umber.Theme.pigment()),
        ub.StyleError,
        f"Figure(): {parameter} is {shown}; it must be finite, above 0 and at most 200 inches.",
        parameter,
        None,
    )


def test_a_figure_has_one_axes() -> None:
    fig = figure()
    assert_reported(
        lambda: fig.add_axes(LINEAR, 10.0, LINEAR, 10.0),
        ub.UsageError,
        "Figure.add_axes(): the figure already has axes; v0.1 supports one axes per figure.",
        None,
        "create a new Figure for another chart.",
    )


def test_a_category_y_axis_is_refused() -> None:
    fig = _umber.Figure(3.0, 2.0, _umber.Theme.pigment())
    for x in (LINEAR, LOG, CATEGORY):

        def call(x: int = x) -> None:
            fig.add_axes(x, 10.0, CATEGORY, 10.0)

        assert_reported(
            call,
            ub.UsageError,
            "Figure.add_axes(): y is a category scale; the y axis must be linear or log.",
            "y",
            "pass y=Scale.linear() or y=Scale.log().",
        )
    fig.add_axes(CATEGORY, 0.0, LOG, 2.0)  # the failed calls added nothing


def test_unknown_scale_kinds_are_internal_errors() -> None:
    fig = _umber.Figure(3.0, 2.0, _umber.Theme.pigment())
    assert_internal(
        lambda: fig.add_axes(3, 10.0, LINEAR, 10.0), ValueError, "3 is not a scale kind"
    )
    assert_internal(
        lambda: fig.add_axes(LINEAR, 10.0, -1, 10.0), ValueError, "-1 is not a scale kind"
    )
    fig.add_axes(LOG, 2.0, LINEAR, 0.0)  # the failed calls added nothing


AXES_CALLS: dict[str, Callable[[_umber.Figure], object]] = {
    "title": lambda f: f.title("t"),
    "subtitle": lambda f: f.subtitle("t"),
    "xlabel": lambda f: f.xlabel("t"),
    "ylabel": lambda f: f.ylabel("t"),
    "xlim": lambda f: f.xlim(0.0, 1.0),
    "ylim": lambda f: f.ylim(0.0, 1.0),
    "legend": lambda f: f.legend(0),
    "line": lambda f: f.line(floats(1), floats(1), "", AUTO, 0, NO_SIZE),
    "area": lambda f: f.area(floats(1), floats(1), "", AUTO, 0, NO_SIZE),
    "scatter": lambda f: f.scatter(floats(1), floats(1), "", AUTO, 0, NO_SIZE),
    "bar": lambda f: f.bar(["a"], floats(1), "", AUTO),
    "grouped_bar": lambda f: f.grouped_bar(["a"], [("s", floats(1), AUTO)]),
    "stacked_bar": lambda f: f.stacked_bar(["a"], [("s", floats(1), AUTO)]),
    "hist": lambda f: f.hist(floats(1), 3, "", AUTO),
}


@pytest.mark.parametrize("name", AXES_CALLS)
def test_axes_methods_without_axes_are_internal_errors(name: str) -> None:
    fig = _umber.Figure(3.0, 2.0, _umber.Theme.pigment())
    assert_internal(lambda: AXES_CALLS[name](fig), RuntimeError, "the figure has no axes")


def test_texts_appear_in_editable_svg() -> None:
    fig = figure()
    fig.title("Growth")
    fig.subtitle("Per year")
    fig.xlabel("Year")
    fig.ylabel("Revenue")
    svg = fig.to_svg(EDITABLE)
    for text in ("Growth", "Per year", "Year", "Revenue"):
        assert f">{text}</text>" in svg
    assert ">Growth</text>" not in fig.to_svg(OUTLINES)


def test_limits_and_legend_change_the_output() -> None:
    fig = line_figure()
    before = fig.to_svg(OUTLINES)
    fig.xlim(-1.0, 5.0)
    fig.ylim(0.0, 10.0)
    limited = fig.to_svg(OUTLINES)
    assert limited != before
    fig.legend(3)
    assert fig.to_svg(OUTLINES) != limited
    assert_internal(lambda: fig.legend(4), ValueError, "4 is not a value of LegendStyle")


def range_message(context: str, limits: str, rule: str) -> str:
    return f"{context}: the limits are {limits}; {rule}."


ORDER_RULE = "both must be finite and the lower limit must be below the upper limit"


LIMIT_ERRORS: list[ErrorCase] = [
    (
        "equal limits",
        figure,
        lambda f: f.xlim(1.0, 1.0),
        ub.StyleError,
        range_message("Axes.xlim()", "1 and 1", ORDER_RULE),
        None,
        None,
    ),
    (
        "NaN limit",
        figure,
        lambda f: f.ylim(float("nan"), 1.0),
        ub.StyleError,
        range_message("Axes.ylim()", "NaN and 1", ORDER_RULE),
        "low",
        None,
    ),
    (
        "limit magnitude",
        figure,
        lambda f: f.xlim(0.0, 1e301),
        ub.StyleError,
        range_message("Axes.xlim()", "0 and 1e301", "their magnitude must not exceed 1e300"),
        "high",
        None,
    ),
    (
        "limits too close",
        figure,
        lambda f: f.xlim(1.0, 1.0 + 1e-12),
        ub.StyleError,
        "Axes.xlim(): the limits 1 and 1.000000000001 are too close; the range must be at "
        "least 1e-10 of their magnitude (and at least 1e-290).",
        None,
        None,
    ),
    (
        "log lower limit",
        lambda: figure(y=LOG),
        lambda f: f.ylim(0.0, 1.0),
        ub.StyleError,
        "Axes.ylim(): the lower limit is 0; a log scale needs a positive lower limit.",
        "low",
        "use a linear axis if either limit is not positive.",
    ),
    (
        "log limits out of range",
        lambda: figure(x=LOG),
        lambda f: f.xlim(1e-301, 1.0),
        ub.StyleError,
        range_message(
            "Axes.xlim()",
            "1e-301 and 1",
            "a log scale supports values from 1e-300 to 1e300",
        ),
        "low",
        None,
    ),
    (
        "log limits too close",
        lambda: figure(y=LOG),
        lambda f: f.ylim(1.0, 1.0 + 1e-12),
        ub.StyleError,
        "Axes.ylim(): the limits are 1 and 1.000000000001; the upper limit must be at least "
        "a factor of 1.000000001 above the lower limit.",
        None,
        None,
    ),
    (
        "category x limits",
        lambda: figure(x=CATEGORY),
        lambda f: f.xlim(0.0, 1.0),
        ub.UsageError,
        "Axes.xlim(): this is a category axis; limits apply to linear and log axes only.",
        None,
        X_SCALE_HINT,
    ),
]


@pytest.mark.parametrize(
    ("make", "call", "cls", "message", "parameter", "hint"),
    [case[1:] for case in LIMIT_ERRORS],
    ids=[case[0] for case in LIMIT_ERRORS],
)
def test_limit_errors(
    make: Callable[[], _umber.Figure],
    call: Callable[[_umber.Figure], object],
    cls: type[BaseException],
    message: str,
    parameter: object,
    hint: object,
) -> None:
    fig = make()
    before = fig.to_svg(OUTLINES)
    assert_reported(lambda: call(fig), cls, message, parameter, hint)
    assert fig.to_svg(OUTLINES) == before


# Marks


def test_marks_draw() -> None:
    empty = figure().to_svg(OUTLINES)
    fig = figure()
    fig.line(floats(0, 1, 2), floats(1, np.nan, 2), "a", AUTO, 1, NO_SIZE)
    with_line = fig.to_svg(OUTLINES)
    assert with_line != empty
    fig.scatter(floats(0, 1, 2), floats(2, 1, 0), "b", (1, 3, 0.5), 4, NO_SIZE)
    assert fig.to_svg(OUTLINES) != with_line

    bars = figure(x=CATEGORY)
    bars.bar(["a", "b"], floats(1, 2), "c", RED)
    bars.grouped_bar(["c"], [("s", floats(1), AUTO), ("t", floats(2), BLUE)])
    bars.stacked_bar(["d"], [("s", floats(1), AUTO), ("t", floats(-2), BLUE)])
    assert bars.to_svg(OUTLINES) != figure(x=CATEGORY).to_svg(OUTLINES)

    categories = figure(x=CATEGORY)
    categories.line(["a", "b"], floats(1, 2), "", AUTO, 0, NO_SIZE)
    categories.scatter(["b", "c"], floats(1, 2), "", AUTO, 0, NO_SIZE)

    hist = figure()
    hist.hist(floats(1.2, 3.4, 2.2, np.nan, 2.9), 3, "h", AUTO)
    assert hist.to_svg(OUTLINES) != empty


def test_mark_labels_and_colors_reach_the_legend() -> None:
    fig = figure()
    fig.legend(2)
    fig.line(floats(0, 1), floats(1, 2), "North", RED, 0, NO_SIZE)
    fig.scatter(floats(0, 1), floats(2, 1), "South", BLUE, 0, NO_SIZE)
    svg = fig.to_svg(EDITABLE)
    assert ">North</text>" in svg
    assert ">South</text>" in svg
    assert 'fill="#D64933"' in svg or 'stroke="#D64933"' in svg


def series(*values: float) -> tuple[str, npt.NDArray[np.float64], tuple[int, int, float]]:
    return ("s", floats(*values), AUTO)


LINE_GAP = "use NaN to leave a gap in the line."
BAR_GAP = "use NaN to leave a category without a bar."
SERIES_GAP = "use NaN to omit that height."
HIST_NAN = "a histogram ignores NaN, so use it to leave a value out."
REPEAT = "combine repeated categories and sum their heights."
REPEAT_IN_SERIES = "combine repeated categories and sum their heights within each series."
FINITE = "values must be finite."

MARK_ERRORS: list[ErrorCase] = [
    # line and scatter
    (
        "line length",
        figure,
        lambda f: f.line(floats(1, 2), floats(1), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        "Axes.line(): x has 2 values but y has 1; x and y must have the same length.",
        None,
        None,
    ),
    (
        "line x infinite",
        figure,
        lambda f: f.line(floats(1, np.inf), floats(1, 2), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        f"Axes.line(): x[1] is inf; {FINITE}",
        "x",
        LINE_GAP,
    ),
    (
        "line y infinite",
        figure,
        lambda f: f.line(floats(1, 2), floats(1, -np.inf), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        f"Axes.line(): y[1] is -inf; {FINITE}",
        "y",
        LINE_GAP,
    ),
    (
        "line magnitude",
        figure,
        lambda f: f.line(floats(1, 2), floats(1, -1e301), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        "Axes.line(): y[1] is -1e301; magnitudes must not exceed 1e300.",
        "y",
        None,
    ),
    (
        "line log not positive",
        lambda: figure(y=LOG),
        lambda f: f.line(floats(1, 2), floats(1, 0), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        "Axes.line(): y[1] is 0 on a log axis; values must be positive.",
        "y",
        "use a linear axis, or remove values that are not positive.",
    ),
    (
        "line log too small",
        lambda: figure(x=LOG),
        lambda f: f.line(floats(1, 1e-310), floats(1, 2), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        "Axes.line(): x[1] is 1e-310 on a log axis; values must be at least 1e-300.",
        "x",
        "use a linear axis, or rescale the values.",
    ),
    (
        "line numbers on a category axis",
        lambda: figure(x=CATEGORY),
        lambda f: f.line(floats(1), floats(1), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        "Axes.line(): x holds numbers but the x axis is a category axis; pass category names.",
        None,
        X_SCALE_HINT,
    ),
    (
        "line names on a linear axis",
        figure,
        lambda f: f.line(["a"], floats(1), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        "Axes.line(): x holds category names but the x axis is not a category axis; pass numbers.",
        None,
        CATEGORY_X_HINT,
    ),
    (
        "area length",
        figure,
        lambda f: f.area(floats(1, 2), floats(1), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        "Axes.area(): x has 2 values but y has 1; x and y must have the same length.",
        None,
        None,
    ),
    (
        "area y infinite",
        figure,
        lambda f: f.area(floats(1, 2), floats(1, np.inf), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        f"Axes.area(): y[1] is inf; {FINITE}",
        "y",
        "use NaN to leave a gap in the area.",
    ),
    (
        "scatter length",
        figure,
        lambda f: f.scatter(floats(1, 2), floats(1), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        "Axes.scatter(): x has 2 values but y has 1; x and y must have the same length.",
        None,
        None,
    ),
    (
        "scatter y infinite",
        figure,
        lambda f: f.scatter(floats(1, 2), floats(1, np.inf), "", AUTO, 0, NO_SIZE),
        ub.DataError,
        f"Axes.scatter(): y[1] is inf; {FINITE}",
        "y",
        "use NaN to leave a point out.",
    ),
    # bar
    (
        "bar length",
        lambda: figure(x=CATEGORY),
        lambda f: f.bar(["a", "b"], floats(1), "", AUTO),
        ub.DataError,
        "Axes.bar(): categories has 2 values but heights has 1; categories and heights must "
        "have the same length.",
        None,
        None,
    ),
    (
        "bar linear x",
        figure,
        lambda f: f.bar(["a"], floats(1), "", AUTO),
        ub.DataError,
        "Axes.bar(): categories holds names but the x axis is not a category axis; "
        "use a category x axis.",
        None,
        CATEGORY_X_HINT,
    ),
    (
        "bar repeated category",
        lambda: figure(x=CATEGORY),
        lambda f: f.bar(["a", "a"], floats(1, 2), "", AUTO),
        ub.DataError,
        'Axes.bar(): categories[1] repeats "a" from categories[0]; each category must appear once.',
        "categories",
        REPEAT,
    ),
    (
        "bar infinite",
        lambda: figure(x=CATEGORY),
        lambda f: f.bar(["a", "b"], floats(1, np.inf), "", AUTO),
        ub.DataError,
        f"Axes.bar(): heights[1] is inf; {FINITE}",
        "heights",
        BAR_GAP,
    ),
    # grouped and stacked bars
    (
        "grouped no series",
        lambda: figure(x=CATEGORY),
        lambda f: f.grouped_bar(["a"], []),
        ub.DataError,
        "Axes.grouped_bar(): series is empty; pass at least one series.",
        "series",
        None,
    ),
    (
        "grouped series length",
        lambda: figure(x=CATEGORY),
        lambda f: f.grouped_bar(["a"], [series(1), series()]),
        ub.DataError,
        "Axes.grouped_bar(): categories has 1 values but series[1].values has 0; each series "
        "must have one value per category.",
        "series",
        None,
    ),
    (
        "grouped repeated category",
        lambda: figure(x=CATEGORY),
        lambda f: f.grouped_bar(["a", "a"], [series(1, 2)]),
        ub.DataError,
        'Axes.grouped_bar(): categories[1] repeats "a" from categories[0]; each category must '
        "appear once.",
        "categories",
        REPEAT_IN_SERIES,
    ),
    (
        "grouped infinite",
        lambda: figure(x=CATEGORY),
        lambda f: f.grouped_bar(["a", "b"], [series(1, 2), series(1, np.inf)]),
        ub.DataError,
        f"Axes.grouped_bar(): series[1].values[1] is inf; {FINITE}",
        "series",
        SERIES_GAP,
    ),
    (
        "grouped linear x",
        figure,
        lambda f: f.grouped_bar(["a"], [series(1)]),
        ub.DataError,
        "Axes.grouped_bar(): categories holds names but the x axis is not a category axis; "
        "use a category x axis.",
        None,
        CATEGORY_X_HINT,
    ),
    (
        "stacked no series",
        lambda: figure(x=CATEGORY),
        lambda f: f.stacked_bar(["a"], []),
        ub.DataError,
        "Axes.stacked_bar(): series is empty; pass at least one series.",
        "series",
        None,
    ),
    (
        "stacked log not positive",
        lambda: figure(x=CATEGORY, y=LOG),
        lambda f: f.stacked_bar(["a"], [series(-1)]),
        ub.DataError,
        "Axes.stacked_bar(): series[0].values[0] is -1 on a log axis; values must be positive.",
        "series",
        "use a linear axis, or remove values that are not positive.",
    ),
    (
        "stacked total",
        lambda: figure(x=CATEGORY),
        lambda f: f.stacked_bar(["a"], [series(1e300), series(1e300)]),
        ub.DataError,
        'Axes.stacked_bar(): the values of categories[0] ("a") stack to 2e300; each stack '
        "must stay within 1e300 on both sides of 0.",
        "series",
        None,
    ),
    # hist
    (
        "hist no bins",
        figure,
        lambda f: f.hist(floats(1), 0, "", AUTO),
        ub.StyleError,
        "Axes.hist(): bins is 0; bins must be between 1 and 10000.",
        "bins",
        "use at least one bin.",
    ),
    (
        "hist too many bins",
        figure,
        lambda f: f.hist(floats(1), _umber.MAX_BINS + 1, "", AUTO),
        ub.StyleError,
        "Axes.hist(): bins is 10001; bins must be between 1 and 10000.",
        "bins",
        None,
    ),
    (
        "hist category x",
        lambda: figure(x=CATEGORY),
        lambda f: f.hist(floats(1), 3, "", AUTO),
        ub.DataError,
        "Axes.hist(): values holds numbers but the x axis is a category axis; use a linear x axis.",
        None,
        LINEAR_X_HINT,
    ),
    (
        "hist log x",
        lambda: figure(x=LOG),
        lambda f: f.hist(floats(1), 3, "", AUTO),
        ub.UsageError,
        "Axes.hist(): the x axis is a log axis, which histograms do not support; "
        "use a linear x axis.",
        None,
        LINEAR_X_HINT,
    ),
    (
        "hist infinite",
        figure,
        lambda f: f.hist(floats(1, np.inf), 3, "", AUTO),
        ub.DataError,
        f"Axes.hist(): values[1] is inf; {FINITE}",
        "values",
        HIST_NAN,
    ),
]


@pytest.mark.parametrize(
    ("make", "call", "cls", "message", "parameter", "hint"),
    [case[1:] for case in MARK_ERRORS],
    ids=[case[0] for case in MARK_ERRORS],
)
def test_mark_errors_change_nothing(
    make: Callable[[], _umber.Figure],
    call: Callable[[_umber.Figure], object],
    cls: type[BaseException],
    message: str,
    parameter: object,
    hint: object,
) -> None:
    fig = make()
    before = fig.to_svg(OUTLINES)
    assert_reported(lambda: call(fig), cls, message, parameter, hint)
    assert fig.to_svg(OUTLINES) == before


def untyped(value: object) -> Any:
    """`value`, typed so that it can be passed where the stub expects another type."""
    return value


INTERNAL_MARK_ERRORS: list[
    tuple[str, Callable[[_umber.Figure], object], type[BaseException], str]
] = [
    (
        "dash",
        lambda f: f.line(floats(1), floats(1), "", AUTO, 3, NO_SIZE),
        ValueError,
        "3 is not a value of Dash",
    ),
    (
        "marker",
        lambda f: f.scatter(floats(1), floats(1), "", AUTO, 5, NO_SIZE),
        ValueError,
        "5 is not a value of Marker",
    ),
    (
        "color kind",
        lambda f: f.hist(floats(1), 3, "", (4, 0, 1.0)),
        ValueError,
        "4 is not a color kind",
    ),
    (
        "list of floats",
        lambda f: f.line(floats(1), untyped([1.0]), "", AUTO, 0, NO_SIZE),
        TypeError,
        "expected a one-dimensional float64 buffer, got list",
    ),
    (
        "series values",
        lambda f: f.grouped_bar(["a"], [("s", untyped((1.0,)), AUTO)]),
        TypeError,
        "expected a one-dimensional float64 buffer, got tuple",
    ),
    (
        "tuple of names",
        lambda f: f.bar(untyped(("a",)), floats(1), "", AUTO),
        TypeError,
        "expected a list of str, got tuple",
    ),
]


@pytest.mark.parametrize(
    ("call", "cls", "message"),
    [case[1:] for case in INTERNAL_MARK_ERRORS],
    ids=[case[0] for case in INTERNAL_MARK_ERRORS],
)
def test_unconvertible_mark_arguments_are_internal_errors_that_change_nothing(
    call: Callable[[_umber.Figure], object], cls: type[BaseException], message: str
) -> None:
    fig = figure()
    before = fig.to_svg(OUTLINES)
    assert_internal(lambda: call(fig), cls, message)
    assert fig.to_svg(OUTLINES) == before


# Rendering


@pytest.mark.parametrize(
    ("width", "height", "dpi"), [(3.0, 2.0, 50), (6.0, 4.0, 100), (2.5, 1.5, 72)]
)
def test_png_pixel_size_is_inches_times_dpi(width: float, height: float, dpi: int) -> None:
    fig = _umber.Figure(width, height, _umber.Theme.pigment())
    expected = (round(width * dpi), round(height * dpi))
    assert png_size(fig.to_png(dpi)) == expected  # a figure without axes renders
    fig.add_axes(LINEAR, 10.0, LINEAR, 10.0)
    fig.line(floats(0, 1), floats(1, 2), "", AUTO, 0, NO_SIZE)
    png = fig.to_png(dpi)
    assert isinstance(png, bytes)
    assert png_size(png) == expected
    assert fig.png_for_file(dpi) == png


def test_svg_text_modes() -> None:
    fig = line_figure()
    editable = fig.to_svg(EDITABLE)
    outlines = fig.to_svg(OUTLINES)
    assert editable.startswith("<svg") or editable.startswith("<?xml")
    assert ">Signal</text>" in editable
    assert ">Signal</text>" not in outlines
    assert fig.svg_for_file(EDITABLE) == editable
    assert fig.svg_for_file(OUTLINES) == outlines
    assert_internal(lambda: fig.to_svg(2), ValueError, "2 is not a value of SvgText")
    assert_internal(lambda: fig.svg_for_file(-1), ValueError, "-1 is not a value of SvgText")


DPI_RULE = "is out of range; use a value from 10 to 2400."
SMALLER = "use a smaller figure size or a lower dpi."
TOO_WIDE = f"the image would be 480000 by 480000 pixels, over 32767 on one side; {SMALLER}"
TOO_MANY = f"the image would have 100000000 pixels, over 67108864; {SMALLER}"


@pytest.mark.parametrize(
    ("method", "context"), [("to_png", "to_png"), ("png_for_file", "save_png")]
)
@pytest.mark.parametrize("dpi", [9, 2401, -(2**63)])
def test_png_errors_name_the_calling_method(method: str, context: str, dpi: int) -> None:
    fig = line_figure()
    assert_reported(
        lambda: getattr(fig, method)(dpi),
        ub.StyleError,
        f"Figure.{context}(): dpi {dpi} {DPI_RULE}",
        "dpi",
        None,
    )


@pytest.mark.parametrize(
    ("method", "context"), [("to_png", "to_png"), ("png_for_file", "save_png")]
)
@pytest.mark.parametrize(
    ("side", "dpi", "message"), [(200.0, 2400, TOO_WIDE), (100.0, 100, TOO_MANY)]
)
def test_png_size_limits(method: str, context: str, side: float, dpi: int, message: str) -> None:
    huge = _umber.Figure(side, side, _umber.Theme.pigment())
    assert_reported(
        lambda: getattr(huge, method)(dpi),
        ub.StyleError,
        f"Figure.{context}(): {message}",
        None,
        None,
    )


def test_rendering_is_deterministic() -> None:
    assert line_figure().to_png(50) == line_figure().to_png(50)
    assert line_figure().to_svg(OUTLINES) == line_figure().to_svg(OUTLINES)


# Copies


def test_a_clone_is_independent_of_the_original() -> None:
    original = line_figure()
    svg = original.to_svg(EDITABLE)
    copy = original.clone()
    assert isinstance(copy, _umber.Figure)
    assert copy is not original
    assert copy.to_svg(EDITABLE) == svg

    copy.title("Changed")
    copy.scatter(floats(0, 1), floats(2, 2), "south", RED, 0, NO_SIZE)
    copy.xlim(-1.0, 3.0)
    assert ">Changed</text>" in copy.to_svg(EDITABLE)
    assert original.to_svg(EDITABLE) == svg

    original.subtitle("Only here")
    assert ">Only here</text>" not in copy.to_svg(EDITABLE)


def test_a_clone_of_a_figure_without_axes_gets_its_own_axes() -> None:
    original = _umber.Figure(3.0, 2.0, _umber.Theme.pigment())
    copy = original.clone()
    copy.add_axes(LINEAR, 10.0, LINEAR, 10.0)
    copy.title("Copy")
    assert_internal(lambda: original.title("Original"), RuntimeError, "the figure has no axes")
    original.add_axes(CATEGORY, 0.0, LINEAR, 10.0)  # the original can still add its own


# Threads


def test_renders_in_threads_while_another_thread_sets_titles() -> None:
    titles = ("North", "South")
    changing = line_figure()
    expected_svgs: set[str] = set()
    expected_pngs: set[bytes] = set()
    for title in titles:
        reference = changing.clone()
        reference.title(title)
        expected_svgs.add(reference.to_svg(OUTLINES))
        expected_pngs.add(reference.to_png(40))
    unchanged = line_figure()
    unchanged_svg = unchanged.to_svg(OUTLINES)
    unchanged_png = unchanged.to_png(40)

    errors: list[BaseException] = []
    svgs: list[str] = []
    pngs: list[bytes] = []
    still_svgs: list[str] = []
    still_pngs: list[bytes] = []
    retitled = 0
    stop = threading.Event()

    def render() -> None:
        try:
            for _ in range(15):
                svgs.append(changing.to_svg(OUTLINES))
                pngs.append(changing.to_png(40))
                still_svgs.append(unchanged.to_svg(OUTLINES))
                still_pngs.append(unchanged.to_png(40))
        except BaseException as err:  # reported below
            errors.append(err)

    def retitle() -> None:
        nonlocal retitled
        try:
            while not stop.is_set():
                changing.title(titles[retitled % 2])
                retitled += 1
        except BaseException as err:  # reported below
            errors.append(err)

    changing.title(titles[0])
    renderers = [threading.Thread(target=render) for _ in range(2)]
    writer = threading.Thread(target=retitle)
    writer.start()
    for thread in renderers:
        thread.start()
    for thread in renderers:
        thread.join()
    stop.set()
    writer.join()

    assert errors == []
    assert retitled > 0
    assert len(svgs) == len(pngs) == 30
    assert set(svgs) <= expected_svgs
    assert set(pngs) <= expected_pngs
    assert set(still_svgs) == {unchanged_svg}
    assert set(still_pngs) == {unchanged_png}
