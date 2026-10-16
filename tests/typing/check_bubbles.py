"""Static typing expectations for bubble coordinates, sizes and options."""

import datetime as dt

import numpy as np

import umber as ub

numeric = ub.Figure().add_axes()
logged = ub.Figure().add_axes(x=ub.Scale.log(), y=ub.Scale.log())
cats = ub.Figure().add_axes(x=ub.Scale.category())
t = [1.0, 2.0]
names = ["a", "b"]
numeric.bubbles(t, t, t, label="Cities", color=ub.Color.BLUE, marker=ub.Marker.DIAMOND)
logged.bubbles(t, t, t)
numeric.bubbles(np.arange(2.0), np.zeros(2), np.ones(2))
chained: ub.Axes[float, float] = numeric.bubbles(t, t, t).line(t, t)
days = [dt.date(2026, 1, 1), dt.date(2026, 1, 2)]
timed = ub.Figure().add_axes(x=ub.Scale.time())
timed_chain: ub.Axes[ub.Time, float] = timed.bubbles(days, t, t).line(days, t)
upright = ub.Figure().add_axes(y=ub.Scale.time())
upright_chain: ub.Axes[float, ub.Time] = upright.bubbles(t, days, t)
timed.bubbles(t, t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
upright.bubbles(days, days, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
cats.bubbles(t, t, t)  # type: ignore[misc, arg-type]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
numeric.bubbles(names, t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, names, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, t, names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, t)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.bubbles(t, t, t, "Cities")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.bubbles(t, t, t, label=1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, t, t, color="red")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, t, t, marker="circle")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, t, t, size=ub.Size.AUTO)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
