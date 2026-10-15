"""Text and binary buffers are not data containers."""

from collections.abc import Callable
from typing import Any, cast

import numpy as np
import pytest

import umber as ub
from umber._errors import details
from umber._values import (
    axis_values,
    check_axis_type,
    check_floats_type,
    check_names_type,
    floats,
    names,
)


@pytest.fixture(params=[bytearray, memoryview], ids=["bytearray", "memoryview"])
def buffer_type(request: pytest.FixtureRequest) -> Callable[[bytes], object]:
    return cast(Callable[[bytes], object], request.param)


@pytest.mark.parametrize("contents", [b"", b"x"])
def test_distribution_rejects_binary_groups(
    buffer_type: Callable[[bytes], object], contents: bytes
) -> None:
    groups = buffer_type(contents)
    with pytest.raises(TypeError) as caught:
        ub.Distribution("N", cast(Any, groups))
    assert str(caught.value) == (
        f"Distribution(): groups is {type(groups).__name__}; "
        "groups must be a sequence of numeric groups."
    )
    assert details(caught.value) == ("groups", "pass a list of groups, e.g. [[1, 2], [3, 4]].")


@pytest.mark.parametrize("mark", ["box", "violin"])
@pytest.mark.parametrize("contents", [b"", b"x"])
def test_marks_reject_binary_groups(
    buffer_type: Callable[[bytes], object], contents: bytes, mark: str
) -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category())
    before = ax.figure.to_svg()
    groups = buffer_type(contents)
    with pytest.raises(TypeError) as caught:
        getattr(ax, mark)(["A"], groups)
    assert str(caught.value) == (
        f"Axes.{mark}(): groups is {type(groups).__name__}; "
        "groups must be a sequence of numeric groups."
    )
    assert vars(caught.value)["_umber_parameter"] == "groups"
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("mark", ["grouped_bar", "stacked_bar", "stacked_area"])
@pytest.mark.parametrize("contents", [b"", b"x"])
def test_marks_reject_binary_series_before_converting_data(
    buffer_type: Callable[[bytes], object], contents: bytes, mark: str
) -> None:
    fig = ub.Figure()
    ax: ub.Axes[Any, Any] = (
        fig.add_axes() if mark == "stacked_area" else fig.add_axes(x=ub.Scale.category())
    )
    before = ax.figure.to_svg()
    series = buffer_type(contents)
    with pytest.raises(TypeError) as caught:
        getattr(ax, mark)([123], series)
    assert str(caught.value) == (
        f"Axes.{mark}(): series is {type(series).__name__}; series must be a sequence of Series."
    )
    assert vars(caught.value)["_umber_parameter"] == "series"
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("mark", ["grouped_box", "grouped_violin"])
@pytest.mark.parametrize("contents", [b"", b"x"])
def test_marks_reject_binary_distributions_before_converting_data(
    buffer_type: Callable[[bytes], object], contents: bytes, mark: str
) -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category())
    distributions = buffer_type(contents)
    with pytest.raises(TypeError) as caught:
        getattr(ax, mark)([123], distributions)
    assert str(caught.value) == (
        f"Axes.{mark}(): distributions is {type(distributions).__name__}; "
        "distributions must be a sequence of Distribution."
    )
    assert vars(caught.value)["_umber_parameter"] == "distributions"


@pytest.mark.parametrize("contents", [b"", b"x"])
def test_palette_rejects_binary_colors(
    buffer_type: Callable[[bytes], object], contents: bytes
) -> None:
    colors = buffer_type(contents)
    with pytest.raises(TypeError) as caught:
        ub.Theme.PIGMENT.with_palette(cast(Any, colors))
    assert str(caught.value) == (
        f"Theme.with_palette(): colors is {type(colors).__name__}; "
        "colors must be a sequence of Color."
    )
    assert details(caught.value) == (
        "colors",
        "pass a list of colors, e.g. [Color.RED, Color.BLUE].",
    )


@pytest.mark.parametrize(
    "convert", [floats, names, axis_values, check_floats_type, check_names_type, check_axis_type]
)
@pytest.mark.parametrize("contents", [b"", b"x"])
def test_data_arguments_reject_binary_containers(
    buffer_type: Callable[[bytes], object],
    contents: bytes,
    convert: Callable[[str, str, object], object],
) -> None:
    values = buffer_type(contents)
    with pytest.raises(TypeError) as caught:
        convert("Axes.line()", "x", values)
    assert str(caught.value) == (
        f"Axes.line(): x is {type(values).__name__}; x must be a sequence or an array."
    )
    assert vars(caught.value)["_umber_parameter"] == "x"


@pytest.mark.parametrize("convert", [floats, axis_values])
def test_numeric_memoryviews_are_not_data_containers(
    convert: Callable[[str, str, object], object],
) -> None:
    with pytest.raises(TypeError, match="x is memoryview; x must be a sequence or an array"):
        convert("Axes.line()", "x", memoryview(np.array([1.0, 2.0])))


@pytest.mark.parametrize("convert", [floats, axis_values])
def test_binary_elements_are_not_nested_sequences(
    buffer_type: Callable[[bytes], object], convert: Callable[[str, str, object], object]
) -> None:
    value = buffer_type(b"x")
    with pytest.raises(ub.DataError) as caught:
        convert("Axes.line()", "x", [value])
    assert str(caught.value) == (
        f"Axes.line(): x[0] is {type(value).__name__}; values must be numbers."
    )
    assert details(caught.value) == ("x", "convert the values with float() first.")
