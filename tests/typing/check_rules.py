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

# Positive: Between places lines and span ends between categories of a category axis.
rows = ub.Figure().add_axes(y=ub.Scale.category())
divider = ub.Between("Q1", "Q2")
by_category: ub.Axes[str, float] = cats.vline(divider, dash=ub.Dash.DASHED).vspan(divider, divider)
by_row: ub.Axes[float, str] = rows.hline(divider, label="Split").hspan(divider, divider)

# Negative: Between needs a category axis along its line.
numeric.vline(divider)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hline(divider)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.vspan(divider, divider)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.hline(divider)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
rows.vspan(divider, divider)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: a category y axis takes no number, and a span's ends are both Between.
rows.hline(2.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.vspan(divider, "Q3")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Between("Q1", 2)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: vspan on a category axis takes Between ends, not names.
cats.vspan("a", "b")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: x of a vline matches the x axis.
numeric.vline("Q1")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.vline(1.0)  # type: ignore[call-overload]  # pyright: ignore[reportCallIssue, reportArgumentType]

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
