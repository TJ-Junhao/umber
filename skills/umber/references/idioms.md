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

## Physical space

For physical space, where a circle must stay a circle, use `ax.equal_aspect()`; do not tune the figure size. One data unit is then equally long on both axes: the limits and the figure stay as they are, except that automatic hexbin limits grow to hold whole cells, and the plot shrinks along one side and is centred in its room. Ticks, labels, the colorbar and `InPlot` positions follow the smaller plot. Both axes must be linear and belong to this panel alone: no `twin_y()`, and no axis shared with another panel.

```python
angle = np.linspace(0, 2 * np.pi, 200)
ax = ub.Figure(8, 4).add_axes().equal_aspect()
ax.line(np.cos(angle), np.sin(angle)).xlabel("x (m)").ylabel("y (m)")
svg = ax.figure.to_svg()
```

## Vector fields

For arrows from a vector field use `ax.quiver(x, y, u, v)`. `x` and `y` are always one-dimensional, and the shape of `u` says which form is meant: two-dimensional `u` and `v` are a grid shaped `(len(y), len(x))`, as `contour()` takes it, and one-dimensional ones are points. Pass the coordinates, not the arrays of `np.meshgrid()`. NaN in `u` or `v` leaves the arrow out; so does a NaN coordinate of points, while the coordinates of a grid are finite and strictly increasing.

```python
x, y = np.linspace(-3, 3, 25), np.linspace(-2, 2, 17)
u, v = -y[:, None] + 0 * x, x[None, :] + 0 * y[:, None]
ax = ub.Figure().add_axes().equal_aspect()
ax.quiver(x, y, u, v, key=(2, "2 m/s")).xlabel("x (km)").ylabel("y (km)")
svg = ax.figure.to_svg()
```

- **The key.** `key=(10, "10 m/s")` adds a legend entry: an arrow at the true length of a magnitude of 10, then the text you write. Do not draw a reference arrow by hand: the key is exact because it uses the mark's own scale, and it follows the legend in every position and style. There is no `label=`.
- **Length.** Lengths are exactly proportional to the magnitudes. By default the longest arrow is as long as the typical spacing of the samples on the final plot, and at most three quarters of its shorter side, so arrows rarely cross. `pt_per_unit=3` draws a magnitude of 10 as 30 pt: pass the same number to compare several axes. The arrows of one axes share one automatic scale.
- **Thinning.** umber never thins arrows: `ax.quiver(x[::4], y[::4], u[::4, ::4], v[::4, ::4])` is the way to draw fewer.
- **Direction.** By default `(u, v)` is measured on the screen: `u == v` is always 45°. `direction=ub.Direction.DATA` points each arrow from `(x, y)` towards `(x + u, y + v)` on the plot, so it follows a streamline on any plot shape; both axes must be linear. On `equal_aspect()` axes both draw the same arrows, so spatial data usually wants `equal_aspect()`.
- **Color.** `color=ub.ColorBy(np.hypot(u, v))` colors arrows by a value with the shape of `u`, on the color scale of the axes. Overlapping mapped arrows are not drawn in data order.

For wind, use `ax.barbs(x, y, u, v)` with the same inputs. `(u, v)` is where the wind blows to, and the staff runs against it from the sample. The speed is rounded to the nearest 5: a flag is 50, a full line 10, a half line 5, and a calm a small circle. umber converts no units, so pass knots if that is what the barbs should read. `flip=True` puts lines and flags on the left, for the southern hemisphere, and `size=` sets the staff length. Barbs take an ordinary `label=`. A time-height section is `ax.barbs(times, heights, u, v)` on `x=ub.Scale.time()`.

### Streamlines

For lines that follow a field on a grid use `ax.streamplot(x, y, u, v)`, with `u` and `v` shaped `(len(y), len(x))` as for `contour()`. It takes the grid form only, on linear or log axes. A NaN in `u` or `v` makes the four cells around it a hole that lines end at, so mask an obstacle with NaN rather than drawing over it.

```python
x, y = np.linspace(-3, 3, 121), np.linspace(-2, 2, 81)
gx, gy = np.meshgrid(x, y)
r2 = gx**2 + gy**2
with np.errstate(divide="ignore", invalid="ignore"):
    u = np.where(r2 < 1, np.nan, 1 - (gx**2 - gy**2) / r2**2)
    v = np.where(r2 < 1, np.nan, -2 * gx * gy / r2**2)
ax = ub.Figure().add_axes().streamplot(x, y, u, v, color=ub.ColorBy(np.hypot(u, v)))
ax.colorbar("Speed").equal_aspect()
svg = ax.figure.to_svg()
```

- **Density.** `density=1` keeps automatic lines about a thirtieth of the grid apart along each axis, measured on the axis scale; `density=2` draws them twice as close. The lines depend on the data alone, never on the figure size, and the same input draws the same lines everywhere.
- **Starts.** `starts=(xs, ys)` draws exactly one line through each point instead of the automatic lines, ignoring their spacing. A point outside the grid, in a hole or where the field is zero raises `ub.DataError` naming it.
- **Color.** `color=ub.ColorBy(np.hypot(u, v))` colors lines by speed, or by any value with the shape of `u`, interpolated along each line; mapped lines take no `label=`. One color takes a `label=` with a line swatch.
- **Arrows.** Each line carries one head at its middle, pointing downstream; `arrows=False` leaves them out.
- **quiver or streamplot.** Use `quiver()` when the magnitude at each sample matters, since arrow lengths are exact, and for scattered points; use `streamplot()` to show the paths of the flow, its eddies and where it parts.

## Triangle meshes

For values at scattered points, such as weather stations or the nodes of a finite-element mesh, join the points into triangles: `ax.tripcolor(x, y, values)` fills each triangle with one color, `ax.tricontour(x, y, values)` draws contours of the values, and `ax.triplot(x, y)` draws the edges. All three triangulate the points with `ub.delaunay(x, y)` unless `triangles=` gives the triangles. Do not grid scattered data first to draw it with `mesh()` or `contour()`. All need linear or log axes; automatic limits are the extent of the points, without padding.

```python
rng = np.random.default_rng(0)
radius, angle = np.sqrt(rng.uniform(0.16, 1, 400)), rng.uniform(0, 2 * np.pi, 400)
x, y = radius * np.cos(angle), radius * np.sin(angle)
tri = ub.delaunay(x, y)
keep = np.hypot(x[tri].mean(axis=1), y[tri].mean(axis=1)) > 0.4
ax = ub.Figure().add_axes().tripcolor(x, y, x**2 - y, triangles=tri[keep])
ax.triplot(x, y, triangles=tri[keep]).colorbar("Pressure").equal_aspect()
svg = ax.figure.to_svg()
```

- **A domain with a hole.** `ub.delaunay()` triangulates the convex hull of the points, so it bridges holes and concave edges. It returns an ordinary `int64` array shaped `(n, 3)`: filter its rows with NumPy, as above, and pass the rest as `triangles=`. There is no `Triangulation` object and no `mask=`.
- **Given triangles.** `triangles=` takes an integer array shaped `(n, 3)` of indices into the points, in any order and with either winding. A float array raises `TypeError`; an index out of range, a repeated corner, a triangle without area, a triangle given twice or an edge of three triangles raise `ub.DataError` naming the rows.
- **Finite-element results.** `on=ub.TriValues.TRIANGLES` takes one value per triangle, in the order of the rows of `triangles` (or of `ub.delaunay(x, y)`); the default `ub.TriValues.POINTS` takes one value per point, and a triangle takes the mean of its corners. The number of values is checked against `on`, never used to guess it. NaN leaves a triangle blank; with `POINTS` one NaN corner is enough.
- **Colors.** `tripcolor()` shares the color scale of the axes with `mesh()` and `contour()`, so it takes `cmap=`, `scale=` and `center=` directly, and `ax.colorbar()` explains it. The triangles are flat, one color each.
- **Edges.** `triplot()` draws each edge once at half the automatic line width and no points; add `ax.scatter(x, y)` for those. `label=` gives a line swatch.
- **Units.** `delaunay()` works on the raw coordinates. For x and y in very different units, or points shown on a log axis, triangulate scaled coordinates, such as `ub.delaunay(np.log10(x), y)`, and pass the result as `triangles=`.
- **mesh, contour or tripcolor.** Use `mesh()` for values on a rectangular grid of cells and `contour()` for a field sampled on a grid; use `tripcolor()` or `tricontour()` when the points are scattered or the mesh is made of triangles.

### Contours of scattered data

`ax.tricontour(x, y, z)` takes one value per point and the arguments of `contour()` with the same meanings: `levels=`, `style=ub.Contour.LINES`, `FILLED` or `BOTH`, `cmap=`, `scale=`, `center=`, `color=`, `annotate=` and `format=`. Within each triangle the field is linear between its corners. There is no `tricontourf`: pass `style=ub.Contour.FILLED`.

```python
rng = np.random.default_rng(1)
x, y = rng.uniform(-2, 2, 500), rng.uniform(-2, 2, 500)
z = np.exp(-(x**2) - y**2) + 0.2 * x
ax = ub.Figure().add_axes().tricontour(x, y, z, annotate=True)
tri = ub.delaunay(x, y)
keep = np.hypot(x[tri].mean(axis=1), y[tri].mean(axis=1)) > 0.6
holed = ub.Figure().add_axes().tricontour(x, y, z, triangles=tri[keep], style=ub.Contour.FILLED)
holed.colorbar("Height").equal_aspect()
svg = holed.figure.to_svg()
```

- **A mesh with a hole.** Filter the rows of `ub.delaunay(x, y)` and pass the rest as `triangles=`, as for `tripcolor()`. Lines end on the edge of the mesh and of each hole, and bands leave holes empty. A NaN in `z` removes every triangle around its point, which also makes a hole; infinite values raise `ub.DataError`.
- **Smooth or flat.** `style=ub.Contour.FILLED` is the smooth alternative to `tripcolor()`: its bands follow the field across the triangles, where `tripcolor()` gives each triangle one color. Use `tripcolor()` when the value of each element is what matters, as for finite-element results with `on=ub.TriValues.TRIANGLES`.
- **Levels on the values.** A point is above a level when its value is at or above it, so integer data often puts levels exactly on points. A triangle whose corners all lie on a level is in the band above it; an edge or a point on the level with lower values all around draws no line, since it bounds no area. Lines lie exactly where two bands meet.
- **Log axes.** On a log axis the field is linear along each edge as drawn, so the lines meet the edges where `triplot()` draws them.
- **contour or tricontour.** For values on a rectangular grid, use `contour(x, y, z)` with `z` shaped `(len(y), len(x))`: it needs no triangles. Contours on grids and meshes that share a colorbar need the same `levels`.

## Values next to points

Label points with `annotate=True`; do not place value text by hand with `ax.text()` at data coordinates. Each label follows its point when the data change, takes the series color (darkened or lightened just enough to read at 4.5:1 against the background), and with the default `place=ub.Place.AUTO` sits on the first side of above, right, below and left that stays inside the plot and clear of markers, error bars, lines, bars, areas and other value labels. A fixed side (`ub.Place.ABOVE`, `BELOW`, `LEFT`, `RIGHT`) never moves, and automatic limits grow so that it fits. `format=` sets how values are written (`AUTO` follows the value axis's tick format) and `label_size=` their size; `size=` stays the marker size.

```python
models = ["Baseline", "Gate", "Gate + aug"]
ax = ub.Figure(6, 3).add_axes(y=ub.Scale.category())
ax.scatter(
    [66.5, 71.6, 73.1],
    models,
    label="80 epochs",
    x_error=([65.8, 70.9, 72.4], [67.2, 72.3, 73.7]),
    annotate=True,
    format=ub.TickFormat.fixed(1),
)
svg = ax.figure.to_svg()
```

When no side inside the plot is clear, `AUTO` lets automatic limits grow to hold the label, such as above the highest point of a line, so leave the limits automatic rather than setting `ylim()` to make room. With explicit limits a point with no clear side inside the plot raises `StyleError` when the figure is rendered: fix the side with `place=`, widen the limits, or use a larger figure. Category axes never grow. Text from `ax.text()` and `ax.annotate()` is not avoided.

For a table of values beside a forest plot, use `place=ub.Place.COLUMN` and `ax.value_column(title=...)`; do not write the values with `ax.text()` at fixed coordinates. With a category y axis the values form a column right of the plot, each level with its point; with values on y they form a row above the plot, each centred on its point. Each scatter, and each group with `by=`, gets its own sub-column in call order, in its series color (moved just enough to read at 4.5:1 against the background), so two series on the same categories never collide; the legend names the colors. The layout reserves the column's room, so the plot narrows to make room and the limits do not change. Values in one notation are aligned on the decimal point.

```python
models = ["Baseline", "Gate", "Gate + aug"]
ax = ub.Figure(6, 3).add_axes(y=ub.Scale.category())
for label, means in [("80 epochs", [66.5, 71.6, 73.1]), ("Long training", [68.1, 74.0, 75.4])]:
    ax.scatter(
        means,
        models,
        label=label,
        annotate=True,
        format=ub.TickFormat.fixed(1),
        place=ub.Place.COLUMN,
    )
ax.value_column(title="Mean (%)")
svg = ax.figure.to_svg()
```

## Error bars and lines that belong to a series

For a series' error bars and connecting line use `scatter(..., y_error=(low, high), connect=True)` (or `x_error=` for horizontal bars), which take the series color at half the line width of a standalone line, so they stay lighter than the points; use `y_error_bars()` and `line()` only for marks that stand alone. Separate calls take their own palette colors and legend entries, so they do not match the points and they shift the colors of later series. `low` and `high` hold one value per point; NaN skips a point's bar, which puts a bar on one point only, such as a "Mean" after the folds. A point left out, for a NaN coordinate or a NaN `ColorBy` value, has no marker, label or bar, and breaks the line. With `by=`, each group gets its own bars and line in its color.

```python
nan = float("nan")
folds = ["Fold 1", "Fold 2", "Fold 3", "Mean"]
ax = ub.Figure(6, 3).add_axes(x=ub.Scale.category())
ax.scatter(
    folds,
    [66.8, 68.1, 65.4, 66.77],
    label="80 epochs",
    y_error=([nan, nan, nan, 65.6], [nan, nan, nan, 67.9]),
    connect=True,
    annotate=True,
)
svg = ax.figure.to_svg()
```

## Colors, colormaps and color scales

`ub.Color.RED`, `ub.Color.hex("#2b6cb0")`, `ub.Color.rgb(...)`; the theme's palette is used when `color=` is left out. Color-mapped marks (`heatmap`, `mesh`, `contour`, `tripcolor`, `tricontour`, `hist2d`, `hexbin`, and `scatter`, `bubbles`, `quiver`, `barbs` and `streamplot` with `color=ub.ColorBy(values)`) share one color scale per axes: set `cmap=ub.Cmap.VIRIDIS` (or `MAGMA`, `CIVIDIS`, `BLUE_RED`, `ub.Cmap.from_colors([...])`); either `scale=ub.Scale.log()` for positive values or `center=0.0` (with the default linear scale) for a diverging map, not both; and `ax.clim(low, high)` for a fixed range. For `scatter`, `bubbles`, `quiver`, `barbs` and `streamplot` these options go inside `ub.ColorBy(values, cmap=..., scale=..., center=...)`. `ax.colorbar("Label")` shows it.

```python
values = np.arange(12.0).reshape(3, 4) - 5.0
ax = ub.Figure(6, 3).add_axes(x=ub.Scale.category(), y=ub.Scale.category())
ax.heatmap(["a", "b", "c", "d"], ["x", "y", "z"], values, center=0.0, annotate=True)
ax.colorbar("Change")
svg = ax.figure.to_svg()
```

## Empty cells and a marked group in a heatmap

To leave zero cells blank, pass NaN: a NaN cell is not drawn and gets no value. To mark a group, such as a group of classes in a confusion matrix, use `xtick_highlight()` and `ytick_highlight()`, which draw the group's labels bold in the theme's emphasis color, and `outline=` with a boolean mask of the shape of `values`, which draws a rectangle inside each `True` cell in the same color. Do not draw circles or boxes over cells with `scatter()` or `text()` at computed positions. Which cells to outline is one line of NumPy; umber does not infer it. `edges=True` draws thin lines between the cells, which helps to follow a row of a large, sparse matrix. A highlighted label must be drawn: on a crowded axis that labels only every k-th category, rendering raises `StyleError`, so make the figure large enough for every label.

```python
names = ["bird", "dog", "drill", "saw"]
counts = np.array([[9, 1, 0, 0], [2, 7, 0, 1], [0, 0, 6, 3], [0, 1, 2, 8]])
group = ["drill", "saw"]
in_group = np.isin(names, group)
mask = in_group[:, None] & in_group[None, :] & ~np.eye(4, dtype=bool) & (counts > 0)
ax = ub.Figure(5, 4).add_axes(x=ub.Scale.category(), y=ub.Scale.category())
values = np.where(counts > 0, counts, np.nan)
ax.heatmap(names, names, values, annotate=True, outline=mask, edges=True)
ax.xtick_highlight(group).ytick_highlight(group)
svg = ax.figure.to_svg()
```

## Ticks and number formats

`xticks([...])`, `xticks_labeled([...], [...])`, `xlim(low, high)`, and `xtick_format(ub.TickFormat.percent(1))` (also `fixed(decimals)`, `scientific()`, `iso()` for times, and `.affix(prefix=, suffix=)`). The y methods mirror them.

## Text sizes

Titles, subtitles, axis labels, legends, colorbar titles, `text`, `annotate`, bar value labels and pie labels take `size=`: `ub.Size.AUTO` (the theme's size), `ub.Size.times(1.5)` (relative to the theme, following its font scale) or `ub.Size.pt(14)` (fixed points). Tick labels are sized per axis with `ax.xtick_labels(size=...)` and `ax.ytick_labels(size=...)`. Heatmap and contour value labels use the theme's tick label size. Contour level labels place and color themselves: each sits at the middle of its line or at the nearest place along it that lies wholly inside the plot area and the field, clear of NaN holes and filtered triangles, and on the bands of `ub.Contour.BOTH` it takes the theme's text or background color, whichever reads better on the bands painted under it; a line with no such place has no label, so do not draw level values with `ax.text()`.

```python
ax = ub.Figure(6, 4).add_axes().line([0, 1], [0.0, 1.0])
ax.title("Headline", size=ub.Size.pt(18)).xlabel("x", size=ub.Size.times(1.2))
ax.ytick_labels(size=ub.Size.pt(7))
svg = ax.figure.to_svg()
```

## Themes and fonts

`ub.Figure(..., theme=ub.Theme.PIGMENT | GRAPHITE | MIDNIGHT)`. Themes are values: derive one with `with_*` methods, for example `ub.Theme.GRAPHITE.with_font_scale(1.2)`. Fonts: the bundled `ub.Font.SOURCE_SERIF_4`, `IBM_PLEX_SANS`, `GEIST`, `INTER`, `JETBRAINS_MONO`, a file with `ub.Font.from_file("Body.ttf", bold="Body-Bold.ttf")`, or an installed family with `ub.Font.system("Times New Roman")`. Titles and direct legend labels are bold: a font without a bold face raises `StyleError` when such text is drawn, unless it was created with `bold_fallback=True` (bold text then uses the regular face). A variable font file is drawn in its named instances: `ub.Font.from_file("Body[wght].ttf")` alone gives regular text the instance closest to weight 400 and bold text the instance closest to 600 among those of weight 600 or more, and `Font.system` does the same for a family installed only as variable files.

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

A PDF or SVG for a paper with a dense scatter (tens of thousands of points or more), a long noisy line, or a tripcolor of many triangles opens slowly, because every point is a shape the viewer draws. Pass `render=ub.Render.RASTER` to that `scatter()`, `line()` or `tripcolor()` call: SVG and PDF output then draw the mark as one image, at the export's `raster_dpi` (300 by default, 10 to 2400), while the axes, text, legend, colorbar, the other marks and the mark's own value labels and error bars stay vector. PNG output does not change. Raise `raster_dpi` for print (`fig.save_pdf(path, raster_dpi=600)`); it changes nothing in a figure without such a mark. Raster marks that follow each other in one axes share one image, so 200 lines drawn with `RASTER` are one image, not 200; a vector mark drawn between two of them keeps them two images. Leave sparse marks vector: the default `render=ub.Render.VECTOR` stays sharp at any zoom, and there is no automatic switch.

```python
import numpy as np

rng = np.random.default_rng(0)
x, y = rng.normal(size=50_000), rng.normal(size=50_000)
dense = ub.Figure().add_axes().scatter(x, y, render=ub.Render.RASTER).line([-2, 2], [-2, 2])
pdf = dense.figure.to_pdf(raster_dpi=300)
assert pdf.count(b"/Subtype /Image") == 2  # the image of the points and its transparency
```
