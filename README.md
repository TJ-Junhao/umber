# umber

umber is a plotting library for Python with a rendering core written in Rust. It draws line, scatter, area, bar and histogram charts with careful typography, three built-in themes, and PNG or SVG output.

[简体中文](README-ZH.md)

![A line chart of daily temperature at two stations, in the Graphite theme](https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/lines-graphite.png)

- **Fast on large data.** All per-point work happens in Rust: a 1,000,000-point line renders to PNG in about 55 ms, and a 100,000-point scatter in about 50 ms (see [Performance](#performance)).
- **Explicit.** Every option is a typed value (`Color`, `Size`, `Dash`, `TickFormat`, …). There are no format strings, no positional style arguments and no silent conversions.
- **Fails fast, pointing at the problem.** Invalid input raises at the call that caused it, with a message, a hint and the offending argument highlighted in the traceback.
- **Statically typed.** Category and numeric axes are distinct types: calling `bar` on a numeric axes is reported by pyright and mypy before the code runs.
- **Self-contained.** Fonts are bundled, so a figure looks the same on every machine.

## Installation

```sh
pip install umber
```

umber requires Python 3.12 or newer and NumPy 2. Prebuilt wheels are published for Linux (x86_64 and aarch64), macOS (Apple silicon and Intel) and Windows (x64). On other platforms `pip` builds from source, which needs Rust 1.88 or newer.

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

Bar charts need a category x axis: `fig.add_axes(x=ub.Scale.category())`. Log axes use `ub.Scale.log()`. Limits, ticks and tick label formats are set with `xlim`, `ylim`, `xticks`, `xticks_labeled`, `ytick_format(ub.TickFormat.percent())` and their counterparts on the other axis.

<p>
  <img alt="A grouped bar chart in the Pigment theme" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/bars-pigment.png" width="49%">
  <img alt="A histogram with a gradient fill in the Pigment theme" src="https://raw.githubusercontent.com/TJ-Junhao/umber/main/assets/readme/histogram-pigment.png" width="49%">
</p>

## Themes

Three themes are built in: `Theme.PIGMENT` (warm paper, serif titles), `Theme.GRAPHITE` (neutral, sans-serif) and `Theme.MIDNIGHT` (dark, monospaced figures). A theme is an immutable value; derive a variant with its `with_*` methods:

```python
theme = ub.Theme.MIDNIGHT.with_fill_style(ub.FillStyle.SOLID).with_legend_style(ub.LegendStyle.HEADER)
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

The package ships type information (`py.typed`) and is checked with pyright and mypy in strict mode. `Figure().add_axes()` is `Axes[float, float]`; `add_axes(x=Scale.category())` is `Axes[str, float]`. Methods that only make sense on one kind of axis are typed accordingly:

```python
ax = ub.Figure().add_axes()
ax.bar(["a", "b"], [1, 2])  # pyright and mypy: bar() needs Axes[str, float]
```

## Performance

Medians from the [criterion](https://github.com/criterion-rs/criterion.rs) suite in `crates/umber-core/benches`, 6 × 4 in at 200 dpi, on an Intel Core Ultra 9 290HX running Linux:

| Case | Time |
| --- | ---: |
| 1,000,000-point line → PNG | 53 ms |
| 1,000,000-point line → SVG | 21 ms |
| 100,000-point scatter → PNG | 47 ms |
| Histogram of 1,000,000 values → PNG | 20 ms |

Lines with more points than the plot has pixel columns are downsampled per column, keeping the first, last, minimum and maximum point, which is visually lossless. Large scatter plots are composited on up to 8 threads; the output is the same for any number of threads.

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

umber is at version 0.x. The API may change between minor versions; patch releases within a minor version are compatible. More examples are in [`examples/`](examples).

## License

Licensed under either of MIT or Apache-2.0, at your option (`MIT OR Apache-2.0`). Bundled fonts are under the SIL Open Font License 1.1; see `crates/umber-core/fonts/LICENSES` and `crates/umber-core/fonts/MANIFEST.md`. Wheels include statically linked Rust dependencies; their licenses are listed in `THIRD-PARTY-LICENSES.txt`.
