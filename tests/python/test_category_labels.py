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
