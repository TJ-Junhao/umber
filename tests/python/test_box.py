"""Box plots, immutable distributions, conversion errors and exact Rust/NumPy quartiles."""

import copy
import pickle
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pandas as pd
import pytest

import umber as ub
from umber import _umber
from umber._errors import details
from umber._series import native_distribution

GAP_HINT = "use NaN to leave a sample out."
COUNT_HINT = "pass one group per category, using an empty group when there are no samples."
COLOR_HINT = 'use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.'


def axes() -> ub.Axes[str, float]:
    return ub.Figure(4, 3).add_axes(x=ub.Scale.category())


def box(ax: ub.Axes[Any, Any]) -> Callable[..., object]:
    return cast(Callable[..., object], ax.box)


def grouped(ax: ub.Axes[Any, Any]) -> Callable[..., object]:
    return cast(Callable[..., object], ax.grouped_box)


def test_returns_self_and_has_keyword_only_options() -> None:
    ax = axes()
    assert ax.box(["A"], [[1, 2]]) is ax
    assert ax.grouped_box(["A"], [ub.Distribution("B", [[3, 4]])]) is ax
    assert not hasattr(ub, "box")
    assert not hasattr(ub, "box_stats")
    with pytest.raises(TypeError):
        box(ax)(["A"], [[1]], "label")
    with pytest.raises(TypeError):
        cast(Callable[..., object], ub.Distribution)("A", [[1]], ub.Color.RED)


def test_lists_tuples_arrays_and_series_have_identical_output() -> None:
    outputs: set[str] = set()
    for names, groups in [
        (["A", "B"], [[1, 2, 3], [4, 5]]),
        (("A", "B"), (np.array([1, 2, 3], dtype=np.int16), (4, 5))),
        (np.array(["A", "B"]), [pd.Series([1.0, 2.0, 3.0]), np.array([4, 5])]),
    ]:
        for grouped_box in [False, True]:
            ax = axes()
            if grouped_box:
                grouped(ax)(
                    names, [cast(Callable[..., ub.Distribution], ub.Distribution)("", groups)]
                )
            else:
                box(ax)(names, groups)
            outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


def test_input_samples_are_copied_and_nan_is_ignored() -> None:
    source = np.array([1.0, 2.0, 3.0, 4.0])
    d = ub.Distribution("Samples", [source])
    plain = axes().box(["A"], [source], label="Samples")
    before = plain.figure.to_svg()
    source[:] = 100.0
    assert plain.figure.to_svg() == before
    assert axes().grouped_box(["A"], [d]).figure.to_svg() == before
    assert axes().box(["A"], [[1, np.nan, 2, 3, 4]], label="Samples").figure.to_svg() == before


def test_distribution_is_immutable_and_copies_share_it() -> None:
    d = ub.Distribution("North", [[1, 2], []], color=ub.Color.BLUE)
    assert repr(d) == "<umber.Distribution 'North' (2 groups)>"
    assert repr(ub.Distribution("A", [[]])) == "<umber.Distribution 'A' (1 group)>"
    assert not hasattr(d, "__dict__")
    assert copy.copy(d) is d
    assert copy.deepcopy(d) is d
    for field in ["_label", "_groups", "_color", "extra"]:
        with pytest.raises(AttributeError, match="immutable"):
            setattr(d, field, None)
        with pytest.raises(AttributeError, match="immutable"):
            delattr(d, field)
    with pytest.raises(AttributeError, match="immutable"):
        d.__init__("Other", [])
    _, groups, _ = native_distribution(d)
    assert not groups[0].flags.writeable
    with pytest.raises(ValueError):
        groups[0][0] = 8


@pytest.mark.parametrize("protocol", range(pickle.HIGHEST_PROTOCOL + 1))
def test_distribution_pickle_round_trip(protocol: int) -> None:
    d = ub.Distribution("North", [[1, np.nan, 2], []], color=ub.Color.RED)
    restored = cast(ub.Distribution, pickle.loads(pickle.dumps(d, protocol)))
    assert type(restored) is ub.Distribution
    assert repr(restored) == repr(d)
    assert axes().grouped_box(["A", "B"], [restored]).figure.to_svg() == (
        axes().grouped_box(["A", "B"], [d]).figure.to_svg()
    )
    assert all(not group.flags.writeable for group in native_distribution(restored)[1])


class RecordsConversion:
    def __init__(self) -> None:
        self.converted = False

    def __array__(self, dtype: object = None, copy: object = None) -> npt.NDArray[np.float64]:
        self.converted = True
        return np.array([1.0, 2.0])


def test_all_type_checks_precede_any_group_conversion() -> None:
    factories: list[Callable[[RecordsConversion], object]] = [
        lambda data: box(axes())(["A"], [data], color="red"),
        lambda data: cast(Callable[..., object], ub.Distribution)("A", [data], color="red"),
    ]
    for factory in factories:
        data = RecordsConversion()
        with pytest.raises(TypeError) as caught:
            factory(data)
        assert details(caught.value) == ("color", COLOR_HINT)
        assert not data.converted
    data = RecordsConversion()
    with pytest.raises(TypeError):
        box(axes())(["A", "B"], [data, None])
    assert not data.converted
    names = RecordsConversion()
    with pytest.raises(TypeError):
        grouped(axes())(names, [object()])
    assert not names.converted


DATA_ERRORS: list[tuple[object, object, str, str, str | None]] = [
    (["A"], [], "groups has 0 groups; categories has 1.", "groups", COUNT_HINT),
    (["A"], [[1, np.inf]], "groups[0][1] is inf; values must be finite.", "groups", GAP_HINT),
    (["A"], [[-np.inf]], "groups[0][0] is -inf; values must be finite.", "groups", GAP_HINT),
    (["A"], [[1e301]], "groups[0][0] is 1e301; magnitudes must not exceed 1e300.", "groups", None),
    (
        ["A", "A"],
        [[], []],
        'categories[1] repeats "A" from categories[0]; each category must appear once.',
        "categories",
        "combine the samples of repeated categories.",
    ),
]


@pytest.mark.parametrize(("categories", "groups", "message", "parameter", "hint"), DATA_ERRORS)
def test_data_errors_are_exact_and_atomic(
    categories: object, groups: object, message: str, parameter: str, hint: str | None
) -> None:
    ax = axes().box(["B"], [[4, 5]])
    fig = ax.figure
    before = fig.to_svg()
    with pytest.raises(ub.DataError) as caught:
        box(ax)(categories, groups)
    assert str(caught.value) == f"Axes.box(): {message}"
    assert details(caught.value) == (parameter, hint)
    assert fig.to_svg() == before


@pytest.mark.parametrize(
    ("ds", "message", "hint"),
    [
        ([], "distributions is empty; pass at least one Distribution.", None),
        (
            [ub.Distribution("A", [])],
            "distributions[0].groups has 0 groups; categories has 1.",
            COUNT_HINT,
        ),
        (
            [ub.Distribution("A", [[1]]), ub.Distribution("B", [[np.inf]])],
            "distributions[1].groups[0][0] is inf; values must be finite.",
            GAP_HINT,
        ),
    ],
)
def test_grouped_errors_are_atomic(
    ds: list[ub.Distribution], message: str, hint: str | None
) -> None:
    ax = axes().box(["B"], [[2, 3]])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.grouped_box(["A"], ds)
    assert str(caught.value) == f"Axes.grouped_box(): {message}"
    assert details(caught.value) == ("distributions", hint)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("value", [None, "bad", 3, [[1]], [ub.Series("A", [1])]])
def test_grouped_box_rejects_non_distributions(value: object) -> None:
    ax = axes()
    before = ax.figure.to_svg()
    with pytest.raises(TypeError) as caught:
        grouped(ax)(["A"], value)
    info = details(caught.value)
    assert info is not None and info[0] == "distributions"
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("value", [None, "bad", 3, [None], [3], [[1, None]], [["bad"]]])
def test_nested_conversion_errors_name_groups(value: object) -> None:
    operations: list[Callable[[], object]] = [
        lambda: box(axes())(["A"], value),
        lambda: cast(Callable[..., object], ub.Distribution)("A", value),
    ]
    for operation in operations:
        with pytest.raises((TypeError, ub.DataError)) as caught:
            operation()
        info = details(caught.value)
        assert info is not None and info[0] == "groups"


@pytest.mark.parametrize("scale", [ub.Scale.linear(), ub.Scale.log()])
def test_numeric_x_is_a_usage_error(scale: ub.Scale[float]) -> None:
    ax = ub.Figure().add_axes(x=scale)
    before = ax.figure.to_svg()
    for method, data in [(box(ax), [[1]]), (grouped(ax), [ub.Distribution("A", [[1]])])]:
        with pytest.raises(ub.UsageError) as caught:
            method(["A"], data)
        assert "the x axis is not a category axis; box plots need a category x axis." in str(
            caught.value
        )
        assert details(caught.value) == (
            "categories",
            "pass x=Scale.category() to Figure.add_axes().",
        )
        assert ax.figure.to_svg() == before


@pytest.mark.parametrize("value", [0.0, -1.0, 1e-310])
def test_log_samples_are_validated(value: float) -> None:
    ax = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.log())
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.box(["A"], [[1.0, value]])
    assert "groups[0][1]" in str(caught.value)
    info = details(caught.value)
    assert info is not None and info[0] == "groups"
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("style", list(ub.LegendStyle))
def test_labels_and_shared_figure_palette(style: ub.LegendStyle) -> None:
    ax = axes().legend(style).grouped_box(["A"], [ub.Distribution("North", [[1, 2]])])
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert (">North</text>" in svg) == (style != ub.LegendStyle.NONE)
    fig = _umber.Figure(8.0, 4.0, _umber.Theme.pigment(), 1, 2, 0, 0, 0)
    for col in range(2):
        fig.add_axes(1, 0, 1, col, 1, 1, 2, 10.0, 0, 10.0)
    fig.box_plot(0, 0, ["A"], [np.array([1.0, 2.0])], "Same", (0, 0, 1.0))
    fig.grouped_box_plot(0, 1, ["A"], [("Same", [np.array([3.0, 4.0])], (0, 0, 1.0))])
    assert fig.to_svg(1).count(">Same</text>") == 1
    before = fig.to_svg(0)
    with pytest.raises(ub.DataError):
        fig.box_plot(0, 1, ["A"], [], "", (0, 0, 1.0))
    assert fig.to_svg(0) == before


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.box(['A'], groups)", "groups"),
        ("ax.box(categories=['A'], groups=groups)", "groups=groups"),
        ("ax.grouped_box(['A'], ds)", "ds"),
        ("ub.Distribution('A', [[None]])", "[[None]]"),
        ("ax.box(['A'], [[1]], color='red')", "color='red'"),
    ],
)
def test_errors_highlight_top_level_arguments(tmp_path: Path, call: str, argument: str) -> None:
    source = tmp_path / "user_code.py"
    source.write_text(
        "import umber as ub\nax = ub.Figure().add_axes(x=ub.Scale.category())\n"
        "groups = [[float('inf')]]\nds = [ub.Distribution('A', groups)]\n" + call + "\n"
    )
    with pytest.raises((TypeError, ub.DataError)) as caught:
        exec(compile(source.read_text(), str(source), "exec"), {})
    expected = f"  {call}\n{' ' * (2 + call.index(argument))}{'^' * len(argument)}"
    assert list(getattr(caught.value, "__notes__", []))[0] == expected


@pytest.mark.parametrize("is_grouped", [False, True])
@pytest.mark.parametrize(
    ("theme", "native_theme"),
    [
        (ub.Theme.PIGMENT, _umber.Theme.pigment),
        (ub.Theme.GRAPHITE, _umber.Theme.graphite),
        (ub.Theme.MIDNIGHT, _umber.Theme.midnight),
    ],
)
def test_native_rendering_parity(
    is_grouped: bool, theme: ub.Theme, native_theme: Callable[[], _umber.Theme]
) -> None:
    fig = ub.Figure(4, 3, theme=theme)
    ax = fig.add_axes(x=ub.Scale.category())
    groups = [np.array([1.0, 2.0, 3.0, 4.0, 12.0]), np.array([2.0, 2.0])]
    native = _umber.Figure(4.0, 3.0, native_theme(), 1, 1, 0, 0, 0)
    native.add_axes(0, 0, 0, 0, 1, 1, 2, 10.0, 0, 10.0)
    if is_grouped:
        ax.grouped_box(["A", "B"], [ub.Distribution("Samples", groups)])
        native.grouped_box_plot(0, 0, ["A", "B"], [("Samples", groups, (0, 0, 1.0))])
    else:
        ax.box(["A", "B"], groups, label="Samples")
        native.box_plot(0, 0, ["A", "B"], groups, "Samples", (0, 0, 1.0))
    assert fig.to_svg() == native.to_svg(0)
    assert fig.to_png(dpi=60) == native.to_png(60)


def assert_box_stats_match_numpy(values: npt.NDArray[np.float64]) -> None:
    observed = _umber.box_stats(values)
    samples = np.sort(values[~np.isnan(values)])
    if samples.size == 0:
        assert observed is None
        return
    assert observed is not None
    q1, median, q3 = np.percentile(samples, [25, 50, 75], method="linear")
    reach = 1.5 * (q3 - q1)
    inside = (samples >= q1 - reach) & (samples <= q3 + reach)
    inliers = samples[inside]
    # A whisker stops at the quartile if its most extreme inlier lies inside the box.
    low_whisker = min(q1, inliers[0])
    high_whisker = max(q3, inliers[-1])
    np.testing.assert_array_equal(observed[:5], [q1, median, q3, low_whisker, high_whisker])
    np.testing.assert_array_equal(observed[5], samples[~inside])


def test_quantiles_match_numpy_type7() -> None:
    rng = np.random.default_rng(7107)
    for i in range(1000):
        values = rng.normal(size=int(rng.integers(1, 513))) * 10.0 ** int(rng.integers(-280, 281))
        if i % 3 == 0:
            values = np.round(values, decimals=2)
        assert_box_stats_match_numpy(values)


@pytest.mark.parametrize(
    "values",
    [
        [],
        [np.nan],
        [3.0],
        [3.0, 3.0],
        [100.0, 0.0, 1.0, np.nan, 2.0, 3.0, 4.0, -100.0],
        [-6.0, 0.0, 0.0, 2.0, 4.0, 4.0, 10.0],
        [100.0, -100.0, 100.0, -100.0, *range(12)],
        [0.0, 100.0, 100.0, 100.0],
        [0.0, 0.0, 0.0, 100.0],
        [0.0, 90.0, 100.0, 100.0],
        [0.0, 0.0, 10.0, 100.0],
    ],
)
def test_box_stats_match_numpy_tukey_edge_cases(values: list[float]) -> None:
    assert_box_stats_match_numpy(np.array(values, dtype=np.float64))
