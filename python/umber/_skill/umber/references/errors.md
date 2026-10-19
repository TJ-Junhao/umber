# Errors

umber checks every call when it is made and raises at once. Each error message reads `Context(): what is wrong; what is required.`; the hint and the highlighted argument are attached as exception notes, which Python and Jupyter print under the traceback. Errors are never silent and a failed call changes nothing, so the fix is always to change the call or the data, never to catch and ignore the error.

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
- **Line breaks and control characters.** `"\n"` is allowed only in titles, subtitles, `text` and `annotate`; elsewhere, and for `\t`, `\r` and other control characters anywhere, the call raises. Split the text into a title and a subtitle, or use spaces.
- **Pie slices.** More slices than palette colors need `colors=`; small slices whose labels cannot be placed should be merged into "Other" or shown with `legend(ub.LegendStyle.HEADER)`.
- **Fonts without bold.** Titles and direct legend labels are bold. Pass `bold=` to `ub.Font.from_file`, choose a family with a bold face, or create the font with `bold_fallback=True`.
- **`by=` with `color=` or `label=`.** Each group already takes the next palette color and its name as the label; drop those arguments.
