"""Static typing expectations for ``Axes.y_error_bars`` and ``Axes.x_error_bars``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

import numpy as np

import umber as ub

numeric = ub.Figure().add_axes()
logged = ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.log())
cats = ub.Figure().add_axes(x=ub.Scale.category())
t = [0.0, 1.0]
names = ["a", "b"]

# Positive: y_error_bars on every kind of x axis, with every keyword, arrays, and chaining.
numeric.y_error_bars(t, t, t)
numeric.y_error_bars(t, t, t, label="95% interval", color=ub.Color.BLUE, width=ub.Size.AUTO)
numeric.y_error_bars(t, t, t, caps=False)
logged.y_error_bars([1.0, 2.0], [1.0, 2.0], [3.0, 4.0])
cats.y_error_bars(names, t, t)
numeric.y_error_bars(np.arange(2.0), np.zeros(2), np.ones(2))
chained: ub.Axes[float, float] = numeric.y_error_bars(t, t, t).y_error_bars(t, t, t).line(t, t)
category_chain: ub.Axes[str, float] = cats.y_error_bars(names, t, t).scatter(names, t)

# Positive: x_error_bars needs a numeric x axis.
numeric.x_error_bars(t, t, t)
numeric.x_error_bars(t, t, t, label="x interval", color=ub.Color.BLUE, width=ub.Size.pt(1.0))
logged.x_error_bars([1.0, 2.0], [1.0, 2.0], [3.0, 4.0], caps=False)
both: ub.Axes[float, float] = numeric.x_error_bars(t, t, t).y_error_bars(t, t, t)

# Negative: the self type needs a numeric x axis, so a category axis refuses x_error_bars.
cats.x_error_bars(t, t, t)  # type: ignore[arg-type]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]

# Negative: a category axis takes names, a numeric axis takes numbers.
numeric.y_error_bars(names, t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.y_error_bars(t, t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: the limits and the y values are numbers.
numeric.y_error_bars(t, names, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.y_error_bars(t, t, names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.x_error_bars(names, t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.x_error_bars(t, t, names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: the data are required, and the options are keyword-only typed values.
numeric.y_error_bars(t, t)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.x_error_bars(t, t)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.y_error_bars(t, t, t, "label")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.y_error_bars(t, t, t, color="red")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.y_error_bars(t, t, t, width=2.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.y_error_bars(t, t, t, caps=1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.x_error_bars(t, t, t, label=3)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: error bars have no dash.
numeric.y_error_bars(t, t, t, dash=ub.Dash.DASHED)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
