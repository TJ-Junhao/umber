"""One PDF document with three pages, and a PNG of the first page."""

from pathlib import Path

import numpy as np

import umber as ub

out = Path(__file__).parent
rng = np.random.default_rng(5)
weeks = np.arange(1, 27)

trend = ub.Figure(7, 4.5)
(
    trend.add_axes()
    .line(weeks, 120 + 4.5 * weeks + rng.normal(0, 9, weeks.size))
    .title("Weekly signups")
    .xlabel("Week")
    .ylabel("Signups")
)

regions = ["North", "South", "East", "West"]
revenue = ub.Figure(7, 4.5)
(
    revenue.add_axes(x=ub.Scale.category())
    .bar(regions, [48.2, 35.7, 52.9, 41.3], annotate=True)
    .title("Revenue by region")
    .ylabel("Revenue (k)")
)

latency = ub.Figure(7, 4.5)
(
    latency.add_axes()
    .hist(rng.lognormal(3.4, 0.45, 4000), bins=40)
    .title("Request latency")
    .xlabel("Latency (ms)")
    .ylabel("Requests")
)

ub.save_pdf(out / "multi_page_pdf.pdf", [trend, revenue, latency])
trend.save_png(out / "multi_page_pdf.png")
