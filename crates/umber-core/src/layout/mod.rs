//! Layout: text measurement, then the rectangles of the header, plot area, tick labels and axis
//! labels. All lengths are in points, with the origin at the top-left and y pointing down.
//! Layout also sets how many ticks a plot area of a given size asks for.
//!
//! Every derived value is finite: font sizes are clamped, text wider than
//! [`MAX_TEXT_WIDTH`] is cut to a prefix that fits, and the plot area is at least [`MIN_PLOT`]
//! by [`MIN_PLOT`] and lies on the canvas. Text that does not fit, such as a title wider than
//! the figure, is clipped by the canvas.
//! A figure whose y tick labels and padding leave the plot less than [`MIN_PLOT`] wide fails the
//! layout with `Error::Style`. Every x tick label keeps [`X_LABEL_EDGE_GAP`] from the canvas
//! edges; x tick labels that would need a larger plot inset than [`END_LABEL_SHARE`] allows, or
//! a plot narrower than [`MIN_PLOT`], fail the layout with `Error::Style` too. So does a figure
//! whose header, x tick labels, axis labels and padding leave the plot less than [`MIN_PLOT`]
//! tall.
//! With figures of at most 200 in a side this keeps every coordinate far below the renderers'
//! `MAX_COORD`.
//!
//! Every panel of a figure is laid out by one solve ([`grid::solve_grid`]): each panel is
//! measured ([`panel::measure_panel`]), the solve chooses every plot area at once, and each
//! panel is placed for its plot area ([`panel::place_panel`]).

mod colorbar;
mod grid;
mod inset;
mod legend;
mod nudge;
mod panel;
pub(crate) mod placement;
mod program;

pub(crate) use colorbar::{ColorbarSide, MeasuredColorbar, PlacedColorbar, SharedColorbar};
pub(crate) use grid::{GridInput, GridSolution, check_panel_contents, solve_grid};
pub(crate) use legend::{DirectLabel, EndDot, LegendRow, MeasuredLegend, RowEntry, SwatchBox};
pub(crate) use panel::{FigureHeader, Frame, PanelBands, measure_panel, place_panel};

use std::cell::RefCell;
use std::collections::HashMap;

use crate::error::{number, quoted};
use crate::model::Axes;
use crate::resolve::{AxisTick, Baseline, NormalizedPos, ValueAxis};
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
/// The smallest plot side. Layout fails for a figure that leaves the plot less room than this
/// in either direction; [`fit_span`] keeps the plot at least this wide against rounding, and
/// the x tick label solve in [`inset`] places x tick labels for a plot at least this wide.
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
    /// Height, at least [`MIN_PLOT`] for the plot area (layout fails otherwise).
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
    /// Where the header ends, gap included, as an absolute canvas y; the header's top when the
    /// header is empty.
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
    /// The x coordinates of vertical gridlines, without the baseline's; none on a category axis.
    pub x_grid: Vec<f64>,
    /// The baseline, across the plot area.
    pub baseline: PlacedBaseline,
    /// The part of the canvas that belongs to the axes: the whole canvas for a figure of one
    /// axes; on a grid, the axes' cells with their bands, reaching halfway into the gap next to
    /// a neighbouring cell and to the canvas edge on the grid's outer sides.
    pub frame: Frame,
    /// The visible colorbar, placed outside the plot.
    pub colorbar: Option<PlacedColorbar>,
}

/// The baseline as laid out: the line across the plot area at the baseline's position along
/// its axis, the value axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlacedBaseline {
    /// The axis the baseline sits on: y for a horizontal line, x for a vertical one.
    pub axis: ValueAxis,
    /// The plot coordinate of the baseline along that axis: a y for a baseline on the y axis,
    /// an x for one on the x axis.
    pub at: f64,
}

impl PlacedBaseline {
    /// The baseline `baseline` placed on the plot area `plot`.
    pub fn on(plot: &Area, baseline: Baseline) -> PlacedBaseline {
        let at = match baseline.axis {
            ValueAxis::X => plot.x_at(baseline.pos),
            ValueAxis::Y => plot.y_at(baseline.pos),
        };
        PlacedBaseline {
            axis: baseline.axis,
            at,
        }
    }
}

impl Layout {
    /// The y coordinate of a baseline on the y axis, for tests of vertical marks.
    #[cfg(test)]
    #[track_caller]
    pub fn baseline_y(&self) -> f64 {
        assert_eq!(self.baseline.axis, ValueAxis::Y, "a baseline on the x axis");
        self.baseline.at
    }

    /// The free room left of the plot area: up to the right edge of the furthest-reaching y
    /// tick label, or to the frame's left edge when there are no y tick labels. Never negative.
    pub fn room_left(&self) -> f64 {
        let edge = self
            .y_tick_labels
            .iter()
            .map(|label| label.x + label.width())
            .fold(self.frame.x, f64::max);
        (self.plot.x - edge).max(0.0)
    }

    /// The free room right of the plot area, up to the frame's right edge. Never negative.
    pub fn room_right(&self) -> f64 {
        (self.frame.right() - self.plot.right()).max(0.0)
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

/// The span `start..end` kept within `low..high`, the frame's extent: the start is clamped to
/// `low..=high - MIN_PLOT`, and the span is at least [`MIN_PLOT`] and ends within the frame.
/// Only a frame narrower than [`MIN_PLOT`] cannot hold it; then the span is
/// `low..low + MIN_PLOT`. Returns the start and the length.
fn fit_span(start: f64, end: f64, low: f64, high: f64) -> (f64, f64) {
    let start = start.clamp(low, (high - MIN_PLOT).max(low));
    let end = end.min(high).max(start + MIN_PLOT);
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

/// The least gap between adjacent automatic tick labels along their axis, in tick label sizes:
/// half the size, so that neighbouring labels never read as one.
const TICK_LABEL_GAP: f64 = 0.5;

/// Whether automatic ticks keep their labels apart on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TickSpacing {
    /// Ticks are chosen for their target alone. The first layout pass measures with them: its
    /// plot size is a guess, so the spacing it would give is not the drawn one.
    Target,
    /// The previous constrained search, including target retries, with its final choice
    /// checked on the solved layout rather than on the size used to choose it.
    Deferred,
    /// Adjacent labels stay apart (see [`TickRoom`]).
    LabelsApart,
}

/// How tick labels take room along their axis.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LabelExtent {
    /// Side by side under the plot: each label takes its own width, at least that of a
    /// tabular digit, which every automatic label holds; this is that width.
    Width(f64),
    /// One above the other left of the plot: each label takes one line, this high (ascent and
    /// descent).
    Line(f64),
}

/// What automatic ticks along one side of a plot area ask for: about `target` ticks
/// ([`tick_targets`]), whose labels stay apart on screen when `spacing` says so, and no more
/// ticks than [`at_most`](TickRoom::at_most) allows.
///
/// Adjacent labels are apart when they leave at least [`TICK_LABEL_GAP`] tick label sizes
/// between them: between their line boxes (ascent and descent) on the y axis, and between their
/// measured widths, centered on their ticks, on the x axis.
pub(crate) struct TickRoom<'a> {
    /// The preferred number of ticks.
    pub target: usize,
    /// Whether the labels keep apart.
    spacing: TickSpacing,
    /// The length of the axis on screen, in points.
    length: f64,
    /// How the labels take room along the axis.
    extent: LabelExtent,
    /// The fonts.
    lib: &'a FontLibrary,
    /// The tick label style.
    style: TextStyle,
    /// The widths of the labels measured so far: tick searches judge the same labels often.
    widths: RefCell<HashMap<String, f64>>,
    /// The most ticks that fit, whatever their labels; `None` for no such limit.
    most: Option<usize>,
}

/// The tick rooms of the x and the y axis of a plot area of `plot`, with tick labels in the
/// tick label style of `theme`, spaced as `spacing` says.
pub(crate) fn tick_rooms<'a>(
    plot: PlotSize,
    theme: &Theme,
    lib: &'a FontLibrary,
    spacing: TickSpacing,
) -> (TickRoom<'a>, TickRoom<'a>) {
    let (x_target, y_target) = tick_targets(plot);
    let style = Styles::new(theme).tick;
    let (ascent, descent) = metrics(lib, &style);
    let digit = measure(lib, "0", style).width();
    let room = |target, length, extent| TickRoom {
        target,
        spacing,
        length,
        extent,
        lib,
        style,
        widths: RefCell::new(HashMap::new()),
        most: None,
    };
    (
        room(x_target, plot.width, LabelExtent::Width(digit)),
        room(y_target, plot.height, LabelExtent::Line(ascent + descent)),
    )
}

impl TickRoom<'_> {
    /// Whether the final constrained choice is checked on the solved layout.
    pub fn deferred(&self) -> bool {
        self.spacing == TickSpacing::Deferred
    }

    /// This room with at most `most` ticks fitting, whatever their labels.
    pub fn at_most(self, most: usize) -> Self {
        TickRoom {
            most: Some(most),
            ..self
        }
    }

    /// The least gap between adjacent labels, in points.
    fn gap(&self) -> f64 {
        TICK_LABEL_GAP * f64::from(self.style.size)
    }

    /// The width of `label`.
    fn width(&self, label: &str) -> f64 {
        if let Some(&width) = self.widths.borrow().get(label) {
            return width;
        }
        let width = measure(self.lib, label, self.style).width();
        self.widths.borrow_mut().insert(label.to_owned(), width);
        width
    }

    /// The least distance between adjacent ticks, as a share of the axis length, that can keep
    /// any automatic labels apart: a tabular digit and the gap on the x axis, a line and the
    /// gap on the y axis; 0 when the labels need not keep apart.
    pub fn least_share(&self) -> f64 {
        if self.spacing == TickSpacing::Target {
            return 0.0;
        }
        let (LabelExtent::Width(least) | LabelExtent::Line(least)) = self.extent;
        (least + self.gap()) / self.length
    }

    /// The room, in points, that the labels of `ticks`, in axis order, leave between the most
    /// crowded pair of adjacent labels beyond the least gap: negative when they crowd, infinite
    /// for fewer than two ticks and when the labels need not keep apart, and negative infinity
    /// for more ticks than [`at_most`](Self::at_most) allows.
    pub fn slack(&self, ticks: &[AxisTick]) -> f64 {
        if self.spacing == TickSpacing::Target {
            return f64::INFINITY;
        }
        if self.most.is_some_and(|most| ticks.len() > most) {
            return f64::NEG_INFINITY;
        }
        let extents: Vec<f64> = match self.extent {
            LabelExtent::Width(_) => ticks.iter().map(|tick| self.width(&tick.label)).collect(),
            LabelExtent::Line(line) => vec![line; ticks.len()],
        };
        let gap = self.gap();
        ticks
            .windows(2)
            .zip(extents.windows(2))
            .map(|(pair, extent)| {
                let distance = (pair[1].pos.get() - pair[0].pos.get()).abs() * self.length;
                distance - (extent[0] + extent[1]) / 2.0 - gap
            })
            .fold(f64::INFINITY, f64::min)
    }
}

#[cfg(test)]
impl<'a> TickRoom<'a> {
    /// Tick rooms for ticks chosen for about `x_target` and `y_target` alone.
    pub(crate) fn unlimited(
        x_target: usize,
        y_target: usize,
        lib: &'a FontLibrary,
    ) -> (TickRoom<'a>, TickRoom<'a>) {
        let size = PlotSize {
            width: 400.0,
            height: 300.0,
        };
        let (x, y) = tick_rooms(size, &Theme::pigment(), lib, TickSpacing::Target);
        (
            TickRoom {
                target: x_target,
                ..x
            },
            TickRoom {
                target: y_target,
                ..y
            },
        )
    }
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
    /// The theme.
    pub theme: &'a Theme,
    /// The axes, for their texts and cells.
    pub axes: &'a Axes,
    /// The legend, measured by [`MeasuredLegend::measure`].
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

/// The hint of the error for a figure too narrow for the plot area.
const FIGURE_WIDTH_HINT: &str = "widen the figure.";

/// The hint of the errors for legend labels that do not fit their panel or figure.
const LEGEND_HINT: &str = "shorten the label or widen the figure.";

/// The text's width and the room it has, in whole points.
fn widths(width: f64, room: f64) -> (impl std::fmt::Display, impl std::fmt::Display) {
    let room = room.floor();
    (number(width.round().max(room + 1.0)), number(room))
}

/// A legend label wider than the room left after its swatch and any chip padding.
fn legend_too_wide(context: &'static str, label: &Label, room: f64, container: &str) -> Error {
    let (w, p) = widths(label.width(), room);
    Error::style(
        context,
        format!(
            "the legend label {} is {w} pt wide; the {container} has {p} pt.",
            quoted(&label.text)
        ),
    )
    .with_hint(LEGEND_HINT)
}

/// The hint of the error for a figure too short for the plot area, naming only the measured
/// header parts and, for a panel below a figure header, that figure header too.
fn figure_height_hint(
    title: bool,
    subtitle: bool,
    legend: bool,
    figure_header: bool,
) -> &'static str {
    match (figure_header, title, subtitle, legend) {
        (false, false, false, false) => "make the figure taller.",
        (false, true, false, false) => "make the figure taller, or remove the title.",
        (false, false, true, false) => "make the figure taller, or remove the subtitle.",
        (false, false, false, true) => "make the figure taller, or remove the legend.",
        (false, true, true, false) => "make the figure taller, or remove the title or subtitle.",
        (false, true, false, true) => "make the figure taller, or remove the title or legend.",
        (false, false, true, true) => "make the figure taller, or remove the subtitle or legend.",
        (false, true, true, true) => {
            "make the figure taller, or remove the title, subtitle or legend."
        }
        (true, false, false, false) => "make the figure taller, or remove the figure header.",
        (true, true, false, false) => {
            "make the figure taller, or remove the figure header or title."
        }
        (true, false, true, false) => {
            "make the figure taller, or remove the figure header or subtitle."
        }
        (true, false, false, true) => {
            "make the figure taller, or remove the figure header or legend."
        }
        (true, true, true, false) => {
            "make the figure taller, or remove the figure header, title or subtitle."
        }
        (true, true, false, true) => {
            "make the figure taller, or remove the figure header, title or legend."
        }
        (true, false, true, true) => {
            "make the figure taller, or remove the figure header, subtitle or legend."
        }
        (true, true, true, true) => {
            "make the figure taller, or remove the figure header, title, subtitle or legend."
        }
    }
}

/// The plot area of the only panel of a figure of one cell, `input`, in `frame`, the whole
/// canvas, for the texts measured in `bands`.
///
/// The header is stacked from the top padding, and the plot starts below it, the y label and
/// the rise of the top y tick label. The plot starts right of the y tick labels and ends left
/// of the room direct labels reserve. The x tick labels are centered on their ticks; they may
/// reach into the outer padding, up to [`X_LABEL_EDGE_GAP`] from the canvas edge. Where one
/// would reach further, the plot edge on that side moves in by exactly the overflow. Each side
/// moves in by at most [`END_LABEL_SHARE`] of the inner width beyond the padding; labels that
/// need more are an error. Direct labels reserve room under the same cap. Under the plot are
/// the x tick labels, the x label and the bottom padding. A Bottom legend sits below the
/// greater of the x label bands and the lowest y tick label's drop, with its gap and height
/// reserved before the bottom padding.
///
/// The canvas starts at 0, so without a Bottom legend every sum here is evaluated as for a
/// figure of one axes before figures had grids, and the plot area is the same to the bit.
///
/// # Errors
///
/// `Error::Style` in `input.context`, checked in this order:
/// - the room between the y tick labels and the right padding (less what direct labels
///   reserve) is narrower than [`MIN_PLOT`], with or without x tick labels: the message names
///   the widest y tick label when narrower ones would leave room, and otherwise the width the
///   canvas needs (see [`no_room_for_plot`]);
/// - the x tick labels cannot all keep within their bounds on a plot at least [`MIN_PLOT`]
///   wide, with each side of the plot inset by at most [`END_LABEL_SHARE`] of the inner width:
///   the message names the widest x tick label, its width and the width up to which labels fit;
/// - the room between the header (title, subtitle and legend row), the y label and the y tick
///   label rising above the top tick, and the bottom bands is shorter than [`MIN_PLOT`]. The
///   bottom bands include the x tick labels, the x label and the bottom padding; with a Bottom
///   legend they also reserve the lowest y tick label's drop when it reaches further, then the
///   legend's gap and height. The message names the height the canvas needs, and the hint names
///   the figure header, when present, the header parts the axes have and any Bottom legend (see
///   [`figure_too_short`]).
fn solve_single(
    input: &LayoutInput<'_>,
    bands: &panel::PanelBands,
    frame: Frame,
    top: f64,
) -> Result<Area> {
    let LayoutInput {
        legend, context, ..
    } = *input;
    let mut plot_top = bands.heading.bottom(top) + bands.top_label_band() + bands.top_label_rise();
    let cap = panel::end_label_cap(frame.width);
    // The widest y tick label and its gap are added to the padding one at a time, not as the
    // one sum `bands.y_tick_column`, which can round differently.
    let left = if bands.y_labels.is_empty() {
        frame.x + PADDING
    } else {
        frame.x + PADDING + bands.y_label_width + Y_LABEL_GAP
    };
    let right = frame.right() - PADDING - bands.right();
    let room = inset::Room {
        left,
        right,
        left_max: left.max(frame.x + PADDING + cap),
        right_min: right.min(frame.right() - PADDING - cap),
        lo: frame.x + X_LABEL_EDGE_GAP,
        hi: frame.right() - X_LABEL_EDGE_GAP,
        min_width: MIN_PLOT,
    };
    // Every layout first needs a plot at least `MIN_PLOT` wide between the y tick labels and
    // the right padding, x tick labels or not; only then are the x tick labels fitted.
    if matches!(inset::fit_x_labels(&room, &[]), inset::XInset::Overflowing) {
        let direct = legend.direct.reserve(f64::INFINITY);
        if bands.right_colorbar > 0.0 && right - left + bands.right_colorbar >= MIN_PLOT {
            let available = frame.right()
                - PADDING
                - left
                - MIN_PLOT
                - bands.direct_reserve
                - bands.right_legend;
            return Err(colorbar::room_error(
                context,
                bands.right_colorbar,
                available,
                false,
            ));
        }
        if bands.right_legend > 0.0 || bands.right_colorbar > 0.0 {
            if bands.right_legend > 0.0 {
                let available = right - left - MIN_PLOT + bands.right_legend;
                placement::check_right_width(context, &legend.column(0.0), available)?;
            }
            let y_room = right - MIN_PLOT - (frame.x + PADDING) - Y_LABEL_GAP;
            if let Some(label) = widest(&bands.y_labels)
                && y_room >= 0.0
            {
                return Err(y_labels_leave_no_room(context, label, y_room));
            }
            return Err(figure_too_small(
                context,
                Direction::Width,
                frame.width,
                2.0 * PADDING + bands.y_tick_column + bands.right() + MIN_PLOT,
                FIGURE_WIDTH_HINT,
            ));
        }
        return Err(no_room_for_plot(
            context,
            frame,
            &bands.y_labels,
            &room,
            direct,
        ));
    }
    let (plot_left, plot_right) = match inset::fit_x_labels(&room, &bands.x_spans) {
        inset::XInset::Contained { left, right } => (left, right),
        inset::XInset::Overflowing => {
            return Err(x_labels_overflow(
                context,
                &bands.x_labels,
                &room,
                &bands.x_spans,
                bands.colorbar_tick_width(),
            ));
        }
    };
    // Everything under the plot: the x tick labels, the x label and the bottom padding.
    if legend.position == crate::LegendPosition::TopRight {
        plot_top = top + bands.above_with_row(&legend.header_within(plot_left, plot_right));
    }
    let axis_below =
        if bands.bottom_legend > 0.0 || bands.bottom_colorbar > 0.0 || bands.right_colorbar > 0.0 {
            bands.axis_below()
        } else {
            // Keep the original single-panel extent when no bottom legend needs the label drop.
            bands.x_band + bands.x_title_band
        };
    let below = axis_below + PADDING + bands.bottom_legend + bands.bottom_colorbar;
    let plot_bottom = frame.bottom() - below;
    let plot_height = plot_bottom - plot_top;
    if plot_height < MIN_PLOT {
        let without = bands.without_colorbar(cap);
        let plain_top = if legend.position == crate::LegendPosition::TopRight {
            top + without.above_with_row(&legend.header_within(plot_left, plot_right))
        } else {
            without.heading.bottom(top) + without.y_label_band + without.label_rise
        };
        let plain_below = PADDING
            + without.bottom_legend
            + if without.bottom_legend > 0.0 {
                without.axis_below()
            } else {
                without.x_band + without.x_title_band
            };
        let plain_height = frame.bottom() - plain_below - plain_top;
        let colorbar_band = (plot_top - plain_top) + (below - plain_below);
        if colorbar_band > 0.0 && plain_height >= MIN_PLOT {
            return Err(colorbar::room_error(
                context,
                colorbar_band,
                plain_height - MIN_PLOT,
                true,
            ));
        }
        return Err(figure_too_short(
            context,
            frame.height,
            plot_top - frame.y + MIN_PLOT + below,
            bands
                .heading
                .height_hint(top > frame.y + PADDING, bands.bottom_legend > 0.0),
        ));
    }
    let (x, plot_width) = fit_span(plot_left, plot_right, frame.x, frame.right());
    let plot = Area {
        x,
        y: plot_top,
        width: plot_width,
        height: plot_height,
    };
    // The solve keeps the labels within the bounds for a plot at least `MIN_PLOT` wide, which
    // `fit_span` leaves in place; this holds the final placement, where `place_panel` puts the
    // labels, to the same bounds.
    let outside = bands.x_labels.iter().any(|(pos, label)| {
        let x = plot.x_at(*pos) - label.width() / 2.0;
        x < room.lo - inset::CONTAINMENT_TOLERANCE
            || x + label.width() > room.hi + inset::CONTAINMENT_TOLERANCE
    });
    if outside {
        return Err(x_labels_overflow(
            context,
            &bands.x_labels,
            &room,
            &bands.x_spans,
            bands.colorbar_tick_width(),
        ));
    }
    Ok(plot)
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

/// The error for a `room` that leaves the plot less than [`MIN_PLOT`] in `frame`, the canvas
/// for a figure of one axes, whose direct labels would reserve `direct` pt without the cap.
///
/// If y tick labels no wider than some width would leave the plot [`MIN_PLOT`], they are the
/// cause: the error names the widest y tick label (see [`y_labels_leave_no_room`]). Otherwise
/// the canvas is too narrow even for y tick labels of no width, or without y tick labels, and
/// the error names the width the frame needs (see [`figure_too_narrow`]).
fn no_room_for_plot(
    context: &'static str,
    frame: Frame,
    y_labels: &[(NormalizedPos, Label)],
    room: &inset::Room,
    direct: f64,
) -> Error {
    let inner_left = frame.x + PADDING;
    // The widest y tick label column that leaves the plot `MIN_PLOT` wide.
    let y_room = room.right - MIN_PLOT - inner_left - Y_LABEL_GAP;
    match widest(y_labels) {
        Some(label) if y_room >= 0.0 => y_labels_leave_no_room(context, label, y_room),
        _ => figure_too_narrow(context, frame.width, room.left - inner_left, direct),
    }
}

/// The narrowest canvas, in points, that leaves the plot [`MIN_PLOT`] wide next to a y tick
/// label column `column` pt wide (its gap included) and direct labels that would reserve
/// `direct` pt without the cap.
///
/// The plot gets the inner width `I` (the canvas inside the outer padding) less the column and
/// the reserve, which is capped at [`END_LABEL_SHARE`] of `I`. That is increasing in `I`, so the
/// narrowest canvas solves `I - column - min(direct, END_LABEL_SHARE × I) = MIN_PLOT`.
fn min_canvas_width(column: f64, direct: f64) -> f64 {
    let need = MIN_PLOT + column;
    // With the reserve capped: (1 - share) I = need, valid when the cap is at most `direct`.
    let capped = need / (1.0 - END_LABEL_SHARE);
    let inner = if END_LABEL_SHARE * capped <= direct {
        capped
    } else {
        need + direct
    };
    2.0 * PADDING + inner
}

/// The error for a canvas `width` pt wide that leaves no room for the plot area next to a y
/// tick label column `column` pt wide (0 without y tick labels) and direct labels that would
/// reserve `direct` pt (see [`figure_too_small`]).
fn figure_too_narrow(context: &'static str, width: f64, column: f64, direct: f64) -> Error {
    figure_too_small(
        context,
        Direction::Width,
        width,
        min_canvas_width(column, direct),
        FIGURE_WIDTH_HINT,
    )
}

/// The error for a canvas `height` pt tall that leaves no room for the plot area: it needs
/// `needed` pt, the plot's top edge plus [`MIN_PLOT`] plus everything under the plot (see
/// [`figure_too_small`]).
fn figure_too_short(context: &'static str, height: f64, needed: f64, hint: &'static str) -> Error {
    figure_too_small(context, Direction::Height, height, needed, hint)
}

/// A direction in which a figure can be too small for its plot area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// Across: the figure's width.
    Width,
    /// Down: the figure's height.
    Height,
}

/// The error for a canvas whose side in `direction` is `side` pt but needs at least `needed`
/// pt for the plot area. The sides are in whole points: the figure's rounded down and the one
/// needed rounded up, and at least one point more, so the two never look equal.
fn figure_too_small(
    context: &'static str,
    direction: Direction,
    side: f64,
    needed: f64,
    hint: &'static str,
) -> Error {
    let along = match direction {
        Direction::Width => "wide",
        Direction::Height => "tall",
    };
    let shown = side.floor();
    Error::style(
        context,
        format!(
            "the figure is {} pt {along}; at least {} pt is needed for the plot area.",
            number(shown),
            number(needed.ceil().max(shown + 1.0)),
        ),
    )
    .with_hint(hint)
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
    colorbar_width: f64,
) -> Error {
    let fits = inset::widest_label_that_fits(room, spans);
    if colorbar_width > fits {
        return colorbar::room_error(context, colorbar_width, fits, false);
    }
    x_labels_wider_than(context, labels, fits)
}

/// The error for x tick `labels` that cannot fit, when labels up to `fits` pt wide would: as
/// [`x_labels_overflow`].
fn x_labels_wider_than(
    context: &'static str,
    labels: &[(NormalizedPos, Label)],
    fits: f64,
) -> Error {
    // Without labels the room's own edges fit: layout checks that first.
    let (text, label_width) = widest(labels).map_or(("", 0.0), |l| (l.text.as_str(), l.width()));
    let fits = fits.floor();
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
    use crate::model::{AxesMut, AxisData, Figure, LineOptions, ScatterOptions};
    use crate::pipeline::plan_figure;
    use crate::resolve::Resolved;
    use crate::test_support::named_height;
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
        plan_figure(figure, (0, 0), lib())
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

        f.single_axes_mut().title("Station output");
        let title = plan_of(&f).1;
        assert!(close(title.header_bottom, PADDING + 1.25 * ts + 8.0));
        assert!(title.subtitle.is_none());

        f.single_axes_mut().subtitle("Daily mean, MW");
        let both = plan_of(&f).1;
        assert!(close(
            both.header_bottom,
            PADDING + 1.25 * ts + 2.0 + 1.35 * bs + 8.0
        ));
        let (t, s) = (both.title.unwrap(), both.subtitle.unwrap());
        assert!(t.y < s.y && t.x == PADDING && s.x == PADDING);

        f.single_axes_mut().title("");
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
        short.single_axes_mut().ylim(0.0, 1.0).unwrap();
        let mut long = figure(Theme::graphite());
        long.single_axes_mut().ylim(100_000.0, 900_000.0).unwrap();
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
            f.single_axes_mut().ylim(0.0, 100.0).unwrap();
            let (resolved, layout) = plan_of(&f);
            assert_eq!(resolved.baseline.pos.get(), 0.0);
            // The baseline replaces the gridline at 0.
            assert!(
                layout
                    .y_grid
                    .iter()
                    .all(|&y| (y - layout.baseline_y()).abs() > 1e-6)
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
                // Without ticks: x tick labels do not fit a 1 in figure at these font scales
                // (see `labels_wider_than_the_capped_room_fail_naming_the_widest`), and y tick
                // labels leave no room for the plot (see
                // `y_labels_that_leave_no_room_for_the_plot_fail_naming_the_widest`).
                let build = |height: f64| {
                    let mut f = Figure::new(1.0, height, theme.clone()).unwrap();
                    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                        .unwrap()
                        .xticks(&[])
                        .unwrap()
                        .yticks(&[])
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
                    f
                };
                // A 1 in square has no room for the header and the axis labels; a figure as tall
                // as the error names has.
                let need = named_height(&overflow(&build(1.0))).unwrap();
                let f = build(need / 72.0);
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
        let build = |height: f64| {
            let mut f = Figure::new(1.0, height, theme.clone()).unwrap();
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .xticks(&[])
                .unwrap()
                .yticks(&[])
                .unwrap()
                .title(&huge)
                .line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[1.0, 2.0],
                    LineOptions::default(),
                )
                .unwrap();
            f
        };
        let f = build(named_height(&overflow(&build(1.0))).unwrap() / 72.0);
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
            f.single_axes_mut()
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
        f.single_axes_mut()
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
        let plot_with = |set: &dyn Fn(&mut AxesMut<'_>)| {
            let mut f = figure(Theme::graphite());
            let mut axes = f.single_axes_mut();
            axes.scatter(
                AxisData::Numbers(vec![0.0, 10.0]),
                &[0.0, 10.0],
                ScatterOptions::default(),
            )
            .unwrap();
            set(&mut axes);
            plan_of(&f).1
        };
        let short = plot_with(&|a: &mut AxesMut<'_>| {
            a.xticks_labeled(&[0.0, 10.0], vec!["a".into(), "b".into()])
                .unwrap()
                .yticks_labeled(&[0.0, 10.0], vec!["a".into(), "b".into()])
                .unwrap();
        });
        let long = plot_with(&|a: &mut AxesMut<'_>| {
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
        let suffixed = plot_with(&|a: &mut AxesMut<'_>| {
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
        f.single_axes_mut()
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
            f.single_axes_mut()
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
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.xlim(0.0, 1.0)
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
        crate::pipeline::try_plan_figure(f, (0, 0), lib()).unwrap_err()
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
        // The y tick labels fail the layout whatever the x ticks: without any, or with one
        // empty label.
        let mut none = end_after_wide_y_labels(100);
        none.single_axes_mut().xticks(&[]).unwrap();
        let mut empty = end_after_wide_y_labels(100);
        empty
            .single_axes_mut()
            .xticks_labeled(&[0.5], vec![String::new()])
            .unwrap();
        for f in [none, empty] {
            let other = overflow(&f);
            assert_eq!(other.to_string(), err.to_string());
            assert_eq!(other.hint(), err.hint());
        }
    }

    #[test]
    fn a_figure_too_narrow_for_the_plot_fails_naming_the_width_it_needs() {
        // 0.3 in is 21.6 pt; without tick labels the plot needs 2 × 12 + 1 = 25 pt.
        let mut f = Figure::new(0.3, 1.0, Theme::midnight()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xticks(&[])
            .unwrap()
            .yticks(&[])
            .unwrap();
        let err = overflow(&f);
        assert_eq!(
            err.to_string(),
            "Figure.to_scene(): the figure is 21 pt wide; at least 25 pt is needed for the plot \
             area."
        );
        assert!(matches!(err, crate::Error::Style { .. }));
        assert_eq!(err.parameter(), None);
        assert_eq!(err.hint(), Some("widen the figure."));
        // 0.4 in is 28.8 pt. A y tick label of one W, 5.1 pt, and its 6 pt gap need 25 + 11.1
        // = 36.1 pt; even a y tick label of no width would need 31 pt, so the figure is named.
        let mut f = Figure::new(0.4, 1.0, Theme::midnight()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xticks(&[])
            .unwrap()
            .yticks_labeled(&[0.5], vec!["W".into()])
            .unwrap();
        assert_eq!(
            overflow(&f).to_string(),
            "Figure.to_scene(): the figure is 28 pt wide; at least 37 pt is needed for the plot \
             area."
        );
        // 25 pt is enough without tick labels.
        let mut f = Figure::new(25.0 / 72.0, 1.0, Theme::midnight()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xticks(&[])
            .unwrap()
            .yticks(&[])
            .unwrap();
        let (_, layout) = plan_of(&f);
        assert!(
            (layout.plot.width - MIN_PLOT).abs() < 1e-9,
            "{:?}",
            layout.plot
        );
    }

    fn narrow_with_right_legend(grid: bool, wide_y: bool) -> Figure {
        let width = if wide_y { 143.0 } else { 35.0 } / 72.0;
        let mut f = Figure::with_grid(
            width,
            4.0,
            Theme::midnight(),
            crate::Grid::new(if grid { 2 } else { 1 }, 1)
                .unwrap()
                .with_legend(crate::LegendMode::PerPanel),
        )
        .unwrap();
        let mut axes = f
            .add_axes_at(
                crate::Placement {
                    row: crate::GridIndex::At(0),
                    col: crate::GridIndex::At(0),
                    ..crate::Placement::default()
                },
                ScaleKind::Linear,
                ScaleKind::Linear,
            )
            .unwrap();
        axes.xticks(&[]).unwrap();
        if wide_y {
            axes.yticks_labeled(&[0.0], vec!["W".repeat(20)]).unwrap();
        } else {
            axes.yticks(&[]).unwrap();
        }
        axes.legend_at(crate::LegendStyle::Header, crate::LegendPosition::Right)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[0.0, 1.0],
                LineOptions {
                    label: "a".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        f
    }

    #[test]
    fn right_legend_without_text_room_names_the_y_tick_label() {
        for grid in [false, true] {
            let err = overflow(&narrow_with_right_legend(grid, true));
            assert!(
                err.to_string().contains("y tick label \"WWWW"),
                "grid={grid}: {err}"
            );
            assert_eq!(
                err.hint(),
                Some("shorten the y tick labels or widen the figure.")
            );
        }
    }

    #[test]
    fn right_legend_without_text_room_names_the_canvas_width() {
        for grid in [false, true] {
            let err = overflow(&narrow_with_right_legend(grid, false));
            assert!(
                err.to_string().contains("the figure is 35 pt wide"),
                "grid={grid}: {err}"
            );
            assert_eq!(err.hint(), Some("widen the figure."));
        }
    }

    fn bottom_without_x_labels(height: f64) -> Figure {
        let theme = Theme::midnight().with_font_scale(2.0).unwrap();
        let mut f = Figure::new(4.0, height, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xticks(&[])
            .unwrap()
            .ylim(0.0, 1.0)
            .unwrap()
            .yticks_labeled(&[0.0, 1.0], vec!["0".into(), "1".into()])
            .unwrap()
            .legend_at(crate::LegendStyle::Header, crate::LegendPosition::Bottom)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[0.0, 1.0],
                LineOptions {
                    label: "a".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        f
    }

    #[test]
    fn bottom_legend_starts_below_the_lowest_y_tick_label() {
        let f = bottom_without_x_labels(4.0);
        let layout = plan_of(&f).1;
        let label = &layout.y_tick_labels[0];
        let bottom = label.y + metrics(lib(), &label.style).1;
        let entry = &layout.legend.entries()[0];
        let top = entry.label.y - metrics(lib(), &entry.label.style).0;
        assert!(
            top >= bottom + placement::BOTTOM_GAP,
            "legend top {top}, y label bottom {bottom}"
        );
    }

    #[test]
    fn bottom_legend_reserves_the_lowest_y_tick_label_drop() {
        let f = bottom_without_x_labels(4.0);
        let layout = plan_of(&f).1;
        let label = &layout.y_tick_labels[0];
        let bottom = label.y + metrics(lib(), &label.style).1;
        let required = bottom + placement::BOTTOM_GAP + layout.legend.height() + PADDING;
        assert!(
            required <= f.height_in() * 72.0 + 1e-9,
            "required {required}"
        );
    }

    #[test]
    fn bottom_legend_is_named_in_the_height_hint() {
        let err = overflow(&bottom_without_x_labels(0.4));
        assert_eq!(
            err.hint(),
            Some("make the figure taller, or remove the legend.")
        );
    }

    #[test]
    fn the_narrowest_canvas_counts_the_capped_direct_label_reserve() {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        // Without a reserve: the padding, the column and the plot.
        assert!(close(min_canvas_width(0.0, 0.0), 25.0));
        assert!(close(min_canvas_width(11.1, 0.0), 36.1));
        // A small reserve is taken whole: 1 + 0.1 inside the padding.
        assert!(close(min_canvas_width(0.0, 0.1), 25.1));
        // A large one is capped at a quarter of the inner width: 0.75 I = 1.
        assert!(close(min_canvas_width(0.0, 100.0), 24.0 + 4.0 / 3.0));
        // At the boundary both readings agree.
        assert!(close(min_canvas_width(2.0, 1.0), 28.0));
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
            f.single_axes_mut()
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

    /// A `width` by `height` in midnight figure without ticks, with the header parts asked for:
    /// a title, a subtitle, and a header legend row of one labelled line.
    fn bare(width: f64, height: f64, title: bool, subtitle: bool, legend: bool) -> Figure {
        let mut f = Figure::new(width, height, Theme::midnight()).unwrap();
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.xticks(&[])
            .unwrap()
            .yticks(&[])
            .unwrap()
            .legend(crate::LegendStyle::Header);
        if title {
            axes.title("Load");
        }
        if subtitle {
            axes.subtitle("Daily mean");
        }
        if legend {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: "North".into(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        f
    }

    #[test]
    fn a_figure_too_short_for_the_plot_fails_naming_the_height_it_needs() {
        // 0.3 in is 21.6 pt; without tick labels or a header the plot needs 2 × 12 + 1 = 25 pt.
        let err = overflow(&bare(1.0, 0.3, false, false, false));
        assert_eq!(
            err.to_string(),
            "Figure.to_scene(): the figure is 21 pt tall; at least 25 pt is needed for the plot \
             area."
        );
        assert!(matches!(err, crate::Error::Style { .. }));
        assert_eq!(err.parameter(), None);
        assert_eq!(err.hint(), Some("make the figure taller."));
        // 25 pt is enough: the plot is exactly `MIN_PLOT` tall, between the paddings.
        let (_, layout) = plan_of(&bare(1.0, 25.0 / 72.0, false, false, false));
        assert!(
            (layout.plot.height - MIN_PLOT).abs() < 1e-9,
            "{:?}",
            layout.plot
        );
        assert!((layout.plot.y - PADDING).abs() < 1e-9, "{:?}", layout.plot);
    }

    #[test]
    fn the_height_hint_names_only_the_header_parts_the_figure_has() {
        let cases = [
            ((false, false, false), "make the figure taller."),
            (
                (true, false, false),
                "make the figure taller, or remove the title.",
            ),
            (
                (false, true, false),
                "make the figure taller, or remove the subtitle.",
            ),
            (
                (false, false, true),
                "make the figure taller, or remove the legend.",
            ),
            (
                (true, true, false),
                "make the figure taller, or remove the title or subtitle.",
            ),
            (
                (true, false, true),
                "make the figure taller, or remove the title or legend.",
            ),
            (
                (false, true, true),
                "make the figure taller, or remove the subtitle or legend.",
            ),
            (
                (true, true, true),
                "make the figure taller, or remove the title, subtitle or legend.",
            ),
        ];
        for ((title, subtitle, legend), hint) in cases {
            let err = overflow(&bare(2.0, 0.3, title, subtitle, legend));
            assert!(
                err.to_string()
                    .starts_with("Figure.to_scene(): the figure is 21 pt tall; at least "),
                "{err}"
            );
            assert_eq!(err.hint(), Some(hint), "{title} {subtitle} {legend}");
        }
        // Direct labels and no legend take no header room, and the hint does not name them.
        for style in [crate::LegendStyle::Direct, crate::LegendStyle::None] {
            let mut f = bare(2.0, 0.3, true, false, true);
            f.single_axes_mut().legend(style);
            assert_eq!(
                overflow(&f).hint(),
                Some("make the figure taller, or remove the title.")
            );
        }
    }

    #[test]
    fn the_height_hint_names_the_figure_header_and_only_the_panel_parts() {
        let cases = [
            (
                (false, false, false),
                "make the figure taller, or remove the figure header.",
            ),
            (
                (true, false, false),
                "make the figure taller, or remove the figure header or title.",
            ),
            (
                (false, true, false),
                "make the figure taller, or remove the figure header or subtitle.",
            ),
            (
                (false, false, true),
                "make the figure taller, or remove the figure header or legend.",
            ),
            (
                (true, true, false),
                "make the figure taller, or remove the figure header, title or subtitle.",
            ),
            (
                (true, false, true),
                "make the figure taller, or remove the figure header, title or legend.",
            ),
            (
                (false, true, true),
                "make the figure taller, or remove the figure header, subtitle or legend.",
            ),
            (
                (true, true, true),
                "make the figure taller, or remove the figure header, title, subtitle or legend.",
            ),
        ];
        for ((title, subtitle, legend), hint) in cases {
            let mut f = bare(2.0, 0.3, title, subtitle, legend);
            f.title("Figure");
            let err = overflow(&f);
            assert!(matches!(err, crate::Error::Style { .. }));
            assert!(
                err.to_string()
                    .starts_with("Figure.to_scene(): the figure is 21 pt tall; at least "),
                "{err}"
            );
            assert_eq!(err.hint(), Some(hint), "{title} {subtitle} {legend}");
        }
        // A direct or hidden legend still takes no panel header room.
        for style in [crate::LegendStyle::Direct, crate::LegendStyle::None] {
            let mut f = bare(2.0, 0.3, true, false, true);
            f.title("Figure");
            f.single_axes_mut().legend(style);
            assert_eq!(
                overflow(&f).hint(),
                Some("make the figure taller, or remove the figure header or title.")
            );
        }
    }

    #[test]
    fn legend_width_errors_round_up_past_the_available_room() {
        for (room, width, shown) in [(20.2, 21, 20), (19.8, 20, 19), (-0.1, 20, -1)] {
            let (w, p) = widths(20.4, room);
            assert_eq!(format!("{w} {p}"), format!("{width} {shown}"));
        }
        let lib = FontLibrary::with_fallbacks(Vec::new());
        let label = measure(&lib, "WWWW", Styles::new(&Theme::pigment()).body);
        let width = label.width().round();
        for container in ["figure", "panel"] {
            for (room, shown) in [(width - 0.6, width - 1.0), (-0.1, -1.0)] {
                let err = legend_too_wide("Figure.to_scene()", &label, room, container);
                assert!(matches!(err, crate::Error::Style { .. }));
                assert_eq!(
                    err.to_string(),
                    format!(
                        "Figure.to_scene(): the legend label \"WWWW\" is {width} pt wide; \
                         the {container} has {shown} pt."
                    )
                );
                assert_eq!(err.hint(), Some("shorten the label or widen the figure."));
            }
        }
    }

    /// A 6 in wide figure `height` in tall in `theme` with every part that takes height: a title,
    /// a subtitle, axis labels, automatic ticks and three labelled lines shown in `style`.
    fn full(theme: &Theme, style: crate::LegendStyle, height: f64) -> Figure {
        let mut f = Figure::new(6.0, height, theme.clone()).unwrap();
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.title("Station output")
            .subtitle("Daily mean, MW")
            .xlabel("Day")
            .ylabel("MW")
            .legend(style);
        for (i, label) in ["North", "South", "East"].into_iter().enumerate() {
            axes.line(
                AxisData::Numbers(vec![0.0, 30.0]),
                &[i as f64, 100.0 - i as f64],
                LineOptions {
                    label: label.to_owned(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        f
    }

    /// The height, in points, that a figure laid out as `layout` on a canvas `height` pt tall
    /// needs: everything above and below the plot, and a plot `MIN_PLOT` tall.
    fn needed_height(layout: &Layout, height: f64) -> f64 {
        layout.plot.y + MIN_PLOT + (height - layout.plot.bottom())
    }

    #[test]
    fn figures_just_tall_enough_lay_out_and_just_shorter_ones_fail() {
        use crate::LegendStyle;
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            for style in [
                LegendStyle::Header,
                LegendStyle::Chips,
                LegendStyle::Direct,
                LegendStyle::None,
            ] {
                let (_, roomy) = plan_of(&full(&theme, style, 4.0));
                let need = needed_height(&roomy, 4.0 * 72.0);
                // Just fits: the plot is 0.01 pt taller than its minimum.
                let (_, fits) = plan_of(&full(&theme, style, (need + 0.01) / 72.0));
                assert!(
                    (fits.plot.height - (MIN_PLOT + 0.01)).abs() < 1e-6,
                    "{style:?}: {:?}",
                    fits.plot
                );
                // Just fails, naming the height needed.
                let short = need - 0.01;
                let err = overflow(&full(&theme, style, short / 72.0));
                assert_eq!(
                    err.to_string(),
                    format!(
                        "Figure.to_scene(): the figure is {} pt tall; at least {} pt is needed \
                         for the plot area.",
                        short.floor(),
                        need.ceil().max(short.floor() + 1.0),
                    )
                );
                let hint = match style {
                    LegendStyle::Header | LegendStyle::Chips => {
                        "make the figure taller, or remove the title, subtitle or legend."
                    }
                    LegendStyle::Direct | LegendStyle::None => {
                        "make the figure taller, or remove the title or subtitle."
                    }
                };
                assert_eq!(err.hint(), Some(hint));
            }
        }
    }

    #[test]
    fn the_height_a_full_header_needs_is_named_exactly() {
        // A 6 in figure with a title, a subtitle, axis labels, automatic ticks and three
        // labelled lines in the theme's own legend style: a header row in pigment, direct
        // labels in graphite and chips in midnight. Under the header row or the chips, the
        // three entries fit one row.
        for (theme, need) in [
            (Theme::pigment(), 132.0),
            (Theme::graphite(), 114.0),
            (Theme::midnight(), 137.0),
        ] {
            let style = theme.legend_style();
            let err = overflow(&full(&theme, style, 1.0));
            assert_eq!(
                err.to_string(),
                format!(
                    "Figure.to_scene(): the figure is 72 pt tall; at least {need} pt is needed \
                     for the plot area."
                )
            );
            assert!(plan_of(&full(&theme, style, need / 72.0)).1.plot.height >= MIN_PLOT);
            assert_eq!(
                named_height(&overflow(&full(&theme, style, (need - 1.0) / 72.0))),
                Some(need)
            );
        }
    }

    #[test]
    fn ordinary_figures_two_inches_tall_lay_out() {
        use crate::LegendStyle;
        // (width in inches, a title and a subtitle, labelled lines): with the header, three
        // lines on a 6 in figure fit one legend row; without it, six lines on a 3 in figure
        // wrap onto two or three rows.
        let cases = [(6.0, true, 3), (3.0, false, 6), (6.0, false, 12)];
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            for style in [
                LegendStyle::Header,
                LegendStyle::Chips,
                LegendStyle::Direct,
                LegendStyle::None,
            ] {
                for (width, header, lines) in cases {
                    let mut f = Figure::new(width, 2.0, theme.clone()).unwrap();
                    let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
                    axes.xlabel("Day").ylabel("MW").legend(style);
                    if header {
                        axes.title("Station output").subtitle("Daily mean, MW");
                    }
                    for i in 0..lines {
                        axes.line(
                            AxisData::Numbers(vec![0.0, 30.0]),
                            &[i as f64, 100.0 - i as f64],
                            LineOptions {
                                label: format!("Station {i}"),
                                ..LineOptions::default()
                            },
                        )
                        .unwrap();
                    }
                    let (_, layout) = plan_of(&f);
                    assert!(layout.plot.height >= MIN_PLOT, "{:?}", layout.plot);
                    if matches!(style, LegendStyle::Header | LegendStyle::Chips) && !header {
                        // At least two rows: taller than two body lines.
                        let line = BODY_LINE * f64::from(theme.body_size());
                        assert!(layout.legend.height() > 2.0 * line, "{style:?} wraps");
                    }
                }
            }
        }
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]

        /// End to end: a figure either renders with a plot at least `MIN_PLOT` tall that lies
        /// between the paddings, or fails; when it fails for its height, a figure as tall as the
        /// height it names does not.
        #[test]
        fn no_successful_layout_has_a_plot_shorter_than_its_minimum(
            theme in 0_usize..3,
            scale in proptest::sample::select(vec![0.5_f64, 1.0, 2.0, 4.0]),
            width in 1.0..8.0_f64,
            height in 0.05..3.0_f64,
            parts in proptest::collection::vec(proptest::bool::ANY, 6),
            entries in 0_usize..16,
            style in 0_usize..5,
            chars in 1_usize..30,
        ) {
            use crate::LegendStyle;
            let build = |height: f64| {
                let theme = [Theme::pigment(), Theme::graphite(), Theme::midnight()][theme]
                    .with_font_scale(scale)
                    .unwrap();
                let mut f = Figure::new(width, height, theme).unwrap();
                let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
                if parts[0] {
                    axes.title("Station output");
                }
                if parts[1] {
                    axes.subtitle("Daily mean, MW");
                }
                if parts[2] {
                    axes.xlabel("Day");
                }
                if parts[3] {
                    axes.ylabel("MW");
                }
                if parts[4] {
                    axes.xticks(&[]).unwrap();
                }
                if parts[5] {
                    axes.yticks(&[]).unwrap();
                }
                let styles = [
                    LegendStyle::Header,
                    LegendStyle::Chips,
                    LegendStyle::Direct,
                    LegendStyle::None,
                ];
                if let Some(&style) = styles.get(style) {
                    axes.legend(style);
                }
                for i in 0..entries {
                    axes.line(
                        AxisData::Numbers(vec![0.0, 1.0]),
                        &[i as f64, 1.0],
                        LineOptions {
                            label: "W".repeat(chars),
                            ..LineOptions::default()
                        },
                    )
                    .unwrap();
                }
                f
            };
            let f = build(height);
            let canvas = height * 72.0;
            let planned = crate::pipeline::try_plan_figure(&f, (0, 0), lib());
            let scene = f.to_scene(lib());
            proptest::prop_assert_eq!(planned.is_ok(), scene.is_ok());
            match planned {
                Ok((_, layout)) => {
                    proptest::prop_assert!(layout.plot.height >= MIN_PLOT, "{:?}", layout.plot);
                    proptest::prop_assert!(layout.plot.y >= PADDING, "{:?}", layout.plot);
                    proptest::prop_assert!(
                        layout.plot.bottom() <= canvas - PADDING + 1e-9,
                        "{:?}",
                        layout.plot
                    );
                    assert_scene_bounded(&scene.unwrap());
                }
                Err(err) => {
                    proptest::prop_assert!(matches!(err, crate::Error::Style { .. }), "{}", err);
                    let message = err.to_string();
                    if let Some(needed) = named_height(&err) {
                        proptest::prop_assert!(needed > canvas.floor(), "{message}");
                        let taller = crate::pipeline::try_plan_figure(
                            &build(needed / 72.0),
                            (0, 0),
                            lib(),
                        );
                        if let Err(again) = taller {
                            proptest::prop_assert!(
                                !again.to_string().contains(" pt tall; "),
                                "{message} then {again}"
                            );
                        }
                    }
                }
            }
        }
    }
}
