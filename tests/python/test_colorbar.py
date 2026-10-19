"""Colorbar placement, options, errors and immutable theme defaults."""

import re
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
            "Figure.to_svg(): the figure is 72 pt wide; "
            "at least 120 pt is needed for the plot area.",
            "widen the figure.",
        ),
        (
            ub.ColorbarPosition.BOTTOM,
            6,
            1,
            "Figure.to_svg(): the colorbar needs 53 pt; the panel has 11 pt.",
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
    expected = str(plain.value)
    hint = details(plain.value)
    if case == "single_height":
        assert expected == (
            "Figure.to_svg(): the figure is 64 pt tall; "
            "at least 135 pt is needed for the plot area."
        )
        if position == ub.ColorbarPosition.BOTTOM:
            expected = expected.replace("135 pt", "172 pt")
    elif case == "grid_height":
        assert expected == (
            "Figure.to_svg(): the figure is 115 pt tall; "
            "at least 182 pt is needed for 2 rows of plots."
        )
        if position == ub.ColorbarPosition.BOTTOM:
            expected = expected.replace("182 pt", "219 pt")
    elif position == ub.ColorbarPosition.RIGHT:
        if case == "grid_legend":
            assert expected == (
                "Figure.to_svg(): the figure is 144 pt wide; "
                "at least 290 pt is needed for 2 columns of plots."
            )
            expected = expected.replace("290 pt", "326 pt")
        else:
            assert expected == (
                'Figure.to_svg(): the legend label "A long legend label" is 73 pt wide; '
                "the panel has 1 pt."
            )
            expected = (
                "Figure.to_svg(): the figure is 86 pt wide; "
                "at least 194 pt is needed for the plot area."
            )
            hint = (None, "widen the figure.")
    assert str(mapped.value) == expected
    assert details(mapped.value) == hint


@pytest.mark.parametrize(
    ("case", "side", "minimum", "area"),
    [
        ("single_height", 0.9, 190, "the plot area"),
        ("single_legend", 2.0, 267, "the plot area"),
        ("single_narrow_legend", 1.2, 194, "the plot area"),
        ("grid_height", 1.6, 219, "2 rows of plots"),
        ("grid_width", 1.5, 193, "3 columns of plots"),
        ("grid_legend", 2.0, 326, "2 columns of plots"),
    ],
)
def test_colorbar_room_error_minimum_renders(
    case: str, side: float, minimum: int, area: str
) -> None:
    def render(size: float) -> str:
        if case == "single_height":
            fig = ub.Figure(6, size).title("A title").subtitle("A subtitle")
            ax = fig.add_axes().title("Panel").xlabel("Distance")
            ax.scatter([1, 2, 3, 4], [1, 2, 3, 4], color=ub.ColorBy([-2, 0, 3, 8]))
            ax.colorbar("Intensity", position=ub.ColorbarPosition.BOTTOM)
        elif case in ("single_legend", "single_narrow_legend"):
            fig = ub.Figure(size, 3)
            ax = fig.add_axes().ylabel("Height")
            ax.scatter([1, 2, 3, 4], [1, 2, 3, 4], color=ub.ColorBy([-2, 0, 3, 8]))
            ax.colorbar("A fairly long colorbar label" if case == "single_legend" else "")
            ax.line([1, 2], [1, 2], label="A long legend label").legend(
                ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT
            )
        elif case == "grid_height":
            fig = ub.Figure(6, size, rows=2, cols=2).title("A title")
            for i in range(4):
                ax = fig.add_axes(row=i // 2, col=i % 2).title("Panel")
                if i == 0:
                    ax.scatter([1, 2, 3, 4], [1, 2, 3, 4], color=ub.ColorBy([-2, 0, 3, 8]))
                    ax.colorbar(position=ub.ColorbarPosition.BOTTOM)
                else:
                    ax.scatter([1, 2], [1, 2])
        elif case == "grid_legend":
            fig = ub.Figure(size, 3, rows=1, cols=2, legend=ub.Legend.PER_PANEL)
            for i in range(2):
                ax = fig.add_axes(row=0, col=i)
                if i == 0:
                    ax.scatter([1, 2, 3, 4], [1, 2, 3, 4], color=ub.ColorBy([-2, 0, 3, 8]))
                else:
                    ax.scatter([1, 2, 3, 4], [1, 2, 3, 4])
                ax.line([1, 2], [1, 2], label="A long legend label").legend(
                    ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT
                )
        else:
            fig = ub.Figure(size, 4, rows=1, cols=3)
            for i in range(3):
                ax = fig.add_axes(row=0, col=i)
                if i == 1:
                    ax.scatter([1, 2], [1, 2], color=ub.ColorBy([-200000, 800000]))
                    ax.colorbar("Intensity")
                else:
                    ax.scatter([1, 2], [1, 2])
        return fig.to_svg()

    with pytest.raises(ub.StyleError) as caught:
        render(side)
    direction = "tall" if "height" in case else "wide"
    assert str(caught.value) == (
        f"Figure.to_svg(): the figure is {int(side * 72)} pt {direction}; "
        f"at least {minimum} pt is needed for {area}."
    )
    assert "linearGradient" in render(minimum / 72)


@pytest.mark.parametrize(
    ("cols", "width", "colored", "available"),
    [(1, 2.0, True, 35), (2, 4.0, True, 35), (2, 4.0, False, 71)],
)
def test_right_legend_reported_difference_renders(
    cols: int, width: float, colored: bool, available: int
) -> None:
    def render(size: float) -> str:
        fig = ub.Figure(size, 3, rows=1, cols=cols, legend=ub.Legend.PER_PANEL)
        for col in range(cols):
            ax = fig.add_axes(row=0, col=col)
            if colored and col == 0:
                ax.scatter([1, 2, 3, 4], [1, 2, 3, 4], color=ub.ColorBy([-2, 0, 3, 8]))
            else:
                ax.scatter([1, 2, 3, 4], [1, 2, 3, 4])
            ax.line([1, 2], [1, 2], label="A long legend label").legend(
                ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT
            )
        return fig.to_svg()

    with pytest.raises(ub.StyleError) as caught:
        render(width)
    message = str(caught.value)
    needed, room = map(int, re.findall(r"(-?\d+) pt", message))
    assert render(width + (needed - room) / 72).startswith("<svg")
    panel = "row 0, col 0: " if cols > 1 else ""
    assert message == (
        f'Figure.to_svg(): {panel}the legend label "A long legend label" is 73 pt wide; '
        f"the panel has {available} pt."
    )
    assert details(caught.value) == (None, "shorten the label or widen the figure.")


@pytest.mark.parametrize(
    ("rows", "cols", "height", "position", "needed", "available"),
    [
        (1, 1, 0.7, ub.ColorbarPosition.RIGHT, 13, 4),
        (1, 1, 0.7, ub.ColorbarPosition.BOTTOM, 53, 4),
        (2, 2, 1.35, ub.ColorbarPosition.RIGHT, 13, 5),
        (2, 2, 1.5, ub.ColorbarPosition.BOTTOM, 53, 16),
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
        (1, 2, 6, 0.8, ub.ColorbarPosition.BOTTOM, 53, 11),
        (2, 1, 1, 6, ub.ColorbarPosition.RIGHT, 45, 28),
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


@pytest.mark.parametrize("axis", ["x", "y"])
def test_colorbar_minimum_width_includes_axis_label(axis: str) -> None:
    def render(width: float) -> str:
        fig = ub.Figure(width, 3)
        ax = fig.add_axes().xticks([]).yticks([])
        setter = ax.xlabel if axis == "x" else ax.ylabel
        setter("Distance along the river")
        ax.scatter([1, 2, 3, 4], [1, 2, 3, 4], color=ub.ColorBy([-2, 0, 3, 8]))
        ax.colorbar("A fairly long colorbar label")
        ax.line([1, 2], [1, 2], label="A long legend label").legend(
            ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT
        )
        return fig.to_svg()

    with pytest.raises(ub.StyleError) as caught:
        render(2.0)
    message = str(caught.value)
    minimum = int(message.split("at least ")[1].split(" pt")[0])
    assert "linearGradient" in render(minimum / 72)
