"""Static typing expectations for arrows and wind barbs."""

import datetime as dt
from typing import assert_type

import numpy as np

import umber as ub

ax = ub.Figure().add_axes()
x = [0.0, 1.0, 2.0]
y = [1.0, 2.0]
u = np.ones((2, 3))
assert_type(ax.quiver(x, y, u, u), ub.Axes[float, float])
assert_type(ax.quiver(x, x, x, x), ub.Axes[float, float])
assert_type(ax.barbs(x, y, u, u), ub.Axes[float, float])
ax.quiver(x, y, u, u, color=ub.ColorBy(u), pivot=ub.Pivot.TIP, direction=ub.Direction.DATA)
ax.quiver(x, y, u, u, pt_per_unit=3, key=(10, "10 m/s"))
ax.quiver(x, y, u, u, pt_per_unit=ub.AUTO, key=ub.AUTO)
ax.barbs(x, y, u, u, label="Wind", color=ub.Color.RED, size=ub.Size.pt(30), flip=True)
timed = ub.Figure().add_axes(x=ub.Scale.time())
times = [dt.date(2026, 1, 1), dt.date(2026, 1, 2), dt.date(2026, 1, 3)]
assert_type(timed.barbs(times, y, u, u), ub.Axes[ub.Time, float])
assert_type(timed.quiver(times, y, u, u), ub.Axes[ub.Time, float])
ax.quiver(x, y, u, u, pivot="tail")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.quiver(x, y, u, u, direction="data")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.quiver(x, y, u, u, key=10)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.quiver(x, y, u, u, label="Wind")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ax.barbs(x, y, u, u, pivot=ub.Pivot.TAIL)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ax.barbs(x, y, u, u, flip="yes")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.quiver(x, y)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
categories = ub.Figure().add_axes(x=ub.Scale.category())
categories.quiver(["a"], y, u, u)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
