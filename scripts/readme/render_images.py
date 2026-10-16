"""Render the images shown in README.md into assets/readme/.

Run from the repository root after building the package:

    uv run --no-sync python scripts/readme/render_images.py
"""

import math
from pathlib import Path

import numpy as np

import umber as ub

OUT = Path(__file__).resolve().parents[2] / "assets" / "readme"
DPI = 144


def temperature() -> None:
    """Two noisy station series as lines, in the Graphite theme."""
    rng = np.random.default_rng(7)
    days = np.arange(60)
    north = 20 + np.cumsum(rng.normal(0, 1, 60))
    south = 18 + np.cumsum(rng.normal(0, 1, 60))
    fig = ub.Figure(6, 4, theme=ub.Theme.GRAPHITE)
    ax = fig.add_axes()
    ax.title("Daily temperature").subtitle("Two stations, last 60 days")
    ax.xlabel("Day").ylabel("°C")
    ax.line(days, north, label="North")
    ax.line(days, south, label="South", dash=ub.Dash.DASHED)
    fig.save_png(OUT / "lines-graphite.png", dpi=DPI)


def sales() -> None:
    """Quarterly sales as a grouped bar chart, in the Pigment theme."""
    quarters = ["Q1", "Q2", "Q3", "Q4"]
    fig = ub.Figure(6, 4, theme=ub.Theme.PIGMENT)
    ax = fig.add_axes(x=ub.Scale.category())
    ax.title("Sales by region").ylabel("units")
    ax.grouped_bar(
        quarters,
        [ub.Series("North", [12, 18, 15, 22]), ub.Series("South", [9, 11, 17, 14])],
    )
    fig.save_png(OUT / "bars-pigment.png", dpi=DPI)


def signals() -> None:
    """Two sine waves, in the Midnight theme."""
    x = np.linspace(0, 10, 400)
    fig = ub.Figure(6, 4, theme=ub.Theme.MIDNIGHT)
    ax = fig.add_axes()
    ax.title("Two signals").xlabel("time (s)").ylabel("level")
    ax.line(x, np.sin(x), label="sin(x)")
    ax.line(x, np.sin(x / 2), label="sin(x/2)")
    fig.save_png(OUT / "lines-midnight.png", dpi=DPI)


def distribution() -> None:
    """A bimodal histogram with the gradient fill style."""
    rng = np.random.default_rng(3)
    values = np.concatenate([rng.normal(40, 8, 4_000), rng.normal(70, 6, 2_000)])
    fig = ub.Figure(6, 4, theme=ub.Theme.PIGMENT.with_fill_style(ub.FillStyle.GRADIENT))
    ax = fig.add_axes()
    ax.title("Response time").subtitle("6,000 requests").xlabel("ms")
    ax.hist(values, bins=40)
    fig.save_png(OUT / "histogram-pigment.png", dpi=DPI)


def correlations() -> None:
    """An annotated correlation matrix, diverging around zero, in the Graphite theme."""
    rng = np.random.default_rng(21)
    metrics = ["Revenue", "Users", "Orders", "Churn", "Latency", "Margin"]
    base = rng.normal(size=(2, 400))
    mix = rng.normal(size=(len(metrics), 2))
    noise = rng.normal(size=(len(metrics), 400))
    matrix = np.corrcoef(mix @ base + 0.8 * noise)
    fig = ub.Figure(6, 4, theme=ub.Theme.GRAPHITE)
    ax = fig.add_axes(x=ub.Scale.category(), y=ub.Scale.category())
    ax.title("Metric correlations")
    ax.heatmap(metrics, metrics, matrix, center=0, annotate=True, format=ub.TickFormat.fixed(2))
    ax.colorbar("Pearson r")
    fig.save_png(OUT / "heatmap-graphite.png", dpi=DPI)


def density() -> None:
    """Hexagonal counts of 20,000 correlated points on a log color scale, in the Midnight theme."""
    rng = np.random.default_rng(41)
    x = rng.normal(size=20_000)
    y = 0.6 * x + rng.normal(scale=0.7, size=x.size)
    fig = ub.Figure(6, 4, theme=ub.Theme.MIDNIGHT)
    ax = fig.add_axes()
    ax.title("Point density").subtitle("20,000 points").xlabel("x").ylabel("y")
    ax.hexbin(x, y, gridsize=30, scale=ub.Scale.log())
    ax.colorbar("Count")
    fig.save_png(OUT / "hexbin-midnight.png", dpi=DPI)


def demand() -> None:
    """A week of hourly demand on a time axis, with its range and a marked window."""
    hours = np.datetime64("2026-03-02T00", "h") + np.arange(7 * 24)
    load = np.array(
        [60 + 20 * math.sin((h - 6) / 24 * 2 * math.pi) + 4 * math.sin(h / 5) for h in range(168)]
    )
    fig = ub.Figure(8, 4, theme=ub.Theme.PIGMENT)
    ax = fig.add_axes(x=ub.Scale.time())
    ax.title("Hourly demand").subtitle("One week in March 2026").ylabel("MW")
    ax.band(hours, load - 6, load + 6, label="Range")
    ax.line(hours, load, label="Demand")
    ax.vspan(np.datetime64("2026-03-05T01"), np.datetime64("2026-03-05T05"), label="Maintenance")
    fig.save_png(OUT / "time-pigment.png", dpi=DPI)


def main() -> None:
    """Render every README image."""
    OUT.mkdir(parents=True, exist_ok=True)
    temperature()
    sales()
    signals()
    distribution()
    correlations()
    density()
    demand()


if __name__ == "__main__":
    main()
