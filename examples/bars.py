"""A bar chart, a grouped bar chart and a stacked bar chart."""

from pathlib import Path

import umber as ub

out = Path(__file__).parent
quarters = ["Q1", "Q2", "Q3", "Q4"]
north = ub.BarSeries("North", [12, 18, 15, 22])
south = ub.BarSeries("South", [9, 11, 17, 14])

simple = ub.Figure()
(
    simple.add_axes(x=ub.Scale.category())
    .bar(quarters, [21, 29, 32, 36], label="Total")
    .title("Sales by quarter")
    .ylabel("units")
)
simple.save_png(out / "bars.png")

grouped = ub.Figure()
(
    grouped.add_axes(x=ub.Scale.category())
    .grouped_bar(quarters, [north, south])
    .title("Sales by region")
    .ylabel("units")
)
grouped.save_png(out / "bars_grouped.png")

stacked = ub.Figure()
(
    stacked.add_axes(x=ub.Scale.category())
    .stacked_bar(quarters, [north, south])
    .title("Sales, stacked")
    .ylabel("units")
)
stacked.save_png(out / "bars_stacked.png")
