"""Static typing expectations for contours."""

from typing import assert_type

import numpy as np

import umber as ub

ax = ub.Figure().add_axes()
x = [0.0, 1.0, 2.0]
y = [1.0, 2.0]
z = [[0.0, 1.0, 2.0], [1.0, 2.0, 3.0]]
assert_type(ax.contour(x, y, z), ub.Axes[float, float])
assert_type(ax.contour(np.arange(3.0), y, np.ones((2, 3))), ub.Axes[float, float])
assert_type(ax.contour(x, y, z, levels=5), ub.Axes[float, float])
assert_type(ax.contour(x, y, z, levels=[0.5, 1.5]), ub.Axes[float, float])
assert_type(ax.contour(x, y, z, levels=np.array([0.5, 1.5])), ub.Axes[float, float])
assert_type(ax.contour(x, y, z, levels=ub.AUTO), ub.Axes[float, float])
ax.contour(x, y, z, style=ub.Contour.FILLED, cmap=ub.Cmap.MAGMA, scale=ub.Scale.log())
ax.contour(x, y, z, style=ub.Contour.BOTH, center=0, annotate=True)
ax.contour(x, y, z, color=ub.Color.RED, annotate=True, format=ub.TickFormat.fixed(1))
ax.contour(x, y, z, style="filled")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.contour(x, y, z, levels="auto")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.contour(x, y, z, color="red")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.contour(x, y, z, annotate=1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.contour(x, y, z, format="%.1f")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.contour(x, y, z, 5)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ax.contour(x, y)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
categories = ub.Figure().add_axes(x=ub.Scale.category())
categories.contour(x, y, z)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
timed = ub.Figure().add_axes(x=ub.Scale.time())
timed.contour(x, y, z)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
