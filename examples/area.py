"""An area chart drawn with the gradient fill style."""

from pathlib import Path

import numpy as np

import umber as ub

rng = np.random.default_rng(3)
days = np.arange(60)
visits = 200 + np.cumsum(rng.normal(2.0, 8.0, size=days.size))

theme = ub.Theme.PIGMENT.with_fill_style(ub.FillStyle.GRADIENT)
fig = ub.Figure(theme=theme)
(
    fig.add_axes()
    .area(days, visits, label="visits")
    .title("Daily visits")
    .xlabel("day")
    .ylabel("visits")
)
fig.save_png(Path(__file__).with_suffix(".png"))
