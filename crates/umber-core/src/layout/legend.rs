//! Legend layout: measuring and wrapping the legend row, the geometry of chips, and the space
//! reserved on the right for direct labels.
//!
//! The legend row sits in the header, under the subtitle, left-aligned with the title. Its
//! entries are laid out left to right and wrap onto further rows within the inner width (the
//! canvas inside the outer padding); an entry wider than a whole row gets a row of its own and
//! is clipped by the canvas. All rows have the height of the tallest entry. The row depends only
//! on the canvas width, so it is measured once and then placed by each layout pass.
//!
//! A plain entry is its swatch and its label. A chip is a pill as tall as the row, holding a dot
//! and the label inside a horizontal padding. Both styles wrap the same way: an entry box is
//! the swatch, the label and the padding around them, and only the gap between boxes differs.
//!
//! Direct labels sit right of the point each labels, its anchor, after the end dot drawn there
//! when the line ends at the anchor; a line that leaves the plot at its anchor has no end dot.
//! The plot area gives up room on its right for them (see [`DirectLabels::reserve`]), and
//! labels that would come too close vertically are moved apart (see [`super::nudge`]). Their
//! anchors move with the axes, so they are placed in every layout pass.

use super::nudge::separate;
use super::{Area, BODY_LINE, Label, PADDING, Styles, baseline_in_line, measure, metrics};
use crate::legend::{Legend, RowStyle, Swatch};
use crate::resolve::{Anchor, AnchorX, AxisScale, EndKind, NormalizedPos, Resolved, ResolvedAxis};
use crate::{FontLibrary, Rgba, TextStyle, Theme, Weight};

/// The gap between a swatch and its label, in both row styles.
const SWATCH_GAP: f64 = 4.0;
/// The gap between two plain entries in a row.
const ENTRY_GAP: f64 = 12.0;
/// The gap between two chips in a row.
const CHIP_GAP: f64 = 6.0;
/// The padding left of a chip's dot and right of its label.
const CHIP_PADDING_X: f64 = 7.0;
/// The padding above and below a chip's line of text.
const CHIP_PADDING_Y: f64 = 3.0;
/// The diameter of a chip's dot, as a share of the label size.
const CHIP_DOT: f64 = 0.6;
/// The vertical gap between wrapped rows.
const ROW_GAP: f64 = 4.0;
/// The gap between the title or subtitle and the legend row.
pub(super) const ROW_TOP_GAP: f64 = 6.0;
/// The length of a line swatch, in line widths.
const LINE_SWATCH_LENGTH: f64 = 10.0;
/// The side of a square swatch, as a share of the label size.
const SQUARE_SWATCH_SIDE: f64 = 0.8;
/// The gap between a line's end dot and its direct label.
const DIRECT_GAP: f64 = 6.0;
/// The smallest distance between the centers of two direct labels, as a multiple of the label
/// size.
const DIRECT_SPACING: f64 = 1.2;
/// The top of row `row` (counted from 0) of rows `row_height` tall, relative to the first
/// row's top.
fn row_top(row: usize, row_height: f64) -> f64 {
    row as f64 * (row_height + ROW_GAP)
}

/// A placed swatch: what it shows, in which color, and its box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SwatchBox {
    /// The swatch kind.
    pub kind: Swatch,
    /// The series color, alpha included.
    pub color: Rgba,
    /// The left edge.
    pub x: f64,
    /// The vertical center.
    pub center_y: f64,
    /// The width: the length of a line, the diameter of a dot, the side of a square.
    pub width: f64,
    /// The height: the line width of a line, the diameter of a dot, the side of a square.
    pub height: f64,
}

/// One entry of the legend row: its swatch and its label, and in a chips row the pill around
/// them.
#[derive(Debug, Clone)]
pub(crate) struct RowEntry {
    /// The swatch; in a chips row always a dot.
    pub swatch: SwatchBox,
    /// The label, in the body style, to the right of the swatch.
    pub label: Label,
    /// The box of the pill, as tall as the row, in a chips row; `None` in a plain row.
    pub chip: Option<Area>,
    /// The row the entry is on, counted from 0.
    row: usize,
}

/// The legend row: its entries, placed, on one or more rows of equal height. Empty, with
/// height 0, when the legend has no row entries.
#[derive(Debug, Clone, Default)]
pub(crate) struct LegendRow {
    entries: Vec<RowEntry>,
    /// The height of one row.
    row_height: f64,
    /// The number of rows; 0 without entries.
    rows: usize,
}

impl LegendRow {
    /// The row entries of `legend`, measured in the body font and wrapped within the inner
    /// width of a `width` pt canvas, with the first row's top at 0.
    ///
    /// A plain entry is its swatch, a 4 pt gap and its label, and entries are 12 pt apart.
    /// Swatches are sized as the marks are drawn: a line `10 ×` the drawn line width long and
    /// that width tall, a dot of the drawn marker diameter, a square of `0.8 ×` the label size.
    /// Plain rows are one body line tall (`1.35 ×` the label size), or as tall as the tallest
    /// swatch.
    ///
    /// A chip is a pill one body line plus `2 × 3` pt tall, holding a dot of `0.6 ×` the label
    /// size, a 4 pt gap and the label, with 7 pt of padding at each end; chips are 6 pt apart.
    ///
    /// Rows are 4 pt apart in both styles, and swatches and labels are centered in them.
    pub fn measure(width: f64, theme: &Theme, legend: &Legend, lib: &FontLibrary) -> LegendRow {
        if legend.row().is_empty() {
            return LegendRow::default();
        }
        let style = Styles::new(theme).body;
        let size = f64::from(style.size);
        let (ascent, descent) = metrics(lib, &style);
        let row_style = legend.row_style();
        let (padding_x, entry_gap, line) = match row_style {
            RowStyle::Plain => (0.0, ENTRY_GAP, BODY_LINE * size),
            RowStyle::Chips => (
                CHIP_PADDING_X,
                CHIP_GAP,
                BODY_LINE * size + 2.0 * CHIP_PADDING_Y,
            ),
        };
        let mut entries: Vec<RowEntry> = legend
            .row()
            .iter()
            .map(|entry| {
                let (kind, w, h) = swatch_size(theme, row_style, entry.swatch(), size);
                RowEntry {
                    swatch: SwatchBox {
                        kind,
                        color: entry.color(),
                        x: 0.0,
                        center_y: 0.0,
                        width: w,
                        height: h,
                    },
                    label: measure(lib, entry.label(), style),
                    chip: None,
                    row: 0,
                }
            })
            .collect();

        let row_height = entries
            .iter()
            .map(|entry| entry.swatch.height)
            .fold(line, f64::max);
        let right = width - PADDING;
        let (mut x, mut row, mut row_used) = (PADDING, 0_usize, false);
        for entry in &mut entries {
            let (inset, entry_width) = entry_extent(row_style, padding_x, row_height, entry);
            if row_used && x + entry_width > right {
                row += 1;
                x = PADDING;
            }
            let top = row_top(row, row_height);
            entry.row = row;
            entry.swatch.x = x + inset;
            entry.swatch.center_y = top + row_height / 2.0;
            entry.label.x = entry.swatch.x + entry.swatch.width + SWATCH_GAP;
            entry.label.y = baseline_in_line(top, row_height, ascent, descent);
            entry.chip = (row_style == RowStyle::Chips).then_some(Area {
                x,
                y: top,
                width: entry_width,
                height: row_height,
            });
            x += entry_width + entry_gap;
            row_used = true;
        }
        LegendRow {
            entries,
            row_height,
            rows: row + 1,
        }
    }

    /// This row moved down so that its first row's top is at `top`. Entries on rows whose top
    /// would be at or below `canvas_height` are dropped: they could not be seen, and dropping
    /// them keeps every coordinate near the canvas however many entries there are. The height
    /// stays that of all rows.
    pub(super) fn placed(&self, top: f64, canvas_height: f64) -> LegendRow {
        let entries = self
            .entries
            .iter()
            .filter(|entry| top + row_top(entry.row, self.row_height) < canvas_height)
            .map(|entry| {
                let mut entry = entry.clone();
                entry.swatch.center_y += top;
                entry.label.y += top;
                if let Some(chip) = &mut entry.chip {
                    chip.y += top;
                }
                entry
            })
            .collect();
        LegendRow { entries, ..*self }
    }

    /// The entries, in series order.
    pub fn entries(&self) -> &[RowEntry] {
        &self.entries
    }

    /// The height of all rows together, gaps included; 0 when there are no entries.
    pub fn height(&self) -> f64 {
        match self.rows.checked_sub(1) {
            Some(last) => row_top(last, self.row_height) + self.row_height,
            None => 0.0,
        }
    }

    /// Whether the row has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The legend of one axes measured for one canvas width: its row and its direct labels. It is
/// measured once, and each layout pass places it.
#[derive(Debug, Clone, Default)]
pub(crate) struct MeasuredLegend<'a> {
    /// The legend row, with its first row's top at 0.
    pub row: LegendRow,
    /// The direct labels, not yet placed.
    pub direct: DirectLabels<'a>,
}

impl<'a> MeasuredLegend<'a> {
    /// The row and the direct labels of `legend`, measured for a `width` pt canvas; see
    /// [`LegendRow::measure`] and [`DirectLabels::measure`].
    pub fn measure(
        width: f64,
        theme: &Theme,
        legend: &Legend<'a>,
        lib: &FontLibrary,
    ) -> MeasuredLegend<'a> {
        if legend.is_empty() {
            return MeasuredLegend::default();
        }
        MeasuredLegend {
            row: LegendRow::measure(width, theme, legend, lib),
            direct: DirectLabels::measure(theme, legend, lib),
        }
    }
}

/// One direct label, measured: its text, its line color and its anchor.
#[derive(Debug, Clone)]
struct MeasuredDirect<'a> {
    label: Label,
    color: Rgba,
    anchor: Anchor<'a>,
}

/// The direct labels of a legend, measured but not placed, in series order, with the sizes
/// that placing them needs. Empty when the legend has no direct labels.
#[derive(Debug, Clone, Default)]
pub(crate) struct DirectLabels<'a> {
    entries: Vec<MeasuredDirect<'a>>,
    /// The label size.
    size: f64,
    /// The ascent of the label style's primary font.
    ascent: f64,
    /// The descent of the label style's primary font.
    descent: f64,
    /// The diameter of the end dots: the drawn marker diameter.
    dot: f64,
}

impl<'a> DirectLabels<'a> {
    /// The direct labels of `legend`, measured in the body font at `SemiBold` weight and the
    /// body size, with end dots of the drawn marker diameter.
    pub fn measure(theme: &Theme, legend: &Legend<'a>, lib: &FontLibrary) -> DirectLabels<'a> {
        if legend.direct().is_empty() {
            return DirectLabels::default();
        }
        let style = TextStyle {
            weight: Weight::SemiBold,
            ..Styles::new(theme).body
        };
        let (ascent, descent) = metrics(lib, &style);
        let entries = legend
            .direct()
            .iter()
            .map(|entry| MeasuredDirect {
                label: measure(lib, entry.label(), style),
                color: entry.color(),
                anchor: entry.anchor(),
            })
            .collect();
        DirectLabels {
            entries,
            size: f64::from(style.size),
            ascent,
            descent,
            dot: f64::from(theme.drawn_marker_size()),
        }
    }

    /// The room the labels need right of the plot area, for a label whose anchor is on the
    /// plot's right edge: half the end dot, the 6 pt gap and the widest label, but at most
    /// `cap`. 0 without labels. A label wider than the room it gets is clipped by the canvas.
    pub(super) fn reserve(&self, cap: f64) -> f64 {
        let widest = self
            .entries
            .iter()
            .map(|e| e.label.width())
            .reduce(f64::max);
        widest.map_or(0.0, |w| (self.dot / 2.0 + DIRECT_GAP + w).min(cap))
    }

    /// The labels placed for the axes `resolved` on the plot area `plot` of a canvas
    /// `canvas_height` pt tall, in series order.
    ///
    /// Each anchor is placed on the plot, its x clamped to the plot's horizontal range. Its end
    /// dot is centered on it when the line ends there ([`EndKind::Point`]) and it lies inside
    /// the plot's vertical range; a dot outside would mark a point the plot does not show, and
    /// a line that leaves the plot at its anchor ([`EndKind::Edge`]) does not end there, so
    /// neither gets one. The label starts half a dot and
    /// 6 pt right of the anchor, and is vertically centered on the anchor's y, clamped to the
    /// plot's vertical range and then moved apart from its neighbours by [`separate`], so that
    /// label centers are at least `1.2 ×` the label size apart. Labels that would not fit
    /// between the plot's top and bottom are stacked around their mean target instead; a label
    /// whose line of text then lies wholly off the canvas is left out, and its dot is kept. An
    /// entry with neither a dot nor a label is left out.
    pub(super) fn placed(
        &self,
        resolved: &Resolved,
        plot: Area,
        canvas_height: f64,
    ) -> Vec<DirectLabel> {
        // Only an anchor that the axes cannot place (impossible after resolve) has a NaN
        // position; its label is left out.
        let anchors: Vec<(&MeasuredDirect<'a>, f64, f64)> = self
            .entries
            .iter()
            .filter_map(|entry| {
                let x = x_position(&resolved.x, entry.anchor.x);
                let y = y_position(&resolved.y, entry.anchor.y);
                (!x.is_nan() && !y.is_nan()).then_some((entry, x.clamp(0.0, 1.0), y))
            })
            .collect();
        let at = |y: f64| plot.y_at(NormalizedPos::new(y.clamp(0.0, 1.0)));
        let targets: Vec<f64> = anchors.iter().map(|&(_, _, y)| at(y)).collect();
        let min_gap = DIRECT_SPACING * self.size;
        let centers = separate(&targets, min_gap, plot.y, plot.bottom());
        let line = BODY_LINE * self.size;
        anchors
            .into_iter()
            .zip(centers)
            .filter_map(|((entry, x, y), center)| {
                let anchor_x = plot.x_at(NormalizedPos::new(x));
                let on_canvas = center + line / 2.0 > 0.0 && center - line / 2.0 < canvas_height;
                let label = on_canvas.then(|| {
                    let mut label = entry.label.clone();
                    label.x = anchor_x + self.dot / 2.0 + DIRECT_GAP;
                    label.y =
                        baseline_in_line(center - line / 2.0, line, self.ascent, self.descent);
                    label
                });
                let ends = entry.anchor.end == EndKind::Point;
                let dot = (ends && (0.0..=1.0).contains(&y)).then(|| EndDot {
                    x: anchor_x,
                    y: at(y),
                    diameter: self.dot,
                });
                (label.is_some() || dot.is_some()).then_some(DirectLabel {
                    label,
                    color: entry.color,
                    dot,
                })
            })
            .collect()
    }
}

/// The normalized x position of an anchor at `x` on `axis`; NaN when the axis cannot place it.
fn x_position(axis: &ResolvedAxis, x: AnchorX<'_>) -> f64 {
    match (&axis.scale, x) {
        (AxisScale::Continuous(scale), AnchorX::Number(v)) => scale.normalize(v),
        (AxisScale::Category(scale), AnchorX::Category(name)) => {
            scale.index_of(name).map_or(f64::NAN, |i| scale.center(i))
        }
        (AxisScale::Continuous(_), AnchorX::Category(_))
        | (AxisScale::Category(_), AnchorX::Number(_)) => f64::NAN,
    }
}

/// The normalized y position of an anchor at `y` on `axis`; NaN when the axis cannot place it.
fn y_position(axis: &ResolvedAxis, y: f64) -> f64 {
    match &axis.scale {
        AxisScale::Continuous(scale) => scale.normalize(y),
        AxisScale::Category(_) => f64::NAN,
    }
}

/// A direct label, placed: its label in its line color, and the end dot at its anchor. At
/// least one of the two is present.
#[derive(Debug, Clone)]
pub(crate) struct DirectLabel {
    /// The label, right of the anchor, vertically centered on its nudged position; `None` when
    /// it would lie wholly off the canvas.
    pub label: Option<Label>,
    /// The line color, alpha included, for the label and the dot.
    pub color: Rgba,
    /// The end dot; `None` when the line leaves the plot at the anchor or the anchor lies above
    /// or below the plot area.
    pub dot: Option<EndDot>,
}

/// A filled dot at the end of a directly labelled line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EndDot {
    /// The center's x.
    pub x: f64,
    /// The center's y.
    pub y: f64,
    /// The diameter.
    pub diameter: f64,
}

/// The horizontal inset of `entry`'s swatch from the left edge of its box, and the width of
/// that box, in a row of `row_style` whose rows are `row_height` tall and whose boxes have
/// `padding_x` at each end.
///
/// A box is the padding, the swatch, the gap, the label and the padding again. A chip is never
/// narrower than it is tall, so that its ends stay half circles: a chip whose dot and label are
/// too short is widened to the row height, with its dot and label centered in it, which leaves
/// more than the padding at each end.
fn entry_extent(
    row_style: RowStyle,
    padding_x: f64,
    row_height: f64,
    entry: &RowEntry,
) -> (f64, f64) {
    let width = padding_x + entry.swatch.width + SWATCH_GAP + entry.label.width() + padding_x;
    match row_style {
        RowStyle::Chips if width < row_height => {
            let content = entry.swatch.width + SWATCH_GAP + entry.label.width();
            ((row_height - content) / 2.0, row_height)
        }
        RowStyle::Chips | RowStyle::Plain => (padding_x, width),
    }
}

/// The swatch a row of `row_style` draws for an entry with `swatch`, and its width and height
/// for a label `size` pt tall.
///
/// A plain row keeps the entry's swatch at its drawn size, so each box is exactly what the
/// drawing fills or strokes; the drawn sizes are finite and capped, and so is the label size.
/// A chips row always draws a dot of `0.6 ×` the label size.
fn swatch_size(
    theme: &Theme,
    row_style: RowStyle,
    swatch: Swatch,
    size: f64,
) -> (Swatch, f64, f64) {
    match (row_style, swatch) {
        (RowStyle::Chips, _) => {
            let d = CHIP_DOT * size;
            (Swatch::Dot, d, d)
        }
        (RowStyle::Plain, Swatch::Line { .. }) => {
            let line_width = f64::from(theme.drawn_line_width());
            (swatch, LINE_SWATCH_LENGTH * line_width, line_width)
        }
        (RowStyle::Plain, Swatch::Dot) => {
            let d = f64::from(theme.drawn_marker_size());
            (swatch, d, d)
        }
        (RowStyle::Plain, Swatch::Square) => {
            let side = SQUARE_SWATCH_SIDE * size;
            (swatch, side, side)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, Figure, LineOptions};
    use crate::pipeline::plan_figure;
    use crate::resolve::{AxisScale, ResolvedAxis};
    use crate::scene::Item;
    use crate::test_support::scene::assert_scene_bounded;
    use crate::{ScaleKind, Theme};

    /// A `w` by `h` in figure in `theme` with a title, a subtitle and a y label, and one line
    /// per label.
    fn figure(w: f64, h: f64, theme: Theme, labels: &[String]) -> Figure {
        let mut f = Figure::new(w, h, theme).unwrap();
        let axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.title("Station output")
            .subtitle("Daily mean, MW")
            .ylabel("MW");
        for label in labels {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: label.clone(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        f
    }

    fn names(n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("Station {i}")).collect()
    }

    fn rows_of(row: &LegendRow) -> Vec<usize> {
        row.entries().iter().map(|e| e.row).collect()
    }

    #[test]
    fn row_sits_under_the_subtitle_left_aligned_with_the_title() {
        let f = figure(6.0, 4.0, Theme::pigment(), &names(3));
        let layout = plan_figure(&f, FontLibrary::global()).1;
        let (title, subtitle) = (layout.title.unwrap(), layout.subtitle.unwrap());
        let row = &layout.legend;
        assert_eq!(rows_of(row), [0, 0, 0]);
        let first = &row.entries()[0];
        assert_eq!(first.swatch.x, title.x);
        assert!(first.label.y > subtitle.y);
        // The y label and the plot sit below the row.
        let ylabel = layout.ylabel.unwrap();
        let row_bottom = first.swatch.center_y - row.row_height / 2.0 + row.height();
        assert!(row_bottom < ylabel.y - f64::from(ylabel.layout.ascent()));
        assert!(row_bottom < layout.plot.y);
        assert!((layout.header_bottom - (row_bottom + 8.0)).abs() < 1e-9);
        // Entries keep their order, swatch, gap and label left to right.
        for pair in row.entries().windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!((a.label.x - (a.swatch.x + a.swatch.width + SWATCH_GAP)).abs() < 1e-9);
            let a_end = a.label.x + a.label.width();
            assert!((b.swatch.x - (a_end + ENTRY_GAP)).abs() < 1e-9);
        }
    }

    #[test]
    fn row_wraps_within_inner_width() {
        let theme = Theme::pigment();
        let width = 6.0 * 72.0;
        for n in [1, 3, 20, 60] {
            let f = figure(6.0, 4.0, theme.clone(), &names(n));
            let row = plan_figure(&f, FontLibrary::global()).1.legend;
            assert_eq!(row.entries().len(), n);
            let rows = rows_of(&row);
            // Rows only ever advance, by one at a time.
            assert!(rows.windows(2).all(|p| p[1] == p[0] || p[1] == p[0] + 1));
            for entry in row.entries() {
                assert!(entry.swatch.x >= PADDING);
                assert!(entry.label.x + entry.label.width() <= width - PADDING + 1e-9);
            }
            // Every entry that starts a row would not have fitted at the end of the row above.
            for pair in row.entries().windows(2) {
                if pair[1].row > pair[0].row {
                    let end = pair[0].label.x + pair[0].label.width() + ENTRY_GAP;
                    let w = pair[1].swatch.width + SWATCH_GAP + pair[1].label.width();
                    assert!(end + w > width - PADDING, "{n}: {end} + {w}");
                    assert_eq!(pair[1].swatch.x, PADDING);
                }
            }
            let last = rows.last().copied().unwrap_or(0);
            let expected = (last + 1) as f64 * row.row_height + last as f64 * ROW_GAP;
            assert!((row.height() - expected).abs() < 1e-9);
            if n == 20 {
                assert!(last >= 2, "20 entries wrap onto several rows");
            }
        }
    }

    /// The box of `entry`: its chip, or for a plain entry the swatch and label together.
    fn entry_box(entry: &RowEntry) -> (f64, f64) {
        match entry.chip {
            Some(chip) => (chip.x, chip.right()),
            None => (entry.swatch.x, entry.label.x + entry.label.width()),
        }
    }

    #[test]
    fn chips_wrap_like_the_header_row() {
        let width = 6.0 * 72.0;
        let chips_theme = Theme::midnight();
        assert_eq!(chips_theme.legend_style(), crate::LegendStyle::Chips);
        let header_theme = chips_theme
            .clone()
            .with_legend_style(crate::LegendStyle::Header)
            .unwrap();
        let size = f64::from(chips_theme.body_size() * chips_theme.font_scale());
        for n in [1, 3, 20, 60] {
            let chips = plan_figure(
                &figure(6.0, 4.0, chips_theme.clone(), &names(n)),
                FontLibrary::global(),
            )
            .1
            .legend;
            let header = plan_figure(
                &figure(6.0, 4.0, header_theme.clone(), &names(n)),
                FontLibrary::global(),
            )
            .1
            .legend;
            assert_eq!(chips.entries().len(), n);
            assert!(header.entries().iter().all(|e| e.chip.is_none()));
            // A chip is the pill around its dot and label, one body line plus 2 × 3 pt tall.
            assert_eq!(chips.row_height, BODY_LINE * size + 2.0 * CHIP_PADDING_Y);
            for entry in chips.entries() {
                let chip = entry.chip.unwrap();
                let top = row_top(entry.row, chips.row_height) + chips.entries()[0].chip.unwrap().y;
                assert!((chip.y - top).abs() < 1e-9 && chip.height == chips.row_height);
                assert_eq!(entry.swatch.kind, Swatch::Dot);
                assert!((entry.swatch.width - CHIP_DOT * size).abs() < 1e-9);
                assert!((entry.swatch.x - (chip.x + CHIP_PADDING_X)).abs() < 1e-9);
                assert!(
                    (entry.label.x - (entry.swatch.x + entry.swatch.width + SWATCH_GAP)).abs()
                        < 1e-9
                );
                let label_end = entry.label.x + entry.label.width();
                assert!((chip.right() - (label_end + CHIP_PADDING_X)).abs() < 1e-9);
                assert!((entry.swatch.center_y - (chip.y + chip.height / 2.0)).abs() < 1e-9);
            }
            // The same wrapping rule for both styles, each with its own entry gap: rows only
            // advance by one, every box stays within the inner width, and an entry starts a
            // row only when it would not have fitted at the end of the row above.
            for (row, gap) in [(&chips, CHIP_GAP), (&header, ENTRY_GAP)] {
                let rows = rows_of(row);
                assert!(rows.windows(2).all(|p| p[1] == p[0] || p[1] == p[0] + 1));
                for pair in row.entries().windows(2) {
                    let ((_, a_end), (b_start, b_end)) = (entry_box(&pair[0]), entry_box(&pair[1]));
                    assert!(b_start >= PADDING && b_end <= width - PADDING + 1e-9);
                    if pair[1].row > pair[0].row {
                        assert!(a_end + gap + (b_end - b_start) > width - PADDING);
                        assert_eq!(b_start, PADDING);
                    } else {
                        assert!((b_start - (a_end + gap)).abs() < 1e-9);
                    }
                }
                let last = rows.last().copied().unwrap_or(0);
                let expected = (last + 1) as f64 * row.row_height + last as f64 * ROW_GAP;
                assert!((row.height() - expected).abs() < 1e-9);
            }
            if n == 20 {
                assert!(chips.rows >= 3, "20 chips wrap onto several rows");
            }
        }
        // A chip wider than a whole row gets a row of its own, as a header entry does.
        let long = "Long legend label ".repeat(17);
        let labels = ["A".to_owned(), long, "B".to_owned()];
        let row = plan_figure(
            &figure(6.0, 4.0, chips_theme, &labels),
            FontLibrary::global(),
        )
        .1
        .legend;
        assert_eq!(rows_of(&row), [0, 1, 2]);
        assert_eq!(row.entries()[1].chip.unwrap().x, PADDING);
    }

    #[test]
    fn a_short_chip_is_as_wide_as_it_is_tall() {
        // A one-character label at a large font scale: dot, gap and label are narrower than
        // the chip is tall.
        let theme = Theme::pigment()
            .with_legend_style(crate::LegendStyle::Chips)
            .unwrap()
            .with_font_scale(10.0)
            .unwrap();
        // No title, so the row starts at the top padding and stays on the canvas.
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: ".".to_owned(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        let row = plan_figure(&f, FontLibrary::global()).1.legend;
        let [short] = row.entries() else {
            panic!("one entry");
        };
        let chip = short.chip.unwrap();
        let content = short.swatch.width + SWATCH_GAP + short.label.width();
        assert!(content + 2.0 * CHIP_PADDING_X < chip.height);
        assert_eq!(chip.width, chip.height);
        // The dot and the label are centered, with more than the padding at each end.
        let left = short.swatch.x - chip.x;
        let right = chip.right() - (short.label.x + short.label.width());
        assert!((left - right).abs() < 1e-9 && left > CHIP_PADDING_X);
    }

    #[test]
    fn an_entry_wider_than_a_row_gets_its_own_row() {
        let long = "Long legend label ".repeat(17);
        let labels = ["A".to_owned(), long, "B".to_owned()];
        let f = figure(6.0, 4.0, Theme::pigment(), &labels);
        let row = plan_figure(&f, FontLibrary::global()).1.legend;
        assert_eq!(rows_of(&row), [0, 1, 2]);
        let wide = &row.entries()[1];
        assert_eq!(wide.swatch.x, PADDING);
        assert!(wide.label.x + wide.label.width() > 6.0 * 72.0);
    }

    #[test]
    fn swatch_sizes_follow_the_theme() {
        let theme = Theme::graphite()
            .with_legend_style(crate::LegendStyle::Header)
            .unwrap();
        let f = figure(6.0, 4.0, theme.clone(), &names(1));
        let row = plan_figure(&f, FontLibrary::global()).1.legend;
        let swatch = row.entries()[0].swatch;
        let line_width = f64::from(theme.line_width());
        assert!((swatch.width - 10.0 * line_width).abs() < 1e-9);
        assert!((swatch.height - line_width).abs() < 1e-9);
        assert_eq!(
            row.row_height,
            BODY_LINE * f64::from(theme.body_size() * theme.font_scale())
        );
    }

    #[test]
    fn many_entries_never_make_a_negative_plot() {
        for theme in [Theme::pigment(), Theme::graphite(), Theme::midnight()] {
            for scale in [1.0, 10.0] {
                let theme = theme.with_font_scale(scale).unwrap();
                for (w, h, n) in [(1.0, 1.0, 50), (6.0, 4.0, 400), (2.0, 1.0, 2000)] {
                    let f = figure(w, h, theme.clone(), &names(n));
                    let layout = plan_figure(&f, FontLibrary::global()).1;
                    let plot = layout.plot;
                    assert!(plot.width >= 1.0 && plot.height >= 1.0, "{plot:?}");
                    assert!(
                        plot.y >= 0.0 && plot.bottom() <= h * 72.0 + 1e-9,
                        "{plot:?}"
                    );
                    assert!(layout.legend.height().is_finite());
                    // Only rows that start on the canvas are kept.
                    for entry in layout.legend.entries() {
                        assert!(entry.swatch.center_y - layout.legend.row_height / 2.0 < h * 72.0);
                    }
                    assert_scene_bounded(&f.to_scene(FontLibrary::global()));
                }
            }
        }
    }

    /// The bounds of a path item: `(left, top, right, bottom)`.
    fn bounds(item: &Item) -> (f64, f64, f64, f64) {
        let path = match item {
            Item::Fill { path, .. } | Item::Stroke { path, .. } => path,
            Item::Markers { shape, .. } => shape,
            other => panic!("not a path item: {other:?}"),
        };
        let (min, max) = path.bounds().unwrap();
        let f = f64::from;
        (f(min.x), f(min.y), f(max.x), f(max.y))
    }

    /// A 6 by 4 in figure without a title, so its legend row starts at the top padding, with
    /// one labelled dashed line or one labelled scatter.
    fn huge_token_figure(theme: Theme, scatter: bool) -> Figure {
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        let axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let (x, y) = (AxisData::Numbers(vec![0.0, 1.0]), [1.0, 2.0]);
        if scatter {
            let options = crate::model::ScatterOptions {
                label: "Points".into(),
                ..crate::model::ScatterOptions::default()
            };
            axes.scatter(x, &y, options).unwrap();
        } else {
            let options = LineOptions {
                label: "Dashed".into(),
                dash: crate::Dash::Dashed,
                ..LineOptions::default()
            };
            axes.line(x, &y, options).unwrap();
        }
        f
    }

    /// The legend's one swatch box, and the marks and legend items of the scene, which must
    /// hold chrome, marks, legend and baseline groups.
    fn swatch_and_items(f: &Figure) -> (SwatchBox, Vec<Item>, Vec<Item>) {
        let layout = plan_figure(f, FontLibrary::global()).1;
        let [entry] = layout.legend.entries() else {
            panic!("one legend entry");
        };
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let [
            _,
            Item::Group { items: marks, .. },
            Item::Group { items: legend, .. },
            _,
        ] = scene.items.as_slice()
        else {
            panic!("chrome, marks, legend and baseline groups");
        };
        (entry.swatch, marks.clone(), legend.clone())
    }

    #[test]
    fn huge_swatch_tokens_match_the_drawing() {
        let (width, height) = (6.0 * 72.0, 4.0 * 72.0);
        // 12 000 is above the drawn cap; f32::MAX is the largest token a theme accepts.
        for token in [12_000.0, f64::from(f32::MAX)] {
            let theme = Theme::pigment()
                .with_line_width(token)
                .unwrap()
                .with_marker_size(token)
                .unwrap();
            let (stroke, diameter) = (
                f64::from(theme.drawn_line_width()),
                f64::from(theme.drawn_marker_size()),
            );
            assert_eq!((stroke, diameter), (1.0e4, 1.0e4));

            // The line swatch is exactly as wide as the stroke of the line mark.
            let (line, marks, legend) = swatch_and_items(&huge_token_figure(theme.clone(), false));
            assert_eq!((line.width, line.height), (10.0 * stroke, stroke));
            let stroke_of = |items: &[Item]| {
                items.iter().find_map(|item| match item {
                    Item::Stroke { style, .. } => Some(f64::from(style.width)),
                    _ => None,
                })
            };
            assert_eq!(stroke_of(&legend), Some(line.height));
            assert_eq!(stroke_of(&marks), Some(line.height));
            let (_, y0, _, y1) = bounds(&legend[0]);
            assert!(y0 == y1 && (y0 - line.center_y).abs() < 1e-3);

            // The dot swatch is exactly as wide as the scatter markers.
            let (dot, marks, legend) = swatch_and_items(&huge_token_figure(theme, true));
            assert_eq!((dot.width, dot.height), (diameter, diameter));
            let (x0, y0, x1, y1) = bounds(&legend[0]);
            assert!((x1 - x0 - dot.width).abs() < 1.0 && (y1 - y0 - dot.height).abs() < 1.0);
            assert!(((y0 + y1) / 2.0 - dot.center_y).abs() < 1.0);
            let markers = marks
                .iter()
                .find(|item| matches!(item, Item::Markers { .. }))
                .unwrap();
            let (x0, _, x1, _) = bounds(markers);
            assert!(
                (x1 - x0 - dot.width).abs() < 1.0,
                "{} vs {}",
                x1 - x0,
                dot.width
            );

            // Each box starts on the canvas, at the row top.
            for b in [line, dot] {
                let top = b.center_y - b.height / 2.0;
                assert!((top - PADDING).abs() < 1e-9, "{token}: {b:?}");
                assert!(b.x < width && top < height, "{token}: {b:?}");
            }
        }
    }

    /// A 6 by 4 in figure in `theme` with a title, a subtitle and a y label, and one labelled
    /// line per `(label, x, y)`.
    fn line_figure(theme: Theme, lines: &[(&str, Vec<f64>, Vec<f64>)]) -> Figure {
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        let axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.title("Station output")
            .subtitle("Daily mean, MW")
            .ylabel("MW");
        for (label, x, y) in lines {
            axes.line(
                AxisData::Numbers(x.clone()),
                y,
                LineOptions {
                    label: (*label).to_owned(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        f
    }

    /// The label of `d`, which must have one.
    fn text_of(d: &DirectLabel) -> &Label {
        d.label.as_ref().expect("a label on the canvas")
    }

    /// The vertical center of the line of text whose baseline is `label.y`.
    fn center_of(label: &Label) -> f64 {
        let (ascent, descent) = (
            f64::from(label.layout.ascent()),
            f64::from(label.layout.descent()),
        );
        label.y - (ascent - descent) / 2.0
    }

    #[test]
    fn direct_labels_sit_right_of_their_line_ends() {
        let theme = Theme::graphite();
        assert_eq!(theme.legend_style(), crate::LegendStyle::Direct);
        let x = vec![0.0, 1.0, 2.0];
        let f = line_figure(
            theme.clone(),
            &[
                ("North", x.clone(), vec![1.0, 2.0, 9.0]),
                ("South", x.clone(), vec![5.0, 4.0, 1.0]),
                // Ends early, in the middle of the plot.
                ("West", vec![0.0, 1.0], vec![5.0, 5.0]),
            ],
        );
        let (resolved, layout) = plan_figure(&f, FontLibrary::global());
        assert!(layout.legend.is_empty());
        let direct = &layout.direct;
        assert_eq!(direct.len(), 3);
        let plot = layout.plot;
        let size = f64::from(theme.body_size() * theme.font_scale());
        let dot = f64::from(theme.drawn_marker_size());
        let palette = theme.palette();
        for (k, (label, (text, x, y))) in direct
            .iter()
            .zip([("North", 2.0, 9.0), ("South", 2.0, 1.0), ("West", 1.0, 5.0)])
            .enumerate()
        {
            assert_eq!(text_of(label).text, text);
            assert_eq!(label.color, palette[k]);
            assert_eq!(text_of(label).style.weight, Weight::SemiBold);
            assert_eq!(f64::from(text_of(label).style.size), size);
            // The dot sits on the anchor, the label half a dot and 6 pt to its right, centered
            // on it: these ends are far apart, so nothing is nudged.
            let continuous = |axis: &ResolvedAxis, v: f64| match &axis.scale {
                AxisScale::Continuous(scale) => scale.normalize(v),
                AxisScale::Category(_) => panic!("a continuous axis"),
            };
            let anchor_x = plot.x_at(NormalizedPos::new(continuous(&resolved.x, x)));
            let anchor_y = plot.y_at(NormalizedPos::new(continuous(&resolved.y, y)));
            let end = label.dot.unwrap();
            assert_eq!((end.x, end.y, end.diameter), (anchor_x, anchor_y, dot));
            assert!((text_of(label).x - (anchor_x + dot / 2.0 + DIRECT_GAP)).abs() < 1e-9);
            assert!((center_of(text_of(label)) - anchor_y).abs() < 1e-3);
        }
        // The plot gives up the room of the widest label on its right.
        let widest = direct
            .iter()
            .map(|d| text_of(d).width())
            .fold(0.0, f64::max);
        let reserve = dot / 2.0 + DIRECT_GAP + widest;
        assert!((plot.right() - (6.0 * 72.0 - PADDING - reserve)).abs() < 1e-9);
        // The widest label of a line ending on the plot's right edge ends on the inner edge.
        let end = direct[..2]
            .iter()
            .map(|d| text_of(d).x + text_of(d).width())
            .fold(0.0, f64::max);
        let widest_at_edge = direct[..2]
            .iter()
            .map(|d| text_of(d).width())
            .fold(0.0, f64::max);
        assert_eq!(widest_at_edge, widest);
        assert!((end - (6.0 * 72.0 - PADDING)).abs() < 1e-9);
    }

    #[test]
    fn reservation_is_capped() {
        let width = 6.0 * 72.0;
        let cap = 0.25 * (width - 2.0 * PADDING);
        let long: String = "Long direct label ".repeat(17).chars().take(300).collect();
        let x = vec![0.0, 1.0];
        let f = line_figure(
            Theme::graphite(),
            &[
                (&long, x.clone(), vec![1.0, 2.0]),
                ("Short", x, vec![2.0, 1.0]),
            ],
        );
        let layout = plan_figure(&f, FontLibrary::global()).1;
        assert!((layout.plot.right() - (width - PADDING - cap)).abs() < 1e-9);
        // The long label runs past the canvas, which clips it.
        let long = text_of(&layout.direct[0]);
        assert!(long.x + long.width() > width);
        assert_scene_bounded(&f.to_scene(FontLibrary::global()));

        // A reservation below the cap is the room the labels need, and none is made without
        // direct labels.
        let measured = |labels: &[&str]| {
            let lines: Vec<(&str, Vec<f64>, Vec<f64>)> = labels
                .iter()
                .map(|&l| (l, vec![0.0, 1.0], vec![1.0, 2.0]))
                .collect();
            let f = line_figure(Theme::graphite(), &lines);
            let axes = f.axes().unwrap();
            let marks = crate::resolve::resolve_marks(axes.marks(), f.theme().palette());
            let limits = crate::resolve::Limits::of(axes);
            let legend = Legend::new(crate::LegendStyle::Direct, &marks, &limits);
            DirectLabels::measure(f.theme(), &legend, FontLibrary::global()).reserve(cap)
        };
        let short = measured(&["Short"]);
        assert!(short > DIRECT_GAP && short < cap);
        assert_eq!(measured(&[""]), 0.0);
        assert_eq!(measured(&[&"W".repeat(300)]), cap);
    }

    #[test]
    fn crossing_lines_are_nudged_apart_in_order() {
        let theme = Theme::graphite();
        let size = f64::from(theme.body_size() * theme.font_scale());
        let x = vec![0.0, 1.0];
        // Three lines crossing into the same end point, and a fourth just above it.
        let f = line_figure(
            theme,
            &[
                ("A", x.clone(), vec![0.0, 5.0]),
                ("B", x.clone(), vec![10.0, 5.0]),
                ("C", x.clone(), vec![3.0, 5.0]),
                ("D", x, vec![7.0, 5.1]),
            ],
        );
        let layout = plan_figure(&f, FontLibrary::global()).1;
        let centers: Vec<f64> = layout
            .direct
            .iter()
            .map(|d| center_of(text_of(d)))
            .collect();
        // Equal ends keep series order top down (y grows downwards); the higher end stays
        // above them.
        assert!(centers[3] < centers[0] && centers[0] < centers[1] && centers[1] < centers[2]);
        let mut sorted = centers.clone();
        sorted.sort_by(f64::total_cmp);
        for pair in sorted.windows(2) {
            assert!(pair[1] - pair[0] >= 1.2 * size - 1e-3, "{centers:?}");
        }
        for c in &centers {
            assert!(*c >= layout.plot.y - 1e-9 && *c <= layout.plot.bottom() + 1e-9);
        }
        // Dots stay on the line ends; only labels move.
        let dots: Vec<f64> = layout.direct.iter().map(|d| d.dot.unwrap().y).collect();
        assert_eq!(dots[0], dots[1]);
        assert_eq!(dots[1], dots[2]);
    }

    #[test]
    fn anchors_outside_the_y_limits_get_no_dot() {
        let x = vec![0.0, 1.0];
        let mut f = line_figure(
            Theme::graphite(),
            &[
                ("High", x.clone(), vec![0.5, 50.0]),
                ("Low", x.clone(), vec![0.5, -50.0]),
                ("Inside", x, vec![0.0, 0.5]),
            ],
        );
        f.axes_mut().unwrap().ylim(0.0, 1.0).unwrap();
        let layout = plan_figure(&f, FontLibrary::global()).1;
        let plot = layout.plot;
        let [high, low, inside] = layout.direct.as_slice() else {
            panic!("three direct labels");
        };
        assert!(high.dot.is_none() && low.dot.is_none());
        assert!(inside.dot.is_some());
        // Their labels are clamped to the plot's vertical range.
        assert!((center_of(text_of(high)) - plot.y).abs() < 1e-3);
        assert!((center_of(text_of(low)) - plot.bottom()).abs() < 1e-3);
        assert_scene_bounded(&f.to_scene(FontLibrary::global()));
    }

    #[test]
    fn a_line_cut_by_the_x_limits_is_labelled_where_it_leaves_the_plot() {
        // "Cut" goes on past x = 1.5 and is labelled where it crosses the right edge, at
        // y = 25, with no end dot; "Ends" stops inside and keeps its dot at its last point.
        let mut f = line_figure(
            Theme::graphite(),
            &[
                ("Cut", vec![0.0, 1.0, 2.0], vec![0.0, 20.0, 30.0]),
                ("Ends", vec![0.0, 1.0], vec![80.0, 90.0]),
            ],
        );
        f.axes_mut()
            .unwrap()
            .xlim(0.0, 1.5)
            .unwrap()
            .ylim(0.0, 100.0)
            .unwrap();
        let layout = plan_figure(&f, FontLibrary::global()).1;
        let plot = layout.plot;
        let [cut, ends] = layout.direct.as_slice() else {
            panic!("two direct labels");
        };
        assert!(cut.dot.is_none());
        let label = text_of(cut);
        let dot = f64::from(Theme::graphite().drawn_marker_size());
        assert!((label.x - (plot.right() + dot / 2.0 + DIRECT_GAP)).abs() < 1e-9);
        let crossing = plot.y_at(NormalizedPos::new(0.25));
        assert!((center_of(label) - crossing).abs() < 1e-6);
        let end = ends.dot.expect("the line ends inside, so it keeps its dot");
        assert!((end.x - plot.x_at(NormalizedPos::new(1.0 / 1.5))).abs() < 1e-9);
        assert!((end.y - plot.y_at(NormalizedPos::new(0.9))).abs() < 1e-9);
        assert_scene_bounded(&f.to_scene(FontLibrary::global()));

        // A line whose last point inside lies on the high end goes on past it, but that point
        // is data the plot shows: it keeps its dot, on the right edge.
        let mut f = line_figure(
            Theme::graphite(),
            &[("On the edge", vec![0.0, 1.0, 2.0], vec![10.0, 40.0, 30.0])],
        );
        f.axes_mut()
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .ylim(0.0, 100.0)
            .unwrap();
        let layout = plan_figure(&f, FontLibrary::global()).1;
        let [edge] = layout.direct.as_slice() else {
            panic!("one direct label");
        };
        let dot = edge.dot.expect("a data point on the edge keeps its dot");
        assert!((dot.x - layout.plot.right()).abs() < 1e-9);
        assert!((dot.y - layout.plot.y_at(NormalizedPos::new(0.4))).abs() < 1e-9);

        // On a log x axis the crossing is interpolated in log space: from (10, 0) to
        // (1000, 40), the edge at 100 lies halfway, at y = 20.
        let mut f = Figure::new(6.0, 4.0, Theme::graphite()).unwrap();
        f.add_axes(
            ScaleKind::Log {
                base: crate::LogBase::TEN,
            },
            ScaleKind::Linear,
        )
        .unwrap()
        .line(
            AxisData::Numbers(vec![1.0, 10.0, 1000.0]),
            &[0.0, 0.0, 40.0],
            LineOptions {
                label: "Log".to_owned(),
                ..LineOptions::default()
            },
        )
        .unwrap()
        .xlim(1.0, 100.0)
        .unwrap()
        .ylim(0.0, 100.0)
        .unwrap();
        let layout = plan_figure(&f, FontLibrary::global()).1;
        let [log] = layout.direct.as_slice() else {
            panic!("one direct label");
        };
        assert!(log.dot.is_none());
        let crossing = layout.plot.y_at(NormalizedPos::new(0.2));
        assert!((center_of(text_of(log)) - crossing).abs() < 1e-6);
    }

    #[test]
    fn many_direct_labels_stay_bounded() {
        for theme in [
            Theme::graphite(),
            Theme::graphite().with_font_scale(10.0).unwrap(),
        ] {
            for (w, h, n) in [(1.0, 1.0, 50), (6.0, 4.0, 400)] {
                let mut f = Figure::new(w, h, theme.clone()).unwrap();
                let axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
                axes.title("Crowded");
                for k in 0..n {
                    axes.line(
                        AxisData::Numbers(vec![0.0, 1.0]),
                        &[0.0, (k % 7) as f64],
                        LineOptions {
                            label: format!("Line {k}"),
                            ..LineOptions::default()
                        },
                    )
                    .unwrap();
                }
                let layout = plan_figure(&f, FontLibrary::global()).1;
                assert!(layout.plot.width >= 1.0 && layout.plot.height >= 1.0);
                // Labels whose line of text is off the canvas are left out; every end dot stays.
                assert_eq!(layout.direct.len(), n);
                assert!(layout.direct.iter().all(|d| d.dot.is_some()));
                for d in layout.direct.iter().filter_map(|d| d.label.as_ref()) {
                    let c = center_of(d);
                    assert!(c.is_finite() && c > -100.0 * 72.0 && c < 2.0 * h * 72.0 + 1e5);
                }
                assert_scene_bounded(&f.to_scene(FontLibrary::global()));
            }
        }
    }

    #[test]
    fn off_canvas_labels_keep_their_end_dots() {
        // Thirty lines ending inside the plot need far more height than the plot has, so the
        // stack of labels runs off the canvas at both ends.
        let theme = Theme::graphite();
        let size = f64::from(theme.body_size() * theme.font_scale());
        let lines: Vec<(String, Vec<f64>, Vec<f64>)> = (0..30)
            .map(|k| {
                (
                    format!("Series {k}"),
                    vec![0.0, 1.0],
                    vec![f64::from(k), 5.0],
                )
            })
            .collect();
        let refs: Vec<(&str, Vec<f64>, Vec<f64>)> = lines
            .iter()
            .map(|(l, x, y)| (l.as_str(), x.clone(), y.clone()))
            .collect();
        let f = line_figure(theme, &refs);
        let layout = plan_figure(&f, FontLibrary::global()).1;
        let height = 4.0 * 72.0;
        let direct = &layout.direct;
        assert_eq!(direct.len(), 30);
        // Some labels are left out ...
        let shown = direct.iter().filter(|d| d.label.is_some()).count();
        assert!(shown < 30, "{shown} labels shown");
        // ... but every line ending inside the plot keeps its dot.
        for d in direct {
            let dot = d.dot.expect("the anchor is inside the plot");
            assert!(dot.y >= layout.plot.y && dot.y <= layout.plot.bottom());
            // A label is shown only when its line of text reaches onto the canvas.
            if let Some(label) = &d.label {
                let (c, half) = (center_of(label), BODY_LINE * size / 2.0);
                assert!(c + half > 0.0 && c - half < height, "{c}");
            }
        }
        // Drawing follows: one dot per line, one label per shown label.
        let scene = f.to_scene(FontLibrary::global());
        assert_scene_bounded(&scene);
        let Some(Item::Group { items, .. }) = scene.items.get(2) else {
            panic!("a legend group");
        };
        let dots = items
            .iter()
            .filter(|i| matches!(i, Item::Fill { .. }))
            .count();
        let texts = items.iter().filter(|i| matches!(i, Item::Text(_))).count();
        assert_eq!((dots, texts), (30, shown));
    }
}
