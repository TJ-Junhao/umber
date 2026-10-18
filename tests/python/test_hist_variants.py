"""``Axes.hist()`` variants: explicit bin edges, ``Hist`` styles, ``Stat`` and ``cumulative``."""

from collections.abc import Callable
from pathlib import Path
from typing import Any, cast

import numpy as np
import pandas as pd
import pytest

import umber as ub
from umber._errors import details

type Numeric = ub.Axes[float, float]

OUTSIDE_HINT = "widen the edges or filter the values first."
CUMULATIVE_HINT = "use Stat.FRACTION, or ecdf()."
STYLE_HINT = "use Hist.BARS, Hist.STEP or Hist.STACKED."
STAT_HINT = "use Stat.COUNT, Stat.FRACTION or Stat.DENSITY."
EDGES_HINT = "pass at least two increasing edges, or a number of bins."
INCREASING_HINT = "sort the edges and remove repeated ones."
BOOLEAN_HINT = "pass True or False."

# Values on and between uneven edges: the edge ties go to the bin above, and the last edge
# closes the last bin, as in NumPy.
EDGES = [0.0, 1.0, 2.5, 4.0, 10.0]
VALUES = [0.0, 0.5, 1.0, 1.0, 2.4, 2.5, 3.9, 4.0, 7.5, 10.0, float("nan")]


def _axes() -> Numeric:
    return ub.Figure(3, 2).add_axes()


def _svg(ax: Numeric) -> str:
    return ax.figure.to_svg(text=ub.SvgText.EDITABLE)


def _notes(err: BaseException) -> list[str]:
    return list(getattr(err, "__notes__", []))


def _raised(tmp_path: Path, line: str, setup: str = "") -> BaseException:
    """Run ``line`` as user code in a file, so the error is reported at its call site."""
    path = tmp_path / f"user_code_{abs(hash((setup, line)))}.py"
    path.write_text(f"import umber as ub\n{setup}\n{line}\n")
    try:
        exec(compile(path.read_text(), str(path), "exec"), {})
    except Exception as err:
        return err
    raise AssertionError(f"{line} raised nothing")


def test_hist_edges_match_numpy() -> None:
    # A histogram is drawn from its counts alone, so values with NumPy's counts placed at the
    # bin centres draw the same histogram exactly when umber counts as NumPy does.
    counts, _ = np.histogram([v for v in VALUES if v == v], bins=EDGES)
    centres = [(a + b) / 2 for a, b in zip(EDGES, EDGES[1:], strict=False)]
    stand_in = np.repeat(centres, counts)
    for style in ub.Hist:
        for stat in ub.Stat:
            cumulative = [False] if stat is ub.Stat.DENSITY else [False, True]
            for running in cumulative:
                kwargs: dict[str, Any] = {"style": style, "stat": stat, "cumulative": running}
                drawn = _svg(_axes().hist(VALUES, bins=EDGES, **kwargs))
                assert drawn == _svg(_axes().hist(stand_in, bins=EDGES, **kwargs)), kwargs
    assert counts.tolist() == [2, 3, 2, 3]


def test_edges_accept_lists_arrays_and_series() -> None:
    outputs = {
        _svg(_axes().hist(VALUES, bins=edges))
        for edges in (EDGES, tuple(EDGES), np.array(EDGES), pd.Series(EDGES))
    }
    assert len(outputs) == 1
    assert outputs != {_svg(_axes().hist(VALUES, bins=4))}


def test_the_defaults_are_the_plain_histogram() -> None:
    plain = _svg(_axes().hist(VALUES, bins=4))
    explicit = _axes().hist(
        VALUES, bins=4, style=ub.Hist.BARS, cumulative=False, stat=ub.Stat.COUNT
    )
    assert _svg(explicit) == plain


def test_stacked_on_one_series_equals_bars() -> None:
    for theme in (ub.Theme.PIGMENT, ub.Theme.MIDNIGHT):
        bars = ub.Figure(3, 2, theme=theme).add_axes().hist(VALUES, bins=4, label="Sizes")
        stacked = ub.Figure(3, 2, theme=theme).add_axes()
        stacked.hist(VALUES, bins=4, style=ub.Hist.STACKED, label="Sizes")
        assert _svg(stacked) == _svg(bars)


def test_step_draws_an_outline_without_fill() -> None:
    color = ub.Color.RED
    bars = _svg(_axes().hist(VALUES, bins=4, color=color))
    step = _svg(_axes().hist(VALUES, bins=4, color=color, style=ub.Hist.STEP))
    assert 'fill="#D64933"' in bars
    assert 'fill="#D64933"' not in step
    assert 'stroke="#D64933"' in step


def test_each_stat_and_cumulative_draws_its_own_histogram() -> None:
    drawn = {
        _svg(_axes().hist(VALUES, bins=EDGES, stat=stat, cumulative=running))
        for stat in ub.Stat
        for running in ([False] if stat is ub.Stat.DENSITY else [False, True])
    }
    assert len(drawn) == 5


def test_density_with_equal_widths_is_the_fraction_divided_by_the_width() -> None:
    # With bins one unit wide, the density of every bin is its fraction.
    unit = [0.0, 1.0, 2.0, 3.0, 4.0]
    values = [0.5, 1.5, 1.5, 3.2, 3.9]
    fraction = _svg(_axes().hist(values, bins=unit, stat=ub.Stat.FRACTION))
    assert _svg(_axes().hist(values, bins=unit, stat=ub.Stat.DENSITY)) == fraction


# (id, call, error class, message, parameter, hint)
ERRORS: list[tuple[str, Callable[[Numeric], object], type, str, Any, Any]] = [
    (
        "values outside the edges",
        lambda ax: ax.hist([-1.0, 0.5, 11.0, 12.0], bins=EDGES),
        ub.DataError,
        "Axes.hist(): values has 3 finite values outside the bin edges, which span 0 to 10; "
        "every finite value must lie within the edges.",
        "values",
        OUTSIDE_HINT,
    ),
    (
        "cumulative density",
        lambda ax: ax.hist(VALUES, cumulative=True, stat=ub.Stat.DENSITY),
        ub.UsageError,
        "Axes.hist(): cumulative is True with Stat.DENSITY; a cumulative histogram shows counts "
        "or fractions.",
        "cumulative",
        CUMULATIVE_HINT,
    ),
    (
        "one edge",
        lambda ax: ax.hist(VALUES, bins=[1.0]),
        ub.DataError,
        "Axes.hist(): bins has 1 edge; bin edges must number at least 2.",
        "bins",
        EDGES_HINT,
    ),
    (
        "no edges",
        lambda ax: ax.hist(VALUES, bins=[]),
        ub.DataError,
        "Axes.hist(): bins has 0 edges; bin edges must number at least 2.",
        "bins",
        EDGES_HINT,
    ),
    (
        "repeated edge",
        lambda ax: ax.hist(VALUES, bins=[0.0, 2.0, 2.0, 10.0]),
        ub.DataError,
        "Axes.hist(): bins[2] is 2, not above bins[1], 2; bin edges must be strictly increasing.",
        "bins",
        INCREASING_HINT,
    ),
    (
        "decreasing edge",
        lambda ax: ax.hist(VALUES, bins=[0.0, 5.0, 3.0]),
        ub.DataError,
        "Axes.hist(): bins[2] is 3, not above bins[1], 5; bin edges must be strictly increasing.",
        "bins",
        INCREASING_HINT,
    ),
    (
        "nan edge",
        lambda ax: ax.hist(VALUES, bins=[0.0, float("nan"), 10.0]),
        ub.DataError,
        "Axes.hist(): bins[1] is NaN; bin edges must be finite.",
        "bins",
        EDGES_HINT,
    ),
    (
        "infinite edge",
        lambda ax: ax.hist(VALUES, bins=[0.0, float("inf")]),
        ub.DataError,
        "Axes.hist(): bins[1] is inf; bin edges must be finite.",
        "bins",
        EDGES_HINT,
    ),
    (
        "huge edge",
        lambda ax: ax.hist(VALUES, bins=[0.0, 1e301]),
        ub.DataError,
        "Axes.hist(): bins[1] is 1e301; magnitudes must not exceed 1e300.",
        "bins",
        None,
    ),
    (
        "style type",
        lambda ax: ax.hist(VALUES, style=cast(ub.Hist, "step")),
        TypeError,
        "Axes.hist(): style is str; style must be of type Hist.",
        "style",
        STYLE_HINT,
    ),
    (
        "stat type",
        lambda ax: ax.hist(VALUES, stat=cast(ub.Stat, 1)),
        TypeError,
        "Axes.hist(): stat is int; stat must be of type Stat.",
        "stat",
        STAT_HINT,
    ),
    (
        "cumulative type",
        lambda ax: ax.hist(VALUES, cumulative=cast(bool, 1)),
        TypeError,
        "Axes.hist(): cumulative is int; cumulative must be a bool.",
        "cumulative",
        BOOLEAN_HINT,
    ),
    (
        "edges dtype",
        lambda ax: ax.hist(VALUES, bins=cast(list[float], ["a", "b"])),
        ub.DataError,
        "Axes.hist(): bins[0] is 'a'; values must be numbers.",
        "bins",
        "convert text to numbers first, e.g. with float().",
    ),
]


@pytest.mark.parametrize(
    ("call", "cls", "message", "parameter", "hint"),
    [case[1:] for case in ERRORS],
    ids=[case[0] for case in ERRORS],
)
def test_values_outside_edges_rejected(
    call: Callable[[Numeric], object],
    cls: type[BaseException],
    message: str,
    parameter: object,
    hint: object,
) -> None:
    ax = _axes().hist([1.0, 2.0], label="Kept")
    before = _svg(ax)
    with pytest.raises(cls) as caught:
        call(ax)
    assert type(caught.value) is cls
    assert str(caught.value) == message
    assert details(caught.value) == (parameter, hint)
    assert _svg(ax) == before


def test_types_are_checked_in_parameter_order() -> None:
    hist = cast(Callable[..., object], _axes().hist)
    with pytest.raises(TypeError) as caught:
        hist([1.0], bins="3", style="x", cumulative=1, stat="y")
    assert details(caught.value) == ("bins", "pass a whole number such as 3.")
    with pytest.raises(TypeError) as caught:
        hist([1.0], style="x", cumulative=1, stat="y")
    assert details(caught.value) == ("style", STYLE_HINT)
    with pytest.raises(TypeError) as caught:
        hist([1.0], cumulative=1, stat="y")
    assert details(caught.value) == ("cumulative", BOOLEAN_HINT)
    with pytest.raises(TypeError) as caught:
        hist([1.0], stat="y")
    assert details(caught.value) == ("stat", STAT_HINT)


def test_edges_errors_highlight_the_argument(tmp_path: Path) -> None:
    err = _raised(tmp_path, "ax.hist([1.0], bins=[2.0, 1.0])", "ax = ub.Figure().add_axes()")
    assert isinstance(err, ub.DataError)
    line = "ax.hist([1.0], bins=[2.0, 1.0])"
    start = line.index("bins=")
    assert _notes(err)[0] == f"  {line}\n{' ' * (2 + start)}{'^' * len('bins=[2.0, 1.0]')}"


def test_quick_hist_takes_the_new_options() -> None:
    quick = ub.hist(VALUES, bins=EDGES, style=ub.Hist.STEP, stat=ub.Stat.FRACTION)
    ax = ub.Figure().add_axes().hist(VALUES, bins=EDGES, style=ub.Hist.STEP, stat=ub.Stat.FRACTION)
    assert _svg(quick) == _svg(ax)
    with pytest.raises(ub.UsageError):
        ub.hist(VALUES, cumulative=True, stat=ub.Stat.DENSITY)


def test_enum_members() -> None:
    assert [(m.name, m.value) for m in ub.Hist] == [("BARS", 0), ("STEP", 1), ("STACKED", 2)]
    assert [(m.name, m.value) for m in ub.Stat] == [
        ("COUNT", 0),
        ("FRACTION", 1),
        ("DENSITY", 2),
    ]


@pytest.mark.parametrize(
    ("values", "edges", "start"),
    [
        (
            [0.0],
            [-1.0, 0.0, 1e-310, 1.0],
            "Axes.hist(): bin 1, from 0 to 1e-310, has a height of inf",
        ),
        ([0.0, 0.5], [0.0, 1e-301, 1.0], "Axes.hist(): bin 0, from 0 to 1e-301, has a height of "),
    ],
    ids=["infinite", "finite but too large"],
)
def test_densities_beyond_the_supported_magnitudes_are_refused(
    values: list[float], edges: list[float], start: str
) -> None:
    ax = _axes().hist([0.5], label="Kept")
    before = _svg(ax)
    with pytest.raises(ub.DataError) as caught:
        ax.hist(values, bins=edges, stat=ub.Stat.DENSITY, style=ub.Hist.STEP)
    message = str(caught.value)
    assert message.startswith(start), message
    assert message.endswith("; bar heights must be finite and at most 1e300."), message
    assert details(caught.value) == (
        "bins",
        "widen the narrow bins, or use Stat.COUNT or Stat.FRACTION.",
    )
    assert _svg(ax) == before
