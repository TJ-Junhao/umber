import copy
from collections.abc import Callable
from typing import Any, cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details


def _details(error: BaseException) -> tuple[str | None, str | None]:
    value = details(error)
    assert value is not None
    return value


def test_grid_round_trip() -> None:
    fig = ub.Figure(rows=2, cols=2)
    ax = fig.add_axes(row=0, col=1, x=ub.Scale.category())
    ax.bar(["a", "b"], [1.0, 2.0])
    assert fig.axes_at(row=0, col=1, x=ub.Scale.category()) is ax


def test_auto_row_on_a_larger_grid_raises() -> None:
    with pytest.raises(ub.UsageError) as info:
        ub.Figure(rows=2).add_axes()
    assert str(info.value) == (
        "Figure.add_axes(): row is AUTO; this figure has 2 rows, so each axes needs a row."
    )


def test_axes_at_checks_the_scales() -> None:
    fig = ub.Figure(cols=2)
    fig.add_axes(col=0, x=ub.Scale.category())
    with pytest.raises(ub.UsageError):
        fig.axes_at(col=0)


def test_cells_yield_row_major_and_count_checked() -> None:
    fig = ub.Figure(rows=2, cols=3)
    assert [(c.row, c.col) for c in fig.cells()] == [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2)]
    assert [(c.row, c.col) for c in fig.cells(2)] == [(0, 0), (0, 1)]
    with pytest.raises(ub.StyleError, match=r"count is 7; a 2 × 3 figure has 6 cells\."):
        list(fig.cells(7))
    with pytest.raises(TypeError):
        list(fig.cells(True))


def test_auto_is_a_typed_sentinel() -> None:
    assert repr(ub.AUTO) == "AUTO" and ub.AUTO is ub.Auto.AUTO and ub.Auto.__module__ == "umber"


def test_auto_size_is_six_by_four_per_panel() -> None:
    svg = ub.Figure(rows=2, cols=3).to_svg()
    assert 'width="1296pt" height="576pt"' in svg


def test_copy_of_grid_is_independent() -> None:
    fig = ub.Figure(rows=2)
    fig.add_axes(row=0).line([0, 1], [0, 1])
    fig.add_axes(row=1)
    before = fig.to_svg()
    twin = copy.copy(fig)
    twin.axes_at(row=1).line([0, 1], [1, 0])
    assert fig.to_svg() == before
    with pytest.raises(ub.UsageError):
        twin.axes_at(row=1, x=ub.Scale.category())


@pytest.mark.parametrize("dimension", ["rows", "cols"])
@pytest.mark.parametrize("value", [-1, 0, 33])
def test_grid_dimensions_have_core_range_errors(dimension: str, value: int) -> None:
    with pytest.raises(ub.StyleError) as caught:
        cast(Any, ub.Figure)(**{dimension: value})
    assert str(caught.value) == (
        f"Figure(): {dimension} is {value}; {dimension} must be between 1 and 32."
    )
    assert _details(caught.value)[0] == dimension
    assert _details(caught.value)[1]


@pytest.mark.parametrize("name", ["rows", "cols"])
@pytest.mark.parametrize("value", [True, np.bool_(True), 1.5, None, ub.AUTO])
def test_grid_dimensions_reject_wrong_types(name: str, value: object) -> None:
    with pytest.raises(TypeError) as caught:
        cast(Any, ub.Figure)(**{name: value})
    assert str(caught.value).startswith(f"Figure(): {name} is ")
    assert str(caught.value).endswith(f"{name} must be an int.")
    assert _details(caught.value) == (name, "pass a whole number such as 3.")


@pytest.mark.parametrize(
    ("width", "height", "rows", "cols", "parameter"),
    [
        (0, 0, 0, 0, "width"),
        (0, ub.AUTO, 0, 0, "width"),
        (ub.AUTO, 0, 0, 0, "height"),
        (6, 0, 0, 0, "height"),
        (ub.AUTO, ub.AUTO, 0, 0, "rows"),
        (6, 4, 0, 0, "rows"),
        (ub.AUTO, ub.AUTO, 1, 0, "cols"),
        (6, ub.AUTO, 1, 0, "cols"),
    ],
)
def test_constructor_value_checks_follow_parameter_order(
    width: float | ub.Auto, height: float | ub.Auto, rows: int, cols: int, parameter: str
) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Figure(width, height, rows=rows, cols=cols)
    assert _details(caught.value)[0] == parameter


@pytest.mark.parametrize(
    "name", ["width", "height", "rows", "cols", "share_x", "share_y", "legend", "theme"]
)
def test_all_constructor_type_checks_precede_value_checks(name: str) -> None:
    arguments: dict[str, object] = {"width": 0, "height": 0, "rows": 0, "cols": 0, name: None}
    with pytest.raises(TypeError) as caught:
        cast(Any, ub.Figure)(**arguments)
    assert _details(caught.value)[0] == name


@pytest.mark.parametrize("name", ["share_x", "share_y", "legend"])
@pytest.mark.parametrize("value", [0, None, ub.AUTO, ub.LegendStyle.HEADER])
def test_grid_options_require_their_enum(name: str, value: object) -> None:
    with pytest.raises(TypeError) as caught:
        cast(Any, ub.Figure)(**{name: value})
    assert _details(caught.value)[0] == name
    assert _details(caught.value)[1]


def test_size_can_be_automatic_on_either_dimension() -> None:
    assert 'width="360pt" height="576pt"' in ub.Figure(5, rows=2, cols=3).to_svg()
    assert 'width="1296pt" height="216pt"' in ub.Figure(height=3, rows=2, cols=3).to_svg()
    assert 'width="360pt" height="216pt"' in ub.Figure(5, 3, rows=2, cols=3).to_svg()


@pytest.mark.parametrize("method", ["add_axes", "axes_at"])
@pytest.mark.parametrize("name", ["row", "col"])
@pytest.mark.parametrize("value", [True, np.bool_(True), None, "0", 0.5])
def test_positions_reject_wrong_types(method: str, name: str, value: object) -> None:
    fig = ub.Figure()
    with pytest.raises(TypeError) as caught:
        getattr(fig, method)(**{name: value})
    shown = "None" if value is None else type(value).__name__
    if isinstance(value, np.generic):
        shown = f"numpy.{shown}"
    assert str(caught.value) == (
        f"Figure.{method}(): {name} is {shown}; {name} must be an int or AUTO."
    )
    assert _details(caught.value) == (
        name,
        "pass a whole number such as 0, or ub.AUTO for a single row or column.",
    )


@pytest.mark.parametrize("name", ["row_span", "col_span"])
@pytest.mark.parametrize("value", [True, np.bool_(True), ub.AUTO, None, 1.5])
def test_spans_reject_wrong_types(name: str, value: object) -> None:
    with pytest.raises(TypeError) as caught:
        cast(Any, ub.Figure().add_axes)(**{name: value})
    assert _details(caught.value) == (name, "pass a whole number such as 3.")


@pytest.mark.parametrize("count", [True, np.bool_(True), None, 1.5])
def test_cells_type_errors_happen_at_the_call(count: object) -> None:
    with pytest.raises(TypeError) as caught:
        ub.Figure().cells(cast(int, count))
    assert str(caught.value).startswith("Figure.cells(): count is ")
    assert _details(caught.value) == ("count", "pass a whole number such as 3.")


@pytest.mark.parametrize("count", [-1, 0, 7])
def test_cells_range_errors_happen_at_the_call(count: int) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.Figure(rows=2, cols=3).cells(count)
    assert str(caught.value).startswith(f"Figure.cells(): count is {count}; ")
    assert _details(caught.value)[0] == "count"
    assert _details(caught.value)[1]


def test_numpy_integer_positions_counts_and_spans() -> None:
    fig = ub.Figure(rows=cast(int, np.int64(2)), cols=cast(int, np.int32(2)))
    ax = fig.add_axes(row=cast(int, np.int64(1)), col=0, col_span=cast(int, np.int32(2)))
    assert fig.axes_at(row=1, col=0) is ax
    assert len(list(fig.cells(cast(int, np.int64(3))))) == 3


def test_cells_are_immutable_and_have_no_public_constructor() -> None:
    with pytest.raises(TypeError) as caught:
        ub.Cell()
    assert str(caught.value) == "Cell(): Cell has no public constructor; cells belong to a figure."
    assert _details(caught.value) == (None, "iterate over a Figure's cells: ub.Figure().cells().")
    cell = list(ub.Figure(rows=2, cols=3).cells())[-1]
    assert repr(cell) == "Cell(row=1, col=2)"
    assert not hasattr(cell, "__dict__")
    for name in ("row", "col", "_row", "_col", "_figure", "extra"):
        with pytest.raises(AttributeError, match="a Cell is immutable"):
            setattr(cell, name, 4)
        with pytest.raises(AttributeError, match="a Cell is immutable"):
            delattr(cell, name)


@pytest.mark.parametrize("duplicate", [copy.copy, copy.deepcopy])
@pytest.mark.parametrize("occupied", [False, True])
def test_cell_copies_keep_identity_and_figure(
    duplicate: Callable[[ub.Cell], ub.Cell], occupied: bool
) -> None:
    fig = ub.Figure(rows=2, cols=2)
    cell = list(fig.cells())[-1]
    if occupied:
        cell.add_axes()
    cloned = duplicate(cell)
    assert cloned is cell
    assert (cloned.row, cloned.col) == (1, 1)
    if occupied:
        with pytest.raises(ub.UsageError):
            cloned.add_axes()
    else:
        assert cloned.add_axes().figure is fig
    assert fig.axes_at(row=1, col=1).figure is fig


def test_cells_stay_attached_and_include_occupied_positions() -> None:
    fig = ub.Figure(cols=2)
    cell = next(fig.cells())
    ax = cell.add_axes(x=ub.Scale.category()).bar(["A"], [2])
    assert ax.figure is fig
    assert fig.axes_at(col=0, x=ub.Scale.category()) is ax
    assert [(c.row, c.col) for c in fig.cells()] == [(0, 0), (0, 1)]
    before = fig.to_svg()
    with pytest.raises(ub.UsageError, match=r"^Figure.add_axes\(\):"):
        cell.add_axes()
    assert fig.to_svg() == before
    for name in ("x", "y"):
        with pytest.raises(TypeError) as caught:
            cast(Any, cell.add_axes)(**{name: ub.AUTO})
        assert str(caught.value).startswith(f"Cell.add_axes(): {name} is Auto;")
        assert _details(caught.value)[0] == name


@pytest.mark.parametrize("method", ["add_axes", "axes_at"])
def test_auto_column_needs_an_explicit_position(method: str) -> None:
    fig = ub.Figure(cols=2)
    with pytest.raises(ub.UsageError) as caught:
        getattr(fig, method)()
    assert str(caught.value) == (
        f"Figure.{method}(): col is AUTO; this figure has 2 columns, so each axes needs a col."
    )
    assert _details(caught.value)[0] == "col"


def test_failed_placements_leave_axes_and_output_unchanged() -> None:
    fig = ub.Figure(rows=2, cols=2, share_x=ub.Share.ALL)
    ax = fig.add_axes(row=0, col=0, col_span=2).line([0, 1], [1, 2])
    before = fig.to_svg()
    invalid: list[dict[str, object]] = [
        {"row": 0, "col": 1},
        {"row": 2, "col": 0},
        {"row": -1, "col": 0},
        {"row": 1, "col": 0, "row_span": 0},
        {"row": 1, "col": 0, "col_span": 3},
        {"row": 1, "col": 0, "x": ub.Scale.category()},
    ]
    for arguments in invalid:
        with pytest.raises(ub.UsageError):
            cast(Any, fig.add_axes)(**arguments)
        assert fig.axes_at(row=0, col=0) is ax
        assert fig.to_svg() == before
    with pytest.raises(ub.UsageError, match="is part of the axes added"):
        fig.axes_at(row=0, col=1)
    with pytest.raises(ub.UsageError, match="holds no axes"):
        fig.axes_at(row=1, col=0)
    fig.add_axes(row=1, col=0)


@pytest.mark.parametrize("duplicate", [copy.copy, copy.deepcopy])
def test_every_copied_panel_uses_the_cloned_native_figure(
    duplicate: Callable[[ub.Figure], ub.Figure],
) -> None:
    fig = ub.Figure(rows=2, cols=2)
    fig.add_axes(row=0, col=0, col_span=2, x=ub.Scale.category()).bar(["A"], [2])
    fig.add_axes(row=1, col=0, x=ub.Scale.log(2), y=ub.Scale.log(10)).line([1, 2], [1, 10])
    before = fig.to_svg()
    twin = duplicate(fig)
    assert twin.to_svg() == before
    category = twin.axes_at(row=0, col=0, x=ub.Scale.category())
    numeric = twin.axes_at(row=1, col=0, x=ub.Scale.log(2), y=ub.Scale.log(10))
    assert category.figure is twin and numeric.figure is twin
    assert category is not fig.axes_at(row=0, col=0, x=ub.Scale.category())
    category.bar(["B"], [3])
    numeric.line([1, 4], [2, 20])
    list(twin.cells())[-1].add_axes().title("New")
    assert twin.to_svg() != before
    assert fig.to_svg() == before
    with pytest.raises(ub.UsageError):
        twin.axes_at(row=1, col=0, x=ub.Scale.log(10), y=ub.Scale.log(10))
    with pytest.raises(ub.UsageError):
        twin.axes_at(row=1, col=0, x=ub.Scale.log(2), y=ub.Scale.log(2))
    with pytest.raises(ub.UsageError):
        twin.axes_at(row=0, col=1, x=ub.Scale.category())


def test_shared_groups_keep_their_validation_on_a_copy() -> None:
    fig = ub.Figure(rows=2, share_x=ub.Share.COL)
    fig.add_axes(row=0).xlim(0, 10)
    fig.add_axes(row=1)
    twin = copy.copy(fig)
    before = twin.to_svg()
    with pytest.raises(ub.UsageError):
        twin.axes_at(row=1).xlim(0, 5)
    assert twin.to_svg() == before == fig.to_svg()


@pytest.mark.parametrize("axis", ["x", "y"])
@pytest.mark.parametrize("share", list(ub.Share))
def test_sharing_modes_are_forwarded(axis: str, share: ub.Share) -> None:
    fig = ub.Figure(
        rows=2,
        cols=2,
        share_x=share if axis == "x" else ub.Share.NONE,
        share_y=share if axis == "y" else ub.Share.NONE,
    )
    first = fig.add_axes(row=0, col=0)
    row, col = (1, 0) if share is ub.Share.COL else (0, 1)
    second = fig.add_axes(row=row, col=col)
    getattr(first, f"{axis}lim")(0, 10)
    if share is ub.Share.NONE:
        getattr(second, f"{axis}lim")(0, 5)
    else:
        before = fig.to_svg()
        with pytest.raises(ub.UsageError):
            getattr(second, f"{axis}lim")(0, 5)
        assert fig.to_svg() == before


def test_spanning_shared_groups_is_atomic() -> None:
    fig = ub.Figure(rows=2, cols=2, share_x=ub.Share.COL)
    before = fig.to_svg()
    with pytest.raises(ub.UsageError):
        fig.add_axes(row=0, col=0, col_span=2)
    assert fig.to_svg() == before
    fig.add_axes(row=0, col=0)


@pytest.mark.parametrize("legend", list(ub.Legend))
def test_legend_mode_is_forwarded(legend: ub.Legend) -> None:
    fig = ub.Figure(cols=2, legend=legend)
    for cell in fig.cells():
        cell.add_axes().line([0, 1], [1, 2], label="Signal")
    svg = fig.to_svg(text=ub.SvgText.EDITABLE)
    assert svg.count(">Signal</text>") == (1 if legend is ub.Legend.MERGED else 2)
