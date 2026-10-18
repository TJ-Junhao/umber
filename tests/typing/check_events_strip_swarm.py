"""Events follow x types; strip and swarm require one numeric and one category axis."""

from datetime import date

import umber as ub

vertical = ub.Figure().add_axes(x=ub.Scale.category())
horizontal = ub.Figure().add_axes(y=ub.Scale.category())
time_rows = ub.Figure().add_axes(x=ub.Scale.time(), y=ub.Scale.category())
numeric = ub.Figure().add_axes()

v: ub.Axes[str, float] = vertical.strip(["A"], [[1.0]], jitter=0.0).swarm(["B"], [[2.0]])
h: ub.Axes[float, str] = horizontal.strip(["A"], [[1.0]]).swarm(["B"], [[2.0]])
horizontal.events(["A"], [[1.0]], length=0.5, width=ub.Size.pt(2), color=ub.Color.RED)
t: ub.Axes[ub.Time, str] = time_rows.events(["A"], [[date(2026, 1, 1)]])

numeric.strip(["A"], [[1.0]])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
numeric.swarm(["A"], [[1.0]])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
numeric.events(["A"], [[1.0]])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
vertical.events(["A"], [[1.0]])  # type: ignore[misc, list-item]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
time_rows.swarm(["A"], [[1.0]])  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
time_rows.events(["A"], [[1.0]])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
horizontal.events(["A"], [[date(2026, 1, 1)]])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
vertical.strip(["A"], [[1.0]], jitter=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
vertical.swarm(["A"], [[1.0]], jitter=0.1)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
vertical.strip(["A"], [[1.0]], size=3.0)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
vertical.swarm(["A"], [["bad"]])  # type: ignore[list-item]  # pyright: ignore[reportArgumentType]
