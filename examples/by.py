"""Group long-form columns into lines, bars and distributions with ``by``."""

from pathlib import Path

import umber as ub

quarter = [1, 1, 2, 2, 3, 3, 4, 4]
region = ["North", "South", "North", "South", "North", "South", "North", "South"]
revenue = [12, 9, 15, 13, 14, 16, 19, 17]

fig = ub.Figure(10, 7, rows=2, cols=2)
(
    fig.add_axes(row=0, col=0)
    .line(quarter, revenue, by=region)
    .title("Quarterly revenue")
    .xlabel("quarter")
    .ylabel("million dollars")
)
(
    fig.add_axes(row=0, col=1, x=ub.Scale.category())
    .bar([f"Q{q}" for q in quarter], revenue, by=region)
    .title("Regions side by side")
    .ylabel("million dollars")
)
(
    fig.add_axes(row=1, col=0)
    .ecdf(revenue, by=region)
    .title("Revenue distributions")
    .xlabel("million dollars")
    .ylabel("cumulative share")
)
(
    fig.add_axes(row=1, col=1)
    .hist(revenue, bins=[8, 12, 16, 20], by=region, style=ub.Hist.STACKED)
    .title("Quarters by revenue")
    .xlabel("million dollars")
    .ylabel("quarters")
)
fig.save_png(Path(__file__).with_suffix(".png"))
