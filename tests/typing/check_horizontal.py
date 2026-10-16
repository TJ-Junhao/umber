"""Category y axes: horizontal bars, boxes and violins, and dot plots.

Checked by pyright and mypy in strict mode, with the marker convention of ``check_errors.py``.
"""

from typing import assert_type

import umber as ub

names = ["North", "South"]
values = [3.0, 5.0]
groups = [[1.0, 2.0], [3.0]]
series = [ub.Series("2025", [1.0, 2.0]), ub.Series("2026", [2.0, 3.0])]
d = ub.Distribution("Samples", groups)

# Positive: a category y axis gives horizontal axes, from every way of adding axes.
fig = ub.Figure(cols=2)
h = fig.add_axes(col=0, y=ub.Scale.category())
assert_type(h, ub.Axes[float, str])
assert_type(fig.axes_at(col=0, y=ub.Scale.category()), ub.Axes[float, str])
assert_type(ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.category()), ub.Axes[float, str])
assert_type(next(ub.Figure().cells()).add_axes(y=ub.Scale.category()), ub.Axes[float, str])
both = ub.Figure().add_axes(x=ub.Scale.category(), y=ub.Scale.category())
assert_type(both, ub.Axes[str, str])

# Positive: the bar, box and violin families keep the type of the axes they draw on.
assert_type(h.bar(names, values), ub.Axes[float, str])
assert_type(h.grouped_bar(names, series), ub.Axes[float, str])
assert_type(h.stacked_bar(names, series), ub.Axes[float, str])
assert_type(h.box(names, groups), ub.Axes[float, str])
assert_type(h.grouped_box(names, [d]), ub.Axes[float, str])
assert_type(h.violin(names, groups), ub.Axes[float, str])
assert_type(h.grouped_violin(names, [d]), ub.Axes[float, str])
cats = ub.Figure().add_axes(x=ub.Scale.category())
assert_type(cats.bar(names, values), ub.Axes[str, float])
assert_type(cats.violin(names, groups), ub.Axes[str, float])

# Positive: a dot plot, horizontal error bars, x limits and vertical references.
assert_type(h.scatter(values, names), ub.Axes[float, str])
assert_type(h.x_error_bars(names, [0.0, 1.0], [1.0, 2.0]), ub.Axes[float, str])
chained: ub.Axes[float, str] = (
    h.bar(names, values, label="Share")
    .box(names, groups)
    .xlim(0, 10)
    .vline(5.0)
    .vspan(1.0, 2.0)
    .xtick_format(ub.TickFormat.percent())
    .title("Share")
)

# Negative: the bar, box and violin families need one category axis beside a numeric one.
numeric = ub.Figure().add_axes()
numeric.bar(names, values)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
both.bar(names, values)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
both.grouped_bar(names, series)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
both.stacked_bar(names, series)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
both.box(names, groups)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
both.violin(names, groups)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]

# Negative: categories are names and values numbers, on either orientation.
h.bar(values, values)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
h.bar(names, names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: a dot plot takes numbers along x and names along y.
h.scatter(names, names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
h.scatter(values, values)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
h.x_error_bars(values, [0.0, 1.0], [1.0, 2.0])  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: a category y axis has no y limits, and histograms bin numbers along x into y.
h.ylim(0, 1)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
h.hist(values)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
