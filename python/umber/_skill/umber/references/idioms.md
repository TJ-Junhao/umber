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

`add_axes()` returns `Axes[float, float]`. `add_axes(x=ub.Scale.category())` returns `Axes[str, float]`, `y=ub.Scale.category()` gives `Axes[float, str]`, `x=ub.Scale.time()` gives `Axes[ub.Time, float]`. Positions and limits use those types: `xlim(low, high)`, `text("…", (x, y))` and `annotate` take values of the axis type — a category name on a category axis, a `datetime` on a time axis. Methods that need a certain kind of axes are typed to it, so pyright and mypy reject, for example, `bar` on two numeric axes before the code runs.

```python
ax = ub.Figure(6, 4).add_axes(x=ub.Scale.category())
ax.bar(["Mon", "Tue", "Wed"], [3.0, 5.0, 4.0]).ylim(0.0, 6.0)
ax.text("busy", ("Tue", 5.5), align=ub.Align.CENTER)
svg = ax.figure.to_svg()
```

## AUTO, never None

Optional arguments default to `ub.AUTO` (or a typed `AUTO` member such as `ub.Color.AUTO`, `ub.Size.AUTO`, `ub.TickFormat.AUTO`). Passing `None` is a `TypeError`. Leave an argument out, or pass `ub.AUTO`, to let umber choose.

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

Marks with a non-empty `label=` appear in the legend. `ax.legend(ub.LegendStyle.HEADER | CHIPS | DIRECT | NONE, position=ub.LegendPosition.TOP_RIGHT | ...)`. In a grid the panels share one legend (`ub.Legend.MERGED`, the default); pass `legend=ub.Legend.PER_PANEL` to `Figure` for one per panel.

## Colors, colormaps and color scales

`ub.Color.RED`, `ub.Color.hex("#2b6cb0")`, `ub.Color.rgb(...)`; the theme's palette is used when `color=` is left out. Color-mapped marks (`heatmap`, `mesh`, `contour`, `hist2d`, `hexbin`, and `scatter`/`bubbles` with `color=ub.ColorBy(values)`) share one color scale per axes: set `cmap=ub.Cmap.VIRIDIS` (or `MAGMA`, `CIVIDIS`, `BLUE_RED`, `ub.Cmap.from_colors([...])`), `scale=ub.Scale.log()`, `center=0.0` for a diverging map, and `ax.clim(low, high)` for a fixed range. `ax.colorbar("Label")` shows it.

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

Every call that writes text takes `size=`: `ub.Size.AUTO` (the theme's size), `ub.Size.times(1.5)` (relative to the theme, following its font scale) or `ub.Size.pt(14)` (fixed points). Tick labels are sized per axis with `ax.xtick_labels(size=...)` and `ax.ytick_labels(size=...)`.

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

`fig.save_png(path, dpi=200)`, `fig.save_svg(path)` (text as outlines by default; `text=ub.SvgText.EDITABLE` keeps real text, and the viewer then needs the font), `fig.save_pdf(path)` (vector, with the fonts embedded as subsets so text is selectable). `to_png()`, `to_svg()` and `to_pdf()` return the bytes or text instead.

```python
fig = ub.Figure(4, 3)
fig.add_axes().scatter([1, 2, 3], [2.0, 1.0, 3.0])
pdf = fig.to_pdf()
assert pdf.startswith(b"%PDF-")
```
