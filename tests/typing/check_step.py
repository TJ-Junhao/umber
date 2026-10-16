"""Static typing expectations for ``Axes.step`` and ``Step``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

import umber as ub


def generic_step[X](
    axes: ub.Axes[X, float], x: ub.Values[X], y: ub.Values[float]
) -> ub.Axes[X, float]:
    return axes.step(x, y)


numeric = ub.Figure().add_axes()
cats = ub.Figure().add_axes(x=ub.Scale.category())
t = [0.0, 1.0]
names = ["a", "b"]

# Positive: numeric and category x, every keyword, every position, and chaining.
numeric.step(t, t)
numeric.step(t, t, where=ub.Step.PRE, label="levels", color=ub.Color.BLUE, dash=ub.Dash.DASHED)
numeric.step(t, t, width=ub.Size.times(2)).step(t, t, width=ub.Size.pt(1.5))
cats.step(names, t, where=ub.Step.MID, label="sales")
chained: ub.Axes[float, float] = numeric.step(t, t).line(t, t)
chained_cats: ub.Axes[str, float] = cats.step(names, t).scatter(names, t)
position: ub.Step = ub.Step.POST
members: list[ub.Step] = list(ub.Step)

# Negative: the position is a Step member, not a string or a number.
numeric.step(t, t, where="post")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.step(t, t, where=0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: the width is a Size, not a number.
numeric.step(t, t, width=2.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: x must match the axes, and y holds numbers.
numeric.step(names, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.step(t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.step(t, names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: style parameters are keyword-only.
numeric.step(t, t, ub.Step.POST)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
