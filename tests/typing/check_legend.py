"""Static typing expectations for titles, title alignment and legend positions.

Both checkers must report each error marked below; see ``check_errors.py``.
"""

from typing import assert_type

import umber as ub

fig = ub.Figure()
assert_type(fig.title("Stations"), ub.Figure)
assert_type(fig.subtitle("Daily measurements"), ub.Figure)
assert_type(ub.Theme.PIGMENT.with_title_align(ub.Align.CENTER), ub.Theme)
assert_type(ub.Theme.PIGMENT.with_legend_position(ub.LegendPosition.RIGHT), ub.Theme)

ax = fig.add_axes()
assert_type(
    ax.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT), ub.Axes[float, float]
)
assert_type(ax.legend(ub.LegendStyle.CHIPS, position=ub.LegendPosition.BEST), ub.Axes[float, float])
assert_type(ax.legend(ub.LegendStyle.HEADER, position=ub.AUTO), ub.Axes[float, float])
assert_type(ax.legend(ub.LegendStyle.DIRECT), ub.Axes[float, float])

cats = ub.Figure().add_axes(x=ub.Scale.category())
assert_type(
    cats.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.BOTTOM), ub.Axes[str, float]
)
assert_type(cats.legend(ub.LegendStyle.HEADER, position=ub.AUTO), ub.Axes[str, float])

ax.legend(ub.LegendStyle.HEADER, position="right")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.legend(ub.LegendStyle.HEADER, position=None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.legend(ub.LegendStyle.HEADER, position=ub.Align.RIGHT)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ax.legend(ub.LegendStyle.HEADER, ub.LegendPosition.RIGHT)  # type: ignore[call-arg]  # pyright: ignore[reportCallIssue]
ub.Theme.PIGMENT.with_title_align("center")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Theme.PIGMENT.with_legend_position(ub.AUTO)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
ub.Theme.PIGMENT.with_legend_position("right")  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.title(42)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
fig.subtitle(None)  # type: ignore[arg-type]  # pyright: ignore[reportArgumentType]
