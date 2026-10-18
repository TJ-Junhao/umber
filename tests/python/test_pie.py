"""Pie panels, direct labels, legends and atomic validation."""

import copy
import random
import xml.etree.ElementTree as ET
from collections.abc import Callable
from typing import Any, cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details


def texts(fig: ub.Figure) -> list[ET.Element]:
    return list(
        ET.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE)).iter("{http://www.w3.org/2000/svg}text")
    )


def test_pie_lookup_on_copy_and_spanned_cells() -> None:
    fig = ub.Figure(rows=2, cols=2)
    ax = fig.add_pie_axes(row=0, col=0, row_span=2).pie(["A"], [1])
    assert fig.pie_axes_at(row=0, col=0) is ax
    with pytest.raises(ub.UsageError):
        fig.pie_axes_at(row=1, col=0)
    twin = copy.copy(fig)
    twin.pie_axes_at(row=0, col=0).title("Copy")
    assert "Copy" not in {t.text for t in texts(fig)}
    assert "Copy" in {t.text for t in texts(twin)}
    with pytest.raises(ub.UsageError) as caught:
        fig.axes_at(row=0, col=0)
    assert "pie_axes_at" in str(details(caught.value))
    with pytest.raises(ub.UsageError):
        fig.pie_axes_at(row=0, col=1)
    fig.add_axes(row=0, col=1)
    with pytest.raises(ub.UsageError):
        fig.pie_axes_at(row=0, col=1)


def test_pie_names_and_second_call_errors() -> None:
    ax = ub.Figure().add_pie_axes()
    with pytest.raises(ub.DataError, match=r"categories\[1\]") as caught:
        ax.pie(["A", ""], [1, 2])
    assert details(caught.value) == ("categories", "give every slice a non-empty name.")
    ax.pie(["A"], [1])
    with pytest.raises(ub.UsageError) as caught_usage:
        ax.pie(["B"], [1])
    info = details(caught_usage.value)
    assert info is not None and info[0] is None


def test_quick_pie_accepts_colors_and_is_final() -> None:
    ax = ub.pie([str(i) for i in range(9)], [1] * 9, colors=[ub.Color.RED] * 9)
    ax.legend(ub.LegendStyle.NONE)
    assert ax.figure.to_svg()
    assert getattr(ub.PieAxes, "__final__", False)
    assert "0.5" in (ub.PieAxes.pie.__doc__ or "")


def test_pie_labels_and_annotations() -> None:
    ax = ub.pie(["North", "South"], [3, 1], annotate=True)
    assert isinstance(ax, ub.PieAxes)
    assert {t.text for t in texts(ax.figure)} == {"North", "South", "75%", "25%"}
    assert ax.figure.to_png(dpi=60).startswith(b"\x89PNG")
    assert copy.copy(ax.figure).to_svg() == ax.figure.to_svg()


def test_small_slices_get_leader_lines() -> None:
    ax = ub.pie(["Large", "Small"], [99, 1], annotate=True)
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert "Small 1%" in svg
    assert 'stroke-width="0.75"' in svg


@pytest.mark.parametrize("theme", [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT])
def test_outside_labels_do_not_overlap(theme: ub.Theme) -> None:
    rng = random.Random(18)
    for _ in range(40):
        names = [f"Slice {i}" for i in range(8)]
        values = [70.0, 40.0, *[rng.uniform(0.01, 2) for _ in range(6)]]
        rng.shuffle(values)
        small = {name for name, value in zip(names, values, strict=True) if value < 3}
        fig = ub.pie(names, values, theme=theme).figure
        try:
            labels = [t for t in texts(fig) if t.text in small]
        except ub.StyleError as error:
            info = details(error)
            assert info is not None
            parameter, hint = info
            assert parameter == "categories"
            assert "Slice" in str(error)
            assert hint is not None and "Other" in hint and "legend" in hint
            continue
        for side in (False, True):
            ys = sorted(
                float(t.attrib["y"]) for t in labels if (float(t.attrib["x"]) > 216) == side
            )
            assert all(b - a >= 10 for a, b in zip(ys, ys[1:], strict=False))


@pytest.mark.parametrize("theme", [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT])
@pytest.mark.parametrize("hole", [0.0, 0.5])
@pytest.mark.parametrize(
    ("names", "values", "size"),
    [
        (
            ["Web", "App", "Shop", "API", "Ads", "Labs", "Tools", "Misc"],
            [50, 20, 12, 8, 4, 3, 2, 1],
            ub.Size.AUTO,
        ),
        (["Everything", "a", "b"], [1e6, 1, 1], ub.Size.AUTO),
        (["A", "B", "C", "D"], [93.3, 2.3, 2.2, 2.2], ub.Size.AUTO),
        ([f"Item {i}" for i in range(6)], [0.5] * 5 + [97.5], ub.Size.AUTO),
        ([f"Item {i}" for i in range(6)], [48, 0.5, 0.5, 0.5, 0.5, 50], ub.Size.AUTO),
        (["Web", "App", "Shop", "Other"], [80, 15, 3, 2], ub.Size.pt(11)),
    ],
)
def test_small_slices_at_top_and_bottom_render(
    theme: ub.Theme, hole: float, names: list[str], values: list[float], size: ub.Size
) -> None:
    fig = ub.Figure(6, 4, theme=theme)
    fig.add_pie_axes().pie(names, values, annotate=True, hole=hole, size=size)
    labels = {t.text for t in texts(fig)}
    assert all(
        any(text is not None and (text == name or text.startswith(name + " ")) for text in labels)
        for name in names
    )
    assert fig.to_png(dpi=60).startswith(b"\x89PNG")


def test_unplaceable_labels_name_slices() -> None:
    ax = ub.Figure(1, 1).add_pie_axes().pie(["An impossibly wide slice name", "B"], [99, 1])
    with pytest.raises(ub.StyleError, match="An impossibly wide slice name") as caught:
        ax.figure.to_svg()
    assert details(caught.value) is not None
    assert "cannot be placed outside the circle" in str(caught.value)
    assert "legend" in str(details(caught.value))
    ax.legend(ub.LegendStyle.NONE)
    assert ax.figure.to_svg()


def test_too_many_slices_rejected() -> None:
    ax = ub.Figure().add_pie_axes()
    with pytest.raises(ub.StyleError, match="colors"):
        ax.pie([str(i) for i in range(100)], [1] * 100)
    ax.pie(["A"], [1])


def test_pie_never_joins_merged_legend() -> None:
    fig = ub.Figure(cols=2)
    fig.add_axes(col=0).line([0, 1], [1, 2], label="North").legend(ub.LegendStyle.HEADER)
    pie = fig.add_pie_axes(col=1).pie(["North", "South"], [1, 2])
    pie.legend(ub.LegendStyle.HEADER, size=ub.Size.pt(9))
    labels = [t for t in texts(fig) if t.text == "North"]
    assert len(labels) == 2
    assert float(labels[0].attrib["y"]) != float(labels[1].attrib["y"])


@pytest.mark.parametrize("values", [[-1, 2], [np.nan, 1], [np.inf, 1], [0, 0], [1]])
def test_invalid_values_are_atomic(values: list[float]) -> None:
    ax = ub.Figure().add_pie_axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.pie(["A", "B"], values)
    info = details(caught.value)
    assert info is not None and info[0] == "values" and info[1]
    assert ax.figure.to_svg() == before
    ax.pie(["A"], [1])


@pytest.mark.parametrize("hole", [-0.1, 1, np.nan, np.inf])
def test_invalid_hole(hole: float) -> None:
    with pytest.raises(ub.StyleError) as caught:
        ub.pie(["A"], [1], hole=hole)
    info = details(caught.value)
    assert info is not None and info[0] == "hole" and info[1]


def test_second_pie_and_cartesian_calls_are_usage_errors() -> None:
    ax = ub.pie(["A"], [1])
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError):
        ax.pie(["B"], [1])
    with pytest.raises(ub.UsageError):
        cast(Any, ax).line([0, 1], [1, 2])
    with pytest.raises(ub.UsageError):
        ax.figure.axes_at()
    assert ax.figure.to_svg() == before


def test_zero_only_in_legend_and_none_hides_names() -> None:
    ax = ub.pie(["A", "Zero"], [1, 0])
    assert {t.text for t in texts(ax.figure)} == {"A"}
    ax.legend(ub.LegendStyle.HEADER)
    assert {t.text for t in texts(ax.figure)} == {"A", "Zero"}
    ax.legend(ub.LegendStyle.NONE)
    assert not texts(ax.figure)


def test_colors_sizes_and_format() -> None:
    ax = (
        ub.Figure()
        .add_pie_axes()
        .pie(
            ["A", "B"],
            [1, 3],
            hole=0.5,
            annotate=True,
            format=ub.TickFormat.percent(decimals=1),
            colors=[ub.Color.RED, ub.Color.BLUE],
            size=ub.Size.pt(8),
        )
    )
    ax.title("Donut", size=ub.Size.pt(18))
    assert {t.text for t in texts(ax.figure)} >= {"Donut", "25.0%", "75.0%"}


@pytest.mark.parametrize("theme", [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT])
def test_direct_default_ignores_theme_legend(theme: ub.Theme) -> None:
    fig = ub.pie(["A", "B"], [3, 1], theme=theme).figure
    assert {t.text for t in texts(fig)} == {"A", "B"}
    assert all(float(t.attrib["y"]) > 40 for t in texts(fig))


def test_explicit_colors_and_zero_keep_their_order() -> None:
    colors = [ub.Color.RED, ub.Color.BLUE, ub.Color.GREEN]
    ax = ub.Figure().add_pie_axes()
    with pytest.raises(ub.StyleError) as caught:
        ax.pie(["A", "B"], [1, 1], colors=colors)
    assert "colors" in str(details(caught.value))
    ax.pie(["A", "Zero", "B"], [1, 0, 1], colors=colors)
    ax.legend(ub.LegendStyle.HEADER)
    assert [t.text for t in texts(ax.figure)] == ["A", "Zero", "B"]


def test_pie_copies_values_and_handles_large_finite_totals() -> None:
    values = np.array([1e308, 1e308])
    ax = ub.pie(["A", "B"], values, annotate=True)
    before = ax.figure.to_svg()
    values[:] = 0
    assert ax.figure.to_svg() == before
    assert [t.text for t in texts(ax.figure)].count("50%") == 2


def test_duplicate_categories_and_empty_values_are_atomic() -> None:
    ax = ub.Figure().add_pie_axes()
    for names, values in [(["A", "A"], [1, 1]), ([], [])]:
        with pytest.raises(ub.DataError):
            ax.pie(names, values)
    ax.pie(["A"], [1])


def test_explicit_direct_position_is_atomic() -> None:
    ax = ub.pie(["A", "B"], [1, 1])
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.legend(ub.LegendStyle.DIRECT, position=ub.LegendPosition.RIGHT)
    assert "position" in str(details(caught.value))
    assert ax.figure.to_svg() == before
    ax.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT)
    assert {t.text for t in texts(ax.figure)} == {"A", "B"}


def test_grid_placement_and_sharing() -> None:
    fig = ub.Figure(rows=2, cols=2, share_x=ub.Share.ALL, share_y=ub.Share.ALL)
    fig.add_pie_axes(col=0, row=0, row_span=2).pie(["A", "B"], [1, 2])
    fig.add_axes(col=1, row=0).line([0, 1], [1, 2])
    fig.add_axes(col=1, row=1).line([0, 1], [2, 3])
    assert fig.to_svg()
    with pytest.raises(ub.UsageError):
        fig.add_axes(col=0, row=1)


@pytest.mark.parametrize("option", ["hole", "annotate", "format", "colors", "size"])
def test_wrong_option_types(option: str) -> None:
    ax = ub.Figure().add_pie_axes()
    call = cast(Callable[..., object], ax.pie)
    with pytest.raises(TypeError):
        call(["A"], [1], **{option: None})
