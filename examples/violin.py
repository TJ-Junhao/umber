"""Compare sample distributions with plain and grouped violin plots."""

from pathlib import Path

import umber as ub

categories = ["North", "East", "South", "West"]
first = [[1, 2, 3, 4, 5, 12], [2, 4, 5, 6, 8], [-4, 1, 2, 3, 4, 5], [3, 4, 6, 8, 9]]
second = [[value + 2 for value in group] for group in first]

fig = ub.Figure(6, 4)
fig.add_axes(x=ub.Scale.category()).title("Samples").violin(categories, first)
fig.save_png(Path(__file__).with_suffix(".png"))

grouped = ub.Figure(6, 4)
grouped.add_axes(x=ub.Scale.category()).title("Grouped samples").grouped_violin(
    categories,
    [ub.Distribution("2025", first), ub.Distribution("2026", second)],
)
grouped.save_png(Path(__file__).with_name("grouped_violin.png"))
