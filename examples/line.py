"""Two lines with labels, a title, a subtitle and axis labels."""

from pathlib import Path

import numpy as np

import umber as ub

x = np.linspace(0, 4 * np.pi, 200)

fig = ub.Figure()
(
    fig.add_axes()
    .line(x, np.sin(x), label="sin(x)")
    .line(x, 0.6 * np.cos(2 * x), label="0.6 cos(2x)")
    .title("Two waves")
    .subtitle("A sine and a faster, smaller cosine")
    .xlabel("x (radians)")
    .ylabel("amplitude")
)
fig.save_png(Path(__file__).with_suffix(".png"))
