"""Static typing expectations for stacked areas, checked by pyright and mypy."""

import numpy as np

import umber as ub

numeric = ub.Figure().add_axes()
logged = ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.log())
cats = ub.Figure().add_axes(x=ub.Scale.category())
x = [1.0, 2.0]
series = [ub.Series("a", [1.0, 2.0])]

numeric.stacked_area(x, series)
logged.stacked_area(x, tuple(series))
numeric.stacked_area(np.arange(2.0), series)
chained: ub.Axes[float, float] = numeric.stacked_area(x, series).line(x, x)
log_result: ub.Axes[float, float] = logged.stacked_area(x, series)

cats.stacked_area(["a", "b"], series)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
numeric.stacked_area(["a", "b"], series)  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
numeric.stacked_area(x, [[1, 2]])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
numeric.stacked_area(x)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.stacked_area(x, series, label="a")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.stacked_area(x, series, color=ub.Color.BLUE)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
