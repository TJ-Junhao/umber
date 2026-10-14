"""Static typing expectations for bubble coordinates, sizes and options."""

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
cats.bubbles(t, t, t)  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
numeric.bubbles(names, t, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, names, t)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, t, names)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, t)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.bubbles(t, t, t, "Cities")  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
numeric.bubbles(t, t, t, label=1)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, t, t, color="red")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, t, t, marker="circle")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
numeric.bubbles(t, t, t, size=ub.Size.AUTO)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
