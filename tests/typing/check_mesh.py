"""Static typing expectations for numeric cells and two-dimensional counts."""

import datetime as dt
from typing import assert_type

import numpy as np

import umber as ub

ax = ub.Figure().add_axes()
x = [0.0, 1.0, 2.0]
y = [1.0, 2.0, 3.0]
assert_type(ax.mesh(x, y, [[1, 2], [3, 4]]), ub.Axes[float, float])
assert_type(ax.hist2d(x, y), ub.Axes[float, float])
assert_type(ax.hist2d(x, y, bins=(4, 5), scale=ub.Scale.log()), ub.Axes[float, float])
assert_type(ax.hexbin(np.arange(3.0), y, gridsize=4), ub.Axes[float, float])
ax.mesh(x, y, np.ones((2, 2)), cmap=ub.Cmap.BLUE_RED, center=0)
ax.hexbin(x, y, cmap=ub.Cmap.MAGMA, scale=ub.Scale.log())
days = [dt.date(2026, 1, 1), dt.date(2026, 1, 2), dt.date(2026, 1, 4)]
timed = ub.Figure().add_axes(x=ub.Scale.time())
assert_type(timed.mesh(days, y, [[1, 2], [3, 4]]), ub.Axes[ub.Time, float])
assert_type(
    timed.mesh(np.array(days, dtype="datetime64[D]"), y, np.ones((2, 2))), ub.Axes[ub.Time, float]
)
up = ub.Figure().add_axes(y=ub.Scale.time())
assert_type(up.mesh(x, days, [[1, 2], [3, 4]]), ub.Axes[float, ub.Time])
timed.mesh(x, y, [[1, 2], [3, 4]])  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
categories = ub.Figure().add_axes(x=ub.Scale.category())
categories.hist2d(x, y)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
categories.hexbin(x, y)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
ax.hist2d(x, y, bins=(2, 3, 4))  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.hexbin(x, y, gridsize=2.5)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.mesh(x, y, [[1, 2], [3, 4]], cmap="viridis")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.hist2d(x, y, scale=ub.Scale.category())  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.hexbin(x, y, 4)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
categories.mesh(x, y, [[1, 2], [3, 4]])  # type: ignore[misc, arg-type]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
rows = ub.Figure().add_axes(y=ub.Scale.category())
rows.mesh(x, y, [[1, 2], [3, 4]])  # type: ignore[misc, arg-type]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
