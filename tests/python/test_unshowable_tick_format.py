"""Automatic fixed-decimal ticks fail at render rather than silently disappearing."""

from pathlib import Path

import pytest

import umber as ub
from umber._errors import details

FORMATS = [ub.TickFormat.fixed(0), ub.TickFormat.fixed(1), ub.TickFormat.percent(0)]


def figure(axis: str, fmt: ub.TickFormat, low: float, high: float) -> ub.Figure:
    fig = ub.Figure(6, 4)
    ax = fig.add_axes()
    if axis == "x":
        ax.line([low, high], [1, 2]).xlim(low, high).xtick_format(fmt)
    elif axis == "y":
        ax.line([1, 2], [low, high]).ylim(low, high).ytick_format(fmt)
    else:
        ax.scatter([1, 2], [1, 2], color=ub.ColorBy([low, high])).colorbar(format=fmt)
    return fig


@pytest.mark.parametrize("axis", ["x", "y", "colorbar"])
@pytest.mark.parametrize("fmt", FORMATS)
@pytest.mark.parametrize("render", ["to_svg", "save_png"])
def test_unshowable_tick_format_fails_at_render(
    axis: str, fmt: ub.TickFormat, render: str, tmp_path: Path
) -> None:
    fig = figure(axis, fmt, 0.005, 0.009)
    path = tmp_path / "unshowable.png"
    with pytest.raises(ub.StyleError) as caught:
        if render == "to_svg":
            fig.to_svg()
        else:
            fig.save_png(path)
    name = repr(fmt).removeprefix("TickFormat.")
    target = "colorbar" if axis == "colorbar" else f"{axis} axis"
    assert str(caught.value) == (
        f"Figure.{render}(): the {target} runs from 0.005 to 0.009, "
        f"but its tick format {name} cannot label any value in that range."
    )
    constructor = "percent" if name.startswith("percent") else "fixed"
    ending = "." if axis == "colorbar" else ", or set the ticks explicitly."
    hint = f"use more decimals with TickFormat.{constructor}(){ending}"
    assert details(caught.value) == (None, hint)
    assert f"hint: {hint}" in caught.value.__notes__
    assert not path.exists()


@pytest.mark.parametrize("axis", ["x", "y"])
@pytest.mark.parametrize("fmt", FORMATS)
@pytest.mark.parametrize("positions", [[0.007], []])
def test_explicit_ticks_bypass_unshowable_format(
    axis: str, fmt: ub.TickFormat, positions: list[float]
) -> None:
    fig = figure(axis, fmt, 0.005, 0.009)
    ax = fig.axes_at()
    if axis == "x":
        ax.xticks(positions)
    else:
        ax.yticks(positions)
    assert "<svg" in fig.to_svg()


@pytest.mark.parametrize("axis", ["x", "y", "colorbar"])
@pytest.mark.parametrize("fmt", FORMATS)
@pytest.mark.parametrize("center", [-1.0, 0.0, 1.0])
def test_one_representable_value_keeps_one_tick(
    axis: str, fmt: ub.TickFormat, center: float
) -> None:
    fig = figure(axis, fmt.affix(prefix="only:"), center - 0.004, center + 0.004)
    assert fig.to_svg(text=ub.SvgText.EDITABLE).count(">only:") == 1


@pytest.mark.parametrize("axis", ["x", "y"])
@pytest.mark.parametrize("fmt", FORMATS)
def test_log_axes_keep_fixed_decimal_ticks(axis: str, fmt: ub.TickFormat) -> None:
    fig = ub.Figure(6, 4)
    ax = fig.add_axes(x=ub.Scale.log(), y=ub.Scale.log()).line([0.005, 0.009], [0.005, 0.009])
    if axis == "x":
        ax.xlim(0.005, 0.009).xtick_format(fmt)
    else:
        ax.ylim(0.005, 0.009).ytick_format(fmt)
    assert "<svg" in fig.to_svg()


def test_unshowable_format_names_affixes_and_tiny_domain() -> None:
    fig = figure("x", ub.TickFormat.fixed(1).affix("$", " USD"), 5e-9, 9e-9)
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    assert str(caught.value) == (
        "Figure.to_svg(): the x axis runs from 5e-9 to 9e-9, "
        'but its tick format fixed(1).affix("$", " USD") '
        "cannot label any value in that range."
    )


@pytest.mark.parametrize("axis", ["x", "y"])
def test_unshowable_format_on_automatic_data_limits(axis: str) -> None:
    fig = ub.Figure(6, 4)
    ax = fig.add_axes().line([0.005, 0.009], [0.005, 0.009])
    if axis == "x":
        ax.xtick_format(ub.TickFormat.fixed(0))
    else:
        ax.ytick_format(ub.TickFormat.fixed(0))
    with pytest.raises(ub.StyleError, match=f"the {axis} axis runs from"):
        fig.to_svg()


@pytest.mark.parametrize("fmt", FORMATS)
@pytest.mark.parametrize("shared", [False, True])
def test_unshowable_inherited_colorbar_names_its_own_setting(
    fmt: ub.TickFormat, shared: bool
) -> None:
    fig = ub.Figure(
        12 if shared else 6,
        4,
        cols=3 if shared else 1,
        share_color=ub.Share.ALL if shared else ub.Share.NONE,
    )
    # The first color-mapped panel is neither the first inserted nor the first cell.
    for col in [2, 0, 1] if shared else [0]:
        ax = fig.add_axes(col=col)
        if shared and col == 0:
            ax.line([1, 2, 3], [0, 500, 1000]).ytick_format(ub.TickFormat.fixed(1))
        else:
            ax.scatter([1, 2, 3], [0, 500, 1000], color=ub.ColorBy([0.005, 0.007, 0.009]))
            ax.ytick_format(ub.TickFormat.fixed(3) if shared and col == 2 else fmt)
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    target = "shared colorbar" if shared else "colorbar"
    cell = "row 0, col 1: " if shared else ""
    name = repr(fmt).removeprefix("TickFormat.")
    assert str(caught.value) == (
        f"Figure.to_svg(): {cell}the {target} runs from 0.005 to 0.009, "
        f"but its tick format {name}, taken from the y tick format, "
        "cannot label any value in that range."
    )
    owner = "fig" if shared else "ax"
    constructor = "percent" if name.startswith("percent") else "fixed"
    hint = (
        f"set the colorbar's own format with {owner}.colorbar(format=...), "
        f"using more decimals with TickFormat.{constructor}()."
    )
    assert details(caught.value) == (None, hint)
    assert f"hint: {hint}" in caught.value.__notes__
    if shared:
        fig.colorbar(format=ub.TickFormat.fixed(3))
    else:
        fig.axes_at().colorbar(format=ub.TickFormat.fixed(3))
    assert "<svg" in fig.to_svg()


@pytest.mark.parametrize("shared", [False, True])
@pytest.mark.parametrize("inherited", [False, True])
def test_hidden_colorbar_does_not_fail(shared: bool, inherited: bool) -> None:
    fig = ub.Figure(8, 4, cols=2, share_color=ub.Share.ALL if shared else ub.Share.NONE)
    for col in range(2):
        ax = fig.add_axes(col=col).scatter([1, 2], [1, 2], color=ub.ColorBy([0.005, 0.009]))
        ax.ytick_format(ub.TickFormat.fixed(0))
        if not shared:
            ax.colorbar(
                position=ub.ColorbarPosition.NONE,
                format=ub.AUTO if inherited else ub.TickFormat.fixed(0),
            )
    if shared:
        fig.colorbar(
            position=ub.ColorbarPosition.NONE,
            format=ub.AUTO if inherited else ub.TickFormat.fixed(0),
        )
    assert "<svg" in fig.to_svg()


@pytest.mark.parametrize("fmt", FORMATS)
def test_unshowable_shared_colorbar_names_its_cell(fmt: ub.TickFormat) -> None:
    fig = ub.Figure(8, 4, cols=2, share_color=ub.Share.ALL)
    for col in range(2):
        fig.add_axes(col=col).scatter([1, 2], [1, 2], color=ub.ColorBy([0.005, 0.009]))
    fig.colorbar(format=fmt)
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    name = repr(fmt).removeprefix("TickFormat.")
    assert str(caught.value) == (
        "Figure.to_svg(): row 0, col 0: the shared colorbar runs from 0.005 to 0.009, "
        f"but its tick format {name} cannot label any value in that range."
    )
    constructor = "percent" if name.startswith("percent") else "fixed"
    hint = f"use more decimals with TickFormat.{constructor}()."
    assert details(caught.value) == (None, hint)
    assert f"hint: {hint}" in caught.value.__notes__


@pytest.mark.parametrize("axis", ["x", "y"])
@pytest.mark.parametrize(
    ("low", "high", "bounds"),
    [
        (0.2, 0.8, "0.17 to 0.83"),
        (0.001, 0.009, "0.0006 to 0.0094"),
        (2.2, 2.8, "2.17 to 2.83"),
        (-0.8, -0.2, "-0.83 to -0.17"),
    ],
)
def test_unshowable_format_prints_rounded_automatic_bounds(
    axis: str, low: float, high: float, bounds: str
) -> None:
    fig = ub.Figure(6, 4)
    ax = fig.add_axes()
    if axis == "x":
        ax.line([low, high], [1, 2]).xtick_format(ub.TickFormat.fixed(0))
        bounds = f"{low} to {high}"
    else:
        ax.line([1, 2], [low, high]).ytick_format(ub.TickFormat.fixed(0))
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    assert str(caught.value) == (
        f"Figure.to_svg(): the {axis} axis runs from {bounds}, "
        "but its tick format fixed(0) cannot label any value in that range."
    )


def test_unshowable_format_bounds_have_at_most_six_significant_digits() -> None:
    fig = figure("y", ub.TickFormat.fixed(0), 1.23456789, 1.87654321)
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    assert str(caught.value) == (
        "Figure.to_svg(): the y axis runs from 1.23457 to 1.87654, "
        "but its tick format fixed(0) cannot label any value in that range."
    )


@pytest.mark.parametrize("fmt", FORMATS)
def test_log_colorbar_keeps_fixed_decimal_ticks(fmt: ub.TickFormat) -> None:
    fig = ub.Figure(6, 4)
    fig.add_axes().scatter(
        [1, 2], [1, 2], color=ub.ColorBy([0.005, 0.009], scale=ub.Scale.log())
    ).colorbar(format=fmt)
    assert "<svg" in fig.to_svg()


@pytest.mark.parametrize("axis", ["x", "y"])
def test_explicit_labeled_ticks_still_render(axis: str) -> None:
    fig = figure(axis, ub.TickFormat.AUTO, 0.005, 0.009)
    ax = fig.axes_at()
    if axis == "x":
        ax.xticks_labeled([0.007], ["exact"])
    else:
        ax.yticks_labeled([0.007], ["exact"])
    assert ">exact</text>" in fig.to_svg(text=ub.SvgText.EDITABLE)
