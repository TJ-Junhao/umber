"""Show event times in category rows."""

from datetime import date
from pathlib import Path

import umber as ub

fig = ub.Figure(6, 4)
fig.add_axes(x=ub.Scale.time(), y=ub.Scale.category()).title("Arrivals").events(
    ["North", "South"],
    [[date(2026, 1, 1), date(2026, 1, 3)], [date(2026, 1, 2), date(2026, 1, 4)]],
    width=ub.Size.pt(2),
)
fig.save_png(Path(__file__).with_suffix(".png"))
