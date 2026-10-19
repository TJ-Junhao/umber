"""Shares of orders, as a pie and a donut."""

from pathlib import Path

import umber as ub

fig = ub.Figure(cols=2).title("Orders by channel")
fig.add_pie_axes(col=0).pie(["Web", "App", "Shop", "Other"], [48, 32, 17, 3], annotate=True).title(
    "Share"
)
fig.add_pie_axes(col=1).pie(["Web", "App", "Shop", "Other"], [48, 32, 17, 3], hole=0.5).title(
    "Channels"
).legend(ub.LegendStyle.HEADER)
fig.save_png(Path(__file__).with_suffix(".png"))
