"""Static typing expectations for ``hist`` variants, ``ecdf`` and ``kde``.

Checked by pyright and mypy in strict mode; see ``check_errors.py`` for the marker convention.
"""

import datetime as dt
from typing import assert_type

import numpy as np

import umber as ub

numeric = ub.Figure().add_axes()
timed = ub.Figure().add_axes(x=ub.Scale.time())
cats = ub.Figure().add_axes(x=ub.Scale.category())
rows = ub.Figure().add_axes(y=ub.Scale.category())
t = [0.0, 1.0, 2.5]
names = ["a", "b"]
days = [dt.date(2026, 1, 1), dt.date(2026, 1, 3)]

# Positive: histogram bins as a count or as edges, with every style, statistic and flag.
assert_type(numeric.hist(t, bins=[0.0, 1.0, 3.0]), ub.Axes[float, float])
assert_type(numeric.hist(t, bins=np.array([0.0, 3.0])), ub.Axes[float, float])
numeric.hist(t, bins=4, style=ub.Hist.STEP, cumulative=True, stat=ub.Stat.FRACTION)
numeric.hist(t, style=ub.Hist.STACKED, stat=ub.Stat.DENSITY, label="Sizes")
assert_type(ub.hist(t, bins=[0.0, 3.0], style=ub.Hist.STEP), ub.Axes[float, float])

# Positive: an ECDF on a linear, log or time x axis keeps the axes type.
assert_type(numeric.ecdf(t), ub.Axes[float, float])
assert_type(timed.ecdf(days), ub.Axes[ub.Time, float])
numeric.ecdf(t, complementary=True, label="Load", color=ub.Color.BLUE)
numeric.ecdf(np.array(t), dash=ub.Dash.DASHED, width=ub.Size.pt(2)).line(t, t)

# Positive: a KDE takes AUTO or a bandwidth in data units.
assert_type(numeric.kde(t), ub.Axes[float, float])
numeric.kde(t, bandwidth=0.5, label="Density", color=ub.Color.RED)
numeric.kde(t, bandwidth=ub.AUTO, dash=ub.Dash.DOTTED, width=ub.Size.times(2))

# Negative: a category x axis takes none of them, and a category y axis neither.
cats.ecdf(names)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
cats.kde(t)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
rows.ecdf(t)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
rows.kde(t)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
timed.kde(t)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]

# Negative: the values follow the x axis; options are typed and keyword-only.
numeric.ecdf(days)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
timed.ecdf(t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.kde(names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hist(t, bins=names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hist(t, style="step")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hist(t, stat=ub.Hist.STEP)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hist(t, cumulative=1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.ecdf(t, complementary="yes")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.ecdf(t, True)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.kde(t, 0.5)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.kde(t, bandwidth="scott")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.kde(t, width=2.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
