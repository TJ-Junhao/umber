"""Error bars: group means with intervals, and measurements with uncertainty in both directions."""

from pathlib import Path

import numpy as np

import umber as ub

groups = ["Control", "Low", "Medium", "High"]
means = np.array([4.2, 5.1, 6.8, 7.4])
spread = np.array([0.6, 0.8, 0.7, 1.1])
color = ub.Color.BLUE

fig = ub.Figure(7, 4)
(
    fig.add_axes(x=ub.Scale.category())
    .y_error_bars(groups, means - spread, means + spread, label="95% interval", color=color)
    .scatter(groups, means, label="Mean", color=color)
    .title("Response by dose")
    .subtitle("Means with 95% intervals")
    .xlabel("dose")
    .ylabel("response")
)
fig.save_png(Path(__file__).with_name("error_bars.png"))

x = np.array([1.0, 2.0, 3.0, 4.5, 6.0, 7.5])
y = 1.5 + 0.9 * x + np.array([0.12, -0.31, 0.24, -0.17, 0.33, -0.09])
y_error = 0.4 + 0.1 * x
x_error = np.array([0.2, 0.3, 0.25, 0.4, 0.35, 0.5])

fig = ub.Figure(7, 4)
(
    fig.add_axes()
    .y_error_bars(x, y - y_error, y + y_error, label="y uncertainty", color=color)
    .x_error_bars(y, x - x_error, x + x_error, label="x uncertainty", color=ub.Color.ORANGE)
    .scatter(x, y, label="Measured", color=color)
    .title("Measurements")
    .subtitle("Uncertainty in both directions")
    .xlabel("x")
    .ylabel("y")
)
fig.save_png(Path(__file__).with_name("error_bars_measurements.png"))
