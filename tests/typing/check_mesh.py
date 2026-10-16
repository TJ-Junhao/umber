"""Static typing expectations for numeric cells and two-dimensional counts."""

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
categories = ub.Figure().add_axes(x=ub.Scale.category())
categories.hist2d(x, y)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
categories.hexbin(x, y)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
ax.hist2d(x, y, bins=(2, 3, 4))  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.hexbin(x, y, gridsize=2.5)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.mesh(x, y, [[1, 2], [3, 4]], cmap="viridis")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.hist2d(x, y, scale=ub.Scale.category())  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.hexbin(x, y, 4)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
