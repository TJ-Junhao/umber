"""Wind as arrows colored by speed on a grid, and as barbs on a time-height section."""

from datetime import datetime
from pathlib import Path

import numpy as np

import umber as ub

x = np.linspace(-4.0, 4.0, 17)
y = np.linspace(-3.0, 3.0, 13)
xx, yy = np.meshgrid(x, y)
u = 8 * np.sin(yy * 0.8) + 3
v = 6 * np.cos(xx * 0.7)
speed = np.hypot(u, v)

times = [datetime(2026, 4, 12, hour) for hour in range(0, 24, 3)]
heights = np.array([500.0, 1500.0, 3000.0, 5500.0, 9000.0])
hh, tt = np.meshgrid(heights, np.arange(len(times)), indexing="ij")
bar_u = 8 + 0.006 * hh + 4 * np.sin(tt * 0.7)
bar_v = -5 + 0.002 * hh + 6 * np.cos(tt * 0.5)

fig = ub.Figure(12, 4.8, cols=2)
(
    fig.add_axes(col=0)
    .quiver(x, y, u, v, color=ub.ColorBy(speed), key=(10, "10 m/s"))
    .colorbar("Speed (m/s)")
    .title("Surface wind")
    .xlabel("East (km)")
    .ylabel("North (km)")
)
(
    fig.add_axes(col=1, x=ub.Scale.time())
    .barbs(times, heights, bar_u, bar_v, size=ub.Size.pt(26))
    .title("Wind aloft")
    .xlabel("Time (UTC)")
    .ylabel("Height (m)")
)
fig.save_png(Path(__file__).with_suffix(".png"))
