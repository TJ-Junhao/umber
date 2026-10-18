"""Swarm placement failures and repeatable drawing."""

import re
from pathlib import Path

import pytest

import umber as ub
from umber._errors import details


def test_swarm_overflow_names_category() -> None:
    ax = ub.Figure(4, 3).add_axes(x=ub.Scale.category())
    ax.swarm(["A", "B"], [[1], [2] * 812], size=ub.Size.pt(6))
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert re.fullmatch(
        r"Figure.to_svg\(\): the swarm of category 'B' needs 4879 pt across; "
        r"its band is \d+ pt\.",
        str(caught.value),
    )
    assert details(caught.value) == (None, "use strip() or violin() for many points.")


def test_swarm_is_deterministic_and_order_independent() -> None:
    outputs: set[str] = set()
    for values in [[1, 1, 2, 2, 3], [3, 2, 1, 2, 1]]:
        ax = ub.Figure().add_axes(x=ub.Scale.category())
        assert ax.swarm(["A"], [values], size=ub.Size.pt(6)) is ax
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


@pytest.mark.parametrize("render", ["to_svg", "to_png", "save_svg", "save_png"])
@pytest.mark.parametrize("grid", [False, True])
def test_swarm_overflow_uses_render_context_and_panel(
    render: str, grid: bool, tmp_path: Path
) -> None:
    fig = ub.Figure(8 if grid else 4, 3, cols=2 if grid else 1)
    ax = fig.add_axes(col=1 if grid else 0, y=ub.Scale.category())
    ax.swarm(["B"], [[2] * 812], size=ub.Size.pt(6))
    with pytest.raises(ub.StyleError) as caught:
        if render.startswith("save"):
            getattr(fig, render)(tmp_path / f"overflow.{render[-3:]}")
        else:
            getattr(fig, render)()
    panel = "row 0, col 1: " if grid else ""
    assert str(caught.value).startswith(
        f"Figure.{render}(): {panel}the swarm of category 'B' needs 4879 pt across; "
    )
    assert details(caught.value) == (None, "use strip() or violin() for many points.")
    assert not list(tmp_path.iterdir())


def test_swarm_packs_only_visible_samples() -> None:
    ax = ub.Figure(4, 3).add_axes(x=ub.Scale.category()).ylim(0, 1)
    ax.swarm(["A"], [[0.5] + [100] * 812])
    assert ax.figure.to_svg()


@pytest.mark.parametrize("horizontal", [False, True])
def test_swarm_required_width_ignores_hidden_samples(horizontal: bool) -> None:
    errors: list[str] = []
    for hidden in [[], [100.0] * 812]:
        fig = ub.Figure(4, 3)
        ax: ub.Axes[float, str] | ub.Axes[str, float]
        if horizontal:
            ax = fig.add_axes(y=ub.Scale.category()).xlim(0, 1)
        else:
            ax = fig.add_axes(x=ub.Scale.category()).ylim(0, 1)
        ax.swarm(["B"], [[0.5] * 80 + hidden], size=ub.Size.pt(6))
        with pytest.raises(ub.StyleError) as caught:
            fig.to_svg()
        errors.append(str(caught.value))
    assert errors[0] == errors[1]
    assert "needs 487 pt across" in errors[0]
