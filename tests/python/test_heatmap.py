"""``Axes.heatmap()``: conversions, exact errors, atomicity, annotations and the colorbar."""

from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"
ROWS_HINT = "pass one row per y category."
GRAYS = ub.Cmap.from_colors([ub.Color.hex("#101030"), ub.Color.hex("#F0F0D0")])


def axes(theme: ub.Theme = ub.Theme.PIGMENT) -> ub.Axes[str, str]:
    return ub.Figure(6, 4, theme=theme).add_axes(x=ub.Scale.category(), y=ub.Scale.category())


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def texts(ax: ub.Axes[Any, Any]) -> list[tuple[str, str]]:
    """The text and fill color of every editable SVG text of the figure of ``ax``."""
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return [(node.text or "", node.get("fill", "")) for node in root.iter(f"{SVG}text")]


def test_heatmap_returns_the_axes_and_draws_a_colorbar() -> None:
    ax = axes()
    assert ax.heatmap(["a", "b"], ["r"], [[1.0, 2.0]]) is ax
    assert "linearGradient" in ax.figure.to_svg()
    assert ax.colorbar(position=ub.ColorbarPosition.NONE) is ax
    assert "linearGradient" not in ax.figure.to_svg()


def test_lists_arrays_and_frames_match() -> None:
    outputs: set[str] = set()
    inputs: list[object] = [
        [[0, 1, 2], [3, 4, 5]],
        ((0.0, 1.0, 2.0), (3.0, 4.0, 5.0)),
        [np.array([0, 1, 2]), np.array([3.0, 4.0, 5.0])],
        np.arange(6, dtype=np.int16).reshape(2, 3),
        np.arange(12.0).reshape(2, 6)[:, ::2] / 2,
        pd.DataFrame([[0, 1, 2], [3, 4, 5]]),
    ]
    for values in inputs:
        ax = axes()
        ax.heatmap(["a", "b", "c"], ["r", "s"], values)
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1
    names = axes().heatmap(np.array(["a", "b", "c"]), pd.Series(["r", "s"]), np.zeros((2, 3)))
    assert names.figure.to_svg().count("<path") > 0


def test_shape_error_exact() -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.heatmap(["a", "b", "c"], ["p", "q", "r", "s"], np.zeros((3, 4)))
    assert str(caught.value) == (
        "Axes.heatmap(): values has shape (3, 4); it must be "
        "(len(y_categories), len(x_categories)) = (4, 3)."
    )
    assert details(caught.value) == ("values", ROWS_HINT)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("values", "message"),
    [
        (np.zeros(6), "values has 1 dimension; values must be two-dimensional."),
        (np.zeros((1, 2, 3)), "values has 3 dimensions; values must be two-dimensional."),
        ([1.0, 2.0], "values[0] is 1.0; each row must be a sequence."),
        (
            [[1.0, 2.0], [3.0]],
            "values[1] has 1 values but values[0] has 2; every row must have the same length.",
        ),
        ([[1.0, None], [3.0, 4.0]], "values[0][1] is None; values must be numbers."),
        (
            np.array([[1.0, 2.0], [3.0, "x"]], dtype=object),
            "values[1][1] is 'x'; values must be numbers.",
        ),
        (np.array([[True, False], [True, True]]), "values has dtype bool; values must be numbers."),
        # Rows given as arrays are checked one by one, before they are stacked.
        (
            [np.array([True, False]), np.array([2.0, 3.0])],
            "values[0] has dtype bool; values must be numbers.",
        ),
        (
            [np.array([1.0, 2.0]), np.array(["2026-01-01", "2026-01-02"], dtype="datetime64[D]")],
            "values[1] has dtype datetime64[D]; values must be numbers.",
        ),
        (
            [np.array([1.0, 2.0]), np.array(["3", "x"])],
            "values[1][0] is '3'; values must be numbers.",
        ),
        ([np.array([1.0, 2.0]), [3.0, "x"]], "values[1][1] is 'x'; values must be numbers."),
        (
            [np.array([1.0, 2.0]), np.array([3.0, None], dtype=object)],
            "values[1][1] is None; values must be numbers.",
        ),
    ],
)
def test_values_that_are_not_a_matrix_of_numbers(values: object, message: str) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.heatmap(["a", "b"], ["r", "s"], values)
    assert str(caught.value) == f"Axes.heatmap(): {message}"
    found = details(caught.value)
    assert found is not None and found[0] == "values"
    assert ax.figure.to_svg() == before


def test_rows_of_arrays_convert_one_by_one() -> None:
    reference = axes().heatmap(["a", "b"], ["r", "s"], [[1.0, 2.0], [3.0, 4.0]])
    mixed = axes().heatmap(
        ["a", "b"],
        ["r", "s"],
        [np.array([1, 2], dtype=np.int8), np.array([3.0, 4.0], dtype=np.float32)],
    )
    objects = axes().heatmap(["a", "b"], ["r", "s"], [np.array([1, 2.0], dtype=object), [3, 4]])
    assert mixed.figure.to_svg() == reference.figure.to_svg()
    assert objects.figure.to_svg() == reference.figure.to_svg()


def test_labels_that_do_not_fit_their_cells_are_refused() -> None:
    values = [[12345.6789 if i % 2 == 0 else float("nan") for i in range(30)]]
    ax = ub.Figure(3, 2).add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    ax.heatmap(
        [f"c{i}" for i in range(30)], ["r"], values, annotate=True, format=ub.TickFormat.fixed(4)
    )
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    message = str(caught.value)
    assert message.startswith('Figure.to_svg(): the heatmap label "12345.6789" is '), message
    assert message.endswith(" pt wide."), message
    found = details(caught.value)
    assert found is not None and found[0] is None
    assert found[1] is not None and "annotate=False" in found[1]


def test_core_errors_name_cells_and_categories() -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.heatmap(["a", "b"], ["r", "s"], [[1.0, 2.0], [np.inf, 3.0]])
    assert str(caught.value) == "Axes.heatmap(): values[1][0] is inf; values must be finite."
    assert details(caught.value) == ("values", "use NaN to leave a cell empty.")
    with pytest.raises(ub.DataError) as caught:
        ax.heatmap(["a", "b", "a"], ["r"], [[1.0, 2.0, 3.0]])
    assert str(caught.value) == (
        'Axes.heatmap(): x_categories[2] repeats "a" from x_categories[0]; each category must '
        "appear once."
    )
    assert details(caught.value) == ("x_categories", "give every column its own name.")
    with pytest.raises(ub.DataError) as caught:
        ax.heatmap(["a"], ["r", "r"], [[1.0], [2.0]], scale=ub.Scale.log())
    assert details(caught.value) == ("y_categories", "give every row its own name.")
    with pytest.raises(ub.DataError) as caught:
        ax.heatmap(["a", "b"], ["r"], [[1.0, 0.0]], scale=ub.Scale.log())
    assert str(caught.value) == (
        "Axes.heatmap(): values[0][1] is 0; values on a log scale must be positive."
    )
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize(
    ("x", "y", "axis", "scale"),
    [
        (ub.Scale.linear(), ub.Scale.category(), "x", "linear"),
        (ub.Scale.category(), ub.Scale.linear(), "y", "linear"),
        (ub.Scale.category(), ub.Scale.time(), "y", "time"),
        (ub.Scale.log(), ub.Scale.category(), "x", "log"),
    ],
)
def test_other_axes_are_refused_before_the_data(
    x: ub.Scale[Any], y: ub.Scale[Any], axis: str, scale: str
) -> None:
    ax = ub.Figure().add_axes(x=x, y=y)
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        # The values have the wrong shape too: the axes are reported first.
        call(ax.heatmap)(["a"], ["r"], [[1.0, 2.0]])
    assert str(caught.value) == (
        f"Axes.heatmap(): {axis} is a {scale} scale; Axes.heatmap() needs a category {axis} axis."
    )
    found = details(caught.value)
    assert found == (axis, "add the axes with x=Scale.category() and y=Scale.category().")
    assert ax.figure.to_svg() == before


def test_other_marks_still_refuse_two_category_axes() -> None:
    ax = axes()
    with pytest.raises(ub.UsageError):
        call(ax.scatter)(["a"], ["b"])
    with pytest.raises(ub.UsageError):
        cast(Any, ax).bar(["a"], [1.0])


@pytest.mark.parametrize(
    ("parameter", "value"),
    [
        ("x_categories", "abc"),
        ("y_categories", 3),
        ("values", "abc"),
        ("values", {1, 2}),
        ("cmap", "viridis"),
        ("scale", ub.Scale.category()),
        ("scale", ub.Scale.time()),
        ("scale", "log"),
        ("center", "0"),
        ("annotate", 1),
        ("format", "0.00"),
    ],
)
def test_wrong_types_are_atomic(parameter: str, value: object) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    arguments: dict[str, object] = {
        "x_categories": ["a"],
        "y_categories": ["r"],
        "values": [[1.0]],
        parameter: value,
    }
    with pytest.raises(TypeError) as caught:
        call(ax.heatmap)(**arguments)
    found = details(caught.value)
    assert found is not None and found[0] == parameter
    assert ax.figure.to_svg() == before


def test_scale_time_is_refused_by_color_by_too() -> None:
    with pytest.raises(TypeError) as caught:
        call(ub.ColorBy)([1.0], scale=ub.Scale.time())
    assert str(caught.value) == (
        "ColorBy(): scale is Scale.time(); scale must be Scale.linear() or Scale.log()."
    )


def test_annotations_flip_between_black_and_white() -> None:
    ax = axes().heatmap(
        ["low", "high"],
        ["row"],
        [[0.0, 1.0]],
        cmap=GRAYS,
        annotate=True,
        format=ub.TickFormat.fixed(1),
    )
    # The colorbar's tick labels are drawn in the theme's muted color.
    notes = [(text, fill) for text, fill in texts(ax) if fill in ("#000000", "#FFFFFF")]
    assert notes == [("0.0", "#FFFFFF"), ("1.0", "#000000")]


def test_automatic_annotations_follow_the_colorbar_ticks() -> None:
    ax = axes().heatmap(["a", "b", "c"], ["r"], [[0.0, 0.7341, 1.0]], annotate=True)
    labels = [text for text, _ in texts(ax)]
    assert "0.73" in labels and "0.7341" not in labels


def test_nan_cells_are_empty_and_unlabelled() -> None:
    full = axes().heatmap(["a", "b"], ["r"], [[1.0, 2.0]], annotate=True)
    holed = axes().heatmap(["a", "b"], ["r"], [[1.0, float("nan")]], annotate=True)
    assert holed.figure.to_svg().count("<path") < full.figure.to_svg().count("<path")
    assert "NaN" not in [text for text, _ in texts(holed)]


def test_format_needs_annotate_and_numbers() -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.heatmap(["a"], ["r"], [[1.0]], format=ub.TickFormat.percent())
    assert str(caught.value) == (
        "Axes.heatmap(): format is given but annotate is false; only annotated cells show "
        "their values."
    )
    assert details(caught.value) == ("format", "pass annotate=True, or leave format out.")
    with pytest.raises(ub.StyleError) as refused:
        ax.heatmap(["a"], ["r"], [[1.0]], annotate=True, format=ub.TickFormat.iso())
    found = details(refused.value)
    assert found is not None and found[0] == "format"
    assert ax.figure.to_svg() == before


def test_heatmaps_share_the_color_scale_of_the_axes() -> None:
    ax = axes().heatmap(["a"], ["r"], [[1.0]], cmap=ub.Cmap.VIRIDIS)
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        ax.heatmap(["b"], ["r"], [[2.0]], cmap=ub.Cmap.MAGMA)
    assert str(caught.value) == (
        "Axes.heatmap(): cmap is Cmap.MAGMA; this axes already maps colors with Cmap.VIRIDIS."
    )
    found = details(caught.value)
    assert found is not None and found[0] == "cmap"
    with pytest.raises(ub.UsageError) as caught:
        ax.heatmap(["b"], ["r"], [[2.0]], cmap=ub.Cmap.VIRIDIS, scale=ub.Scale.log())
    found = details(caught.value)
    assert found is not None and found[0] == "scale"
    assert ax.figure.to_svg() == before
    ax.heatmap(["b"], ["s"], [[2.0]], cmap=ub.Cmap.VIRIDIS)
    assert ax.figure.to_svg() != before


def test_clim_and_center() -> None:
    ax = axes().clim(0, 10)
    with pytest.raises(ub.StyleError) as caught:
        ax.heatmap(["a"], ["r"], [[1.0]], center=12)
    assert str(caught.value) == (
        "Axes.heatmap(): center is 12; it must lie inside the color range 0 to 10."
    )
    found = details(caught.value)
    assert found is not None and found[0] == "center"
    clamped = axes().clim(0, 10).heatmap(["a", "b"], ["r"], [[-50.0, 50.0]], cmap=GRAYS)
    reference = axes().heatmap(["a", "b"], ["r"], [[0.0, 10.0]], cmap=GRAYS)
    clamped.colorbar(position=ub.ColorbarPosition.NONE)
    reference.colorbar(position=ub.ColorbarPosition.NONE)
    assert clamped.figure.to_svg() == reference.figure.to_svg()


@pytest.mark.parametrize("theme", [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT])
def test_centered_heatmap_renders_in_every_theme(theme: ub.Theme) -> None:
    rng = np.random.default_rng(5)
    data = np.corrcoef(rng.normal(size=(4, 50)))
    names = ["w", "x", "y", "z"]
    ax = axes(theme).heatmap(names, names, data, center=0, annotate=True)
    ax.colorbar("Correlation")
    png = ax.figure.to_png()
    assert png.startswith(b"\x89PNG")
    assert ("1.0", "#000000") in texts(ax) or ("1.0", "#FFFFFF") in texts(ax)
