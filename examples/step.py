"""Step lines: a price that holds between changes, drawn in each step position."""

from pathlib import Path

import umber as ub

changes = [0, 3, 7, 12, 18, 21, 28]
price = [4.2, 4.6, 4.1, 5.0, 4.4, 4.4, 5.3]

fig = ub.Figure(8, 4)
(
    fig.add_axes()
    .step(changes, price, where=ub.Step.POST, label="Post")
    .step(changes, [p - 1.0 for p in price], where=ub.Step.PRE, label="Pre")
    .step(changes, [p - 2.0 for p in price], where=ub.Step.MID, dash=ub.Dash.DASHED, label="Mid")
    .title("Price changes")
    .xlabel("day")
    .ylabel("price")
)
fig.save_png(Path(__file__).with_suffix(".png"))
