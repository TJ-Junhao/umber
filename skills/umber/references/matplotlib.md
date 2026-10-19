# Coming from matplotlib

umber has no global state: there is no `plt`, no current figure and no `show()`. Build a figure, draw on its axes, then write it out.

| matplotlib | umber |
|---|---|
| `fig, ax = plt.subplots(figsize=(6, 4))` | `fig = ub.Figure(6, 4); ax = fig.add_axes()` |
| `fig, axs = plt.subplots(2, 3, sharey=True)` | `fig = ub.Figure(9, 6, rows=2, cols=3, share_y=ub.Share.ALL)`; `fig.add_axes(row=r, col=c)` |
| `ax.plot(x, y, "r--", lw=2, label="a")` | `ax.line(x, y, color=ub.Color.RED, dash=ub.Dash.DASHED, width=ub.Size.pt(2), label="a")` |
| `ax.scatter(x, y, c=z, cmap="viridis")` | `ax.scatter(x, y, color=ub.ColorBy(z, cmap=ub.Cmap.VIRIDIS))` |
| `ax.scatter(x, y, s=sizes)` | `ax.bubbles(x, y, sizes)`: sizes are relative (the largest bubble gets an 18 pt radius), not areas in points² |
| `ax.bar(names, values)` | `ax = fig.add_axes(x=ub.Scale.category()); ax.bar(names, values)` |
| `ax.barh(names, values)` | `ax = fig.add_axes(y=ub.Scale.category()); ax.bar(names, values)` |
| `ax.bar_label(bars)` | set `annotate=True` on the `bar` (or `grouped_bar`, `stacked_bar`) call itself |
| `ax.hist(x, bins=30, density=True, histtype="step")` | `ax.hist(x, bins=30, stat=ub.Stat.DENSITY, style=ub.Hist.STEP)` |
| `ax.hist([a, b], stacked=True)` | `ax.hist(values, style=ub.Hist.STACKED, by=groups)` |
| `ax.ecdf(x)` | `ax.ecdf(x)` |
| `ax.boxplot(groups)` / `ax.violinplot(groups)` | `ax.box(names, groups)` / `ax.violin(names, groups)` on a category axis |
| `ax.eventplot(positions)` | `ax.events(rows, positions)` on a category y axis, one sequence per row; for one row `ax.events(["events"], [positions])` |
| `ax.fill_between(x, lo, hi)` | `ax.band(x, lo, hi)` |
| `ax.stackplot(x, ys, labels=...)` | `ax.stacked_area(x, [ub.Series(label, y), ...])` |
| `ax.errorbar(x, y, yerr=e)` | `ax.y_error_bars(x, y - e, y + e)` plus the line or points |
| `ax.axhline(y)` / `ax.axvspan(a, b)` | `ax.hline(y)` / `ax.vspan(a, b)` |
| `ax.imshow(m)` with labelled ticks | `ax.heatmap(x_names, y_names, m)` on two category axes |
| `ax.pcolormesh(xe, ye, m)` | `ax.mesh(xe, ye, m)` |
| `ax.contour(x, y, z)` | `ax.contour(x, y, z)` |
| `ax.contourf(x, y, z)` | `ax.contour(x, y, z, style=ub.Contour.FILLED)` |
| `ax.clabel(cs)` | set `annotate=True` on the `contour` call itself |
| `ax.hist2d(x, y)` / `ax.hexbin(x, y)` | `ax.hist2d(x, y)` / `ax.hexbin(x, y)` |
| `ax.pie(values, labels=names, autopct="%1.0f%%")` | `ub.pie(names, values, annotate=True)` |
| `ax.pie(..., wedgeprops={"width": 0.5})` | `ub.pie(names, values, hole=0.5)` |
| `fig.colorbar(im, label="v")` | `ax.colorbar("v")` (or `fig.colorbar("v")` for a shared color scale) |
| `ax.text(x, y, "s")` | `ax.text("s", (x, y))` |
| `ax.annotate("s", xy=(x, y), xytext=(0, 12), textcoords="offset points", arrowprops={...})` | `ax.annotate("s", (x, y), offset=(0.0, 12.0))` |
| `ax2 = ax.twinx()` | `ax2 = ax.twin_y()`: shares the x axis, its own y axis on the right; one legend for both |
| `ax.set_title(...)`, `set_xlabel`, `set_ylabel` | `ax.title(...)`, `xlabel`, `ylabel` (chainable) |
| `ax.legend(loc="upper right")` | `ax.legend(ub.LegendStyle.HEADER, position=ub.LegendPosition.TOP_RIGHT)` |
| `ax.set_xticks(positions, labels)` | `ax.xticks_labeled(positions, labels)` (numeric or time axes) |
| `ax.grid(True, axis="y")` | `theme=ub.Theme.PIGMENT.with_grid_lines(ub.GridLines.HORIZONTAL)` (`VERTICAL`, `BOTH`, `NONE`) |
| `ax.step(x, y, where="post")` / `ax.stem(x, y)` | `ax.step(x, y)` / `ax.stem(x, y)` |
| `ax.set_xlim(a, b)`, `set_xscale("log")` | `ax.xlim(a, b)`; the scale is chosen in `add_axes(x=ub.Scale.log())` |
| `ax.xaxis.set_major_formatter(PercentFormatter(1, decimals=n))` | `ax.xtick_format(ub.TickFormat.percent(n))`; choose the decimals (umber's default is 0) |
| `ax.title.set_fontsize(16)` | `ax.title("…", size=ub.Size.pt(16))` |
| `plt.rcParams["font.family"] = "serif"` | `theme=ub.Theme.PIGMENT.with_body_font(ub.Font.SOURCE_SERIF_4)` |
| `sns.lineplot(data=df, x="day", y="temp", hue="station", estimator=None, errorbar=None, sort=False)` | `ax.line(df["day"], df["temp"], by=df["station"])` (umber draws each group's rows as given; aggregate and sort first to match seaborn's defaults) |
| `plt.savefig("f.png", dpi=200)` / `"f.svg"` / `"f.pdf"` | `fig.save_png("f.png", dpi=200)` / `fig.save_svg("f.svg")` / `fig.save_pdf("f.pdf")` |
| `plt.show()` | none: write the figure to a file, or show its SVG in Jupyter with `IPython.display.SVG(fig.to_svg())` |

Things that work differently:

- **No guessing.** matplotlib draws something for almost any input; umber raises an error with a hint when the input does not fit the chart (a NaN where it matters, a log axis with zeros, labels that cannot fit). Fix the input or the call instead of suppressing the error.
- **Categories are an axis type,** not strings that happen to be on an axis. Create the axes with `ub.Scale.category()` before drawing bars, boxes or heatmaps.
- **Styling is a theme,** not per-call keyword soup. Change the look with `Theme.with_*` methods.
- **Dataframes are columns plus `by=`.** umber does not take `data=df` or column names.

```python
import umber as ub

names = ["Web", "App", "Shop"]
values = [50.0, 30.0, 20.0]
ax = ub.Figure(6, 4).add_axes(x=ub.Scale.category())
ax.bar(names, values, annotate=True).title("Visits by channel")
svg = ax.figure.to_svg()
```
