"""Static typing expectations for streamlines."""

from typing import assert_type

import numpy as np

import umber as ub

ax = ub.Figure().add_axes()
x = [0.0, 1.0, 2.0]
y = [1.0, 2.0]
u = np.ones((2, 3))
assert_type(ax.streamplot(x, y, u, u), ub.Axes[float, float])
assert_type(ax.streamplot(np.arange(3.0), y, [[1.0, 2.0, 3.0]] * 2, u), ub.Axes[float, float])
ax.streamplot(x, y, u, u, label="Flow", density=2, color=ub.Color.RED, arrows=False)
ax.streamplot(x, y, u, u, color=ub.ColorBy(u), starts=([0.5, 1.5], [1.2, 1.8]))
ax.streamplot(x, y, u, u, starts=(np.array([0.5]), np.array([1.5])))
ax.streamplot(x, y, u, u, density=ub.AUTO)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.streamplot(x, y, u, u, starts=ub.AUTO)
logs = ub.Figure().add_axes(x=ub.Scale.log())
assert_type(logs.streamplot([1.0, 10.0, 100.0], y, u, u), ub.Axes[float, float])
ax.streamplot(x, y, u, u, starts=[0.5])  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.streamplot(x, y, u, u, density="dense")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.streamplot(x, y, u, u, arrows="yes")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.streamplot(x, y, u, u, key=(1, "1 m/s"))  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ax.streamplot(x, y, u)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
categories = ub.Figure().add_axes(x=ub.Scale.category())
categories.streamplot(x, y, u, u)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
timed = ub.Figure().add_axes(x=ub.Scale.time())
timed.streamplot(x, y, u, u)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
