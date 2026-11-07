"""Equal aspect keeps the axes type and applies to numeric axes only."""

from typing import assert_type

import umber as ub

ax = ub.Figure().add_axes()
assert_type(ax.equal_aspect(), ub.Axes[float, float])
assert_type(ax.equal_aspect().line([0.0, 1.0], [0.0, 1.0]), ub.Axes[float, float])

category = ub.Figure().add_axes(x=ub.Scale.category())
category.equal_aspect()  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
timed = ub.Figure().add_axes(y=ub.Scale.time())
timed.equal_aspect()  # type: ignore[misc]  # pyright: ignore[reportAttributeAccessIssue, reportUnknownMemberType]
ax.equal_aspect(True)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
