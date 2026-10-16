"""Rectangular and hexagonal density counts, alongside an edge-spanning numeric mesh."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(41)
x = rng.normal(size=12_000)
y = 0.6 * x + rng.normal(scale=0.7, size=x.size)
fig = ub.Figure(15, 4.5, cols=3)
(
    fig.add_axes(col=0)
    .hist2d(x, y, bins=35, scale=ub.Scale.log())
    .title("Rectangular counts")
    .xlabel("x")
    .ylabel("y")
)
(
    fig.add_axes(col=1)
    .hexbin(x, y, gridsize=35, scale=ub.Scale.log())
    .title("Hexagonal counts")
    .xlabel("x")
)
edges = np.linspace(-3, 3, 41)
centers = (edges[1:] + edges[:-1]) / 2
xx, yy = np.meshgrid(centers, centers)
values = np.sin(xx) * np.cos(yy)
(fig.add_axes(col=2).mesh(edges, edges, values, center=0).title("A sampled field").xlabel("x"))
fig.save_png(Path(__file__).with_suffix(".png"))
