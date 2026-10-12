"""The same chart in each preset theme."""

from pathlib import Path

import numpy as np

import umber as ub

x = np.linspace(0, 10, 100)
out = Path(__file__).parent

themes = {
    "pigment": ub.Theme.PIGMENT,
    "graphite": ub.Theme.GRAPHITE,
    "midnight": ub.Theme.MIDNIGHT,
}
for name, theme in themes.items():
    fig = ub.Figure(theme=theme)
    (
        fig.add_axes()
        .line(x, np.sin(x), label="sin(x)")
        .line(x, np.sin(x / 2), label="sin(x/2)")
        .title(f"Theme {name}")
        .xlabel("x")
        .ylabel("y")
    )
    fig.save_png(out / f"themes_{name}.png")
