"""Stems: a damped impulse response with positive and negative samples."""

from math import cos, exp
from pathlib import Path

import umber as ub

samples = list(range(24))
response = [10 * exp(-n / 7) * cos(0.9 * n) for n in samples]

fig = ub.Figure(6, 4)
(
    fig.add_axes()
    .stem(samples, response, label="Response")
    .title("Stems")
    .subtitle("Damped impulse response")
    .xlabel("sample")
)
fig.save_png(Path(__file__).with_suffix(".png"))
