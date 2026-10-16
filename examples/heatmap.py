"""Heatmaps: an annotated correlation matrix centered at zero, and visits by day and hour."""

from pathlib import Path

import numpy as np

import umber as ub

out = Path(__file__).parent
rng = np.random.default_rng(21)

metrics = ["Revenue", "Users", "Orders", "Churn", "Latency", "Support", "Margin"]
base = rng.normal(size=(2, 400))
mix = rng.normal(size=(len(metrics), 2))
noise = rng.normal(size=(len(metrics), 400))
correlations = np.corrcoef(mix @ base + 0.8 * noise)

matrix = ub.Figure(6.4, 5.2)
(
    matrix.add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    .heatmap(
        metrics,
        metrics,
        correlations,
        center=0,
        annotate=True,
        format=ub.TickFormat.fixed(2),
    )
    .colorbar("Correlation")
    .title("Metric correlations")
    .subtitle("Pearson r over 400 days, centered at 0")
)
matrix.save_png(out / "heatmap.png")

days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
hours = [f"{hour:02d}:00" for hour in range(8, 20, 2)]
visits = rng.poisson(40, size=(len(hours), len(days))) * np.linspace(0.6, 1.4, len(hours))[:, None]
visits[2, 6] = np.nan

week = ub.Figure(6.4, 4.0)
(
    week.add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    .heatmap(days, hours, visits, cmap=ub.Cmap.VIRIDIS)
    .colorbar("Visits", position=ub.ColorbarPosition.BOTTOM)
    .title("Visits by day and hour")
    .subtitle("The Sunday noon count is missing")
)
week.save_png(out / "heatmap-week.png")
