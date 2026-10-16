"""Static typing expectations for heatmaps on two category axes."""

from typing import assert_type

import numpy as np

import umber as ub

grid = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
numeric = ub.Figure().add_axes()
columns = ub.Figure().add_axes(x=ub.Scale.category())
rows = ub.Figure().add_axes(y=ub.Scale.category())
names = ["a", "b"]
numbers = [1.0, 2.0]
cells = [[1.0, 2.0], [3.0, 4.0]]

# Positive: on two category axes, with every option, the axes keep their type.
assert_type(grid.heatmap(names, names, cells), ub.Axes[str, str])
assert_type(grid.heatmap(names, names, np.ones((2, 2))), ub.Axes[str, str])
grid.heatmap(
    names,
    names,
    cells,
    cmap=ub.Cmap.MAGMA,
    scale=ub.Scale.log(),
    center=ub.AUTO,
    annotate=True,
    format=ub.TickFormat.fixed(2),
)
grid.heatmap(names, names, cells, cmap=ub.AUTO, center=0, format=ub.AUTO)
assert_type(grid.heatmap(names, names, cells).colorbar("Value").clim(0, 4), ub.Axes[str, str])

# Negative: any other axes, and wrong argument types.
numeric.heatmap(names, names, cells)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
columns.heatmap(names, names, cells)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
rows.heatmap(names, names, cells)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
grid.heatmap(numbers, names, cells)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
grid.heatmap(names, names, cells, cmap="viridis")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
grid.heatmap(names, names, cells, scale=ub.Scale.category())  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
grid.heatmap(names, names, cells, center="0")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
grid.heatmap(names, names, cells, annotate=1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
grid.heatmap(names, names, cells, format=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
grid.heatmap(names, names, cells, ub.Cmap.MAGMA)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
grid.heatmap(names, names)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
