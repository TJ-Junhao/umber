"""Category label thinning through the public Python API."""

from xml.etree import ElementTree

import pytest

import umber as ub


def labels(figure: ub.Figure) -> list[str]:
    root = ElementTree.fromstring(figure.to_svg(text=ub.SvgText.EDITABLE))
    return [node.text or "" for node in root.iter("{http://www.w3.org/2000/svg}text")]


@pytest.mark.parametrize("horizontal", [False, True])
def test_dense_bars_label_round_category_steps(horizontal: bool) -> None:
    figure = ub.Figure(6, 4)
    categories = [f"c{i}" for i in range(200)]
    if horizontal:
        figure.add_axes(y=ub.Scale.category()).bar(categories, [1.0] * 200)
    else:
        figure.add_axes(x=ub.Scale.category()).bar(categories, [1.0] * 200)
    actual = [text for text in labels(figure) if text.startswith("c")]
    assert actual == categories[::20]


def test_category_thinning_keeps_all_heatmap_annotations() -> None:
    figure = ub.Figure(6, 4)
    categories = [f"category {i}" for i in range(20)]
    figure.add_axes(x=ub.Scale.category(), y=ub.Scale.category()).heatmap(
        categories, ["row"], [[7.0] * 20], annotate=True, format=ub.TickFormat.fixed(0)
    ).colorbar(position=ub.ColorbarPosition.NONE)
    text = labels(figure)
    assert text.count("7") == 20
    assert 1 < sum(label.startswith("category ") for label in text) < 20
    assert "category 0" in text


def test_explicit_tick_labels_remain_crowded_and_complete() -> None:
    figure = ub.Figure(6, 4)
    explicit = [f"given{i}" for i in range(100)]
    figure.add_axes().xticks_labeled(list(range(100)), explicit).yticks_labeled(
        list(range(100)), explicit
    )
    assert labels(figure) == explicit + explicit


def text_nodes(figure: ub.Figure) -> dict[str, tuple[float, float, str]]:
    """Each editable SVG text by its content: its x, its baseline and its text-anchor."""
    root = ElementTree.fromstring(figure.to_svg(text=ub.SvgText.EDITABLE))
    return {
        node.text or "": (
            float(node.get("x", "nan")),
            float(node.get("y", "nan")),
            node.get("text-anchor", "start"),
        )
        for node in root.iter("{http://www.w3.org/2000/svg}text")
    }


def test_a_category_name_breaks_into_lines_under_its_tick() -> None:
    figure = ub.Figure(5, 3)
    figure.add_axes(x=ub.Scale.category()).bar(["2025", "2026\n(to Oct)"], [4.0, 3.0])
    nodes = text_nodes(figure)
    assert "2026\n(to Oct)" not in nodes
    first, second = nodes["2026"], nodes["(to Oct)"]
    # The second line sits one line pitch, 1.2 tick label sizes, below the first; both lines
    # share the first line's baseline row with the label of one line.
    assert second[1] > first[1]
    assert first[1] == pytest.approx(nodes["2025"][1])


def test_a_category_axis_of_several_line_names_leaves_less_room_for_the_plot() -> None:
    def plot_bottom(names: list[str]) -> float:
        figure = ub.Figure(5, 3)
        figure.add_axes(x=ub.Scale.category()).bar(names, [1.0] * len(names)).yticks([0, 1])
        return text_nodes(figure)["0"][1]

    assert plot_bottom(["a", "b\nc"]) < plot_bottom(["a", "b"])


def test_numeric_tick_labels_stay_one_line() -> None:
    with pytest.raises(ub.DataError, match="tick labels are one line"):
        ub.Figure().add_axes().xticks_labeled([0, 1], ["low", "high\nend"])
