"""Colorbar placement, options, errors and immutable theme defaults."""

from collections.abc import Callable
from pathlib import Path
from typing import cast
from xml.etree import ElementTree

import pytest

import umber as ub
from umber._errors import details


def axes(theme: ub.Theme = ub.Theme.PIGMENT) -> ub.Axes[float, float]:
    return (
        ub.Figure(6, 4, theme=theme)
        .add_axes()
        .scatter([1, 2, 3], [1, 2, 3], color=ub.ColorBy([0, 0.5, 1]))
    )


def test_colorbar_default_and_none() -> None:
    ax = axes()
    assert "linearGradient" in ax.figure.to_svg()
    assert ax.colorbar(position=ub.ColorbarPosition.NONE) is ax
    assert "linearGradient" not in ax.figure.to_svg()
    ax.colorbar()
    assert "linearGradient" in ax.figure.to_svg()


def test_colorbar_without_mark_rejected() -> None:
    ax = ub.Figure().add_axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.colorbar("Intensity")
    assert str(caught.value) == "Axes.colorbar(): this axes has no color-mapped mark."
    assert ax.figure.to_svg() == before
    assert any("add a heatmap, mesh, hist2d, hexbin" in note for note in caught.value.__notes__)


def test_colorbar_theme_and_override_reset() -> None:
    base = ub.Theme.PIGMENT
    bottom = base.with_colorbar_position(ub.ColorbarPosition.BOTTOM)
    ax = axes(bottom)
    expected = ax.figure.to_svg()
    assert expected != axes(base).figure.to_svg()
    assert expected == axes(base).colorbar(position=ub.ColorbarPosition.BOTTOM).figure.to_svg()
    ax.colorbar(position=ub.ColorbarPosition.NONE)
    ax.colorbar()
    assert ax.figure.to_svg() == expected


def test_colorbar_format_follows_y_and_can_override() -> None:
    ax = axes().ytick_format(ub.TickFormat.percent())
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert svg.count("100%") >= 2
    ax.colorbar("Intensity", format=ub.TickFormat.fixed(2).affix(suffix=" units"))
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert "Intensity" in svg and "1.00 units" in svg
    ax.colorbar()
    assert ax.figure.to_svg(text=ub.SvgText.EDITABLE).count("100%") >= 2


@pytest.mark.parametrize("parameter", ["label", "position", "format"])
@pytest.mark.parametrize("value", [None, 42, "invalid"])
def test_colorbar_wrong_types_are_atomic(parameter: str, value: object) -> None:
    if parameter == "label" and isinstance(value, str):
        return
    ax = axes()
    before = ax.figure.to_svg()
    call = cast(Callable[..., object], ax.colorbar)
    with pytest.raises(TypeError) as caught:
        call(**{parameter: value})
    found = details(caught.value)
    assert found is not None and found[0] == parameter
    assert ax.figure.to_svg() == before


def test_colorbar_all_nan_is_absent() -> None:
    ax = ub.Figure().add_axes().scatter([1], [1], color=ub.ColorBy([float("nan")]))
    ax.colorbar("No values")
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert "linearGradient" not in svg and "No values" not in svg


@pytest.mark.parametrize("argument", ["position=None", "format=None", "label=42"])
def test_colorbar_errors_highlight_the_argument(tmp_path: Path, argument: str) -> None:
    line = f"ax.colorbar({argument})"
    path = tmp_path / "colorbar_call.py"
    source = (
        "import umber as ub\n"
        "ax = ub.Figure().add_axes().scatter([1], [1], color=ub.ColorBy([1]))\n"
        f"{line}\n"
    )
    path.write_text(source)
    with pytest.raises(TypeError) as caught:
        exec(compile(source, str(path), "exec"), {})
    marked = f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"
    assert getattr(caught.value, "__notes__", [])[0] == marked


@pytest.mark.parametrize("value", [None, "RIGHT", 0, ub.LegendPosition.RIGHT])
def test_theme_colorbar_position_rejects_other_types(value: object) -> None:
    call = cast(Callable[..., object], ub.Theme.PIGMENT.with_colorbar_position)
    with pytest.raises(TypeError) as caught:
        call(value)
    found = details(caught.value)
    assert found is not None and found[0] == "position"


def test_colorbar_label_failure_keeps_previous_settings() -> None:
    ax = axes().colorbar("Intensity", position=ub.ColorbarPosition.BOTTOM)
    before = ax.figure.to_svg()
    with pytest.raises(ub.StyleError) as caught:
        ax.colorbar("x" * 1001, position=ub.ColorbarPosition.NONE)
    found = details(caught.value)
    assert found is not None and found[0] == "label"
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("position", "width", "height", "message", "hint"),
    [
        (
            ub.ColorbarPosition.RIGHT,
            1,
            4,
            "Figure.to_svg(): the colorbar needs 44 pt; the panel has 28 pt.",
            "shorten the label or widen the figure.",
        ),
        (
            ub.ColorbarPosition.BOTTOM,
            6,
            1,
            "Figure.to_svg(): the colorbar needs 52 pt; the panel has 11 pt.",
            "increase the figure height.",
        ),
    ],
)
def test_colorbar_room_errors_exact(
    position: ub.ColorbarPosition, width: float, height: float, message: str, hint: str
) -> None:
    ax = (
        ub.Figure(width, height)
        .add_axes()
        .scatter([1, 2, 3], [1, 2, 3], color=ub.ColorBy([-2, 0, 8], center=0.3))
    )
    ax.xlabel("Distance").colorbar("Intensity", position=position)
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert str(caught.value) == message
    assert details(caught.value) == (None, hint)


@pytest.mark.parametrize("position", [ub.ColorbarPosition.RIGHT, ub.ColorbarPosition.BOTTOM])
@pytest.mark.parametrize("case", ["single_height", "grid_height", "single_legend", "grid_legend"])
def test_colorbar_preserves_unrelated_room_errors(case: str, position: ub.ColorbarPosition) -> None:
    def render(colored: bool) -> None:
        grid = case.startswith("grid")
        if case == "single_height":
            fig = ub.Figure(6, 0.9).title("A title").subtitle("A subtitle")
        elif case == "grid_height":
            fig = ub.Figure(6, 1.6, rows=2, cols=2).title("A title")
        elif grid:
            fig = ub.Figure(2, 3, rows=1, cols=2, legend=ub.Legend.PER_PANEL)
        else:
            fig = ub.Figure(1.2, 3)
        count = 4 if case == "grid_height" else 2 if grid else 1
        for i in range(count):
            ax = fig.add_axes(row=i // 2, col=i % 2) if grid else fig.add_axes()
            if "height" in case:
                ax.title("Panel")
            if not grid:
                ax.ylabel("Height")
            if colored and i == 0:
                ax.scatter([1, 2, 3, 4], [1, 2, 3, 4], color=ub.ColorBy([-2, 0, 3, 8]))
                ax.colorbar(position=position)
            else:
                ax.scatter([1, 2, 3, 4], [1, 2, 3, 4])
            if "legend" in case:
                ax.line([1, 2], [1, 2], label="A long legend label").legend(
                    ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT
                )
        fig.to_svg()

    with pytest.raises(ub.StyleError) as plain:
        render(False)
    with pytest.raises(ub.StyleError) as mapped:
        render(True)
    assert str(mapped.value) == str(plain.value)
    assert details(mapped.value) == details(plain.value)
    if case == "single_height":
        assert str(mapped.value) == (
            "Figure.to_svg(): the figure is 64 pt tall; "
            "at least 135 pt is needed for the plot area."
        )
    elif case == "grid_height":
        assert str(mapped.value) == (
            "Figure.to_svg(): the figure is 115 pt tall; "
            "at least 182 pt is needed for 2 rows of plots."
        )


@pytest.mark.parametrize(
    ("rows", "cols", "height", "position", "needed", "available"),
    [
        (1, 1, 0.7, ub.ColorbarPosition.RIGHT, 13, 4),
        (1, 1, 0.7, ub.ColorbarPosition.BOTTOM, 52, 4),
        (2, 2, 1.35, ub.ColorbarPosition.RIGHT, 13, 5),
        (2, 2, 1.5, ub.ColorbarPosition.BOTTOM, 52, 16),
    ],
)
def test_colorbar_height_error_reports_its_own_band(
    rows: int, cols: int, height: float, position: ub.ColorbarPosition, needed: int, available: int
) -> None:
    fig = ub.Figure(6, height, rows=rows, cols=cols)
    for i in range(rows * cols):
        ax = fig.add_axes(row=i // cols, col=i % cols)
        if i in (0, rows * cols - 1):
            ax.scatter([1, 2, 3], [1, 2, 3], color=ub.ColorBy([-2, 0, 8]))
        else:
            ax.scatter([1, 2, 3], [1, 2, 3])
    ax = fig.axes_at(row=rows - 1, col=cols - 1)
    ax.colorbar("Intensity", position=position)
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    panel = f"row {rows - 1}, col {cols - 1}: " if rows * cols > 1 else ""
    assert str(caught.value) == (
        f"Figure.to_svg(): {panel}the colorbar needs {needed} pt; the panel has {available} pt."
    )
    assert details(caught.value) == (None, "increase the figure height.")
    ax.colorbar(position=ub.ColorbarPosition.NONE)
    fig.to_svg()


@pytest.mark.parametrize(
    ("rows", "cols", "width", "height", "position", "needed", "available"),
    [
        (1, 2, 6, 0.8, ub.ColorbarPosition.BOTTOM, 52, 11),
        (2, 1, 1, 6, ub.ColorbarPosition.RIGHT, 44, 28),
    ],
)
def test_colorbars_sharing_a_grid_band_report_its_size(
    rows: int,
    cols: int,
    width: float,
    height: float,
    position: ub.ColorbarPosition,
    needed: int,
    available: int,
) -> None:
    fig = ub.Figure(width, height, rows=rows, cols=cols)
    for i in range(rows * cols):
        fig.add_axes(row=i // cols, col=i % cols).scatter(
            [1, 2, 3], [1, 2, 3], color=ub.ColorBy([-2, 0, 8])
        ).colorbar("Intensity", position=position)
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    assert str(caught.value) == (
        f"Figure.to_svg(): row 0, col 0: the colorbar needs {needed} pt; "
        f"the panel has {available} pt."
    )
    for i in range(rows * cols):
        fig.axes_at(row=i // cols, col=i % cols).colorbar(position=ub.ColorbarPosition.NONE)
    fig.to_svg()


def test_colorbar_single_value_has_one_tick() -> None:
    ax = ub.Figure().add_axes().scatter([1], [1], color=ub.ColorBy([17]))
    doc = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    assert [node.text for node in doc.iter() if node.tag.endswith("}text")].count("17") == 1
