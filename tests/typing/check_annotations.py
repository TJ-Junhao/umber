"""Static typing expectations for text, annotations and bar value labels."""

from datetime import date
from typing import assert_type

import umber as ub

numeric = ub.Figure().add_axes()
columns = ub.Figure().add_axes(x=ub.Scale.category())
rows = ub.Figure().add_axes(y=ub.Scale.category())
timed = ub.Figure().add_axes(x=ub.Scale.time())

# Positive: the point follows the axes' value types, and the axes keep their type.
assert_type(numeric.text("Peak", (1.0, 2.0)), ub.Axes[float, float])
assert_type(columns.text("Q2", ("Q2", 5.0), align=ub.Align.CENTER), ub.Axes[str, float])
assert_type(rows.text("North", (3.0, "North"), align=ub.Align.RIGHT), ub.Axes[float, str])
assert_type(timed.text("Launch", (date(2026, 3, 1), 4.0)), ub.Axes[ub.Time, float])
numeric.text("x", (1, 2), size=ub.Size.pt(12), color=ub.Color.RED)
assert_type(numeric.annotate("Peak", (1.0, 2.0)), ub.Axes[float, float])
numeric.annotate("Dip", (1.0, 2.0), offset=(18.0, -22.0), arrow=False, size=ub.Size.times(1.2))
assert_type(timed.annotate("Launch", (date(2026, 3, 1), 4.0)), ub.Axes[ub.Time, float])
assert_type(columns.bar(["A"], [1.0], annotate=True), ub.Axes[str, float])
columns.bar(["A"], [1.0], annotate=True, format=ub.TickFormat.fixed(1), size=ub.Size.pt(9))
columns.bar(["A"], [1.0], annotate=True, format=ub.AUTO)
series = [ub.Series("North", [1.0])]
assert_type(rows.grouped_bar(["A"], series, annotate=True), ub.Axes[float, str])
assert_type(columns.stacked_bar(["A"], series, annotate=True), ub.Axes[str, float])

# Negative: points of the wrong types, and option values of the wrong types.
columns.text("x", (1.0, 2.0))  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.text("x", ("a", 2.0))  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
timed.text("x", (1.0, 2.0))  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.text("x", [1.0, 2.0])  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.text("x", (1.0, 2.0), align="left")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.text("x", (1.0, 2.0), size=12)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.annotate("x", (1.0, 2.0), offset=[1.0, 2.0])  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.annotate("x", (1.0, 2.0), arrow=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
columns.bar(["A"], [1.0], format="%.1f")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
columns.bar(["A"], [1.0], size=9)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
columns.stacked_bar(["A"], series, annotate=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
