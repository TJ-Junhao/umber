"""Static typing expectations for triangle meshes and their contours."""

from typing import assert_type

import numpy as np
import numpy.typing as npt

import umber as ub

ax = ub.Figure().add_axes()
x = [0.0, 1.0, 1.0, 0.0]
y = [0.0, 0.0, 1.0, 1.0]
tri = ub.delaunay(x, y)
assert_type(tri, npt.NDArray[np.int64])
assert_type(ub.delaunay(np.arange(4.0), y), npt.NDArray[np.int64])
assert_type(ax.triplot(x, y), ub.Axes[float, float])
assert_type(
    ax.triplot(x, y, triangles=tri[:1], label="Mesh", color=ub.Color.RED), ub.Axes[float, float]
)
ax.triplot(x, y, triangles=[[0, 1, 2]])
ax.triplot(x, y, triangles=ub.AUTO)
assert_type(ax.tripcolor(x, y, [1.0, 2.0, 3.0, 4.0]), ub.Axes[float, float])
ax.tripcolor(x, y, [1.0, 2.0], triangles=tri, on=ub.TriValues.TRIANGLES)
ax.tripcolor(x, y, x, cmap=ub.Cmap.VIRIDIS, scale=ub.Scale.log(), center=ub.AUTO)
ax.tripcolor(x, y, x, center=0.5)
assert_type(ax.tricontour(x, y, [1.0, 2.0, 3.0, 4.0]), ub.Axes[float, float])
ax.tricontour(x, y, x, triangles=tri, levels=3, style=ub.Contour.BOTH, annotate=True)
ax.tricontour(x, y, np.arange(4.0), levels=[0.5, 1.5], cmap=ub.Cmap.MAGMA, center=1.0)
ax.tricontour(x, y, x, color=ub.Color.RED, annotate=True, format=ub.TickFormat.fixed(1))
ax.tricontour(x, y, x, scale=ub.Scale.log(), levels=ub.AUTO, triangles=ub.AUTO)
logs = ub.Figure().add_axes(x=ub.Scale.log())
assert_type(logs.triplot([1.0, 10.0, 100.0], y[:3]), ub.Axes[float, float])
ax.triplot(x, y, triangles="012")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.triplot(x, y, label=3)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.tripcolor(x, y, x, on="points")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.tripcolor(x, y)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ax.tricontour(x, y)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ax.tricontour(x, y, x, style="filled")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.tricontour(x, y, x, levels="auto")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.tricontour(x, y, x, tri)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ax.triplot(x, y, cmap=ub.Cmap.VIRIDIS)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
categories = ub.Figure().add_axes(x=ub.Scale.category())
categories.triplot(x, y)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
timed = ub.Figure().add_axes(x=ub.Scale.time())
timed.tripcolor(x, y, x)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
categories.tricontour(x, y, x)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
