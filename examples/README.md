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
- `by.py`: long-form columns grouped with `by=` into lines, side-by-side bars, ECDFs and a stacked histogram.
- `box.py`: plain and grouped box plots, with Tukey whiskers and outliers.
- `violin.py`: plain and grouped violin plots comparing sample distributions across regions.
- `horizontal.py`: horizontal grouped bars, horizontal box plots and a dot plot on a category y axis.
- `histogram.py`: a histogram with a chosen number of bins.
- `hist_variants.py`: a histogram with uneven bin edges and densities, and cumulative fractions as bars and as an outline.
- `ecdf.py`: empirical cumulative distributions of response times, and the complementary share on a log axis.
- `kde.py`: kernel density lines with Scott's bandwidth and a narrower one, over a density histogram.
- `density.py`: rectangular and hexagonal density counts with log colors, and a numeric mesh centered on zero.
- `annotations.py`: annotations with and without arrows on a time series, text at a point, and values written on plain and stacked bars.
- `heatmap.py`: an annotated correlation matrix on a diverging colormap centered at zero, and visits by day and hour with an empty cell.
- `contour.py`: a filled elevation map with labelled lines over its bands, and a wave field drawn as labelled lines in one color.
- `themes.py`: the same chart in the Pigment, Graphite and Midnight themes.
- `ticks.py`: custom x tick labels and a percent y tick format.
- `time_series.py`: a week of hourly demand on a time axis, with its range, a window and an event.
- `pie.py`: pies and donuts with share annotations, outside labels and a separate panel legend.
- `quick.py`: the one-call functions `ub.line()` and `ub.hist()`.
- `twin_y.py`: rainfall bars and a temperature line sharing x, with independent left and right y axes.
