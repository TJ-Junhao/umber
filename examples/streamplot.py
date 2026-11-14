"""Streamlines of a flow past a source and a sink, colored by speed, and of a saddle flow."""

from pathlib import Path

import numpy as np

import umber as ub

x = np.linspace(-3.0, 3.0, 121)
y = np.linspace(-2.0, 2.0, 81)
xx, yy = np.meshgrid(x, y)


def source(x0: float, y0: float, strength: float) -> tuple[np.ndarray, np.ndarray]:
    """Velocity of a point source, or of a sink with a negative strength."""
    dx, dy = xx - x0, yy - y0
    r2 = dx**2 + dy**2 + 0.02
    return strength * dx / r2, strength * dy / r2


su, sv = source(-1.2, 0.0, 1.0)
ku, kv = source(1.2, 0.0, -1.0)
u = 1.0 + su + ku
v = sv + kv
speed = np.hypot(u, v)

fig = ub.Figure(12, 4.8, cols=2)
(
    fig.add_axes(col=0)
    .streamplot(x, y, u, v, density=1.6, color=ub.ColorBy(np.minimum(speed, 4.0)))
    .colorbar("Speed")
    .title("Source and sink in a stream")
    .xlabel("x")
    .ylabel("y")
)
fig.add_axes(col=1).streamplot(x, y, xx, -yy, density=1.2).title("A saddle").xlabel("x")
fig.save_png(Path(__file__).with_suffix(".png"))
