"""Bands: a mean over its 95% interval, which widens with x."""

from pathlib import Path

import numpy as np

import umber as ub

x = np.linspace(0, 12, 97)
mean = 10 + 3 * np.sin(x / 2) + 0.4 * x
half = 1.2 + 0.8 * x / 12 + 0.3 * np.abs(np.sin(x))
color = ub.Color.BLUE

fig = ub.Figure(8, 4.5)
(
    fig.add_axes()
    .band(x, mean - half, mean + half, label="95% interval", color=color)
    .line(x, mean, label="Mean", color=color)
    .title("Mean with a 95% band")
    .subtitle("Monthly estimate, two-sided interval")
    .xlabel("month")
    .ylabel("estimate")
)
fig.save_png(Path(__file__).with_suffix(".png"))
