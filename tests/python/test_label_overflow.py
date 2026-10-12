"""Tick labels that leave no room for the plot or its x tick labels raise ``StyleError``."""

from collections.abc import Callable
from pathlib import Path

import pytest

import umber as ub
from umber._errors import details

HINT = "shorten the labels or widen the figure."
WIDE = "W" * 60
# A 6 in midnight figure: the last tick may move in to 318 pt and its label may reach 426 pt,
# so labels fit up to 216 pt wide; 60 W's are 306 pt wide.
MESSAGE = f'x tick label "{"W" * 38}… is 306 pt wide; at most 216 pt fits this figure.'


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


def _end_after_wide_y_labels(y_chars: int) -> ub.Figure:
    fig = ub.Figure(2, 2, theme=ub.Theme.MIDNIGHT)
    wide = "W" * y_chars
    fig.add_axes().xlim(0, 1).xticks_labeled([1], ["END"]).yticks_labeled([0, 1], [wide, wide])
    return fig


@pytest.mark.parametrize(("context", "render"), RENDERS, ids=[c for c, _ in RENDERS])
def test_y_labels_that_leave_no_room_for_the_plot_raise_style_error(
    tmp_path: Path, context: str, render: Callable[[ub.Figure, Path], object]
) -> None:
    # 100 W's, 510 pt, push the plot's left edge past its right edge at 132 pt; y tick labels
    # up to 113 pt wide leave the plot 1 pt.
    with pytest.raises(ub.StyleError) as caught:
        render(_end_after_wide_y_labels(100), tmp_path)
    assert str(caught.value) == (
        f'{context}: y tick label "{"W" * 38}… is 510 pt wide; at most 113 pt leaves room for '
        "the plot area."
    )
    hint = "shorten the y tick labels or widen the figure."
    assert details(caught.value) == (None, hint)
    assert _notes(caught.value) == [f"hint: {hint}"]
    assert not (tmp_path / "out.png").exists()
    assert not (tmp_path / "out.svg").exists()


def test_x_labels_need_a_plot_at_least_one_point_wide() -> None:
    # 22 W's leave the plot room, but END fits only on a plot narrower than 1 pt.
    with pytest.raises(ub.StyleError) as caught:
        _end_after_wide_y_labels(22).to_svg()
    assert str(caught.value) == (
        'Figure.to_svg(): x tick label "END" is 15 pt wide; at most 13 pt fits this figure.'
    )
    # 21 W's leave room for END.
    assert _end_after_wide_y_labels(21).to_svg().startswith("<svg")


def test_y_labels_fail_the_same_way_without_x_ticks_or_with_an_empty_label() -> None:
    with pytest.raises(ub.StyleError) as caught:
        _end_after_wide_y_labels(100).to_svg()
    expected = str(caught.value)
    assert expected.startswith('Figure.to_svg(): y tick label "W')
    no_ticks = _end_after_wide_y_labels(100)
    no_ticks.axes.xticks([])
    empty = _end_after_wide_y_labels(100)
    empty.axes.xticks_labeled([0.5], [""])
    for fig in (no_ticks, empty):
        with pytest.raises(ub.StyleError) as caught:
            fig.to_svg()
        assert str(caught.value) == expected
        assert details(caught.value) == (None, "shorten the y tick labels or widen the figure.")


def test_a_figure_too_narrow_for_the_plot_raises_style_error() -> None:
    # 0.3 in is 21.6 pt; without tick labels the plot needs 2 x 12 + 1 = 25 pt.
    fig = ub.Figure(0.3, 1, theme=ub.Theme.MIDNIGHT)
    fig.add_axes().xticks([]).yticks([])
    with pytest.raises(ub.StyleError) as caught:
        fig.to_png(dpi=72)
    assert str(caught.value) == (
        "Figure.to_png(): the figure is 21 pt wide; at least 25 pt is needed for the plot area."
    )
    assert details(caught.value) == (None, "widen the figure.")
