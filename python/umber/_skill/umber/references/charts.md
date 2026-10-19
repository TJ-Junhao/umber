# Choosing a chart

Every chart is a method on an axes. The axis scales chosen in `add_axes` decide which methods apply. All examples below run as written.

```python
import numpy as np
import umber as ub

rng = np.random.default_rng(0)
```

## Trends and relationships (numeric or time x)

| Data | Method |
|---|---|
| A series over x | `ax.line(x, y)`, `ax.step(x, y)` |
| Filled area, stacked shares | `ax.area(x, y)`, `ax.stacked_area(x, [ub.Series(...), ...])` |
| A range around a line | `ax.band(x, low, high)` |
| Points | `ax.scatter(x, y)`; colored by a third value with `color=ub.ColorBy(values)` |
| Points sized by a value | `ax.bubbles(x, y, sizes)`: areas relative to the largest size |
| Discrete samples | `ax.stem(x, y)` |
| Uncertainty | `ax.y_error_bars(x, low, high)`, `ax.x_error_bars(y, low, high)` |
| Reference lines and regions | `ax.hline(y)`, `ax.vline(x)`, `ax.hspan(low, high)`, `ax.vspan(low, high)` |

```python
fig = ub.Figure(6, 4)
ax = fig.add_axes()
x = np.linspace(0, 10, 50)
ax.band(x, np.sin(x) - 0.2, np.sin(x) + 0.2, label="range")
ax.line(x, np.sin(x), label="mean")
ax.hline(0.0)
svg = fig.to_svg()
```

A time x axis takes `datetime`, `date` or `numpy.datetime64` values:

```python
import datetime as dt

days = [dt.date(2026, 3, d) for d in range(1, 15)]
ax = ub.Figure(6, 3).add_axes(x=ub.Scale.time())
ax.line(days, list(range(14)))
svg = ax.figure.to_svg()
```

## Comparing categories (category x, or category y for horizontal)

| Data | Method |
|---|---|
| One value per category | `ax.bar(categories, heights)`; values on the bars with `annotate=True` |
| Several series side by side | `ax.grouped_bar(categories, [ub.Series(...), ...])` |
| Parts of a whole per category | `ax.stacked_bar(categories, [ub.Series(...), ...])` |
| A distribution per category | `ax.box(categories, groups)`, `ax.violin(categories, groups)` |
| Every point per category | `ax.strip(categories, groups)`, `ax.swarm(categories, groups)` |
| Grouped distributions | `ax.grouped_box(categories, [ub.Distribution(...), ...])`, `grouped_violin` |

```python
ax = ub.Figure(6, 4).add_axes(x=ub.Scale.category())
ax.grouped_bar(
    ["Q1", "Q2", "Q3"],
    [ub.Series("North", [12.0, 15.0, 9.0]), ub.Series("South", [8.0, 11.0, 14.0])],
    annotate=True,
)
svg = ax.figure.to_svg()
```

Make the y axis the category axis for horizontal bars, boxes and violins:

```python
ax = ub.Figure(6, 4).add_axes(y=ub.Scale.category())
groups = [rng.normal(m, 1.0, 80) for m in (3.0, 4.5, 4.0)]
ax.box(["A", "B", "C"], groups).strip(["A", "B", "C"], groups)
svg = ax.figure.to_svg()
```

## Distributions of one variable (numeric x)

| Data | Method |
|---|---|
| Histogram | `ax.hist(values, bins=30)` on a linear x axis; `style=ub.Hist.STEP` or `ub.Hist.STACKED` (with `by=`); `stat=ub.Stat.FRACTION` or `ub.Stat.DENSITY`; `cumulative=True` (with counts or fractions, not density) |
| Smooth density | `ax.kde(values)` |
| Cumulative distribution | `ax.ecdf(values)`; `complementary=True` for 1 − F |
| Events on rows (spike trains, logs) | `ax.events(rows, positions)` on a category y axis |

```python
values = rng.normal(0.0, 1.0, 500)
ax = ub.Figure(6, 4).add_axes()
ax.hist(values, bins=30, stat=ub.Stat.DENSITY, label="sample")
ax.kde(values, label="density")
svg = ax.figure.to_svg()
```

## Two-dimensional fields and densities

| Data | Method |
|---|---|
| A matrix with named rows and columns | `ax.heatmap(x_categories, y_categories, values)` on two category axes |
| A field on a grid with edges | `ax.mesh(x_edges, y_edges, values)` |
| A field on grid points, as level lines or bands | `ax.contour(x, y, z, style=ub.Contour.LINES \| FILLED \| BOTH)` |
| Density of many points | `ax.hist2d(x, y)`, `ax.hexbin(x, y)` |

Color-mapped marks share the axes' color scale; add `ax.colorbar("Label")`.

```python
x = np.linspace(-2, 2, 40)
y = np.linspace(-1, 1, 30)
z = np.exp(-(x[None, :] ** 2 + y[:, None] ** 2))
ax = ub.Figure(6, 4).add_axes()
ax.contour(x, y, z, style=ub.Contour.FILLED).colorbar("Intensity")
svg = ax.figure.to_svg()
```

## Parts of a whole

`ub.pie(categories, values)` or `fig.add_pie_axes().pie(...)`; `hole=0.5` makes a donut, `annotate=True` adds the shares. A pie panel only has `pie`, `title` and `legend`.

```python
ax = ub.pie(["Web", "App", "Shop"], [50.0, 30.0, 20.0], hole=0.5, annotate=True)
svg = ax.figure.to_svg()
```

## Words on the chart

`ax.text(text, (x, y))` places text at a data position; `ax.annotate(text, (x, y), offset=(dx, dy))` writes it a few points away with an arrow. They never change the limits.

```python
ax = ub.Figure(6, 4).add_axes()
ax.line([0, 1, 2, 3], [1.0, 3.0, 2.0, 4.0])
ax.annotate("peak", (1, 3.0), offset=(0.0, 14.0))
svg = ax.figure.to_svg()
```
