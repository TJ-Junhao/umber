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


def gallery() -> None:
    """Twelve kinds of chart on one 3 × 4 grid, the image at the top of the README."""
    rng = np.random.default_rng(12)

    fig = ub.Figure(13.5, 9, rows=3, cols=4, legend=ub.Legend.PER_PANEL, theme=ub.Theme.PIGMENT)
    fig.title("umber").subtitle("Plotting for Python, rendered in Rust")

    # Lines with a band.
    x = np.linspace(0, 12, 200)
    ax = fig.add_axes(row=0, col=0).title("Lines and bands")
    ax.band(x, np.sin(x) - 0.3, np.sin(x) + 0.3).line(x, np.sin(x), label="mean").line(
        x, np.cos(x) * 0.8, label="trend"
    )
    ax.legend(ub.LegendStyle.DIRECT)

    # Grouped bars with values.
    ax = fig.add_axes(row=0, col=1, x=ub.Scale.category()).title("Grouped bars")
    ax.grouped_bar(
        ["Q1", "Q2", "Q3"],
        [ub.Series("North", [12.0, 18.0, 15.0]), ub.Series("South", [9.0, 11.0, 17.0])],
        annotate=True,
    )
    ax.legend(ub.LegendStyle.NONE)

    # Scatter coloured by value.
    px = rng.normal(size=400)
    py = 0.6 * px + rng.normal(scale=0.6, size=400)
    ax = fig.add_axes(row=0, col=2).title("Color by value")
    ax.scatter(px, py, color=ub.ColorBy(px * py, center=0.0)).colorbar("x · y")

    # Donut.
    ax = fig.add_pie_axes(row=0, col=3).title("Donut")
    ax.pie(["Web", "App", "Shop"], [52.0, 30.0, 18.0], hole=0.45, annotate=True).legend(
        ub.LegendStyle.CHIPS
    )

    # Histogram and KDE.
    v = np.concatenate([rng.normal(40, 8, 3000), rng.normal(70, 6, 1500)])
    ax = fig.add_axes(row=1, col=0).title("Histogram and KDE")
    ax.hist(v, bins=40, stat=ub.Stat.DENSITY).kde(v)
    ax.legend(ub.LegendStyle.NONE)

    # Violin with swarm.
    groups = [rng.normal(m, s, 60) for m, s in ((3.0, 0.8), (4.5, 1.1), (4.0, 0.6))]
    ax = fig.add_axes(row=1, col=1, x=ub.Scale.category()).title("Violins and swarms")
    ax.violin(["A", "B", "C"], groups).swarm(["A", "B", "C"], groups, size=ub.Size.pt(3))
    ax.legend(ub.LegendStyle.NONE)

    # Contour.
    cx = np.linspace(-3, 3, 121)
    cy = np.linspace(-2, 2, 81)
    gx, gy = np.meshgrid(cx, cy)
    z = 1.2 * np.exp(-((gx - 1) ** 2) - (gy - 0.4) ** 2) + 0.8 * np.exp(
        -((gx + 1.2) ** 2) / 0.8 - (gy + 0.5) ** 2 / 0.6
    )
    ax = fig.add_axes(row=1, col=2).title("Contours")
    ax.contour(cx, cy, z, style=ub.Contour.BOTH, annotate=True)

    # Heatmap.
    names = ["Mon", "Tue", "Wed", "Thu", "Fri"]
    hours = ["9", "12", "15", "18"]
    m = rng.integers(10, 90, size=(len(hours), len(names))).astype(float)
    ax = fig.add_axes(row=1, col=3, x=ub.Scale.category(), y=ub.Scale.category()).title("Heatmap")
    ax.heatmap(names, hours, m, annotate=True, format=ub.TickFormat.fixed(0))

    # Time series with annotation.
    days = np.datetime64("2026-03-01") + np.arange(30)
    load = 25 + np.cumsum(rng.normal(0, 1.5, 30))
    ax = fig.add_axes(row=2, col=0, x=ub.Scale.time()).title("Time and annotations")
    ax.line(days, load)
    peak = int(np.argmax(load))
    ax.annotate("peak", (days[peak], float(load[peak])), offset=(20.0, 12.0))

    # Twin y.
    months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"]
    ax = fig.add_axes(row=2, col=1, x=ub.Scale.category()).title("Two y axes")
    ax.bar(months, [52.0, 48.0, 55.0, 61.0, 72.0, 84.0], label="rain")
    ax.twin_y().line(months, [4.0, 5.0, 9.0, 13.0, 18.0, 22.0], label="°C")
    ax.legend(ub.LegendStyle.NONE)

    # Hexbin.
    hx = rng.normal(size=20000)
    hy = 0.6 * hx + rng.normal(scale=0.7, size=hx.size)
    ax = fig.add_axes(row=2, col=2).title("Hexbin, log color")
    ax.hexbin(hx, hy, gridsize=25, scale=ub.Scale.log()).colorbar("count")

    # ECDF by group.
    vals = np.concatenate([rng.lognormal(3.0, 0.4, 400), rng.lognormal(2.7, 0.3, 400)])
    grp = ["before"] * 400 + ["after"] * 400
    ax = fig.add_axes(row=2, col=3).title("ECDF, grouped by=")
    ax.ecdf(vals, by=grp).legend(ub.LegendStyle.DIRECT)

    fig.save_png(OUT / "gallery.png", dpi=160)


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


def channels() -> None:
    """Visits by channel as an annotated donut, in the Pigment theme."""
    ax = ub.Figure(6, 4, theme=ub.Theme.PIGMENT).add_pie_axes()
    ax.title("Visits by channel")
    ax.pie(
        ["Search", "Direct", "Social", "Email", "Other"],
        [41, 24, 17, 11, 7],
        hole=0.55,
        annotate=True,
    )
    ax.figure.save_png(OUT / "pie-pigment.png", dpi=DPI)


def elevation() -> None:
    """A filled elevation map with labelled level lines, in the Graphite theme."""
    x = np.linspace(-3, 3, 121)
    y = np.linspace(-2, 2, 81)
    gx, gy = np.meshgrid(x, y)
    z = (
        1.2 * np.exp(-((gx - 1.0) ** 2) - (gy - 0.4) ** 2)
        + 0.8 * np.exp(-((gx + 1.2) ** 2) / 0.8 - (gy + 0.5) ** 2 / 0.6)
        - 0.15 * gx
    )
    fig = ub.Figure(6, 4, theme=ub.Theme.GRAPHITE)
    ax = fig.add_axes()
    ax.title("Elevation").xlabel("km").ylabel("km")
    ax.contour(x, y, z, style=ub.Contour.BOTH, annotate=True)
    ax.colorbar("km")
    fig.save_png(OUT / "contour-graphite.png", dpi=DPI)


def weather() -> None:
    """Monthly rainfall bars and a temperature line on a twin y axis, in the Midnight theme."""
    months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
    rainfall = [52, 48, 55, 61, 72, 84, 91, 88, 70, 64, 58, 54]
    temperature = [4, 5, 9, 13, 18, 22, 25, 24, 20, 14, 9, 5]
    fig = ub.Figure(6, 4, theme=ub.Theme.MIDNIGHT)
    left = fig.add_axes(x=ub.Scale.category())
    left.title("A year of weather").bar(months, rainfall, label="Rainfall").ylabel("mm")
    left.twin_y().line(months, temperature, label="Temperature").ylabel("°C")
    fig.save_png(OUT / "twin-midnight.png", dpi=DPI)


def requests() -> None:
    """A time series with an annotated peak and dip and a target line, in the Pigment theme."""
    days = np.datetime64("2026-03-01") + np.arange(21)
    load = [21, 22, 24, 33, 25, 23, 15, 20, 23, 24, 27, 25, 24, 26, 28, 27, 25, 22, 24, 26, 25]
    fig = ub.Figure(6, 4, theme=ub.Theme.PIGMENT)
    ax = fig.add_axes(x=ub.Scale.time())
    ax.title("Requests").subtitle("Three weeks in March 2026").ylabel("requests / s")
    ax.line(days, load).hline(30.0, color=ub.Color.GRAY, dash=ub.Dash.DASHED).ylim(10, 40)
    ax.annotate("Release", (days[3], 33.0), offset=(28, 14))
    ax.annotate("Outage", (days[6], 15.0), offset=(22, -20))
    ax.text("Target", (days[16], 32.0), color=ub.Color.GRAY)
    fig.save_png(OUT / "annotations-pigment.png", dpi=DPI)


def main() -> None:
    """Render every README image."""
    OUT.mkdir(parents=True, exist_ok=True)
    gallery()
    temperature()
    sales()
    signals()
    distribution()
    correlations()
    density()
    demand()
    channels()
    elevation()
    weather()
    requests()


if __name__ == "__main__":
    main()
