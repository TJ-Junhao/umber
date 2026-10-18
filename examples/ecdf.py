"""Empirical cumulative distributions of response times, and the share still waiting."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(5)
before = rng.lognormal(mean=3.0, sigma=0.5, size=800)
after = rng.lognormal(mean=2.7, sigma=0.4, size=800)

fig = ub.Figure(10, 4, cols=2)
(
    fig.add_axes(col=0)
    .ecdf(before, label="before")
    .ecdf(after, label="after")
    .title("Response times")
    .xlabel("milliseconds")
    .ylabel("share at or below")
)
(
    fig.add_axes(col=1, x=ub.Scale.log())
    .ecdf(after, complementary=True, label="after", dash=ub.Dash.DASHED)
    .title("Still waiting")
    .xlabel("milliseconds")
    .ylabel("share above")
)
fig.save_png(Path(__file__).with_suffix(".png"))
