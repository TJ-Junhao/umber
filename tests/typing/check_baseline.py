"""Baseline settings preserve the axes type and require an explicit boolean or AUTO."""

from typing import assert_type

import umber as ub

ax = ub.Figure().add_axes()
assert_type(ax.baseline(shown=True), ub.Axes[float, float])
assert_type(ax.baseline(shown=False), ub.Axes[float, float])
assert_type(ax.baseline(shown=ub.AUTO), ub.Axes[float, float])
categories = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
assert_type(categories.baseline(shown=False), ub.Axes[str, str])
assert_type(categories.baseline(shown=ub.AUTO), ub.Axes[str, str])

ax.baseline(shown=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.baseline(shown=1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.baseline(shown="yes")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.baseline()  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ax.baseline(True)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
