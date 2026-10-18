"""Draw non-overlapping samples above horizontal violins."""

from pathlib import Path

import umber as ub

categories = ["North", "South"]
groups = [[1, 2, 2, 3, 4, 5], [2, 3, 3, 4, 5, 7]]
fig = ub.Figure(6, 4)
ax = fig.add_axes(y=ub.Scale.category()).title("Individual samples")
ax.violin(categories, groups).swarm(categories, groups, label="Samples", size=ub.Size.pt(5))
fig.save_png(Path(__file__).with_suffix(".png"))
