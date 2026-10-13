"""Render the images shown in README.md into assets/readme/.

Run from the repository root after building the package:

    uv run --no-sync python scripts/readme/render_images.py
"""

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


def main() -> None:
    """Render every README image."""
    OUT.mkdir(parents=True, exist_ok=True)
    temperature()
    sales()
    signals()
    distribution()


if __name__ == "__main__":
    main()
