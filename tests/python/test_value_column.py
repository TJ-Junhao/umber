"""Values written in a column beside the plot: placement, sub-columns, the title and errors."""

from collections.abc import Callable
from typing import Any, cast
from xml.etree import ElementTree

import pytest

import umber as ub
from umber._errors import details

SVG = "{http://www.w3.org/2000/svg}"
MODELS = ["Baseline", "Dropout", "Gate", "Gate + aug"]
SHORT = [66.5, 69.14, 71.64, 73.06]
LONG = [68.08, 70.7, 74.0, 75.4]


def call(function: object) -> Callable[..., object]:
    return cast(Callable[..., object], function)


def texts(ax: ub.Axes[Any, Any]) -> dict[str, ElementTree.Element]:
    """Every editable SVG text of the figure of ``ax`` by its content; the last one wins."""
    root = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    return {node.text or "": node for node in root.iter(f"{SVG}text")}


def x_of(node: ElementTree.Element) -> float:
    return float(node.get("x") or "nan")


def forest(title: str | None = "Mean (%)") -> ub.Axes[float, str]:
    ax = ub.Figure(6.4, 3.6).add_axes(y=ub.Scale.category())
    for label, means in [("80 epochs", SHORT), ("Long training", LONG)]:
        ax = ax.scatter(
            means,
            MODELS,
            label=label,
            annotate=True,
            format=ub.TickFormat.fixed(1),
            place=ub.Place.COLUMN,
        )
    if title is not None:
        ax = ax.value_column(title)
    return ax


def test_values_go_in_sub_columns_right_of_the_plot_in_call_order() -> None:
    found = texts(forest())
    assert {"66.5", "68.1", "Mean (%)"} <= set(found)
    assert x_of(found["66.5"]) < x_of(found["68.1"])
    # Right of the y tick labels, which start at the left of the figure.
    assert x_of(found["66.5"]) > x_of(found["Baseline"])


def test_groups_with_by_each_get_a_sub_column() -> None:
    ax = ub.Figure(6.4, 3.6).add_axes(y=ub.Scale.category())
    ax = ax.scatter(
        SHORT + LONG,
        MODELS + MODELS,
        by=["80 epochs"] * 4 + ["Long training"] * 4,
        annotate=True,
        format=ub.TickFormat.fixed(1),
        place=ub.Place.COLUMN,
    ).value_column("Mean (%)")
    found = texts(ax)
    assert x_of(found["69.1"]) < x_of(found["70.7"])


def test_values_on_y_go_in_a_row_above_the_plot() -> None:
    ax = ub.Figure(6.4, 3.8).add_axes(x=ub.Scale.category())
    ax = ax.scatter(
        ["Fold 1", "Fold 2", "Fold 3"],
        [66.8, 68.1, 65.4],
        annotate=True,
        place=ub.Place.COLUMN,
    ).value_column("Per fold")
    found = texts(ax)
    assert {"66.8", "68.1", "65.4", "Per fold"} <= set(found)
    assert float(found["66.8"].get("y") or "nan") < float(found["Fold 1"].get("y") or "nan")


def test_a_second_call_replaces_the_title() -> None:
    ax = forest("Mean").value_column("Mean (%)", size=ub.Size.pt(13))
    found = texts(ax)
    assert "Mean" not in found and "Mean (%)" in found
    assert "13" in (found["Mean (%)"].get("font-size") or "")


def test_value_column_checks_its_arguments_when_called() -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.category())
    with pytest.raises(TypeError) as caught:
        call(ax.value_column)(3)
    info = details(caught.value)
    assert info is not None and info[0] == "title"
    with pytest.raises(TypeError) as caught:
        call(ax.value_column)("Mean", size=12)
    info = details(caught.value)
    assert info is not None and info[0] == "size"
    with pytest.raises(ub.StyleError) as style:
        ax.value_column("Mean\n(%)")
    assert "value column titles are one line" in str(style.value)


def test_a_title_without_a_column_scatter_raises_when_rendered() -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.category())
    ax = ax.scatter(LONG, MODELS, annotate=True).value_column("Mean (%)")
    with pytest.raises(ub.UsageError) as caught:
        ax.figure.to_svg()
    assert "none of their scatters writes its values in the column" in str(caught.value)
    info = details(caught.value)
    assert info is not None and "place=Place.COLUMN" in (info[1] or "")


def test_overlapping_values_of_one_sub_column_raise_when_rendered() -> None:
    ax = ub.Figure().add_axes(y=ub.Scale.category())
    ax = ax.scatter(
        [66.5, 71.0], ["Base", "Base"], label="80 epochs", annotate=True, place=ub.Place.COLUMN
    )
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_png()
    message = str(caught.value)
    assert '"66.5" and "71"' in message and '"80 epochs"' in message
    info = details(caught.value)
    assert info is not None and "taller" in (info[1] or "")


def test_twin_axes_refuse_a_column() -> None:
    twin = ub.Figure().add_axes().twin_y()
    with pytest.raises(ub.UsageError) as caught:
        twin.scatter([1], [2], annotate=True, place=ub.Place.COLUMN)
    info = details(caught.value)
    assert info is not None and info[0] == "place"


def test_quick_scatter_takes_a_column() -> None:
    ax = ub.scatter([1, 2, 3], [3.25, 4.75, 6.5], annotate=True, place=ub.Place.COLUMN)
    assert {"3.25", "4.75", "6.5", "Value"} <= set(texts(ax.value_column("Value")))


def test_png_svg_and_pdf_agree() -> None:
    ax = forest()
    editable = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert ">66.5</text>" in editable and ">Mean (%)</text>" in editable
    assert ax.figure.to_png().startswith(b"\x89PNG")
    assert ax.figure.to_pdf().startswith(b"%PDF")
    again = forest()
    assert again.figure.to_svg() == ax.figure.to_svg()
    assert again.figure.to_png() == ax.figure.to_png()


def test_row_values_that_leave_the_figure_raise_when_rendered() -> None:
    ax = ub.Figure(4, 3).add_axes().xlim(0, 1)
    ax = ax.scatter(
        [0, 1],
        [123.456, 789.012],
        annotate=True,
        place=ub.Place.COLUMN,
        format=ub.TickFormat.fixed(3).affix("Measured = ", " units"),
    )
    with pytest.raises(ub.StyleError) as caught:
        ax.figure.to_svg()
    assert "reaches outside the figure" in str(caught.value)
