# Idioms

```python
import numpy as np
import umber as ub
```

## Figures, axes and grids

`ub.Figure(width, height)` takes inches; `rows=` and `cols=` make a grid, and `add_axes(row=, col=)` places an axes (spans with `row_span=`, `col_span=`). `share_x=`, `share_y=` and `share_color=` take `ub.Share.NONE`, `ROW`, `COL` or `ALL`.

```python
fig = ub.Figure(8, 4, cols=2, share_y=ub.Share.ROW)
left = fig.add_axes(col=0).line([0, 1, 2], [1.0, 2.0, 1.5], label="a")
right = fig.add_axes(col=1).line([0, 1, 2], [2.0, 1.0, 3.0], label="b")
fig.title("Two panels")
svg = fig.to_svg()
```

Retrieve an axes later with `fig.axes_at(row=, col=, x=..., y=...)` using the same scales; a pie panel with `fig.pie_axes_at(row=, col=)`.

## Typing: the axis types follow the scales

`add_axes()` returns `Axes[float, float]`. `add_axes(x=ub.Scale.category())` returns `Axes[str, float]`, `y=ub.Scale.category()` gives `Axes[float, str]`, `x=ub.Scale.time()` gives `Axes[ub.Time, float]`. Positions use those types: `text("…", (x, y))` and `annotate` take values of each axis' type — a category name on a category axis, a `datetime` on a time axis. `xlim` and `ylim` apply to numeric and time axes only (numbers or times); category axes have no limits. Methods that need a certain kind of axes are typed to it, so pyright and mypy reject, for example, `bar` on two numeric axes before the code runs.

```python
ax = ub.Figure(6, 4).add_axes(x=ub.Scale.category())
ax.bar(["Mon", "Tue", "Wed"], [3.0, 5.0, 4.0]).ylim(0.0, 6.0)
ax.text("busy", ("Tue", 5.5), align=ub.Align.CENTER)
svg = ax.figure.to_svg()
```

## AUTO, never None

Omit an optional argument to use its documented default. Where automatic selection is supported, the signature names the exact value: `ub.AUTO`, `ub.Color.AUTO`, `ub.Size.AUTO` or `ub.TickFormat.AUTO`. They are not interchangeable (`color=ub.AUTO` is a `TypeError`), and `None` is never accepted.

## Long-form data with `by=`

`line`, `scatter`, `step`, `area`, `ecdf`, `kde`, `hist` and `bar` take `by=`: a column of group names as long as the data. Each group becomes one series, labelled with its name and colored in order. Columns from pandas or polars work as they are; umber never reads a Series' name.

```python
day = [1, 2, 3, 1, 2, 3]
temp = [12.0, 14.5, 13.0, 9.0, 10.5, 11.0]
station = ["North", "North", "North", "South", "South", "South"]
ax = ub.Figure(6, 4).add_axes()
ax.line(day, temp, by=station)
svg = ax.figure.to_svg()
```

With `by=`, give no `color=` or `label=` (each group takes its own); `hist` needs `style=ub.Hist.STEP` or `ub.Hist.STACKED`; `bar(categories, heights, by=groups)` draws grouped bars.

## Legends

Marks with a non-empty `label=` appear in the legend. Choose the style with `ax.legend(ub.LegendStyle.HEADER | CHIPS | DIRECT | NONE)`; `position=ub.LegendPosition.TOP_LEFT | TOP_RIGHT | RIGHT | BOTTOM | BEST` applies to `HEADER` and `CHIPS` (omit it with `DIRECT`, which labels each line at its end). In a grid the Cartesian panels share one legend under the figure title (`ub.Legend.MERGED`, the default), so positions apply to single-panel figures and to `legend=ub.Legend.PER_PANEL`. A pie's legend always belongs to its own panel.

## Footnotes

Put a footnote under the chart with `fig.caption("…")`, not with `ax.text()` at data coordinates: the layout reserves its room, so it never covers the plot and does not move when the data changes. It follows the title alignment, `"\n"` breaks lines, and a line wider than the figure raises `StyleError`.

```python
ax = ub.Figure(6, 4).add_axes().line([0, 1, 2], [1.0, 3.0, 2.0])
ax.figure.caption("Up to 1,000 epochs; patience 100.")
svg = ax.figure.to_svg()
```

For text at a fixed place in the plot, use `at=ub.InPlot(x, y)`, not data coordinates: `x` and `y` are fractions of the plot area (0, 0 is the lower left corner), so a note such as `ax.text("n = 5 folds", at=ub.InPlot(0.02, 0.96))` stays put when the data range changes. `ax.annotate("best", (x, y), text_at=ub.InPlot(0.9, 0.9))` writes its text there, with the arrow to the data point.

## Values next to points

Label points with `annotate=True`; do not place value text by hand with `ax.text()` at data coordinates. Each label follows its point when the data change, takes the series color, and with the default `place=ub.Place.AUTO` sits on the first side of above, right, below and left that stays inside the plot and clear of markers, error bars, lines, bars, areas and other value labels. A fixed side (`ub.Place.ABOVE`, `BELOW`, `LEFT`, `RIGHT`) never moves, and automatic limits grow so that it fits. `format=` sets how values are written (`AUTO` follows the value axis's tick format) and `label_size=` their size; `size=` stays the marker size.

```python
models = ["Baseline", "Gate", "Gate + aug"]
ax = ub.Figure(6, 3).add_axes(y=ub.Scale.category())
ax.x_error_bars(models, [65.8, 70.9, 72.4], [67.2, 72.3, 73.7])
ax.scatter(
    [66.5, 71.6, 73.1], models, label="80 epochs", annotate=True, format=ub.TickFormat.fixed(1)
)
svg = ax.figure.to_svg()
```

`AUTO` never grows the limits: a point at the edge of the data with lines on both sides may have no clear side, which raises `StyleError` when the figure is rendered. Fix the side with `place=`, or leave room with `ylim()` or a larger figure. Text from `ax.text()` and `ax.annotate()` is not avoided.

## Colors, colormaps and color scales

`ub.Color.RED`, `ub.Color.hex("#2b6cb0")`, `ub.Color.rgb(...)`; the theme's palette is used when `color=` is left out. Color-mapped marks (`heatmap`, `mesh`, `contour`, `hist2d`, `hexbin`, and `scatter`/`bubbles` with `color=ub.ColorBy(values)`) share one color scale per axes: set `cmap=ub.Cmap.VIRIDIS` (or `MAGMA`, `CIVIDIS`, `BLUE_RED`, `ub.Cmap.from_colors([...])`); either `scale=ub.Scale.log()` for positive values or `center=0.0` (with the default linear scale) for a diverging map, not both; and `ax.clim(low, high)` for a fixed range. For `scatter` and `bubbles` these options go inside `ub.ColorBy(values, cmap=..., scale=..., center=...)`. `ax.colorbar("Label")` shows it.

```python
values = np.arange(12.0).reshape(3, 4) - 5.0
ax = ub.Figure(6, 3).add_axes(x=ub.Scale.category(), y=ub.Scale.category())
ax.heatmap(["a", "b", "c", "d"], ["x", "y", "z"], values, center=0.0, annotate=True)
ax.colorbar("Change")
svg = ax.figure.to_svg()
```

## Ticks and number formats

`xticks([...])`, `xticks_labeled([...], [...])`, `xlim(low, high)`, and `xtick_format(ub.TickFormat.percent(1))` (also `fixed(decimals)`, `scientific()`, `iso()` for times, and `.affix(prefix=, suffix=)`). The y methods mirror them.

## Text sizes

Titles, subtitles, axis labels, legends, colorbar titles, `text`, `annotate`, bar value labels and pie labels take `size=`: `ub.Size.AUTO` (the theme's size), `ub.Size.times(1.5)` (relative to the theme, following its font scale) or `ub.Size.pt(14)` (fixed points). Tick labels are sized per axis with `ax.xtick_labels(size=...)` and `ax.ytick_labels(size=...)`. Heatmap and contour value labels use the theme's tick label size.

```python
ax = ub.Figure(6, 4).add_axes().line([0, 1], [0.0, 1.0])
ax.title("Headline", size=ub.Size.pt(18)).xlabel("x", size=ub.Size.times(1.2))
ax.ytick_labels(size=ub.Size.pt(7))
svg = ax.figure.to_svg()
```

## Themes and fonts

`ub.Figure(..., theme=ub.Theme.PIGMENT | GRAPHITE | MIDNIGHT)`. Themes are values: derive one with `with_*` methods, for example `ub.Theme.GRAPHITE.with_font_scale(1.2)`. Fonts: the bundled `ub.Font.SOURCE_SERIF_4`, `IBM_PLEX_SANS`, `GEIST`, `INTER`, `JETBRAINS_MONO`, a file with `ub.Font.from_file("Body.ttf", bold="Body-Bold.ttf")`, or an installed family with `ub.Font.system("Times New Roman")`. Titles and direct legend labels are bold: a font without a bold face raises `StyleError` when such text is drawn, unless it was created with `bold_fallback=True` (bold text then uses the regular face).

```python
theme = ub.Theme.PIGMENT.with_title_font(ub.Font.SOURCE_SERIF_4).with_body_font(ub.Font.INTER)
ax = ub.Figure(6, 4, theme=theme).add_axes().line([0, 1, 2], [1.0, 0.5, 2.0])
svg = ax.figure.title("Serif title").to_svg()
```

## Saving

A figure or axes at the end of a Jupyter cell displays its whole figure as a PNG; `fig.show()` or `ax.show()` displays it explicitly in Jupyter or opens a temporary PNG in the system's default viewer elsewhere.

`fig.save_png(path, dpi=200)`, `fig.save_svg(path)` (text as outlines by default; `text=ub.SvgText.EDITABLE` keeps real text, and the viewer then needs the font), `fig.save_pdf(path)` (vector; fonts are embedded as subsets so text stays selectable, and text whose font cannot be embedded is drawn as outlines). `to_png()`, `to_svg()` and `to_pdf()` return the bytes or text instead.

```python
fig = ub.Figure(4, 3)
fig.add_axes().scatter([1, 2, 3], [2.0, 1.0, 3.0])
pdf = fig.to_pdf()
assert pdf.startswith(b"%PDF-")
```

For a multi-page PDF, one figure per page at its own size, use `ub.save_pdf(path, [fig1, fig2])` or `ub.to_pdf([fig1, fig2])` instead of merging single-page files with another tool. Fonts shared by pages are embedded once, and no file is written if any figure fails to render.

```python
other = ub.Figure(3, 5)
other.add_axes().line([1, 2, 3], [2.0, 1.0, 3.0])
assert ub.to_pdf([fig, other]).startswith(b"%PDF-")
```
