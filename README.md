# umber

umber is a plotting library for Python with a rendering core written in Rust. It draws line, step, scatter, area, stacked area, bar, histogram, box, violin, bubble, error bar, band, stem, reference-line, heatmap, mesh, 2-D histogram and hexbin charts on numeric, log, category and time axes, alone or on a grid of panels with shared axes. It has colormaps and colorbars, careful typography, three built-in themes, and PNG or SVG output.

[简体中文](https://github.com/TJ-Junhao/umber/blob/main/README-ZH.md)

![A line chart of daily temperature at two stations, in the Graphite theme](https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/lines-graphite.png)

- **Fast on large data.** Rendering runs in Rust: a 1,000,000-point line renders to PNG in about 58 ms, and a 100,000-point scatter in about 50 ms (see [Performance](#performance)).
- **Explicit.** Every option is a typed value (`Color`, `Size`, `Dash`, `TickFormat`, …). There are no format strings, no positional style arguments and no silent conversions.
- **Fails fast, pointing at the problem.** Invalid input raises at the call that caused it, with a message, a hint and the offending argument highlighted in the traceback.
- **Statically typed.** Category and numeric axes are distinct types: calling `bar` on a numeric axes is reported by pyright and mypy before the code runs.
- **Bundled fonts.** Text in the bundled fonts' coverage looks the same on every machine; other characters fall back to system fonts.

## Installation

```sh
pip install umber
```

umber requires Python 3.12 or newer and NumPy 2. Prebuilt wheels are published for Linux (x86_64 and aarch64), macOS (Apple silicon and Intel) and Windows (x64); free-threaded Python 3.14 and 3.15 or newer have wheels of their own. On other platforms `pip` builds from source, which needs Rust 1.88 or newer.

## Quick start

```python
import numpy as np
import umber as ub

t = np.linspace(0, 10, 1_000)
ub.line(t, np.sin(t), label="sin").title("A first chart").figure.save_png("chart.png")
```

The quick functions `ub.line`, `ub.scatter`, `ub.bar` and `ub.hist` create a figure with one axes and return the axes, so the rest of the chart is built by chaining.

## Building a figure

```python
import numpy as np
import umber as ub

rng = np.random.default_rng(7)
days = np.arange(60)
north = 20 + np.cumsum(rng.normal(0, 1, 60))
south = 18 + np.cumsum(rng.normal(0, 1, 60))

fig = ub.Figure(6, 4, theme=ub.Theme.GRAPHITE)  # width and height in inches
ax = fig.add_axes()
ax.title("Daily temperature").subtitle("Two stations, last 60 days")
ax.xlabel("Day").ylabel("°C")
ax.line(days, north, label="North")
ax.line(days, south, label="South", dash=ub.Dash.DASHED)

fig.save_png("temperature.png", dpi=200)
fig.save_svg("temperature.svg")
```

This is the chart at the top of this page. Data can be a list, a tuple, a NumPy array, or a pandas or polars Series. Missing values (`NaN`, or nulls in nullable pandas and polars columns) leave a gap.

| Chart | Method |
| --- | --- |
| Line | `ax.line(x, y)` |
| Scatter | `ax.scatter(x, y, marker=ub.Marker.TRIANGLE, size=ub.Size.times(1.5))` |
| Area | `ax.area(x, y)` |
| Bar, grouped, stacked | `ax.bar(categories, heights)`, `ax.grouped_bar(categories, series)`, `ax.stacked_bar(categories, series)` |
| Histogram | `ax.hist(values, bins=30)` |
| Step | `ax.step(x, y, where=ub.Step.MID)` |
| Stacked area | `ax.stacked_area(x, [ub.Series("A", a), ub.Series("B", b)])` |
| Box, grouped | `ax.box(categories, groups)`, `ax.grouped_box(categories, [ub.Distribution("A", groups)])` |
| Violin, grouped | `ax.violin(categories, groups)`, `ax.grouped_violin(categories, distributions)` |
| Bubbles | `ax.bubbles(x, y, sizes, label="Cities")` (adds a size legend) |
| Error bars | `ax.y_error_bars(x, low, high)`, `ax.x_error_bars(y, low, high)` |
| Band | `ax.band(x, low, high)` |
| Stem | `ax.stem(x, y)` |
| Reference lines and spans | `ax.hline(y)`, `ax.vline(x)`, `ax.hspan(low, high)`, `ax.vspan(low, high)` |
| Heatmap | `ax.heatmap(x_categories, y_categories, values, annotate=True)` |
| Mesh | `ax.mesh(x_edges, y_edges, values)` |
| 2-D histogram, hexbin | `ax.hist2d(x, y, bins=40)`, `ax.hexbin(x, y, gridsize=30)` |
| Color-mapped points | `ax.scatter(x, y, color=ub.ColorBy(values))` |

Bar, box and violin charts need a category axis. On a category x axis, `fig.add_axes(x=ub.Scale.category())`, they are vertical; on a category y axis, `fig.add_axes(y=ub.Scale.category())`, they are horizontal, and a scatter of values against categories is a dot plot. A heatmap needs category axes on both sides. Automatic category labels start at 0 and use the smallest step in 1, 2, 5, 10, 20, 50, ... that keeps neighbouring labels apart by the same minimum gap as numeric ticks; shared axes show the same labels. Log axes use `ub.Scale.log()` and time axes `ub.Scale.time()`. Limits, ticks and tick label formats are set with `xlim`, `ylim`, `xticks`, `xticks_labeled`, `ytick_format(ub.TickFormat.percent())` and their counterparts on the other axis.

Automatic limits contain whole bubble markers, outlines included; explicit limits clip them. Legend swatches show each mark's fill as drawn over the plot background.

<p>
  <img alt="A grouped bar chart in the Pigment theme" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/bars-pigment.png" width="49%">
  <img alt="A histogram with a gradient fill in the Pigment theme" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/histogram-pigment.png" width="49%">
</p>

## Grids and shared axes

`Figure(rows=2, cols=2)` declares a grid. Its default `width=ub.AUTO` and `height=ub.AUTO` give each column 6 inches and each row 4 inches; an explicit size is for the whole figure. Add panels by their zero-based top-left cell:

```python
import copy
import umber as ub

fig = ub.Figure(rows=2, cols=2, share_x=ub.Share.COL, legend=ub.Legend.MERGED)
fig.title("Station signals").subtitle("Shared time scales")
fig.add_axes(row=0, col=0).line([0, 1, 2], [1, 3, 2], label="North")
fig.add_axes(row=1, col=0).line([0, 1, 2], [2, 1, 3], label="South")
fig.add_axes(row=0, col=1, row_span=2).line([0, 1, 2], [1, 3, 2], label="North")
fig.save_png("subplots.png")

variant = copy.copy(fig)
variant.axes_at(row=0, col=1).title("Comparison")  # the original is unchanged
```

`Share.NONE`, `Share.ROW`, `Share.COL` and `Share.ALL` choose which panels share a scale, limits, ticks and tick format. `Legend.PER_PANEL` keeps a separate legend for each panel instead of the default merged legend. Empty cells remain empty; overlapping panels are refused.

Use `fig.axes_at(row=..., col=..., x=..., y=...)` to retrieve axes, including those in a copy. The scale kinds and log bases must match creation; for category axes, pass `x=ub.Scale.category()` to get `Axes[str, float]`. A spanning panel is found only at its top-left cell. A row or column can be omitted only when that dimension has size 1. `Figure.axes` has been removed in favor of this typed method.

`fig.cells()` yields immutable `Cell` positions in row-major order, occupied or not; `fig.cells(n)` yields the first `n`. For example, `for cell, values in zip(fig.cells(), datasets): cell.add_axes().line(x, values)` adds one panel per dataset. `AUTO` is a typed sentinel, not `None`, and only parameters that declare it accept it.

## Colors and colorbars

Heatmaps, meshes, 2-D histograms and hexbins color each cell by its value, and `color=ub.ColorBy(values)` does the same for the points of a scatter or bubble chart. The color-mapped marks of one axes share a single color scale, and a colorbar beside the plot explains it.

```python
import numpy as np
import umber as ub

rng = np.random.default_rng(41)
x = rng.normal(size=20_000)
y = 0.6 * x + rng.normal(scale=0.7, size=x.size)

ax = ub.Figure(theme=ub.Theme.MIDNIGHT).add_axes()
ax.hexbin(x, y, gridsize=30, scale=ub.Scale.log()).colorbar("Count")
ax.figure.save_png("density.png")
```

`cmap=` chooses the colormap: each theme has its own sequential and diverging colormaps, and `Cmap.VIRIDIS`, `Cmap.MAGMA`, `Cmap.CIVIDIS` and `Cmap.BLUE_RED` are built in. `Cmap.from_colors([...])` builds one from your colors, mixed in the perceptual Oklab space, and `.reversed()` turns one around. `scale=ub.Scale.log()` maps values on a log scale, and `center=0` makes the mapping diverging and symmetric about that value. The color range runs from the smallest to the largest finite value; `ax.clim(low, high)` sets it, and values outside it take the end colors.

`ax.colorbar(label, position=..., format=...)` labels the colorbar, moves it below the plot with `ColorbarPosition.BOTTOM` or hides it with `ColorbarPosition.NONE`. On a grid, `Figure(share_color=ub.Share.ROW)` gives the panels of each row one color range and one colorbar at the right of the row; `Share.COL` puts one below each column, and `Share.ALL` one beside the whole grid. `fig.colorbar(...)` sets those shared colorbars.

<p>
  <img alt="An annotated correlation heatmap with a diverging colormap in the Graphite theme" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/heatmap-graphite.png" width="49%">
  <img alt="Hexagonal point counts on a log color scale in the Midnight theme" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/hexbin-midnight.png" width="49%">
</p>

## Time axes

`ub.Scale.time()` makes a time axis. It takes `datetime`, `date` and `numpy.datetime64` values, NumPy `datetime64` arrays of any unit, and naive pandas and polars datetime columns:

```python
import numpy as np
import umber as ub

hours = np.datetime64("2026-03-02T00", "h") + np.arange(7 * 24)
demand = 60 + 20 * np.sin((np.arange(7 * 24) - 6) / 24 * 2 * np.pi)

ax = ub.Figure(8, 4).add_axes(x=ub.Scale.time())
ax.band(hours, demand - 6, demand + 6, label="Range").line(hours, demand, label="Demand")
ax.vspan(np.datetime64("2026-03-05T01"), np.datetime64("2026-03-05T05"), label="Maintenance")
ax.figure.save_png("demand.png")
```

Ticks fall on calendar steps, such as months, days or hours, with concise labels; `xtick_format(ub.TickFormat.iso())` writes them in ISO 8601 instead. Limits, ticks, reference lines and spans on a time axis take times. Times are kept to the microsecond, `NaT` leaves a gap, and a time zone is refused: convert to naive times first, for example with `.tz_convert(None)` in pandas. Bands and stacked areas take times on the x axis only.

![A week of hourly demand with its range and a maintenance window, in the Pigment theme](https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/time-pigment.png)

## Titles and legends

`fig.title(text)` and `fig.subtitle(text)` set text above the whole grid; `ax.title(text)` and `ax.subtitle(text)` belong to one panel. Each returns the same object for chaining, and an empty string removes the text. The figure title is 1.5 times the panel title size.

`Theme.with_title_align(ub.Align.LEFT)` sets title alignment; `CENTER` and `RIGHT` are also available. Subtitles follow their title. On a multi-panel figure, the merged legend stays below the figure title and subtitle and follows their alignment, without a separate position setting. To place legends by panel, use `Legend.PER_PANEL`.

```python
import umber as ub

theme = ub.Theme.PIGMENT.with_title_align(ub.Align.CENTER).with_legend_position(
    ub.LegendPosition.BOTTOM
)
fig = ub.Figure(theme=theme).title("Station signals")
ax = fig.add_axes().line([0, 1, 2], [1, 3, 2], label="North")
ax.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.RIGHT)
ax.legend(ub.LegendStyle.HEADER, position=ub.AUTO)  # back to the theme's BOTTOM
```

For single-panel figures and `Legend.PER_PANEL`, `LegendPosition.TOP_LEFT` (default) and `TOP_RIGHT` place a row below the panel title; `RIGHT` places a column beside the plot; `BOTTOM` places a row below the x axis title; `BEST` places the legend inside the plot where it covers the least data. Panel legend positions do not follow title alignment. Omitting `position`, or passing `ub.AUTO`, clears a previous override and uses the theme's position; it does not mean `BEST`. `LegendStyle.DIRECT` accepts only the default `position=ub.AUTO`, since its labels sit at line ends. `LegendStyle.NONE` hides the legend.

## Themes

Three themes are built in: `Theme.PIGMENT` (warm paper, serif titles), `Theme.GRAPHITE` (neutral, sans-serif) and `Theme.MIDNIGHT` (dark, monospaced figures). A theme is an immutable value; derive a variant with its `with_*` methods:

```python
import umber as ub

theme = ub.Theme.MIDNIGHT.with_fill_style(ub.FillStyle.SOLID).with_legend_style(
    ub.LegendStyle.HEADER
)
fig = ub.Figure(theme=theme)
```

![Two sine waves in the Midnight theme](https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/lines-midnight.png)

## Errors

An error is raised by the call that caused it: `umber.DataError` for invalid data, `umber.StyleError` for an invalid option value, `umber.UsageError` for a call that is not allowed in the current state, and `TypeError` for a value of the wrong type. The three umber errors derive from `umber.UmberError`. The traceback points at the argument:

```text
Traceback (most recent call last):
  File "example.py", line 7, in <module>
    ax.line(t, y, label="signal")
    ~~~~~~~^^^^^^^^^^^^^^^^^^^^^^
umber.DataError: Axes.line(): y[2] is inf; values must be finite.
  ax.line(t, y, label="signal")
             ^
hint: use NaN to leave a gap in the line.
```

```text
TypeError: Axes.line(): color is str; color must be of type Color.
  ax.line([0, 1, 2], [1, 3, 2], color="red")
                                ^^^^^^^^^^^
hint: use Color.hex("#3366B8"), Color.rgb() or a named color such as Color.BLUE.
```

In a terminal that supports it, the hint and the highlight are shown in color; set `NO_COLOR` to turn color off or `FORCE_COLOR` to force it. The same notes appear in Jupyter.

## Static typing

The package ships type information (`py.typed`) and is checked with pyright and mypy in strict mode. `Figure().add_axes()` is `Axes[float, float]`; `add_axes(x=Scale.category())` is `Axes[str, float]`, `add_axes(y=Scale.category())` is `Axes[float, str]`, two category axes are `Axes[str, str]` and `add_axes(x=Scale.time())` is `Axes[Time, float]`. Methods that only make sense on one kind of axis are typed accordingly:

```python
import umber as ub

ax = ub.Figure().add_axes()
ax.bar(["a", "b"], [1, 2])  # pyright and mypy: bar() needs Axes[str, float]
```

## Performance

Medians from the [criterion](https://github.com/criterion-rs/criterion.rs) suite in `crates/umber-core/benches`, 6 × 4 in at 200 dpi, on an Intel Core Ultra 9 290HX running Linux:

| Case | Time |
| --- | ---: |
| 1,000,000-point line → PNG | 58 ms |
| 1,000,000-point line on a time axis → PNG | 54 ms |
| 1,000,000-point line → SVG | 19 ms |
| 100,000-point scatter → PNG | 50 ms |
| Histogram of 1,000,000 values → PNG | 21 ms |
| 2-D histogram of 1,000,000 points, 100 × 100 bins → PNG | 12 ms |
| Hexbin of 1,000,000 points, grid size 50 → PNG | 69 ms |
| Heatmap of 1,000 × 1,000 cells → PNG | 1.26 s |
| 4 × 5 grid, one 10,000-point line per panel → PNG | 103 ms |

Lines with sorted numeric or time x values are downsampled when they have more than four points per pixel column of the plot (counted at a nominal 300 dpi), keeping the first, last, minimum and maximum point in each column, which is visually lossless. Large scatter plots are composited on up to 8 threads; the output is the same for any number of threads.

## Rust

The rendering core is published as the [`umber-core`](https://crates.io/crates/umber-core) crate and can be used directly from Rust:

```rust
use umber_core::{AxisData, Figure, LineOptions, ScaleKind, Theme};

let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
figure
    .add_axes(ScaleKind::Linear, ScaleKind::Linear)?
    .title("Signal")
    .line(AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0]), &[1.0, 3.0, 2.0, 4.0], LineOptions::default())?;
std::fs::write("signal.png", figure.to_png(144)?)?;
```

## Status

umber is at version 0.x. The API is not yet stable and may change or be removed without a deprecation period. More examples are in [`examples/`](https://github.com/TJ-Junhao/umber/tree/main/examples).

## License

Licensed under either of MIT or Apache-2.0, at your option (`MIT OR Apache-2.0`). Bundled fonts are under the SIL Open Font License 1.1; see `crates/umber-core/fonts/LICENSES` and `crates/umber-core/fonts/MANIFEST.md`. Wheels include statically linked Rust dependencies; their licenses are listed in `THIRD-PARTY-LICENSES.txt`.
