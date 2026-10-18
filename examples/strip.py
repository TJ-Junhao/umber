"""Draw reproducible sample jitter above box plots."""

from pathlib import Path

import umber as ub

categories = ["North", "South"]
groups = [[1, 2, 2, 3, 4, 5], [2, 3, 3, 4, 5, 7]]
fig = ub.Figure(6, 4)
ax = fig.add_axes(x=ub.Scale.category()).title("Individual samples")
ax.box(categories, groups).strip(categories, groups, label="Samples", size=ub.Size.pt(5))
fig.save_png(Path(__file__).with_suffix(".png"))
