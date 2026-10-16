"""Static typing expectations for colorbar settings."""

from typing import assert_type

import umber as ub

ax = ub.Figure().add_axes()
assert_type(ax.colorbar(), ub.Axes[float, float])
assert_type(ax.colorbar("Intensity", position=ub.ColorbarPosition.RIGHT), ub.Axes[float, float])
ax.colorbar(position=ub.AUTO, format=ub.AUTO)
ax.colorbar(position=ub.ColorbarPosition.BOTTOM, format=ub.TickFormat.percent())
assert_type(ub.Theme.PIGMENT.with_colorbar_position(ub.ColorbarPosition.NONE), ub.Theme)
ax.colorbar(position=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.colorbar(position="right")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.colorbar(format=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.colorbar("x", ub.ColorbarPosition.RIGHT)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
