# pyright: reportPrivateUsage=false
import copy
import math
import pickle
import traceback
from typing import cast

import numpy as np
import numpy.typing as npt
import pytest

import umber as ub
from umber import _umber
from umber._args import type_name
from umber._errors import details
from umber._size import native

AUTO = (0, 0, 1.0)
# A native figure of one cell (rows, cols, share_x, share_y, legend, share_color), and the
# axes position: row and column AUTO (grid index kind 0) spanning one cell.
ONE_CELL = (1, 1, 0, 0, 0, 0)
ONLY_CELL = (0, 0, 0, 0, 1, 1)
NO_SIZE = (0, 0.0)
LINEAR = 0
OUTLINES = 0
TIMES_HINT = "use Size.AUTO for the theme's size, or a factor below 1 for a smaller one."
POINTS_HINT = "use Size.AUTO for the theme's size."
BEYOND_FACTOR_HINT = "use Size.pt() for a size in points."
FACTOR_RANGE = "a size factor must be greater than 0 and at most 10."
POINTS_RANGE = "a size in points must be greater than 0 and at most 72."


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _marked(line: str, argument: str) -> str:
    """The highlight note for ``argument`` in the source ``line``."""
    return f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"


def test_constructors_and_native_form() -> None:
    assert native(ub.Size.AUTO) == (0, 0.0)
    assert native(ub.Size.times(2)) == (1, 2.0)
    assert native(ub.Size.pt(6)) == (2, 6.0)


@pytest.mark.parametrize("factor", [1e-300, 0.5, 1, 10, 10.0])
def test_times_accepts_factors_in_range(factor: float) -> None:
    assert native(ub.Size.times(factor)) == (1, float(factor))


@pytest.mark.parametrize("points", [1e-300, 0.75, 72, 72.0])
def test_pt_accepts_sizes_in_range(points: float) -> None:
    assert native(ub.Size.pt(points)) == (2, float(points))


@pytest.mark.parametrize(
    ("factor", "shown", "hint"),
    [
        (0, "0", TIMES_HINT),
        (-1.5, "-1.5", TIMES_HINT),
        (math.nan, "NaN", None),
        (math.inf, "inf", None),
        (10.000001, "10.000001", BEYOND_FACTOR_HINT),
        (11, "11", BEYOND_FACTOR_HINT),
    ],
)
def test_times_rejects_factors_out_of_range(factor: float, shown: str, hint: str | None) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Size.times(factor)
    assert str(caught.value) == f"Size.times(): factor is {shown}; {FACTOR_RANGE}"
    assert details(caught.value) == ("factor", hint)


@pytest.mark.parametrize(
    ("points", "shown", "hint"),
    [
        (0, "0", POINTS_HINT),
        (-2, "-2", POINTS_HINT),
        (math.nan, "NaN", None),
        (math.inf, "inf", None),
        (72.1, "72.1", None),
        (73, "73", None),
    ],
)
def test_pt_rejects_sizes_out_of_range(points: float, shown: str, hint: str | None) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Size.pt(points)
    assert str(caught.value) == f"Size.pt(): points is {shown}; {POINTS_RANGE}"
    assert details(caught.value) == ("points", hint)


def test_an_int_beyond_the_float_range_is_rejected_natively() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Size.times(10**400)
    assert str(caught.value) == f"Size.times(): factor is inf; {FACTOR_RANGE}"
    with pytest.raises(ub.StyleError) as caught:
        ub.Size.pt(-(10**400))
    assert str(caught.value) == f"Size.pt(): points is -inf; {POINTS_RANGE}"


@pytest.mark.parametrize("value", [True, False, "2", None, [2], b"2"])
def test_constructors_reject_other_types(value: object) -> None:
    name = type_name(value)
    with pytest.raises(TypeError) as caught:
        ub.Size.times(cast(float, value))
    assert str(caught.value) == f"Size.times(): factor is {name}; factor must be a float."
    assert details(caught.value) == ("factor", "pass a number such as 1.5.")
    with pytest.raises(TypeError) as caught:
        ub.Size.pt(cast(float, value))
    assert str(caught.value) == f"Size.pt(): points is {name}; points must be a float."
    assert details(caught.value) == ("points", "pass a number such as 1.5.")


def test_constructors_accept_numpy_numbers() -> None:
    assert ub.Size.times(cast(float, np.int64(2))) == ub.Size.times(2.0)
    assert ub.Size.pt(cast(float, np.float32(4.0))) == ub.Size.pt(4.0)


def test_value_errors_highlight_the_argument() -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Size.times(0.0)
    assert _notes(caught.value) == [_marked("ub.Size.times(0.0)", "0.0"), f"hint: {TIMES_HINT}"]
    with pytest.raises(ub.StyleError) as caught:
        ub.Size.pt(points=100)
    assert _notes(caught.value) == [_marked("ub.Size.pt(points=100)", "points=100")]


def test_type_errors_highlight_the_argument() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Size.pt(cast(float, "2"))
    assert _notes(caught.value) == [
        _marked('ub.Size.pt(cast(float, "2"))', 'cast(float, "2")'),
        "hint: pass a number such as 1.5.",
    ]


def test_equality_and_hash() -> None:
    assert ub.Size.AUTO == ub.Size.AUTO
    assert ub.Size.times(2) == ub.Size.times(2.0)
    assert hash(ub.Size.times(2)) == hash(ub.Size.times(2.0))
    assert ub.Size.pt(2) == ub.Size.pt(2.0)
    assert ub.Size.times(2) != ub.Size.pt(2)
    assert ub.Size.times(2) != ub.Size.times(3)
    assert ub.Size.AUTO != ub.Size.times(1)
    assert ub.Size.AUTO != "auto"
    assert len({ub.Size.AUTO, ub.Size.AUTO, ub.Size.times(1), ub.Size.pt(1)}) == 3


def test_repr() -> None:
    assert repr(ub.Size.AUTO) == "Size.AUTO"
    assert repr(ub.Size.times(1.5)) == "Size.times(1.5)"
    assert repr(ub.Size.times(2)) == "Size.times(2)"
    assert repr(ub.Size.pt(2)) == "Size.pt(2)"
    assert repr(ub.Size.pt(0.75)) == "Size.pt(0.75)"


def test_size_is_immutable_and_has_no_public_constructor() -> None:
    size = ub.Size.times(2)
    with pytest.raises(AttributeError):
        size._kind = 2
    with pytest.raises(AttributeError):
        del size._value
    with pytest.raises(AttributeError):
        size.extra = 1
    with pytest.raises(TypeError) as caught:
        ub.Size()
    assert str(caught.value) == (
        "Size(): Size has no public constructor; use Size.AUTO or one of its constructors."
    )
    assert details(caught.value) == (None, "use Size.AUTO, Size.times() or Size.pt().")
    frames = [entry.name for entry in traceback.extract_tb(caught.value.__traceback__)]
    assert frames == ["test_size_is_immutable_and_has_no_public_constructor"]


SIZES = [ub.Size.AUTO, ub.Size.times(1.5), ub.Size.pt(2)]


@pytest.mark.parametrize("size", SIZES, ids=repr)
def test_copy_and_deepcopy_return_the_same_value(size: ub.Size) -> None:
    assert copy.copy(size) is size
    assert copy.deepcopy(size) is size


@pytest.mark.parametrize("size", SIZES, ids=repr)
def test_pickle_round_trip_gives_an_equal_size(size: ub.Size) -> None:
    for protocol in range(pickle.HIGHEST_PROTOCOL + 1):
        restored: object = pickle.loads(pickle.dumps(size, protocol))
        assert restored == size
        assert repr(restored) == repr(size)


def test_the_module_is_the_package() -> None:
    assert ub.Size.__module__ == "umber"
    assert ub.Size.times.__module__ == "umber"
    assert ub.Size.pt.__module__ == "umber"
    assert "Size" in ub.__all__


# Marks: the size parameters of line and scatter.


@pytest.mark.parametrize("value", [2.0, 2, "2", None, True])
def test_plain_values_are_refused_with_a_hint(value: object) -> None:
    ax = ub.Figure().add_axes()
    hint = (
        "use Size.times(1.5) for a multiple of the theme's size, Size.pt(2.0) for points, "
        "or Size.AUTO."
    )
    name = type_name(value)
    with pytest.raises(TypeError) as caught:
        ax.line([0, 1], [0, 1], width=cast(ub.Size, value))
    assert str(caught.value) == f"Axes.line(): width is {name}; width must be of type Size."
    assert details(caught.value) == ("width", hint)
    with pytest.raises(TypeError) as caught:
        ax.scatter([0, 1], [0, 1], size=cast(ub.Size, value))
    assert str(caught.value) == f"Axes.scatter(): size is {name}; size must be of type Size."
    assert details(caught.value) == ("size", hint)


def test_a_plain_width_highlights_the_argument() -> None:
    ax = ub.Figure().add_axes()
    with pytest.raises(TypeError) as caught:
        ax.line([0, 1], [0, 1], width=cast(ub.Size, 2.0))
    assert _notes(caught.value)[0] == _marked(
        "ax.line([0, 1], [0, 1], width=cast(ub.Size, 2.0))", "width=cast(ub.Size, 2.0)"
    )


def test_a_failed_call_changes_nothing() -> None:
    fig = ub.Figure()
    ax = fig.add_axes()
    before = fig.to_svg()
    with pytest.raises(TypeError):
        ax.line([0, 1], [0, 1], width=cast(ub.Size, 2.0))
    with pytest.raises(TypeError):
        ax.scatter([0, 1], [0, 1], size=cast(ub.Size, 2.0))
    assert fig.to_svg() == before


def test_quick_functions_pass_the_sizes_through() -> None:
    wide = ub.line([0, 1, 2], [1, 3, 2], width=ub.Size.times(3))
    plain = ub.line([0, 1, 2], [1, 3, 2])
    assert wide.figure.to_svg() != plain.figure.to_svg()
    big = ub.scatter([0, 1, 2], [1, 3, 2], size=ub.Size.pt(12))
    small = ub.scatter([0, 1, 2], [1, 3, 2])
    assert big.figure.to_svg() != small.figure.to_svg()


def test_quick_functions_report_a_plain_size() -> None:
    with pytest.raises(TypeError) as caught:
        ub.line([0, 1], [0, 1], width=cast(ub.Size, 2.0))
    assert str(caught.value).startswith("Axes.line(): width is float;")
    assert _notes(caught.value)[0] == _marked(
        "ub.line([0, 1], [0, 1], width=cast(ub.Size, 2.0))", "width=cast(ub.Size, 2.0)"
    )
    with pytest.raises(TypeError) as caught:
        ub.scatter([0, 1], [0, 1], size=cast(ub.Size, 2.0))
    assert str(caught.value).startswith("Axes.scatter(): size is float;")


def floats(*values: float) -> npt.NDArray[np.float64]:
    return np.array(values, dtype=np.float64)


def test_a_line_with_a_relative_width_matches_the_native_figure() -> None:
    fig = ub.Figure(4, 3, theme=ub.Theme.GRAPHITE)
    fig.add_axes().line([0, 1, 2, 3], [1, 3, 2, 4], label="a", width=ub.Size.times(2))
    native_fig = _umber.Figure(4.0, 3.0, _umber.Theme.graphite(), *ONE_CELL)
    native_fig.add_axes(*ONLY_CELL, LINEAR, 10.0, LINEAR, 10.0)
    native_fig.line(0, 0, floats(0, 1, 2, 3), floats(1, 3, 2, 4), "a", AUTO, 0, (1, 2.0))
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)
    assert fig.to_png(dpi=60) == native_fig.to_png(60)
    plain = ub.Figure(4, 3, theme=ub.Theme.GRAPHITE)
    plain.add_axes().line([0, 1, 2, 3], [1, 3, 2, 4], label="a")
    assert fig.to_svg() != plain.to_svg()


def test_a_scatter_with_a_size_in_points_matches_the_native_figure() -> None:
    fig = ub.Figure(4, 3)
    fig.add_axes().scatter([0, 1, 2], [2, 1, 3], marker=ub.Marker.SQUARE, size=ub.Size.pt(11))
    native_fig = _umber.Figure(4.0, 3.0, _umber.Theme.pigment(), *ONE_CELL)
    native_fig.add_axes(*ONLY_CELL, LINEAR, 10.0, LINEAR, 10.0)
    native_fig.scatter(0, 0, floats(0, 1, 2), floats(2, 1, 3), "", AUTO, 1, (2, 11.0))
    assert fig.to_svg() == native_fig.to_svg(OUTLINES)
    assert fig.to_png(dpi=60) == native_fig.to_png(60)
    plain = ub.Figure(4, 3)
    plain.add_axes().scatter([0, 1, 2], [2, 1, 3], marker=ub.Marker.SQUARE)
    assert fig.to_svg() != plain.to_svg()


def test_auto_matches_leaving_the_size_out() -> None:
    explicit = ub.Figure(4, 3)
    explicit.add_axes().line([0, 1], [1, 2], width=ub.Size.AUTO).scatter(
        [0, 1], [1, 2], size=ub.Size.AUTO
    )
    implicit = ub.Figure(4, 3)
    implicit.add_axes().line([0, 1], [1, 2]).scatter([0, 1], [1, 2])
    assert explicit.to_svg() == implicit.to_svg()


# The native layer.


@pytest.mark.parametrize("kind", [-1, 3, 99])
def test_an_unknown_native_size_kind_is_an_internal_error(kind: int) -> None:
    fig = _umber.Figure(3.0, 2.0, _umber.Theme.pigment(), *ONE_CELL)
    fig.add_axes(*ONLY_CELL, LINEAR, 10.0, LINEAR, 10.0)
    with pytest.raises(ValueError, match="internal error: .* is not a size kind"):
        fig.line(0, 0, floats(0, 1), floats(1, 2), "", AUTO, 0, (kind, 1.0))


def test_native_checks_report_the_core_errors() -> None:
    _umber.check_size_times(10.0)
    _umber.check_size_points(72.0)
    with pytest.raises(ub.StyleError) as caught:
        _umber.check_size_times(0.0)
    assert str(caught.value) == f"Size.times(): factor is 0; {FACTOR_RANGE}"
    assert details(caught.value) == ("factor", TIMES_HINT)
    with pytest.raises(ub.StyleError) as caught:
        _umber.check_size_points(72.1)
    assert str(caught.value) == f"Size.pt(): points is 72.1; {POINTS_RANGE}"
    with pytest.raises(ub.StyleError) as caught:
        _umber.check_size_times(math.nan)
    assert details(caught.value) == ("factor", None)


def test_native_marks_report_the_core_error_for_a_bad_size() -> None:
    fig = _umber.Figure(3.0, 2.0, _umber.Theme.pigment(), *ONE_CELL)
    fig.add_axes(*ONLY_CELL, LINEAR, 10.0, LINEAR, 10.0)
    before = fig.to_svg(OUTLINES)
    with pytest.raises(ub.StyleError) as caught:
        fig.scatter(0, 0, floats(0, 1), floats(1, 2), "", AUTO, 0, (2, 100.0))
    assert str(caught.value) == f"Size.pt(): points is 100; {POINTS_RANGE}"
    # The failed call stored no series.
    assert fig.to_svg(OUTLINES) == before
