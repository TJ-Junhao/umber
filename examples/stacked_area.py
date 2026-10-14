"""Renewable generation: three layers stacked from zero with flat fills and upper outlines."""

from pathlib import Path

import numpy as np

import umber as ub

x = np.arange(2010, 2026, dtype=float)
t = x - 2010
hydro = 12 + 0.6 * np.sin(x)
wind = 6 + 0.8 * t + 0.5 * np.sin(1.7 * t)
solar = 2 + 0.9 * t**1.3 / 4

fig = ub.Figure(8, 4.5)
(
    fig.add_axes()
    .stacked_area(
        x,
        [ub.Series("Hydro", hydro), ub.Series("Wind", wind), ub.Series("Solar", solar)],
    )
    .title("Renewable generation")
    .subtitle("TWh per year, stacked")
    .xlabel("year")
    .ylabel("TWh")
)
fig.save_png(Path(__file__).with_suffix(".png"))
