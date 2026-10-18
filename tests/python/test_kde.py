"""``Axes.kde()``: a Gaussian kernel density line, with the Scott bandwidth of ``violin()``."""

from collections.abc import Callable
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pandas as pd
import pytest

import umber as ub
from umber import _umber
from umber._errors import details

type Numeric = ub.Axes[float, float]

RNG = np.random.default_rng(7)
SAMPLES = np.concatenate([RNG.normal(0.0, 1.0, 60), RNG.normal(4.0, 0.5, 40)])
LINEAR_X_HINT = "pass x=Scale.linear() to Figure.add_axes()."
LINEAR_Y_HINT = "pass y=Scale.linear() to Figure.add_axes()."
BANDWIDTH_HINT = "pass a bandwidth in data units greater than 0, or ub.AUTO."


def _svg(ax: ub.Axes[Any, Any]) -> str:
    return ax.figure.to_svg(text=ub.SvgText.EDITABLE)


def _axes(x: ub.Scale[float] | None = None, y: ub.Scale[float] | None = None) -> Numeric:
    return ub.Figure(4, 3).add_axes(x=x or ub.Scale.linear(), y=y or ub.Scale.linear())


def _gaussian_kde(
    samples: npt.NDArray[np.float64], bandwidth: float, at: npt.NDArray[np.float64]
) -> npt.NDArray[np.float64]:
    """The Gaussian kernel density of ``samples`` at ``at``, with area 1."""
    z = (at[:, None] - samples[None, :]) / bandwidth
    return cast(
        npt.NDArray[np.float64],
        np.exp(-0.5 * z * z).sum(axis=1) / (samples.size * bandwidth * np.sqrt(2.0 * np.pi)),
    )


def test_kde_matches_violin_bandwidth() -> None:
    n = SAMPLES.size
    scott = SAMPLES.std(ddof=1) * n ** (-0.2)
    bandwidth = _umber.scott_bandwidth(SAMPLES)
    assert bandwidth is not None
    assert bandwidth == pytest.approx(scott, rel=1e-12)
    # The violin densities are relative to the unit range of the samples: the same density in
    # data units, times the span.
    violin = np.asarray(_umber.kde_scott(SAMPLES))
    span = SAMPLES.max() - SAMPLES.min()
    expected = _gaussian_kde(SAMPLES, scott, violin[:, 0]) * span
    np.testing.assert_allclose(violin[:, 1], expected, rtol=1e-10)
    # The KDE line with AUTO is the line with the Scott bandwidth given explicitly.
    auto = _svg(_axes().kde(SAMPLES))
    assert auto == _svg(_axes().kde(SAMPLES, bandwidth=bandwidth))
    assert auto != _svg(_axes().kde(SAMPLES, bandwidth=bandwidth * 2))


def test_the_curve_is_a_density_in_data_units() -> None:
    bandwidth = 0.4
    low, high = SAMPLES.min() - 3 * bandwidth, SAMPLES.max() + 3 * bandwidth
    curve = _umber.kde_curve(SAMPLES, bandwidth, low, high, 256)
    assert curve is not None
    points = np.asarray(curve)
    assert points.shape == (256, 2)
    assert points[0, 0] == low
    assert points[-1, 0] == high
    np.testing.assert_allclose(points[:, 1], _gaussian_kde(SAMPLES, bandwidth, points[:, 0]))
    wide = np.asarray(_umber.kde_curve(SAMPLES, bandwidth, low - 10, high + 10, 4001))
    area = float(np.sum(np.diff(wide[:, 0]) * (wide[1:, 1] + wide[:-1, 1]) / 2))
    assert area == pytest.approx(1.0, rel=1e-6)


def test_scott_needs_two_distinct_values() -> None:
    assert _umber.scott_bandwidth(np.array([2.0, 2.0, np.nan])) is None
    assert _umber.scott_bandwidth(np.array([2.0, 3.0])) is not None


def test_kde_accepts_lists_arrays_and_series() -> None:
    values = [1.0, 2.0, 2.5, float("nan"), 4.0]
    drawn = {
        _svg(_axes().kde(v)) for v in (values, tuple(values), np.array(values), pd.Series(values))
    }
    assert len(drawn) == 1


def test_one_value_with_a_bandwidth_draws_a_curve() -> None:
    one = _svg(_axes().kde([1.0], bandwidth=1.0))
    assert one != _svg(_axes())
    assert one == _svg(_axes().kde([1.0, float("nan")], bandwidth=1.0))


def test_kde_takes_the_line_options() -> None:
    plain = _svg(_axes().kde(SAMPLES))
    styled = _svg(
        _axes().kde(
            SAMPLES, label="Density", color=ub.Color.RED, dash=ub.Dash.DOTTED, width=ub.Size.pt(1)
        )
    )
    assert styled != plain
    assert 'stroke="#D64933"' in styled
    assert ">Density</text>" in styled
    auto = _axes().kde(SAMPLES, bandwidth=ub.AUTO, dash=ub.Dash.SOLID, width=ub.Size.AUTO)
    assert _svg(auto) == plain


# (id, axes, call, error class, message, parameter, hint)
ERRORS: list[tuple[str, Callable[[], Any], Callable[[Any], object], type, str, Any, Any]] = [
    (
        "log x",
        lambda: _axes(x=ub.Scale.log()),
        lambda ax: ax.kde(SAMPLES),
        ub.UsageError,
        "Axes.kde(): the x axis is a log axis, which density curves do not support; use a "
        "linear x axis.",
        None,
        LINEAR_X_HINT,
    ),
    (
        "time x",
        lambda: ub.Figure().add_axes(x=ub.Scale.time()),
        lambda ax: ax.kde(SAMPLES),
        ub.UsageError,
        "Axes.kde(): the x axis is a time axis, which density curves do not support; use a "
        "linear x axis.",
        None,
        LINEAR_X_HINT,
    ),
    (
        "category x",
        lambda: ub.Figure().add_axes(x=ub.Scale.category()),
        lambda ax: ax.kde(SAMPLES),
        ub.DataError,
        "Axes.kde(): values holds numbers but the x axis is a category axis; use a linear x axis.",
        None,
        LINEAR_X_HINT,
    ),
    (
        "log y",
        lambda: _axes(y=ub.Scale.log()),
        lambda ax: ax.kde(SAMPLES),
        ub.UsageError,
        "Axes.kde(): the y axis is a log axis; a density curve runs down to 0 and needs a "
        "linear y axis.",
        "y",
        LINEAR_Y_HINT,
    ),
    (
        "one distinct value",
        _axes,
        lambda ax: ax.kde([2.0, 2.0, float("nan")]),
        ub.DataError,
        "Axes.kde(): values has 1 distinct finite value; the Scott bandwidth needs at least 2.",
        "values",
        "pass bandwidth= in data units.",
    ),
    (
        "no finite value",
        _axes,
        lambda ax: ax.kde([float("nan")], bandwidth=1.0),
        ub.DataError,
        "Axes.kde(): values has no finite value; a density curve needs at least one.",
        "values",
        "pass at least one value that is not NaN.",
    ),
    (
        "infinite",
        _axes,
        lambda ax: ax.kde([1.0, float("-inf")]),
        ub.DataError,
        "Axes.kde(): values[1] is -inf; values must be finite.",
        "values",
        "a density curve ignores NaN, so use it to leave a value out.",
    ),
    (
        "zero bandwidth",
        _axes,
        lambda ax: ax.kde(SAMPLES, bandwidth=0.0),
        ub.StyleError,
        "Axes.kde(): bandwidth is 0; bandwidth must be a finite number greater than 0.",
        "bandwidth",
        BANDWIDTH_HINT,
    ),
    (
        "nan bandwidth",
        _axes,
        lambda ax: ax.kde(SAMPLES, bandwidth=float("nan")),
        ub.StyleError,
        "Axes.kde(): bandwidth is NaN; bandwidth must be a finite number greater than 0.",
        "bandwidth",
        BANDWIDTH_HINT,
    ),
    (
        "bandwidth type",
        _axes,
        lambda ax: ax.kde(SAMPLES, bandwidth=cast(float, "wide")),
        TypeError,
        "Axes.kde(): bandwidth is str; bandwidth must be a float or ub.AUTO.",
        "bandwidth",
        BANDWIDTH_HINT,
    ),
    (
        "huge bandwidth",
        _axes,
        lambda ax: ax.kde(SAMPLES, bandwidth=1e301),
        ub.StyleError,
        "Axes.kde(): bandwidth is 1e301; magnitudes must not exceed 1e300.",
        "bandwidth",
        BANDWIDTH_HINT,
    ),
]


@pytest.mark.parametrize(
    ("make", "call", "cls", "message", "parameter", "hint"),
    [case[1:] for case in ERRORS],
    ids=[case[0] for case in ERRORS],
)
def test_kde_log_axis_rejected(
    make: Callable[[], Any],
    call: Callable[[Any], object],
    cls: type[BaseException],
    message: str,
    parameter: object,
    hint: object,
) -> None:
    ax = make().title("Kept")
    before = _svg(ax)
    with pytest.raises(cls) as caught:
        call(ax)
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert _svg(ax) == before


def test_a_density_beyond_the_supported_magnitudes_is_refused() -> None:
    ax = _axes().title("Kept")
    before = _svg(ax)
    with pytest.raises(ub.DataError) as caught:
        ax.kde([0.0], bandwidth=1e-305)
    message = str(caught.value)
    assert message.startswith("Axes.kde(): the density reaches "), message
    assert message.endswith("; densities must not exceed 1e300."), message
    assert details(caught.value) == ("bandwidth", "use a wider bandwidth.")
    assert _svg(ax) == before
