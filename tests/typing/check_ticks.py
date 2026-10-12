"""Static typing expectations for tick control and ``TickFormat``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

from typing import assert_type

import umber as ub

numeric = ub.Figure().add_axes()
cats = ub.Figure().add_axes(x=ub.Scale.category())
t = [0.0, 1.0]

# Positive: positions and labels from lists, tuples and arrays; every method chains.
assert_type(numeric.xticks(t), ub.Axes[float, float])
assert_type(numeric.yticks([0, 1]), ub.Axes[float, float])
assert_type(numeric.xticks_labeled(t, ["a", "b"]), ub.Axes[float, float])
assert_type(numeric.yticks_labeled(t, ("a", "b")), ub.Axes[float, float])
assert_type(numeric.xtick_format(ub.TickFormat.percent()), ub.Axes[float, float])
assert_type(numeric.ytick_format(ub.TickFormat.AUTO), ub.Axes[float, float])
chained: ub.Axes[float, float] = numeric.xticks(t).yticks(t).xtick_format(ub.TickFormat.AUTO)

# Positive: the y methods accept a category chart, whose x axis stays a category axis.
assert_type(cats.yticks(t), ub.Axes[str, float])
assert_type(cats.yticks_labeled(t, ["a", "b"]), ub.Axes[str, float])
assert_type(cats.ytick_format(ub.TickFormat.fixed(1)), ub.Axes[str, float])

# Positive: the format constructors and their results.
assert_type(ub.TickFormat.AUTO, ub.TickFormat)
assert_type(ub.TickFormat.fixed(2), ub.TickFormat)
assert_type(ub.TickFormat.percent(), ub.TickFormat)
assert_type(ub.TickFormat.percent(1).affix(suffix=" pts"), ub.TickFormat)
assert_type(ub.TickFormat.scientific().affix("~", "!"), ub.TickFormat)
fmt: ub.TickFormat = ub.TickFormat.fixed(decimals=2)

# Negative: a category x axis has no x ticks and no x tick format.
cats.xticks([1.0])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
cats.xticks_labeled([1.0], ["a"])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
cats.xtick_format(ub.TickFormat.AUTO)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]

# Negative: a format is a TickFormat, not a string or a tuple.
numeric.xtick_format("percent")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.ytick_format((1, 2, "", ""))  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]

# Negative: positions are numbers and labels are strings.
numeric.xticks(["a"])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
numeric.xticks_labeled(t, [1, 2])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]

# Negative: decimals and the affix text are an int and strings; there is no public constructor.
ub.TickFormat.fixed("2")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.TickFormat.percent(1.5)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.TickFormat.AUTO.affix(1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.TickFormat(2)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
