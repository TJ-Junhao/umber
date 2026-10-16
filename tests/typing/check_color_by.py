"""Static typing expectations for ``ColorBy`` and ``Axes.clim()``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

from typing import assert_type

import numpy as np

import umber as ub

numeric = ub.Figure().add_axes()
cats = ub.Figure().add_axes(x=ub.Scale.category())
t = [1.0, 2.0]
names = ["a", "b"]

# Positive: every accepted spelling, and ``color=ColorBy(...)`` on scatter and bubbles.
by = ub.ColorBy(t)
assert_type(by, ub.ColorBy)
ub.ColorBy(np.arange(2.0), ub.Cmap.MAGMA, scale=ub.Scale.log(2), center=ub.AUTO)
ub.ColorBy(t, ub.AUTO, center=0)
ub.ColorBy(t, cmap=ub.Cmap.BLUE_RED.reversed(), center=0.5)
assert_type(numeric.scatter(t, t, color=by), ub.Axes[float, float])
assert_type(cats.scatter(["a", "b"], t, color=by), ub.Axes[str, float])
assert_type(numeric.bubbles(t, t, t, label="Population", color=by), ub.Axes[float, float])
assert_type(numeric.clim(0, 10), ub.Axes[float, float])
assert_type(cats.clim(0.5, 1.5), ub.Axes[str, float])
numeric.scatter(t, t, color=ub.Color.BLUE)

# Negative: wrong argument types and positions, and ColorBy on marks without per-point colors.
ub.ColorBy(t, "viridis")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.ColorBy(names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.ColorBy(t, scale=ub.Scale.category())  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.ColorBy(t, ub.Cmap.MAGMA, ub.Scale.linear())  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ub.ColorBy(t, center="0")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.line(t, t, color=by)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.area(t, t, color=by)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.stem(t, t, color=by)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.bar(["a", "b"], t, color=by)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.clim("0", 1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
