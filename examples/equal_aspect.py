"""Equal scales on both axes: a circle stays round, and a survey map keeps its shape."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(14)
angle = np.linspace(0, 2 * np.pi, 200)

east = rng.uniform(0, 8, 160)
north = rng.uniform(0, 3, 160)
elevation = 120 + 18 * east + 9 * np.sin(north * 2.0) + rng.normal(0, 4, 160)

fig = ub.Figure(12, 4.6, cols=2)
(
    fig.add_axes(col=0)
    .equal_aspect()
    .line(np.cos(angle), np.sin(angle), label="Unit circle")
    .line(1.5 * np.cos(angle), np.sin(angle), label="Ellipse")
    .line([-1.5, 1.5], [0, 0], color=ub.Color.GRAY)
    .title("Circles stay round")
    .xlabel("x")
    .ylabel("y")
)
(
    fig.add_axes(col=1)
    .equal_aspect()
    .scatter(east, north, color=ub.ColorBy(elevation), size=ub.Size.pt(7))
    .colorbar("m")
    .title("Survey points")
    .xlabel("East (km)")
    .ylabel("North (km)")
)
fig.save_png(Path(__file__).with_suffix(".png"))
