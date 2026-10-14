"""Violins, immutable distributions, conversion errors and Rust/NumPy kernel densities."""

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


def violin(ax: ub.Axes[Any, Any]) -> Callable[..., object]:
    return cast(Callable[..., object], ax.violin)


def grouped(ax: ub.Axes[Any, Any]) -> Callable[..., object]:
    return cast(Callable[..., object], ax.grouped_violin)


def test_returns_self_and_has_keyword_only_options() -> None:
    ax = axes()
    assert ax.violin(["A"], [[1, 2]]) is ax
    assert ax.grouped_violin(["A"], [ub.Distribution("B", [[3, 4]])]) is ax
    assert not hasattr(ub, "violin")
    assert not hasattr(ub, "kde_scott")
    with pytest.raises(TypeError):
        violin(ax)(["A"], [[1]], "label")
    with pytest.raises(TypeError):
        cast(Callable[..., object], ub.Distribution)("A", [[1]], ub.Color.RED)


def test_lists_tuples_arrays_and_series_have_identical_output() -> None:
    outputs: set[str] = set()
    for names, groups in [
        (["A", "B"], [[1, 2, 3], [4, 5]]),
        (("A", "B"), (np.array([1, 2, 3], dtype=np.int16), (4, 5))),
        (np.array(["A", "B"]), [pd.Series([1.0, 2.0, 3.0]), np.array([4, 5])]),
    ]:
        for grouped_violin in [False, True]:
            ax = axes()
            if grouped_violin:
                grouped(ax)(
                    names, [cast(Callable[..., ub.Distribution], ub.Distribution)("", groups)]
                )
            else:
                violin(ax)(names, groups)
            outputs.add(ax.figure.to_svg())
    assert len(outputs) == 1


def test_input_samples_are_copied_and_nan_is_ignored() -> None:
    source = np.array([1.0, 2.0, 3.0, 4.0])
    d = ub.Distribution("Samples", [source])
    plain = axes().violin(["A"], [source], label="Samples")
    before = plain.figure.to_svg()
    source[:] = 100.0
    assert plain.figure.to_svg() == before
    assert axes().grouped_violin(["A"], [d]).figure.to_svg() == before
    assert axes().violin(["A"], [[1, np.nan, 2, 3, 4]], label="Samples").figure.to_svg() == before


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
    assert axes().grouped_violin(["A", "B"], [restored]).figure.to_svg() == (
        axes().grouped_violin(["A", "B"], [d]).figure.to_svg()
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
        lambda data: violin(axes())(["A"], [data], color="red"),
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
        violin(axes())(["A", "B"], [data, None])
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
    ax = axes().violin(["B"], [[4, 5]])
    fig = ax.figure
    before = fig.to_svg()
    with pytest.raises(ub.DataError) as caught:
        violin(ax)(categories, groups)
    assert str(caught.value) == f"Axes.violin(): {message}"
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
    ax = axes().violin(["B"], [[2, 3]])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        ax.grouped_violin(["A"], ds)
    assert str(caught.value) == f"Axes.grouped_violin(): {message}"
    assert details(caught.value) == ("distributions", hint)
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("value", [None, "bad", 3, [[1]], [ub.Series("A", [1])]])
def test_grouped_violin_rejects_non_distributions(value: object) -> None:
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
        lambda: violin(axes())(["A"], value),
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
    for method, data in [(violin(ax), [[1]]), (grouped(ax), [ub.Distribution("A", [[1]])])]:
        with pytest.raises(ub.UsageError) as caught:
            method(["A"], data)
        assert "the x axis is not a category axis; violins need a category x axis." in str(
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
        ax.violin(["A"], [[1.0, value]])
    assert "groups[0][1]" in str(caught.value)
    info = details(caught.value)
    assert info is not None and info[0] == "groups"
    assert ax.figure.to_svg() == before


@pytest.mark.parametrize("style", list(ub.LegendStyle))
def test_labels_and_shared_figure_palette(style: ub.LegendStyle) -> None:
    ax = axes().legend(style).grouped_violin(["A"], [ub.Distribution("North", [[1, 2]])])
    svg = ax.figure.to_svg(text=ub.SvgText.EDITABLE)
    assert (">North</text>" in svg) == (style != ub.LegendStyle.NONE)
    fig = _umber.Figure(8.0, 4.0, _umber.Theme.pigment(), 1, 2, 0, 0, 0)
    for col in range(2):
        fig.add_axes(1, 0, 1, col, 1, 1, 2, 10.0, 0, 10.0)
    fig.violin(0, 0, ["A"], [np.array([1.0, 2.0])], "Same", (0, 0, 1.0))
    fig.grouped_violin(0, 1, ["A"], [("Same", [np.array([3.0, 4.0])], (0, 0, 1.0))])
    assert fig.to_svg(1).count(">Same</text>") == 1
    before = fig.to_svg(0)
    with pytest.raises(ub.DataError):
        fig.violin(0, 1, ["A"], [], "", (0, 0, 1.0))
    assert fig.to_svg(0) == before


@pytest.mark.parametrize(
    ("call", "argument"),
    [
        ("ax.violin(['A'], groups)", "groups"),
        ("ax.violin(categories=['A'], groups=groups)", "groups=groups"),
        ("ax.grouped_violin(['A'], ds)", "ds"),
        ("ub.Distribution('A', [[None]])", "[[None]]"),
        ("ax.violin(['A'], [[1]], color='red')", "color='red'"),
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
        ax.grouped_violin(["A", "B"], [ub.Distribution("Samples", groups)])
        native.grouped_violin(0, 0, ["A", "B"], [("Samples", groups, (0, 0, 1.0))])
    else:
        ax.violin(["A", "B"], groups, label="Samples")
        native.violin(0, 0, ["A", "B"], groups, "Samples", (0, 0, 1.0))
    assert fig.to_svg() == native.to_svg(0)
    assert fig.to_png(dpi=60) == native.to_png(60)


def test_kde_matches_reference_within_1e9() -> None:
    rng = np.random.default_rng(8108)
    maximum_error = 0.0
    for i in range(100):
        values = rng.normal(size=int(rng.integers(2, 513)))
        if i % 3 == 0:
            values = np.round(values, decimals=1)
        if i % 5 == 0:
            values = np.append(values, np.nan)
        samples = values[np.isfinite(values)]
        observed = _umber.kde_scott(values)
        assert observed is not None
        grid = np.linspace(samples.min(), samples.max(), 128)
        h = samples.std(ddof=1) * samples.size ** (-1.0 / 5.0)
        density = np.exp(-0.5 * ((grid[:, None] - samples[None, :]) / h) ** 2).sum(axis=1)
        density /= samples.size * h * np.sqrt(2.0 * np.pi)
        actual = np.asarray(observed)
        np.testing.assert_allclose(actual[:, 0], grid, rtol=1e-14, atol=1e-14)
        np.testing.assert_allclose(actual[:, 1], density, rtol=1e-9, atol=0)
        maximum_error = max(maximum_error, float(np.max(np.abs(actual[:, 1] / density - 1))))
    print(f"kde_matches_reference_within_1e9: maximum relative error {maximum_error:.17g}")


@pytest.mark.parametrize("offset", [-1e12, 1e12])
def test_kde_evaluates_at_rounded_grid_positions(offset: float) -> None:
    samples = offset + np.arange(5, dtype=np.float64)
    observed = _umber.kde_scott(samples)
    assert observed is not None
    grid = np.linspace(samples.min(), samples.max(), 128)
    h = samples.std(ddof=1) * samples.size ** (-1.0 / 5.0)
    density = np.exp(-0.5 * ((grid[:, None] - samples[None, :]) / h) ** 2).sum(axis=1)
    density /= samples.size * h * np.sqrt(2.0 * np.pi)
    actual = np.asarray(observed)
    np.testing.assert_array_equal(actual[:, 0], grid)
    np.testing.assert_allclose(actual[:, 1], density, rtol=1e-9, atol=0)


@pytest.mark.parametrize("values", [[], [np.nan], [3.0], [3.0, 3.0], [3.0, np.nan]])
def test_kde_degenerate_groups(values: list[float]) -> None:
    assert _umber.kde_scott(np.array(values, dtype=np.float64)) is None


@pytest.mark.parametrize("scale", [1e-300, 1.0, 1e300])
def test_kde_is_scale_equivariant(scale: float) -> None:
    values = np.array([-1.0, -0.5, 0.5, 1.0])
    base = np.asarray(_umber.kde_scott(values))
    actual = np.asarray(_umber.kde_scott(values * scale))
    np.testing.assert_allclose(actual[:, 0] / scale, base[:, 0], rtol=1e-12, atol=1e-15)
    np.testing.assert_allclose(actual[:, 1] * scale, base[:, 1], rtol=1e-12, atol=0)
