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
    assert "shared" in str(caught.value)
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
    with pytest.raises(ub.UsageError, match="shared with row 0, col 1"):
        b.clim(0, 30)
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
def test_shared_color_spans_stay_within_group(mode: ub.Share) -> None:
    fig = ub.Figure(rows=2, cols=2, share_color=mode)
    before = fig.to_svg()
    with pytest.raises(ub.UsageError):
        fig.add_axes(
            row=0,
            col=0,
            row_span=2 if mode is ub.Share.ROW else 1,
            col_span=2 if mode is ub.Share.COL else 1,
        )
    assert fig.to_svg() == before
    ax = fig.add_axes(
        row=0,
        col=0,
        row_span=2 if mode is ub.Share.COL else 1,
        col_span=2 if mode is ub.Share.ROW else 1,
    )
    ax.scatter([0, 1], [1, 2], color=ub.ColorBy([1, 2]))
    assert len(geometry(fig)[0]) == 1


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
