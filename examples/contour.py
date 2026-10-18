"""Contours: a filled elevation map with labelled lines, and the lines of a field in one color."""

from pathlib import Path

import numpy as np

import umber as ub

x = np.linspace(-3.0, 3.0, 121)
y = np.linspace(-2.0, 2.0, 81)
xx, yy = np.meshgrid(x, y)
elevation = (
    900 * np.exp(-((xx + 1.0) ** 2 + (yy - 0.4) ** 2) / 0.9)
    + 650 * np.exp(-((xx - 1.3) ** 2 + (yy + 0.5) ** 2) / 0.6)
    + 120 * xx
    + 400
)
wave = np.sin(xx * 1.6) * np.cos(yy * 2.1)

fig = ub.Figure(12, 4.4, cols=2)
(
    fig.add_axes(col=0)
    .contour(x, y, elevation, style=ub.Contour.BOTH, annotate=True)
    .colorbar("m")
    .title("Elevation")
    .xlabel("East (km)")
    .ylabel("North (km)")
)
(
    fig.add_axes(col=1)
    .contour(x, y, wave, levels=[-0.6, -0.2, 0.2, 0.6], color=ub.Color.BLUE, annotate=True)
    .title("A wave field")
    .xlabel("x")
)
fig.save_png(Path(__file__).with_suffix(".png"))
