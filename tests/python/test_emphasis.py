"""Emphasis: ``xtick_highlight()``, ``ytick_highlight()``, heatmap ``outline=``,
``outline_color=`` and ``edges=``, and the theme's emphasis color."""

from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"
FLAGS_HINT = "pass a boolean matrix of the shape of values, e.g. values > 0."
EMPHASIS = {"PIGMENT": "#BF2A17", "GRAPHITE": "#C8301E", "MIDNIGHT": "#FF7438"}


def bars(
    names: list[str], width: float = 5.0, theme: ub.Theme = ub.Theme.PIGMENT
) -> ub.Axes[str, float]:
    ax = ub.Figure(width, 3, theme=theme).add_axes(x=ub.Scale.category())
    return ax.bar(names, [1.0] * len(names))


def grid(theme: ub.Theme = ub.Theme.PIGMENT) -> ub.Axes[str, str]:
    return ub.Figure(5, 4, theme=theme).add_axes(x=ub.Scale.category(), y=ub.Scale.category())


def labels(ax: ub.Axes[Any, Any]) -> dict[str, tuple[str, str]]:
    """The font weight and fill of every editable SVG text of the figure of ``ax``, by text."""
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return {
        node.text or "": (node.get("font-weight", ""), node.get("fill", ""))
        for node in root.iter(f"{SVG}text")
    }


def strokes(ax: ub.Axes[Any, Any]) -> list[tuple[str, str]]:
    """The stroke color and width of every stroked SVG path of the figure of ``ax``."""
    root = ElementTree.fromstring(ax.figure.to_svg())
    return [
        (node.get("stroke", ""), node.get("stroke-width", ""))
        for node in root.iter(f"{SVG}path")
        if node.get("stroke") not in (None, "none")
    ]


# The emphasis color


@pytest.mark.parametrize("name", ["PIGMENT", "GRAPHITE", "MIDNIGHT"])
def test_presets_have_an_emphasis_color(name: str) -> None:
    theme: ub.Theme = getattr(ub.Theme, name)
    assert theme.emphasis_color == ub.Color.hex(EMPHASIS[name])


def test_with_emphasis_color_sets_the_automatic_color() -> None:
    theme = ub.Theme.PIGMENT.with_emphasis_color(ub.Color.TEAL)
    assert theme.emphasis_color == ub.Color.TEAL
    assert ub.Theme.PIGMENT.emphasis_color == ub.Color.hex("#BF2A17")
    ax = bars(["a", "b"], theme=theme).xtick_highlight(["b"])
    assert labels(ax)["b"] == ("600", "#2E8B8B")
    outlined = grid(theme).heatmap(["a"], ["p"], [[1.0]], outline=[[True]])
    assert strokes(outlined).count(("#2E8B8B", "1.5")) == 1


# Highlighted tick labels


def test_highlighted_labels_are_bold_in_the_emphasis_color() -> None:
    for name, color in EMPHASIS.items():
        ax = bars(["North", "South", "East"], theme=getattr(ub.Theme, name))
        assert ax.xtick_highlight(["South"]) is ax
        found = labels(ax)
        assert found["South"] == ("600", color)
        assert found["North"][0] == "400"
        assert found["North"][1] != color


def test_y_labels_and_an_explicit_color() -> None:
    ax = grid().heatmap(["a", "b"], ["p", "q", "r"], np.arange(6.0).reshape(3, 2))
    ax.ytick_highlight(np.array(["q", "r"]), color=ub.Color.BLUE)
    found = labels(ax)
    assert found["q"] == ("600", "#3366B8")
    assert found["r"] == ("600", "#3366B8")
    assert found["p"][0] == "400"
    assert found["a"][0] == "400"


def test_a_second_call_replaces_and_an_empty_list_removes() -> None:
    plain = bars(["a", "b", "c"]).figure.to_svg()
    ax = bars(["a", "b", "c"]).xtick_highlight(["a", "b"]).xtick_highlight(pd.Series(["c"]))
    found = labels(ax)
    assert found["a"][0] == "400"
    assert found["c"][0] == "600"
    assert ax.xtick_highlight([]).figure.to_svg() == plain


@pytest.mark.parametrize(
    ("scale", "name"),
    [(ub.Scale.linear(), "linear"), (ub.Scale.log(), "log"), (ub.Scale.time(), "time")],
)
def test_numeric_and_time_axes_refuse_a_highlight(scale: ub.Scale[Any], name: str) -> None:
    ax = ub.Figure().add_axes(x=scale, y=scale)
    with pytest.raises(ub.UsageError) as caught:
        ax.xtick_highlight(["a"])
    assert str(caught.value) == (
        f"Axes.xtick_highlight(): this is a {name} axis; tick highlights apply to category "
        "axes only."
    )
    assert details(caught.value) == (None, "pass x=Scale.category() to Figure.add_axes().")
    with pytest.raises(ub.UsageError):
        ax.ytick_highlight(["a"])


def test_argument_types_are_checked() -> None:
    ax = bars(["a", "b"])
    with pytest.raises(TypeError):
        ax.xtick_highlight(cast(Any, "a"))
    with pytest.raises(TypeError):
        ax.xtick_highlight(["a"], color=cast(Any, "red"))
    with pytest.raises(ub.DataError):
        ax.xtick_highlight(["a\nb"])


def test_an_unknown_category_is_a_data_error_when_rendering() -> None:
    ax = bars(["North", "South"]).xtick_highlight(["Sout"])
    with pytest.raises(ub.DataError) as caught:
        ax.figure.to_svg()
    assert str(caught.value) == (
        'Figure.to_svg(): xtick_highlight() names "Sout", which is not a category of the x axis.'
    )


def test_a_highlight_left_out_by_crowded_labels_is_a_style_error() -> None:
    names = [f"class {i:02}" for i in range(40)]
    narrow = bars(names, width=4.0)
    shown = [text for text in labels(narrow) if text.startswith("class ")]
    hidden = next(name for name in names if name not in shown)
    narrow.xtick_highlight([hidden])
    with pytest.raises(ub.StyleError) as caught:
        narrow.figure.to_png()
    assert f'which leaves out the highlighted "{hidden}".' in str(caught.value)
    assert details(caught.value) == (
        None,
        "enlarge the figure, or set a smaller tick label size with xtick_labels().",
    )
    # A shown label renders, and so does the hidden one on a figure wide enough for all.
    narrow.xtick_highlight([shown[1]])
    assert labels(narrow)[shown[1]][0] == "600"
    wide = bars(names, width=24.0).xtick_highlight([hidden])
    assert labels(wide)[hidden][0] == "600"


def test_shared_axes_share_the_highlight() -> None:
    fig = ub.Figure(5, 5, rows=2, share_x=ub.Share.ALL)
    top = fig.add_axes(row=0, x=ub.Scale.category()).bar(["a", "b"], [1, 2])
    bottom = fig.add_axes(row=1, x=ub.Scale.category()).bar(["a", "b"], [2, 1])
    top.xtick_highlight(["b"])
    assert labels(bottom)["b"][0] == "600"
    with pytest.raises(ub.UsageError) as caught:
        bottom.xtick_highlight(["a"])
    assert str(caught.value) == (
        "Axes.xtick_highlight(): x tick highlight is shared with row 0, col 0, which set "
        "another highlight."
    )
    bottom.xtick_highlight(["b"])


def test_a_twin_highlights_the_shared_x_axis() -> None:
    left = ub.Figure().add_axes(x=ub.Scale.category()).bar(["Jan", "Feb"], [40, 55])
    right = left.twin_y().line(["Jan", "Feb"], [7, 9])
    right.xtick_highlight(["Feb"])
    assert labels(left)["Feb"][0] == "600"
    with pytest.raises(ub.UsageError):
        right.ytick_highlight(["Feb"])


# Outlined cells and edges


def confusion(**options: Any) -> ub.Axes[str, str]:
    names = ["bird", "dog", "drill", "saw"]
    counts = np.array([[9, 1, 0, 0], [2, 7, 0, 1], [0, 0, 6, 3], [0, 1, 2, 8]])
    group = np.isin(names, ["drill", "saw"])
    mask = group[:, None] & group[None, :] & ~np.eye(4, dtype=bool) & (counts > 0)
    values = np.where(counts > 0, counts, np.nan)
    ax = grid().heatmap(names, names, values, annotate=True, outline=mask, **options)
    return ax.xtick_highlight(["drill", "saw"]).ytick_highlight(["drill", "saw"])


def test_outlines_take_the_emphasis_color_or_an_explicit_one() -> None:
    assert strokes(confusion()).count(("#BF2A17", "1.5")) == 2
    blue = strokes(confusion(outline_color=ub.Color.BLUE))
    assert blue.count(("#3366B8", "1.5")) == 2
    assert ("#BF2A17", "1.5") not in blue


def test_masks_of_lists_arrays_and_frames_match() -> None:
    outputs: set[str] = set()
    masks: list[object] = [
        [[True, False], [False, True]],
        np.eye(2, dtype=bool),
        [np.array([True, False]), np.array([False, True])],
        pd.DataFrame([[True, False], [False, True]]),
    ]
    for mask in masks:
        ax = grid().heatmap(["a", "b"], ["p", "q"], [[1, 2], [3, 4]], outline=cast(Any, mask))
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


def test_a_nan_cell_can_be_outlined() -> None:
    ax = grid().heatmap(["a", "b"], ["p"], [[np.nan, 2.0]], outline=[[True, False]])
    assert strokes(ax).count(("#BF2A17", "1.5")) == 1


def test_outline_shape_errors_name_both_shapes() -> None:
    with pytest.raises(ub.DataError) as caught:
        grid().heatmap(["a", "b", "c"], ["p", "q"], np.zeros((2, 3)), outline=np.eye(3, dtype=bool))
    assert str(caught.value) == (
        "Axes.heatmap(): outline has shape (3, 3); it must be the shape of values, "
        "(len(y_categories), len(x_categories)) = (2, 3)."
    )
    assert details(caught.value) == ("outline", FLAGS_HINT)
    with pytest.raises(ub.DataError) as caught:
        grid().heatmap(["a"], ["p"], [[1.0]], outline=cast(Any, [True]))
    assert "outline[0] is True; each row must be a sequence." in str(caught.value)
    with pytest.raises(ub.DataError) as caught:
        grid().heatmap(["a", "b"], ["p", "q"], np.zeros((2, 2)), outline=[[True, False], [True]])
    assert str(caught.value) == (
        "Axes.heatmap(): outline[1] has 1 flags but outline[0] has 2; every row must have "
        "the same length."
    )


def test_a_mask_must_be_two_dimensional_and_unmasked() -> None:
    with pytest.raises(ub.DataError) as caught:
        grid().heatmap(["a", "b"], ["p"], [[1.0, 2.0]], outline=np.array([True, False]))
    assert str(caught.value) == (
        "Axes.heatmap(): outline has 1 dimension; outline must be two-dimensional."
    )
    with pytest.raises(ub.DataError) as caught:
        grid().heatmap(["a"], ["p"], [[1.0]], outline=[np.array([[True]])])
    assert "outline[0] has 2 dimensions; each row must be one-dimensional." in str(caught.value)
    with pytest.raises(ub.DataError):
        grid().heatmap(["a"], ["p"], [[1.0]], outline=np.ma.masked_array([[True]]))


def test_a_mask_that_is_not_boolean_is_a_type_error() -> None:
    with pytest.raises(TypeError) as caught:
        grid().heatmap(["a", "b"], ["p"], [[1.0, 2.0]], outline=np.array([[1, 0]]))
    assert str(caught.value) == "Axes.heatmap(): outline has dtype int64; outline must hold bools."
    assert details(caught.value) == ("outline", FLAGS_HINT)
    with pytest.raises(TypeError) as caught:
        grid().heatmap(["a", "b"], ["p"], [[1.0, 2.0]], outline=cast(Any, [[True, 0]]))
    assert str(caught.value) == "Axes.heatmap(): outline[0][1] is int; outline must hold bools."
    with pytest.raises(TypeError) as caught:
        grid().heatmap(["a"], ["p"], [[1.0]], outline=[np.array([0.5])])
    assert str(caught.value) == (
        "Axes.heatmap(): outline[0] has dtype float64; outline must hold bools."
    )
    with pytest.raises(TypeError):
        grid().heatmap(["a"], ["p"], [[1.0]], outline=cast(Any, True))


def test_outline_color_needs_an_outline() -> None:
    ax = grid()
    with pytest.raises(ub.UsageError) as caught:
        ax.heatmap(["a"], ["p"], [[1.0]], outline_color=ub.Color.BLUE)
    assert str(caught.value) == (
        "Axes.heatmap(): outline_color is given but outline is not; only outlined cells take it."
    )
    assert details(caught.value) == (
        "outline_color",
        "pass outline= with one flag per cell, or leave outline_color out.",
    )
    # Nothing was added, and the automatic color needs no outline.
    assert ax.heatmap(["a"], ["p"], [[1.0]], outline_color=ub.Color.AUTO) is ax


def test_edges_lie_between_cells_in_the_grid_color() -> None:
    for rows, columns in [(2, 3), (4, 5), (1, 1)]:
        x = [f"c{i}" for i in range(columns)]
        y = [f"r{i}" for i in range(rows)]
        for theme, grid_color in [(ub.Theme.PIGMENT, "#DDD3C7"), (ub.Theme.MIDNIGHT, "#272B34")]:
            values = np.ones((rows, columns))
            plain = strokes(grid(theme).heatmap(x, y, values))
            edged = strokes(grid(theme).heatmap(x, y, values, edges=True))
            added = len(edged) - len(plain)
            assert added == rows - 1 + columns - 1
            assert edged.count((grid_color, "0.5")) - plain.count((grid_color, "0.5")) == added
    with pytest.raises(TypeError):
        grid().heatmap(["a"], ["p"], [[1.0]], edges=cast(Any, 1))


def test_raster_cells_keep_vector_edges_and_outlines() -> None:
    vector = confusion(render=ub.Render.VECTOR, edges=True)
    raster = confusion(render=ub.Render.RASTER, edges=True)
    assert "<image" in raster.figure.to_svg()
    assert strokes(raster).count(("#BF2A17", "1.5")) == 2
    assert strokes(raster).count(("#DDD3C7", "0.5")) == strokes(vector).count(("#DDD3C7", "0.5"))


def test_png_svg_and_pdf_render() -> None:
    ax = confusion(edges=True)
    assert ax.figure.to_png()[:8] == b"\x89PNG\r\n\x1a\n"
    assert ax.figure.to_pdf()[:5] == b"%PDF-"
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    bold = [node.text or "" for node in root.iter(f"{SVG}text") if node.get("font-weight") == "600"]
    assert sorted(bold) == ["drill", "drill", "saw", "saw"]
