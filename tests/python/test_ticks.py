# pyright: reportPrivateUsage=false
import copy
import math
import pickle
import traceback
from collections.abc import Callable
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pytest

import umber as ub
from umber import _umber
from umber._args import type_name
from umber._errors import details
from umber._ticks import native

type AnyAxes = ub.Axes[Any, Any]

LINEAR, LOG, CATEGORY = 0, 1, 2
OUTLINES = 0
AUTO = (0, 0, 1.0)
# A native figure of one cell (rows, cols, share_x, share_y, legend), and the position of its
# axes: row and column AUTO (grid index kind 0) spanning one cell.
ONE_CELL = (1, 1, 0, 0, 0)
ONLY_CELL = (0, 0, 0, 0, 1, 1)
NO_SIZE = (0, 0.0)
NATIVE_AUTO = (0, 0, "", "")
TEXT_CAP = "text must be at most 1,000 characters."
SHORTEN = "shorten the text; it would not fit the figure."
DECIMALS_RANGE = "decimals must be between 0 and 12."
SORT_HINT = "sort the positions and remove repeats; they are used in the order given."
CATEGORY_HINT = "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes()."
LABELS_HINT = "pass only the positions to have them formatted, or write the format into the labels."
FORMAT_HINT = "set the tick format back to TickFormat.AUTO first, or pass only the positions."
TYPE_HINT = (
    "use TickFormat.fixed(2), TickFormat.percent(), TickFormat.scientific() or TickFormat.AUTO."
)
CONFLICT_WITH_LABELS = (
    "the axis has tick labels given with their positions; they are drawn as given and take no "
    "format other than TickFormat.AUTO."
)
CONFLICT_WITH_FORMAT = (
    "the axis has a tick format other than TickFormat.AUTO; labels given with their positions "
    "are drawn as given and take no format."
)


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def _parameter(err: BaseException) -> object:
    """The parameter an error names."""
    return vars(err)["_umber_parameter"]


def _floats(*values: float) -> npt.NDArray[np.float64]:
    return np.array(values, dtype=np.float64)


def _axes(*, log: bool = False) -> AnyAxes:
    scale = ub.Scale.log() if log else ub.Scale.linear()
    return ub.Figure(4, 3).add_axes(x=scale, y=scale).line([1, 2, 3], [1, 10, 100])


def _native(x: int = LINEAR, y: int = LINEAR) -> _umber.Figure:
    fig = _umber.Figure(4.0, 3.0, _umber.Theme.pigment(), *ONE_CELL)
    fig.add_axes(*ONLY_CELL, x, 10.0, y, 10.0)
    fig.line(0, 0, _floats(1, 2, 3), _floats(1, 10, 100), "", AUTO, 0, NO_SIZE)
    return fig


# The TickFormat value class.


def test_constructors_and_native_form() -> None:
    assert native(ub.TickFormat.AUTO) == NATIVE_AUTO
    assert native(ub.TickFormat.fixed(2)) == (1, 2, "", "")
    assert native(ub.TickFormat.percent()) == (2, 0, "", "")
    assert native(ub.TickFormat.percent(3)) == (2, 3, "", "")
    assert native(ub.TickFormat.scientific()) == (3, 0, "", "")
    assert native(ub.TickFormat.fixed(1).affix("$", " k")) == (1, 1, "$", " k")


@pytest.mark.parametrize("decimals", [0, 1, 12])
def test_decimals_accept_the_range(decimals: int) -> None:
    assert native(ub.TickFormat.fixed(decimals)) == (1, decimals, "", "")
    assert native(ub.TickFormat.percent(decimals)) == (2, decimals, "", "")


@pytest.mark.parametrize("decimals", [-1, 13, 255, 256, 2**70, -(2**70)])
def test_decimals_outside_the_range_are_refused_natively(decimals: int) -> None:
    shown = min(max(decimals, -(2**63)), 2**63 - 1)
    for name, build in (("fixed", ub.TickFormat.fixed), ("percent", ub.TickFormat.percent)):
        with pytest.raises(ub.StyleError) as caught:
            build(decimals)
        assert str(caught.value) == f"TickFormat.{name}(): decimals is {shown}; {DECIMALS_RANGE}"
        assert details(caught.value) == ("decimals", None)


@pytest.mark.parametrize("value", [True, False, 1.0, 2.5, "2", None, [2], b"2"])
def test_decimals_must_be_an_int(value: object) -> None:
    name = type_name(value)
    for method, build in (("fixed", ub.TickFormat.fixed), ("percent", ub.TickFormat.percent)):
        with pytest.raises(TypeError) as caught:
            build(cast(int, value))
        assert (
            str(caught.value)
            == f"TickFormat.{method}(): decimals is {name}; decimals must be an int."
        )
        assert details(caught.value) == ("decimals", "pass a whole number such as 3.")


def test_decimals_accept_numpy_integers() -> None:
    assert ub.TickFormat.fixed(cast(int, np.int64(2))) == ub.TickFormat.fixed(2)
    assert ub.TickFormat.percent(cast(int, np.uint8(1))) == ub.TickFormat.percent(1)


def test_affix_text_is_capped_in_characters() -> None:
    assert native(ub.TickFormat.AUTO.affix("x" * 1000, "y" * 1000)) == (
        0,
        0,
        "x" * 1000,
        "y" * 1000,
    )
    with pytest.raises(ub.StyleError) as caught:
        ub.TickFormat.AUTO.affix("x" * 1001)
    assert str(caught.value) == f"TickFormat.affix(): prefix has 1,001 characters; {TEXT_CAP}"
    assert details(caught.value) == ("prefix", SHORTEN)
    with pytest.raises(ub.StyleError) as caught:
        ub.TickFormat.fixed(2).affix(suffix="y" * 1001)
    assert str(caught.value) == f"TickFormat.affix(): suffix has 1,001 characters; {TEXT_CAP}"
    assert details(caught.value) == ("suffix", SHORTEN)


@pytest.mark.parametrize("value", [1, None, b"x", ["x"]])
def test_affix_text_must_be_a_str(value: object) -> None:
    name = type_name(value)
    with pytest.raises(TypeError) as caught:
        ub.TickFormat.AUTO.affix(cast(str, value))
    assert str(caught.value) == f"TickFormat.affix(): prefix is {name}; prefix must be a str."
    with pytest.raises(TypeError) as caught:
        ub.TickFormat.AUTO.affix("", cast(str, value))
    assert str(caught.value) == f"TickFormat.affix(): suffix is {name}; suffix must be a str."


def test_affix_replaces_the_text_and_keeps_the_style() -> None:
    base = ub.TickFormat.percent(1)
    assert base.affix("~", "!").affix(suffix=" pts") == base.affix(suffix=" pts")
    assert base.affix("~", "!").affix() == base
    assert ub.TickFormat.AUTO.affix() == ub.TickFormat.AUTO
    assert base == ub.TickFormat.percent(1)  # the original is unchanged
    assert ub.TickFormat.AUTO.affix("$") != ub.TickFormat.AUTO


def test_value_errors_highlight_the_argument() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.TickFormat.fixed(13)
    assert _notes(caught.value) == [_marked("ub.TickFormat.fixed(13)", "13")]
    with pytest.raises(ub.StyleError) as caught:
        ub.TickFormat.percent(decimals=-1)
    assert _notes(caught.value) == [_marked("ub.TickFormat.percent(decimals=-1)", "decimals=-1")]
    with pytest.raises(TypeError) as other:
        ub.TickFormat.fixed(cast(int, 1.5))
    assert _notes(other.value) == [
        _marked("ub.TickFormat.fixed(cast(int, 1.5))", "cast(int, 1.5)"),
        "hint: pass a whole number such as 3.",
    ]
    with pytest.raises(ub.StyleError) as caught:
        ub.TickFormat.AUTO.affix(suffix="y" * 1001)
    assert _notes(caught.value)[0].splitlines()[1].strip() == "^" * len('suffix="y" * 1001')


def test_equality_and_hash() -> None:
    assert ub.TickFormat.AUTO == ub.TickFormat.AUTO
    assert ub.TickFormat.fixed(2) == ub.TickFormat.fixed(2)
    assert hash(ub.TickFormat.fixed(2)) == hash(ub.TickFormat.fixed(2))
    assert ub.TickFormat.fixed(2) != ub.TickFormat.fixed(3)
    assert ub.TickFormat.fixed(2) != ub.TickFormat.percent(2)
    assert ub.TickFormat.percent() == ub.TickFormat.percent(0)
    assert ub.TickFormat.scientific() == ub.TickFormat.scientific()
    assert ub.TickFormat.AUTO != ub.TickFormat.scientific()
    assert ub.TickFormat.AUTO != "auto"
    assert ub.TickFormat.fixed(1).affix("$") == ub.TickFormat.fixed(1).affix(prefix="$")
    assert ub.TickFormat.fixed(1).affix("$") != ub.TickFormat.fixed(1).affix(suffix="$")
    formats = {
        ub.TickFormat.AUTO,
        ub.TickFormat.AUTO,
        ub.TickFormat.fixed(0),
        ub.TickFormat.percent(0),
        ub.TickFormat.scientific(),
        ub.TickFormat.AUTO.affix("%"),
    }
    assert len(formats) == 5


def test_repr() -> None:
    assert repr(ub.TickFormat.AUTO) == "TickFormat.AUTO"
    assert repr(ub.TickFormat.fixed(2)) == "TickFormat.fixed(2)"
    assert repr(ub.TickFormat.percent()) == "TickFormat.percent(0)"
    assert repr(ub.TickFormat.scientific()) == "TickFormat.scientific()"
    assert repr(ub.TickFormat.percent(1).affix(suffix=" pts")) == (
        "TickFormat.percent(1).affix(suffix=' pts')"
    )
    assert repr(ub.TickFormat.fixed(2).affix("$")) == "TickFormat.fixed(2).affix(prefix='$')"
    assert repr(ub.TickFormat.AUTO.affix("<", ">")) == (
        "TickFormat.AUTO.affix(prefix='<', suffix='>')"
    )
    assert repr(ub.TickFormat.scientific().affix(suffix="x")) == (
        "TickFormat.scientific().affix(suffix='x')"
    )


def test_tick_format_is_immutable_and_has_no_public_constructor() -> None:
    fmt = ub.TickFormat.fixed(2)
    with pytest.raises(AttributeError):
        fmt._kind = 2
    with pytest.raises(AttributeError):
        del fmt._decimals
    with pytest.raises(AttributeError):
        fmt.extra = 1
    assert not hasattr(fmt, "__dict__")
    with pytest.raises(TypeError) as caught:
        ub.TickFormat()
    assert str(caught.value) == (
        "TickFormat(): TickFormat has no public constructor; use TickFormat.AUTO or one of its "
        "constructors."
    )
    assert details(caught.value) == (
        None,
        "use TickFormat.AUTO, TickFormat.fixed(), TickFormat.percent() or TickFormat.scientific().",
    )
    frames = [entry.name for entry in traceback.extract_tb(caught.value.__traceback__)]
    assert frames == ["test_tick_format_is_immutable_and_has_no_public_constructor"]


FORMATS = [
    ub.TickFormat.AUTO,
    ub.TickFormat.fixed(2),
    ub.TickFormat.percent(),
    ub.TickFormat.scientific(),
    ub.TickFormat.percent(1).affix(suffix=" pts"),
    ub.TickFormat.AUTO.affix("<", ">"),
]


@pytest.mark.parametrize("fmt", FORMATS, ids=repr)
def test_copy_and_deepcopy_return_the_same_value(fmt: ub.TickFormat) -> None:
    assert copy.copy(fmt) is fmt
    assert copy.deepcopy(fmt) is fmt


@pytest.mark.parametrize("fmt", FORMATS, ids=repr)
def test_pickle_round_trip_gives_an_equal_format(fmt: ub.TickFormat) -> None:
    for protocol in range(pickle.HIGHEST_PROTOCOL + 1):
        restored: object = pickle.loads(pickle.dumps(fmt, protocol))
        assert restored == fmt
        assert repr(restored) == repr(fmt)


def test_the_module_is_the_package() -> None:
    assert ub.TickFormat.__module__ == "umber"
    for name in ("fixed", "percent", "scientific", "affix"):
        assert getattr(ub.TickFormat, name).__module__ == "umber"
    assert "TickFormat" in ub.__all__


# Positions.


@pytest.mark.parametrize("axis", ["x", "y"])
def test_xticks_and_yticks_reject_bad_positions(axis: str) -> None:
    ax = _axes()
    method = getattr(ax, f"{axis}ticks")
    context = f"Axes.{axis}ticks()"
    cases: list[tuple[list[float], str, str | None]] = [
        ([0, math.nan], "positions[1] is NaN; tick positions must be finite.", None),
        ([math.inf], "positions[0] is inf; tick positions must be finite.", None),
        ([-math.inf, 1], "positions[0] is -inf; tick positions must be finite.", None),
        (
            [0, 2, 1],
            "positions[2] is 1 but positions[1] is 2; tick positions must be strictly increasing.",
            SORT_HINT,
        ),
        (
            [1, 1],
            "positions[1] is 1 but positions[0] is 1; tick positions must be strictly increasing.",
            SORT_HINT,
        ),
        (
            [0, 1e301],
            "positions[1] is 1e301; tick positions must not exceed 1e300 in magnitude.",
            None,
        ),
        (
            [-1e301],
            "positions[0] is -1e301; tick positions must not exceed 1e300 in magnitude.",
            None,
        ),
    ]
    for positions, message, hint in cases:
        with pytest.raises(ub.StyleError) as caught:
            method(positions)
        assert str(caught.value) == f"{context}: {message}"
        assert details(caught.value) == ("positions", hint)


@pytest.mark.parametrize("axis", ["x", "y"])
def test_ticks_on_a_log_axis_must_be_positive_and_in_range(axis: str) -> None:
    ax = _axes(log=True)
    method = getattr(ax, f"{axis}ticks")
    for value, shown in ((0, "0"), (-5, "-5")):
        with pytest.raises(ub.StyleError) as caught:
            method([1, value])
        assert str(caught.value) == (
            f"Axes.{axis}ticks(): positions[1] is {shown} on a log axis; "
            "tick positions must be positive."
        )
        assert details(caught.value) == (
            "positions",
            "use a linear axis, or remove positions that are not positive.",
        )
    with pytest.raises(ub.StyleError) as caught:
        method([1, 1e301])
    assert str(caught.value).startswith(f"Axes.{axis}ticks(): positions[1] is 1e301 on a log axis;")


def test_ticks_on_a_category_axis_are_a_usage_error() -> None:
    cats: AnyAxes = ub.Figure().add_axes(x=ub.Scale.category()).bar(["a", "b"], [1, 2])
    calls: list[tuple[str, Callable[[], object]]] = [
        ("xticks", lambda: cats.xticks([1.0])),
        ("xticks_labeled", lambda: cats.xticks_labeled([1.0], ["a"])),
        ("xtick_format", lambda: cats.xtick_format(ub.TickFormat.AUTO)),
    ]
    for name, call in calls:
        with pytest.raises(ub.UsageError) as caught:
            call()
        assert str(caught.value).startswith(f"Axes.{name}(): this is a category axis;")
        assert details(caught.value) == (None, CATEGORY_HINT)
    # The y axis of a category chart is numeric.
    cats.yticks_labeled([0, 1, 2], ["a", "b", "c"]).yticks([0, 1]).ytick_format(
        ub.TickFormat.fixed(1)
    )


@pytest.mark.parametrize("axis", ["x", "y"])
def test_labels_must_match_the_positions(axis: str) -> None:
    ax = _axes()
    method = getattr(ax, f"{axis}ticks_labeled")
    with pytest.raises(ub.StyleError) as caught:
        method([0, 1], ["a"])
    assert str(caught.value) == (
        f"Axes.{axis}ticks_labeled(): positions has 2 values but labels has 1; "
        "pass one label per position."
    )
    assert details(caught.value) == ("labels", None)
    with pytest.raises(ub.StyleError) as caught:
        method([0, 1], ["a", "b" * 1001])
    assert str(caught.value) == (
        f"Axes.{axis}ticks_labeled(): labels[1] has 1,001 characters; {TEXT_CAP}"
    )
    assert details(caught.value) == ("labels", SHORTEN)
    with pytest.raises(ub.StyleError) as caught:
        method([1, 0], ["a", "b"])
    assert str(caught.value).startswith(f"Axes.{axis}ticks_labeled(): positions[1] is 0 but")
    method([0, 1], ["a", "b" * 1000])


def test_labels_are_converted_as_names() -> None:
    ax = _axes()
    with pytest.raises(ub.DataError) as caught:
        ax.xticks_labeled([0, 1], cast(Any, ["a", 1]))
    assert str(caught.value).startswith("Axes.xticks_labeled(): labels[1] is 1;")
    with pytest.raises(TypeError) as other:
        ax.xticks_labeled([0, 1], "ab")
    assert str(other.value).startswith("Axes.xticks_labeled(): labels is str;")
    with pytest.raises(TypeError) as other:
        ax.yticks(cast(Any, "ab"))
    assert str(other.value).startswith("Axes.yticks(): positions is str;")
    ax.xticks_labeled(np.array([0.0, 1.0]), np.array(["lo", "hi"]))
    ax.yticks_labeled((0, 1), ("lo", "hi"))


def test_positions_are_converted_like_data() -> None:
    ax = _axes()
    with pytest.raises(ub.DataError) as caught:
        ax.xticks(cast(Any, [0, "a"]))
    assert str(caught.value).startswith("Axes.xticks(): positions[1] is 'a';")
    ax.xticks(np.array([1, 2, 3], dtype=np.int32))
    ax.yticks(range(3))


# The format.


@pytest.mark.parametrize("axis", ["x", "y"])
def test_labels_then_a_format_conflict(axis: str) -> None:
    ax = _axes()
    getattr(ax, f"{axis}ticks_labeled")([0, 1], ["a", "b"])
    method = getattr(ax, f"{axis}tick_format")
    for fmt in (
        ub.TickFormat.fixed(1),
        ub.TickFormat.AUTO.affix("$"),
        ub.TickFormat.AUTO.affix("", "%"),
    ):
        with pytest.raises(ub.UsageError) as caught:
            method(fmt)
        assert str(caught.value) == f"Axes.{axis}tick_format(): {CONFLICT_WITH_LABELS}"
        assert details(caught.value) == ("format", LABELS_HINT)
    method(ub.TickFormat.AUTO)  # plain AUTO does not conflict


@pytest.mark.parametrize("axis", ["x", "y"])
def test_a_format_then_labels_conflict(axis: str) -> None:
    ax = _axes()
    getattr(ax, f"{axis}tick_format")(ub.TickFormat.percent())
    with pytest.raises(ub.UsageError) as caught:
        getattr(ax, f"{axis}ticks_labeled")([0, 1], ["a", "b"])
    assert str(caught.value) == f"Axes.{axis}ticks_labeled(): {CONFLICT_WITH_FORMAT}"
    assert details(caught.value) == ("labels", FORMAT_HINT)
    # An affix on AUTO counts as a format.
    ax = _axes()
    getattr(ax, f"{axis}tick_format")(ub.TickFormat.AUTO.affix(suffix=" s"))
    with pytest.raises(ub.UsageError):
        getattr(ax, f"{axis}ticks_labeled")([0, 1], ["a", "b"])
    # Setting AUTO again makes labels possible.
    getattr(ax, f"{axis}tick_format")(ub.TickFormat.AUTO)
    getattr(ax, f"{axis}ticks_labeled")([0, 1], ["a", "b"])


def test_the_conflict_highlights_the_call() -> None:
    ax = _axes()
    ax.xticks_labeled([0, 1], ["a", "b"])
    with pytest.raises(ub.UsageError) as caught:
        ax.xtick_format(ub.TickFormat.fixed(1))
    assert _notes(caught.value) == [
        _marked("ax.xtick_format(ub.TickFormat.fixed(1))", "ub.TickFormat.fixed(1)"),
        f"hint: {LABELS_HINT}",
    ]
    ax = _axes()
    ax.xtick_format(ub.TickFormat.fixed(1))
    with pytest.raises(ub.UsageError) as caught:
        ax.xticks_labeled([0, 1], ["a", "b"])
    assert _notes(caught.value) == [
        _marked('ax.xticks_labeled([0, 1], ["a", "b"])', '["a", "b"]'),
        f"hint: {FORMAT_HINT}",
    ]


def test_the_two_axes_are_independent() -> None:
    ax = _axes()
    ax.xticks_labeled([0, 1], ["a", "b"]).ytick_format(ub.TickFormat.percent())
    ax.yticks([1, 10]).xticks([1, 2])


@pytest.mark.parametrize("value", ["percent", None, 1, ub.TickFormat, (1, 2, "", "")])
def test_a_format_must_be_a_tick_format(value: object) -> None:
    ax = _axes()
    name = type_name(value)
    for axis in ("x", "y"):
        with pytest.raises(TypeError) as caught:
            getattr(ax, f"{axis}tick_format")(value)
        assert str(caught.value) == (
            f"Axes.{axis}tick_format(): format is {name}; format must be of type TickFormat."
        )
        assert details(caught.value) == ("format", TYPE_HINT)


def test_a_wrong_format_type_highlights_the_argument() -> None:
    ax = _axes()
    with pytest.raises(TypeError) as caught:
        ax.xtick_format(cast(ub.TickFormat, "percent"))
    assert _notes(caught.value)[0] == _marked(
        'ax.xtick_format(cast(ub.TickFormat, "percent"))', 'cast(ub.TickFormat, "percent")'
    )


def test_check_order_is_types_then_conversion_then_native() -> None:
    ax = _axes()
    with pytest.raises(TypeError) as caught:
        ax.xtick_format(cast(ub.TickFormat, None))
    assert "format is None" in str(caught.value)
    # Positions convert before labels: the first bad argument wins.
    with pytest.raises(ub.DataError) as other:
        ax.xticks_labeled(cast(Any, [0, "a"]), cast(Any, [1, 2]))
    assert str(other.value).startswith("Axes.xticks_labeled(): positions[1]")
    with pytest.raises(ub.DataError) as other:
        ax.xticks_labeled([0, 1], cast(Any, [1, 2]))
    assert str(other.value).startswith("Axes.xticks_labeled(): labels[0]")


def test_errors_keep_the_axes_context_and_highlight_the_call() -> None:
    ax = _axes()
    with pytest.raises(ub.StyleError) as caught:
        ax.xticks([0, math.nan])
    assert _notes(caught.value) == [_marked("ax.xticks([0, math.nan])", "[0, math.nan]")]
    with pytest.raises(ub.StyleError) as caught:
        ax.yticks_labeled([0, 1], ["a"])
    assert _notes(caught.value) == [_marked('ax.yticks_labeled([0, 1], ["a"])', '["a"]')]


# Replacement and atomicity.


def _svg(configure: Callable[[AnyAxes], object]) -> str:
    fig = ub.Figure(4, 3)
    configure(fig.add_axes().line([1, 2, 3], [1, 10, 100]))
    return fig.to_svg()


def test_a_tick_setter_replaces_the_ticks() -> None:
    once = _svg(lambda ax: ax.xticks([1, 2]))
    assert _svg(lambda ax: ax.xticks([1, 1.5]).xticks([1, 2])) == once
    assert _svg(lambda ax: ax.xticks_labeled([1, 1.5], ["a", "b"]).xticks([1, 2])) == once
    assert _svg(lambda ax: ax.xticks([1, 2]).xticks_labeled([1, 2], ["a", "b"])) != once
    labeled = _svg(lambda ax: ax.xticks_labeled([1, 2], ["a", "b"]))
    assert (
        _svg(lambda ax: ax.xticks_labeled([1, 1.5], ["c", "d"]).xticks_labeled([1, 2], ["a", "b"]))
        == labeled
    )
    # Plain positions drop the labels, so a format is accepted again.
    ax = _axes().xticks_labeled([1, 2], ["a", "b"]).xticks([1, 2])
    ax.xtick_format(ub.TickFormat.fixed(1))


def test_a_format_replaces_the_format_and_keeps_the_ticks() -> None:
    first = _svg(lambda ax: ax.xticks([1, 2]).xtick_format(ub.TickFormat.percent()))
    second = _svg(lambda ax: ax.xticks([1, 2]).xtick_format(ub.TickFormat.fixed(2)))
    assert first != second
    both = _svg(
        lambda ax: (
            ax.xticks([1, 2])
            .xtick_format(ub.TickFormat.percent())
            .xtick_format(ub.TickFormat.fixed(2))
        )
    )
    assert both == second
    reset = _svg(
        lambda ax: ax.xtick_format(ub.TickFormat.fixed(2)).xtick_format(ub.TickFormat.AUTO)
    )
    assert reset == _svg(lambda ax: ax)


def test_failed_calls_leave_the_figure_unchanged() -> None:
    fig = ub.Figure(4, 3)
    ax = fig.add_axes().line([1, 2, 3], [1, 10, 100])
    ax.xticks([1, 2]).yticks_labeled([0, 50], ["low", "high"])
    before = fig.to_svg()
    failures: list[Callable[[], object]] = [
        lambda: ax.xticks([1, math.nan]),
        lambda: ax.xticks([2, 1]),
        lambda: ax.yticks([0, 1e301]),
        lambda: ax.xticks_labeled([1, 2], ["a"]),
        lambda: ax.xticks_labeled([1, 2], ["a", "b" * 1001]),
        lambda: ax.ytick_format(ub.TickFormat.fixed(1)),
        lambda: ax.xtick_format(cast(ub.TickFormat, "percent")),
        lambda: ax.xticks([0, cast(float, "a")]),
    ]
    for call in failures:
        with pytest.raises((ub.UmberError, TypeError)):
            call()
        assert fig.to_svg() == before
    # A conflicting call changes nothing either, in both orders.
    with pytest.raises(ub.UsageError):
        ax.ytick_format(ub.TickFormat.fixed(1))
    assert fig.to_svg() == before
    other = ub.Figure(4, 3)
    bx = other.add_axes().line([1, 2, 3], [1, 10, 100]).xtick_format(ub.TickFormat.percent())
    kept = other.to_svg()
    with pytest.raises(ub.UsageError):
        bx.xticks_labeled([1, 2], ["a", "b"])
    assert other.to_svg() == kept


# Rendering parity with the native layer.


def test_custom_ticks_match_the_native_figure() -> None:
    fig = ub.Figure(4, 3)
    fig.add_axes().line([1, 2, 3], [1, 10, 100]).xticks([1, 1.5, 3]).yticks([0, 25, 100])
    native_fig = _native()
    native_fig.xticks(0, 0, _floats(1, 1.5, 3))
    native_fig.yticks(0, 0, _floats(0, 25, 100))
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)
    assert fig.to_png(dpi=60) == native_fig.to_png(60)
    assert fig.to_svg() != _svg(lambda ax: ax)


def test_labeled_ticks_match_the_native_figure() -> None:
    fig = ub.Figure(4, 3)
    fig.add_axes().line([1, 2, 3], [1, 10, 100]).xticks_labeled(
        [1, 3], ["start", "end"]
    ).yticks_labeled([0, 100], ["none", "all"])
    native_fig = _native()
    native_fig.xticks_labeled(0, 0, _floats(1, 3), ["start", "end"])
    native_fig.yticks_labeled(0, 0, _floats(0, 100), ["none", "all"])
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)
    assert fig.to_png(dpi=60) == native_fig.to_png(60)
    plain = _svg(lambda ax: ax.xticks([1, 3]).yticks([0, 100]))
    assert fig.to_svg() != plain


FORMAT_CASES: list[tuple[ub.TickFormat, tuple[int, int, str, str]]] = [
    (ub.TickFormat.AUTO.affix("~"), (0, 0, "~", "")),
    (ub.TickFormat.fixed(2), (1, 2, "", "")),
    (ub.TickFormat.fixed(0).affix("$"), (1, 0, "$", "")),
    (ub.TickFormat.percent(), (2, 0, "", "")),
    (ub.TickFormat.percent(1).affix(suffix=" pts"), (2, 1, "", " pts")),
    (ub.TickFormat.scientific(), (3, 0, "", "")),
    (ub.TickFormat.scientific().affix("<", ">"), (3, 0, "<", ">")),
]


@pytest.mark.parametrize(("fmt", "arg"), FORMAT_CASES, ids=lambda case: repr(case))
@pytest.mark.parametrize("scale", [LINEAR, LOG])
def test_each_format_matches_the_native_figure(
    fmt: ub.TickFormat, arg: tuple[int, int, str, str], scale: int
) -> None:
    kind = ub.Scale.linear() if scale == LINEAR else ub.Scale.log()
    fig = ub.Figure(4, 3)
    fig.add_axes(x=kind, y=kind).line([1, 2, 3], [1, 10, 100]).xtick_format(fmt).ytick_format(fmt)
    native_fig = _native(scale, scale)
    native_fig.xtick_format(0, 0, arg)
    native_fig.ytick_format(0, 0, arg)
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)
    plain = ub.Figure(4, 3)
    plain.add_axes(x=kind, y=kind).line([1, 2, 3], [1, 10, 100])
    assert fig.to_svg() != plain.to_svg()


def test_auto_matches_leaving_the_format_out() -> None:
    assert _svg(lambda ax: ax.xtick_format(ub.TickFormat.AUTO)) == _svg(lambda ax: ax)


def test_a_format_applies_to_custom_positions() -> None:
    fig = ub.Figure(4, 3)
    fig.add_axes().line([1, 2, 3], [1, 10, 100]).xticks([1, 2, 3]).xtick_format(
        ub.TickFormat.fixed(1)
    )
    native_fig = _native()
    native_fig.xticks(0, 0, _floats(1, 2, 3))
    native_fig.xtick_format(0, 0, (1, 1, "", ""))
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)


def test_an_empty_tick_list_removes_the_ticks() -> None:
    fig = ub.Figure(4, 3)
    fig.add_axes().line([1, 2, 3], [1, 10, 100]).xticks([]).yticks(np.array([]))
    native_fig = _native()
    native_fig.xticks(0, 0, _floats())
    native_fig.yticks(0, 0, _floats())
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)
    assert fig.to_svg() != _svg(lambda ax: ax)
    _axes().xticks_labeled([], [])


def test_the_methods_chain_and_return_the_axes() -> None:
    ax = _axes()
    assert ax.xticks([1, 2]) is ax
    assert ax.yticks([1, 2]) is ax
    assert ax.xticks_labeled([1, 2], ["a", "b"]) is ax
    assert ax.yticks_labeled([1, 2], ["a", "b"]) is ax
    assert ax.xtick_format(ub.TickFormat.AUTO) is ax
    assert ax.ytick_format(ub.TickFormat.AUTO) is ax


def test_the_axes_methods_report_the_package_as_their_module() -> None:
    for name in (
        "xticks",
        "yticks",
        "xticks_labeled",
        "yticks_labeled",
        "xtick_format",
        "ytick_format",
    ):
        assert getattr(ub.Axes, name).__module__ == "umber"


# The native layer.


def test_native_check_accepts_valid_formats() -> None:
    for arg in [NATIVE_AUTO, (1, 12, "", ""), (2, 0, "a", "b"), (3, 0, "", "")]:
        _umber.check_tick_format(arg)


@pytest.mark.parametrize("kind", [-1, 4, 99])
def test_an_unknown_native_format_kind_is_an_internal_error(kind: int) -> None:
    with pytest.raises(ValueError, match="internal error: .* is not a tick format kind"):
        _umber.check_tick_format((kind, 0, "", ""))
    fig = _native()
    with pytest.raises(ValueError, match="internal error: .* is not a tick format kind"):
        fig.xtick_format(0, 0, (kind, 0, "", ""))


def test_native_checks_report_the_core_errors() -> None:
    with pytest.raises(ub.StyleError) as caught:
        _umber.check_tick_format((1, 13, "", ""))
    assert str(caught.value) == f"TickFormat.fixed(): decimals is 13; {DECIMALS_RANGE}"
    assert details(caught.value) == ("decimals", None)
    with pytest.raises(ub.StyleError) as caught:
        _umber.check_tick_format((2, -1, "", ""))
    assert str(caught.value) == f"TickFormat.percent(): decimals is -1; {DECIMALS_RANGE}"
    with pytest.raises(ub.StyleError) as caught:
        _umber.check_tick_format((0, 0, "x" * 1001, ""))
    assert str(caught.value) == f"TickFormat.affix(): prefix has 1,001 characters; {TEXT_CAP}"
    assert details(caught.value) == ("prefix", SHORTEN)
    with pytest.raises(ub.StyleError) as caught:
        _umber.check_tick_format((3, 0, "", "y" * 1001))
    assert details(caught.value) == ("suffix", SHORTEN)


def test_native_methods_report_the_core_errors_and_store_nothing() -> None:
    fig = _native()
    before = fig.to_svg(OUTLINES)
    with pytest.raises(ub.StyleError) as caught:
        fig.xticks(0, 0, _floats(2, 1))
    assert str(caught.value).startswith("Axes.xticks(): positions[1] is 1 but positions[0] is 2")
    with pytest.raises(ub.StyleError):
        fig.yticks_labeled(0, 0, _floats(1, 2), ["a"])
    with pytest.raises(ub.StyleError):
        fig.xtick_format(0, 0, (1, 13, "", ""))
    assert fig.to_svg(OUTLINES) == before
    fig.xticks_labeled(0, 0, _floats(1, 2), ["a", "b"])
    with pytest.raises(ub.UsageError):
        fig.xtick_format(0, 0, (1, 1, "", ""))


def test_native_category_x_is_refused() -> None:
    fig = _umber.Figure(4.0, 3.0, _umber.Theme.pigment(), *ONE_CELL)
    fig.add_axes(*ONLY_CELL, CATEGORY, 10.0, LINEAR, 10.0)
    with pytest.raises(ub.UsageError) as caught:
        fig.xticks(0, 0, _floats(1))
    assert str(caught.value).startswith("Axes.xticks(): this is a category axis;")
    fig.yticks(0, 0, _floats(1))


def test_native_tick_methods_need_axes() -> None:
    fig = _umber.Figure(4.0, 3.0, _umber.Theme.pigment(), *ONE_CELL)
    calls: list[Callable[[], object]] = [
        lambda: fig.xticks(0, 0, _floats(1)),
        lambda: fig.yticks_labeled(0, 0, _floats(1), ["a"]),
        lambda: fig.ytick_format(0, 0, NATIVE_AUTO),
    ]
    for call in calls:
        with pytest.raises(
            ub.UsageError, match=r"Figure.axes_at\(\): row 0, col 0 holds no axes\."
        ):
            call()


# Check order: the types of all parameters, then conversion, then the native call.


class _RecordsConversion:
    """A positions object that notes whether it was converted to an array."""

    def __init__(self) -> None:
        self.converted = False

    def __array__(self, dtype: object = None, copy: object = None) -> npt.NDArray[np.float64]:
        self.converted = True
        return _floats(0, 1)


@pytest.mark.parametrize("axis", ["x", "y"])
def test_labeled_types_are_checked_before_any_conversion(axis: str) -> None:
    ax = _axes()
    method = getattr(ax, f"{axis}ticks_labeled")
    with pytest.raises(TypeError) as caught:
        method([0, "bad"], None)
    assert str(caught.value) == (
        f"Axes.{axis}ticks_labeled(): labels is None; labels must be a sequence or an array."
    )
    assert _parameter(caught.value) == "labels"
    recorder = _RecordsConversion()
    with pytest.raises(TypeError) as caught:
        method(recorder, 5)
    assert _parameter(caught.value) == "labels"
    assert not recorder.converted
    # With valid types, a bad value still reports the first parameter.
    with pytest.raises(ub.DataError) as data:
        method([0, "bad"], ["a", "b"])
    assert _parameter(data.value) == "positions"
    # The types of positions come first when both are wrong.
    with pytest.raises(TypeError) as first:
        method(None, None)
    assert _parameter(first.value) == "positions"


def test_category_axis_is_checked_before_label_length() -> None:
    ax: AnyAxes = ub.Figure().add_axes(x=ub.Scale.category())
    fig = _umber.Figure(4.0, 3.0, _umber.Theme.pigment(), *ONE_CELL)
    fig.add_axes(*ONLY_CELL, CATEGORY, 10.0, LINEAR, 10.0)
    labels = ["x" * 1001]
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as expected:
        fig.xticks_labeled(0, 0, _floats(0), labels)
    with pytest.raises(ub.UsageError) as caught:
        ax.xticks_labeled([0.0], labels)
    assert str(caught.value) == str(expected.value)
    assert details(caught.value) == details(expected.value)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("axis", ["x", "y"])
@pytest.mark.parametrize("formatted", [False, True])
@pytest.mark.parametrize("positions", [[float("nan")], [1.0, 0.0], [0.0, 1.0]])
def test_labeled_value_errors_follow_core_order(
    axis: str, formatted: bool, positions: list[float]
) -> None:
    ax = _axes()
    fig = _native()
    if formatted:
        getattr(ax, f"{axis}tick_format")(ub.TickFormat.fixed(1))
        getattr(fig, f"{axis}tick_format")(0, 0, native(ub.TickFormat.fixed(1)))
    before = ax.figure.to_svg()
    labels = ["x" * 1001]
    with pytest.raises(ub.StyleError) as expected:
        getattr(fig, f"{axis}ticks_labeled")(0, 0, _floats(*positions), labels)
    with pytest.raises(ub.StyleError) as caught:
        getattr(ax, f"{axis}ticks_labeled")(positions, labels)
    assert str(caught.value) == str(expected.value)
    assert details(caught.value) == details(expected.value)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("axis", ["x", "y"])
def test_labeled_conversion_checks_encoding_and_plain_strings(axis: str) -> None:
    ax = _axes()
    method = getattr(ax, f"{axis}ticks_labeled")
    with pytest.raises(ub.DataError) as caught:
        method([0], ["a\ud800"])
    assert str(caught.value) == (
        f"Axes.{axis}ticks_labeled(): labels[0] has a lone surrogate at index 1; "
        "text must be valid Unicode."
    )
    assert _parameter(caught.value) == "labels"

    class Label(str):
        def __str__(self) -> str:
            return "changed"

        def __len__(self) -> int:
            return 0

    with pytest.raises(ub.StyleError) as long:
        method([0], [Label("x" * 1001)])
    assert str(long.value) == (
        f"Axes.{axis}ticks_labeled(): labels[0] has 1,001 characters; {TEXT_CAP}"
    )
    method([0], [Label("x" * 1000)])


def test_affix_types_are_checked_before_any_text_validation() -> None:
    with pytest.raises(TypeError) as caught:
        ub.TickFormat.AUTO.affix("x" * 1001, cast(str, None))
    assert str(caught.value) == "TickFormat.affix(): suffix is None; suffix must be a str."
    assert _parameter(caught.value) == "suffix"
    with pytest.raises(ub.StyleError) as long:
        ub.TickFormat.AUTO.affix("x" * 1001, "ok")
    assert _parameter(long.value) == "prefix"
