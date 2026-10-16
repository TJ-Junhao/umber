"""Panel legend positions, theme defaults and argument errors."""

from typing import cast

import pytest

import umber as ub
from umber import _umber
from umber._errors import details
from umber._theme import native


def test_direct_style_rejects_position() -> None:
    ax = ub.Figure().add_axes().line([0, 1], [1, 2], label="North")
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.legend(ub.LegendStyle.DIRECT, position=ub.LegendPosition.TOP_RIGHT)
    assert str(caught.value) == (
        "Axes.legend(): position is LegendPosition.TOP_RIGHT; direct labels sit at the "
        "ends of their lines and take no position."
    )
    hint = "drop position=, or use another legend style."
    assert details(caught.value) == ("position", hint)
    line = "ax.legend(ub.LegendStyle.DIRECT, position=ub.LegendPosition.TOP_RIGHT)"
    argument = "position=ub.LegendPosition.TOP_RIGHT"
    assert getattr(caught.value, "__notes__", []) == [
        f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}",
        f"hint: {hint}",
    ]
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("position", list(ub.LegendPosition))
def test_legend_position_default_follows_theme(position: ub.LegendPosition) -> None:
    theme = ub.Theme.PIGMENT.with_legend_position(position)
    ax = ub.Figure(theme=theme).add_axes().line([0, 1], [1, 2], label="North")
    default = ax.figure.to_svg()
    assert ax.legend(ub.LegendStyle.HEADER) is ax
    assert ax.figure.to_svg() == default
    ax.legend(ub.LegendStyle.HEADER, position=position)
    assert ax.figure.to_svg() == default
    other = (
        ub.LegendPosition.BOTTOM if position is ub.LegendPosition.RIGHT else ub.LegendPosition.RIGHT
    )
    ax.legend(ub.LegendStyle.HEADER, position=other)
    assert ax.figure.to_svg() != default
    ax.legend(ub.LegendStyle.HEADER, position=ub.AUTO)
    assert ax.figure.to_svg() == default
    ax.legend(ub.LegendStyle.HEADER, position=other)
    ax.legend(ub.LegendStyle.HEADER)
    assert ax.figure.to_svg() == default


@pytest.mark.parametrize("position", list(ub.LegendPosition))
@pytest.mark.parametrize(
    "style", [ub.LegendStyle.HEADER, ub.LegendStyle.CHIPS, ub.LegendStyle.NONE]
)
def test_explicit_position_matches_native(
    position: ub.LegendPosition, style: ub.LegendStyle
) -> None:
    fig = ub.Figure()
    ax = fig.add_axes().title("Panel")
    ax.hline(1, label="North")
    assert ax.legend(style, position=position) is ax
    expected = _umber.Figure(6, 4, native(ub.Theme.PIGMENT), 1, 1, 0, 0, 0, 0)
    expected.add_axes(0, 0, 0, 0, 1, 1, 0, 10.0, 0, 10.0)
    expected.title(0, 0, "Panel")
    expected.hline(0, 0, 1, "North", (0, 0, 1.0), ub.Dash.SOLID.value, (0, 0.0))
    expected.legend_at(0, 0, style.value, position.value)
    assert fig.to_svg() == expected.to_svg(ub.SvgText.OUTLINES.value)


@pytest.mark.parametrize("position", list(ub.LegendPosition))
def test_direct_rejects_every_explicit_position_atomically(position: ub.LegendPosition) -> None:
    ax = ub.Figure().add_axes().line([0, 1], [1, 2], label="North")
    ax.legend(ub.LegendStyle.CHIPS, position=ub.LegendPosition.BOTTOM)
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.legend(ub.LegendStyle.DIRECT, position=position)
    assert str(caught.value) == (
        f"Axes.legend(): position is LegendPosition.{position.name}; direct labels sit at the "
        "ends of their lines and take no position."
    )
    assert details(caught.value) == ("position", "drop position=, or use another legend style.")
    assert ax.figure.to_svg() == before


def test_direct_accepts_auto_and_ignores_theme_position() -> None:
    theme = ub.Theme.PIGMENT.with_legend_position(ub.LegendPosition.RIGHT)
    ax = ub.Figure(theme=theme).add_axes().line([0, 1], [1, 2], label="North")
    assert ax.legend(ub.LegendStyle.DIRECT, position=ub.AUTO) is ax
    expected = ub.Figure().add_axes().line([0, 1], [1, 2], label="North")
    expected.legend(ub.LegendStyle.DIRECT)
    assert ax.figure.to_svg() == expected.figure.to_svg()


@pytest.mark.parametrize("value", ["right", None, 2, True, ub.LegendStyle.HEADER, ub.Align.RIGHT])
def test_position_type_error_changes_nothing(value: object) -> None:
    ax = ub.Figure().add_axes().line([0, 1], [1, 2], label="North")
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        ax.legend(ub.LegendStyle.DIRECT, position=cast(ub.LegendPosition, value))
    shown = "None" if value is None else type(value).__name__
    assert str(caught.value) == (
        f"Axes.legend(): position is {shown}; position must be of type LegendPosition."
    )
    assert details(caught.value) == (
        "position",
        "use LegendPosition.TOP_LEFT, LegendPosition.TOP_RIGHT, LegendPosition.RIGHT, "
        "LegendPosition.BOTTOM or LegendPosition.BEST. Use ub.AUTO for the theme's position.",
    )
    assert ax.figure.to_svg() == before


def test_position_is_keyword_only() -> None:
    ax = ub.Figure().add_axes()
    with pytest.raises(TypeError):
        ax.legend(ub.LegendStyle.HEADER, ub.LegendPosition.RIGHT)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]


@pytest.mark.parametrize("mode", [ub.Legend.MERGED, ub.Legend.PER_PANEL])
def test_grid_positions_apply_only_to_per_panel_legends(mode: ub.Legend) -> None:
    fig = ub.Figure(cols=2, legend=mode).title("Stations")
    for cell in fig.cells():
        cell.add_axes().line([0, 1], [1, 2], label="North")
    before = fig.to_svg()
    fig.axes_at(col=0).legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT)
    fig.axes_at(col=1).legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.BOTTOM)
    assert (fig.to_svg() == before) is (mode is ub.Legend.MERGED)


def test_best_is_distinct_from_auto() -> None:
    assert "AUTO" not in ub.LegendPosition.__members__
    ax = ub.Figure().add_axes().line([0, 1], [1, 2], label="North")
    ax.legend(ub.LegendStyle.HEADER, position=ub.AUTO)
    before = ax.figure.to_svg()
    ax.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.BEST)
    assert ax.figure.to_svg() != before
