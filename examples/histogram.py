"""A histogram of normally distributed samples."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(11)
samples = rng.normal(loc=100.0, scale=15.0, size=2000)

fig = ub.Figure()
(
    fig.add_axes()
    .hist(samples, bins=24, label="samples")
    .title("Response times")
    .xlabel("milliseconds")
    .ylabel("count")
)
fig.save_png(Path(__file__).with_suffix(".png"))
