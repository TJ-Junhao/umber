"""Bubble conversions, exact errors, highlights, legend formats and binding parity."""

from collections.abc import Callable
from pathlib import Path
from typing import Any, cast
from xml.etree import ElementTree

import numpy as np
import numpy.typing as npt
import pandas as pd
import pytest

import umber as ub
from umber import _umber
from umber._errors import details


def axes() -> ub.Axes[float, float]:
    return ub.Figure(6, 4).add_axes()


def bubbles(ax: ub.Axes[Any, Any]) -> Callable[..., object]:
    return cast(Callable[..., object], ax.bubbles)


def test_return_self_and_keyword_only_options() -> None:
    ax = axes()
    assert ax.bubbles([0, 1], [1, 2], [100, 870]) is ax
    assert ub.Axes.bubbles.__module__ == "umber"
    assert not hasattr(ub, "bubbles")
    with pytest.raises(TypeError):
        bubbles(ax)([0], [1], [100], "Cities")


def test_lists_arrays_and_series_match() -> None:
    outputs: set[str] = set()
    for sizes in [
        [100, 870],
        (100, 870),
        np.array([100, 870], dtype=np.int16),
        pd.Series([100, 870]),
    ]:
        ax = axes()
        bubbles(ax)([0, 1], [1, 2], sizes, label="Cities")
        outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


@pytest.mark.parametrize(
    ("value", "text"),
    [(-1.0, "-1"), (np.inf, "inf"), (-np.inf, "-inf")],
)
def test_invalid_sizes_are_atomic(value: float, text: str) -> None:
    ax = axes().bubbles([0, 1], [1, 2], [100, 870], label="Cities")
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.bubbles([0, 1, 2], [1, 2, 3], [0, value, -2])
    assert str(caught.value) == (
        f"Axes.bubbles(): sizes[1] is {text}; bubble sizes must be finite and at least 0."
    )
    assert details(caught.value) == ("sizes", "use 0 for a point to leave out.")
    assert ax.figure.to_svg() == before


def _gapped_pair() -> tuple[ub.Axes[float, float], ub.Axes[float, float]]:
    """Axes with a far-off bubble of NaN size, and axes with the same bubbles without it."""
    with_gap = axes().bubbles([1, 5000, 2], [2, -5000, 4], [100, np.nan, 400], label="Cities")
    without = axes().bubbles([1, 2], [2, 4], [100, 400], label="Cities")
    return with_gap, without


def _same_figure(with_gap: ub.Axes[Any, Any], without: ub.Axes[Any, Any]) -> bool:
    return with_gap.figure.to_svg() == without.figure.to_svg()


def test_nan_size_leaves_the_bubble_out_of_the_figure() -> None:
    assert _same_figure(*_gapped_pair())


def _polars_nullable() -> object:
    pl = pytest.importorskip("polars")
    return pl.Series([100.0, None, 400.0])


@pytest.mark.parametrize(
    "sizes",
    [
        [100, np.nan, 400],
        (100.0, float("nan"), 400.0),
        np.array([100, np.nan, 400]),
        pd.Series([100.0, np.nan, 400.0]),
        pd.Series([100.0, None, 400.0], dtype="Float64"),
        pd.Series([100, None, 400], dtype="Int64"),
        _polars_nullable,
    ],
    ids=["list", "tuple", "numpy", "pandas", "pandas-Float64", "pandas-Int64", "polars"],
)
def test_missing_sizes_leave_their_bubbles_out(sizes: object) -> None:
    if callable(sizes):
        sizes = sizes()
    gapped = axes()
    bubbles(gapped)([1, 5000, 2], [2, -5000, 4], sizes, label="Cities")
    plain = axes().bubbles([1, 2], [2, 4], [100, 400], label="Cities")
    assert _same_figure(gapped, plain)


def test_none_in_a_list_of_sizes_still_raises_and_names_the_gap_marker() -> None:
    with pytest.raises(ub.DataError) as caught:
        bubbles(axes())([0, 1, 2], [1, 2, 3], [100, None, 400])
    assert str(caught.value) == "Axes.bubbles(): sizes[1] is None; values must be numbers."
    assert details(caught.value) == (
        "sizes",
        "use NaN for missing values, e.g. series.to_numpy(dtype=float, na_value=np.nan).",
    )


@pytest.mark.parametrize(
    ("y", "sizes", "message"),
    [
        ([], [], "x has 1 values but y has 0; x and y must have the same length."),
        ([1], [], "x has 1 values but sizes has 0; x and sizes must have the same length."),
    ],
)
def test_lengths_precede_coordinate_values(
    y: list[float], sizes: list[float], message: str
) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.bubbles([np.inf], y, sizes)
    assert str(caught.value) == f"Axes.bubbles(): {message}"
    assert details(caught.value) == (None, None)
    assert ax.figure.to_svg() == before


def test_category_x_is_rejected_atomically() -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category())
    before = ax.figure.to_svg()
    with pytest.raises(ub.UsageError) as caught:
        bubbles(ax)([], [], [])
    assert str(caught.value) == (
        "Axes.bubbles(): this is a category axis; bubbles apply to linear and log axes only."
    )
    assert details(caught.value) == (
        "x",
        "pass x=Scale.linear() or x=Scale.log() to Figure.add_axes().",
    )
    assert ax.figure.to_svg() == before


class RecordsConversion:
    def __init__(self) -> None:
        self.converted = False

    def __array__(self, dtype: object = None, copy: object = None) -> npt.NDArray[np.float64]:
        self.converted = True
        return np.array([1.0])


def test_type_checks_precede_conversion() -> None:
    data = RecordsConversion()
    with pytest.raises(TypeError):
        bubbles(axes())(data, [1], [1], marker="circle")
    assert not data.converted
    with pytest.raises(TypeError) as caught:
        bubbles(axes())(["bad"], [1], None)
    assert (
        str(caught.value) == "Axes.bubbles(): sizes is None; sizes must be a sequence or an array."
    )
    with pytest.raises(ub.StyleError) as long_label:
        axes().bubbles([0], [1], [1], label="x" * 1001)
    assert details(long_label.value) == ("label", "shorten the text; it would not fit the figure.")


@pytest.mark.parametrize("position", list(ub.LegendPosition))
@pytest.mark.parametrize("theme", [ub.Theme.PIGMENT, ub.Theme.GRAPHITE, ub.Theme.MIDNIGHT])
def test_size_references_in_every_position(position: ub.LegendPosition, theme: ub.Theme) -> None:
    ax = ub.Figure(6, 4, theme=theme).add_axes()
    ax.legend(ub.LegendStyle.HEADER, position=position).bubbles(
        [1, 2], [2, 3], [100, 870], label="Cities"
    )
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    for label in ["Cities", "500", "200", "100"]:
        assert f">{label}</text>" in svg


def test_y_tick_format_applies_to_size_references() -> None:
    ax = axes().bubbles([1, 2], [0.5, 1], [0.5, 1], label="Share")
    ax.ytick_format(ub.TickFormat.percent(0).affix(prefix="~", suffix=" share"))
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    for label in ["~100% share", "~50% share", "~20% share"]:
        assert f">{label}</text>" in svg


@pytest.mark.parametrize(
    ("maximum", "tick_format", "expected"),
    [
        (1.0, ub.TickFormat.fixed(0), ["1"]),
        (0.9, ub.TickFormat.fixed(0), ["0.5"]),
        (0.9, ub.TickFormat.fixed(0).affix(prefix="~", suffix=" units"), ["0.5"]),
        (0.09, ub.TickFormat.fixed(2), ["0.05", "0.02", "0.01"]),
        (0.01, ub.TickFormat.percent(0).affix(prefix="~", suffix=" share"), ["~1% share"]),
        (0.009, ub.TickFormat.percent(0).affix(prefix="~", suffix=" share"), ["0.005"]),
    ],
)
def test_size_legend_formats_keep_nonzero_distinct_labels(
    maximum: float, tick_format: ub.TickFormat, expected: list[str]
) -> None:
    ax = axes().bubbles([1], [2], [maximum], label="S").ytick_format(tick_format)
    ax.xticks([]).yticks([])
    svg = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    labels = [element.text for element in svg.iter("{http://www.w3.org/2000/svg}text")]
    assert labels == ["S", *expected]


@pytest.mark.parametrize("bits", range(1, 9))
@pytest.mark.parametrize("tick_format", [ub.TickFormat.fixed(0), ub.TickFormat.percent(0)])
def test_subnormal_size_legend_uses_one_nonzero_automatic_label(
    bits: int, tick_format: ub.TickFormat
) -> None:
    maximum = bits * float.fromhex("0x0.0000000000001p-1022")
    ax = axes().bubbles([1], [2], [maximum], label="S").ytick_format(tick_format)
    ax.xticks([]).yticks([])
    svg = ElementTree.fromstring(ax.figure.to_svg(text=ub.SvgText.EDITABLE))
    labels = [element.text for element in svg.iter("{http://www.w3.org/2000/svg}text")]
    assert len(labels) == 2
    assert labels[0] == "S"
    assert labels[1] is not None
    assert 0 < float(labels[1].replace("−", "-")) <= maximum


def test_empty_zero_and_nan_coordinates_render() -> None:
    ax = axes().xlim(0, 3).ylim(0, 3)
    before = ax.figure.to_svg()
    ax.bubbles([], [], []).bubbles([1, 2], [1, 2], [0, 0])
    ax.bubbles([np.nan, 1], [1, np.nan], [1, 1])
    assert ax.figure.to_svg() == before
    assert axes().bubbles([2, 0, 1], [1, 2, 3], [1, 2, 3]).figure.to_svg()


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.bubbles(t, t, sizes)", "sizes"),
        ("ax.bubbles(t, t, sizes=sizes)", "sizes=sizes"),
        ("ax.bubbles(t, t, t, marker='circle')", "marker='circle'"),
        ("ax.bubbles(t, t, t, color='red')", "color='red'"),
    ],
)
def test_highlight_names_the_argument(tmp_path: Path, call: str, argument: str) -> None:
    source = tmp_path / "user_code.py"
    source.write_text(
        "import umber as ub\nt = [1.0, 2.0]\nsizes = [1.0, -1.0]\n"
        f"ax = ub.Figure().add_axes()\n{call}\n"
    )
    with pytest.raises((TypeError, ub.DataError)) as caught:
        exec(compile(source.read_text(), str(source), "exec"), {})
    expected = f"  {call}\n{' ' * (2 + call.index(argument))}{'^' * len(argument)}"
    assert list(getattr(caught.value, "__notes__", []))[0] == expected


@pytest.mark.parametrize(
    "maximum",
    [
        float.fromhex("0x0.0000000000001p-1022"),
        1e-300,
        1e300,
    ],
)
def test_finite_size_extremes_render(maximum: float) -> None:
    ax = axes().bubbles([1, 2], [2, 3], [maximum / 4, maximum], label="Sizes")
    assert ax.figure.to_svg()
    assert ax.figure.to_png(dpi=60)


def test_size_magnitude_limit_is_atomic() -> None:
    ax = axes().bubbles([1], [2], [1e300], label="Sizes")
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.bubbles([1], [2], [2e300])
    assert str(caught.value) == (
        "Axes.bubbles(): sizes[0] is 2e300; magnitudes must not exceed 1e300."
    )
    assert details(caught.value) == ("sizes", None)
    assert ax.figure.to_svg() == before


def test_log_axes_and_coordinate_errors_are_atomic() -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.log())
    ax.bubbles([1, 10], [10, 100], [0, 1], label="Log")
    before = ax.figure.to_svg()
    for parameter in ["x", "y"]:
        x, y = ([0.0], [1.0]) if parameter == "x" else ([1.0], [0.0])
        with pytest.raises(ub.DataError) as caught:
            ax.bubbles(x, y, [1])
        assert (
            str(caught.value)
            == f"Axes.bubbles(): {parameter}[0] is 0 on a log axis; values must be positive."
        )
        assert details(caught.value) == (
            parameter,
            "use a linear axis, or remove values that are not positive.",
        )
        assert ax.figure.to_svg() == before
        x, y = ([np.inf], [1.0]) if parameter == "x" else ([1.0], [np.inf])
        with pytest.raises(ub.DataError) as caught:
            ax.bubbles(x, y, [1])
        assert str(caught.value) == f"Axes.bubbles(): {parameter}[0] is inf; values must be finite."
        assert details(caught.value) == (parameter, "use NaN to leave a point out.")
        assert ax.figure.to_svg() == before


@pytest.mark.parametrize("marker", list(ub.Marker))
def test_native_rendering_parity(marker: ub.Marker) -> None:
    fig = ub.Figure(6, 4)
    fig.add_axes().bubbles([1, 2], [2, 3], [100, 870], label="Cities", marker=marker)
    native = _umber.Figure(6.0, 4.0, _umber.Theme.pigment(), 1, 1, 0, 0, 0, 0)
    native.add_axes(0, 0, 0, 0, 1, 1, 0, 10.0, 0, 10.0)
    native.bubbles(
        0,
        0,
        np.array([1.0, 2.0]),
        np.array([2.0, 3.0]),
        np.array([100.0, 870.0]),
        "Cities",
        (0, 0, 1.0),
        marker.value,
    )
    assert fig.to_svg() == native.to_svg(0)
    assert fig.to_png(dpi=60) == native.to_png(60)


def test_bubbles_too_large_for_their_plot_raise_with_a_figure_size_that_holds_them() -> None:
    def axes_on(width: float, height: float) -> ub.Axes[float, float]:
        ax = ub.Figure(width, height).add_axes().legend(ub.LegendStyle.NONE)
        return ax.xticks([]).yticks([]).bubbles([0, 1], [0, 1], [1, 4], label="Cities")

    with pytest.raises(ub.StyleError) as caught:
        axes_on(3, 0.75).figure.to_png()
    assert str(caught.value) == (
        'Figure.to_png(): the bubbles "Cities" need a figure at least 216 × 61 pt; '
        "the figure is 216 × 54 pt."
    )
    assert details(caught.value) == (
        None,
        "make the figure larger, or set explicit limits, which clip the bubbles.",
    )
    # The figure of the named size holds them; explicit limits clip them instead.
    assert axes_on(3, 61 / 72).figure.to_png().startswith(b"\x89PNG")
    clipped = axes_on(3, 0.75).xlim(-1, 2).ylim(-1, 2)
    assert clipped.figure.to_png().startswith(b"\x89PNG")


def _bare(width: float, height: float) -> ub.Axes[float, float]:
    return ub.Figure(width, height).add_axes().legend(ub.LegendStyle.NONE).xticks([]).yticks([])


def test_bubbles_without_a_color_are_not_fitted() -> None:
    # The large bubble's value maps to no color, so it is not drawn; the small one fits.
    color = ub.ColorBy([float("nan"), 1])
    ax = _bare(0.75, 0.75).bubbles([0, 0], [0, 0], [10000, 1], color=color)
    ax.colorbar(position=ub.ColorbarPosition.NONE)
    assert ax.figure.to_png().startswith(b"\x89PNG")


def test_mapped_outlines_fit_a_plot_the_widest_outline_would_not() -> None:
    # A 36.5 pt mapped circle with its 0.5 pt outline fits the 36.6 pt plot.
    ax = _bare(60.6 / 72, 3).bubbles([0], [0], [1], color=ub.ColorBy([1]))
    ax.colorbar(position=ub.ColorbarPosition.NONE)
    assert ax.figure.to_png().startswith(b"\x89PNG")
