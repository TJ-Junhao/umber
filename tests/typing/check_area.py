"""Static typing expectations for ``Axes.area`` and ``FillStyle``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

import umber as ub

numeric = ub.Figure().add_axes()
cats = ub.Figure().add_axes(x=ub.Scale.category())
t = [0.0, 1.0]
names = ["a", "b"]

# Positive: numeric and category x, every keyword, and chaining.
numeric.area(t, t)
numeric.area(t, t, label="visitors", color=ub.Color.BLUE, dash=ub.Dash.DASHED)
numeric.area(t, t, width=ub.Size.times(2)).area(t, t, width=ub.Size.pt(1.5))
cats.area(names, t, label="sales")
chained: ub.Axes[float, float] = numeric.area(t, t).line(t, t)
chained_cats: ub.Axes[str, float] = cats.area(names, t).scatter(names, t)
theme: ub.Theme = ub.Theme.PIGMENT.with_fill_style(ub.FillStyle.GRADIENT)
fill: ub.FillStyle = ub.FillStyle.SOLID

# Negative: the fill style is an enum member, not a string or a number.
ub.Theme.PIGMENT.with_fill_style("solid")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Theme.PIGMENT.with_fill_style(0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: the width is a Size, not a number.
numeric.area(t, t, width=2.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: x must match the axes.
numeric.area(names, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.area(t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: style parameters are keyword-only.
numeric.area(t, t, "label")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
