//! Panel legend positions and deterministic placement inside the plot.

use super::inset::CONTAINMENT_TOLERANCE;
use super::{Area, Frame, Layout, LayoutInput, LegendRow, MeasuredLegend, legend_too_wide, widths};
use crate::resolve::ResolvedMark;
use crate::{Error, LegendPosition, Result};

/// The gap from the plot's right edge to a column legend, in points.
pub(super) const RIGHT_GAP: f64 = 12.0;
/// The gap below the x tick labels or x axis title, in points.
pub(super) const BOTTOM_GAP: f64 = 8.0;
/// Both the padding inside an automatic legend and its inset from the plot edges, in points.
pub(super) const BEST_PADDING: f64 = 6.0;

/// Eight candidates in tie order, never the centre. The caller checks that the box fits.
fn candidates(plot: &Area, width: f64, height: f64) -> [Frame; 8] {
    let left = plot.x + BEST_PADDING;
    let right = plot.right() - BEST_PADDING - width;
    let middle_x = plot.x + (plot.width - width) / 2.0;
    let top = plot.y + BEST_PADDING;
    let bottom = plot.bottom() - BEST_PADDING - height;
    let middle_y = plot.y + (plot.height - height) / 2.0;
    [
        (right, top),
        (left, top),
        (right, bottom),
        (left, bottom),
        (middle_x, top),
        (right, middle_y),
        (middle_x, bottom),
        (left, middle_y),
    ]
    .map(|(x, y)| Frame {
        x,
        y,
        width,
        height,
    })
}

/// Places the measured column at the least covered candidate, with its padded background.
/// Geometry is prepared once after layout, not once per candidate or layout pass.
pub(crate) fn best(
    legend: &MeasuredLegend<'_>,
    plot: &Area,
    marks: &[ResolvedMark<'_>],
) -> LegendRow {
    let column = legend.column(0.0);
    if column.is_empty() {
        return column;
    }
    let width = column.width() + 2.0 * BEST_PADDING;
    let height = column.height() + 2.0 * BEST_PADDING;
    let boxes = candidates(plot, width, height);
    let mut winner = boxes[0];
    let mut least = f64::INFINITY;
    for box_ in boxes {
        let score = marks
            .iter()
            .fold(0.0, |score, mark| score + mark.coverage(plot, &box_));
        // Translating the same filled overlap can change its last few floating-point bits.
        // Such differences are ties, not data that should move the legend.
        if score < least - 1e-9 {
            least = score;
            winner = box_;
        }
    }
    let mut row = legend
        .column(winner.x + BEST_PADDING)
        .placed(winner.y + BEST_PADDING);
    row.background = Some(Area {
        x: winner.x,
        y: winner.y,
        width,
        height,
    });
    row
}

/// Whether the inside column, its padding and its inset fit the final plot.
pub(crate) fn best_fits(legend: &MeasuredLegend<'_>, plot: &Area) -> bool {
    let column = legend.column(0.0);
    column.is_empty()
        || (column.width() + 4.0 * BEST_PADDING <= plot.width + CONTAINMENT_TOLERANCE
            && column.height() + 4.0 * BEST_PADDING <= plot.height + CONTAINMENT_TOLERANCE)
}

/// Checks every new placement even on a single panel. The legacy top-left row retains canvas
/// clipping on a single cell; on a grid all entries, including chip padding, must fit.
pub(super) fn check(input: &LayoutInput<'_>, layout: &Layout, single: bool) -> Result<()> {
    let position = input.legend.position;
    if layout.legend.is_empty() || (single && position == LegendPosition::TopLeft) {
        return Ok(());
    }
    let plot = layout.plot;
    let width = match position {
        LegendPosition::TopLeft | LegendPosition::TopRight | LegendPosition::Bottom => plot.width,
        LegendPosition::Right => layout.frame.right() - (plot.right() + RIGHT_GAP),
        LegendPosition::Best => plot.width - 4.0 * BEST_PADDING,
    };
    for entry in layout.legend.entries() {
        let start = entry.chip.map_or(entry.swatch.x, |c| c.x);
        let end = entry
            .chip
            .map_or(entry.label.x + entry.label.width(), |c| c.right());
        if end - start > width + CONTAINMENT_TOLERANCE {
            let room = width - (end - start - entry.label.width());
            return Err(legend_too_wide(input.context, &entry.label, room, "panel"));
        }
    }
    let height = layout.legend.height();
    let room = match position {
        LegendPosition::Right => plot.height,
        LegendPosition::Best => plot.height - 4.0 * BEST_PADDING,
        LegendPosition::TopLeft | LegendPosition::TopRight | LegendPosition::Bottom => {
            return Ok(());
        }
    };
    if height > room + CONTAINMENT_TOLERANCE {
        let (height, room) = widths(height, room);
        return Err(Error::style(
            input.context,
            format!("the legend is {height} pt tall; the plot has {room} pt."),
        )
        .with_hint("make the figure taller, or use fewer legend entries."));
    }
    Ok(())
}
