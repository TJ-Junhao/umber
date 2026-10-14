"""Static typing for grid positions, cell iteration and exact scale witnesses.

Checked by pyright and mypy; see ``check_errors.py`` for the marker convention.
"""

from collections.abc import Iterator
from typing import assert_type

import umber as ub

fig = ub.Figure(rows=2, cols=2, share_x=ub.Share.COL, legend=ub.Legend.MERGED)
assert_type(fig.add_axes(row=0, col=1), ub.Axes[float, float])
assert_type(fig.axes_at(x=ub.Scale.category()), ub.Axes[str, float])
assert_type(fig.axes_at(), ub.Axes[float, float])
assert_type(fig.axes_at(x=ub.Scale.log(2)), ub.Axes[float, float])
assert_type(fig.cells(), Iterator[ub.Cell])
assert_type(next(fig.cells()), ub.Cell)
cell = next(fig.cells())
assert_type(cell.row, int)
assert_type(cell.col, int)
assert_type(cell.add_axes(), ub.Axes[float, float])
assert_type(cell.add_axes(x=ub.Scale.category()), ub.Axes[str, float])

ub.Figure(ub.AUTO, 4, rows=2)
fig.add_axes(row=ub.AUTO, col=ub.AUTO)
fig.axes_at(row=ub.AUTO, col=ub.AUTO)
fig.cells(ub.AUTO)

fig.add_axes(row="0")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.cells(count=2)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ub.Figure(width=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Figure(rows=ub.AUTO)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.add_axes(row_span=ub.AUTO)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cell.row = 1  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue]
