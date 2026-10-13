"""Reference lines and spans: a threshold, an event, a normal range and a window over a signal."""

from pathlib import Path

import numpy as np

import umber as ub

t = np.linspace(0, 60, 241)
signal = 50 + 12 * np.sin(t / 7) + np.random.default_rng(7).normal(0, 2.5, t.size)

fig = ub.Figure(8, 4.5)
(
    fig.add_axes()
    .line(t, signal, label="Signal")
    .hline(66, label="Threshold", dash=ub.Dash.DASHED)
    .vline(31, label="Event")
    .hspan(42, 58, label="Normal range")
    .vspan(40, 50, label="Window")
    .title("Reference lines and spans")
    .subtitle("Spans under the data, lines over it")
    .xlabel("minute")
    .ylabel("level")
)
fig.save_png(Path(__file__).with_suffix(".png"))
