"""Kernel density lines over a density histogram: Scott's bandwidth and a narrower one."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(17)
heights = np.concatenate([rng.normal(162, 6, 600), rng.normal(176, 7, 400)])

fig = ub.Figure()
(
    fig.add_axes()
    .hist(heights, bins=30, stat=ub.Stat.DENSITY, label="histogram")
    .kde(heights, label="Scott")
    .kde(heights, bandwidth=1.5, label="narrow", dash=ub.Dash.DASHED)
    .title("Heights")
    .xlabel("centimetres")
    .ylabel("density")
)
fig.save_png(Path(__file__).with_suffix(".png"))
