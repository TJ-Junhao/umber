"""Histogram variants: uneven bin edges with densities, and cumulative fractions."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(23)
waits = rng.gamma(shape=2.0, scale=6.0, size=1500)

fig = ub.Figure(10, 4, cols=2)
(
    fig.add_axes(col=0)
    .hist(waits, bins=[0, 5, 10, 15, 20, 30, 45, 90], stat=ub.Stat.DENSITY, label="density")
    .title("Uneven bins")
    .xlabel("minutes")
    .ylabel("density")
)
(
    fig.add_axes(col=1)
    .hist(waits, bins=30, cumulative=True, stat=ub.Stat.FRACTION, label="bars")
    .hist(
        waits,
        bins=30,
        style=ub.Hist.STEP,
        cumulative=True,
        stat=ub.Stat.FRACTION,
        label="outline",
    )
    .title("Cumulative share")
    .xlabel("minutes")
    .ylabel("fraction")
)
fig.save_png(Path(__file__).with_suffix(".png"))
