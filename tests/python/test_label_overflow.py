"""X tick labels that cannot fit the figure make every render method raise ``StyleError``."""

from collections.abc import Callable
from pathlib import Path

import pytest

import umber as ub
from umber._errors import details

HINT = "shorten the labels or widen the figure."
WIDE = "W" * 60
# A 6 in midnight figure: the last tick may move in to 318 pt and its label may reach 426 pt,
# so labels fit up to 216 pt wide; 60 W's are 306 pt wide.
MESSAGE = f'x tick label "{"W" * 39}…" is 306 pt wide; at most 216 pt fits this figure.'


def _figure(last: str) -> ub.Figure:
    fig = ub.Figure(6, 4, theme=ub.Theme.MIDNIGHT)
    fig.add_axes().xlim(0, 1).xticks_labeled([0, 1], ["0", last])
    return fig


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


RENDERS: list[tuple[str, Callable[[ub.Figure, Path], object]]] = [
    ("Figure.to_png()", lambda fig, _: fig.to_png(dpi=72)),
    ("Figure.to_svg()", lambda fig, _: fig.to_svg()),
    ("Figure.save_png()", lambda fig, tmp: fig.save_png(tmp / "out.png", dpi=72)),
    ("Figure.save_svg()", lambda fig, tmp: fig.save_svg(tmp / "out.svg")),
]


@pytest.mark.parametrize(("context", "render"), RENDERS, ids=[c for c, _ in RENDERS])
def test_labels_that_cannot_fit_raise_style_error(
    tmp_path: Path, context: str, render: Callable[[ub.Figure, Path], object]
) -> None:
    fig = _figure(WIDE)
    with pytest.raises(ub.StyleError) as caught:
        render(fig, tmp_path)
    assert str(caught.value) == f"{context}: {MESSAGE}"
    assert details(caught.value) == (None, HINT)
    assert _notes(caught.value) == [f"hint: {HINT}"]
    assert not (tmp_path / "out.png").exists()
    assert not (tmp_path / "out.svg").exists()


def test_the_figure_renders_once_the_labels_are_shortened(tmp_path: Path) -> None:
    fig = _figure(WIDE)
    with pytest.raises(ub.StyleError):
        fig.to_png(dpi=72)
    # The failed render left the figure as it was: it fails again the same way.
    with pytest.raises(ub.StyleError) as caught:
        fig.to_svg()
    assert str(caught.value) == f"Figure.to_svg(): {MESSAGE}"
    # 42 W's are 214.2 pt wide and fit.
    fig.axes.xticks_labeled([0, 1], ["0", "W" * 42])
    assert fig.to_png(dpi=72)[1:4] == b"PNG"
    assert fig.to_svg().startswith("<svg")
    fig.save_png(tmp_path / "out.png", dpi=72)
    fig.save_svg(tmp_path / "out.svg")
    assert fig.to_svg() == _figure("W" * 42).to_svg()


def test_a_wider_figure_fits_the_same_labels() -> None:
    fig = ub.Figure(12, 4, theme=ub.Theme.MIDNIGHT)
    fig.add_axes().xlim(0, 1).xticks_labeled([0, 1], ["0", WIDE])
    assert fig.to_svg().startswith("<svg")
