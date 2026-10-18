"""Swarm placement failures and repeatable drawing."""

from pathlib import Path

import pytest

import umber as ub
from umber._errors import details


def test_swarm_overflow_names_category() -> None:
    ax = ub.Figure(4, 3).add_axes(x=ub.Scale.category())
    ax.swarm(["A", "B"], [[1], [2] * 812])
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert str(caught.value).startswith(
        "Axes.swarm(): category 'B' has 812 points, which do not fit in its width;"
    )
    assert details(caught.value) == ("groups", "use strip() or violin() for many points.")


def test_swarm_is_deterministic_and_order_independent() -> None:
    outputs: set[str] = set()
    for values in [[1, 1, 2, 2, 3], [3, 2, 1, 2, 1]]:
        ax = ub.Figure().add_axes(x=ub.Scale.category())
        assert ax.swarm(["A"], [values], size=ub.Size.pt(6)) is ax
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


@pytest.mark.parametrize("render", ["to_svg", "to_png", "save_svg", "save_png"])
def test_swarm_overflow_keeps_context_in_every_renderer(render: str, tmp_path: Path) -> None:
    ax = ub.Figure(4, 3).add_axes(y=ub.Scale.category())
    ax.swarm(["B"], [[2] * 812])
    with pytest.raises(ub.StyleError, match=r"^Axes.swarm\(\): category 'B' has 812 points"):
        if render.startswith("save"):
            getattr(ax.figure, render)(tmp_path / f"overflow.{render[-3:]}")
        else:
            getattr(ax.figure, render)()
    assert not list(tmp_path.iterdir())


def test_swarm_packs_only_visible_samples() -> None:
    ax = ub.Figure(4, 3).add_axes(x=ub.Scale.category()).ylim(0, 1)
    ax.swarm(["A"], [[0.5] + [100] * 812])
    assert ax.figure.to_svg()
