"""Color-mapped points: readings colored by temperature, and anomalies around zero."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(41)
distance = rng.uniform(0, 10, 300)
depth = 2 + 0.6 * distance + rng.normal(0, 0.9, 300)
temperature = 18 + 4.5 * np.sin(distance / 2.2) + rng.normal(0, 0.8, 300)

fig = ub.Figure(10, 4, cols=2)
(
    fig.add_axes(col=0)
    .scatter(distance, depth, color=ub.ColorBy(temperature))
    .title("Readings by depth and distance")
    .subtitle("Colored by temperature, °C")
    .xlabel("Distance, km")
    .ylabel("Depth, m")
)
month = np.arange(48)
anomaly = 1.5 * np.sin(month / 5) + rng.normal(0, 0.4, 48)
(
    fig.add_axes(col=1)
    .clim(-2, 2)
    .scatter(month, anomaly, color=ub.ColorBy(anomaly, center=0))
    .title("Monthly anomaly")
    .subtitle("Diverging around zero, clamped to ±2")
    .xlabel("Month")
)
fig.save_png(Path(__file__).with_suffix(".png"))
