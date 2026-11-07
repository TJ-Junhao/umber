# Errors

umber checks every call when it is made and raises at once. Each error message reads `Context(): what is wrong; what is required.`; the hint and the highlighted argument are attached as exception notes, which Python and Jupyter print under the traceback. Errors are never silent and a failed call changes nothing, so the fix is to change the call or the data rather than to catch and ignore the error. The exception is plotting inside a long-running job, below.

| Class | Meaning | Typical fix |
|---|---|---|
| `TypeError` | An argument has the wrong type: a string for a `Color`, a float for a `Size`, `None` where a typed value is required. (`None` or NaN inside data or a `by=` column is a `DataError`.) Type checkers (pyright, mypy) also reject marks on the wrong kind of axes before the code runs. | Pass the typed value the signature names: `ub.Color.hex("#…")`, `ub.Size.pt(2)`, an enum member, `ub.AUTO`. |
| `ub.UsageError` | The API is used in a way it does not support: conflicting options, `by=` together with `label=`, a Cartesian method on a pie panel, a missing font file. | Read the hint; it names the supported way. |
| `ub.DataError` | The data cannot be drawn as asked: NaN or infinite values where they matter, lengths that differ, zero or negative values on a log axis, category names on an axis that is not a category axis, duplicate categories, a file that is not a font. | Clean or reshape the data, or choose another axis scale. |
| `ub.StyleError` | The figure cannot be laid out or styled as asked: labels that do not fit, too many slices for the palette, a font without the needed bold face. Raised when the figure is rendered if it depends on the final layout. | Make the figure larger, use fewer or shorter labels, pass explicit colors, or change the font. |

All three umber classes derive from `ub.UmberError`; `DataError` and `StyleError` are also `ValueError`s.

```python
import umber as ub

try:
    ub.line([0.0, 1.0], [1.0, float("inf")])
except ub.DataError as err:
    assert "values must be finite" in str(err)
```

```python
ax = ub.Figure(6, 4).add_axes(y=ub.Scale.log())
try:
    ax.line([0, 1, 2], [1.0, 0.0, 10.0])
except ub.DataError as err:
    assert "values must be positive" in str(err)
```

```python
ax = ub.Figure(6, 4).add_axes()
try:
    ax.bar(["a", "b"], [1.0, 2.0])
except ub.DataError as err:
    assert "not a category axis" in str(err)
```

Common cases:

- **"the x axis is not a category axis".** Create the axes with `add_axes(x=ub.Scale.category())` (or `y=` for horizontal bars) before calling `bar`, `box`, `violin`, `strip`, `swarm`; `heatmap` needs both axes to be category axes.
- **Values that must be finite.** Drop or fill NaN and infinite values first, unless the method documents NaN as a gap (lines and areas break at NaN; points with NaN are left out).
- **Log axes.** Every value on a log axis must be positive.
- **Labels or legends that do not fit.** The error names the text and the room it needs, titles and axis labels included; make the figure larger, shorten the text, use a smaller `size=` or font scale, or move the legend.
- **Line breaks and control characters.** `"\n"` is allowed only in titles, subtitles, captions, `text` and `annotate`; elsewhere, and for `\t`, `\r` and other control characters anywhere, the call raises. Split the text into a title and a subtitle, or use spaces.
- **Pie slices.** More slices than palette colors need `colors=`; small slices whose labels cannot be placed should be merged into "Other" or shown with `legend(ub.LegendStyle.HEADER)`.
- **Fonts without bold.** Titles and direct legend labels are bold. Pass `bold=` to `ub.Font.from_file`, choose a family with a bold face, or create the font with `bold_fallback=True`.
- **Highlights and outlines.** `xtick_highlight()` and `ytick_highlight()` raise `ub.UsageError` on a linear, log or time axis: only category labels can be highlighted. `heatmap(outline_color=...)` without `outline=` raises `ub.UsageError`; an `outline=` mask of another shape than `values` raises `ub.DataError` naming both shapes, and one that is not boolean raises `TypeError`.
- **Equal aspect.** `equal_aspect()` raises `ub.UsageError` naming the axis on a log, time or category axis, and on an axis shared with another panel, because a shared axis is sized by its whole row or column. It raises `ub.UsageError` on a twin and on axes that have one, and `twin_y()` raises it on axes with equal aspect, because a twin y axis has its own scale. Use two linear axes of the panel's own, and draw a second y scale on a panel of its own.
- **`by=` with `color=` or `label=`.** Each group already takes the next palette color and its name as the label; drop those arguments.

## Errors raised when the figure is rendered

These errors cannot be found when the mark is added, because they depend on the final layout, so they are raised by `to_svg()`, `to_png()`, `to_pdf()`, `show()` or a notebook display. In a long job, render a preview early, with the same data shapes, label lengths, figure size and formats as the final figure.

| Class | Raised when | Change |
|---|---|---|
| `ub.StyleError` | A fixed or percent tick format cannot label any value in the axis or colorbar range, such as `TickFormat.fixed(0)` on values between 0.1 and 0.2. | Use more decimals, or set the ticks explicitly. |
| `ub.StyleError` | A title, subtitle, caption, axis label, tick label, legend label or colorbar does not fit its room, or the figure is too small for the plot area. The message names the text and both widths. | Make the figure larger, shorten the text, or use a smaller `size=`. |
| `ub.StyleError` | Bubbles do not fit the plot space: a marker is as long as the plot. The message names a figure size that holds them. | Make the figure at least that large. |
| `ub.StyleError` | Bubbles need limits beyond the supported numeric range. A larger figure does not help. | Set explicit `xlim()` and `ylim()`, which clip the bubbles. |
| `ub.StyleError` | `text()` or `annotate()` is placed outside the axes limits, or an annotation reaches past the edge of the figure or has no room for its arrow. | Include the point in `xlim()` and `ylim()`, or change `offset=` and `align=`, or pass `arrow=False`. |
| `ub.DataError` | `vline()`, `text()` or `annotate()` names a category that no mark on the axes has. | Draw a mark with that category first. |
| `ub.StyleError` | The values written on bars (`annotate=True`) or heatmap cells do not fit inside the plot or the cell. | Make the figure larger, write fewer decimals with `format=`, or pass `annotate=False`. |
| `ub.StyleError` | A value written next to a scatter point (`scatter(annotate=True)`) has no clear position with `place=ub.Place.AUTO`: above, right, below and left each overlap a mark or another value label, or leave the plot across explicit limits or a category axis (automatic limits grow to make room instead). Or the labels do not settle: their sides keep changing as the limits grow. The message names the series (its label, or `scatter 2` for the second scatter of the axes) and the value. On a fixed side, the label does not fit inside the plot. | Fix the side with `place=ub.Place.ABOVE` (or another side), make the figure larger, or use a smaller `label_size=`; under explicit limits, widen `xlim()` or `ylim()`, or leave them automatic. |
| `ub.DataError` | `xtick_highlight()` or `ytick_highlight()` names a category that no mark on the axis has. The message names it. | Pass the category names exactly as the marks give them. |
| `ub.StyleError` | A category highlighted with `xtick_highlight()` or `ytick_highlight()` is among the labels a crowded axis leaves out: automatic category labels show every k-th category when they do not all fit, and a highlight on a label that is not drawn would not show. | Make the figure larger, or set a smaller tick label size with `xtick_labels(size=...)` or `ytick_labels(size=...)`. |
| `ub.UsageError` | `ax.value_column()` titles a column, but no scatter on those axes writes its values there; raised when the figure is rendered. | Pass `annotate=True, place=ub.Place.COLUMN` to the scatter whose values the column shows, or remove the `value_column()` call. |
| `ub.StyleError` | Two values in one sub-column of `place=ub.Place.COLUMN` overlap, because two points of the series lie too close along the category axis (or along x, for a row above the plot); raised when the figure is rendered. The message names the series and both values. Or a value of a row above the plot, centred on a point near the plot's edge, reaches outside the figure (outside its panel on a grid), or comes closer than 6 pt to the figure's left or right edge, as x tick labels may not. | Make the figure taller (wider for a row), pass a smaller `label_size=`, or use another `place=`; the limits never change to make room. |
| `ub.UsageError` | An axis with `equal_aspect()` is shared with a panel added after the call. The message names the axis and, on a grid, the cell. | Stop sharing that axis, or remove `equal_aspect()`. |
| `ub.StyleError` | `equal_aspect()` leaves the plot less than 1 pt on a side, because the x and y ranges are too far apart. The message names both ranges. | Bring the ranges closer together, or remove `equal_aspect()`. |
| `ub.StyleError` | The y axis label is wider than a plot that `equal_aspect()` made narrow. A wider figure need not help, since the limits set the plot's shape. | Shorten the label, use a smaller `size=`, or remove `equal_aspect()`. |
| `ub.StyleError` | The bubbles on axes with `equal_aspect()` do not settle: each fit of the limits changes the plot's shape again. | Make the figure larger, or set explicit limits, which clip the bubbles. |
| `ub.StyleError` | Contours that share a colorbar have different levels, on one axes or on panels that share the colorbar. | Give them identical `levels`. |
| `ub.StyleError` | `swarm()` points do not fit their category band. | Use `strip()` or `violin()` for many points. |
| `ub.StyleError` | Pie labels cannot be placed outside the circle without overlap. | Merge small slices into "Other", enlarge the figure, or use `legend(ub.LegendStyle.HEADER)` or `ub.LegendStyle.CHIPS`. |
| `ub.StyleError` | A mesh or 2-D histogram drawn with `ub.Render.RASTER` has a cell cut by explicit limits that reaches far past them, or an edge that lands more than 0.001 pt from its place in the image (for example a mesh edge at 1.0000999 with `xlim(0.98, 1.02)` at `dpi=600`). | Use `render=ub.Render.VECTOR` or `ub.AUTO`, or widen the limits. |
| `ub.DataError` | The ranges of a `hexbin()` are too far apart for one lattice of regular hexagons on these axes, or its hexagons do not settle at the plot size. | Draw them on separate axes, set explicit limits, or choose another `gridsize`. |
| `ub.StyleError` | `to_png()` gets a `dpi` outside 10 to 2400, or the pixel size is too large. | Use a dpi in range, a smaller figure or a lower dpi. |

Errors about the data itself (NaN, infinite or non-positive values, too few values for `kde()` or `ecdf()`, bin edges that leave values out) are raised by the call that adds the data, not when the figure is rendered.

## Long-running jobs

Errors that depend only on how a figure is set up (argument types, titles, sizes, legends) appear the first time the figure is drawn. A few depend on the data and can appear late in a long job: a value that becomes infinite (a diverging loss), a value of 0 or below on a log axis. In a training loop or a simulation, a plot must never end the job. Record the values while it runs and plot them afterwards, or catch `ub.UmberError` around the plotting code and log it:

```python
import logging
import math

import umber as ub

log = logging.getLogger("train")


def plot_loss(epochs: list[int], losses: list[float]) -> bytes:
    ax = ub.Figure(6, 4).add_axes().line(epochs, losses, label="loss").title("Training loss")
    return ax.figure.to_png()


for losses in ([1.0, 0.5, 0.25], [1.0, math.inf, 0.2]):
    try:
        png = plot_loss([1, 2, 3], losses)
    except ub.UmberError as err:
        log.warning("loss plot skipped: %s", err)
```

`ub.UmberError` is the base of `UsageError`, `DataError` and `StyleError`; a `TypeError` from a wrong argument type is a bug in the plotting code and is not caught by it.
