"""Time series: a week of hourly demand with its range, a maintenance window and an event."""

import datetime as dt
import math
from pathlib import Path

import numpy as np

import umber as ub

hours = np.datetime64("2026-03-02T00", "h") + np.arange(7 * 24)
demand = [60 + 20 * math.sin((h - 6) / 24 * 2 * math.pi) + 4 * math.sin(h / 5) for h in range(168)]
low = [d - 6 for d in demand]
high = [d + 6 for d in demand]

fig = ub.Figure(8, 4)
(
    fig.add_axes(x=ub.Scale.time())
    .band(hours, low, high, label="Range")
    .line(hours, demand, label="Demand")
    .vspan(dt.datetime(2026, 3, 5, 1), dt.datetime(2026, 3, 5, 5), label="Maintenance")
    .vline(dt.datetime(2026, 3, 7, 18), dash=ub.Dash.DASHED, label="Outage")
    .title("Hourly demand")
    .subtitle("One week in March 2026")
    .ylabel("MW")
)
fig.save_png(Path(__file__).with_suffix(".png"))
