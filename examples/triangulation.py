"""Scattered measurements as a triangle mesh, colored triangles and contours."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(9)
x = rng.uniform(0, 10, 220)
y = rng.uniform(0, 6, 220)
depth = 40 + 9 * np.sin(x * 0.6) + 6 * np.cos(y * 1.1) + 0.8 * x

fig = ub.Figure(15, 4.6, cols=3)
(
    fig.add_axes(col=0)
    .triplot(x, y)
    .scatter(x, y, size=ub.Size.pt(3))
    .title("Mesh of stations")
    .xlabel("East (km)")
    .ylabel("North (km)")
)
(
    fig.add_axes(col=1)
    .tripcolor(x, y, depth)
    .colorbar("Depth (m)")
    .title("Seabed depth")
    .xlabel("East (km)")
)
(
    fig.add_axes(col=2)
    .tricontour(x, y, depth, style=ub.Contour.BOTH, annotate=True)
    .colorbar("Depth (m)")
    .title("Depth contours")
    .xlabel("East (km)")
)
fig.save_png(Path(__file__).with_suffix(".png"))
