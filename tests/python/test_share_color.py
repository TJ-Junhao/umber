"""Shared color ranges, figure colorbars and atomic setters."""

import copy
import re
from collections.abc import Callable
from datetime import date
from pathlib import Path
from typing import cast
from xml.etree import ElementTree as ET

import pytest

import umber as ub
from umber._errors import details

NS = "{http://www.w3.org/2000/svg}"
CELL_MARKS = ["heatmap", "mesh", "hist2d", "hexbin"]


def cell_axes(
    fig: ub.Figure,
    mark: str,
    *,
    row: int = 0,
    col: int = 0,
    row_span: int = 1,
    col_span: int = 1,
) -> ub.Axes[str, str] | ub.Axes[float, float]:
    if mark == "heatmap":
        return fig.add_axes(
            x=ub.Scale.category(),
            y=ub.Scale.category(),
            row=row,
            col=col,
            row_span=row_span,
            col_span=col_span,
        )
    return fig.add_axes(row=row, col=col, row_span=row_span, col_span=col_span)


def cell_arguments(mark: str) -> str:
    if mark == "heatmap":
        return '["a", "b"], ["r"], [[1, 2]]'
    if mark == "mesh":
        return "[0, 1, 2], [0, 1], [[1, 2]]"
    return "[0, 1, 2], [0, 1, 2]"


def colored(fig: ub.Figure, row: int, col: int, values: list[float]) -> ub.Axes[float, float]:
    return fig.add_axes(row=row, col=col).scatter(
        [float(i) for i in range(len(values))], values, color=ub.ColorBy(values)
    )


def grid(mode: ub.Share, *, width: float = 12, height: float = 8) -> ub.Figure:
    fig = ub.Figure(width, height, rows=2, cols=2, share_color=mode)
    for row in range(2):
        for col in range(2):
            colored(fig, row, col, [0, 5, 10 + row * 20 + col * 10])
    return fig


def geometry(fig: ub.Figure) -> tuple[list[dict[str, float]], list[dict[str, float]]]:
    root = ET.fromstring(fig.to_svg(text=ub.SvgText.EDITABLE))
    bars = [
        {k: float(node.attrib[k]) for k in ("x1", "y1", "x2", "y2")}
        for node in root.iter(f"{NS}linearGradient")
    ]
    plots = [
        {k: float(node.attrib[k]) for k in ("x", "y", "width", "height")}
        for clip in root.iter(f"{NS}clipPath")
        for node in clip.iter(f"{NS}rect")
    ]
    return bars, plots


@pytest.mark.parametrize("position", [ub.ColorbarPosition.RIGHT, ub.ColorbarPosition.BOTTOM])
def test_share_all_one_colorbar_spans_grid(position: ub.ColorbarPosition) -> None:
    fig = grid(ub.Share.ALL)
    assert fig.colorbar("Intensity", position=position) is fig
    bars, plots = geometry(fig)
    assert len(bars) == 1 and len(plots) == 4
    bar = bars[0]
    if position is ub.ColorbarPosition.RIGHT:
        assert bar["y2"] == pytest.approx(min(p["y"] for p in plots), abs=0.002)
        assert bar["y1"] == pytest.approx(max(p["y"] + p["height"] for p in plots), abs=0.002)
        assert bar["x1"] > max(p["x"] + p["width"] for p in plots)
    else:
        assert bar["x1"] == pytest.approx(min(p["x"] for p in plots), abs=0.002)
        assert bar["x2"] == pytest.approx(max(p["x"] + p["width"] for p in plots), abs=0.002)
        assert bar["y1"] > max(p["y"] + p["height"] for p in plots)


def test_share_row_bars_per_row() -> None:
    bars, plots = geometry(grid(ub.Share.ROW))
    assert len(bars) == 2
    for row, bar in enumerate(bars):
        assert bar["x1"] > plots[row * 2 + 1]["x"] + plots[row * 2 + 1]["width"]
        assert bar["y2"] == pytest.approx(plots[row * 2]["y"], abs=0.002)
        assert bar["y1"] - bar["y2"] == pytest.approx(plots[row * 2]["height"], abs=0.002)


def test_share_col_bars_per_column() -> None:
    bars, plots = geometry(grid(ub.Share.COL))
    assert len(bars) == 2
    for col, bar in enumerate(bars):
        assert bar["y1"] > plots[col + 2]["y"] + plots[col + 2]["height"]
        assert bar["x1"] == pytest.approx(plots[col]["x"], abs=0.002)
        assert bar["x2"] - bar["x1"] == pytest.approx(plots[col]["width"], abs=0.002)


@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL, ub.Share.ALL])
def test_plot_areas_stay_aligned_with_colorbars(mode: ub.Share) -> None:
    _, plots = geometry(grid(mode))
    assert len(plots) == 4
    for p in plots:
        assert p["width"] == pytest.approx(plots[0]["width"], abs=0.002)
        assert p["height"] == pytest.approx(plots[0]["height"], abs=0.002)
    assert plots[0]["x"] == plots[2]["x"] and plots[1]["x"] == plots[3]["x"]
    assert plots[0]["y"] == plots[1]["y"] and plots[2]["y"] == plots[3]["y"]


def test_group_range_is_union() -> None:
    fig = grid(ub.Share.ALL)
    before = fig.to_svg()
    fig.axes_at(row=0, col=0).clim(0, 40)
    assert fig.to_svg() == before
    fig.axes_at(row=0, col=0).clim(0, 80)
    assert fig.to_svg() != before


@pytest.mark.parametrize("which", ["cmap", "scale", "center"])
def test_conflicting_member_rejected_atomically(which: str) -> None:
    fig = ub.Figure(cols=2, share_color=ub.Share.ALL)
    colored(fig, 0, 1, [1, 2])
    ax = fig.add_axes(col=0)
    before = fig.to_svg()
    color = (
        ub.ColorBy([1, 2], ub.Cmap.MAGMA)
        if which == "cmap"
        else ub.ColorBy([1, 2], scale=ub.Scale.log())
        if which == "scale"
        else ub.ColorBy([1, 2], center=1)
    )
    with pytest.raises(ub.UsageError) as caught:
        ax.scatter([1, 2], [1, 2], color=color)
    found = details(caught.value)
    assert found is not None and found[0] == "color" and found[1]
    given, existing = {
        "cmap": ("Cmap.MAGMA", "the theme's colormap"),
        "scale": ("Scale.log(10)", "Scale.linear()"),
        "center": ("center 1", "no center"),
    }[which]
    assert str(caught.value) == (
        f"Axes.scatter(): color uses {given}; row 0, col 1 maps colors with {existing}."
    )
    assert fig.to_svg() == before


def test_shared_clim_provenance_and_copy() -> None:
    fig = ub.Figure(cols=2, share_color=ub.Share.ALL)
    a = colored(fig, 0, 1, [1, 2]).clim(0, 10)
    b = colored(fig, 0, 0, [3, 4])
    a.clim(0, 20)
    before = fig.to_svg()
    other = copy.deepcopy(fig)
    other.axes_at(col=1).clim(0, 30)
    assert fig.to_svg() == before and other.to_svg() != before
    with pytest.raises(ub.UsageError, match="shared with row 0, col 1") as caught:
        b.clim(0, 30)
    assert details(caught.value) == (
        "low",
        "set it once for the group, or share colors across fewer panels.",
    )
    assert fig.to_svg() == before
    b.clim(0, 20)
    with pytest.raises(ub.UsageError):
        a.clim(0, 30)
    assert fig.to_svg() == before


def test_shared_clim_validates_other_members_before_mutation() -> None:
    fig = ub.Figure(cols=2, share_color=ub.Share.ALL)
    ax = fig.add_axes(col=0).scatter([1, 2], [1, 2], color=ub.ColorBy([1, 2], scale=ub.Scale.log()))
    empty = fig.add_axes(col=1)
    before = fig.to_svg()
    with pytest.raises(ub.StyleError):
        empty.clim(0, 10)
    assert fig.to_svg() == before
    ax.clim(1, 10)


@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL, ub.Share.ALL])
def test_axes_colorbar_under_sharing_is_atomic(mode: ub.Share) -> None:
    fig = grid(mode)
    before = fig.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        fig.axes_at(row=0, col=0).colorbar("Changed")
    assert str(caught.value) == "Axes.colorbar(): colors are shared across panels."
    assert details(caught.value) == (None, "set the shared colorbar with Figure.colorbar().")
    assert fig.to_svg() == before


def test_figure_colorbar_without_sharing_is_atomic() -> None:
    fig = ub.Figure()
    colored(fig, 0, 0, [1, 2])
    before = fig.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        fig.colorbar()
    assert details(caught.value) == (
        None,
        "set each panel's colorbar with Axes.colorbar(), or share colors.",
    )
    assert fig.to_svg() == before


@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL])
@pytest.mark.parametrize("position", list(ub.ColorbarPosition))
def test_fixed_shared_position_rejects_conflicts(
    mode: ub.Share, position: ub.ColorbarPosition
) -> None:
    fig = grid(mode)
    wanted = ub.ColorbarPosition.RIGHT if mode is ub.Share.ROW else ub.ColorbarPosition.BOTTOM
    before = fig.to_svg()
    if position is wanted:
        fig.colorbar(position=position)
    else:
        with pytest.raises(ub.StyleError) as caught:
            fig.colorbar(position=position)
        found = details(caught.value)
        assert found is not None and found[0] == "position" and found[1]
    assert fig.to_svg() == before


def test_shared_empty_groups_and_theme_position() -> None:
    theme = ub.Theme.PIGMENT.with_colorbar_position(ub.ColorbarPosition.BOTTOM)
    fig = ub.Figure(rows=2, cols=2, share_color=ub.Share.ROW, theme=theme)
    fig.colorbar("Shared")
    colored(fig, 0, 0, [1, 2])
    colored(fig, 1, 1, [float("nan")])
    bars, _ = geometry(fig)
    assert len(bars) == 1 and bars[0]["x1"] == bars[0]["x2"]


def test_figure_colorbar_reset_and_format() -> None:
    fig = grid(ub.Share.ALL)
    before = fig.to_svg()
    fig.colorbar("Intensity", position=ub.ColorbarPosition.BOTTOM, format=ub.TickFormat.fixed(2))
    assert "40.00" in fig.to_svg(text=ub.SvgText.EDITABLE)
    fig.colorbar(position=ub.ColorbarPosition.NONE)
    assert not geometry(fig)[0]
    fig.colorbar()
    assert fig.to_svg() == before


@pytest.mark.parametrize("parameter", ["label", "position", "format"])
def test_figure_colorbar_type_errors_are_atomic(parameter: str) -> None:
    fig = grid(ub.Share.ALL)
    before = fig.to_svg()
    call = cast(Callable[..., object], fig.colorbar)
    with pytest.raises(TypeError) as caught:
        call(**{parameter: None})
    found = details(caught.value)
    assert found is not None and found[0] == parameter
    assert fig.to_svg() == before


@pytest.mark.parametrize("value", [None, "ALL", 1])
def test_share_color_rejects_wrong_types(value: object) -> None:
    call = cast(Callable[..., object], ub.Figure)
    with pytest.raises(TypeError) as caught:
        call(share_color=value)
    found = details(caught.value)
    assert found is not None and found[0] == "share_color"


@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL, ub.Share.ALL])
def test_shared_colorbar_room_minimum_renders(mode: ub.Share) -> None:
    vertical = mode is ub.Share.COL

    def make(size: float) -> ub.Figure:
        fig = grid(mode, width=12 if vertical else size, height=size if vertical else 8)
        return fig.colorbar("Intensity")

    with pytest.raises(ub.StyleError) as caught:
        make(1).to_svg()
    message = str(caught.value)
    assert "colorbar" in message or "columns of plots" in message or "rows of plots" in message
    match = re.search(r"at least (\d+) pt", message)
    assert match is not None, message
    assert "linearGradient" in make(int(match[1]) / 72).to_svg()


@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL])
def test_shared_bar_follows_last_occupied_panel(mode: ub.Share) -> None:
    fig = ub.Figure(rows=2, cols=2, share_color=mode)
    colored(fig, 0, 0, [1, 2])
    colored(fig, 1, 1, [3, 4])
    bars, plots = geometry(fig)
    if mode is ub.Share.ROW:
        assert bars[0]["x1"] < plots[1]["x"]
    else:
        assert bars[0]["y1"] < plots[1]["y"]


@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL, ub.Share.ALL])
@pytest.mark.parametrize("case", ["header", "legend", "direct"])
def test_shared_room_minimum_with_other_contents(mode: ub.Share, case: str) -> None:
    vertical = case == "header"

    def make(size: float) -> ub.Figure:
        fig = ub.Figure(
            12 if vertical else size,
            size if vertical else 8,
            rows=2,
            cols=2,
            share_color=mode,
            legend=ub.Legend.PER_PANEL,
        )
        if vertical:
            fig.title("A title").subtitle("A subtitle")
        for row in range(2):
            for col in range(2):
                ax = colored(fig, row, col, [1, 2, 3])
                if vertical:
                    ax.title("Panel").xlabel("Distance")
                else:
                    ax.line([0, 1], [1, 2], label="A long legend label")
                    if case == "legend":
                        ax.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT)
                    else:
                        ax.legend(ub.LegendStyle.DIRECT)
        return fig.colorbar("Intensity")

    with pytest.raises(ub.StyleError) as caught:
        make(0.9 if vertical else 1.5).to_svg()
    match = re.search(r"at least (\d+) pt", str(caught.value))
    assert match is not None, str(caught.value)
    assert "linearGradient" in make(int(match[1]) / 72).to_svg()


@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL])
@pytest.mark.parametrize("mark", ["scatter", "bubbles"])
def test_shared_color_spans_stay_within_group(mode: ub.Share, mark: str, tmp_path: Path) -> None:
    fig = ub.Figure(rows=2, cols=2, share_color=mode)
    ax = fig.add_axes(
        row=0,
        col=0,
        row_span=2 if mode is ub.Share.ROW else 1,
        col_span=2 if mode is ub.Share.COL else 1,
    ).line([0, 1], [1, 2])
    before = fig.to_svg()
    args = "[0, 1], [1, 2]" + (", [2, 3]" if mark == "bubbles" else "")
    argument = "color=ub.ColorBy([1, 2])"
    line = f"ax.{mark}({args}, {argument})"
    path = tmp_path / "spanning_color_call.py"
    path.write_text(line + "\n")
    with pytest.raises(ub.UsageError) as caught:
        exec(compile(line, str(path), "exec"), {"ax": ax, "ub": ub})
    dimension, group = ("rows", "row") if mode is ub.Share.ROW else ("columns", "column")
    assert str(caught.value) == (
        f"Axes.{mark}(): the panel spans {dimension} that share colors in different groups."
    )
    assert details(caught.value) == (
        "color",
        f"share colors by ALL, or place the color-mapped mark on a panel inside one {group}.",
    )
    marked = f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"
    assert getattr(caught.value, "__notes__", [])[0] == marked
    assert fig.to_svg() == before

    other = ub.Figure(rows=2, cols=2, share_color=mode)
    other.add_axes(
        row=0,
        col=0,
        row_span=2 if mode is ub.Share.COL else 1,
        col_span=2 if mode is ub.Share.ROW else 1,
    ).scatter([0, 1], [1, 2], color=ub.ColorBy([1, 2]))
    assert len(geometry(other)[0]) == 1


@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL])
def test_shared_bar_includes_trailing_unmapped_panel(mode: ub.Share) -> None:
    fig = ub.Figure(rows=2, cols=2, share_color=mode)
    colored(fig, 0, 0, [1, 2])
    fig.add_axes(
        row=0 if mode is ub.Share.ROW else 1, col=1 if mode is ub.Share.ROW else 0
    ).scatter([0, 1], [1, 2])
    bars, plots = geometry(fig)
    if mode is ub.Share.ROW:
        assert bars[0]["x1"] > plots[1]["x"] + plots[1]["width"]
    else:
        assert bars[0]["y1"] > plots[1]["y"] + plots[1]["height"]


def test_nan_first_member_uses_finite_group_range() -> None:
    fig = ub.Figure(cols=2, share_color=ub.Share.ALL)
    colored(fig, 0, 0, [float("nan")])
    colored(fig, 0, 1, [2, 8])
    before = fig.to_svg()
    assert len(geometry(fig)[0]) == 1
    fig.axes_at(col=0).clim(2, 8)
    assert fig.to_svg() == before


def test_bubbles_shared_color_conflict_is_atomic() -> None:
    fig = ub.Figure(cols=2, share_color=ub.Share.ALL)
    colored(fig, 0, 0, [1, 2])
    ax = fig.add_axes(col=1)
    before = fig.to_svg()
    with pytest.raises(ub.UsageError):
        ax.bubbles([0, 1], [1, 2], [2, 3], color=ub.ColorBy([1, 2], ub.Cmap.MAGMA))
    assert fig.to_svg() == before


@pytest.mark.parametrize("argument", ["position=None", "format=None", "label=42"])
def test_figure_colorbar_errors_highlight_argument(tmp_path: Path, argument: str) -> None:
    line = f"fig.colorbar({argument})"
    path = tmp_path / "figure_colorbar_call.py"
    source = "import umber as ub\nfig = ub.Figure(share_color=ub.Share.ALL)\n" + line + "\n"
    path.write_text(source)
    with pytest.raises(TypeError) as caught:
        exec(compile(source, str(path), "exec"), {})
    marked = f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"
    assert getattr(caught.value, "__notes__", [])[0] == marked


def test_shared_colorbar_label_error_preserves_options() -> None:
    fig = grid(ub.Share.ALL).colorbar("Intensity", position=ub.ColorbarPosition.BOTTOM)
    before = fig.to_svg()
    with pytest.raises(ub.StyleError):
        fig.colorbar("x" * 1001, position=ub.ColorbarPosition.NONE)
    assert fig.to_svg() == before


def test_mixed_grid_with_shared_colorbars() -> None:
    fig = ub.Figure(rows=2, cols=2, share_color=ub.Share.COL)
    fig.add_axes(row=0, col=0, y=ub.Scale.category()).bar(["A", "B"], [2, 4], label="Amount").title(
        "Amounts"
    )
    fig.add_axes(row=0, col=1, x=ub.Scale.time()).line(
        [date(2026, 1, 1), date(2026, 2, 1)], [1, 2], label="Signal"
    ).title("Time")
    colored(fig, 1, 0, [0, 5, 10]).title("Intensity A")
    colored(fig, 1, 1, [10, 20, 30]).title("Intensity B")
    fig.colorbar("Intensity")
    bars, plots = geometry(fig)
    assert len(bars) == 2 and len(plots) == 4
    # Line clips extend equally on both sides for end caps; their centers stay aligned.
    for col in range(2):
        top, bottom = plots[col], plots[col + 2]
        assert top["x"] + top["width"] / 2 == pytest.approx(
            bottom["x"] + bottom["width"] / 2, abs=0.002
        )
    assert fig.to_svg() == fig.to_svg()


@pytest.mark.parametrize(
    ("mode", "position"),
    [
        (ub.Share.NONE, ub.ColorbarPosition.RIGHT),
        (ub.Share.ALL, ub.ColorbarPosition.NONE),
        (ub.Share.ALL, ub.ColorbarPosition.RIGHT),
        (ub.Share.ALL, ub.ColorbarPosition.BOTTOM),
        (ub.Share.ROW, ub.ColorbarPosition.RIGHT),
        (ub.Share.COL, ub.ColorbarPosition.BOTTOM),
    ],
)
def test_shared_width_error_names_x_label_and_available_room_renders(
    mode: ub.Share, position: ub.ColorbarPosition
) -> None:
    def make(label: str) -> ub.Figure:
        fig = ub.Figure(5, 4, cols=2, share_color=mode)
        if mode is not ub.Share.NONE:
            fig.colorbar(position=position)
        for col in range(2):
            colored(fig, 0, col, [0, 1]).xticks_labeled([0, 1], [label, label])
        return fig

    label = "W" * 24
    with pytest.raises(ub.StyleError) as caught:
        make(label).to_svg()
    message = str(caught.value)
    assert f'row 0, col 0: x tick label "{label}"' in message
    match = re.search(r"is (\d+) pt wide; at most (\d+) pt fits", message)
    assert match is not None, message
    # The reported label width is rounded; its upper bound gives a safe shorter label.
    count = int(len(label) * int(match[2]) / (int(match[1]) + 0.5))
    assert count > 0
    assert "<svg" in make(label[:count]).to_svg()


@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL])
@pytest.mark.parametrize("trailing", [False, True])
def test_unmapped_spanning_panel_keeps_bars_outside(mode: ub.Share, trailing: bool) -> None:
    fig = ub.Figure(12, 8, rows=2, cols=2, share_color=mode)
    for i in range(2):
        row, col = (i, 0 if trailing else 1) if mode is ub.Share.ROW else (0 if trailing else 1, i)
        colored(fig, row, col, [1, 2])
    row, col = (0, 1 if trailing else 0) if mode is ub.Share.ROW else (1 if trailing else 0, 0)
    fig.add_axes(
        row=row,
        col=col,
        row_span=2 if mode is ub.Share.ROW else 1,
        col_span=2 if mode is ub.Share.COL else 1,
    ).line([0, 1], [1, 2])
    bars, plots = geometry(fig)
    assert len(bars) == 2 and len(plots) == 3
    for bar in bars:
        if mode is ub.Share.ROW:
            assert bar["x1"] > max(p["x"] + p["width"] for p in plots)
        else:
            assert bar["y1"] > max(p["y"] + p["height"] for p in plots)


@pytest.mark.parametrize("mode", list(ub.Share))
def test_theme_none_hides_colorbars_in_every_share_mode(mode: ub.Share) -> None:
    theme = ub.Theme.PIGMENT.with_colorbar_position(ub.ColorbarPosition.NONE)
    fig = ub.Figure(12, 8, rows=2, cols=2, share_color=mode, theme=theme)
    for row in range(2):
        for col in range(2):
            colored(fig, row, col, [1, 2])
    assert not geometry(fig)[0]
    if mode is ub.Share.NONE:
        for row in range(2):
            for col in range(2):
                fig.axes_at(row=row, col=col).colorbar(position=ub.ColorbarPosition.RIGHT)
        assert len(geometry(fig)[0]) == 4
    else:
        position = ub.ColorbarPosition.BOTTOM if mode is ub.Share.COL else ub.ColorbarPosition.RIGHT
        fig.colorbar(position=position)
        assert len(geometry(fig)[0]) == (1 if mode is ub.Share.ALL else 2)
        fig.colorbar()
        assert not geometry(fig)[0]


@pytest.mark.parametrize("mark", CELL_MARKS)
@pytest.mark.parametrize(
    ("mode", "parameter", "value", "given", "existing"),
    [
        (ub.Share.ALL, "cmap", "ub.Cmap.MAGMA", "Cmap.MAGMA", "the theme's colormap"),
        (ub.Share.ROW, "scale", "ub.Scale.log()", "Scale.log(10)", "Scale.linear()"),
    ],
)
def test_shared_cell_conflict_is_atomic(
    mark: str,
    mode: ub.Share,
    parameter: str,
    value: str,
    given: str,
    existing: str,
    tmp_path: Path,
) -> None:
    fig = ub.Figure(12, 8, rows=2, cols=2, share_color=mode)
    first = cell_axes(fig, mark, col=1)
    args = cell_arguments(mark)
    exec(f"ax.{mark}({args})", {"ax": first})
    ax = cell_axes(fig, mark)
    before = fig.to_svg()
    argument = f"{parameter}={value}"
    line = f"ax.{mark}({args}, {argument})"
    path = tmp_path / "shared_cell_call.py"
    path.write_text(line + "\n")
    with pytest.raises(ub.UsageError) as caught:
        exec(compile(line, str(path), "exec"), {"ax": ax, "ub": ub})
    assert str(caught.value) == (
        f"Axes.{mark}(): {parameter} uses {given}; row 0, col 1 maps colors with {existing}."
    )
    assert details(caught.value) == (
        parameter,
        "use the same cmap, scale and center for every color-mapped mark in the shared group.",
    )
    marked = f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"
    assert getattr(caught.value, "__notes__", [])[0] == marked
    assert fig.to_svg() == before
    exec(f"ax.{mark}({args})", {"ax": ax})
    assert len(geometry(fig)[0]) == 1


@pytest.mark.parametrize("mark", CELL_MARKS)
@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL])
def test_shared_cell_spans_are_atomic(mark: str, mode: ub.Share, tmp_path: Path) -> None:
    fig = ub.Figure(12, 8, rows=2, cols=2, share_color=mode)
    ax = cell_axes(
        fig,
        mark,
        row_span=2 if mode is ub.Share.ROW else 1,
        col_span=2 if mode is ub.Share.COL else 1,
    )
    before = fig.to_svg()
    argument = "cmap=ub.Cmap.MAGMA"
    line = f"ax.{mark}({cell_arguments(mark)}, {argument})"
    path = tmp_path / "spanning_cell_call.py"
    path.write_text(line + "\n")
    with pytest.raises(ub.UsageError) as caught:
        exec(compile(line, str(path), "exec"), {"ax": ax, "ub": ub})
    dimension, group = ("rows", "row") if mode is ub.Share.ROW else ("columns", "column")
    assert str(caught.value) == (
        f"Axes.{mark}(): the panel spans {dimension} that share colors in different groups."
    )
    assert details(caught.value) == (
        "cmap",
        f"share colors by ALL, or place the color-mapped mark on a panel inside one {group}.",
    )
    marked = f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"
    assert getattr(caught.value, "__notes__", [])[0] == marked
    assert fig.to_svg() == before

    other = ub.Figure(12, 8, rows=2, cols=2, share_color=mode)
    inside = cell_axes(
        other,
        mark,
        row_span=2 if mode is ub.Share.COL else 1,
        col_span=2 if mode is ub.Share.ROW else 1,
    )
    exec(line, {"ax": inside, "ub": ub})
    assert len(geometry(other)[0]) == 1


@pytest.mark.parametrize("mark", ["heatmap", "mesh"])
def test_shared_cell_center_conflict_is_atomic(mark: str) -> None:
    fig = ub.Figure(cols=2, share_color=ub.Share.ALL)
    colored(fig, 0, 1, [1, 2])
    ax = cell_axes(fig, mark)
    before = fig.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        exec(f"ax.{mark}({cell_arguments(mark)}, center=1)", {"ax": ax})
    assert str(caught.value) == (
        f"Axes.{mark}(): center uses center 1; row 0, col 1 maps colors with no center."
    )
    found = details(caught.value)
    assert found is not None and found[0] == "center"
    assert fig.to_svg() == before


@pytest.mark.parametrize("first", CELL_MARKS)
@pytest.mark.parametrize("later", CELL_MARKS)
def test_shared_cell_kinds_use_one_mapping(first: str, later: str) -> None:
    fig = ub.Figure(12, 4, cols=2, share_color=ub.Share.ALL)
    a = cell_axes(fig, first)
    b = cell_axes(fig, later, col=1)
    exec(f"ax.{first}({cell_arguments(first)})", {"ax": a})
    before = fig.to_svg()
    with pytest.raises(ub.UsageError, match="row 0, col 0 maps colors"):
        exec(f"ax.{later}({cell_arguments(later)}, cmap=ub.Cmap.MAGMA)", {"ax": b, "ub": ub})
    assert fig.to_svg() == before
    exec(f"ax.{later}({cell_arguments(later)})", {"ax": b})
    assert len(geometry(fig)[0]) == 1


@pytest.mark.parametrize("mark", CELL_MARKS)
@pytest.mark.parametrize("mode", [ub.Share.NONE, ub.Share.ROW, ub.Share.COL])
def test_separate_cell_groups_allow_different_mappings(mark: str, mode: ub.Share) -> None:
    fig = ub.Figure(12, 8, rows=2, cols=2, share_color=mode)
    a = cell_axes(fig, mark)
    b = cell_axes(fig, mark, row=1, col=1)
    exec(f"ax.{mark}({cell_arguments(mark)})", {"ax": a})
    exec(f"ax.{mark}({cell_arguments(mark)}, cmap=ub.Cmap.MAGMA)", {"ax": b, "ub": ub})
    assert len(geometry(fig)[0]) == 2


@pytest.mark.parametrize("centered", [False, True])
def test_shared_heatmap_clim_validates_all_members_atomically(centered: bool) -> None:
    fig = ub.Figure(cols=2, share_color=ub.Share.ALL)
    heatmap = fig.add_axes(x=ub.Scale.category(), y=ub.Scale.category(), col=1)
    if centered:
        heatmap.heatmap(["a", "b"], ["r"], [[1, 2]], center=1)
    else:
        heatmap.heatmap(["a", "b"], ["r"], [[1, 2]], scale=ub.Scale.log())
    empty = fig.add_axes(col=0)
    before = fig.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        empty.clim(2 if centered else 0, 10)
    assert "row 0, col 1:" in str(caught.value)
    found = details(caught.value)
    assert found is not None and found[0] == "low" and found[1]
    assert fig.to_svg() == before
    empty.clim(1, 10)
    before = fig.to_svg()
    with pytest.raises(ub.UsageError, match="shared with row 0, col 0"):
        heatmap.clim(1, 20)
    assert fig.to_svg() == before
    heatmap.clim(1, 10)
    assert fig.to_svg() == before


@pytest.mark.parametrize("new_panel", [False, True])
@pytest.mark.parametrize("centered", [False, True])
def test_heatmap_checks_inherited_shared_clim(new_panel: bool, centered: bool) -> None:
    fig = ub.Figure(cols=2, share_color=ub.Share.ALL)
    limits = fig.add_axes(col=0)
    if new_panel:
        limits.clim(2 if centered else 0, 10)
    ax = fig.add_axes(x=ub.Scale.category(), y=ub.Scale.category(), col=1)
    if not new_panel:
        limits.clim(2 if centered else 0, 10)
    before = fig.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        if centered:
            ax.heatmap(["a", "b"], ["r"], [[1, 2]], center=1)
        else:
            ax.heatmap(["a", "b"], ["r"], [[1, 2]], scale=ub.Scale.log())
    found = details(caught.value)
    assert found is not None and found[0] == ("center" if centered else "scale") and found[1]
    assert fig.to_svg() == before
    limits.clim(1, 10)
    if centered:
        ax.heatmap(["a", "b"], ["r"], [[1, 2]], center=1)
    else:
        ax.heatmap(["a", "b"], ["r"], [[1, 2]], scale=ub.Scale.log())
    assert len(geometry(fig)[0]) == 1


@pytest.mark.parametrize(
    ("mode", "position"),
    [
        (ub.Share.ALL, ub.ColorbarPosition.RIGHT),
        (ub.Share.ALL, ub.ColorbarPosition.BOTTOM),
        (ub.Share.ROW, ub.ColorbarPosition.RIGHT),
        (ub.Share.COL, ub.ColorbarPosition.BOTTOM),
    ],
)
def test_shared_plain_width_message_minimum_renders(
    mode: ub.Share, position: ub.ColorbarPosition
) -> None:
    def make(width: float) -> ub.Figure:
        return grid(mode, width=width).colorbar(position=position)

    with pytest.raises(ub.StyleError) as caught:
        make(0.25).to_svg()
    message = str(caught.value)
    match = re.fullmatch(
        r"Figure.to_svg\(\): the figure is 18 pt wide; at least (\d+) pt is needed for 2 columns "
        r"of plots\.",
        message,
    )
    assert match is not None, message
    assert "linearGradient" in make(int(match[1]) / 72).to_svg()


@pytest.mark.parametrize("mode", [ub.Share.ROW, ub.Share.COL, ub.Share.ALL])
@pytest.mark.parametrize("position", [ub.ColorbarPosition.RIGHT, ub.ColorbarPosition.BOTTOM])
def test_theme_visible_side_only_selects_all_colorbar_side(
    mode: ub.Share, position: ub.ColorbarPosition
) -> None:
    theme = ub.Theme.PIGMENT.with_colorbar_position(position)
    fig = ub.Figure(12, 8, rows=2, cols=2, share_color=mode, theme=theme)
    colored(fig, 0, 0, [1, 2])
    bars, _ = geometry(fig)
    assert len(bars) == 1
    vertical = mode is ub.Share.ROW or (
        mode is ub.Share.ALL and position is ub.ColorbarPosition.RIGHT
    )
    assert (bars[0]["x1"] == bars[0]["x2"]) is vertical


@pytest.mark.parametrize(("low", "high", "parameter"), [(2, 4, "low"), (-2, 0, "high")])
def test_shared_clim_names_panel_with_center(low: float, high: float, parameter: str) -> None:
    fig = ub.Figure(cols=2, share_color=ub.Share.ALL)
    fig.add_axes(col=1).scatter([0, 1], [1, 2], color=ub.ColorBy([1, 2], center=1))
    ax = fig.add_axes(col=0)
    before = fig.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        ax.clim(low, high)
    assert str(caught.value) == (
        "Axes.clim(): row 0, col 1: center is 1; "
        f"it must lie inside the color range {low} to {high}."
    )
    assert details(caught.value) == (
        parameter,
        "set color limits that hold center, or choose another center.",
    )
    assert fig.to_svg() == before
