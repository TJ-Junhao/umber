"""A twin preserves its x value type and has numeric y values."""

from datetime import date
from typing import assert_type

import umber as ub

numeric = ub.Figure().add_axes()
category = ub.Figure().add_axes(x=ub.Scale.category())
timed = ub.Figure().add_axes(x=ub.Scale.time())
assert_type(numeric.twin_y(), ub.Axes[float, float])
assert_type(category.twin_y(), ub.Axes[str, float])
assert_type(timed.twin_y(scale=ub.Scale.log()), ub.Axes[ub.Time, float])
category.twin_y().bar(["A"], [1.0], annotate=True).text("A", ("A", 1.0))
timed.twin_y().line([date(2026, 1, 1)], [1.0])
numeric.twin_y(scale=ub.Scale.category())  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.twin_y(scale=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
category.twin_y().line([1.0], [1.0])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
non_numeric = ub.Figure().add_axes(y=ub.Scale.category())
non_numeric.twin_y()  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
