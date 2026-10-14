"""Static typing expectations for ``Axes.band``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

import numpy as np

import umber as ub

numeric = ub.Figure().add_axes()
logged = ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.log())
cats = ub.Figure().add_axes(x=ub.Scale.category())
t = [0.0, 1.0]
names = ["a", "b"]

# Positive: numeric and log axes, every keyword, arrays, and chaining.
numeric.band(t, t, t)
numeric.band(t, t, t, label="95% interval", color=ub.Color.BLUE)
logged.band([1.0, 2.0], [1.0, 2.0], [3.0, 4.0])
numeric.band(np.arange(2.0), np.zeros(2), np.ones(2))
chained: ub.Axes[float, float] = numeric.band(t, t, t).band(t, t, t).line(t, t)
logged_y: ub.Axes[float, float] = logged.band(t, t, t)

# Negative: the self type needs a numeric x axis, so a category axis refuses a band.
cats.band(t, t, t)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]

# Negative: all three arrays are numbers.
numeric.band(names, t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.band(t, names, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.band(t, t, names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: the arrays are required, and the styles are keyword-only typed options.
numeric.band(t, t)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.band(t, t, t, "label")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.band(t, t, t, color="red")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.band(t, t, t, label=3)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: a band has no outline, dash or width.
numeric.band(t, t, t, dash=ub.Dash.DASHED)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.band(t, t, t, width=ub.Size.AUTO)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
