"""Static typing expectations for ``Axes.hline``, ``vline``, ``hspan`` and ``vspan``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

import umber as ub

numeric = ub.Figure().add_axes()
logged = ub.Figure().add_axes(x=ub.Scale.log())
cats = ub.Figure().add_axes(x=ub.Scale.category())
t = [0.0, 1.0]
names = ["a", "b"]

# Positive: every keyword, both x kinds, and chaining.
numeric.hline(2.5)
numeric.hline(2, label="Threshold", color=ub.Color.RED, dash=ub.Dash.DASHED, width=ub.Size.AUTO)
numeric.vline(31).vline(2.5, label="Event", width=ub.Size.times(2))
cats.vline("Q2", label="Launch", dash=ub.Dash.DOTTED)
numeric.hspan(1, 2.5, label="Normal range", color=ub.Color.BLUE)
numeric.vspan(40, 50).vspan(1.0, 2.0, label="Window")
logged.vspan(1, 10)
cats.hline(3.0).hspan(1.0, 2.0)
chained: ub.Axes[float, float] = numeric.hline(1).vline(1).hspan(1, 2).vspan(1, 2).line(t, t)
chained_cats: ub.Axes[str, float] = cats.hline(1).vline("a").hspan(1, 2).scatter(names, t)

# Negative: vspan needs a numeric x axis, so it is refused on a category axis.
cats.vspan(0, 1)  # type: ignore[arg-type]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]

# Negative: x of a vline matches the x axis.
numeric.vline("Q1")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.vline(1.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: values are numbers.
numeric.hline("2")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hspan(1, "2")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hspan(t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: styles are the typed options, and keyword-only.
numeric.hline(1, dash="--")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hline(1, width=2.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hline(1, color="red")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hline(1, "label")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.hspan(1, 2, "label")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]

# Negative: spans have no dash or width.
numeric.hspan(1, 2, dash=ub.Dash.DASHED)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.vspan(1, 2, width=ub.Size.AUTO)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
