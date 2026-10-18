"""Static typing of grouping columns and the unchanged axes restrictions."""

import datetime as dt
from typing import assert_type

import numpy as np
import pandas as pd
import polars as pl

import umber as ub

numeric = ub.Figure().add_axes()
categories = ub.Figure().add_axes(x=ub.Scale.category())
rows = ub.Figure().add_axes(y=ub.Scale.category())
timed = ub.Figure().add_axes(x=ub.Scale.time())
values = [1.0, 2.0, 3.0, 4.0]
groups = ["B", "A", "B", "A"]
days = [dt.date(2026, 1, i) for i in (1, 1, 2, 2)]

assert_type(numeric.line(values, values, by=groups), ub.Axes[float, float])
assert_type(numeric.scatter(values, values, by=groups), ub.Axes[float, float])
assert_type(numeric.step(values, values, by=groups), ub.Axes[float, float])
assert_type(numeric.area(values, values, by=groups), ub.Axes[float, float])
assert_type(numeric.ecdf(values, by=groups), ub.Axes[float, float])
assert_type(numeric.kde(values, by=groups), ub.Axes[float, float])
assert_type(numeric.hist(values, by=groups, style=ub.Hist.STEP), ub.Axes[float, float])
assert_type(categories.bar(groups, values, by=["X", "X", "Y", "Y"]), ub.Axes[str, float])
assert_type(rows.bar(groups, values, by=["X", "X", "Y", "Y"]), ub.Axes[float, str])
assert_type(timed.line(days, values, by=groups), ub.Axes[ub.Time, float])
assert_type(timed.ecdf(days, by=groups), ub.Axes[ub.Time, float])
numeric.line(values, values, by=ub.AUTO)
numeric.line(values, values, by=np.array(groups))
numeric.line(values, values, by=pd.Series(groups))
numeric.line(values, values, by=pl.Series(groups))
assert_type(ub.line(values, values, by=groups), ub.Axes[float, float])
assert_type(ub.scatter(values, values, by=groups), ub.Axes[float, float])
assert_type(ub.hist(values, by=groups, style=ub.Hist.STACKED), ub.Axes[float, float])
assert_type(ub.bar(groups, values, by=["X", "X", "Y", "Y"]), ub.Axes[str, float])

numeric.line(values, values, by=[1, 2, 1, 2])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
numeric.scatter(values, values, by=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.step(values, values, by=[True])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
numeric.area(values, values, by=[1.0])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
numeric.ecdf(values, by=[1])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
numeric.kde(values, by=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.hist(values, by=[1])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
categories.bar(groups, values, by=[1])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
timed.kde(values, by=groups)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
rows.line(values, groups, by=groups)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
numeric.line(values, values, "", ub.Color.AUTO, ub.Dash.SOLID, ub.Size.AUTO, groups)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
