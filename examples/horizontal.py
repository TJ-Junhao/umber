"""Horizontal grouped bars, horizontal box plots and a dot plot on a category y axis."""

from pathlib import Path

import umber as ub

out = Path(__file__).parent
countries = ["United States", "Germany", "Brazil", "India", "Japan", "New Zealand"]

bars = ub.Figure(6.4, 4.4)
(
    bars.add_axes(y=ub.Scale.category())
    .grouped_bar(
        countries,
        [
            ub.Series("2015", [13, 30, 75, 16, 15, 80]),
            ub.Series("2023", [23, 54, 89, 21, 22, 82]),
        ],
    )
    .title("Renewable share of electricity")
    .xlabel("Share, %")
)
bars.save_png(out / "horizontal.png")

boxes = ub.Figure(6.4, 3.6)
(
    boxes.add_axes(y=ub.Scale.category())
    .box(
        countries[:3],
        [[24, 31, 35, 40, 52], [30, 38, 42, 47, 70], [20, 26, 29, 33, 41]],
        label="Commuters",
    )
    .title("Commute time")
    .xlabel("Minutes")
)
boxes.save_png(out / "horizontal_box.png")

dots = ub.Figure(6.4, 4.4)
(
    dots.add_axes(y=ub.Scale.category())
    .scatter([78.7, 81.0, 75.3, 68.3, 83.7, 81.5], countries, label="2015")
    .scatter([77.5, 80.8, 75.9, 70.8, 84.8, 82.5], countries, label="2023")
    .title("Life expectancy at birth")
    .xlabel("Years")
)
dots.save_png(out / "dot_plot.png")
