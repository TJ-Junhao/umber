"""A group marked in a confusion matrix with tick highlights, outlines and an emphasis color."""

from pathlib import Path

import numpy as np

import umber as ub

names = ["cat", "dog", "fox", "car", "bus", "van"]
rng = np.random.default_rng(4)
counts = np.diag(rng.integers(70, 95, 6)).astype(float)
counts[0, 1], counts[1, 0] = 9, 12
counts[1, 2], counts[2, 1] = 7, 6
counts[0, 2], counts[2, 0] = 3, 4
counts[3, 4], counts[4, 3] = 10, 8
counts[4, 5], counts[5, 4] = 6, 11
counts[3, 5], counts[5, 3] = 4, 5

animals = np.isin(names, ["cat", "dog", "fox"])
vehicles = np.isin(names, ["car", "bus", "van"])
within = (animals[:, None] & animals[None, :]) | (vehicles[:, None] & vehicles[None, :])
mask = within & ~np.eye(6, dtype=bool) & (counts > 0)
shown = np.where(counts > 0, counts, np.nan)

theme = ub.Theme.PIGMENT.with_emphasis_color(ub.Color.hex("#1A5FB4"))

fig = ub.Figure(12, 5, cols=2, theme=theme)
(
    fig.add_axes(col=0, x=ub.Scale.category(), y=ub.Scale.category())
    .heatmap(names, names, shown, annotate=True, outline=mask, format=ub.TickFormat.fixed(0))
    .xtick_highlight(["cat", "dog", "fox"])
    .ytick_highlight(["cat", "dog", "fox"])
    .title("Confusions within animals")
    .xlabel("Predicted")
    .ylabel("Actual")
)
(
    fig.add_axes(col=1, x=ub.Scale.category())
    .bar(names, [91.0, 88.5, 94.2, 79.8, 83.1, 86.4], annotate=True)
    .xtick_highlight(["car", "bus", "van"])
    .title("Recall by class")
    .ylabel("Recall (%)")
)
fig.save_png(Path(__file__).with_suffix(".png"))
