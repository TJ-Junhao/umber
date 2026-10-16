"""Shared axes through the native binding, before the public Python grid API."""

from xml.etree import ElementTree

import numpy as np
import pytest

import umber as ub
from umber import _umber

AUTO = (0, 0, 1.0)
NO_SIZE = (0, 0.0)
LINEAR, LOG, CATEGORY = 0, 1, 2
NONE, ROW, COL, ALL = 0, 1, 2, 3
EDITABLE = 1


def figure(share_x: int = ALL, share_y: int = ALL) -> _umber.Figure:
    return _umber.Figure(12.0, 8.0, _umber.Theme.graphite(), 2, 2, share_x, share_y, 1, NONE)


def add(fig: _umber.Figure, row: int, col: int) -> None:
    fig.add_axes(1, row, 1, col, 1, 1, LINEAR, 10.0, LINEAR, 10.0)
    fig.line(
        row,
        col,
        np.array([0.0, 10.0]),
        np.array([1.0, 3.0]),
        "",
        AUTO,
        0,
        NO_SIZE,
    )


def set_value(fig: _umber.Figure, row: int, col: int, operation: int, other: bool) -> None:
    high = 5.0 if other else 10.0
    positions = np.array([0.0, high])
    labels = ["start", "end"]
    match operation:
        case 0:
            fig.xlim(row, col, 0.0, high)
        case 1:
            fig.ylim(row, col, 0.0, high)
        case 2:
            fig.xticks(row, col, positions)
        case 3:
            fig.yticks(row, col, positions)
        case 4:
            fig.xticks_labeled(row, col, positions, labels)
        case 5:
            fig.yticks_labeled(row, col, positions, labels)
        case 6:
            fig.xtick_format(row, col, (1, 2 if other else 1, "", ""))
        case _:
            fig.ytick_format(row, col, (1, 2 if other else 1, "", ""))


@pytest.mark.parametrize("operation", range(8))
def test_group_setters_propagate_and_conflicts_are_atomic(operation: int) -> None:
    fig = figure()
    add(fig, 1, 1)
    set_value(fig, 1, 1, operation, False)
    add(fig, 0, 0)
    before = fig.to_svg(EDITABLE)
    copy = fig.clone()
    set_value(copy, 1, 1, operation, True)
    assert fig.to_svg(EDITABLE) == before
    assert copy.to_svg(EDITABLE) != before
    with pytest.raises(ub.UsageError) as caught:
        set_value(fig, 0, 0, operation, True)
    assert "shared with row 1, col 1" in str(caught.value)
    assert vars(caught.value)["_umber_hint"] == "set it once for the group, or share fewer axes."
    assert fig.to_svg(EDITABLE) == before
    set_value(fig, 0, 0, operation, False)
    assert fig.to_svg(EDITABLE) == before


@pytest.mark.parametrize("share_x", [NONE, ROW, COL, ALL])
@pytest.mark.parametrize("share_y", [NONE, ROW, COL, ALL])
def test_all_modes_render_with_only_outer_tick_labels(share_x: int, share_y: int) -> None:
    fig = figure(share_x, share_y)
    for row, col in [(0, 0), (0, 1), (1, 0), (1, 1)]:
        add(fig, row, col)
        fig.xticks_labeled(row, col, np.array([0.0, 10.0]), ["LOWX", "HIGHX"])
        fig.yticks_labeled(row, col, np.array([1.0, 3.0]), ["LOWY", "HIGHY"])
        fig.xlabel(row, col, "Time")
        fig.ylabel(row, col, "Level")
    root = ElementTree.fromstring(fig.to_svg(EDITABLE))
    texts = ["".join(node.itertext()) for node in root.iter("{http://www.w3.org/2000/svg}text")]
    assert texts.count("LOWX") == (2 if share_x in (COL, ALL) else 4)
    assert texts.count("LOWY") == (2 if share_y in (ROW, ALL) else 4)
    assert texts.count("Time") == 4
    assert texts.count("Level") == 4


@pytest.mark.parametrize("dimension", ["x", "y"])
@pytest.mark.parametrize("reverse", [False, True])
def test_mixed_spans_keep_labels_on_each_panel(dimension: str, reverse: bool) -> None:
    fig = figure()
    places = (
        [(0, 0, 1, 1), (0, 1, 1, 1), (1, 0, 1, 2)]
        if dimension == "x"
        else [(0, 1, 1, 1), (1, 1, 1, 1), (0, 0, 2, 1)]
    )
    for row, col, row_span, col_span in reversed(places) if reverse else places:
        fig.add_axes(1, row, 1, col, row_span, col_span, LINEAR, 10.0, LINEAR, 10.0)
        fig.xticks_labeled(row, col, np.array([0.0, 1.0]), ["LOWX", "HIGHX"])
        fig.yticks_labeled(row, col, np.array([0.0, 1.0]), ["LOWY", "HIGHY"])
    root = ElementTree.fromstring(fig.to_svg(EDITABLE))
    texts = ["".join(node.itertext()) for node in root.iter("{http://www.w3.org/2000/svg}text")]
    for label in ("LOWX", "HIGHX") if dimension == "x" else ("LOWY", "HIGHY"):
        assert texts.count(label) == len(places)


def test_mixed_scales_and_cross_group_spans_surface_as_usage_errors() -> None:
    fig = figure(ROW, NONE)
    add(fig, 0, 0)
    before = fig.to_svg(EDITABLE)
    with pytest.raises(ub.UsageError) as caught:
        fig.add_axes(1, 0, 1, 1, 1, 1, CATEGORY, 10.0, LINEAR, 10.0)
    assert str(caught.value) == (
        "Figure.add_axes(): x is Scale.category(); the axes at row 0, col 0 "
        "shares its x axis and has Scale.linear()."
    )
    assert vars(caught.value)["_umber_parameter"] == "x"
    assert fig.to_svg(EDITABLE) == before
    with pytest.raises(ub.UsageError) as caught:
        fig.add_axes(1, 0, 1, 1, 2, 1, LINEAR, 10.0, LINEAR, 10.0)
    assert str(caught.value) == (
        "Figure.add_axes(): the axes spans rows 0 to 1, which share x in different groups."
    )
    assert vars(caught.value)["_umber_parameter"] == "row_span"
    assert fig.to_svg(EDITABLE) == before
