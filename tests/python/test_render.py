"""The façade adds no drawing of its own: its figures render exactly as native ones."""

from pathlib import Path

import numpy as np
import numpy.typing as npt

import umber as ub
from umber import _umber

LINEAR, LOG, CATEGORY = 0, 1, 2
OUTLINES, EDITABLE = 0, 1
AUTO = (0, 0, 1.0)
NO_SIZE = (0, 0.0)


def floats(*values: float) -> npt.NDArray[np.float64]:
    return np.array(values, dtype=np.float64)


def facade_figure() -> ub.Figure:
    theme = ub.Theme.GRAPHITE.with_line_width(2.5)
    fig = ub.Figure(5, 3.5, theme=theme)
    (
        fig.add_axes(y=ub.Scale.log(2))
        .title("Growth")
        .subtitle("Two regions")
        .xlabel("Year")
        .ylabel("Revenue")
        .line([2020, 2021, 2022, 2023], [1, 2, 4, 8], label="North")
        .line(
            np.array([2020, 2021, 2022, 2023]),
            [3, 3.5, float("nan"), 6],
            label="South",
            color=ub.Color.TEAL.alpha(0.8),
            dash=ub.Dash.DASHED,
        )
        .scatter(
            (2020.5, 2022.5),
            np.array([2, 5], dtype=np.float32),
            label="Events",
            color=ub.Color.palette(3),
            marker=ub.Marker.DIAMOND,
        )
        .xlim(2019.5, 2023.5)
        .ylim(0.5, 16)
        .legend(ub.LegendStyle.CHIPS)
    )
    return fig


def native_figure() -> _umber.Figure:
    theme = _umber.Theme.graphite().with_line_width(2.5)
    fig = _umber.Figure(5.0, 3.5, theme)
    fig.add_axes(LINEAR, 10.0, LOG, 2.0)
    fig.title("Growth")
    fig.subtitle("Two regions")
    fig.xlabel("Year")
    fig.ylabel("Revenue")
    fig.line(floats(2020, 2021, 2022, 2023), floats(1, 2, 4, 8), "North", AUTO, 0, NO_SIZE)
    teal = (2, 0x2E8B8BCC, 1.0)
    fig.line(floats(2020, 2021, 2022, 2023), floats(3, 3.5, np.nan, 6), "South", teal, 1, NO_SIZE)
    fig.scatter(floats(2020.5, 2022.5), floats(2, 5), "Events", (1, 3, 1.0), 3, NO_SIZE)
    fig.xlim(2019.5, 2023.5)
    fig.ylim(0.5, 16.0)
    fig.legend(2)
    return fig


def category_figures() -> tuple[ub.Figure, _umber.Figure]:
    fig = ub.Figure(4, 3, theme=ub.Theme.MIDNIGHT)
    fig.add_axes(x=ub.Scale.category()).line(["Q1", "Q2", "Q3"], [3, 1, 2], label="Sales").scatter(
        ["Q2", "Q4"], [2, 4]
    )
    native = _umber.Figure(4.0, 3.0, _umber.Theme.midnight())
    native.add_axes(CATEGORY, 10.0, LINEAR, 10.0)
    native.line(["Q1", "Q2", "Q3"], floats(3, 1, 2), "Sales", AUTO, 0, NO_SIZE)
    native.scatter(["Q2", "Q4"], floats(2, 4), "", AUTO, 0, NO_SIZE)
    return fig, native


def test_png_and_svg_bytes_match_the_native_figure() -> None:
    fig, native = facade_figure(), native_figure()
    assert fig.to_png(dpi=72) == native.to_png(72)
    assert fig.to_png() == native.to_png(200)
    assert fig.to_svg() == native.to_svg(OUTLINES)
    assert fig.to_svg(text=ub.SvgText.EDITABLE) == native.to_svg(EDITABLE)


def test_category_axes_match_the_native_figure() -> None:
    fig, native = category_figures()
    assert fig.to_png(dpi=60) == native.to_png(60)
    assert fig.to_svg() == native.to_svg(OUTLINES)


def test_saved_files_match_the_native_figure(tmp_path: Path) -> None:
    fig, native = facade_figure(), native_figure()
    fig.save_png(tmp_path / "chart.png", dpi=90)
    fig.save_svg(tmp_path / "chart.svg", text=ub.SvgText.EDITABLE)
    assert (tmp_path / "chart.png").read_bytes() == native.png_for_file(90)
    assert (tmp_path / "chart.svg").read_bytes() == native.svg_for_file(EDITABLE).encode("utf-8")


def test_an_empty_figure_matches_the_native_figure() -> None:
    fig = ub.Figure(2, 1.5, theme=ub.Theme.PIGMENT)
    native = _umber.Figure(2.0, 1.5, _umber.Theme.pigment())
    assert fig.to_png(dpi=50) == native.to_png(50)
    assert fig.to_svg() == native.to_svg(OUTLINES)


def test_a_copy_renders_as_the_original() -> None:
    import copy

    fig = facade_figure()
    assert copy.copy(fig).to_png(dpi=50) == native_figure().to_png(50)
    assert copy.deepcopy(fig).to_svg() == native_figure().to_svg(OUTLINES)
