"""Static typing expectations for ``Figure`` and ``Axes``.

Checked by pyright and mypy in strict mode. The marker convention is the one described in
``check_errors.py``: a line that must produce an error carries both
``# type: ignore[<mypy-code>]`` and ``# pyright: ignore[<rule>]``.
"""

from typing import assert_type

import umber as ub

t = [0.0, 1.0, 2.0]
y = [1.0, 2.0, 3.0]

# Positive: a default figure has numeric axes and every chain step keeps them.
ax = ub.Figure().add_axes()
assert_type(ax, ub.Axes[float, float])
assert_type(ax.title("Signal"), ub.Axes[float, float])
assert_type(ax.subtitle("s").xlabel("x").ylabel("y"), ub.Axes[float, float])
assert_type(ax.line(t, y), ub.Axes[float, float])
assert_type(ax.line(t, y).xlim(0, 10), ub.Axes[float, float])
assert_type(ax.line(t, y).xlim(0, 10).ylim(0, 5), ub.Axes[float, float])
assert_type(ax.scatter(t, y).legend(ub.LegendStyle.CHIPS), ub.Axes[float, float])
assert_type(ax.hist(y, bins=10), ub.Axes[float, float])
assert_type(ax.figure, ub.Figure)

# Positive: log scales are still continuous; a category x axis changes the type.
assert_type(ub.Figure().add_axes(x=ub.Scale.log(2)), ub.Axes[float, float])
assert_type(ub.Figure().add_axes(y=ub.Scale.log()), ub.Axes[float, float])
cats = ub.Figure().add_axes(x=ub.Scale.category())
assert_type(cats, ub.Axes[str, float])
assert_type(cats.bar(["A", "B"], [3, 5]), ub.Axes[str, float])
assert_type(cats.bar(["A", "B"], [3, 5]).title("Sales").ylim(0, 10), ub.Axes[str, float])
assert_type(
    cats.grouped_bar(["A", "B"], [ub.BarSeries("North", [1, 2])]),
    ub.Axes[str, float],
)
assert_type(
    cats.stacked_bar(["A", "B"], [ub.BarSeries("North", [1, 2])]),
    ub.Axes[str, float],
)
assert_type(cats.line(["A", "B"], [1, 2]), ub.Axes[str, float])

# Negative: bar needs a category x axis.
ax.bar(["A", "B"], [3, 5])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]

# Negative: a category x axis refuses numeric x values, hist and xlim.
cats.line([0.1, 0.2], [1, 2])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
cats.hist([1.0])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
cats.xlim(0, 1)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]

# Negative: the y axis is always continuous (the parameter is ``Scale[float]``).
ub.Figure().add_axes(y=ub.Scale.category())  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: mark methods that need a category axis refuse a numeric one for the series too.
ax.grouped_bar(["A"], [ub.BarSeries("a", [1])])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
ax.stacked_bar(["A"], [ub.BarSeries("a", [1])])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
