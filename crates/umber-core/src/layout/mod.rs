//! Layout: text measurement, then the rectangles of the header, plot area, tick labels and axis
//! labels. All lengths are in points, with the origin at the top-left and y pointing down.
//! Layout also sets how many ticks a plot area of a given size asks for.
//!
//! Every derived value is finite: font sizes are clamped, text wider than
//! [`MAX_TEXT_WIDTH`] is cut to a prefix that fits, and the plot area is at least 1 by 1 pt and
//! lies on the canvas (unless the canvas itself is narrower than 1 pt). Text that does not fit,
//! such as a title wider than the figure, is clipped by the canvas.
//! Every x tick label keeps [`X_LABEL_EDGE_GAP`] from the canvas edges; x tick labels that would
//! need a larger plot inset than [`END_LABEL_SHARE`] allows, or a plot narrower than
//! [`MIN_PLOT`], fail the layout with `Error::Style`.
//! With figures of at most 200 in a side this keeps every coordinate far below the renderers'
//! `MAX_COORD`.

mod inset;
mod legend;
mod nudge;

pub(crate) use legend::{DirectLabel, EndDot, LegendRow, MeasuredLegend, RowEntry};

use crate::error::{number, quoted};
use crate::model::Axes;
use crate::resolve::{AxisTick, NormalizedPos, Resolved, coincides};
use crate::{Error, FontLibrary, Result, TextLayout, TextStyle, Theme, Weight};

/// The outer padding on every side.
pub(crate) const PADDING: f64 = 12.0;
/// Title line height, as a multiple of the title size.
const TITLE_LINE: f64 = 1.25;
/// Body text line height (the subtitle's and a legend row's), as a multiple of the body size.
const BODY_LINE: f64 = 1.35;
/// The gap between title and subtitle.
const TITLE_GAP: f64 = 2.0;
/// The gap below a non-empty header.
const HEADER_GAP: f64 = 8.0;
/// The gap between the y tick labels and the plot area.
const Y_LABEL_GAP: f64 = 6.0;
/// A y tick label's baseline sits this many sizes below its tick.
const Y_LABEL_CENTER: f64 = 0.35;
/// The gap between the plot bottom and the top of the x tick labels.
const X_LABEL_GAP: f64 = 4.0;
/// The gap between the tick labels and an axis label.
const AXIS_LABEL_GAP: f64 = 4.0;
/// The smallest plot side. [`fit_span`] keeps the plot at least this wide and tall, and the x
/// tick label solve in [`inset`] places x tick labels for a plot at least this wide.
const MIN_PLOT: f64 = 1.0;
/// The smallest gap between an x tick label and the left or right canvas edge: half the outer
/// padding, so an end label may reach into the padding but never ends flush with the edge.
const X_LABEL_EDGE_GAP: f64 = PADDING / 2.0;
/// The largest share of the inner width (inside the outer padding) by which x tick labels may
/// inset one side of the plot area, and that direct labels may reserve on the right.
const END_LABEL_SHARE: f64 = 0.25;
/// The widest text drawn; longer text is cut to a prefix that fits.
pub(crate) const MAX_TEXT_WIDTH: f64 = 1.0e5;
/// The largest font size used, whatever the theme says.
const MAX_FONT_SIZE: f32 = 1.0e4;

/// A rectangle in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Area {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width, at least [`MIN_PLOT`] for the plot area.
    pub width: f64,
    /// Height, at least [`MIN_PLOT`] for the plot area.
    pub height: f64,
}

impl Area {
    /// The right edge.
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// The bottom edge.
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// The size.
    pub fn size(&self) -> PlotSize {
        PlotSize {
            width: self.width,
            height: self.height,
        }
    }

    /// The x coordinate of normalized position `pos` (0 = left).
    pub fn x_at(&self, pos: NormalizedPos) -> f64 {
        self.x + pos.get() * self.width
    }

    /// The y coordinate of normalized position `pos` (0 = bottom).
    pub fn y_at(&self, pos: NormalizedPos) -> f64 {
        self.bottom() - pos.get() * self.height
    }
}

/// The size of a plot area, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlotSize {
    /// The width.
    pub width: f64,
    /// The height.
    pub height: f64,
}

/// Measured text, placed: `x` is the left end of the baseline, `y` the baseline.
#[derive(Debug, Clone)]
pub(crate) struct Label {
    /// The text, possibly cut to fit [`MAX_TEXT_WIDTH`].
    pub text: String,
    /// The style it was measured with.
    pub style: TextStyle,
    /// The shaped text.
    pub layout: TextLayout,
    /// Left end of the baseline.
    pub x: f64,
    /// Baseline.
    pub y: f64,
}

impl Label {
    /// The advance width.
    pub fn width(&self) -> f64 {
        f64::from(self.layout.width())
    }
}

/// The geometry of one axes.
#[derive(Debug, Clone)]
pub(crate) struct Layout {
    /// The plot area.
    pub plot: Area,
    /// Where the header ends, gap included; [`PADDING`] when the header is empty.
    #[allow(dead_code, reason = "read by tests")]
    pub header_bottom: f64,
    /// The title, if not empty.
    pub title: Option<Label>,
    /// The subtitle, if not empty.
    pub subtitle: Option<Label>,
    /// The legend row, under the subtitle; empty when the legend has no row entries.
    pub legend: LegendRow,
    /// The direct labels, right of the ends of their lines, in series order.
    pub direct: Vec<DirectLabel>,
    /// The x axis label, if not empty.
    pub xlabel: Option<Label>,
    /// The y axis label, if not empty.
    pub ylabel: Option<Label>,
    /// The x tick labels.
    pub x_tick_labels: Vec<Label>,
    /// The y tick labels, to the left of the plot area.
    pub y_tick_labels: Vec<Label>,
    /// The y coordinates of horizontal gridlines, without the baseline's.
    pub y_grid: Vec<f64>,
    /// The x coordinates of vertical gridlines; none on a category axis.
    pub x_grid: Vec<f64>,
    /// The y coordinate of the baseline.
    pub baseline_y: f64,
    /// The canvas width.
    pub canvas_width: f64,
}

impl Layout {
    /// The free room left of the plot area: up to the right edge of the furthest-reaching y
    /// tick label, or to the canvas left edge when there are no y tick labels. Never negative.
    pub fn room_left(&self) -> f64 {
        let edge = self
            .y_tick_labels
            .iter()
            .map(|label| label.x + label.width())
            .fold(0.0, f64::max);
        (self.plot.x - edge).max(0.0)
    }

    /// The free room right of the plot area, up to the canvas right edge. Never negative.
    pub fn room_right(&self) -> f64 {
        (self.canvas_width - self.plot.right()).max(0.0)
    }
}

/// `size * scale`, finite and within `0..=MAX_FONT_SIZE`.
fn scaled(size: f32, scale: f32) -> f32 {
    let v = size * scale;
    if v.is_finite() && v > 0.0 {
        v.min(MAX_FONT_SIZE)
    } else {
        0.0
    }
}

/// The text styles of one figure.
struct Styles {
    title: TextStyle,
    body: TextStyle,
    tick: TextStyle,
}

impl Styles {
    fn new(theme: &Theme) -> Styles {
        let scale = theme.font_scale();
        let body = TextStyle {
            font: theme.body_font(),
            weight: Weight::Regular,
            size: scaled(theme.body_size(), scale),
            tabular_figures: false,
        };
        Styles {
            title: TextStyle {
                font: theme.title_font(),
                weight: Weight::SemiBold,
                size: scaled(theme.title_size(), scale),
                tabular_figures: false,
            },
            body,
            tick: TextStyle {
                tabular_figures: true,
                ..body
            },
        }
    }
}

/// Ascent and descent of `style`'s primary font.
fn metrics(lib: &FontLibrary, style: &TextStyle) -> (f64, f64) {
    let layout = lib.layout("", style);
    (f64::from(layout.ascent()), f64::from(layout.descent()))
}

fn fits(layout: &TextLayout) -> bool {
    let w = f64::from(layout.width());
    w.is_finite() && w <= MAX_TEXT_WIDTH
}

/// Shapes `text`, cutting it to the longest prefix (on a character boundary) whose width is at
/// most [`MAX_TEXT_WIDTH`].
fn measure(lib: &FontLibrary, text: &str, style: TextStyle) -> Label {
    let full = lib.layout(text, &style);
    let (text, layout) = if fits(&full) {
        (text.to_owned(), full)
    } else {
        let ends: Vec<usize> = text
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(text.len()))
            .collect();
        // Invariant: the prefix ending at ends[lo] fits, the one at ends[hi] does not.
        let (mut lo, mut hi) = (0_usize, ends.len().saturating_sub(1));
        let mut best = (String::new(), lib.layout("", &style));
        while hi > lo + 1 {
            let mid = lo + (hi - lo) / 2;
            let prefix = ends.get(mid).and_then(|&end| text.get(..end)).unwrap_or("");
            let candidate = lib.layout(prefix, &style);
            if fits(&candidate) {
                lo = mid;
                best = (prefix.to_owned(), candidate);
            } else {
                hi = mid;
            }
        }
        best
    };
    Label {
        text,
        style,
        layout,
        x: 0.0,
        y: 0.0,
    }
}

/// Measures the label of every tick, keeping each tick's position.
fn measure_ticks(
    lib: &FontLibrary,
    ticks: &[AxisTick],
    style: TextStyle,
) -> Vec<(NormalizedPos, Label)> {
    ticks
        .iter()
        .map(|tick| (tick.pos, measure(lib, &tick.label, style)))
        .collect()
}

/// The baseline of text with `ascent` and `descent` in a line box of `height` whose top is
/// `top`: the leading is split evenly above and below.
fn baseline_in_line(top: f64, height: f64, ascent: f64, descent: f64) -> f64 {
    top + (height - ascent - descent) / 2.0 + ascent
}

/// The span `start..end` kept on a canvas of `extent`: the start is clamped to
/// `0..=extent - MIN_PLOT`, and the span is at least [`MIN_PLOT`] and ends on the canvas. Only a
/// canvas narrower than [`MIN_PLOT`] cannot hold it; then the span is `0..MIN_PLOT`.
fn fit_span(start: f64, end: f64, extent: f64) -> (f64, f64) {
    let start = start.clamp(0.0, (extent - MIN_PLOT).max(0.0));
    let end = end.min(extent).max(start + MIN_PLOT);
    (start, end - start)
}

/// The tick targets for a plot area of `plot`:
/// x `clamp(round(w / 100) + 1, 3, 8)`, y `clamp(round(h / 45) + 1, 3, 8)`.
///
/// These divisors were chosen so a 6 by 4 in figure gets about 5 ticks per axis
/// (y 0/25/50/75/100, x 0/10/20/30). A y divisor of 60 pt gave too few y ticks.
/// The y divisor is 45 rather than 50 so that plot areas shortened by a header and axis labels
/// still get 5 y ticks down to a height of 157.5 pt (50 would need 175 pt).
pub(crate) fn tick_targets(plot: PlotSize) -> (usize, usize) {
    fn target(size: f64, per_tick: f64, lo: usize, hi: usize) -> usize {
        let n = (size / per_tick).round() + 1.0;
        if n.is_nan() {
            return lo;
        }
        // The clamp below keeps the cast within range; NaN was handled above.
        (n.clamp(lo as f64, hi as f64)) as usize
    }
    (
        target(plot.width, 100.0, 3, 8),
        target(plot.height, 45.0, 3, 8),
    )
}

/// The plot size assumed before any tick label is measured: the `width` by `height` pt figure
/// inside the outer padding, at least [`MIN_PLOT`] a side.
pub(crate) fn initial_plot_size(width: f64, height: f64) -> PlotSize {
    PlotSize {
        width: (width - 2.0 * PADDING).max(MIN_PLOT),
        height: (height - 2.0 * PADDING).max(MIN_PLOT),
    }
}

/// What the layout of one axes is computed from besides its resolved scales. It is the same
/// for every layout pass.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LayoutInput<'a> {
    /// The canvas width, in points.
    pub width: f64,
    /// The canvas height, in points.
    pub height: f64,
    /// The theme.
    pub theme: &'a Theme,
    /// The axes, for their texts.
    pub axes: &'a Axes,
    /// The legend, measured by [`MeasuredLegend::measure`] for this canvas width.
    pub legend: &'a MeasuredLegend<'a>,
    /// The fonts.
    pub lib: &'a FontLibrary,
    /// The API call that renders, which layout errors are reported in, e.g. `"Figure.to_png()"`.
    pub context: &'static str,
}

/// The hint of the error for x tick labels that cannot fit.
const X_LABEL_HINT: &str = "shorten the labels or widen the figure.";

/// The hint of the error for y tick labels that leave no room for the plot area.
const Y_LABEL_HINT: &str = "shorten the y tick labels or widen the figure.";

/// Lays out the axes of `input` for already resolved scales.
///
/// # Errors
///
/// `Error::Style` in `input.context` when the x tick labels cannot all keep
/// [`X_LABEL_EDGE_GAP`] from the canvas edges on a plot at least [`MIN_PLOT`] wide, with each
/// side of the plot inset by at most [`END_LABEL_SHARE`] of the inner width. The message names
/// the widest x tick label, its width and the width up to which labels fit; when the y tick
/// labels alone leave the plot less than [`MIN_PLOT`], it names the widest y tick label
/// instead (see [`no_room_for_x_labels`]). Without x tick labels, nothing fails.
pub(crate) fn lay_out(input: &LayoutInput<'_>, resolved: &Resolved) -> Result<Layout> {
    let LayoutInput {
        width,
        height,
        theme,
        axes,
        legend,
        lib,
        context,
    } = *input;
    let styles = Styles::new(theme);

    // Header, top down.
    let mut y = PADDING;
    let title = (!axes.title_text().is_empty()).then(|| {
        let mut label = measure(lib, axes.title_text(), styles.title);
        let (ascent, descent) = metrics(lib, &styles.title);
        let line = TITLE_LINE * f64::from(styles.title.size);
        label.x = PADDING;
        label.y = baseline_in_line(y, line, ascent, descent);
        y += line;
        label
    });
    let subtitle = (!axes.subtitle_text().is_empty()).then(|| {
        if title.is_some() {
            y += TITLE_GAP;
        }
        let mut label = measure(lib, axes.subtitle_text(), styles.body);
        let (ascent, descent) = metrics(lib, &styles.body);
        let line = BODY_LINE * f64::from(styles.body.size);
        label.x = PADDING;
        label.y = baseline_in_line(y, line, ascent, descent);
        y += line;
        label
    });
    let has_row = !legend.row.is_empty();
    let row = if has_row {
        if title.is_some() || subtitle.is_some() {
            y += legend::ROW_TOP_GAP;
        }
        let placed = legend.row.placed(y, height);
        y += legend.row.height();
        placed
    } else {
        LegendRow::default()
    };
    if title.is_some() || subtitle.is_some() || has_row {
        y += HEADER_GAP;
    }
    let header_bottom = y;

    // Tick labels.
    let (tick_ascent, tick_descent) = metrics(lib, &styles.tick);
    let tick_size = f64::from(styles.tick.size);
    let baseline = resolved.baseline;
    let mut y_labels = measure_ticks(lib, &resolved.y.ticks, styles.tick);
    let mut x_labels = measure_ticks(lib, &resolved.x.ticks, styles.tick);
    let y_label_width = y_labels
        .iter()
        .map(|(_, label)| label.width())
        .fold(0.0, f64::max);

    // How far the y tick label of a tick at the plot top reaches above it.
    let label_row_rise = if y_labels.is_empty() {
        0.0
    } else {
        (tick_ascent - Y_LABEL_CENTER * tick_size).max(0.0)
    };

    let mut xlabel =
        (!axes.xlabel_text().is_empty()).then(|| measure(lib, axes.xlabel_text(), styles.body));
    let mut ylabel =
        (!axes.ylabel_text().is_empty()).then(|| measure(lib, axes.ylabel_text(), styles.body));
    let (body_ascent, body_descent) = metrics(lib, &styles.body);

    // The y label sits in the header region, above the top label row.
    let ylabel_band = if ylabel.is_some() {
        body_ascent + AXIS_LABEL_GAP
    } else {
        0.0
    };
    let plot_top = header_bottom + ylabel_band + label_row_rise;
    // The plot starts right of the y tick labels and ends left of the room direct labels
    // reserve. The x tick labels are centered on their ticks and may reach into the outer
    // padding, up to `X_LABEL_EDGE_GAP` from the canvas edge; where one would reach further,
    // the plot edge on that side moves in by exactly the overflow. Each side moves in by at
    // most a quarter of the inner width beyond the padding; labels that need more are an
    // error. Direct labels reserve room under the same cap.
    let inner = (width - 2.0 * PADDING).max(0.0);
    let cap = END_LABEL_SHARE * inner;
    let left = if y_labels.is_empty() {
        PADDING
    } else {
        PADDING + y_label_width + Y_LABEL_GAP
    };
    let right = width - PADDING - legend.direct.reserve(cap);
    let room = inset::Room {
        left,
        right,
        left_max: left.max(PADDING + cap),
        right_min: right.min(width - PADDING - cap),
        lo: X_LABEL_EDGE_GAP,
        hi: width - X_LABEL_EDGE_GAP,
        min_width: MIN_PLOT,
    };
    let spans: Vec<inset::TickSpan> = x_labels
        .iter()
        .map(|(pos, label)| inset::TickSpan {
            pos: pos.get(),
            half: label.width() / 2.0,
        })
        .collect();
    // Without x tick labels there is nothing to keep inside: the plot takes the room as it is,
    // and `fit_span` keeps it at least `MIN_PLOT` wide and on the canvas.
    let solved = if spans.is_empty() {
        inset::XInset::Contained { left, right }
    } else {
        inset::fit_x_labels(&room, &spans)
    };
    let (plot_left, plot_right) = match solved {
        inset::XInset::Contained { left, right } => (left, right),
        inset::XInset::Overflowing => {
            return Err(no_room_for_x_labels(
                context, &x_labels, &y_labels, &room, &spans,
            ));
        }
    };
    let x_band = if x_labels.is_empty() {
        0.0
    } else {
        X_LABEL_GAP + tick_ascent + tick_descent
    };
    let xlabel_band = if xlabel.is_some() {
        AXIS_LABEL_GAP + body_ascent + body_descent
    } else {
        0.0
    };
    let plot_bottom = height - PADDING - x_band - xlabel_band;
    let (x, plot_width) = fit_span(plot_left, plot_right, width);
    let (y, plot_height) = fit_span(plot_top, plot_bottom, height);
    let plot = Area {
        x,
        y,
        width: plot_width,
        height: plot_height,
    };

    // Place everything relative to the final plot area.
    for (pos, label) in &mut y_labels {
        label.x = plot.x - Y_LABEL_GAP - label.width();
        label.y = plot.y_at(*pos) + Y_LABEL_CENTER * tick_size;
    }
    for (pos, label) in &mut x_labels {
        label.x = plot.x_at(*pos) - label.width() / 2.0;
        label.y = plot.bottom() + X_LABEL_GAP + tick_ascent;
    }
    // The solve keeps the labels within the bounds for a plot at least `MIN_PLOT` wide, which
    // `fit_span` leaves in place; this holds the final placement to the same bounds.
    let outside = x_labels.iter().any(|(_, label)| {
        label.x < room.lo - inset::CONTAINMENT_TOLERANCE
            || label.x + label.width() > room.hi + inset::CONTAINMENT_TOLERANCE
    });
    if outside {
        return Err(x_labels_overflow(context, &x_labels, &room, &spans));
    }
    if let Some(label) = &mut xlabel {
        label.x = plot.x + plot.width / 2.0 - label.width() / 2.0;
        label.y = plot.bottom() + x_band + AXIS_LABEL_GAP + body_ascent;
    }
    if let Some(label) = &mut ylabel {
        // Left-aligned with the left edge of the y tick label column.
        label.x = if y_labels.is_empty() {
            plot.x
        } else {
            plot.x - Y_LABEL_GAP - y_label_width
        };
        label.y = plot.y - label_row_rise - AXIS_LABEL_GAP;
    }

    let direct = legend.direct.placed(resolved, plot, height);
    let baseline_y = plot.y_at(baseline);
    let y_grid = resolved
        .y
        .ticks
        .iter()
        .filter(|tick| !coincides(tick.pos, baseline))
        .map(|tick| plot.y_at(tick.pos))
        .collect();
    let x_grid = if resolved.x.is_category() {
        Vec::new()
    } else {
        resolved
            .x
            .ticks
            .iter()
            .map(|tick| plot.x_at(tick.pos))
            .collect()
    };

    Ok(Layout {
        plot,
        header_bottom,
        title,
        subtitle,
        legend: row,
        direct,
        xlabel,
        ylabel,
        x_tick_labels: x_labels.into_iter().map(|(_, label)| label).collect(),
        y_tick_labels: y_labels.into_iter().map(|(_, label)| label).collect(),
        y_grid,
        x_grid,
        baseline_y,
        canvas_width: width,
    })
}

/// The widest of `labels`, the first of equally wide ones; `None` when there are none.
fn widest(labels: &[(NormalizedPos, Label)]) -> Option<&Label> {
    labels
        .iter()
        .map(|(_, label)| label)
        .reduce(|widest, label| {
            if label.width() > widest.width() {
                label
            } else {
                widest
            }
        })
}

/// The error for x tick labels that no plot edges in `room` keep within its bounds.
///
/// When the room is narrower than [`MIN_PLOT`] even without labels, no x tick label fits,
/// however narrow. If y tick labels no wider than some width would leave the plot
/// [`MIN_PLOT`], they are the cause: the error names the widest y tick label (see
/// [`y_labels_leave_no_room`]). Otherwise the room is too narrow without them, or the x tick
/// labels need more room than the caps give, and the error names the widest x tick label (see
/// [`x_labels_overflow`]).
fn no_room_for_x_labels(
    context: &'static str,
    x_labels: &[(NormalizedPos, Label)],
    y_labels: &[(NormalizedPos, Label)],
    room: &inset::Room,
    spans: &[inset::TickSpan],
) -> Error {
    let plot_fits = matches!(
        inset::fit_x_labels(room, &[]),
        inset::XInset::Contained { .. }
    );
    // The widest y tick label column that leaves the plot `MIN_PLOT` wide.
    let y_room = room.right - MIN_PLOT - PADDING - Y_LABEL_GAP;
    match widest(y_labels) {
        Some(label) if !plot_fits && y_room >= 0.0 => {
            y_labels_leave_no_room(context, label, y_room)
        }
        _ => x_labels_overflow(context, x_labels, room, spans),
    }
}

/// The error for the y tick `label`, the widest, that leaves no room for the plot area: y tick
/// labels up to `y_room` wide would. The widths are in whole points, as in
/// [`x_labels_overflow`].
fn y_labels_leave_no_room(context: &'static str, label: &Label, y_room: f64) -> Error {
    let fits = y_room.floor();
    Error::style(
        context,
        format!(
            "y tick label {} is {} pt wide; at most {} pt leaves room for the plot area.",
            quoted(&label.text),
            number(label.width().round().max(fits + 1.0)),
            number(fits),
        ),
    )
    .with_hint(Y_LABEL_HINT)
}

/// The error for x tick `labels` that cannot fit `room`. It names the widest label (the first
/// of equally wide ones), which is always wider than the labels that fit, and gives the widths
/// in whole points: the width up to which labels fit rounded down, and the label's width
/// rounded, but at least one point more than that, so the two never look equal.
fn x_labels_overflow(
    context: &'static str,
    labels: &[(NormalizedPos, Label)],
    room: &inset::Room,
    spans: &[inset::TickSpan],
) -> Error {
    // Layout solves for x tick labels only when there is at least one.
    let (text, label_width) = widest(labels).map_or(("", 0.0), |l| (l.text.as_str(), l.width()));
    let fits = inset::widest_label_that_fits(room, spans).floor();
    Error::style(
        context,
        format!(
            "x tick label {} is {} pt wide; at most {} pt fits this figure.",
            quoted(text),
            number(label_width.round().max(fits + 1.0)),
            number(fits),
        ),
    )
    .with_hint(X_LABEL_HINT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScaleKind;
    use crate::model::{AxisData, Figure, LineOptions, ScatterOptions};
    use crate::pipeline::plan_figure;
    use crate::test_support::scene::assert_scene_bounded;

    fn lib() -> &'static FontLibrary {
        FontLibrary::global()
    }

    fn figure(theme: Theme) -> Figure {
        let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
        figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        figure
    }

    fn plan_of(figure: &Figure) -> (Resolved, Layout) {
        plan_figure(figure, lib())
    }

    #[test]
    fn header_parts_collapse_when_empty() {
        let theme = Theme::graphite();
        let ts = f64::from(theme.title_size());
        let bs = f64::from(theme.body_size());
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;

        let mut f = figure(theme.clone());
        let empty = plan_of(&f).1;
        assert!(empty.title.is_none() && empty.subtitle.is_none());
        assert_eq!(empty.header_bottom, PADDING);
        assert!(empty.legend.is_empty() && empty.legend.height() == 0.0);

        f.axes_mut().unwrap().title("Station output");
        let title = plan_of(&f).1;
        assert!(close(title.header_bottom, PADDING + 1.25 * ts + 8.0));
        assert!(title.subtitle.is_none());

        f.axes_mut().unwrap().subtitle("Daily mean, MW");
        let both = plan_of(&f).1;
        assert!(close(
            both.header_bottom,
            PADDING + 1.25 * ts + 2.0 + 1.35 * bs + 8.0
        ));
        let (t, s) = (both.title.unwrap(), both.subtitle.unwrap());
        assert!(t.y < s.y && t.x == PADDING && s.x == PADDING);

        f.axes_mut().unwrap().title("");
        let sub = plan_of(&f).1;
        assert!(close(sub.header_bottom, PADDING + 1.35 * bs + 8.0));
        assert!(sub.title.is_none());

        // The plot area moves up by exactly the collapsed header.
        assert!(close(
            sub.plot.y - empty.plot.y,
            sub.header_bottom - empty.header_bottom
        ));
    }

    #[test]
    fn plot_area_shrinks_with_long_y_labels() {
        let mut short = figure(Theme::graphite());
        short.axes_mut().unwrap().ylim(0.0, 1.0).unwrap();
        let mut long = figure(Theme::graphite());
        long.axes_mut().unwrap().ylim(100_000.0, 900_000.0).unwrap();
        let (_, a) = plan_of(&short);
        let (_, b) = plan_of(&long);
        assert!(b.plot.x > a.plot.x + 10.0, "{} vs {}", b.plot.x, a.plot.x);
        assert!(b.plot.width < a.plot.width);
        assert_eq!(a.plot.right(), b.plot.right());
        // Outside: labels are right-aligned 6 pt left of the plot area.
        for label in &b.y_tick_labels {
            assert!((label.x + label.width() - (b.plot.x - 6.0)).abs() < 1e-6);
        }
        let widest = b.y_tick_labels.iter().map(Label::width).fold(0.0, f64::max);
        assert!((b.plot.x - (PADDING + widest + 6.0)).abs() < 1e-9);
    }

    #[test]
    fn y_labels_sit_left_of_the_plot_in_every_theme() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let mut f = figure(theme);
            f.axes_mut().unwrap().ylim(0.0, 100.0).unwrap();
            let (resolved, layout) = plan_of(&f);
            assert_eq!(resolved.baseline.get(), 0.0);
            // The baseline replaces the gridline at 0.
            assert!(
                layout
                    .y_grid
                    .iter()
                    .all(|&y| (y - layout.baseline_y).abs() > 1e-6)
            );
            assert_eq!(layout.y_grid.len() + 1, resolved.y.ticks.len());
            // Every tick has a label, the baseline's included, right-aligned 6 pt left of the
            // plot area.
            let labels: Vec<&str> = layout
                .y_tick_labels
                .iter()
                .map(|l| l.text.as_str())
                .collect();
            assert_eq!(labels.len(), resolved.y.ticks.len());
            assert_eq!(labels.first(), Some(&"0"));
            assert_eq!(labels.last(), Some(&"100"));
            for label in &layout.y_tick_labels {
                assert!((label.x + label.width() - (layout.plot.x - 6.0)).abs() < 1e-6);
            }
            let widest = layout
                .y_tick_labels
                .iter()
                .map(Label::width)
                .fold(0.0, f64::max);
            assert!((layout.plot.x - (PADDING + widest + 6.0)).abs() < 1e-9);
        }
    }

    #[test]
    fn tiny_figures_and_long_text_lay_out() {
        let long: String = "Station output ".repeat(34).chars().take(500).collect();
        assert_eq!(long.chars().count(), 500);
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            for scale in [1.0, 10.0] {
                let theme = theme.with_font_scale(scale).unwrap();
                // Without x ticks: their labels do not fit a 1 in figure at these font scales
                // (see `labels_wider_than_the_capped_room_fail_naming_the_widest`).
                let mut f = Figure::new(1.0, 1.0, theme).unwrap();
                f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                    .unwrap()
                    .xticks(&[])
                    .unwrap()
                    .title(&long)
                    .subtitle(&long)
                    .xlabel(&long)
                    .ylabel(&long)
                    .ylim(100_000.0, 900_000.0)
                    .unwrap()
                    .line(
                        AxisData::Numbers(vec![0.0, 1.0]),
                        &[200_000.0, 300_000.0],
                        LineOptions::default(),
                    )
                    .unwrap();
                let (_, layout) = plan_of(&f);
                assert!(layout.plot.width >= 1.0 && layout.plot.height >= 1.0);
                for v in [
                    layout.plot.x,
                    layout.plot.y,
                    layout.plot.width,
                    layout.plot.height,
                ] {
                    assert!(v.is_finite());
                }
                let scene = f.to_scene(lib()).unwrap();
                assert_scene_bounded(&scene);
            }
        }

        // Text far wider than any figure is cut to a prefix that fits.
        let huge = "W".repeat(20_000);
        let theme = Theme::pigment().with_font_scale(10.0).unwrap();
        let mut f = Figure::new(1.0, 1.0, theme.clone()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xticks(&[])
            .unwrap()
            .title(&huge)
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions::default(),
            )
            .unwrap();
        let (_, layout) = plan_of(&f);
        let title = layout.title.as_ref().unwrap();
        assert!(title.width() <= MAX_TEXT_WIDTH && title.width() > MAX_TEXT_WIDTH * 0.9);
        assert!(huge.starts_with(&title.text));
        assert_scene_bounded(&f.to_scene(lib()).unwrap());
        // A tick label as wide is cut the same way, and then named in the error. Without y
        // ticks: their labels at this font scale leave no room for the plot, and the error would
        // name them (see `y_labels_that_leave_no_room_for_the_plot_fail_naming_the_widest`).
        let mut f = Figure::new(1.0, 1.0, theme).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .yticks(&[])
            .unwrap()
            .line(
                AxisData::Categories(vec![huge.clone()]),
                &[1.0],
                LineOptions::default(),
            )
            .unwrap();
        let message = overflow(&f).to_string();
        let shown = format!("x tick label \"{}… is ", "W".repeat(38));
        assert!(message.contains(&shown), "{message}");
        let width: f64 = message
            .split(" is ")
            .nth(1)
            .and_then(|rest| rest.split(" pt").next())
            .and_then(|n| n.parse().ok())
            .unwrap();
        assert!(
            width <= MAX_TEXT_WIDTH && width > MAX_TEXT_WIDTH * 0.9,
            "{message}"
        );
    }

    #[test]
    fn tick_targets_follow_plot_size() {
        let size = |width: f64, height: f64| PlotSize { width, height };
        assert_eq!(tick_targets(size(0.0, 0.0)), (3, 3));
        assert_eq!(tick_targets(size(1.0, 1.0)), (3, 3));
        // 6 by 4 in plot areas: 408 / 100 + 1 = 5; 172 / 45 + 1 = 5 and 178 / 45 + 1 = 5 (a plot
        // under a title, a subtitle and axis labels is about 178 pt tall in every theme).
        assert_eq!(tick_targets(size(408.0, 172.0)), (5, 5));
        assert_eq!(tick_targets(size(408.0, 178.0)), (5, 5));
        // Rounding: 3.5 rounds up, just below 3.5 rounds down (3.5 * 45 = 157.5).
        assert_eq!(tick_targets(size(350.0, 157.0)), (5, 4));
        assert_eq!(tick_targets(size(349.0, 158.0)), (4, 5));
        // Upper clamps.
        assert_eq!(tick_targets(size(10_000.0, 10_000.0)), (8, 8));
        assert_eq!(tick_targets(size(f64::NAN, f64::INFINITY)), (3, 8));
    }

    #[test]
    fn six_by_four_figures_get_five_ticks_per_axis() {
        // Chosen so a 6 by 4 in figure gets about five ticks per axis: y 0/25/50/75/100 and x 0/10/20/30 in every theme.
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let mut f = figure(theme);
            f.axes_mut()
                .unwrap()
                .title("Station output")
                .subtitle("Daily mean, MW")
                .xlabel("Day")
                .ylabel("MW")
                .scatter(
                    AxisData::Numbers(vec![0.0, 15.0, 30.0]),
                    &[3.0, 50.0, 97.0],
                    ScatterOptions::default(),
                )
                .unwrap()
                .xlim(0.0, 30.0)
                .unwrap();
            let (resolved, _) = plan_of(&f);
            let labels = |ticks: &[AxisTick]| -> Vec<String> {
                ticks.iter().map(|t| t.label.clone()).collect()
            };
            assert_eq!(labels(&resolved.y.ticks), ["0", "25", "50", "75", "100"]);
            assert_eq!(labels(&resolved.x.ticks), ["0", "10", "20", "30"]);
        }
    }

    fn category_plot(theme: Theme, names: [&str; 3]) -> Area {
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Categories(names.iter().map(|s| (*s).to_owned()).collect()),
                &[1.0, 2.0, 3.0],
                LineOptions::default(),
            )
            .unwrap();
        plan_of(&f).1.plot
    }

    #[test]
    fn huge_end_labels_fail_to_lay_out() {
        let (wide, narrow) = ("W".repeat(500), "V".repeat(500));
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let mut f = Figure::new(6.0, 4.0, theme).unwrap();
            f.add_axes(ScaleKind::Category, ScaleKind::Linear)
                .unwrap()
                .line(
                    AxisData::Categories(vec![wide.clone(), "Q2".into(), narrow.clone()]),
                    &[1.0, 2.0, 3.0],
                    LineOptions::default(),
                )
                .unwrap();
            let message = overflow(&f).to_string();
            assert!(
                message.starts_with(&format!(
                    "Figure.to_scene(): x tick label \"{}… is ",
                    "W".repeat(38)
                )),
                "{message}"
            );
        }
    }

    #[test]
    fn end_label_reservation_uses_tick_positions() {
        // Category end ticks sit at band centers, a sixth of the plot in from each edge, so
        // these labels need (almost) no reservation.
        let names = [
            "First quarter of the year",
            "Q2",
            "Last quarter of the year",
        ];
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let reserved = category_plot(theme.clone(), names).width;
            let free = category_plot(theme, ["A", "B", "C"]).width;
            assert!(reserved >= 0.98 * free, "{reserved} vs {free}");
        }
    }

    #[test]
    fn log_end_ticks_do_not_crowd_their_neighbors() {
        // The domain ends 1e-300 and 1e300 are not powers of these bases.
        for base in [2.0, std::f64::consts::E, 999.0] {
            for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
                let min_gap = 1.2 * f64::from(theme.body_size() * theme.font_scale());
                let mut f = Figure::new(6.0, 4.0, theme).unwrap();
                f.add_axes(
                    ScaleKind::Linear,
                    ScaleKind::Log {
                        base: crate::LogBase::new(base).unwrap(),
                    },
                )
                .unwrap()
                .line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[1e-300, 1e300],
                    LineOptions::default(),
                )
                .unwrap();
                let (resolved, layout) = plan_of(&f);
                let labels = &layout.y_tick_labels;
                assert!(labels.len() >= 2);
                assert_eq!(resolved.y.ticks.first().map(|t| t.pos.get()), Some(0.0));
                assert_eq!(resolved.y.ticks.last().map(|t| t.pos.get()), Some(1.0));
                for pair in labels.windows(2) {
                    let gap = (pair[0].y - pair[1].y).abs();
                    assert!(
                        gap >= min_gap,
                        "base {base}: {:?} and {:?} are {gap} pt apart",
                        pair[0].text,
                        pair[1].text
                    );
                }
            }
        }
    }

    #[test]
    fn labels_sit_around_the_plot() {
        let mut f = figure(Theme::graphite());
        f.axes_mut()
            .unwrap()
            .xlabel("Day")
            .ylabel("MW")
            .xlim(0.0, 30.0)
            .unwrap()
            .ylim(0.0, 100.0)
            .unwrap();
        let (_, layout) = plan_of(&f);
        let plot = layout.plot;
        let tick = layout.x_tick_labels.first().unwrap();
        assert!(tick.y > plot.bottom());
        let xlabel = layout.xlabel.as_ref().unwrap();
        assert!(xlabel.y > tick.y);
        assert!((xlabel.x + xlabel.width() / 2.0 - (plot.x + plot.width / 2.0)).abs() < 1e-9);
        let ylabel = layout.ylabel.as_ref().unwrap();
        assert!(ylabel.y < plot.y);
        assert_eq!(ylabel.x, PADDING);
        // Everything stays inside the figure.
        assert!(xlabel.y <= 4.0 * 72.0 - PADDING);
        assert!(plot.right() <= 6.0 * 72.0 - PADDING);
    }

    #[test]
    fn custom_tick_labels_are_measured_like_automatic_ones() {
        let plot_with = |set: &dyn Fn(&mut Axes)| {
            let mut f = figure(Theme::graphite());
            let axes = f.axes_mut().unwrap();
            axes.scatter(
                AxisData::Numbers(vec![0.0, 10.0]),
                &[0.0, 10.0],
                ScatterOptions::default(),
            )
            .unwrap();
            set(axes);
            plan_of(&f).1
        };
        let short = plot_with(&|a: &mut Axes| {
            a.xticks_labeled(&[0.0, 10.0], vec!["a".into(), "b".into()])
                .unwrap()
                .yticks_labeled(&[0.0, 10.0], vec!["a".into(), "b".into()])
                .unwrap();
        });
        let long = plot_with(&|a: &mut Axes| {
            let wide = "a rather long label".to_owned();
            a.xticks_labeled(&[0.0, 10.0], vec!["a".into(), wide.clone()])
                .unwrap()
                .yticks_labeled(&[0.0, 10.0], vec!["a".into(), wide])
                .unwrap();
        });
        // The y label column grows to the widest label.
        let widest = long
            .y_tick_labels
            .iter()
            .map(Label::width)
            .fold(0.0, f64::max);
        assert!((long.plot.x - (PADDING + widest + Y_LABEL_GAP)).abs() < 1e-9);
        assert!(long.plot.x > short.plot.x + 20.0);
        // The last x label, centered near the right end, insets the plot so that it ends
        // `X_LABEL_EDGE_GAP` from the canvas edge.
        let last = long.x_tick_labels.last().unwrap();
        assert!(long.plot.right() < short.plot.right() - 1.0);
        assert!(
            (last.x + last.width() - (6.0 * 72.0 - X_LABEL_EDGE_GAP)).abs() < 1e-9,
            "{last:?}"
        );
        // A suffix from the tick format is measured the same way.
        let suffixed = plot_with(&|a: &mut Axes| {
            let format = crate::TickFormat::AUTO
                .affix("", " a rather long label")
                .unwrap();
            a.ytick_format(format).unwrap();
        });
        assert!(suffixed.plot.x > short.plot.x + 20.0);
        assert!(
            suffixed
                .y_tick_labels
                .iter()
                .all(|l| l.text.ends_with(" a rather long label"))
        );
    }

    /// The text items of `items`, groups included.
    fn texts(items: &[crate::scene::Item]) -> Vec<&crate::scene::TextItem> {
        use crate::scene::Item;
        items
            .iter()
            .flat_map(|item| match item {
                Item::Text(text) => vec![text],
                Item::Group { items, .. } => texts(items),
                _ => Vec::new(),
            })
            .collect()
    }

    /// Lays out `f` and asserts that every x tick label keeps `X_LABEL_EDGE_GAP` from the
    /// canvas edges, both in the layout and as drawn in the scene. Returns the layout.
    fn assert_x_labels_inside(f: &Figure) -> Layout {
        let (_, layout) = plan_of(f);
        let width = f.width_in() * 72.0;
        assert!(!layout.x_tick_labels.is_empty());
        for label in &layout.x_tick_labels {
            assert!(label.x >= X_LABEL_EDGE_GAP - 1e-9, "{label:?}");
            assert!(
                label.x + label.width() <= width - X_LABEL_EDGE_GAP + 1e-9,
                "{label:?}"
            );
        }
        let scene = f.to_scene(lib()).unwrap();
        let drawn = texts(&scene.items);
        for label in &layout.x_tick_labels {
            let item = drawn
                .iter()
                .find(|t| {
                    t.text == label.text
                        && (f64::from(t.origin.x) - label.x).abs() < 1e-3
                        && (f64::from(t.origin.y) - label.y).abs() < 1e-3
                })
                .unwrap_or_else(|| panic!("{label:?} is not drawn"));
            let (left, right) = (
                f64::from(item.origin.x),
                f64::from(item.origin.x + item.width),
            );
            assert!(
                left >= X_LABEL_EDGE_GAP - 1e-3 && right <= width - X_LABEL_EDGE_GAP + 1e-3,
                "{item:?}"
            );
        }
        layout
    }

    fn day_line(theme: Theme, labels: [&str; 5]) -> Figure {
        let mut f = figure(theme);
        f.axes_mut()
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 6.0, 12.0, 18.0, 24.0]),
                &[1.0, 3.0, 2.0, 4.0, 3.0],
                LineOptions::default(),
            )
            .unwrap()
            .xticks_labeled(
                &[0.0, 6.0, 12.0, 18.0, 24.0],
                labels.iter().map(|s| (*s).to_owned()).collect(),
            )
            .unwrap();
        f
    }

    #[test]
    fn a_wide_last_label_insets_the_plot_by_its_overflow() {
        let width = 6.0 * 72.0;
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let narrow =
                assert_x_labels_inside(&day_line(theme.clone(), ["0", "6", "12", "18", "24"]));
            let wide = assert_x_labels_inside(&day_line(
                theme,
                ["midnight", "6 am", "noon", "6 pm", "midnight"],
            ));
            // The last tick sits on the plot's right edge, so its label ends exactly
            // `X_LABEL_EDGE_GAP` from the canvas edge. The first label fits left of the y tick
            // labels' column.
            let last = wide.x_tick_labels.last().unwrap();
            assert!(
                (last.x + last.width() - (width - X_LABEL_EDGE_GAP)).abs() < 1e-9,
                "{last:?}"
            );
            assert!(wide.plot.right() < narrow.plot.right() - 1.0);
            assert_eq!(wide.plot.x, narrow.plot.x);
            // The labels stay centered on their ticks.
            assert!((last.x + last.width() / 2.0 - wide.plot.right()).abs() < 1e-9);
        }
    }

    #[test]
    fn a_narrow_last_label_leaves_the_layout_unchanged() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let layout = assert_x_labels_inside(&day_line(theme, ["0", "6", "12", "18", "24"]));
            let widest = layout
                .y_tick_labels
                .iter()
                .map(Label::width)
                .fold(0.0, f64::max);
            // "24" reaches past the plot's right edge by half its width, less than the padding
            // minus the edge gap, so the plot keeps its full width.
            let last = layout.x_tick_labels.last().unwrap();
            assert!(last.x + last.width() > layout.plot.right());
            assert_eq!(layout.plot.right(), 6.0 * 72.0 - PADDING);
            assert_eq!(layout.plot.x, PADDING + widest + Y_LABEL_GAP);
        }
    }

    #[test]
    fn wide_affixed_automatic_labels_stay_inside() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let mut f = figure(theme);
            let format = crate::TickFormat::AUTO
                .affix("", " thousand units")
                .unwrap();
            f.axes_mut()
                .unwrap()
                .scatter(
                    AxisData::Numbers(vec![0.0, 40.0, 100.0]),
                    &[1.0, 3.0, 2.0],
                    ScatterOptions::default(),
                )
                .unwrap()
                .xtick_format(format)
                .unwrap();
            let layout = assert_x_labels_inside(&f);
            // The padded domain puts the last tick inside the plot, yet the label still
            // overflowed and moved the right edge.
            assert!(layout.plot.right() < 6.0 * 72.0 - PADDING - 1.0);
            let last = layout.x_tick_labels.last().unwrap();
            assert!(last.text.ends_with(" thousand units"));
            assert!(
                (last.x + last.width() - (6.0 * 72.0 - X_LABEL_EDGE_GAP)).abs() < 1e-9,
                "{last:?}"
            );
        }
    }

    #[test]
    fn a_wide_last_category_label_stays_inside() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let mut f = Figure::new(6.0, 4.0, theme).unwrap();
            f.add_axes(ScaleKind::Category, ScaleKind::Linear)
                .unwrap()
                .line(
                    AxisData::Categories(vec![
                        "North".into(),
                        "East".into(),
                        "Southern hemisphere stations, all regions combined".into(),
                    ]),
                    &[1.0, 2.0, 3.0],
                    LineOptions::default(),
                )
                .unwrap();
            let layout = assert_x_labels_inside(&f);
            // The last band center sits a sixth of the plot in from the right edge, and the
            // edge still moved.
            assert!(layout.plot.right() < 6.0 * 72.0 - PADDING - 1.0);
            let last = layout.x_tick_labels.last().unwrap();
            assert!(
                (last.x + last.width() - (6.0 * 72.0 - X_LABEL_EDGE_GAP)).abs() < 1e-9,
                "{last:?}"
            );
        }
    }

    /// A `width` by 4 in midnight figure whose x axis has explicit `limits` and two ticks at
    /// `ticks`, each labelled with `chars` W's.
    fn close_ticks(
        width: f64,
        x: ScaleKind,
        limits: (f64, f64),
        ticks: [f64; 2],
        chars: usize,
    ) -> Figure {
        let mut f = Figure::new(width, 4.0, Theme::midnight()).unwrap();
        let label = "W".repeat(chars);
        f.add_axes(x, ScaleKind::Linear)
            .unwrap()
            .xlim(limits.0, limits.1)
            .unwrap()
            .xticks_labeled(&ticks, vec![label.clone(), label])
            .unwrap();
        f
    }

    #[test]
    fn close_interior_ticks_with_wide_labels_stay_inside() {
        // Each edge moves the other edge's label almost as far as its own, so the edges settle
        // only where both bounds hold at once. The labels, about 413 pt wide on 6 in and 4218 pt
        // on 60 in, are wide enough that both bounds bind, and narrow enough that the edges
        // stay within the caps.
        let log = ScaleKind::Log {
            base: crate::LogBase::new(10.0).unwrap(),
        };
        let at = |share: f64| 10f64.powf(3.0 * share);
        for (f, width) in [
            (
                close_ticks(6.0, ScaleKind::Linear, (0.0, 1.0), [0.485, 0.515], 81),
                432.0,
            ),
            (
                close_ticks(6.0, log, (1.0, 1000.0), [at(0.485), at(0.515)], 81),
                432.0,
            ),
            (
                close_ticks(60.0, ScaleKind::Linear, (0.0, 1.0), [0.485, 0.515], 827),
                4320.0,
            ),
        ] {
            let layout = assert_x_labels_inside(&f);
            let (first, last) = (&layout.x_tick_labels[0], &layout.x_tick_labels[1]);
            assert!((first.x - X_LABEL_EDGE_GAP).abs() < 1e-6, "{first:?}");
            assert!(
                (last.x + last.width() - (width - X_LABEL_EDGE_GAP)).abs() < 1e-6,
                "{last:?}"
            );
        }
    }

    /// A `width` by `width` in midnight figure on x 0..1 with ticks at 0 and 1 labelled
    /// `labels`, and `y_ticks` as its y ticks.
    fn end_labels(width: f64, labels: [String; 2], y_ticks: Option<&[f64]>) -> Figure {
        let mut f = Figure::new(width, width.min(4.0), Theme::midnight()).unwrap();
        let axes = f
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .xticks_labeled(&[0.0, 1.0], labels.to_vec())
            .unwrap();
        if let Some(ticks) = y_ticks {
            axes.yticks(ticks).unwrap();
        }
        f
    }

    /// The error of laying out `f`, which must fail.
    fn overflow(f: &Figure) -> crate::Error {
        crate::pipeline::try_plan_figure(f, lib()).unwrap_err()
    }

    #[test]
    fn labels_wider_than_the_capped_room_fail_naming_the_widest() {
        // The last label needs more inset than a quarter of the inner width: its tick may move
        // in to 318 pt, and the label may reach 426 pt, so it fits up to 2 × 108 pt wide.
        let f = end_labels(6.0, ["0".into(), "W".repeat(60)], None);
        let err = overflow(&f);
        let shown = format!("\"{}…", "W".repeat(38));
        assert_eq!(
            err.to_string(),
            format!(
                "Figure.to_scene(): x tick label {shown} is 306 pt wide; at most 216 pt fits \
                 this figure."
            )
        );
        assert!(matches!(err, crate::Error::Style { .. }));
        assert_eq!(err.parameter(), None);
        assert_eq!(err.hint(), Some("shorten the labels or widen the figure."));

        // Wide labels at both ends of a 1 in figure without y tick labels: each tick may move
        // in by 12 pt, to 24 and 48 pt, and the labels may reach 6 and 66 pt, so they fit up
        // to 2 × 18 pt wide.
        let wide = "W".repeat(10);
        let f = end_labels(1.0, [wide.clone(), wide], Some(&[]));
        let err = overflow(&f);
        assert_eq!(
            err.to_string(),
            "Figure.to_scene(): x tick label \"WWWWWWWWWW\" is 51 pt wide; at most 36 pt fits \
             this figure."
        );
        assert_eq!(err.hint(), Some("shorten the labels or widen the figure."));
    }

    #[test]
    fn labels_just_inside_the_capped_room_render() {
        // 42 W's are 214.2 pt wide in midnight, just under the 216 pt that fit: the right edge
        // moves in almost to its cap and the label ends on the edge gap. One more W fails.
        let width = 6.0 * 72.0;
        let f = end_labels(6.0, ["0".into(), "W".repeat(42)], None);
        let layout = assert_x_labels_inside(&f);
        let last = layout.x_tick_labels.last().unwrap();
        assert!((last.width() - 214.2).abs() < 1e-3, "{last:?}");
        assert!(
            (last.x + last.width() - (width - X_LABEL_EDGE_GAP)).abs() < 1e-9,
            "{last:?}"
        );
        let cap = END_LABEL_SHARE * (width - 2.0 * PADDING);
        assert!(layout.plot.right() > width - PADDING - cap);
        let f = end_labels(6.0, ["0".into(), "W".repeat(43)], None);
        assert!(
            overflow(&f)
                .to_string()
                .ends_with("is 219 pt wide; at most 216 pt fits this figure.")
        );
    }

    /// A 2 in midnight figure on x 0..1 whose only x tick, at 1, is labelled `END`, and whose
    /// two y ticks are labelled with `y_chars` W's each.
    fn end_after_wide_y_labels(y_chars: usize) -> Figure {
        let mut f = Figure::new(2.0, 2.0, Theme::midnight()).unwrap();
        let wide = "W".repeat(y_chars);
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .xticks_labeled(&[1.0], vec!["END".into()])
            .unwrap()
            .yticks_labeled(&[0.0, 1.0], vec![wide.clone(), wide])
            .unwrap();
        f
    }

    #[test]
    fn y_labels_that_leave_no_room_for_the_plot_fail_naming_the_widest() {
        // The y tick labels, 510 pt wide, push the plot's left edge to 528 pt, past its right
        // edge at 132 pt. Labels up to 132 - 1 - 12 - 6 = 113 pt wide leave the plot 1 pt.
        let err = overflow(&end_after_wide_y_labels(100));
        assert_eq!(
            err.to_string(),
            format!(
                "Figure.to_scene(): y tick label \"{}… is 510 pt wide; at most 113 pt leaves \
                 room for the plot area.",
                "W".repeat(38)
            )
        );
        assert!(matches!(err, crate::Error::Style { .. }));
        assert_eq!(err.parameter(), None);
        assert_eq!(
            err.hint(),
            Some("shorten the y tick labels or widen the figure.")
        );
        // Without x tick labels there is nothing to keep inside: the figure renders, its plot
        // kept 1 pt wide on the canvas.
        let mut f = end_after_wide_y_labels(100);
        f.axes_mut().unwrap().xticks(&[]).unwrap();
        let (_, layout) = plan_of(&f);
        assert_eq!(layout.plot.width, MIN_PLOT);
        assert!(layout.plot.right() <= 2.0 * 72.0);
    }

    #[test]
    fn x_labels_need_the_plot_at_least_its_minimum_width() {
        // 22 W's, 112.2 pt, leave the plot room (130.2..132 pt), but END, 15.3 pt wide, may
        // reach 138 pt only if its tick, on the right edge, is at most 130.35 pt: a plot 0.15 pt
        // wide. On a plot 1 pt wide END fits up to 2 × (138 - 131.2) = 13.6 pt.
        let err = overflow(&end_after_wide_y_labels(22));
        assert_eq!(
            err.to_string(),
            "Figure.to_scene(): x tick label \"END\" is 15 pt wide; at most 13 pt fits this \
             figure."
        );
        assert_eq!(err.hint(), Some("shorten the labels or widen the figure."));
        // 21 W's, 107.1 pt, leave END room on a plot about 5 pt wide.
        let layout = assert_x_labels_inside(&end_after_wide_y_labels(21));
        assert!(layout.plot.width >= MIN_PLOT, "{:?}", layout.plot);
        let end = layout.x_tick_labels.first().unwrap();
        assert!(
            (end.x + end.width() - (2.0 * 72.0 - X_LABEL_EDGE_GAP)).abs() < 1e-9,
            "{end:?}"
        );
    }

    #[test]
    fn long_y_labels_that_fit_still_render() {
        // 40 W's, 204 pt, leave the plot of a 6 in figure 198 pt wide.
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let mut f = figure(theme);
            let wide = "W".repeat(40);
            f.axes_mut()
                .unwrap()
                .xlim(0.0, 30.0)
                .unwrap()
                .yticks_labeled(&[0.0, 1.0], vec![wide.clone(), wide])
                .unwrap();
            let layout = assert_x_labels_inside(&f);
            let widest = layout
                .y_tick_labels
                .iter()
                .map(Label::width)
                .fold(0.0, f64::max);
            assert!(widest > 150.0, "{widest}");
            assert_eq!(layout.plot.x, PADDING + widest + Y_LABEL_GAP);
            assert_eq!(layout.plot.right(), 6.0 * 72.0 - PADDING);
        }
    }

    #[test]
    fn the_widest_label_is_named() {
        // Both labels overflow; the wider one, here the first, is named.
        let f = end_labels(1.0, ["W".repeat(12), "W".repeat(10)], Some(&[]));
        assert!(
            overflow(&f)
                .to_string()
                .contains(&format!("x tick label \"{}\" is 61 pt", "W".repeat(12)))
        );
    }
}
