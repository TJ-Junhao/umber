"""Custom tick labels on x and a percent format on y."""

from pathlib import Path

import numpy as np

import umber as ub

hours = np.arange(0, 25)
load = 0.5 + 0.4 * np.sin((hours - 6) * np.pi / 12)

fig = ub.Figure()
(
    fig.add_axes()
    .line(hours, load, label="load")
    .xticks_labeled([0, 6, 12, 18, 24], ["midnight", "6 am", "noon", "6 pm", "midnight"])
    .ytick_format(ub.TickFormat.percent())
    .title("Server load over a day")
    .ylabel("utilization")
)
fig.save_png(Path(__file__).with_suffix(".png"))
