"""Static typing expectations for the quick functions.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

from typing import assert_type

import umber as ub

t = [0.0, 1.0, 2.0]

# Positive: return types, and chaining through ``figure``.
assert_type(ub.line(t, t), ub.Axes[float, float])
assert_type(ub.scatter(t, t, marker=ub.Marker.TRIANGLE), ub.Axes[float, float])
assert_type(ub.scatter(t, t, color=ub.ColorBy(t)), ub.Axes[float, float])
assert_type(ub.scatter(t, t, color=ub.ColorBy(t, ub.Cmap.MAGMA, center=1)), ub.Axes[float, float])
assert_type(ub.bar(["A", "B"], [3, 5]), ub.Axes[str, float])
assert_type(ub.hist(t, bins=5), ub.Axes[float, float])
assert_type(ub.line(t, t, theme=ub.Theme.GRAPHITE).title("Signal"), ub.Axes[float, float])
assert_type(ub.line(t, t).figure, ub.Figure)
quick_bar = ub.bar(["A"], [1])
titled = quick_bar.title("Sales")
assert_type(titled, ub.Axes[str, float])
assert_type(titled.ylim(0, 4), ub.Axes[str, float])

# Negative: invariance of the returned axes.
as_numeric: ub.Axes[float, float] = ub.bar(["A"], [1])  # type: ignore[assignment]  # pyright: ignore[reportAssignmentType]
as_text: ub.Axes[str, float] = ub.line(t, t)  # type: ignore[assignment]  # pyright: ignore[reportAssignmentType]

# Negative: a bar chart cannot take numeric x limits, a line chart cannot take a category.
ub.bar(["A"], [1]).xlim(0, 1)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
ub.line(["A"], [1])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
ub.hist(t, theme="graphite")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
