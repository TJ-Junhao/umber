"""Static typing expectations for time axes.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

import datetime as dt
from typing import assert_type

import numpy as np
import pandas as pd

import umber as ub

times = [dt.datetime(2026, 1, 1), dt.datetime(2026, 1, 2)]
days = [dt.date(2026, 1, 1), dt.date(2026, 1, 2)]
stamps = np.array(["2026-01-01", "2026-01-02"], dtype="datetime64[D]")
y = [1.0, 2.0]

# Positive: a time scale gives a time axis on x or on y, and every chain step keeps it.
assert_type(ub.Scale.time(), ub.Scale[ub.Time])
timed = ub.Figure().add_axes(x=ub.Scale.time())
assert_type(timed, ub.Axes[ub.Time, float])
assert_type(timed.line(times, y), ub.Axes[ub.Time, float])
assert_type(timed.line(days, y).scatter(stamps, y), ub.Axes[ub.Time, float])
assert_type(timed.step(pd.Series(pd.to_datetime(["2026-01-01"])), [1.0]), ub.Axes[ub.Time, float])
assert_type(timed.area(times, y).stem(times, y), ub.Axes[ub.Time, float])
assert_type(timed.band(times, y, y).stacked_area(times, []), ub.Axes[ub.Time, float])
assert_type(timed.y_error_bars(times, y, y), ub.Axes[ub.Time, float])
assert_type(
    timed.xlim(dt.datetime(2026, 1, 1), np.datetime64("2026-02-01")),
    ub.Axes[ub.Time, float],
)
assert_type(timed.xticks(days).xticks_labeled(days, ["a", "b"]), ub.Axes[ub.Time, float])
assert_type(timed.xtick_format(ub.TickFormat.iso()), ub.Axes[ub.Time, float])
assert_type(timed.vline(dt.date(2026, 1, 1)).vspan(days[0], days[1]), ub.Axes[ub.Time, float])
assert_type(timed.hline(2.0).hspan(1.0, 2.0).ylim(0, 5), ub.Axes[ub.Time, float])
when: ub.Time = dt.date(2026, 1, 1)
also: ub.Time = np.datetime64("2026-01-01")

up = ub.Figure().add_axes(y=ub.Scale.time())
assert_type(up, ub.Axes[float, ub.Time])
assert_type(up.line(y, times).ylim(days[0], days[1]), ub.Axes[float, ub.Time])
assert_type(up.hline(days[0]).hspan(days[0], days[1]), ub.Axes[float, ub.Time])
assert_type(up.yticks(days).yticks_labeled(days, ["a", "b"]), ub.Axes[float, ub.Time])
assert_type(up.x_error_bars(days, y, y).xlim(0, 1), ub.Axes[float, ub.Time])
both = ub.Figure().add_axes(x=ub.Scale.time(), y=ub.Scale.time())
assert_type(both, ub.Axes[ub.Time, ub.Time])
assert_type(ub.Figure().axes_at(x=ub.Scale.time()), ub.Axes[ub.Time, float])

# Negative: bars, histograms, boxes, violins and bubbles need numbers or names, not times.
timed.bar(["a"], [1.0])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
timed.hist([1.0])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
timed.box(["a"], [[1.0]])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
timed.violin(["a"], [[1.0]])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
timed.bubbles(y, y, y)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]

# Error bar limits along a time axis are times.
assert_type(up.y_error_bars(y, days, days), ub.Axes[float, ub.Time])
assert_type(timed.x_error_bars(y, times, times), ub.Axes[ub.Time, float])
up.y_error_bars(y, y, y)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
timed.x_error_bars(y, y, y)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: values of the wrong kind for the axis.
timed.line(["a"], [1.0])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
timed.xlim(0, 1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
timed.vline(1.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Figure().add_axes().xlim(days[0], days[1])  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
up.hline(1.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
not_a_time: ub.Time = 3.0  # type: ignore[assignment]  # pyright: ignore[reportAssignmentType]
