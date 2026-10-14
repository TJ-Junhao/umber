"""Bubbles: city population encoded by marker area."""

from pathlib import Path

import umber as ub

fig = ub.Figure(6, 4)
(
    fig.add_axes()
    .bubbles(
        [12, 18, 24, 35, 42, 55, 67, 78],
        [3.2, 4.8, 2.9, 6.4, 5.7, 7.1, 4.2, 8.3],
        [80, 140, 220, 500, 330, 870, 190, 650],
        label="Cities",
    )
    .title("Cities")
    .subtitle("Marker area shows population, in thousands")
    .xlabel("Income")
    .ylabel("Output")
)
fig.save_png(Path(__file__).with_suffix(".png"))
