"""Time axes: the accepted time inputs and their conversion, time zones, gaps, limits, ticks and
reference marks on time axes, the marks a time axis refuses, and failed calls."""

import copy
import datetime as dt
import pickle
import zoneinfo
from collections.abc import Callable
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl
import pytest

import umber as ub
from umber import _time
from umber._errors import details

EPOCH = dt.datetime(1970, 1, 1)
NAT = np.iinfo(np.int64).min
TZ_HINT = "convert first with .tz_localize(None) or .tz_convert(...)."
TIME_VALUES_HINT = "pass datetime, date or numpy.datetime64 values."
CATEGORY_HINT = "use a category axis with formatted labels."

# Whole seconds, so that every unit from seconds down holds them exactly.
TIMES = [
    dt.datetime(2026, 3, 3, 6, 0, 0),
    dt.datetime(2026, 3, 3, 6, 0, 1),
    dt.datetime(1999, 12, 31, 23, 59, 59),
]

type AnyAxes = ub.Axes[Any, Any]


def _us(moment: dt.datetime) -> int:
    return (moment - EPOCH) // dt.timedelta(microseconds=1)


EXPECTED = np.array([_us(t) for t in TIMES], dtype=np.int64)


def _parts(err: BaseException) -> tuple[str, object, object]:
    found = details(err)
    assert found is not None
    return str(err), found[0], found[1]


def _call(ax: AnyAxes, name: str) -> Callable[..., object]:
    """The method ``name`` of ``ax`` with arguments of any type, for calls that must be refused."""
    return cast(Callable[..., object], getattr(ax, name))


def _time_axes(theme: ub.Theme = ub.Theme.PIGMENT) -> ub.Axes[ub.Time, float]:
    return ub.Figure(5, 3, theme=theme).add_axes(x=ub.Scale.time())


def _inputs() -> dict[str, object]:
    return {
        "datetime list": list(TIMES),
        "datetime tuple": tuple(TIMES),
        "numpy datetime64 list": [np.datetime64(t, "s") for t in TIMES],
        "numpy s": np.array(TIMES, dtype="datetime64[s]"),
        "numpy ms": np.array(TIMES, dtype="datetime64[ms]"),
        "numpy us": np.array(TIMES, dtype="datetime64[us]"),
        "numpy ns": np.array(TIMES, dtype="datetime64[ns]"),
        "numpy strided": np.repeat(np.array(TIMES, dtype="datetime64[us]"), 2)[::2],
        "numpy object": np.array(TIMES, dtype=object),
        "pandas naive": pd.Series(pd.to_datetime(TIMES)),
        "pandas ms": pd.Series(pd.to_datetime(TIMES)).astype("datetime64[ms]"),
        "pandas index": pd.DatetimeIndex(TIMES),
        "pandas timestamps": [pd.Timestamp(t) for t in TIMES],
        "polars ms": pl.Series(TIMES, dtype=pl.Datetime("ms")),
        "polars us": pl.Series(TIMES, dtype=pl.Datetime("us")),
        "polars ns": pl.Series(TIMES, dtype=pl.Datetime("ns")),
    }


@pytest.mark.parametrize("kind", list(_inputs()))
def test_time_inputs_round_trip(kind: str) -> None:
    values = _inputs()[kind]
    converted = _time.micros("Axes.line()", "x", values)
    assert converted.dtype == np.int64
    np.testing.assert_array_equal(converted, EXPECTED)
    # Every kind draws the same figure.
    reference = _time_axes().line(list(TIMES), [1.0, 3.0, 2.0]).figure.to_svg()
    figure = _call(_time_axes(), "line")(values, [1.0, 3.0, 2.0])
    assert cast(AnyAxes, figure).figure.to_svg() == reference


def test_dates_are_midnight_and_mix_with_datetimes() -> None:
    values = [dt.date(2026, 3, 3), dt.datetime(2026, 3, 3, 12, 0)]
    converted = _time.micros("Axes.line()", "x", values)
    midnight = _us(dt.datetime(2026, 3, 3))
    np.testing.assert_array_equal(converted, [midnight, midnight + 12 * 3600 * 10**6])
    days = np.array(["2026-03-03", "2026-03-04"], dtype="datetime64[D]")
    np.testing.assert_array_equal(
        _time.micros("Axes.line()", "x", days), [midnight, midnight + 86400 * 10**6]
    )
    polars_dates = pl.Series([dt.date(2026, 3, 3)], dtype=pl.Date)
    np.testing.assert_array_equal(_time.micros("Axes.line()", "x", polars_dates), [midnight])
    months = np.array(["2026-03", "2026-04"], dtype="datetime64[M]")
    assert _time.micros("Axes.line()", "x", months)[0] == _us(dt.datetime(2026, 3, 1))


def test_finer_values_round_half_to_even() -> None:
    nanoseconds = np.array([1500, 2500, 3499, 3500, -1500, -2500, 999], dtype="datetime64[ns]")
    np.testing.assert_array_equal(
        _time.micros("Axes.line()", "x", nanoseconds), [2, 2, 3, 4, -2, -2, 1]
    )
    picoseconds = np.array([1_500_000, 500_000], dtype="datetime64[ps]")
    np.testing.assert_array_equal(_time.micros("Axes.line()", "x", picoseconds), [2, 0])
    stamp = pd.Timestamp("2026-03-03 06:00:00.0000015")
    assert _time.micros("Axes.line()", "x", [stamp])[0] == _us(dt.datetime(2026, 3, 3, 6)) + 2


def test_nat_leaves_gap() -> None:
    gapped: dict[str, object] = {
        "numpy": np.array(["2026-01-01", "NaT", "2026-01-03"], dtype="datetime64[D]"),
        "pandas": pd.Series(
            [pd.Timestamp("2026-01-01"), pd.NaT, pd.Timestamp("2026-01-03")],
            dtype="datetime64[ns]",
        ),
        "pandas NaT element": [dt.date(2026, 1, 1), pd.NaT, dt.date(2026, 1, 3)],
        "numpy NaT element": [dt.date(2026, 1, 1), np.datetime64("NaT", "s"), dt.date(2026, 1, 3)],
        "polars null": pl.Series(
            [dt.datetime(2026, 1, 1), None, dt.datetime(2026, 1, 3)], dtype=pl.Datetime("us")
        ),
    }
    first = _us(dt.datetime(2026, 1, 1))
    for name, values in gapped.items():
        converted = _time.micros("Axes.line()", "x", values)
        np.testing.assert_array_equal(
            converted, [first, NAT, first + 2 * 86400 * 10**6], err_msg=name
        )
        drawn = cast(AnyAxes, _call(_time_axes(), "line")(values, [1.0, 2.0, 3.0]))
        svg = drawn.figure.to_svg()
        assert svg.startswith("<svg"), name
    # All missing draws nothing, without an error.
    empty = np.array(["NaT", "NaT"], dtype="datetime64[s]")
    assert _time_axes().line(empty, [1.0, 2.0]).figure.to_svg().startswith("<svg")


def _aware() -> dict[str, tuple[object, str, str]]:
    berlin = zoneinfo.ZoneInfo("Europe/Berlin")
    utc = dt.datetime(2026, 1, 1, tzinfo=dt.UTC)
    local = dt.datetime(2026, 1, 1, tzinfo=berlin)
    return {
        "datetime": ([dt.datetime(2026, 1, 1), utc], "x[1]", "UTC"),
        "zoneinfo": ([local], "x[0]", "Europe/Berlin"),
        "pandas column": (pd.Series(pd.to_datetime(TIMES)).dt.tz_localize("UTC"), "x", "UTC"),
        "pandas index": (
            pd.DatetimeIndex(TIMES).tz_localize("Europe/Berlin"),
            "x",
            "Europe/Berlin",
        ),
        "pandas timestamp": ([pd.Timestamp("2026-01-01", tz="UTC")], "x[0]", "UTC"),
        "polars": (
            pl.Series(TIMES, dtype=pl.Datetime("us")).dt.replace_time_zone("Europe/Berlin"),
            "x",
            "Europe/Berlin",
        ),
        "object array": (np.array([utc], dtype=object), "x[0]", "UTC"),
    }


@pytest.mark.parametrize("kind", list(_aware()))
def test_tz_aware_rejected_for_each_input_kind(kind: str) -> None:
    values, label, zone = _aware()[kind]
    ax = _time_axes().line(list(TIMES), [1.0, 2.0, 3.0])
    before = ax.figure.to_svg()
    with pytest.raises(ub.DataError) as caught:
        _call(ax, "line")(values, [1.0] * 3)
    assert _parts(caught.value) == (
        f"Axes.line(): {label} has time zone {zone}; time values must not carry a time zone.",
        "x",
        TZ_HINT,
    )
    assert ax.figure.to_svg() == before
    # Limits and reference values refuse a time zone as well.
    with pytest.raises(ub.DataError) as caught:
        ax.xlim(dt.datetime(2026, 1, 1, tzinfo=dt.UTC), dt.datetime(2026, 2, 1))
    assert _parts(caught.value) == (
        "Axes.xlim(): low has time zone UTC; time values must not carry a time zone.",
        "low",
        TZ_HINT,
    )
    assert ax.figure.to_svg() == before


def test_out_of_range_and_wrong_values_are_data_errors() -> None:
    ax = _time_axes()
    beyond = (dt.date(9999, 12, 31) - dt.date(1970, 1, 1)).days + 1
    years = np.array([0, beyond], dtype="datetime64[D]")
    with pytest.raises(ub.DataError) as caught:
        ax.line(years, [1.0, 2.0])
    assert _parts(caught.value) == (
        "Axes.line(): x[1] is 10000-01-01; times must lie between 0001-01-01 and 9999-12-31.",
        "x",
        None,
    )
    far = np.array([56, 300000 - 1970], dtype="datetime64[Y]")
    with pytest.raises(ub.DataError) as caught:
        ax.line(far, [1.0, 2.0])
    assert str(caught.value).startswith("Axes.line(): x[1] is 300000-01-01; times must lie")
    cases: list[tuple[list[object], str]] = [
        ([dt.datetime(2026, 1, 1), 3.5], "x[1] is 3.5"),
        ([dt.timedelta(days=1)], "x[0] is timedelta"),
        (["2026-01-01"], "x[0] is '2026-01-01'"),
        ([dt.datetime(2026, 1, 1), None], "x[1] is None"),
    ]
    for values, shown in cases:
        with pytest.raises(ub.DataError) as caught:
            _call(ax, "line")(values, [1.0] * len(values))
        message, parameter, _ = _parts(caught.value)
        assert message == f"Axes.line(): {shown}; values must be times on a time axis."
        assert parameter == "x"
    with pytest.raises(ub.DataError) as caught:
        _call(ax, "line")(np.array([1.0, 2.0]), [1.0, 2.0])
    assert _parts(caught.value) == (
        "Axes.line(): x has dtype float64; values must be times on a time axis.",
        "x",
        TIME_VALUES_HINT,
    )


def test_time_xlim_and_vline_accept_times() -> None:
    ax = _time_axes().line(list(TIMES[:2]), [1.0, 2.0])
    start = dt.datetime(2026, 3, 3, 5, 59, 50)
    end = np.datetime64("2026-03-03T06:00:10")
    same = (
        ax.xlim(start, end)
        .vline(dt.datetime(2026, 3, 3, 6, 0, 0, 500000), label="Event")
        .vspan(dt.datetime(2026, 3, 3, 6, 0, 2), np.datetime64("2026-03-03T06:00:04"))
        .xticks([dt.datetime(2026, 3, 3, 6, 0), np.datetime64("2026-03-03T06:00:05")])
        .xtick_format(ub.TickFormat.iso())
    )
    assert same is ax
    assert ax.figure.to_svg().startswith("<svg")
    ax.xtick_format(ub.TickFormat.AUTO).xticks_labeled([dt.date(2026, 3, 3)], ["day"])
    assert ax.figure.to_png().startswith(b"\x89PNG")
    # The wrong kind of value is a type error, as a name is on a numeric axis.
    for name, args in [("xlim", (1.0, 2.0)), ("vline", (3,)), ("vspan", ("a", "b"))]:
        with pytest.raises(TypeError) as raised:
            _call(ax, name)(*args)
        message, parameter, hint = _parts(raised.value)
        assert message.endswith(" must be a time."), message
        assert parameter in ("low", "x"), parameter
        assert hint == "pass a datetime, date or numpy.datetime64."
    with pytest.raises(ub.StyleError):
        ax.xlim(end, start)
    with pytest.raises(ub.DataError) as caught:
        ax.vline(np.datetime64("NaT", "s"))
    assert _parts(caught.value)[0] == "Axes.vline(): x is NaT; it must be a time."


def test_number_formats_need_a_numeric_axis() -> None:
    ax = _time_axes()
    with pytest.raises(ub.UsageError) as caught:
        ax.xtick_format(ub.TickFormat.fixed(1))
    assert _parts(caught.value) == (
        "Axes.xtick_format(): this is a time axis; number formats label linear and log axes only.",
        "format",
        "use TickFormat.AUTO or TickFormat.iso() on a time axis.",
    )
    with pytest.raises(ub.UsageError):
        ub.Figure().add_axes().xtick_format(ub.TickFormat.iso())


def test_time_y_axis() -> None:
    fig = ub.Figure(5, 3)
    ax = fig.add_axes(y=ub.Scale.time())
    days = [dt.date(2026, 1, d) for d in (1, 5, 9)]
    ax.line([0.0, 1.0, 2.0], days).ylim(dt.date(2025, 12, 30), dt.date(2026, 1, 12))
    ax.hline(dt.date(2026, 1, 3)).hspan(dt.date(2026, 1, 6), dt.date(2026, 1, 7))
    ax.yticks([dt.date(2026, 1, 1), dt.date(2026, 1, 8)]).scatter([0.5], [dt.date(2026, 1, 2)])
    ax.x_error_bars([dt.date(2026, 1, 4)], [0.5], [1.5])
    assert fig.axes_at(y=ub.Scale.time()) is ax
    assert fig.to_svg().startswith("<svg")
    ax.y_error_bars(
        [1.0, 2.0], [dt.date(2026, 1, 2), np.datetime64("NaT", "s")], [dt.date(2026, 1, 4)] * 2
    )
    assert fig.to_svg().startswith("<svg")
    with pytest.raises(ub.DataError) as caught:
        _call(ax, "y_error_bars")([1.0], [1.0], [2.0])
    assert str(caught.value) == (
        "Axes.y_error_bars(): low[0] is 1.0; values must be times on a time axis."
    )
    with pytest.raises(ub.DataError) as caught:
        ax.y_error_bars([1.0], [dt.date(2026, 1, 5)], [dt.date(2026, 1, 4)])
    assert str(caught.value) == (
        "Axes.y_error_bars(): low[0] is 2026-01-05, above high[0] = 2026-01-04; low must not "
        "exceed high."
    )


def test_bar_on_time_axis_rejected() -> None:
    ax = _time_axes().line(list(TIMES), [1.0, 2.0, 3.0])
    before = ax.figure.to_svg()
    for name, args, needs in [
        ("bar", (["a"], [1.0]), "a category x axis"),
        ("grouped_bar", (["a"], [ub.Series("s", [1.0])]), "a category x axis"),
        ("stacked_bar", (["a"], [ub.Series("s", [1.0])]), "a category x axis"),
        ("box", (["a"], [[1.0, 2.0]]), "a category x axis"),
        ("violin", (["a"], [[1.0, 2.0]]), "a category x axis"),
        ("hist", ([1.0, 2.0],), "a linear x axis"),
    ]:
        with pytest.raises(ub.UsageError) as caught:
            _call(ax, name)(*args)
        context = "Axes.box()" if name == "box" else f"Axes.{name}()"
        assert _parts(caught.value) == (
            f"{context}: x is a time scale; {context} needs {needs}.",
            "x",
            CATEGORY_HINT,
        )
    assert ax.figure.to_svg() == before


def test_time_and_numeric_axes_do_not_share() -> None:
    fig = ub.Figure(rows=1, cols=2, share_x=ub.Share.ALL)
    fig.add_axes(col=0)
    with pytest.raises(ub.UsageError) as caught:
        fig.add_axes(col=1, x=ub.Scale.time())
    assert str(caught.value) == (
        "Figure.add_axes(): x is Scale.time(); the axes at row 0, col 0 shares its x axis and "
        "has Scale.linear()."
    )


def test_failed_calls_change_nothing() -> None:
    ax = _time_axes().line(list(TIMES), [1.0, 2.0, 3.0]).xlim(TIMES[2], TIMES[0])
    before = ax.figure.to_svg()
    attempts: list[Callable[[], object]] = [
        lambda: ax.line([dt.datetime(2026, 1, 1, tzinfo=dt.UTC)], [1.0]),
        lambda: ax.xlim(TIMES[0], TIMES[2]),
        lambda: ax.xticks([TIMES[0], TIMES[0]]),
        lambda: ax.vspan(TIMES[0], TIMES[2]),
        lambda: _call(ax, "bar")(["a"], [1.0]),
        lambda: ax.xtick_format(ub.TickFormat.percent()),
    ]
    for attempt in attempts:
        with pytest.raises(ub.UmberError):
            attempt()
    assert ax.figure.to_svg() == before


def test_band_and_stacked_area_take_times() -> None:
    ax = _time_axes(ub.Theme.MIDNIGHT)
    days = [dt.date(2026, 1, d) for d in range(1, 6)]
    ax.band(days, [1.0, 1.5, 1.0, 2.0, 1.5], [3.0, 3.5, 3.0, 4.0, 3.5], label="Range")
    ax.stacked_area(days, [ub.Series("A", [1.0, 2.0, 1.0, 2.0, 1.0])])
    ax.y_error_bars(days[:2], [1.0, 2.0], [2.0, 3.0]).stem(days, [1.0] * 5)
    ax.x_error_bars([2.5], [days[1]], [days[3]])
    assert ax.figure.to_svg().startswith("<svg")
    with pytest.raises(ub.UsageError) as caught:
        _call(ax, "bubbles")(days, [1.0] * 5, [1.0] * 5)
    assert str(caught.value) == (
        "Axes.bubbles(): x is a time scale; Axes.bubbles() needs a linear or log x axis."
    )


def test_scale_time_is_a_value() -> None:
    scale = ub.Scale.time()
    assert scale == ub.Scale.time()
    assert hash(scale) == hash(ub.Scale.time())
    assert scale != ub.Scale.linear()
    assert repr(scale) == "Scale.time()"
    assert copy.copy(scale) is scale
    assert pickle.loads(pickle.dumps(scale)) == scale
    assert ub.TickFormat.iso() == ub.TickFormat.iso()
    assert repr(ub.TickFormat.iso().affix("", " UTC")) == "TickFormat.iso().affix(suffix=' UTC')"


def test_micros_returns_an_aligned_int64_array() -> None:
    values: npt.NDArray[np.int64] = _time.micros("Axes.line()", "x", [])
    assert values.dtype == np.int64 and values.shape == (0,)
    with pytest.raises(TypeError):
        _time.micros("Axes.line()", "x", dt.datetime(2026, 1, 1))
    with pytest.raises(ub.DataError) as caught:
        _time.micros("Axes.line()", "x", np.zeros((2, 2), dtype="datetime64[s]"))
    assert str(caught.value) == ("Axes.line(): x has 2 dimensions; values must be one-dimensional.")


def test_byte_order_and_unit_multipliers_convert_exactly() -> None:
    swapped = np.array(["1970-01-01T00:00:00.000001", "NaT"], dtype=">M8[us]")
    np.testing.assert_array_equal(_time.micros("Axes.line()", "x", swapped), [1, NAT])
    swapped_ns = np.array([1500, 2500], dtype=">M8[ns]")
    np.testing.assert_array_equal(_time.micros("Axes.line()", "x", swapped_ns), [2, 2])
    # 2500-01-01 in tens of nanoseconds: valid, though its nanoseconds overflow int64.
    tens = np.array([1672522560000000000, NAT], dtype="datetime64[10ns]")
    np.testing.assert_array_equal(
        _time.micros("Axes.line()", "x", tens), [_us(dt.datetime(2500, 1, 1)), NAT]
    )
    halves = np.array([3, 5, -3], dtype="datetime64[500ns]")
    np.testing.assert_array_equal(_time.micros("Axes.line()", "x", halves), [2, 2, -2])
    np.testing.assert_array_equal(
        _time.micros("Axes.line()", "x", np.array([2, NAT], dtype="datetime64[3h]")),
        [6 * 3600 * 10**6, NAT],
    )
    # Values too far from 1970 for microseconds give the documented error, never a raw one.
    for far in [
        np.array([0, 2**62], dtype="datetime64[10s]"),
        np.array([0, 2**62], dtype="datetime64[3M]"),
        np.array([0, -(2**62)], dtype=">M8[D]"),
    ]:
        with pytest.raises(ub.DataError) as caught:
            _time.micros("Axes.line()", "x", far)
        message, parameter, _ = _parts(caught.value)
        assert message.startswith("Axes.line(): x[1] is "), message
        assert message.endswith("; times must lie between 0001-01-01 and 9999-12-31."), message
        assert parameter == "x"
