# Examples

Each script is self-contained and writes its chart as a PNG next to itself. Run one with `python line.py`.

- `line.py`: two lines with labels, a title, a subtitle and axis labels.
- `subplots.py`: a 2 × 2 grid with a centered figure title and subtitle, x scales shared by column, a comparison panel spanning both rows and a merged legend.
- `scatter.py`: two point clouds with different markers and sizes.
- `bubbles.py`: cities with population encoded by marker area and reference circles showing the size scale.
- `color_by.py`: points colored by a value with `ColorBy`, on a sequential colormap and on a diverging one centered on zero with `clim()`.
- `area.py`: an area chart with the gradient fill style.
- `stacked_area.py`: renewable generation in three stacked layers, outlined over flat fills.
- `step.py`: step lines in the three positions.
- `reference_lines.py`: a threshold, an event, a normal range and a window over a line.
- `band.py`: a mean line over its 95% interval band.
- `stem.py`: a damped impulse response drawn as stems, with positive and negative samples.
- `error_bars.py`: group means with intervals on a category axis, and measurements with uncertainty in x and y.
- `bars.py`: a bar chart, a grouped bar chart and a stacked bar chart.
- `box.py`: plain and grouped box plots, with Tukey whiskers and outliers.
- `violin.py`: plain and grouped violin plots comparing sample distributions across regions.
- `horizontal.py`: horizontal grouped bars, horizontal box plots and a dot plot on a category y axis.
- `histogram.py`: a histogram with a chosen number of bins.
- `heatmap.py`: an annotated correlation matrix on a diverging colormap centered at zero, and visits by day and hour with an empty cell.
- `themes.py`: the same chart in the Pigment, Graphite and Midnight themes.
- `ticks.py`: custom x tick labels and a percent y tick format.
- `time_series.py`: a week of hourly demand on a time axis, with its range, a window and an event.
- `quick.py`: the one-call functions `ub.line()` and `ub.hist()`.
