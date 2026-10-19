"""Annotations with arrows on a time series, text at a point, and values written on bars."""

from datetime import date, timedelta
from pathlib import Path

import umber as ub

out = Path(__file__).parent
days = [date(2026, 3, 1) + timedelta(days=i) for i in range(12)]
requests = [21, 22, 24, 32, 25, 23, 15, 20, 23, 24, 27, 25]

series = ub.Figure()
(
    series.add_axes(x=ub.Scale.time())
    .line(days, requests)
    .ylim(5, 40)
    .title("Peak and dip")
    .ylabel("Requests / s")
    .annotate("Peak", (days[3], 32.0), offset=(24, 18))
    .annotate("Dip", (days[6], 15.0), offset=(18, -22), arrow=False)
    .text("Target 30", (days[0], 30.0), size=ub.Size.times(0.9), color=ub.Color.GRAY)
)
series.save_png(out / "annotations.png")

totals = ub.Figure()
(
    totals.add_axes(x=ub.Scale.category())
    .bar(["A", "B", "C", "D", "E", "F"], [28.4, 42.0, -16.5, 36.2, 52.8, 31.5], annotate=True)
    .title("Net change")
    .ylabel("Orders")
)
totals.save_png(out / "annotations_bars.png")

parts = ub.Figure()
(
    parts.add_axes(x=ub.Scale.category())
    .stacked_bar(
        ["Q1", "Q2", "Q3", "Q4"],
        [
            ub.Series("Core", [32, 38, 29, 41]),
            ub.Series("Service", [21, 24, 26, 20]),
            ub.Series("Other", [18, 22, 19, 25]),
        ],
        annotate=True,
    )
    .title("Components of each total")
    .ylabel("Orders")
)
parts.save_png(out / "annotations_stacked.png")
