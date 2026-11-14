"""Notes placed by position in the plot, which stay put when the data range changes."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(21)
hours = np.arange(0, 24)
load = 40 + 28 * np.exp(-((hours - 14) ** 2) / 18) + 12 * np.exp(-((hours - 8) ** 2) / 8)
load = load + rng.normal(0, 1.5, hours.size)

fig = ub.Figure(11, 4.6, cols=2)
(
    fig.add_axes(col=0)
    .line(hours, load)
    .title("Server load")
    .xlabel("Hour of day")
    .ylabel("CPU (%)")
    .text("Weekday average\nn = 120 days", ub.InPlot(0.03, 0.95), size=ub.Size.times(0.9))
    .annotate(
        "Afternoon peak", (14, float(load[14])), text_at=ub.InPlot(0.97, 0.55), align=ub.Align.RIGHT
    )
)
height = rng.normal(172, 8, 400)
weight = 0.9 * (height - 100) + rng.normal(0, 6, 400)
# The point farthest above the least-squares line.
residual = weight - np.polyval(np.polyfit(height, weight, 1), height)
outlier = int(np.argmax(residual))
(
    fig.add_axes(col=1)
    .scatter(height, weight, size=ub.Size.pt(5))
    .title("Height and weight")
    .xlabel("Height (cm)")
    .ylabel("Weight (kg)")
    .text("n = 400", ub.InPlot(0.97, 0.05), align=ub.Align.RIGHT, color=ub.Color.GRAY)
    .annotate(
        "Heaviest for its height",
        (float(height[outlier]), float(weight[outlier])),
        text_at=ub.InPlot(0.2, 0.9),
    )
)
fig.save_png(Path(__file__).with_suffix(".png"))
