---
name: umber
description: "Plot data in Python with umber: line, scatter, bar, histogram, box, violin, heatmap, contour, pie and other charts, saved as PNG, SVG or PDF. Use when writing or changing code that makes charts with umber, or when turning matplotlib code into umber code."
metadata:
  umber-version: 0.4.0
---

Before using this skill, compare `metadata.umber-version` above with `umber.__version__` in the project's Python environment. If they differ, ask the user to reinstall with `python -m umber skill install --agent NAME`, choosing `claude-code`, `codex`, `cursor` or `pi` for NAME and adding `--user` if this skill was installed in the home directory.

# Writing umber code

umber is a typed plotting library: `import umber as ub`. A figure holds axes; marks are methods on an axes; every call checks its arguments at once and raises a clear error instead of guessing.

```python
import umber as ub

fig = ub.Figure(6, 4)
ax = fig.add_axes()
ax.line([1, 2, 3, 4], [3.0, 5.5, 4.0, 6.5], label="2025")
ax.line([1, 2, 3, 4], [2.5, 4.0, 4.5, 5.0], label="2026")
ax.title("Weekly signups").xlabel("Week").ylabel("Signups (k)")
png = fig.to_png()
```

Rules that matter most:

- **Pick the axis types when you create the axes.** `add_axes(x=..., y=...)` takes `ub.Scale.linear()` (the default), `ub.Scale.log()`, `ub.Scale.category()` or `ub.Scale.time()`. The scales decide which marks are allowed: `bar`, `box` and `violin` need a category axis, `heatmap` needs two. Type checkers see this: `Axes[str, float]` has a category x axis.
- **Options are keyword-only and typed.** Colors are `ub.Color` values, sizes are `ub.Size.pt(...)` or `ub.Size.times(...)`, number formats are `ub.TickFormat` values, styles are enums. There are no format strings and no `None` defaults: `ub.AUTO` means "let umber choose".
- **Calls chain and change one figure.** Each method returns its axes. A call that raises changes nothing.
- **Errors explain themselves.** umber raises `TypeError`, `ub.UsageError`, `ub.DataError` or `ub.StyleError` with a message, a hint, and the offending argument highlighted. Read the hint and fix the call; never catch and ignore these errors.
- **Output:** `fig.to_png()`, `fig.to_svg()`, `fig.to_pdf()`, or `fig.save_png(path)`, `fig.save_svg(path)`, `fig.save_pdf(path)`. Quick functions (`ub.line`, `ub.scatter`, `ub.bar`, `ub.hist`, `ub.pie`) return an axes; reach its figure with `.figure`.
- **Long-form data:** pass columns as data and group with `by=`. umber never takes a dataframe plus column names.

Further reading in this skill:

- [Choosing a chart](references/charts.md): which method draws what, with short examples.
- [Idioms](references/idioms.md): axes types, grids, legends, colors and colormaps, text sizes, themes and fonts, saving.
- [Coming from matplotlib](references/matplotlib.md): a translation table.
- [Errors](references/errors.md): what each error class means and how to fix the common ones.
- [API reference](references/api.md): every public name with its typed signature and examples, generated from this version of umber. When in doubt about a parameter, look it up there rather than guessing.

Installing for both Claude Code and Cursor gives Cursor two identical copies, because Cursor also reads `.claude/skills/`.
