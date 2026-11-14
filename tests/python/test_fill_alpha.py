"""Translucent fill colors used as given, and the width of a scatter's joining line."""

import re
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import pytest

import umber as ub

THEMES = [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT]
AREA = 82 / 255
BLUE = ub.Color.hex("#1F77B4")
X = [0.0, 1.0, 2.0, 3.0]
HALF = ub.Size.times(0.5)
HALF_BLUE = BLUE.alpha(0.5)


def _opacities(fig: ub.Figure) -> list[float]:
    """The ``fill-opacity`` values of the figure's SVG, in drawing order."""
    return [float(v) for v in re.findall(r'fill-opacity="([0-9.]+)"', fig.to_svg())]


def _band(color: ub.Color) -> Callable[[ub.Axes[Any, Any]], object]:
    return lambda ax: ax.band(X, [1.0, 2.0, 1.0, 2.0], [3.0, 4.0, 3.0, 4.0], color=color)


def _area(color: ub.Color) -> Callable[[ub.Axes[Any, Any]], object]:
    return lambda ax: ax.area(X, [3.0, 5.0, 4.0, 6.0], color=color)


def _hspan(color: ub.Color) -> Callable[[ub.Axes[Any, Any]], object]:
    return lambda ax: ax.line(X, [0.0, 5.0, 2.0, 6.0]).hspan(2.0, 3.0, color=color)


def _vspan(color: ub.Color) -> Callable[[ub.Axes[Any, Any]], object]:
    return lambda ax: ax.line(X, [0.0, 5.0, 2.0, 6.0]).vspan(1.0, 2.0, color=color)


@pytest.mark.parametrize("make", [_band, _area, _hspan, _vspan])
@pytest.mark.parametrize("theme", THEMES)
def test_a_translucent_fill_is_drawn_at_the_opacity_given(
    make: Callable[[ub.Color], Callable[[ub.Axes[Any, Any]], object]], theme: ub.Theme
) -> None:
    for alpha in (0.15, 0.45, 0.9):
        fig = ub.Figure(4, 3, theme=theme.with_fill_style(ub.FillStyle.SOLID))
        make(BLUE.alpha(alpha))(fig.add_axes())
        assert _opacities(fig)[0] == pytest.approx(alpha, abs=0.004)


@pytest.mark.parametrize("make", [_band, _area, _hspan, _vspan])
def test_an_opaque_fill_keeps_the_default_opacity(
    make: Callable[[ub.Color], Callable[[ub.Axes[Any, Any]], object]],
) -> None:
    for color in (BLUE, BLUE.alpha(1.0), ub.Color.AUTO):
        fig = ub.Figure(4, 3, theme=ub.Theme.PIGMENT)
        make(color)(fig.add_axes())
        assert _opacities(fig)[0] == pytest.approx(AREA, abs=0.004)
    # alpha(1.0) is the same color as the opaque one: the figures are byte-identical.
    plain, one = ub.Figure(4, 3), ub.Figure(4, 3)
    make(BLUE)(plain.add_axes())
    make(BLUE.alpha(1.0))(one.add_axes())
    assert plain.to_svg() == one.to_svg()


def test_a_translucent_gradient_area_and_histogram_start_at_the_opacity_given() -> None:
    theme = ub.Theme.MIDNIGHT
    area = ub.Figure(4, 3, theme=theme)
    area.add_axes().area(X, [3.0, 5.0, 4.0, 6.0], color=BLUE.alpha(0.6))
    assert "linearGradient" in area.to_svg()
    assert 'stop-opacity="0.6' in area.to_svg()
    default = ub.Figure(4, 3, theme=theme)
    default.add_axes().area(X, [3.0, 5.0, 4.0, 6.0], color=BLUE)
    assert 'stop-opacity="0.322' in default.to_svg()
    hist = ub.Figure(4, 3, theme=theme)
    hist.add_axes().hist([1.0, 2.0, 2.0, 3.0, 3.0, 3.0], color=BLUE.alpha(0.5))
    assert 'stop-opacity="0.5' in hist.to_svg()


GROUPS = [[1.0, 2.0, 3.0, 4.0, 5.0], [2.0, 3.0, 4.0, 5.0, 9.0]]


def _box(ax: ub.Axes[str, float]) -> object:
    return ax.box(["A", "B"], GROUPS, color=HALF_BLUE)


def _violin(ax: ub.Axes[str, float]) -> object:
    return ax.violin(["A", "B"], GROUPS, color=HALF_BLUE)


def _scatter(ax: ub.Axes[float, float]) -> object:
    return ax.scatter(X, X, color=BLUE.alpha(0.4))


def _bubbles(ax: ub.Axes[float, float]) -> object:
    return ax.bubbles(X, X, [1.0, 2.0, 3.0, 4.0], color=BLUE.alpha(0.4))


def _stem(ax: ub.Axes[float, float]) -> object:
    return ax.stem(X, X, color=BLUE.alpha(0.4))


def test_boxes_violins_and_markers_use_a_translucent_color_as_given() -> None:
    for draw in (_box, _violin):
        fig = ub.Figure(4, 3)
        draw(fig.add_axes(x=ub.Scale.category()))
        assert _opacities(fig)[0] == pytest.approx(0.5, abs=0.004)
    for marks in (_scatter, _bubbles, _stem):
        fig = ub.Figure(4, 3)
        marks(fig.add_axes())
        assert _opacities(fig)[0] == pytest.approx(0.4, abs=0.004)
    # An opaque color still takes the theme's marker alpha.
    fig = ub.Figure(4, 3, theme=ub.Theme.PIGMENT)
    fig.add_axes().scatter(X, X, color=BLUE)
    assert _opacities(fig)[0] == pytest.approx(0.85, abs=0.004)


def _svg(ax: ub.Axes[Any, Any]) -> str:
    return ax.figure.to_svg(text=ub.SvgText.OUTLINES)


def _connected(width: ub.Size) -> ub.Axes[float, float]:
    return (
        ub.Figure().add_axes().scatter(X, [2.0, 1.0, 3.0, 2.5], connect=True, connect_width=width)
    )


def _lined(width: ub.Size) -> ub.Axes[float, float]:
    alone = ub.Figure().add_axes()
    alone.line(X, [2.0, 1.0, 3.0, 2.5], color=ub.Color.palette(0), width=width)
    return alone.scatter(X, [2.0, 1.0, 3.0, 2.5])


@pytest.mark.parametrize("width", [ub.Size.pt(3.0), ub.Size.pt(0.5), ub.Size.times(2.0), HALF])
def test_connect_width_is_the_width_of_the_line(width: ub.Size) -> None:
    assert _svg(_connected(width)) == _svg(_lined(width))


def test_the_automatic_connect_width_is_half_the_line_width() -> None:
    default = ub.Figure().add_axes().scatter(X, [2.0, 1.0, 3.0, 2.5], connect=True)
    assert _svg(default) == _svg(_connected(ub.Size.AUTO)) == _svg(_connected(HALF))
    assert _svg(default) != _svg(_connected(ub.Size.pt(3.0)))


def test_connect_width_leaves_the_error_bars_and_applies_to_every_group() -> None:
    wide = ub.Size.pt(4.0)
    ends = ([1.5, 1.0, 2.5, 2.0], [2.5, 2.0, 3.5, 3.0])
    own = ub.Figure().add_axes()
    own.scatter(X, [2.0, 1.0, 3.0, 2.5], y_error=ends, connect=True, connect_width=wide)
    alone = ub.Figure().add_axes()
    color = ub.Color.palette(0)
    alone.line(X, [2.0, 1.0, 3.0, 2.5], color=color, width=wide)
    alone.y_error_bars(X, *ends, color=color, width=HALF)
    alone.scatter(X, [2.0, 1.0, 3.0, 2.5])
    assert _svg(own) == _svg(alone)
    grouped = ub.Figure().add_axes()
    grouped.scatter(
        X * 2,
        [1.0, 2.0, 1.5, 2.5, 3.0, 4.0, 3.5, 4.5],
        by=list("AAAABBBB"),
        connect=True,
        connect_width=wide,
    )
    manual = ub.Figure().add_axes()
    manual.scatter(X, [1.0, 2.0, 1.5, 2.5], label="A", connect=True, connect_width=wide)
    manual.scatter(X, [3.0, 4.0, 3.5, 4.5], label="B", connect=True, connect_width=wide)
    assert _svg(grouped) == _svg(manual)


def test_connect_width_without_connect_is_a_usage_error(tmp_path: Path) -> None:
    line = "ub.Figure().add_axes().scatter([1.0, 2.0], [2.0, 3.0], connect_width=ub.Size.pt(2))"
    path = tmp_path / "user_code.py"
    path.write_text(f"import umber as ub\n{line}\n")
    with pytest.raises(ub.UsageError) as raised:
        exec(compile(path.read_text(), str(path), "exec"), {})
    err = raised.value
    assert "connect_width is set, but connect is False" in str(err)
    notes = list(getattr(err, "__notes__", []))
    assert "hint: pass connect=True, or leave connect_width out." in notes
    argument = "connect_width=ub.Size.pt(2)"
    marked = f"  {line}\n{' ' * (2 + line.index(argument))}{'^' * len(argument)}"
    assert marked in notes
    # Grouped, and through the quick function.
    ax = ub.Figure().add_axes()
    with pytest.raises(ub.UsageError, match="connect_width"):
        cast(Callable[..., object], ax.scatter)(
            [1.0, 2.0], [2.0, 3.0], by=["A", "B"], connect_width=ub.Size.pt(2)
        )
    with pytest.raises(TypeError, match="connect_width"):
        cast(Callable[..., object], ax.scatter)([1.0], [2.0], connect=True, connect_width=3.0)
    with pytest.raises(ub.UsageError, match="connect_width"):
        cast(Callable[..., object], ub.scatter)([1.0, 2.0], [2.0, 3.0], connect_width=ub.Size.pt(2))


def test_the_quick_scatter_passes_connect_width_on() -> None:
    quick = ub.scatter(X, [2.0, 1.0, 3.0, 2.5], connect=True, connect_width=ub.Size.pt(3.0))
    assert _svg(quick) == _svg(_connected(ub.Size.pt(3.0)))
