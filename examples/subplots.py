"""A shared x scale, a spanning panel and one merged legend on a two-by-two grid."""

from pathlib import Path

import numpy as np

import umber as ub

x = np.linspace(0, 4 * np.pi, 200)
north = np.sin(x)
south = 0.6 * np.cos(x)

fig = ub.Figure(rows=2, cols=2, share_x=ub.Share.COL, legend=ub.Legend.MERGED)
fig.add_axes(row=0, col=0).line(x, north, label="North").title("North station").ylabel("Signal")
fig.add_axes(row=1, col=0).line(x, south, label="South").title("South station").ylabel("Signal")
(
    fig.add_axes(row=0, col=1, row_span=2)
    .line(x, north, label="North")
    .line(x, south, label="South")
    .title("Station comparison")
    .xlabel("Time (s)")
    .ylabel("Signal")
)
fig.save_png(Path(__file__).with_suffix(".png"))
