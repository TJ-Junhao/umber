//! Layout: text measurement, then the rectangles of the header, plot area, tick labels and axis
//! labels. All lengths are in points, with the origin at the top-left and y pointing down.
//!
//! Every derived value is finite: font sizes are clamped, text wider than
//! [`MAX_TEXT_WIDTH`] is cut to a prefix that fits, and the plot area is at least 1 by 1 pt and
//! lies on the canvas (unless the canvas itself is narrower than 1 pt). Text that does not fit,
//! such as a tick label wider than the space reserved for it, is clipped by the canvas.
//! With figures of at most 200 in a side this keeps every coordinate far below the renderers'
//! `MAX_COORD`.

use crate::figure::Axes;
use crate::resolve::{self, AxisTick, Resolved, coincides};
use crate::{FontLibrary, TextLayout, TextStyle, Theme, TickLabelPlacement, Weight};

/// The outer padding on every side.
pub(crate) const PADDING: f64 = 12.0;
/// Title line height, as a multiple of the title size.
const TITLE_LINE: f64 = 1.25;
/// Subtitle line height, as a multiple of the body size.
const SUBTITLE_LINE: f64 = 1.35;
/// The gap between title and subtitle.
const TITLE_GAP: f64 = 2.0;
/// The gap below a non-empty header.
const HEADER_GAP: f64 = 8.0;
/// `Outside`: the gap between the y tick labels and the plot area.
const Y_LABEL_GAP: f64 = 6.0;
/// `Outside`: a y label's baseline sits this many sizes below its tick.
const Y_LABEL_CENTER: f64 = 0.35;
/// `AboveGridline`: a y label's baseline sits this far above its gridline.
const ABOVE_GRIDLINE_GAP: f64 = 3.0;
/// The gap between the plot bottom and the top of the x tick labels.
const X_LABEL_GAP: f64 = 4.0;
/// The gap between the tick labels and an axis label.
const AXIS_LABEL_GAP: f64 = 4.0;
/// The smallest plot side.
const MIN_PLOT: f64 = 1.0;
/// The largest share of the inner width (inside the outer padding) that one side's x end-label
/// reservation may take.
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

    /// The x coordinate of normalized position `pos` (0 = left).
    pub fn x_at(&self, pos: f64) -> f64 {
        self.x + pos * self.width
    }

    /// The y coordinate of normalized position `pos` (0 = bottom).
    pub fn y_at(&self, pos: f64) -> f64 {
        self.bottom() - pos * self.height
    }
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
    #[allow(dead_code, reason = "read by tests now and by the M5b legend row")]
    pub header_bottom: f64,
    /// Height reserved under the subtitle for a legend row (M5b); 0 in M5a.
    #[allow(dead_code, reason = "the M5b legend row sets and reads it")]
    pub header_extra_height: f64,
    /// The title, if not empty.
    pub title: Option<Label>,
    /// The subtitle, if not empty.
    pub subtitle: Option<Label>,
    /// The x axis label, if not empty.
    pub xlabel: Option<Label>,
    /// The y axis label, if not empty.
    pub ylabel: Option<Label>,
    /// The x tick labels.
    pub x_tick_labels: Vec<Label>,
    /// The y tick labels; the baseline's label is left out for `AboveGridline`.
    pub y_tick_labels: Vec<Label>,
    /// The y coordinates of horizontal gridlines, without the baseline's.
    pub y_grid: Vec<f64>,
    /// The x coordinates of vertical gridlines; none on a category axis.
    pub x_grid: Vec<f64>,
    /// The y coordinate of the baseline.
    pub baseline_y: f64,
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
    skip: impl Fn(f64) -> bool,
) -> Vec<(f64, Label)> {
    ticks
        .iter()
        .filter(|tick| !skip(tick.pos))
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

/// Lays out the axes of a `width` by `height` pt figure for already resolved scales.
/// `reference_width` is the plot width the x end-label reservation measures tick offsets on.
pub(crate) fn lay_out(
    width: f64,
    height: f64,
    theme: &Theme,
    axes: &Axes,
    resolved: &Resolved,
    lib: &FontLibrary,
    reference_width: f64,
) -> Layout {
    let styles = Styles::new(theme);
    let placement = theme.tick_label_placement();
    let header_extra_height = 0.0;

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
        let line = SUBTITLE_LINE * f64::from(styles.body.size);
        label.x = PADDING;
        label.y = baseline_in_line(y, line, ascent, descent);
        y += line;
        label
    });
    y += header_extra_height;
    if title.is_some() || subtitle.is_some() || header_extra_height > 0.0 {
        y += HEADER_GAP;
    }
    let header_bottom = y;

    // Tick labels. For `AboveGridline`, the label of the gridline at the baseline is omitted.
    let (tick_ascent, tick_descent) = metrics(lib, &styles.tick);
    let tick_size = f64::from(styles.tick.size);
    let above = placement == TickLabelPlacement::AboveGridline;
    let baseline = resolved.baseline;
    let mut y_labels = measure_ticks(lib, &resolved.y.ticks, styles.tick, |pos| {
        above && coincides(pos, baseline)
    });
    let mut x_labels = measure_ticks(lib, &resolved.x.ticks, styles.tick, |_| false);
    let y_label_width = y_labels
        .iter()
        .map(|(_, label)| label.width())
        .fold(0.0, f64::max);

    // How far the y tick label of a tick at the plot top reaches above it.
    let label_row_rise = if y_labels.is_empty() {
        0.0
    } else if above {
        ABOVE_GRIDLINE_GAP + tick_ascent
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
    // The first and last x labels are centered on their ticks. Where a label's half width
    // reaches past the plot edge by more than the outer padding, the plot is inset by the
    // difference, so the label stays on the canvas. The tick's distance from the plot edge is
    // measured on `reference_width` (the previous pass's plot width). Each side's reservation
    // is capped at a quarter of the inner width; labels wider than that are clipped by the
    // canvas. This is the one case in which an `AboveGridline` plot is inset by more than the
    // outer padding.
    let inner = (width - 2.0 * PADDING).max(0.0);
    let cap = END_LABEL_SHARE * inner;
    let reach = |label: Option<&(f64, Label)>, from_left: bool| {
        label.map_or(0.0, |(pos, l)| {
            let fraction = if from_left { *pos } else { 1.0 - *pos };
            let offset = fraction * reference_width;
            let beyond = l.width() / 2.0 - offset.max(0.0);
            (beyond - PADDING).clamp(0.0, cap)
        })
    };
    let left_reserve = reach(x_labels.first(), true);
    let right_reserve = reach(x_labels.last(), false);
    let plot_left = if above || y_labels.is_empty() {
        PADDING
    } else {
        PADDING + y_label_width + Y_LABEL_GAP
    }
    .max(PADDING + left_reserve);
    let plot_right = width - PADDING - right_reserve;
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
        let tick_y = plot.y_at(*pos);
        if above {
            label.x = plot.x;
            label.y = tick_y - ABOVE_GRIDLINE_GAP;
        } else {
            label.x = plot.x - Y_LABEL_GAP - label.width();
            label.y = tick_y + Y_LABEL_CENTER * tick_size;
        }
    }
    for (pos, label) in &mut x_labels {
        label.x = plot.x_at(*pos) - label.width() / 2.0;
        label.y = plot.bottom() + X_LABEL_GAP + tick_ascent;
    }
    if let Some(label) = &mut xlabel {
        label.x = plot.x + plot.width / 2.0 - label.width() / 2.0;
        label.y = plot.bottom() + x_band + AXIS_LABEL_GAP + body_ascent;
    }
    if let Some(label) = &mut ylabel {
        // Left-aligned with the left edge of the y tick label column.
        label.x = if above || y_labels.is_empty() {
            plot.x
        } else {
            plot.x - Y_LABEL_GAP - y_label_width
        };
        label.y = plot.y - label_row_rise - AXIS_LABEL_GAP;
    }

    let baseline_y = plot.y_at(baseline);
    let y_grid = if resolved.y.is_category() {
        Vec::new()
    } else {
        resolved
            .y
            .ticks
            .iter()
            .filter(|tick| !coincides(tick.pos, baseline))
            .map(|tick| plot.y_at(tick.pos))
            .collect()
    };
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

    Layout {
        plot,
        header_bottom,
        header_extra_height,
        title,
        subtitle,
        xlabel,
        ylabel,
        x_tick_labels: x_labels.into_iter().map(|(_, label)| label).collect(),
        y_tick_labels: y_labels.into_iter().map(|(_, label)| label).collect(),
        y_grid,
        x_grid,
        baseline_y,
    }
}

/// Resolves and lays out the axes in two passes: ticks depend on the plot size, and the plot
/// size depends on the tick label widths. The first pass assumes the plot fills the figure
/// inside the padding; the second uses the plot size of the first, and is final.
pub(crate) fn plan(
    width: f64,
    height: f64,
    theme: &Theme,
    axes: &Axes,
    lib: &FontLibrary,
) -> (Resolved, Layout) {
    let guess_w = (width - 2.0 * PADDING).max(MIN_PLOT);
    let guess_h = (height - 2.0 * PADDING).max(MIN_PLOT);
    let (tx, ty) = resolve::tick_targets(guess_w, guess_h);
    let first = resolve::resolve(axes, tx, ty);
    let first = lay_out(width, height, theme, axes, &first, lib, guess_w);
    let (tx, ty) = resolve::tick_targets(first.plot.width, first.plot.height);
    let resolved = resolve::resolve(axes, tx, ty);
    let layout = lay_out(width, height, theme, axes, &resolved, lib, first.plot.width);
    (resolved, layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{AxisData, LineOptions};
    use crate::test_support::scene::assert_scene_bounded;
    use crate::{Figure, ScaleKind};

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
        plan(
            figure.width_in() * 72.0,
            figure.height_in() * 72.0,
            figure.theme(),
            figure.axes().unwrap(),
            lib(),
        )
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
        assert_eq!(empty.header_extra_height, 0.0);

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

        // AboveGridline insets only by the padding, whatever the labels.
        let mut pigment = figure(Theme::pigment());
        pigment
            .axes_mut()
            .unwrap()
            .ylim(100_000.0, 900_000.0)
            .unwrap();
        assert_eq!(plan_of(&pigment).1.plot.x, PADDING);
    }

    #[test]
    fn above_gridline_omits_the_baseline_label() {
        let labels = |theme: Theme| {
            let mut f = figure(theme);
            f.axes_mut().unwrap().ylim(0.0, 100.0).unwrap();
            let (resolved, layout) = plan_of(&f);
            assert_eq!(resolved.baseline, 0.0);
            // The baseline replaces the gridline at 0 in every theme.
            assert!(
                layout
                    .y_grid
                    .iter()
                    .all(|&y| (y - layout.baseline_y).abs() > 1e-6)
            );
            assert_eq!(layout.y_grid.len() + 1, resolved.y.ticks.len());
            (
                layout
                    .y_tick_labels
                    .iter()
                    .map(|l| l.text.clone())
                    .collect::<Vec<_>>(),
                layout,
            )
        };
        let (pigment, layout) = labels(Theme::pigment());
        assert!(!pigment.contains(&"0".to_owned()), "{pigment:?}");
        assert!(pigment.contains(&"100".to_owned()), "{pigment:?}");
        // Labels sit left-aligned, 3 pt above their gridline.
        for label in &layout.y_tick_labels {
            assert_eq!(label.x, layout.plot.x);
            assert!(
                layout
                    .y_grid
                    .iter()
                    .any(|&y| (y - 3.0 - label.y).abs() < 1e-9)
            );
        }
        let (graphite, _) = labels(Theme::graphite());
        assert_eq!(graphite.first().map(String::as_str), Some("0"));
        assert_eq!(graphite.len(), pigment.len() + 1);
    }

    #[test]
    fn tiny_figures_and_long_text_lay_out() {
        let long: String = "Station output ".repeat(34).chars().take(500).collect();
        assert_eq!(long.chars().count(), 500);
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            for scale in [1.0, 10.0] {
                let theme = theme.with_font_scale(scale).unwrap();
                let mut f = Figure::new(1.0, 1.0, theme).unwrap();
                f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
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
                let scene = f.to_scene(lib());
                assert_scene_bounded(&scene);
            }
        }

        // Text far wider than any figure is cut to a prefix that fits.
        let huge = "W".repeat(20_000);
        let mut f = Figure::new(1.0, 1.0, Theme::pigment().with_font_scale(10.0).unwrap()).unwrap();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .title(&huge)
            .line(
                AxisData::Categories(vec![huge.clone()]),
                &[1.0],
                LineOptions::default(),
            )
            .unwrap();
        let (_, layout) = plan_of(&f);
        let title = layout.title.as_ref().unwrap();
        assert!(title.width() <= MAX_TEXT_WIDTH && title.width() > MAX_TEXT_WIDTH * 0.9);
        assert!(huge.starts_with(&title.text));
        assert_scene_bounded(&f.to_scene(lib()));
    }

    #[test]
    fn tick_targets_follow_plot_size() {
        // The second pass uses the first pass's plot size: a taller figure gets more y ticks.
        let ticks = |w: f64, h: f64| {
            let mut f = Figure::new(w, h, Theme::graphite()).unwrap();
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .ylim(0.0, 100.0)
                .unwrap()
                .xlim(0.0, 100.0)
                .unwrap();
            let (resolved, layout) = plan_of(&f);
            let targets = resolve::tick_targets(layout.plot.width, layout.plot.height);
            (resolved.x.ticks.len(), resolved.y.ticks.len(), targets)
        };
        let (sx, sy, small) = ticks(2.0, 1.5);
        let (lx, ly, large) = ticks(12.0, 9.0);
        assert_eq!(small, (3, 3));
        assert_eq!(large, (8, 8));
        assert!(lx > sx && ly > sy, "{sx} {sy} {lx} {ly}");
    }

    #[test]
    fn six_by_four_figures_get_five_ticks_per_axis() {
        // Calibrated against the mocks: y 0/25/50/75/100 and x 0/10/20/30 in every theme.
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
                    crate::ScatterOptions::default(),
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
    fn huge_end_labels_keep_the_plot_on_the_canvas() {
        let (w, h) = (6.0 * 72.0, 4.0 * 72.0);
        let inner = w - 2.0 * PADDING;
        let (wide, narrow) = ("W".repeat(500), "V".repeat(500));
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            let plot = category_plot(theme, [&wide, "Q2", &narrow]);
            assert!(plot.x >= 0.0 && plot.right() <= w, "{plot:?}");
            assert!(plot.y >= 0.0 && plot.bottom() <= h, "{plot:?}");
            assert!(plot.width >= 0.5 * inner - 1e-9, "{plot:?}");
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
                assert_eq!(resolved.y.ticks.first().map(|t| t.pos), Some(0.0));
                assert_eq!(resolved.y.ticks.last().map(|t| t.pos), Some(1.0));
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
}
